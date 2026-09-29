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

use crate::ElementLocation;

/// A document with its `color()` functions resolved, and what did not resolve.
pub(crate) struct Resolved {
    /// The rewritten document, or `None` when no function resolved.
    pub source: Option<String>,
    /// How many `color()` functions resolved.
    pub colors: usize,
    /// Elements whose paint keeps a `color()` function that did not resolve,
    /// which the SVG parser drops along with its sRGB fallback.
    pub unresolved: Vec<ElementLocation>,
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
    let mut edits: Vec<(std::ops::Range<usize>, Rewrite)> = Vec::new();
    let mut unresolved = Vec::new();
    for node in document.descendants() {
        let (element, left) = if node.is_element() {
            let mut left = false;
            for attribute in node.attributes() {
                let range = attribute.range_value();
                let paint =
                    attribute.namespace().is_none() && PAINT_ATTRIBUTES.contains(&attribute.name());
                left |= push_edit(source, range, attribute.value(), &mut edits) && paint;
            }
            (node, left)
        } else if let Some(style) = node
            .parent_element()
            .filter(|parent| node.is_text() && parent.has_tag_name("style"))
        {
            let left = push_edit(
                source,
                cdata_content(source, node.range()),
                node.text().unwrap_or_default(),
                &mut edits,
            );
            (style, left)
        } else {
            continue;
        };
        if left {
            let position = document.text_pos_at(element.range().start);
            unresolved.push(ElementLocation {
                element: element.tag_name().name().to_owned(),
                line: position.row,
                column: position.col,
            });
        }
    }
    if edits.is_empty() {
        return Resolved {
            source: None,
            colors: 0,
            unresolved,
        };
    }
    edits.sort_by_key(|(range, _)| range.start);
    let mut rewritten = String::with_capacity(source.len());
    let mut copied = 0;
    let mut colors = 0;
    for (range, replacement) in edits {
        // Attributes are visited per element, so a range can only repeat if
        // two edits overlap; keeping the first leaves the source consistent.
        if range.start < copied {
            continue;
        }
        rewritten.push_str(&source[copied..range.start]);
        rewritten.push_str(&replacement.value);
        copied = range.end;
        colors += replacement.colors;
    }
    rewritten.push_str(&source[copied..]);
    Resolved {
        source: Some(rewritten),
        colors,
        unresolved,
    }
}

/// One rewritten value, and how many `color()` functions it resolved.
struct Rewrite {
    value: String,
    colors: usize,
}

/// Queue a rewrite of `range` when the source spells `value` literally there
/// and the value holds a `color()` function that resolves. `true` when a
/// `color()` function is left in the value unresolved.
fn push_edit(
    source: &str,
    range: std::ops::Range<usize>,
    value: &str,
    edits: &mut Vec<(std::ops::Range<usize>, Rewrite)>,
) -> bool {
    // Character references make the parsed value differ from the source text,
    // and splicing over them would corrupt the document.
    if source.get(range.clone()) != Some(value) {
        return function_starts(value).next().is_some();
    }
    match resolve(value) {
        Some(resolved) => {
            let left = function_starts(&resolved.value).next().is_some();
            edits.push((range, resolved));
            left
        }
        None => function_starts(value).next().is_some(),
    }
}

/// The range of the text inside a `<![CDATA[...]]>` section, which design
/// tools wrap `<style>` text in; any other range as it is.
fn cdata_content(source: &str, range: std::ops::Range<usize>) -> std::ops::Range<usize> {
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

/// Replace the `color()` functions in one value. `None` when it has none, or
/// none that resolve.
fn resolve(value: &str) -> Option<Rewrite> {
    let mut resolved = String::new();
    let mut copied = 0;
    let mut colors = 0;
    for start in function_starts(value) {
        // A function nested inside one already resolved went with it.
        if start < copied {
            continue;
        }
        let body = start + "color(".len();
        let Some(end) = closing_parenthesis(value, body) else {
            continue;
        };
        let Some(color) = srgb(&value[body..end]) else {
            continue;
        };
        resolved.push_str(&value[copied..start]);
        resolved.push_str(&color);
        copied = end + 1;
        colors += 1;
    }
    (copied > 0).then(|| {
        resolved.push_str(&value[copied..]);
        Rewrite {
            value: resolved,
            colors,
        }
    })
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
            .map(|location| location.element)
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
