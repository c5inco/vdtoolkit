use std::fs;
use std::process::Command;

use svg2vd::vector::VectorNode;
use svg2vd::{Compatibility, DiagnosticCode, Error};

#[test]
fn converts_viewbox_geometry_colors_and_fill_rule() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="10 20 10 5">
        <path d="M10 20 L20 20 L20 25 Z" fill="#123456" fill-opacity=".25" fill-rule="evenodd"/>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);

    assert_eq!(asset.analysis.compatibility, Compatibility::Exact);
    assert!(xml.contains("android:width=\"20dp\""));
    assert!(xml.contains("android:viewportHeight=\"10\""));
    assert!(xml.contains("android:pathData=\"M0,0 L20,0 L20,10 Z\""));
    assert!(xml.contains("android:fillColor=\"#123456\""));
    assert!(xml.contains("android:fillAlpha=\"0.25\""));
    assert!(xml.contains("android:fillType=\"evenOdd\""));
    assert_eq!(asset.analysis.minimum_api, Some(24));
}

#[test]
fn normalizes_shapes_nested_transforms_and_uniform_strokes() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="30">
        <g transform="translate(3 5)"><g transform="scale(2)">
            <rect x="1" y="2" width="4" height="3" fill="none" stroke="#ABCDEF"
                  stroke-width="2" stroke-linecap="round" stroke-linejoin="bevel"/>
        </g></g>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);

    assert_eq!(
        asset.analysis.compatibility,
        Compatibility::ExactWithNormalization
    );
    assert!(xml.contains("android:pathData=\"M5,9 L13,9 L13,15 L5,15 Z\""));
    assert!(xml.contains("android:strokeWidth=\"4\""));
    assert!(xml.contains("android:strokeLineCap=\"round\""));
    assert!(xml.contains("android:strokeLineJoin=\"bevel\""));
    assert!(!xml.contains("android:fillColor"));
}

#[test]
fn normalizes_every_core_shape_and_local_use() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
        <defs><path id="tick" d="M1 1L3 3"/></defs>
        <rect x="1" y="2" width="3" height="4"/>
        <circle cx="10" cy="10" r="2"/>
        <ellipse cx="16" cy="10" rx="3" ry="2"/>
        <line x1="1" y1="20" x2="8" y2="20" stroke="#000"/>
        <polygon points="10,20 14,20 12,24"/>
        <polyline points="16,20 18,24 20,20" fill="none" stroke="#000"/>
        <use href="#tick" transform="translate(24 20)"/>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    assert_eq!(
        asset.analysis.compatibility,
        Compatibility::ExactWithNormalization
    );
    assert_eq!(asset.analysis.metrics.paths, 7);
    assert!(
        asset
            .drawable
            .children
            .iter()
            .all(|node| matches!(node, VectorNode::Path(path) if !path.path_data.0.is_empty()))
    );
}

#[test]
fn resolves_inherited_paint_and_flattens_rotation() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <g fill="#102030" stroke="#405060" stroke-width="2" stroke-opacity=".4"
           transform="rotate(90)">
            <path d="M1 2L3 2"/>
        </g>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);
    assert!(xml.contains("android:pathData=\"M-2,1 L-2,3\""));
    assert!(xml.contains("android:fillColor=\"#102030\""));
    assert!(xml.contains("android:strokeColor=\"#405060\""));
    assert!(xml.contains("android:strokeAlpha=\"0.4\""));
    assert!(xml.contains("android:strokeWidth=\"2\""));
}

#[test]
fn lowers_single_path_opacity_without_changing_compositing() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M2 2H22V22H2Z" fill="#102030" style="opacity: .3"/>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);
    assert_eq!(
        asset.analysis.compatibility,
        Compatibility::ExactWithNormalization
    );
    assert!(xml.contains("android:fillAlpha=\"0.3\""));
}

#[test]
fn rejects_opacity_when_painted_content_can_overlap() {
    for content in [
        r##"<path d="M2 2H22V22H2Z" fill="#102030" stroke="#405060" opacity=".3"/>"##,
        r##"<g opacity=".3"><rect x="2" y="2" width="12" height="12"/><rect x="8" y="8" width="12" height="12"/></g>"##,
    ] {
        let source = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">{content}</svg>"#
        );
        let analysis = svg2vd::analyze(source.as_bytes()).unwrap();
        assert_eq!(analysis.compatibility, Compatibility::Unsupported);
        assert!(analysis.diagnostics.iter().any(|diagnostic| {
            matches!(diagnostic.code, DiagnosticCode::UnsupportedPaint)
                && diagnostic.message.contains("group opacity")
        }));
    }
}

