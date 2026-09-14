use crate::analysis::{
    Analysis, Compatibility, Diagnostic, DiagnosticCode, ElementLocation, Metrics, Severity,
};
use crate::error::{Error, Result};
use crate::vector::{self, VectorDrawable};

pub(crate) struct Processed {
    pub analysis: Analysis,
    pub drawable: Option<VectorDrawable>,
    /// Whether the root declares a size rather than borrowing its viewBox.
    pub declared_size: bool,
}

pub(crate) fn process(source: &[u8], keep_drawable: bool) -> Result<Processed> {
    let text = std::str::from_utf8(source)?;
    reject_unsafe_xml(text)?;
    let document = roxmltree::Document::parse(text)?;
    let (mut compatibility, mut diagnostics) = preflight(&document);

    let options = usvg::Options {
        resources_dir: None,
        image_href_resolver: usvg::ImageHrefResolver {
            resolve_data: Box::new(|_, _, _| None),
            resolve_string: Box::new(|_, _| None),
        },
        ..Default::default()
    };
    let tree = usvg::Tree::from_data(source, &options)?;
    let mut metrics = Metrics::default();
    let drawable = vector::lower(&tree, &mut compatibility, &mut diagnostics, &mut metrics);
    if let Some(ref drawable) = drawable {
        metrics.estimated_xml_bytes = crate::xml::write(drawable).len();
    }
    let minimum_api = if compatibility.convertible() {
        drawable.as_ref().map(VectorDrawable::minimum_api)
    } else {
        None
    };
    Ok(Processed {
        analysis: Analysis {
            compatibility,
            minimum_api,
            diagnostics,
            metrics,
        },
        drawable: keep_drawable.then_some(drawable).flatten(),
        declared_size: ["width", "height"]
            .into_iter()
            .any(|attribute| document.root_element().has_attribute(attribute)),
    })
}

fn reject_unsafe_xml(source: &str) -> Result<()> {
    let uppercase = source.to_ascii_uppercase();
    if uppercase.contains("<!DOCTYPE") {
        return Err(Error::UnsafeXml("DOCTYPE declarations are forbidden"));
    }
    if uppercase.contains("<!ENTITY") {
        return Err(Error::UnsafeXml("entity declarations are forbidden"));
    }
    Ok(())
}

