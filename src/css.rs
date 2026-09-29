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

use std::collections::HashSet;
use std::ops::Range;

use crate::ElementLocation;

/// A document with its `color()` functions resolved, and what did not resolve.
pub(crate) struct Resolved {
    /// The rewritten document, or `None` when no function resolved.
    pub source: Option<String>,
    /// How many `color()` functions resolved.
    pub colors: usize,
    /// Each resolved function's range in the source, and its sRGB.
    edits: Vec<(Range<usize>, String)>,
    /// `color()` functions in paint that did not resolve, which the SVG
    /// parser drops along with their sRGB fallbacks.
    unresolved: Vec<Unresolved>,
}

/// A `color()` function in paint that did not resolve.
struct Unresolved {
    location: ElementLocation,
    /// Where the function sits in the source, when the source spells it
    /// literally.
    range: Option<Range<usize>>,
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

/// Rewrite every `color()` function in `source` as a plain sRGB color.
///
/// Only attribute values and `<style>` text are touched, and only where the
/// source spells the value literally, so nothing else in the document moves.
pub(crate) fn resolve_color_functions(
    source: &str,
    document: &roxmltree::Document<'_>,
) -> Resolved {
    let mut edits = Vec::new();
    let mut unresolved = Vec::new();
    for node in document.descendants() {
        let (element, left) = if node.is_element() {
            let mut left = Vec::new();
            for attribute in node.attributes() {
                let found = push_edits(
                    source,
                    attribute.range_value(),
                    attribute.value(),
                    &mut edits,
                );
                if attribute.namespace().is_none() && PAINT_ATTRIBUTES.contains(&attribute.name()) {
                    left.extend(found);
                }
            }
            (node, left)
        } else if let Some(style) = node
            .parent_element()
            .filter(|parent| node.is_text() && parent.has_tag_name("style"))
        {
            let left = push_edits(
                source,
                cdata_content(source, node.range()),
                node.text().unwrap_or_default(),
                &mut edits,
            );
            (style, left)
        } else {
            continue;
        };
        if left.is_empty() {
            continue;
        }
        let position = document.text_pos_at(element.range().start);
        let location = ElementLocation {
            element: element.tag_name().name().to_owned(),
            line: position.row,
            column: position.col,
        };
        unresolved.extend(left.into_iter().map(|range| Unresolved {
            location: location.clone(),
            range,
        }));
    }
    Resolved {
        source: (!edits.is_empty()).then(|| splice(source, edits.iter().cloned())),
        colors: edits.len(),
        edits,
        unresolved,
    }
}

impl Resolved {
    /// Elements whose paint a `color()` function left unresolved would reach.
    ///
    /// The parser drops such a function along with the fallback beside it,
    /// so the paint turns black or takes an inherited color. But a function
    /// in a rule that matches nothing, on an element that is hidden or never
    /// used, or in a property something else overrides paints nothing. To tell
    /// them apart, each function is swapped for a color `tree` does not use
    /// and the document parsed again: the functions whose colors come through
    /// are the ones that would have been painted.
    pub(crate) fn painted(
        &self,
        source: &str,
        tree: &usvg::Tree,
        options: &usvg::Options<'_>,
    ) -> Vec<ElementLocation> {
        if self.unresolved.is_empty() {
            return Vec::new();
        }
        let mut used = HashSet::new();
        paint_colors(tree.root(), &mut used);
        let mut spare = (1..=0xFF_FFFF).filter(|color| !used.contains(color));
        let probes: Vec<Option<u32>> = self
            .unresolved
            .iter()
            .map(|function| function.range.as_ref().and_then(|_| spare.next()))
            .collect();
        let mut edits = self.edits.clone();
        edits.extend(
            self.unresolved
                .iter()
                .zip(&probes)
                .filter_map(|(function, probe)| {
                    Some((function.range.clone()?, format!("#{:06X}", (*probe)?)))
                }),
        );
        edits.sort_by_key(|(range, _)| range.start);
        let mut painted = HashSet::new();
        match usvg::Tree::from_data(splice(source, edits).as_bytes(), options) {
            Ok(probed) => paint_colors(probed.root(), &mut painted),
            // Without a parse to go by, every function counts as painted.
            Err(_) => painted.extend(probes.iter().flatten()),
        }
        let mut locations: Vec<ElementLocation> = Vec::new();
        for (function, probe) in self.unresolved.iter().zip(probes) {
            // A function spelled with character references cannot be swapped
            // out, so it counts as painted.
            if probe.is_none_or(|probe| painted.contains(&probe))
                && locations.last().is_none_or(|last| {
                    (last.line, last.column) != (function.location.line, function.location.column)
                })
            {
                locations.push(function.location.clone());
            }
        }
        locations
    }
}

/// Every color the paint under `group` uses, as `0xRRGGBB`.
fn paint_colors(group: &usvg::Group, colors: &mut HashSet<u32>) {
    fn key(color: usvg::Color) -> u32 {
        u32::from(color.red) << 16 | u32::from(color.green) << 8 | u32::from(color.blue)
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => paint_colors(group, colors),
            usvg::Node::Path(path) => {
                let fill = path.fill().map(usvg::Fill::paint);
                let stroke = path.stroke().map(usvg::Stroke::paint);
                for paint in fill.into_iter().chain(stroke) {
                    match paint {
                        usvg::Paint::Color(color) => {
                            colors.insert(key(*color));
                        }
                        usvg::Paint::LinearGradient(gradient) => {
                            colors.extend(gradient.stops().iter().map(|stop| key(stop.color())));
                        }
                        usvg::Paint::RadialGradient(gradient) => {
                            colors.extend(gradient.stops().iter().map(|stop| key(stop.color())));
                        }
                        // Its content is one of the path's subroots.
                        usvg::Paint::Pattern(_) => {}
                    }
                }
            }
            usvg::Node::Image(_) | usvg::Node::Text(_) => {}
        }
        node.subroots(|root| paint_colors(root, colors));
    }
}

