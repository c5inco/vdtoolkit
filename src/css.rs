//! Wide-gamut CSS colors, rewritten as the sRGB the SVG parser can read.
//!
//! Design tools write a wide-gamut color as a `color()` function, usually with
//! an sRGB fallback ahead of it in the same declaration list:
//!
//! ```text
//! style="stroke:#6C707E;stroke:color(display-p3 0.4235 0.4392 0.4941);"
//! ```
//!
//! A browser keeps the fallback when it cannot read the `color()` function.
//! The SVG parser instead drops the property outright, and a `style`
//! declaration outranks the matching presentation attribute, so the fallback
//! never applies: strokes vanish and fills turn black, silently. Resolving
//! each `color()` function to sRGB before parsing keeps the artwork painted,
//! and sRGB is what VectorDrawable renders in anyway.

use std::ops::Range;

use crate::ElementLocation;

/// A document with its `color()` functions resolved, and what did not resolve.
pub(crate) struct Resolved {
    /// The rewritten document, or `None` when no function resolved.
    pub source: Option<String>,
    /// How many `color()` functions resolved.
    pub colors: usize,
    /// Every attribute value and `<style>` text holding a `color()` function.
    values: Vec<Value>,
    /// `color()` functions in paint that did not resolve, which the SVG
    /// parser drops along with their sRGB fallbacks.
    unresolved: Vec<Unresolved>,
}

/// An attribute value or `<style>` text holding `color()` functions.
struct Value {
    /// Where the value sits in the source.
    range: Range<usize>,
    /// The value with character references replaced.
    text: String,
    /// Whether the source spells the value as `text`, without character
    /// references, so a function can be replaced where it sits.
    literal: bool,
    functions: Vec<Function>,
}

/// A `color()` function in a value.
struct Function {
    /// Where the function sits in the value's text.
    range: Range<usize>,
    /// The sRGB it resolves to, when it does.
    color: Option<String>,
}

/// A `color()` function in paint that did not resolve.
struct Unresolved {
    value: usize,
    function: usize,
    location: ElementLocation,
}

/// Attributes that take a color, where a `color()` function left unresolved
/// changes how the artwork is painted. `style` covers the same properties.
const PAINT_ATTRIBUTES: &[&str] = &[
    "fill",
    "stroke",
    "stop-color",
    "flood-color",
    "lighting-color",
    "solid-color",
    "color",
    "style",
];

/// How many unresolved functions are probed one at a time; the rest are
/// probed together, so a document full of them costs a bounded number of
/// parses.
const PROBES: usize = 16;

/// Rewrite every `color()` function in `source` as a plain sRGB color.
///
/// Only attribute values and `<style>` text holding a `color()` function are
/// touched, so nothing else in the document moves.
pub(crate) fn resolve_color_functions(
    source: &str,
    document: &roxmltree::Document<'_>,
) -> Resolved {
    let mut values = Vec::new();
    let mut unresolved = Vec::new();
    for node in document.descendants() {
        let mut found = Vec::new();
        let element = if node.is_element() {
            for attribute in node.attributes() {
                let paint =
                    attribute.namespace().is_none() && PAINT_ATTRIBUTES.contains(&attribute.name());
                found.push((attribute.range_value(), attribute.value(), paint));
            }
            node
        } else if let Some(style) = node
            .parent_element()
            .filter(|parent| node.is_text() && parent.has_tag_name("style"))
        {
            let range = cdata_content(source, node.range());
            found.push((range, node.text().unwrap_or_default(), true));
            style
        } else {
            continue;
        };
        let mut location = None;
        for (range, text, paint) in found {
            let functions = functions(text);
            if functions.is_empty() {
                continue;
            }
            for (index, function) in functions.iter().enumerate() {
                if paint && function.color.is_none() {
                    let location = location.get_or_insert_with(|| {
                        let position = document.text_pos_at(element.range().start);
                        ElementLocation {
                            element: element.tag_name().name().to_owned(),
                            line: position.row,
                            column: position.col,
                        }
                    });
                    unresolved.push(Unresolved {
                        value: values.len(),
                        function: index,
                        location: location.clone(),
                    });
                }
            }
            values.push(Value {
                literal: source.get(range.clone()) == Some(text),
                range,
                text: text.to_owned(),
                functions,
            });
        }
    }
    let colors = values
        .iter()
        .flat_map(|value| &value.functions)
        .filter(|function| function.color.is_some())
        .count();
    let mut resolved = Resolved {
        source: None,
        colors,
        values,
        unresolved,
    };
    if colors > 0 {
        resolved.source = Some(resolved.splice(source, |_| None));
    }
    resolved
}