#[test]
fn normalizes_arc_geometry_and_emits_parseable_xml() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M2 12A10 10 0 0 1 22 12" fill="none" stroke="#000"/>
    </svg>"##;

    let xml = svg2vd::xml::write(&svg2vd::convert(source).unwrap().drawable);
    assert!(xml.contains(" C"), "arc should normalize to cubic geometry");
    assert!(xml.contains("22,12"));
    roxmltree::Document::parse(&xml).unwrap();
}

#[test]
fn malformed_svg_is_a_structured_error_not_a_panic() {
    let result = svg2vd::convert(br#"<svg><path d="M0 0"></svg>"#);
    assert!(matches!(result, Err(Error::Xml(_))));
}

#[test]
fn never_converts_known_lossy_features() {
    let source = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <image href="https://example.com/pixel.png" width="24" height="24"/>
    </svg>"#;

    let analysis = svg2vd::analyze(source).unwrap();
    assert_eq!(analysis.compatibility, Compatibility::Unsupported);
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.code, DiagnosticCode::ExternalImage))
    );
    assert!(matches!(
        svg2vd::convert(source),
        Err(Error::Incompatible(_))
    ));
}

#[test]
fn rejects_non_uniform_transforms_on_strokes() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path transform="scale(2 3)" d="M1 1L4 1" fill="none" stroke="#000"/>
    </svg>"##;
    let analysis = svg2vd::analyze(source).unwrap();
    assert_eq!(analysis.compatibility, Compatibility::Unsupported);
    assert!(
        analysis.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.code,
            DiagnosticCode::UnsupportedStrokeTransform
        ))
    );
}

#[test]
fn rejects_stroke_features_vector_drawable_cannot_express() {
    for extra in [
        r#"stroke-dasharray="2 1""#,
        r#"paint-order="stroke fill""#,
        r#"vector-effect="non-scaling-stroke""#,
    ] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
                <path d="M1 1L20 20" fill="#fff" stroke="#000" {extra}/>
            </svg>"##
        );
        let analysis = svg2vd::analyze(source.as_bytes()).unwrap();
        assert_eq!(
            analysis.compatibility,
            Compatibility::Unsupported,
            "feature was silently accepted: {extra}"
        );
    }
}

#[test]
fn aosp_edge_fixtures_have_stable_conversion_outcomes() {
    let convertible = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 12 8">
        <g transform="matrix(0 -1 1 0 0 8)" fill="#123456">
            <path d="M1 1H7V4H1Z"/>
        </g>
    </svg>"##;
    let asset = svg2vd::convert(convertible).unwrap();
    assert_eq!(
        asset.analysis.compatibility,
        Compatibility::ExactWithNormalization
    );
    assert_eq!(asset.drawable.width_dp, 12.0);
    assert_eq!(asset.drawable.height_dp, 8.0);

    let unsupported = [
        (
            "percentage dimensions",
            DiagnosticCode::UnsupportedDimensions,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="24"><path d="M0 0H1V1Z"/></svg>"##,
        ),
        (
            "gradient paint",
            DiagnosticCode::UnsupportedGradient,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><linearGradient id="g"><stop/><stop offset="1"/></linearGradient></defs><path d="M0 0H24V24Z" fill="url(#g)"/></svg>"##,
        ),
        (
            "multi-path clip union",
            DiagnosticCode::UnsupportedClipPath,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><clipPath id="c"><path d="M0 0H8V24H0Z"/><path d="M16 0H24V24H16Z"/></clipPath></defs><path d="M0 0H24V24H0Z" clip-path="url(#c)"/></svg>"##,
        ),
        (
            "mask",
            DiagnosticCode::UnsupportedMask,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><mask id="m"><path d="M0 0H12V24H0Z"/></mask><path d="M0 0H24V24H0Z" mask="url(#m)"/></svg>"##,
        ),
        (
            "pattern paint",
            DiagnosticCode::UnsupportedPattern,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><pattern id="p" width="2" height="2"><path d="M0 0H1V1H0Z"/></pattern></defs><path d="M0 0H24V24H0Z" fill="url(#p)"/></svg>"##,
        ),
        (
            "animation",
            DiagnosticCode::UnsupportedAnimation,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M0 0H1V1Z"><animate attributeName="opacity" values="0;1"/></path></svg>"##,
        ),
    ];

    for (name, expected_code, source) in unsupported {
        let analysis = svg2vd::analyze(source.as_bytes()).unwrap();
        assert_eq!(analysis.compatibility, Compatibility::Unsupported, "{name}");
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_str() == expected_code.as_str()),
            "{name} did not report {}",
            expected_code.as_str()
        );
    }
}