/// `source` with each range, in order, replaced.
fn splice(source: &str, edits: impl IntoIterator<Item = (Range<usize>, String)>) -> String {
    let mut spliced = String::with_capacity(source.len());
    let mut copied = 0;
    for (range, replacement) in edits {
        // Functions do not nest across values, so ranges only overlap when
        // one function sits inside another; keeping the outer one is enough.
        if range.start < copied {
            continue;
        }
        spliced.push_str(&source[copied..range.start]);
        spliced.push_str(&replacement);
        copied = range.end;
    }
    spliced.push_str(&source[copied..]);
    spliced
}

/// Queue a rewrite of each `color()` function in `value` that resolves, when
/// the source spells `value` literally at `range`. Returns one entry for each
/// function left unresolved: its range in the source, or `None` when the
/// source does not spell it literally.
fn push_edits(
    source: &str,
    range: Range<usize>,
    value: &str,
    edits: &mut Vec<(Range<usize>, String)>,
) -> Vec<Option<Range<usize>>> {
    // Character references make the parsed value differ from the source text,
    // and splicing over them would corrupt the document.
    let literal = source.get(range.clone()) == Some(value);
    let mut left = Vec::new();
    for (function, color) in functions(value) {
        let function = range.start + function.start..range.start + function.end;
        match color {
            Some(color) if literal => edits.push((function, color)),
            _ => left.push(literal.then_some(function)),
        }
    }
    left
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
/// comments, which paint nothing.
fn function_starts(value: &str) -> impl Iterator<Item = usize> + '_ {
    let mut comment_end = 0;
    value.char_indices().filter_map(move |(start, character)| {
        if start < comment_end {
            return None;
        }
        if value[start..].starts_with("/*") {
            comment_end = value[start + 2..]
                .find("*/")
                .map_or(value.len(), |end| start + 2 + end + 2);
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

/// The `color()` functions in one value: each one's range in the value, and
/// the sRGB it resolves to when it does.
fn functions(value: &str) -> Vec<(Range<usize>, Option<String>)> {
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
        functions.push((start..end, color));
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
            // A character reference keeps the value out of the rewrite.
            r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0)&#59;"/></svg>"##,
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
    fn character_references_keep_a_value_out_of_the_rewrite() {
        // The parsed value is shorter than the source text, so splicing over
        // it would corrupt the document; leaving it alone is the safe answer.
        assert!(
            rewrite(
                r##"<svg xmlns="http://www.w3.org/2000/svg"><path style="fill:color(srgb 1 0 0)&#59;"/></svg>"##
            )
            .is_none()
        );
    }
}