impl Resolved {
    /// Elements whose paint a `color()` function left unresolved would reach.
    ///
    /// The parser drops such a function, and the declaration or fallback
    /// around it, so the paint turns black, takes an inherited color, or
    /// loses the gradient it names. But a function in a rule that matches
    /// nothing, on an element that is hidden or never used, in a property
    /// something else overrides, or in paint that is fully transparent
    /// changes nothing. To tell them apart, each function is swapped for an
    /// sRGB color, and then removed, and the document parsed again: a
    /// function matters when the paint comes out different. A notification icon is drawn white, so
    /// there only a change of opacity or of what is painted matters.
    pub(crate) fn painted(
        &self,
        source: &str,
        tree: &usvg::Tree,
        options: &usvg::Options<'_>,
        notification: bool,
    ) -> Vec<ElementLocation> {
        if self.unresolved.is_empty() {
            return Vec::new();
        }
        let mut looks = Vec::new();
        paint_looks(tree.root(), 1.0, notification, &mut looks);
        // A color the artwork does not use, so swapping it in shows.
        let spare = (1..=0xFF_FFFF)
            .find(|&color| {
                !looks.iter().any(|look| match look {
                    Look::Solid(used, _) => *used == color,
                    Look::Gradient(_, stops) => stops.iter().any(|(used, _)| *used == color),
                    _ => false,
                })
            })
            .unwrap_or(0);
        let differs = |swapped: &[&Unresolved], stand_in: &dyn Fn(&str) -> String| {
            let probe = self.splice(source, |(value, function)| {
                swapped
                    .iter()
                    .any(|unresolved| (unresolved.value, unresolved.function) == (value, function))
                    .then(|| {
                        let range = &self.values[value].functions[function].range;
                        stand_in(&self.values[value].text[range.clone()])
                    })
            });
            match usvg::Tree::from_data(probe.as_bytes(), options) {
                Ok(probed) => {
                    let mut probed_looks = Vec::new();
                    paint_looks(probed.root(), 1.0, notification, &mut probed_looks);
                    probed_looks != looks
                }
                // Without a parse to go by, the function counts as painted.
                Err(_) => true,
            }
        };
        // Swapped for a color, a function shows where its own color would
        // be painted. Removed, it shows where it spoils a value around it:
        // `color(...) #FF0000` or `url(#g) color(...)` is dropped whole, so
        // the fallback beside the function never applies either.
        let changes = |swapped: &[&Unresolved]| {
            differs(swapped, &|function| substitute(function, spare))
                || differs(swapped, &|_| String::new())
        };
        let (single, rest) = self.unresolved.split_at(self.unresolved.len().min(PROBES));
        let mut painted: Vec<&Unresolved> = single
            .iter()
            .filter(|unresolved| changes(&[unresolved]))
            .collect();
        let rest: Vec<&Unresolved> = rest.iter().collect();
        if !rest.is_empty() && changes(&rest) {
            painted.extend(rest.first());
        }
        let mut locations: Vec<ElementLocation> = Vec::new();
        for unresolved in painted {
            let location = &unresolved.location;
            if locations
                .last()
                .is_none_or(|last| (last.line, last.column) != (location.line, location.column))
            {
                locations.push(location.clone());
            }
        }
        locations
    }

    /// `source` with each resolved function replaced by its sRGB, and each
    /// unresolved one by whatever `swap` gives for it, if anything. A value
    /// spelled with character references is written out again whole.
    fn splice(&self, source: &str, swap: impl Fn((usize, usize)) -> Option<String>) -> String {
        let mut spliced = String::with_capacity(source.len());
        let mut copied = 0;
        for (index, value) in self.values.iter().enumerate() {
            let mut text = String::with_capacity(value.text.len());
            let mut kept = 0;
            for (position, function) in value.functions.iter().enumerate() {
                let Some(replacement) = function.color.clone().or_else(|| swap((index, position)))
                else {
                    continue;
                };
                text.push_str(&value.text[kept..function.range.start]);
                text.push_str(&replacement);
                kept = function.range.end;
            }
            if kept == 0 {
                continue;
            }
            text.push_str(&value.text[kept..]);
            // Values do not overlap, but keep the source intact if they did.
            if value.range.start < copied {
                continue;
            }
            spliced.push_str(&source[copied..value.range.start]);
            if value.literal {
                spliced.push_str(&text);
            } else {
                spliced.push_str(&escape(&text));
            }
            copied = value.range.end;
        }
        spliced.push_str(&source[copied..]);
        spliced
    }
}