#[test]
fn lowers_transformed_single_path_clips_with_api_21_ordering() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><clipPath id="c"><rect x="2" y="3" width="8" height="9"/></clipPath></defs>
        <g transform="translate(4 5)" clip-path="url(#c)">
            <path d="M0 0H20V20H0Z" fill="#123456"/>
        </g>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);
    let clip = xml.find("<clip-path").unwrap();
    let path = xml.find("<path").unwrap();
    assert!(clip < path, "clip must precede the content it affects");
    assert!(xml.contains("android:pathData=\"M6,8 L14,8 L14,17 L6,17 Z\""));
    assert!(xml.contains("android:pathData=\"M4,5 L24,5 L24,25 L4,25 Z\""));
    assert_eq!(asset.analysis.minimum_api, Some(21));
    roxmltree::Document::parse(&xml).unwrap();
}

#[test]
fn lowers_object_bounding_box_clips_to_viewport_geometry() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><clipPath id="c" clipPathUnits="objectBoundingBox">
            <rect x="0" y="0" width=".5" height="1"/>
        </clipPath></defs>
        <rect x="4" y="6" width="8" height="10" fill="#123456" clip-path="url(#c)"/>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);
    assert!(xml.contains("android:pathData=\"M4,6 L8,6 L8,16 L4,16 Z\""));
    assert!(xml.contains("android:pathData=\"M4,6 L12,6 L12,16 L4,16 Z\""));
    assert_eq!(asset.analysis.minimum_api, Some(21));
}

#[test]
fn lowers_opaque_white_masks_to_scoped_clips() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><mask id="m" maskUnits="userSpaceOnUse" x="0" y="0" width="20" height="18" fill="#fff">
            <rect x="2" y="3" width="8" height="9"/>
        </mask></defs>
        <g transform="translate(4 5)">
            <path d="M0 0H20V20H0Z" fill="#123456" mask="url(#m)"/>
        </g>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);
    assert_eq!(xml.matches("<clip-path").count(), 2);
    let region = xml.find("M4,5 L24,5 L24,23 L4,23 Z").unwrap();
    let mask = xml.find("M6,8 L14,8 L14,17 L6,17 Z").unwrap();
    let path = xml.find("M4,5 L24,5 L24,25 L4,25 Z").unwrap();
    assert!(region < mask && mask < path);
    assert_eq!(asset.analysis.minimum_api, Some(24));
}

#[test]
fn lowers_object_bounding_box_mask_content() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><mask id="m" maskContentUnits="objectBoundingBox" fill="white">
            <rect x="0" y="0" width=".5" height="1"/>
        </mask></defs>
        <rect x="4" y="6" width="8" height="10" fill="#123456" mask="url(#m)"/>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);
    assert_eq!(xml.matches("<clip-path").count(), 2);
    assert!(xml.contains("android:pathData=\"M4,6 L8,6 L8,16 L4,16 Z\""));
    assert!(xml.contains("android:pathData=\"M4,6 L12,6 L12,16 L4,16 Z\""));
    assert_eq!(asset.analysis.minimum_api, Some(24));
}

#[test]
fn rejects_masks_that_are_not_hard_white_geometry() {
    for source in [
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><mask id="m"><path fill="#888" d="M0 0H12V24H0Z"/></mask><path d="M0 0H24V24H0Z" mask="url(#m)"/></svg>"##.as_slice(),
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><mask id="m"><path fill="#fff" fill-opacity=".5" d="M0 0H12V24H0Z"/></mask><path d="M0 0H24V24H0Z" mask="url(#m)"/></svg>"##.as_slice(),
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><mask id="m"><path fill="#fff" fill-rule="evenodd" d="M0 0H24V24H0ZM4 4V20H20V4Z"/></mask><path d="M0 0H24V24H0Z" mask="url(#m)"/></svg>"##.as_slice(),
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><mask id="m" fill="#000"><path fill="#fff" d="M0 0H24V24H0Z"/><path d="M4 4H20V20H4Z"/></mask><path d="M0 0H24V24H0Z" mask="url(#m)"/></svg>"##.as_slice(),
    ] {
        let analysis = svg2vd::analyze(source).unwrap();
        assert_eq!(analysis.compatibility, Compatibility::Unsupported);
        assert!(analysis
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.code, DiagnosticCode::UnsupportedMask)));
    }
}