fn preflight(document: &roxmltree::Document<'_>) -> (Compatibility, Vec<Diagnostic>) {
    let mut compatibility = Compatibility::Exact;
    let mut diagnostics = Vec::new();
    for node in document.descendants().filter(roxmltree::Node::is_element) {
        let name = node.tag_name().name();
        let location = || {
            let position = document.text_pos_at(node.range().start);
            Some(ElementLocation {
                element: name.to_owned(),
                line: position.row,
                column: position.col,
            })
        };
        match name {
            "svg" => {
                let width = node.attribute("width");
                let height = node.attribute("height");
                if width.is_some_and(is_percentage) || height.is_some_and(is_percentage) {
                    push_unsupported(
                        &mut compatibility,
                        &mut diagnostics,
                        DiagnosticCode::UnsupportedDimensions,
                        "percentage dimensions depend on an external SVG viewport",
                        location(),
                        Some("Use absolute width and height values for an asset."),
                    );
                } else if width.is_none() || height.is_none() {
                    if width.is_none() && height.is_none() && node.has_attribute("viewBox") {
                        mark_normalization(&mut compatibility, &mut diagnostics, location());
                    } else {
                        push_unsupported(
                            &mut compatibility,
                            &mut diagnostics,
                            DiagnosticCode::UnsupportedDimensions,
                            "both width and height are required unless viewBox supplies both",
                            location(),
                            Some("Set width and height or provide a viewBox."),
                        );
                    }
                }
            }
            "mask" if source_mask_may_lower(&node) => {
                mark_normalization(&mut compatibility, &mut diagnostics, location())
            }
            "mask" => push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::UnsupportedMask,
                "only one fully opaque white mask shape can be lowered exactly",
                location(),
                Some("Replace alpha, grayscale, or subtractive masking with outlined geometry."),
            ),
            "filter"
                if node
                    .parent_element()
                    .is_some_and(|parent| parent.has_tag_name("svg")) =>
            {
                push_unsupported(
                    &mut compatibility,
                    &mut diagnostics,
                    DiagnosticCode::UnsupportedFilter,
                    "SVG filters cannot be represented by VectorDrawable",
                    location(),
                    None,
                )
            }
            value if value.starts_with("fe") => push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::UnsupportedFilter,
                "SVG filter primitives cannot be represented by VectorDrawable",
                location(),
                None,
            ),
            "text" | "textPath" | "tspan" => push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::TextNotOutlined,
                "text must be converted to outlines before conversion",
                location(),
                Some("Convert text to paths in the source design tool."),
            ),
            "image" => {
                let href = href(&node).unwrap_or_default();
                let (code, message) = if href.starts_with("data:") {
                    (
                        DiagnosticCode::EmbeddedImage,
                        "embedded raster or SVG images are unsupported",
                    )
                } else {
                    (
                        DiagnosticCode::ExternalImage,
                        "external images are unsupported and are never loaded",
                    )
                };
                push_unsupported(
                    &mut compatibility,
                    &mut diagnostics,
                    code,
                    message,
                    location(),
                    Some("Replace the image with vector path geometry."),
                );
            }
            "pattern" => push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::UnsupportedPattern,
                "patterns cannot be represented by VectorDrawable",
                location(),
                None,
            ),
            "animate" | "animateColor" | "animateMotion" | "animateTransform" | "set" => {
                push_unsupported(
                    &mut compatibility,
                    &mut diagnostics,
                    DiagnosticCode::UnsupportedAnimation,
                    "SVG animation is outside the VectorDrawable compatibility profile",
                    location(),
                    None,
                )
            }
            "foreignObject" | "switch" | "script" => push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::UnsupportedPaint,
                "this SVG container has no reliable VectorDrawable equivalent",
                location(),
                None,
            ),
            "rect" | "circle" | "ellipse" | "line" | "polyline" | "polygon" | "use" | "style"
            | "marker" | "symbol" | "a" => {
                mark_normalization(&mut compatibility, &mut diagnostics, location())
            }
            _ => {}
        }

        if node.has_attribute("transform") {
            mark_normalization(&mut compatibility, &mut diagnostics, location());
        }
        if has_nondefault_opacity(&node) {
            mark_normalization(&mut compatibility, &mut diagnostics, location());
        }
        if node.has_attribute("filter") || style_contains(&node, "filter:") {
            push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::UnsupportedFilter,
                "filter usage cannot be represented by VectorDrawable",
                location(),
                None,
            );
        }
        if node
            .attribute("vector-effect")
            .is_some_and(|value| value != "none")
            || style_contains(&node, "vector-effect:")
        {
            push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::UnsupportedStrokeTransform,
                "non-scaling strokes cannot be represented by VectorDrawable",
                location(),
                None,
            );
        }
        if style_contains(&node, "mix-blend-mode:")
            || style_contains(&node, "isolation:")
            || (name == "style"
                && node.text().is_some_and(|text| {
                    let text = text.to_ascii_lowercase();
                    text.contains("filter:")
                        || text.contains("mix-blend-mode:")
                        || text.contains("isolation:")
                }))
        {
            push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::UnsupportedPaint,
                "filtering or compositing styles cannot be represented by VectorDrawable",
                location(),
                None,
            );
        }
        if name != "image"
            && href(&node).is_some_and(|value| !value.is_empty() && !value.starts_with('#'))
        {
            push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::ExternalReference,
                "external references are unsupported and are never loaded",
                location(),
                None,
            );
        }
        if node
            .attributes()
            .any(|attribute| has_external_url(attribute.value()))
            || (name == "style" && node.text().is_some_and(has_external_url))
        {
            push_unsupported(
                &mut compatibility,
                &mut diagnostics,
                DiagnosticCode::ExternalReference,
                "external URL references are unsupported and are never loaded",
                location(),
                None,
            );
        }
    }
    (compatibility, diagnostics)
}

fn href<'a, 'input>(node: &roxmltree::Node<'a, 'input>) -> Option<&'a str> {
    node.attributes()
        .find(|attribute| attribute.name() == "href")
        .map(|attribute| attribute.value())
}

fn style_contains(node: &roxmltree::Node<'_, '_>, property: &str) -> bool {
    node.attribute("style")
        .is_some_and(|style| style.to_ascii_lowercase().contains(property))
}

fn has_nondefault_opacity(node: &roxmltree::Node<'_, '_>) -> bool {
    let nondefault = |value: &str| value.trim().parse::<f32>() != Ok(1.0);
    node.attribute("opacity").is_some_and(nondefault)
        || node.attribute("style").is_some_and(|style| {
            style.split(';').any(|declaration| {
                declaration.split_once(':').is_some_and(|(name, value)| {
                    name.trim().eq_ignore_ascii_case("opacity") && nondefault(value)
                })
            })
        })
}