/// How one fill or stroke looks, for telling whether a probe changed it.
#[derive(PartialEq)]
enum Look {
    /// A paint with no opacity; its color does not matter.
    Clear,
    /// `0xRRGGBB` and an 8-bit opacity.
    Solid(u32, u8),
    /// A gradient's id, and each stop's color and opacity.
    Gradient(String, Vec<(u32, u8)>),
    /// A pattern's id; its content is looked at on its own.
    Pattern(String),
}

/// How each fill and stroke under `group` looks, in drawing order, with
/// `none` left out. `opacity` is the groups' around `group`.
fn paint_looks(group: &usvg::Group, opacity: f32, notification: bool, looks: &mut Vec<Look>) {
    let opacity = opacity * group.opacity().get();
    let byte = |value: f32| (value.clamp(0.0, 1.0) * 255.0).round() as u8;
    // A notification icon is repainted white, so only opacity survives.
    let rgb = |color: usvg::Color| {
        if notification {
            0
        } else {
            u32::from(color.red) << 16 | u32::from(color.green) << 8 | u32::from(color.blue)
        }
    };
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => paint_looks(group, opacity, notification, looks),
            usvg::Node::Path(path) => {
                let fill = path.fill().map(|fill| (fill.paint(), fill.opacity().get()));
                let stroke = path
                    .stroke()
                    .map(|stroke| (stroke.paint(), stroke.opacity().get()));
                for (paint, alpha) in fill.into_iter().chain(stroke) {
                    let alpha = opacity * alpha;
                    let stops = |stops: &[usvg::Stop]| {
                        stops
                            .iter()
                            .map(|stop| match byte(alpha * stop.opacity().get()) {
                                0 => (0, 0),
                                opacity => (rgb(stop.color()), opacity),
                            })
                            .collect()
                    };
                    looks.push(match paint {
                        _ if byte(alpha) == 0 => Look::Clear,
                        usvg::Paint::Color(color) => Look::Solid(rgb(*color), byte(alpha)),
                        usvg::Paint::LinearGradient(gradient) => {
                            Look::Gradient(gradient.id().to_owned(), stops(gradient.stops()))
                        }
                        usvg::Paint::RadialGradient(gradient) => {
                            Look::Gradient(gradient.id().to_owned(), stops(gradient.stops()))
                        }
                        usvg::Paint::Pattern(pattern) => Look::Pattern(pattern.id().to_owned()),
                    });
                }
            }
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
        node.subroots(|root| paint_looks(root, opacity, notification, looks));
    }
}

/// An sRGB stand-in for an unresolved `color()` function: `color` with the
/// function's own opacity, so a probe changes the hue and nothing else.
fn substitute(function: &str, color: u32) -> String {
    let alpha = function
        .strip_suffix(')')
        .and_then(|function| function.split_once('/'))
        .and_then(|(_, alpha)| component(alpha))
        .unwrap_or(1.0);
    let [_, red, green, blue] = color.to_be_bytes();
    if alpha >= 1.0 {
        format!("#{red:02X}{green:02X}{blue:02X}")
    } else {
        format!(
            "rgba({red}, {green}, {blue}, {})",
            crate::xml::number(alpha.clamp(0.0, 1.0))
        )
    }
}

/// `text` escaped to stand in an attribute value or element text.
fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&apos;"),
            character => escaped.push(character),
        }
    }
    escaped
}

/// The range of the text inside a `<![CDATA[...]]>` section, which design
/// tools wrap `<style>` text in; any other range as it is.
fn cdata_content(source: &str, range: Range<usize>) -> Range<usize> {
    const OPEN: &str = "<![CDATA[";
    const CLOSE: &str = "]]>";
    match source.get(range.clone()) {
        Some(text)
            if text.starts_with(OPEN)
                && text.ends_with(CLOSE)
                && text.len() >= OPEN.len() + CLOSE.len() =>
        {
            range.start + OPEN.len()..range.end - CLOSE.len()
        }
        _ => range,
    }
}