#[test]
fn preserves_nested_clip_intersection_and_scope() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs>
            <clipPath id="outer"><rect x="0" y="0" width="20" height="20"/></clipPath>
            <clipPath id="inner"><rect x="2" y="2" width="10" height="10"/></clipPath>
        </defs>
        <g transform="translate(1 0)" clip-path="url(#outer)">
            <g transform="translate(3 0)" clip-path="url(#inner)">
                <path d="M0 0H20V20H0Z" fill="#123456"/>
            </g>
        </g>
    </svg>"##;

    let asset = svg2vd::convert(source).unwrap();
    let xml = svg2vd::xml::write(&asset.drawable);
    assert_eq!(xml.matches("<group>").count(), 2);
    assert_eq!(xml.matches("<clip-path").count(), 2);
    let outer_clip = xml.find("M1,0 L21,0 L21,20 L1,20 Z").unwrap();
    let inner_group = xml[outer_clip..].find("<group>").unwrap() + outer_clip;
    let inner_clip = xml.find("M6,2 L16,2 L16,12 L6,12 Z").unwrap();
    let path = xml.find("M4,0 L24,0 L24,20 L4,20 Z").unwrap();
    assert!(outer_clip < inner_group && inner_group < inner_clip && inner_clip < path);
    assert_eq!(asset.analysis.minimum_api, Some(24));
}

#[test]
fn rejects_clip_semantics_android_cannot_represent_exactly() {
    for source in [
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><clipPath id="c"><path d="M0 0H8V24H0Z"/><path d="M16 0H24V24H16Z"/></clipPath></defs><path d="M0 0H24V24H0Z" clip-path="url(#c)"/></svg>"##.as_slice(),
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><clipPath id="c"><path clip-rule="evenodd" d="M0 0H24V24H0ZM4 4V20H20V4Z"/></clipPath></defs><path d="M0 0H24V24H0Z" clip-path="url(#c)"/></svg>"##.as_slice(),
    ] {
        let analysis = svg2vd::analyze(source).unwrap();
        assert_eq!(analysis.compatibility, Compatibility::Unsupported);
        assert!(analysis.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.code,
            DiagnosticCode::UnsupportedClipPath
        )));
    }
}

#[test]
fn output_is_deterministic() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <circle cx="12" cy="12" r="8" fill="#ff0055"/>
    </svg>"##;
    let first = svg2vd::xml::write(&svg2vd::convert(source).unwrap().drawable);
    let second = svg2vd::xml::write(&svg2vd::convert(source).unwrap().drawable);
    assert_eq!(first.as_bytes(), second.as_bytes());
}

#[test]
fn cli_converts_directories_and_check_has_ci_exit_code() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icons");
    let nested = input.join("nested");
    let output = temp.path().join("drawable");
    fs::create_dir_all(&nested).unwrap();
    fs::write(
        input.join("ok.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0L1 1" fill="#000"/></svg>"##,
    )
    .unwrap();
    fs::write(
        nested.join("bad.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><text>x</text></svg>"#,
    )
    .unwrap();

    let check = Command::new(env!("CARGO_BIN_EXE_svg2vd"))
        .args(["check", input.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(2));
    let json: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);

    fs::remove_file(nested.join("bad.svg")).unwrap();
    let convert = Command::new(env!("CARGO_BIN_EXE_svg2vd"))
        .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(convert.status.success(), "{:?}", convert.stderr);
    assert!(output.join("ok.xml").is_file());
}

#[test]
fn optimize_reports_before_and_after_sizes() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("decimal.svg");
    let output = temp.path().join("decimal.xml");
    fs::write(
        &input,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
            <defs><clipPath id="c"><path d="M1.234567 2.345678L20.987654 21.876543Z"/></clipPath></defs>
            <path d="M1.234567 2.345678L20.987654 21.876543" fill="#123456" clip-path="url(#c)"/>
        </svg>"##,
    )
    .unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_svg2vd"))
        .args([
            "optimize",
            input.to_str().unwrap(),
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(result.status.success());
    let report = String::from_utf8(result.stderr).unwrap();
    assert!(report.contains("Generated drawable:"));
    assert!(report.contains("Optimized drawable:"));
    assert!(report.contains("Reduction:"));
    let xml = fs::read_to_string(output).unwrap();
    assert!(xml.contains("<clip-path"));
    assert_eq!(xml.matches("M1.235,2.346").count(), 2);
    assert!(!xml.contains("1.234567"));
    assert!(!xml.contains("2.345678"));
}
