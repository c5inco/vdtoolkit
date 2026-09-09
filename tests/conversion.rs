use std::fs;
use std::process::Command;

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
            .all(|path| !path.path_data.0.is_empty())
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
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M1.234567 2.345678L20.987654 21.876543" fill="#123456"/></svg>"##,
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
    assert!(fs::read_to_string(output).unwrap().contains("M1.235,2.346"));
}