fn source_mask_may_lower(mask: &roxmltree::Node<'_, '_>) -> bool {
    let children = mask
        .children()
        .filter(roxmltree::Node::is_element)
        .collect::<Vec<_>>();
    if children.len() != 1 {
        return false;
    }
    let shape = children[0];
    if !matches!(
        shape.tag_name().name(),
        "path" | "rect" | "circle" | "ellipse" | "polygon" | "polyline"
    ) || shape.children().any(|child| child.is_element())
    {
        return false;
    }

    let fill = shape.attribute("fill").or_else(|| mask.attribute("fill"));
    if !fill.is_some_and(is_white) || has_nondefault_alpha(mask) || has_nondefault_alpha(&shape) {
        return false;
    }
    if [mask, &shape].into_iter().any(|node| {
        node.attribute("stroke")
            .is_some_and(|stroke| !stroke.eq_ignore_ascii_case("none"))
            || node.has_attribute("filter")
            || node.has_attribute("mask")
            || node.has_attribute("clip-path")
    }) {
        return false;
    }

    if mask
        .attribute("mask-type")
        .is_some_and(|kind| !kind.eq_ignore_ascii_case("luminance"))
        || mask.attribute("style").is_some_and(|style| {
            style
                .split(';')
                .filter(|declaration| !declaration.trim().is_empty())
                .any(|declaration| {
                    declaration.split_once(':').is_none_or(|(name, value)| {
                        !name.trim().eq_ignore_ascii_case("mask-type")
                            || !value.trim().eq_ignore_ascii_case("luminance")
                    })
                })
        })
    {
        return false;
    }
    for attribute in ["width", "height"] {
        if mask.attribute(attribute).is_some_and(|value| {
            value
                .trim()
                .parse::<f32>()
                .map_or(true, |value| value <= 0.0)
        }) {
            return false;
        }
    }
    true
}

fn has_nondefault_alpha(node: &roxmltree::Node<'_, '_>) -> bool {
    ["opacity", "fill-opacity"].into_iter().any(|attribute| {
        node.attribute(attribute)
            .is_some_and(|value| value.trim().parse::<f32>() != Ok(1.0))
    })
}

fn is_white(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "white" | "#fff" | "#ffffff" | "rgb(255,255,255)"
    )
}

fn is_percentage(value: &str) -> bool {
    value.trim_end().ends_with('%')
}

fn has_external_url(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    let mut remainder = lower.as_str();
    while let Some(index) = remainder.find("url(") {
        let target = remainder[index + 4..].trim_start();
        let target = target.trim_start_matches(['\'', '"']);
        if !target.starts_with('#') {
            return true;
        }
        remainder = &target[1..];
    }
    lower.contains("@import")
}

fn mark_normalization(
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
    location: Option<ElementLocation>,
) {
    compatibility.worsen(Compatibility::ExactWithNormalization);
    if diagnostics
        .iter()
        .all(|item| item.code.as_str() != DiagnosticCode::NormalizationRequired.as_str())
    {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::NormalizationRequired,
            severity: Severity::Info,
            message: "safe SVG normalization is required before lowering".to_owned(),
            location,
            suggestion: None,
        });
    }
}

fn push_unsupported(
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagnosticCode,
    message: &str,
    location: Option<ElementLocation>,
    suggestion: Option<&str>,
) {
    compatibility.worsen(Compatibility::Unsupported);
    diagnostics.push(Diagnostic {
        code,
        severity: Severity::Error,
        message: message.to_owned(),
        location,
        suggestion: suggestion.map(str::to_owned),
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_doctype_before_xml_parser() {
        let error = process(b"<!DOCTYPE svg><svg/>", false).err().unwrap();
        assert!(matches!(error, Error::UnsafeXml(_)));
    }

    #[test]
    fn preflight_sees_features_normalization_may_drop() {
        let source = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
            <filter id="blur"><feGaussianBlur stdDeviation="2"/></filter>
            <path d="M0 0h1v1z" filter="url(#blur)"/>
        </svg>"#;
        let analysis = process(source, false).unwrap().analysis;
        assert_eq!(analysis.compatibility, Compatibility::Unsupported);
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diagnostic| { matches!(diagnostic.code, DiagnosticCode::UnsupportedFilter) })
        );
    }

    #[test]
    fn rejects_external_css_urls_and_relative_dimensions() {
        for source in [
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="24"><path d="M0 0L1 1"/></svg>"##,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M0 0L1 1" fill="url(https://example.com/colors.svg#red)"/></svg>"##,
        ] {
            assert_eq!(
                process(source.as_bytes(), false)
                    .unwrap()
                    .analysis
                    .compatibility,
                Compatibility::Unsupported
            );
        }
    }
}