/// Byte indexes where a `color()` function starts in `value`, outside CSS
/// comments, which paint nothing, and quoted strings, which are not colors.
fn function_starts(value: &str) -> impl Iterator<Item = usize> + '_ {
    let mut skip_to = 0;
    value.char_indices().filter_map(move |(start, character)| {
        if start < skip_to {
            return None;
        }
        if value[start..].starts_with("/*") {
            skip_to = value[start + 2..]
                .find("*/")
                .map_or(value.len(), |end| start + 2 + end + 2);
            return None;
        }
        if character == '"' || character == '\'' {
            skip_to = string_end(value, start, character);
            return None;
        }
        let function = (character == 'c' || character == 'C')
            && value
                .get(start..start + "color(".len())
                .is_some_and(|text| text.eq_ignore_ascii_case("color("))
            // `color(` has to start a token: `stop-color(` is not a
            // color function.
            && !value[..start].chars().next_back().is_some_and(|previous| {
                previous.is_alphanumeric() || previous == '-' || previous == '_'
            });
        function.then_some(start)
    })
}

/// Byte index just past the CSS string opening with `quote` at `start`. A
/// string ends at its closing quote, or unclosed at the end of the line.
fn string_end(value: &str, start: usize, quote: char) -> usize {
    let mut escaped = false;
    for (offset, character) in value[start + 1..].char_indices() {
        let index = start + 1 + offset;
        match character {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '\n' => return index,
            _ if character == quote => return index + 1,
            _ => {}
        }
    }
    value.len()
}

/// The `color()` functions in one value, each with the sRGB it resolves to
/// when it does.
fn functions(value: &str) -> Vec<Function> {
    let mut functions = Vec::new();
    let mut end = 0;
    for start in function_starts(value) {
        // A function nested inside another goes with it.
        if start < end {
            continue;
        }
        let body = start + "color(".len();
        let closing = closing_parenthesis(value, body);
        end = closing.map_or(value.len(), |closing| closing + 1);
        let color = closing.and_then(|closing| srgb(&value[body..closing]));
        functions.push(Function {
            range: start..end,
            color,
        });
    }
    functions
}

