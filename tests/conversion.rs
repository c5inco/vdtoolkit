use std::fs;
use std::process::Command;

use vdtoolkit::{Compatibility, DiagnosticCode, Error, Severity};

#[test]
fn converts_viewbox_geometry_colors_and_fill_rule() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10" viewBox="10 20 10 5">
        <path d="M10 20 L20 20 L20 25 Z" fill="#123456" fill-opacity=".25" fill-rule="evenodd"/>
    </svg>"##;

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();

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

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();

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

    let asset = vdtoolkit::convert(source).unwrap();
    assert_eq!(
        asset.analysis.compatibility,
        Compatibility::ExactWithNormalization
    );
    assert_eq!(asset.analysis.metrics.paths, 7);
    let xml = asset.to_xml();
    assert_eq!(xml.matches("<path").count(), 7);
    assert!(!xml.contains("android:pathData=\"\""));
}

#[test]
fn resolves_inherited_paint_and_flattens_rotation() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <g fill="#102030" stroke="#405060" stroke-width="2" stroke-opacity=".4"
           transform="rotate(90)">
            <path d="M1 2L3 2"/>
        </g>
    </svg>"##;

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
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

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
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
        let analysis = vdtoolkit::analyze(source.as_bytes()).unwrap();
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

    let xml = vdtoolkit::convert(source).unwrap().to_xml();
    assert!(xml.contains(" C"), "arc should normalize to cubic geometry");
    assert!(xml.contains("22,12"));
    roxmltree::Document::parse(&xml).unwrap();
}

#[test]
fn malformed_svg_is_a_structured_error_not_a_panic() {
    let result = vdtoolkit::convert(br#"<svg><path d="M0 0"></svg>"#);
    assert!(matches!(result, Err(Error::Xml(_))));
}

#[test]
fn never_converts_known_lossy_features() {
    let source = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <image href="https://example.com/pixel.png" width="24" height="24"/>
    </svg>"#;

    let analysis = vdtoolkit::analyze(source).unwrap();
    assert_eq!(analysis.compatibility, Compatibility::Unsupported);
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diagnostic| matches!(diagnostic.code, DiagnosticCode::ExternalImage))
    );
    assert!(matches!(
        vdtoolkit::convert(source),
        Err(Error::Incompatible(_))
    ));
}

#[test]
fn rejects_non_uniform_transforms_on_strokes() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path transform="scale(2 3)" d="M1 1L4 1" fill="none" stroke="#000"/>
    </svg>"##;
    let analysis = vdtoolkit::analyze(source).unwrap();
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
        let analysis = vdtoolkit::analyze(source.as_bytes()).unwrap();
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
    let asset = vdtoolkit::convert(convertible).unwrap();
    assert_eq!(
        asset.analysis.compatibility,
        Compatibility::ExactWithNormalization
    );
    assert_eq!(asset.analysis.metrics.width, 12.0);
    assert_eq!(asset.analysis.metrics.height, 8.0);

    let unsupported = [
        (
            "percentage dimensions",
            DiagnosticCode::UnsupportedDimensions,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="100%" height="24"><path d="M0 0H1V1Z"/></svg>"##,
        ),
        (
            "focal radial gradient",
            DiagnosticCode::UnsupportedGradient,
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><radialGradient id="g" fx=".2"><stop stop-color="#fff"/><stop offset="1"/></radialGradient></defs><path d="M0 0H24V24H0Z" fill="url(#g)"/></svg>"##,
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
        let analysis = vdtoolkit::analyze(source.as_bytes()).unwrap();
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

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
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

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
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

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
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

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
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
        let analysis = vdtoolkit::analyze(source).unwrap();
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

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
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
        let analysis = vdtoolkit::analyze(source).unwrap();
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
    let first = vdtoolkit::convert(source).unwrap().to_xml();
    let second = vdtoolkit::convert(source).unwrap().to_xml();
    assert_eq!(first.as_bytes(), second.as_bytes());
}

#[test]
fn embedding_api_serializes_and_optimizes_an_asset() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M1.234567 2.345678L20.987654 21.876543" fill="#123456"/>
    </svg>"##;
    let mut asset = vdtoolkit::convert(source).unwrap();
    let before = asset.to_xml();

    asset.optimize();
    let after = asset.to_xml();

    assert!(after.len() < before.len());
    assert!(after.contains("M1.235,2.346"));
    assert_eq!(asset.analysis.metrics.estimated_xml_bytes, after.len());
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

    let check = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["check", input.to_str().unwrap(), "--format", "json"])
        .output()
        .unwrap();
    assert_eq!(check.status.code(), Some(2));
    let json: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    assert_eq!(json.as_array().unwrap().len(), 2);

    fs::remove_file(nested.join("bad.svg")).unwrap();
    let convert = Command::new(env!("CARGO_BIN_EXE_vdt"))
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

    let result = Command::new(env!("CARGO_BIN_EXE_vdt"))
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