/// Byte index of the `)` closing the function whose body starts at `start`.
fn closing_parenthesis(value: &str, start: usize) -> Option<usize> {
    let mut depth = 0usize;
    for (offset, character) in value[start..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' if depth == 0 => return Some(start + offset),
            ')' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Resolve the body of a `color()` function, `display-p3 0.4 0.5 0.6 / 50%`,
/// to `#RRGGBB` or `rgba(r, g, b, a)`. `None` for a color space that is not
/// a plain RGB one, which is left for the SVG parser to deal with.
fn srgb(body: &str) -> Option<String> {
    let (components, alpha) = match body.split_once('/') {
        Some((components, alpha)) => (components, component(alpha.trim())?),
        None => (body, 1.0),
    };
    let mut parts = components.split_whitespace();
    let space = parts.next()?.to_ascii_lowercase();
    let channels = [
        component(parts.next()?)?,
        component(parts.next()?)?,
        component(parts.next()?)?,
    ];
    if parts.next().is_some() {
        return None;
    }
    let linear = match space.as_str() {
        // Both use the sRGB transfer function; only the primaries differ.
        "srgb" => channels.map(to_linear),
        "display-p3" => from_display_p3(channels.map(to_linear)),
        "srgb-linear" => channels,
        _ => return None,
    };
    let [red, green, blue] = linear.map(|channel| (to_gamma(channel) * 255.0).round() as u8);
    if alpha >= 1.0 {
        return Some(format!("#{red:02X}{green:02X}{blue:02X}"));
    }
    Some(format!(
        "rgba({red}, {green}, {blue}, {})",
        crate::xml::number(alpha.clamp(0.0, 1.0))
    ))
}

/// One `color()` component: a number, a percentage, or `none` for zero.
fn component(text: &str) -> Option<f32> {
    let text = text.trim();
    if text.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    let value = match text.strip_suffix('%') {
        Some(percentage) => percentage.trim_end().parse::<f32>().ok()? / 100.0,
        None => text.parse::<f32>().ok()?,
    };
    value.is_finite().then_some(value)
}

/// Linear Display P3 to linear sRGB. Both are D65, so this is the change of
/// primaries alone, with no chromatic adaptation.
fn from_display_p3([red, green, blue]: [f32; 3]) -> [f32; 3] {
    [
        1.224_940_2 * red - 0.224_940_18 * green,
        -0.042_056_955 * red + 1.042_056_9 * green,
        -0.019_637_618 * red - 0.078_636_17 * green + 1.098_273_8 * blue,
    ]
}

/// The sRGB transfer function, undone.
fn to_linear(channel: f32) -> f32 {
    if channel <= 0.040_45 {
        channel / 12.92
    } else {
        ((channel + 0.055) / 1.055).powf(2.4)
    }
}

/// The sRGB transfer function, applied. Colors outside the sRGB gamut are
/// clipped to it, which is all an sRGB drawable can show.
fn to_gamma(channel: f32) -> f32 {
    let channel = channel.clamp(0.0, 1.0);
    if channel <= 0.003_130_8 {
        12.92 * channel
    } else {
        1.055 * channel.powf(1.0 / 2.4) - 0.055
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rewrite(source: &str) -> Option<String> {
        let document = roxmltree::Document::parse(source).unwrap();
        resolve_color_functions(source, &document).source
    }

    fn colors(source: &str) -> usize {
        let document = roxmltree::Document::parse(source).unwrap();
        resolve_color_functions(source, &document).colors
    }

    fn unresolved(source: &str) -> Vec<String> {
        let document = roxmltree::Document::parse(source).unwrap();
        resolve_color_functions(source, &document)
            .unresolved
            .into_iter()
            .map(|function| function.location.element)
            .collect()
    }

    #[test]
    fn paint_left_with_a_color_function_is_reported() {
        for source in [
            // A color space that is not a plain RGB one.
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path fill="color(rec2020 1 0 0)"/></svg>"##,
            // Resolved beside one that is not.
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0);stroke:color(lab 50 20 30)"/></svg>"##,
            // Spelled with a character reference.
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(rec2020 1 0 0)&#59;"/></svg>"##,
            // No closing parenthesis.
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path fill="color(srgb 1 0 0"/></svg>"##,
        ] {
            assert_eq!(unresolved(source), ["path"], "{source}");
        }
        assert_eq!(
            unresolved(
                r##"<svg xmlns="http://www.w3.org/2000/svg"><style>.a { fill: color(rec2020 1 0 0); }</style></svg>"##
            ),
            ["style"]
        );
    }

    #[test]
    fn style_text_in_a_cdata_section_is_rewritten() {
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg"><style><![CDATA[ .a { fill: color(display-p3 1 0 0); } ]]></style></svg>"##;
        let rewritten = rewrite(source).unwrap();
        assert!(
            rewritten.contains("<![CDATA[ .a { fill: #FF0000; } ]]>"),
            "{rewritten}"
        );
        assert!(unresolved(source).is_empty());
    }

    #[test]
    fn resolved_colors_and_other_attributes_are_not_reported() {
        for source in [
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0);stroke:color(display-p3 0 1 0)"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg" data-x="color(rec2020 1 0 0)"/>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path fill="#6C707E" stroke="currentColor"/></svg>"##,
            // Comments paint nothing.
            r##"<svg xmlns="http://www.w3.org/2000/svg"><style>/* color(rec2020 1 0 0) */ path { fill: #FF0000; }</style></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="/* color(lab 50 20 30) */fill:#FF0000"/></svg>"##,
        ] {
            assert!(unresolved(source).is_empty(), "{source}");
        }
    }

    #[test]
    fn display_p3_resolves_to_the_srgb_the_design_tool_wrote_as_its_fallback() {
        // Figma pairs this stroke with a #6C707E fallback; resolving the P3
        // values it rounded to four decimals lands within one 8-bit level.
        assert_eq!(srgb("display-p3 0.4235 0.4392 0.4941").unwrap(), "#6B707F");
        assert_eq!(srgb("display-p3 0 0 0").unwrap(), "#000000");
        assert_eq!(srgb("display-p3 1 1 1").unwrap(), "#FFFFFF");
    }

    #[test]
    fn percentages_none_and_alpha_are_understood() {
        assert_eq!(srgb("srgb 100% 0% 0%").unwrap(), "#FF0000");
        assert_eq!(srgb("srgb 1 none none").unwrap(), "#FF0000");
        assert_eq!(srgb("srgb 1 0 0 / 0.5").unwrap(), "rgba(255, 0, 0, 0.5)");
        assert_eq!(srgb("srgb 1 0 0 / 50%").unwrap(), "rgba(255, 0, 0, 0.5)");
        assert_eq!(srgb("srgb 1 0 0 / 100%").unwrap(), "#FF0000");
    }

    #[test]
    fn color_spaces_that_are_not_plain_rgb_are_left_alone() {
        assert!(srgb("lab 50% 40 59.5").is_none());
        assert!(srgb("rec2020 0.4 0.5 0.6").is_none());
        assert!(srgb("display-p3 0.4 0.5").is_none());
        assert!(srgb("display-p3 0.4 0.5 0.6 0.7").is_none());
    }

    #[test]
    fn the_fallback_ahead_of_the_function_is_kept_alongside_it() {
        let rewritten = rewrite(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="stroke:#6C707E;stroke:color(display-p3 0.4235 0.4392 0.4941);stroke-opacity:1;"/></svg>"##,
        )
        .unwrap();
        assert!(
            rewritten.contains("stroke:#6C707E;stroke:#6B707F;stroke-opacity:1;"),
            "{rewritten}"
        );
    }

    #[test]
    fn several_functions_in_one_value_all_resolve() {
        let rewritten = rewrite(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0);stroke:color(srgb 0 0 1)"/></svg>"##,
        )
        .unwrap();
        assert!(
            rewritten.contains("fill:#FF0000;stroke:#0000FF"),
            "{rewritten}"
        );
    }

    #[test]
    fn style_elements_are_rewritten_too() {
        let rewritten = rewrite(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><style>.a { fill: color(display-p3 1 0 0); }</style></svg>"##,
        )
        .unwrap();
        assert!(rewritten.contains("fill: #FF0000;"), "{rewritten}");
    }

    #[test]
    fn every_resolved_function_is_counted_for_the_note() {
        assert_eq!(
            colors(
                r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0);stroke:color(display-p3 0 1 0)"/><path style="fill:color(srgb 0 0 1)"/></svg>"##
            ),
            3
        );
        // A color space that is left alone is not counted as resolved.
        assert_eq!(
            colors(
                r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0);stroke:color(rec2020 0 1 0)"/></svg>"##
            ),
            1
        );
    }

    #[test]
    fn documents_without_color_functions_are_left_untouched() {
        assert!(
            rewrite(r##"<svg xmlns="http://www.w3.org/2000/svg"><path fill="#6C707E"/></svg>"##)
                .is_none()
        );
        // `stop-color(` only looks like the function; it is a different token.
        assert!(
            rewrite(
                r##"<svg xmlns="http://www.w3.org/2000/svg" data-x="stop-color(srgb 1 0 0)"/>"##
            )
            .is_none()
        );
    }

    #[test]
    fn values_with_character_references_are_written_out_again() {
        // The parsed value is not the source text, so the value is replaced
        // whole, escaped again, rather than spliced where the function sits.
        let source = r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0)&#59;stroke:none" data-a="&amp;"/><style>.a { fill: color(display-p3 0 0 1) } .b::after { content: "&lt;" }</style></svg>"##;
        let rewritten = rewrite(source).unwrap();
        let document = roxmltree::Document::parse(&rewritten).unwrap();
        let path = document
            .descendants()
            .find(|node| node.has_tag_name("path"))
            .unwrap();
        assert_eq!(path.attribute("style"), Some("fill:#FF0000;stroke:none"));
        assert_eq!(path.attribute("data-a"), Some("&"));
        let style = document
            .descendants()
            .find(|node| node.has_tag_name("style"))
            .unwrap();
        assert_eq!(
            style.text(),
            Some(r#".a { fill: #0000FF } .b::after { content: "<" }"#)
        );
        assert!(unresolved(source).is_empty());
    }

    #[test]
    fn quoted_strings_are_not_colors_or_comments() {
        let rewritten = rewrite(
            r##"<svg xmlns="http://www.w3.org/2000/svg"><style>path { font-family: "a/*b", 'color(srgb 0 1 0)'; fill: color(display-p3 1 0 0); }</style></svg>"##,
        )
        .unwrap();
        assert!(
            rewritten.contains(r#""a/*b", 'color(srgb 0 1 0)'; fill: #FF0000;"#),
            "{rewritten}"
        );
    }
}