fn attribute(xml: &str, name: &str) -> f32 {
    let key = format!("android:{name}=\"");
    let start = xml
        .find(&key)
        .unwrap_or_else(|| panic!("missing android:{name} in\n{xml}"))
        + key.len();
    let end = xml[start..].find('"').unwrap() + start;
    xml[start..end].parse().unwrap()
}

fn assert_close(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() < 1.0e-3,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn lowers_user_space_linear_gradient_with_stop_alpha_and_spread() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="24" y2="0" spreadMethod="reflect">
            <stop offset="0" stop-color="#3DDC84"/>
            <stop offset="1" stop-color="#0B6E4F" stop-opacity=".5"/>
        </linearGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#g)" fill-opacity=".8"/>
    </svg>"##;

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
    assert_eq!(asset.analysis.compatibility, Compatibility::Exact);
    assert_eq!(asset.analysis.minimum_api, Some(24));
    assert!(xml.contains(r#"xmlns:aapt="http://schemas.android.com/aapt""#));
    assert!(xml.contains(r#"<aapt:attr name="android:fillColor">"#));
    assert!(!xml.contains("android:fillColor=\""));
    assert!(xml.contains(r#"android:type="linear""#));
    assert!(xml.contains(r#"android:tileMode="mirror""#));
    assert!(xml.contains(r##"<item android:offset="0" android:color="#3DDC84"/>"##));
    assert!(xml.contains(r##"<item android:offset="1" android:color="#800B6E4F"/>"##));
    assert!(xml.contains(r#"android:fillAlpha="0.8""#));
    for (name, expected) in [
        ("startX", 0.0),
        ("startY", 0.0),
        ("endX", 24.0),
        ("endY", 0.0),
    ] {
        assert_close(attribute(&xml, name), expected);
    }
    roxmltree::Document::parse(&xml).unwrap();
}

#[test]
fn solid_drawables_do_not_declare_the_aapt_namespace() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#123456"/>
    </svg>"##;
    let asset = vdtoolkit::convert(source).unwrap();
    assert!(!asset.to_xml().contains("xmlns:aapt"));
    assert_eq!(asset.analysis.minimum_api, Some(21));
}

#[test]
fn maps_object_bounding_box_linear_gradients_to_viewport_coordinates() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g"><stop stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs>
        <rect x="4" y="6" width="8" height="10" fill="url(#g)"/>
    </svg>"##;

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
    assert_eq!(
        asset.analysis.compatibility,
        Compatibility::ExactWithNormalization
    );
    for (name, expected) in [
        ("startX", 4.0),
        ("startY", 6.0),
        ("endX", 12.0),
        ("endY", 6.0),
    ] {
        assert_close(attribute(&xml, name), expected);
    }
}

#[test]
fn skewed_linear_gradients_follow_the_inverse_transpose() {
    // skewX keeps horizontal lines horizontal, so a vertical gradient stays
    // vertical. Mapping only the end point would tilt it to 45 degrees.
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" gradientUnits="userSpaceOnUse" gradientTransform="skewX(45)"
            x1="0" y1="0" x2="0" y2="10"><stop stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs>
        <path d="M0 0H10V10H0Z" fill="url(#g)"/>
    </svg>"##;

    let xml = vdtoolkit::convert(source).unwrap().to_xml();
    for (name, expected) in [
        ("startX", 0.0),
        ("startY", 0.0),
        ("endX", 0.0),
        ("endY", 10.0),
    ] {
        assert_close(attribute(&xml, name), expected);
    }
}

#[test]
fn zero_length_linear_gradients_paint_their_last_stop() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="5" y1="5" x2="5" y2="5">
            <stop stop-color="#3DDC84"/><stop offset="1" stop-color="#0B6E4F" stop-opacity=".5"/>
        </linearGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#g)"/>
    </svg>"##;

    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
    assert!(xml.contains(r##"android:fillColor="#0B6E4F""##));
    assert!(xml.contains(r#"android:fillAlpha="0.5""#));
    assert!(!xml.contains("aapt"));
    assert_eq!(asset.analysis.minimum_api, Some(21));
}

#[test]
fn lowers_circular_radial_gradients_and_gradient_strokes_under_uniform_scale() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs>
            <radialGradient id="r" gradientUnits="userSpaceOnUse" cx="6" cy="6" r="4" spreadMethod="repeat">
                <stop stop-color="#fff"/><stop offset="1" stop-color="#000"/>
            </radialGradient>
            <linearGradient id="s" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="12" y2="0">
                <stop stop-color="#f00"/><stop offset="1" stop-color="#00f"/>
            </linearGradient>
        </defs>
        <g transform="scale(2)">
            <path d="M0 0H12V12H0Z" fill="url(#r)" stroke="url(#s)" stroke-width=".5"/>
        </g>
    </svg>"##;

    let mut asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
    assert!(xml.contains(r#"android:type="radial""#));
    assert!(xml.contains(r#"android:tileMode="repeat""#));
    assert!(xml.contains(r#"<aapt:attr name="android:strokeColor">"#));
    assert!(xml.contains(r#"android:strokeWidth="1""#));
    assert_close(attribute(&xml, "centerX"), 12.0);
    assert_close(attribute(&xml, "centerY"), 12.0);
    assert_close(attribute(&xml, "gradientRadius"), 8.0);
    assert_close(attribute(&xml, "endX"), 24.0);
    assert_eq!(xml.matches("</path>").count(), 1);
    roxmltree::Document::parse(&xml).unwrap();

    asset.optimize();
    roxmltree::Document::parse(&asset.to_xml()).unwrap();
}

#[test]
fn rejects_radial_gradients_android_cannot_draw() {
    for (name, source) in [
        (
            "ellipse from a non-square bounding box",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><radialGradient id="r"><stop/><stop offset="1" stop-color="#fff"/></radialGradient></defs><rect width="20" height="10" fill="url(#r)"/></svg>"##,
        ),
        (
            "non-uniform gradient transform",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><radialGradient id="r" gradientUnits="userSpaceOnUse" cx="12" cy="12" r="6" gradientTransform="scale(2 1)"><stop/><stop offset="1" stop-color="#fff"/></radialGradient></defs><path d="M0 0H24V24H0Z" fill="url(#r)"/></svg>"##,
        ),
        (
            "focal radius",
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><radialGradient id="r" gradientUnits="userSpaceOnUse" cx="12" cy="12" r="10" fr="2"><stop/><stop offset="1" stop-color="#fff"/></radialGradient></defs><path d="M0 0H24V24H0Z" fill="url(#r)"/></svg>"##,
        ),
    ] {
        let analysis = vdtoolkit::analyze(source.as_bytes()).unwrap();
        assert_eq!(analysis.compatibility, Compatibility::Unsupported, "{name}");
        assert!(
            analysis
                .diagnostics
                .iter()
                .any(|diagnostic| matches!(diagnostic.code, DiagnosticCode::UnsupportedGradient)),
            "{name}"
        );
    }
}

#[test]
fn reports_content_bounds_including_strokes_clamped_to_the_viewport() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <rect x="4" y="6" width="8" height="10" fill="#000"/>
        <path d="M2 20H30" stroke="#000" stroke-width="2"/>
    </svg>"##;
    let bounds = vdtoolkit::analyze(source)
        .unwrap()
        .metrics
        .content_bounds
        .unwrap();
    assert_close(bounds.left, 2.0);
    assert_close(bounds.top, 6.0);
    assert_close(bounds.right, 24.0);
    assert_close(bounds.bottom, 21.0);

    let empty = br#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"/>"#;
    assert!(
        vdtoolkit::analyze(empty)
            .unwrap()
            .metrics
            .content_bounds
            .is_none()
    );
}

#[test]
fn cli_directory_runs_report_every_file_and_continue_past_failures() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icons");
    let output = temp.path().join("drawable");
    fs::create_dir_all(&input).unwrap();
    let ok = r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0L1 1" fill="#000"/></svg>"##;
    fs::write(input.join("a_ok.svg"), ok).unwrap();
    fs::write(input.join("b_broken.svg"), "not svg").unwrap();
    fs::write(input.join("c_ok.svg"), ok).unwrap();
    fs::write(
        input.join("d_text.svg"),
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><text>x</text></svg>"#,
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(args)
            .output()
            .unwrap()
    };

    let convert = run(&[input.to_str().unwrap(), "-o", output.to_str().unwrap()]);
    assert_eq!(convert.status.code(), Some(1));
    assert!(output.join("a_ok.xml").is_file());
    assert!(output.join("c_ok.xml").is_file());
    let stderr = String::from_utf8(convert.stderr).unwrap();
    assert!(stderr.contains("b_broken.svg"), "{stderr}");
    assert!(stderr.contains("d_text.svg"), "{stderr}");
    assert!(stderr.contains("2 of 4 SVGs failed"), "{stderr}");

    let check = run(&["check", input.to_str().unwrap(), "--format", "json"]);
    assert_eq!(check.status.code(), Some(1));
    let json: serde_json::Value = serde_json::from_slice(&check.stdout).unwrap();
    let entries = json.as_array().unwrap();
    assert_eq!(entries.len(), 4);
    assert!(
        entries[1]["error"]
            .as_str()
            .unwrap()
            .contains("malformed SVG XML")
    );
    assert_eq!(entries[3]["compatibility"], "unsupported");
    assert!(entries[0]["metrics"]["content_bounds"].is_object());

    let inspect = run(&["inspect", input.to_str().unwrap()]);
    assert_eq!(inspect.status.code(), Some(1));
    let stdout = String::from_utf8(inspect.stdout).unwrap();
    assert!(
        stdout.contains("Could not analyze: malformed SVG XML"),
        "{stdout}"
    );
    assert!(stdout.contains("1 could not be analyzed"), "{stdout}");
    assert!(stdout.contains("Content bounds:"), "{stdout}");
}

#[test]
fn content_bounds_ignore_hidden_and_unpainted_geometry() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" visibility="hidden" fill="#000"/>
        <path d="M2 2H30V6H2Z" fill="none" stroke="none"/>
        <rect x="4" y="6" width="8" height="10" fill="#000"/>
    </svg>"##;
    let analysis = vdtoolkit::analyze(source).unwrap();
    assert_eq!(analysis.metrics.paths, 1);
    let bounds = analysis.metrics.content_bounds.unwrap();
    assert_close(bounds.left, 4.0);
    assert_close(bounds.top, 6.0);
    assert_close(bounds.right, 12.0);
    assert_close(bounds.bottom, 16.0);

    let only_hidden = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" visibility="hidden" fill="#000"/>
    </svg>"##;
    assert!(
        vdtoolkit::analyze(only_hidden)
            .unwrap()
            .metrics
            .content_bounds
            .is_none()
    );
}

#[test]
fn single_stop_gradients_paint_a_solid_color() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs>
            <linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="24" y2="0">
                <stop offset="0" stop-color="#3DDC84" stop-opacity=".5"/>
            </linearGradient>
            <radialGradient id="r"><stop offset="0" stop-color="#123456"/></radialGradient>
        </defs>
        <path d="M0 0H12V24H0Z" fill="url(#g)"/>
        <path d="M12 0H24V24H12Z" fill="url(#r)"/>
    </svg>"##;
    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
    assert!(xml.contains(r##"android:fillColor="#3DDC84""##));
    assert!(xml.contains(r#"android:fillAlpha="0.5""#));
    assert!(xml.contains(r##"android:fillColor="#123456""##));
    assert!(
        !xml.contains("aapt"),
        "single-stop gradients must not become gradients:\n{xml}"
    );
    assert_eq!(asset.analysis.minimum_api, Some(21));
}

#[test]
fn content_bounds_ignore_fully_transparent_paint() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#000" fill-opacity="0"/>
        <path d="M0 0H24V24H0Z" fill="none" stroke="#000" stroke-width="4" stroke-opacity="0"/>
        <g opacity="0"><path d="M0 0H24V24H0Z" fill="#000"/></g>
        <path d="M0 0H24V24H0Z" fill="none" stroke="#000" stroke-width="0"/>
        <rect x="4" y="6" width="8" height="10" fill="#000" stroke="#000" stroke-width="4" stroke-opacity="0"/>
    </svg>"##;
    let analysis = vdtoolkit::analyze(source).unwrap();
    let bounds = analysis.metrics.content_bounds.unwrap();
    // Only the rect's fill paints; its transparent stroke must not widen the bounds.
    assert_close(bounds.left, 4.0);
    assert_close(bounds.top, 6.0);
    assert_close(bounds.right, 12.0);
    assert_close(bounds.bottom, 16.0);

    let nothing_visible = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#000" fill-opacity="0"/>
    </svg>"##;
    assert!(
        vdtoolkit::analyze(nothing_visible)
            .unwrap()
            .metrics
            .content_bounds
            .is_none()
    );
}

#[test]
fn content_bounds_ignore_gradients_whose_stops_are_all_transparent() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs>
            <linearGradient id="clear" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="24" y2="0">
                <stop offset="0" stop-color="#000" stop-opacity="0"/>
                <stop offset="1" stop-color="#fff" stop-opacity="0"/>
            </linearGradient>
            <radialGradient id="faint" gradientUnits="userSpaceOnUse" cx="8" cy="11" r="6">
                <stop offset="0" stop-color="#000" stop-opacity="0"/>
                <stop offset="1" stop-color="#000" stop-opacity=".5"/>
            </radialGradient>
        </defs>
        <path d="M0 0H24V24H0Z" fill="url(#clear)"/>
        <path d="M0 0H24V24H0Z" fill="none" stroke="url(#clear)" stroke-width="4"/>
        <rect x="4" y="6" width="8" height="10" fill="url(#faint)"/>
    </svg>"##;
    let analysis = vdtoolkit::analyze(source).unwrap();
    assert_eq!(
        analysis.metrics.paths, 3,
        "transparent gradients are still emitted"
    );
    let bounds = analysis.metrics.content_bounds.unwrap();
    assert_close(bounds.left, 4.0);
    assert_close(bounds.top, 6.0);
    assert_close(bounds.right, 12.0);
    assert_close(bounds.bottom, 16.0);

    let only_clear = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="clear" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="24" y2="0">
            <stop offset="0" stop-color="#000" stop-opacity="0"/><stop offset="1" stop-color="#fff" stop-opacity="0"/>
        </linearGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#clear)"/>
    </svg>"##;
    assert!(
        vdtoolkit::analyze(only_clear)
            .unwrap()
            .metrics
            .content_bounds
            .is_none()
    );
}

#[test]
fn optimize_keeps_gradient_geometry_valid() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs>
            <radialGradient id="tiny" gradientUnits="userSpaceOnUse" cx="12" cy="12" r="0.0004">
                <stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#000"/>
            </radialGradient>
            <linearGradient id="short" gradientUnits="userSpaceOnUse" x1="5" y1="5" x2="5.0004" y2="5">
                <stop offset="0" stop-color="#f00"/><stop offset="1" stop-color="#00f"/>
            </linearGradient>
        </defs>
        <path d="M0 0H12V24H0Z" fill="url(#tiny)"/>
        <path d="M12 0H24V24H12Z" fill="url(#short)"/>
    </svg>"##;
    let mut asset = vdtoolkit::convert(source).unwrap();
    asset.optimize();
    let xml = asset.to_xml();
    let radius = attribute(&xml, "gradientRadius");
    assert!(
        radius > 0.0,
        "radius must stay positive after optimize, got {radius}"
    );
    assert!(
        (attribute(&xml, "startX"), attribute(&xml, "startY"))
            != (attribute(&xml, "endX"), attribute(&xml, "endY")),
        "linear gradient axis must not collapse to a point:\n{xml}"
    );
    roxmltree::Document::parse(&xml).unwrap();
}

#[test]
fn gradient_degeneracy_is_judged_after_the_gradient_transform() {
    // A 0.0003-unit axis scaled by 100000 spans 30 viewport units.
    let scaled = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="0.0003" y2="0"
            gradientTransform="scale(100000)"><stop stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#g)"/>
    </svg>"##;
    let xml = vdtoolkit::convert(scaled).unwrap().to_xml();
    assert!(
        xml.contains("aapt"),
        "scaled gradient must stay a gradient:\n{xml}"
    );
    assert_close(attribute(&xml, "endX") - attribute(&xml, "startX"), 30.0);

    // A nonzero axis below the writer's resolution is a hard edge between the
    // stop colors, so it stays a gradient with distinctly serialized ends.
    let sub_resolution = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="5" y1="5" x2="5.0000004" y2="5.0000004">
            <stop stop-color="#f00"/><stop offset="1" stop-color="#00f"/></linearGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#g)"/>
    </svg>"##;
    let xml = vdtoolkit::convert(sub_resolution).unwrap().to_xml();
    assert!(
        xml.contains("aapt"),
        "sub-resolution axis must stay a gradient:\n{xml}"
    );
    assert!(
        attribute(&xml, "startX") != attribute(&xml, "endX")
            && attribute(&xml, "startY") != attribute(&xml, "endY"),
        "axis ends must serialize distinctly:\n{xml}"
    );
    assert!((attribute(&xml, "endX") - 5.0).abs() < 0.00001);
}

#[test]
fn radial_radius_stays_positive_without_optimization() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><radialGradient id="r" gradientUnits="userSpaceOnUse" cx="12" cy="12" r="0.0000004">
            <stop stop-color="#fff"/><stop offset="1" stop-color="#000"/></radialGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#r)"/>
    </svg>"##;
    let xml = vdtoolkit::convert(source).unwrap().to_xml();
    let radius = attribute(&xml, "gradientRadius");
    assert!(
        radius > 0.0,
        "unoptimized radius must be positive, got {radius}:\n{xml}"
    );
}

#[test]
fn content_bounds_use_the_serialized_gradient_alpha() {
    // Every stop is positive but below 0.5/255, so each item serializes with
    // alpha 00 and the gradient renders nothing.
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="faint" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="24" y2="0">
            <stop offset="0" stop-color="#000" stop-opacity="0.001"/>
            <stop offset="1" stop-color="#fff" stop-opacity="0.0019"/>
        </linearGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#faint)"/>
        <rect x="4" y="6" width="8" height="10" fill="#000"/>
    </svg>"##;
    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
    assert!(xml.contains(r##"android:color="#00000000""##), "{xml}");
    assert!(xml.contains(r##"android:color="#00FFFFFF""##), "{xml}");
    let bounds = asset.analysis.metrics.content_bounds.unwrap();
    assert_close(bounds.left, 4.0);
    assert_close(bounds.top, 6.0);
    assert_close(bounds.right, 12.0);
    assert_close(bounds.bottom, 16.0);

    // The first opacity that rounds to alpha 01 counts as painted.
    let barely = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="24" y2="0">
            <stop offset="0" stop-color="#000" stop-opacity="0.002"/>
            <stop offset="1" stop-color="#000" stop-opacity="0.002"/>
        </linearGradient></defs>
        <path d="M0 0H24V24H0Z" fill="url(#g)"/>
    </svg>"##;
    let asset = vdtoolkit::convert(barely).unwrap();
    assert!(asset.to_xml().contains(r##"android:color="#01000000""##));
    assert_close(asset.analysis.metrics.content_bounds.unwrap().right, 24.0);
}

#[test]
fn content_bounds_use_the_serialized_path_alpha_and_stroke_width() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#000" fill-opacity="0.0000004"/>
        <path d="M0 0H24V24H0Z" fill="none" stroke="#000" stroke-width="4" stroke-opacity="0.0000004"/>
        <path d="M0 0H24V24H0Z" fill="none" stroke="#000" stroke-width="0.0000004"/>
        <rect x="4" y="6" width="8" height="10" fill="#000"/>
    </svg>"##;
    let asset = vdtoolkit::convert(source).unwrap();
    let xml = asset.to_xml();
    assert!(xml.contains(r#"android:fillAlpha="0""#), "{xml}");
    assert!(xml.contains(r#"android:strokeAlpha="0""#), "{xml}");
    assert!(xml.contains(r#"android:strokeWidth="0""#), "{xml}");
    let bounds = asset.analysis.metrics.content_bounds.unwrap();
    assert_close(bounds.left, 4.0);
    assert_close(bounds.top, 6.0);
    assert_close(bounds.right, 12.0);
    assert_close(bounds.bottom, 16.0);

    // The smallest alpha the writer keeps nonzero still counts as painted.
    let faint = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#000" fill-opacity="0.000001"/>
    </svg>"##;
    let asset = vdtoolkit::convert(faint).unwrap();
    assert!(asset.to_xml().contains(r#"android:fillAlpha="0.000001""#));
    assert_close(asset.analysis.metrics.content_bounds.unwrap().right, 24.0);
}

const LARGE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320">
    <path d="M0 0H480V320Z" fill="#123456"/>
</svg>"##;

#[test]
fn large_drawables_warn_without_changing_compatibility() {
    let asset = vdtoolkit::convert(LARGE).unwrap();
    assert_eq!(asset.analysis.compatibility, Compatibility::Exact);
    let [warning] = asset.analysis.diagnostics.as_slice() else {
        panic!("expected one diagnostic: {:?}", asset.analysis.diagnostics);
    };
    assert_eq!(
        warning.code.as_str(),
        DiagnosticCode::LargeDimensions.as_str()
    );
    assert!(matches!(warning.severity, Severity::Warning));
    assert!(warning.message.contains("480×320dp"));

    let at_limit = br##"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="200">
        <path d="M0 0H200V200Z" fill="#123456"/>
    </svg>"##;
    assert!(
        vdtoolkit::convert(at_limit)
            .unwrap()
            .analysis
            .diagnostics
            .is_empty()
    );
}

#[test]
fn fits_adaptive_layers_by_uniform_scale_and_centering() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="12" viewBox="0 0 24 12">
        <defs><radialGradient id="g" cx="12" cy="6" r="6" gradientUnits="userSpaceOnUse">
            <stop offset="0" stop-color="#FFF"/><stop offset="1" stop-color="#000"/>
        </radialGradient></defs>
        <path d="M0 0H24V12H0Z" fill="url(#g)" stroke="#123456" stroke-width="2"/>
    </svg>"##;

    let mut asset = vdtoolkit::convert(source).unwrap();
    let api = asset.analysis.minimum_api;
    asset
        .fit_adaptive_layer(vdtoolkit::ADAPTIVE_ICON_SAFE_ZONE)
        .unwrap();
    let xml = asset.to_xml();

    // 24 wide fits into 66: scale 2.75, offset (21, 37.5).
    assert!(xml.contains("android:width=\"108dp\""));
    assert!(xml.contains("android:viewportHeight=\"108\""));
    assert!(xml.contains("android:pathData=\"M21,37.5 L87,37.5 L87,70.5 L21,70.5 Z\""));
    assert!(xml.contains("android:strokeWidth=\"5.5\""));
    assert!(xml.contains("android:centerX=\"54\""));
    assert!(xml.contains("android:centerY=\"54\""));
    assert!(xml.contains("android:gradientRadius=\"16.5\""));
    assert_eq!(asset.analysis.minimum_api, api);
    let metrics = &asset.analysis.metrics;
    assert_eq!(metrics.viewport_width, 108.0);
    // Bounds are clamped to the source viewport before being moved.
    let bounds = metrics.content_bounds.unwrap();
    assert_eq!((bounds.left, bounds.top), (21.0, 37.5));
    assert_eq!((bounds.right, bounds.bottom), (87.0, 70.5));
    assert_eq!(metrics.estimated_xml_bytes, xml.len());

    assert!(matches!(
        asset.fit_adaptive_layer(0.0),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        asset.fit_adaptive_layer(109.0),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn detects_content_outside_the_safe_zone() {
    let logo = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2H22V22H2Z" fill="#000"/></svg>"##;
    let mut full = vdtoolkit::convert(logo).unwrap();
    full.fit_adaptive_layer(vdtoolkit::ADAPTIVE_ICON_SIZE)
        .unwrap();
    let bounds = full.outside_adaptive_safe_zone().unwrap();
    assert_eq!((bounds.left, bounds.right), (9.0, 99.0));
    let mut safe = vdtoolkit::convert(logo).unwrap();
    safe.fit_adaptive_layer(vdtoolkit::ADAPTIVE_ICON_SAFE_ZONE)
        .unwrap();
    assert!(safe.outside_adaptive_safe_zone().is_none());
    // Content at exactly the safe zone edge is inside it.
    let mut edge = vdtoolkit::convert(logo).unwrap();
    edge.fit_adaptive_layer(72.0).unwrap();
    assert!(edge.outside_adaptive_safe_zone().is_none());
}

#[test]
fn composes_a_masked_legacy_icon() {
    let logo = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2H22V22H2Z" fill="#102030"/></svg>"##;
    let mut foreground = vdtoolkit::convert(logo).unwrap();
    foreground.fit_adaptive_layer(66.0).unwrap();
    let background = vdtoolkit::Asset::solid_adaptive_layer("#803DDC84").unwrap();
    assert!(
        background
            .to_xml()
            .contains("android:fillAlpha=\"0.501961\"")
    );
    assert!(matches!(
        vdtoolkit::Asset::solid_adaptive_layer("#fff"),
        Err(Error::InvalidInput(_))
    ));

    let legacy = vdtoolkit::Asset::legacy_launcher_icon(&background, &foreground);
    let xml = legacy.to_xml();
    assert!(xml.contains("android:width=\"48dp\""));
    assert!(xml.contains("android:viewportWidth=\"108\""));
    assert!(xml.starts_with("<?xml"));
    assert!(xml.contains("<clip-path\n        android:pathData=\"M54,18 C"));
    assert_eq!(xml.matches("<group>").count(), 2);
    assert!(xml.contains("android:fillColor=\"#3DDC84\""));
    assert!(xml.contains("android:pathData=\"M26.5,26.5 L81.5,26.5 L81.5,81.5 L26.5,81.5 Z\""));
    assert_eq!(legacy.analysis.minimum_api, Some(21));
    assert_eq!(legacy.analysis.compatibility, Compatibility::Exact);
    let metrics = &legacy.analysis.metrics;
    assert_eq!(
        (metrics.paths, metrics.clip_paths, metrics.groups),
        (2, 1, 2)
    );
    assert_eq!(metrics.estimated_xml_bytes, xml.len());

    // A gradient background lifts the legacy icon to API 24.
    let gradient = br##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108">
        <defs><linearGradient id="g" x1="0" y1="0" x2="108" y2="0" gradientUnits="userSpaceOnUse">
            <stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#FFF"/>
        </linearGradient></defs>
        <rect width="108" height="108" fill="url(#g)"/>
    </svg>"##;
    let mut background = vdtoolkit::convert(gradient).unwrap();
    background.fit_adaptive_layer(108.0).unwrap();
    let legacy = vdtoolkit::Asset::legacy_launcher_icon(&background, &foreground);
    assert_eq!(legacy.analysis.minimum_api, Some(24));
}

#[test]
fn writes_adaptive_icon_resources() {
    assert_eq!(
        vdtoolkit::adaptive_icon_xml("@color/bg", "@drawable/fg", Some("@drawable/mono")),
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n\
         \x20   <background android:drawable=\"@color/bg\"/>\n\
         \x20   <foreground android:drawable=\"@drawable/fg\"/>\n\
         \x20   <monochrome android:drawable=\"@drawable/mono\"/>\n\
         </adaptive-icon>\n"
    );
    assert!(
        !vdtoolkit::adaptive_icon_xml("@color/bg", "@drawable/fg", None).contains("monochrome")
    );
    assert_eq!(
        vdtoolkit::color_resource_xml("ic_launcher_background", "#3DDC84"),
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n\
         \x20   <color name=\"ic_launcher_background\">#3DDC84</color>\n</resources>\n"
    );
}

#[test]
fn fit_within_scales_size_but_keeps_the_viewport() {
    let mut asset = vdtoolkit::convert(LARGE).unwrap();
    assert!(!asset.fit_within(500.0));
    assert!(asset.fit_within(200.0));
    let xml = asset.to_xml();
    assert!(xml.contains("android:width=\"200dp\""));
    assert!(xml.contains("android:height=\"133dp\""));
    assert!(xml.contains("android:viewportWidth=\"480\""));
    assert!(xml.contains("android:viewportHeight=\"320\""));
    assert_eq!(asset.analysis.metrics.width, 200.0);
    assert_eq!(asset.analysis.metrics.height, 133.0);
    assert_eq!(asset.analysis.metrics.estimated_xml_bytes, xml.len());
    assert!(asset.analysis.diagnostics.is_empty());

    let tall = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="600">
        <path d="M0 0H100V600Z" fill="#123456"/>
    </svg>"##;
    let mut asset = vdtoolkit::convert(tall).unwrap();
    assert!(asset.fit_within(300.0));
    let xml = asset.to_xml();
    assert!(xml.contains("android:width=\"50dp\""));
    assert!(xml.contains("android:height=\"300dp\""));
    assert!(
        asset
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "SVGVD016")
    );
}

#[test]
fn convert_prints_large_dimension_warnings_to_stderr() {
    let dir = std::env::temp_dir().join(format!("vdtoolkit-large-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let input = dir.join("large.svg");
    fs::write(&input, LARGE).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["convert", input.to_str().unwrap()])
        .output()
        .unwrap();
    fs::remove_dir_all(&dir).unwrap();

    assert!(output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("warning: "));
    assert!(stderr.contains("SVGVD016"));
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("android:width=\"480dp\""));
}

#[test]
fn cli_adaptive_writes_layers_icon_and_color_resource() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    let background = temp.path().join("bg.svg");
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 2L2 22h20z" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    fs::write(
        &background,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108"><rect width="108" height="108" fill="#FFFFFF"/></svg>"##,
    )
    .unwrap();
    let res = temp.path().join("res");

    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&foreground)
        .arg("--background")
        .arg(&background)
        .arg("--monochrome")
        .arg(&foreground)
        .args(["--fit", "66", "-o"])
        .arg(&res)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 5);
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.is_empty(),
        "fit 66 keeps the logo in the safe zone: {stderr}"
    );

    let foreground_xml =
        fs::read_to_string(res.join("drawable/ic_launcher_foreground.xml")).unwrap();
    assert!(foreground_xml.contains("android:width=\"108dp\""));
    assert!(foreground_xml.contains("android:pathData=\"M54,26.5 L26.5,81.5 L81.5,81.5 Z\""));
    let background_xml =
        fs::read_to_string(res.join("drawable/ic_launcher_background.xml")).unwrap();
    assert!(background_xml.contains("android:pathData=\"M0,0 L108,0 L108,108 L0,108 Z\""));
    assert_eq!(
        fs::read_to_string(res.join("drawable/ic_launcher_monochrome.xml")).unwrap(),
        foreground_xml
    );
    let icon = fs::read_to_string(res.join("mipmap-anydpi-v26/ic_launcher.xml")).unwrap();
    assert!(icon.contains("<background android:drawable=\"@drawable/ic_launcher_background\"/>"));
    assert!(icon.contains("<foreground android:drawable=\"@drawable/ic_launcher_foreground\"/>"));
    assert!(icon.contains("<monochrome android:drawable=\"@drawable/ic_launcher_monochrome\"/>"));
    assert_eq!(
        fs::read_to_string(res.join("mipmap-anydpi-v26/ic_launcher_round.xml")).unwrap(),
        icon
    );

    // A solid background becomes a color resource, and --name renames everything.
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&foreground)
        .args(["--background-color", "3ddc84", "--name", "ic_app", "-o"])
        .arg(&res)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(res.join("values/ic_app_background.xml")).unwrap(),
        vdtoolkit::color_resource_xml("ic_app_background", "#3DDC84")
    );
    let icon = fs::read_to_string(res.join("mipmap-anydpi-v26/ic_app.xml")).unwrap();
    assert!(icon.contains("<background android:drawable=\"@color/ic_app_background\"/>"));
    assert!(icon.contains("<foreground android:drawable=\"@drawable/ic_app_foreground\"/>"));
    assert!(!icon.contains("monochrome"));
    assert!(res.join("drawable/ic_app_foreground.xml").is_file());
    assert!(!res.join("drawable/ic_app_background.xml").exists());
    assert!(!res.join("mipmap").exists());

    // The default fit leaves a plain logo outside the safe zone; --legacy
    // adds the masked fallback icon.
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&foreground)
        .args([
            "--background-color",
            "#3DDC84",
            "--name",
            "ic_old",
            "--legacy",
            "-o",
        ])
        .arg(&res)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("warning:"), "{stderr}");
    assert!(stderr.contains("outside the 66dp safe zone"), "{stderr}");
    let legacy = fs::read_to_string(res.join("mipmap/ic_old.xml")).unwrap();
    assert!(legacy.contains("android:width=\"48dp\""));
    assert!(legacy.contains("<clip-path"));
    assert!(legacy.contains("android:fillColor=\"#3DDC84\""));
    assert_eq!(
        fs::read_to_string(res.join("mipmap/ic_old_round.xml")).unwrap(),
        legacy
    );
}

#[test]
fn cli_adaptive_rejects_bad_input_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    let text = temp.path().join("text.svg");
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 2L2 22h20z"/></svg>"##,
    )
    .unwrap();
    fs::write(
        &text,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><text>x</text></svg>"##,
    )
    .unwrap();
    let res = temp.path().join("res");
    let run = |extra: &[&str], layer: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(layer)
            .args(extra)
            .arg("-o")
            .arg(&res)
            .output()
            .unwrap()
    };

    let cases: [(&[&str], &std::path::Path, &str); 6] = [
        (&[], &foreground, "--background"),
        (&["--background-color", "#fff"], &foreground, "#RRGGBB"),
        (
            &["--background-color", "#ffffff", "--name", "Icon"],
            &foreground,
            "resource name",
        ),
        (
            &["--background-color", "#ffffff", "--fit", "0"],
            &foreground,
            "--fit",
        ),
        (&["--background-color", "#ffffff"], &text, "SVGVD006"),
        (
            &["--background", text.to_str().unwrap()],
            &foreground,
            "not exactly representable",
        ),
    ];
    for (extra, layer, message) in cases {
        let output = run(extra, layer);
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!output.status.success(), "{extra:?}");
        assert!(stderr.contains(message), "{extra:?}: {stderr}");
    }
    assert!(!res.exists());
}
