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
    let [note] = asset.analysis.diagnostics.as_slice() else {
        panic!("expected one diagnostic: {:?}", asset.analysis.diagnostics);
    };
    assert_eq!(
        note.code.as_str(),
        DiagnosticCode::ApiLevelRequirement.as_str()
    );
    assert!(matches!(note.severity, Severity::Info));
    assert_eq!(
        note.message,
        "needs API 24 for even-odd fills (android:fillType)"
    );
}

#[test]
fn api_level_note_names_every_api_24_feature_without_changing_exit_codes() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs>
            <linearGradient id="g" x1="0" y1="0" x2="24" y2="0" gradientUnits="userSpaceOnUse">
                <stop offset="0" stop-color="#123456"/><stop offset="1" stop-color="#ABCDEF"/>
            </linearGradient>
            <clipPath id="a"><path d="M0 0H20V20H0Z"/></clipPath>
            <clipPath id="b"><path d="M4 4H24V24H4Z"/></clipPath>
        </defs>
        <g clip-path="url(#a)"><g clip-path="url(#b)">
            <path d="M0 0H24V24H0Z M6 6V18H18V6Z" fill="url(#g)" fill-rule="evenodd"/>
        </g></g>
    </svg>"##;

    let asset = vdtoolkit::convert(source).unwrap();
    assert_eq!(asset.analysis.minimum_api, Some(24));
    let note = asset
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "SVGVD004")
        .expect("API level note");
    assert!(matches!(note.severity, Severity::Info));
    assert_eq!(
        note.message,
        "needs API 24 for gradients, even-odd fills (android:fillType), more than one clip path"
    );
    assert_eq!(
        note.suggestion.as_deref(),
        Some(
            "Use it with minSdk 24 or higher; to support API 21, use solid colors instead of \
             gradients; draw holes with reversed path direction under the nonzero fill \
             rule; combine the clips into a single clip path."
        )
    );

    let plain = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M2 2H22V22H2Z" fill="#123456"/>
    </svg>"##;
    assert!(
        vdtoolkit::convert(plain)
            .unwrap()
            .analysis
            .diagnostics
            .is_empty()
    );

    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("gradient.svg");
    fs::write(&input, source).unwrap();
    let vdt = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(args)
            .arg(&input)
            .output()
            .unwrap()
    };
    for args in [&["check"][..], &["inspect", "--format", "json"]] {
        let output = vdt(args);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stdout).contains("SVGVD004"));
    }
    let converted = vdt(&["convert"]);
    assert!(converted.status.success(), "{converted:?}");
    assert!(!String::from_utf8_lossy(&converted.stderr).contains("SVGVD004"));
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
fn compact_xml_preserves_the_pretty_drawable_structure() {
    const ANDROID: &str = "http://schemas.android.com/apk/res/android";
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M1 2H23V22H1Z" fill="#123456" fill-opacity=".5"/>
    </svg>"##;
    let asset = vdtoolkit::convert(source).unwrap();
    let pretty = asset.to_xml();
    let compact = asset.to_compact_xml();

    assert!(compact.len() < pretty.len());
    assert!(!compact.contains('\n'));
    let pretty = roxmltree::Document::parse(&pretty).unwrap();
    let compact = roxmltree::Document::parse(&compact).unwrap();
    assert_eq!(
        pretty.root_element().attribute((ANDROID, "viewportWidth")),
        compact.root_element().attribute((ANDROID, "viewportWidth"))
    );
    let pretty_path = pretty
        .descendants()
        .find(|node| node.has_tag_name("path"))
        .unwrap();
    let compact_path = compact
        .descendants()
        .find(|node| node.has_tag_name("path"))
        .unwrap();
    for attribute in ["pathData", "fillColor", "fillAlpha"] {
        assert_eq!(
            pretty_path.attribute((ANDROID, attribute)),
            compact_path.attribute((ANDROID, attribute)),
            "android:{attribute}"
        );
    }
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
    assert!(after.contains("M1.235 2.346"));
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
    assert_eq!(xml.matches("M1.235 2.346").count(), 2);
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
        .fit_adaptive_layer(vdtoolkit::Fit::contain(vdtoolkit::ADAPTIVE_ICON_SAFE_ZONE))
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
        asset.fit_adaptive_layer(vdtoolkit::Fit::contain(0.0)),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        asset.fit_adaptive_layer(vdtoolkit::Fit::contain(109.0)),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn fitting_an_adaptive_layer_drops_the_large_dimensions_warning() {
    let mut asset = vdtoolkit::convert(LARGE).unwrap();
    assert_eq!(asset.analysis.diagnostics.len(), 1);
    asset
        .fit_adaptive_layer(vdtoolkit::Fit::contain(vdtoolkit::ADAPTIVE_ICON_SIZE))
        .unwrap();
    assert!(asset.analysis.diagnostics.is_empty());
    assert_eq!(asset.analysis.metrics.width, 108.0);
}

#[test]
fn measures_how_far_a_foreground_reaches_from_the_centre() {
    // A launcher mask is a circle, so what matters is distance from the
    // centre, not a bounding box: the corners of a box sit further out than
    // the artwork inside it.
    let square = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2H22V22H2Z" fill="#000"/></svg>"##;
    let round = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="12" fill="#000"/></svg>"##;
    let reach = |svg: &[u8], fit: f32| {
        let mut asset = vdtoolkit::convert(svg).unwrap();
        asset
            .fit_adaptive_layer(vdtoolkit::Fit::contain(fit))
            .unwrap();
        asset.painted_reach().unwrap()
    };

    // The same fit, two shapes: the square's corners reach much further.
    let square_66 = reach(square, 66.0);
    let round_66 = reach(round, 66.0);
    assert!((square_66 - 38.9).abs() < 0.3, "{square_66}");
    assert!((round_66 - 33.0).abs() < 0.3, "{round_66}");
    assert!(round_66 < vdtoolkit::ADAPTIVE_ICON_MASK_RADIUS);
    assert!(square_66 > vdtoolkit::ADAPTIVE_ICON_MASK_RADIUS);

    let codes = |svg: &[u8], fit: f32| -> Vec<(String, String)> {
        let mut asset = vdtoolkit::convert(svg).unwrap();
        asset
            .to_icon(
                vdtoolkit::IconKind::AdaptiveForeground,
                vdtoolkit::Fit::contain(fit),
            )
            .unwrap();
        asset
            .analysis
            .diagnostics
            .iter()
            .filter(|d| d.code.as_str() == "SVGVD019")
            .map(|d| {
                (
                    format!("{:?}", d.severity),
                    d.suggestion.clone().unwrap_or_default(),
                )
            })
            .collect()
    };

    // A round logo filling the 66dp box is fine, and is exactly the false
    // positive a bounding-box test against the circle would have produced.
    assert!(codes(round, 66.0).is_empty());
    // A square one at the same fit is clipped, and the remedy names the fit
    // that would bring it inside the circle.
    let square_finding = codes(square, 66.0);
    assert_eq!(square_finding.len(), 1);
    assert_eq!(square_finding[0].0, "Warning");
    assert!(
        square_finding[0].1.contains("--fit 56"),
        "{}",
        square_finding[0].1
    );
    // Taking that advice clears it.
    assert!(codes(square, 56.0).is_empty());

    // Between the mask and the safe zone the finding is a note, not a warning:
    // a circular mask still shows it, another mask may not.
    let note = codes(square, 61.0);
    assert_eq!(note.len(), 1);
    assert_eq!(note[0].0, "Info");

    // Nothing painted, nothing to measure.
    let blank = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"></svg>"##;
    let mut empty = vdtoolkit::convert(blank).unwrap();
    empty
        .fit_adaptive_layer(vdtoolkit::Fit::contain(66.0))
        .unwrap();
    assert!(empty.painted_reach().is_none());
}

#[test]
fn detects_backgrounds_that_do_not_fill_the_layer() {
    let fitted = |svg: &str| {
        let mut asset = vdtoolkit::convert(svg.as_bytes()).unwrap();
        asset
            .fit_adaptive_layer(vdtoolkit::Fit::contain(vdtoolkit::ADAPTIVE_ICON_SIZE))
            .unwrap();
        asset
    };
    let svg = |size: &str, content: &str| {
        format!(r#"<svg xmlns="http://www.w3.org/2000/svg" {size}>{content}</svg>"#)
    };

    // Full bleed fills the layer at any square size, including overscan.
    assert!(
        fitted(&svg(
            r#"width="24" height="24""#,
            r##"<rect width="24" height="24" fill="#3DDC84"/>"##
        ))
        .fills_adaptive_layer()
    );
    assert!(
        fitted(&svg(
            r#"width="48" height="48""#,
            r##"<rect x="-10" y="-10" width="68" height="68" fill="#3DDC84"/>"##
        ))
        .fills_adaptive_layer()
    );

    // Full bleed 16:9 artwork is letterboxed into a band.
    let wide = fitted(&svg(
        r#"width="160" height="90""#,
        r##"<rect width="160" height="90" fill="#3DDC84"/>"##,
    ));
    assert!(!wide.fills_adaptive_layer());
    let bounds = wide.analysis.metrics.content_bounds.unwrap();
    for (actual, expected) in [
        (bounds.left, 0.0),
        (bounds.right, 108.0),
        (bounds.top, 23.625),
        (bounds.bottom, 84.375),
    ] {
        assert!((actual - expected).abs() < 1e-3, "{bounds:?}");
    }

    // Full-bleed paint under an inset clip, and a hole, are not filling
    // either, even though their bounding boxes reach every edge.
    assert!(
        !fitted(&svg(
            r#"width="108" height="108""#,
            r##"<defs><clipPath id="c"><rect x="4" y="4" width="100" height="100"/></clipPath></defs>
                <rect width="108" height="108" fill="#3DDC84" clip-path="url(#c)"/>"##
        ))
        .fills_adaptive_layer()
    );
    assert!(
        !fitted(&svg(
            r#"width="108" height="108""#,
            r##"<path d="M0 0H108V108H0Z M40 40H68V68H40Z" fill="#3DDC84" fill-rule="evenodd"/>"##
        ))
        .fills_adaptive_layer()
    );
    // Semi-transparent paint still covers; only unpainted pixels count.
    assert!(
        fitted(&svg(
            r#"width="108" height="108""#,
            r##"<rect width="108" height="108" fill="#3DDC84" fill-opacity=".4"/>"##
        ))
        .fills_adaptive_layer()
    );

    // Inset content, and a background that paints nothing.
    assert!(
        !fitted(&svg(
            r#"width="108" height="108""#,
            r##"<rect x="4" width="104" height="108" fill="#3DDC84"/>"##
        ))
        .fills_adaptive_layer()
    );
    assert!(
        !fitted(&svg(
            r#"width="108" height="108""#,
            r#"<rect width="108" height="108" fill="none"/>"#
        ))
        .fills_adaptive_layer()
    );
}

#[test]
fn fits_full_bleed_gradient_backgrounds() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48">
        <defs><linearGradient id="g" x1="0" y1="0" x2="24" y2="12"
                gradientUnits="userSpaceOnUse" spreadMethod="reflect">
            <stop offset="0" stop-color="#1267D6"/>
            <stop offset=".5" stop-color="#E37A19" stop-opacity=".6"/>
            <stop offset="1" stop-color="#159A55"/>
        </linearGradient></defs>
        <rect width="48" height="48" fill="url(#g)"/>
    </svg>"##;

    let mut asset = vdtoolkit::convert(source).unwrap();
    asset
        .fit_adaptive_layer(vdtoolkit::Fit::contain(vdtoolkit::ADAPTIVE_ICON_SIZE))
        .unwrap();
    assert!(asset.fills_adaptive_layer());
    assert_eq!(asset.analysis.minimum_api, Some(24));
    let xml = asset.to_xml();

    // Scale 2.25 with no offset: the axis (0,0)-(24,12) becomes (0,0)-(54,27).
    assert!(xml.contains("android:pathData=\"M0,0 L108,0 L108,108 L0,108 Z\""));
    assert!(xml.contains("android:tileMode=\"mirror\""));
    let attribute = |name: &str| -> f32 {
        let key = format!("android:{name}=\"");
        let start = xml.find(&key).unwrap_or_else(|| panic!("{name}\n{xml}")) + key.len();
        xml[start..start + xml[start..].find('"').unwrap()]
            .parse()
            .unwrap()
    };
    for (name, expected) in [
        ("startX", 0.0),
        ("startY", 0.0),
        ("endX", 54.0),
        ("endY", 27.0),
    ] {
        assert!((attribute(name) - expected).abs() < 1e-4, "{name}\n{xml}");
    }
    assert!(xml.contains("<item android:offset=\"0.5\" android:color=\"#99E37A19\"/>"));
}

#[test]
fn cli_adaptive_places_a_raster_foreground_and_judges_it_as_a_foreground() {
    let temp = tempfile::tempdir().unwrap();
    let res = temp.path().join("res");

    // Circles of known radius on a 432px layer: the 66dp safe zone is the
    // middle 264px, so r=120 sits inside it and r=200 runs past it.
    let disc = |radius: f32, alpha: u8| {
        let size = 432u32;
        let mut pixels = vec![0u8; (size * size * 4) as usize];
        let center = size as f32 / 2.0;
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (x as f32 + 0.5 - center, y as f32 + 0.5 - center);
                if dx * dx + dy * dy <= radius * radius {
                    let at = ((y * size + x) * 4) as usize;
                    pixels[at..at + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, alpha]);
                }
            }
        }
        let mut out = Vec::new();
        image_webp::WebPEncoder::new(std::io::Cursor::new(&mut out))
            .encode(&pixels, size, size, image_webp::ColorType::Rgba8)
            .unwrap();
        out
    };
    let write = |name: &str, bytes: &[u8]| {
        let path = temp.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    };
    let inside = write("inside.webp", &disc(120.0, 0xFF));
    let outside = write("outside.webp", &disc(200.0, 0xFF));
    let opaque = write("opaque.webp", &disc(1000.0, 0xFF));
    let empty = write("empty.webp", &disc(0.0, 0xFF));

    let run = |args: &[&str], out: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_vdt"))
            .arg("adaptive")
            .args(args)
            .args(["--background-color", "#3DDC84", "-o"])
            .arg(out)
            .output()
            .unwrap()
    };
    let codes = |output: &std::process::Output| -> Vec<String> {
        String::from_utf8(output.stderr.clone())
            .unwrap()
            .lines()
            .filter_map(|line| {
                line.split_whitespace()
                    .find(|word| word.starts_with("SVGVD"))
            })
            .map(str::to_owned)
            .collect()
    };

    // Artwork inside the safe zone, with transparency around it, is what a
    // foreground should be.
    let output = run(&["--foreground-image", inside.to_str().unwrap()], &res);
    assert!(output.status.success(), "{output:?}");
    assert!(codes(&output).is_empty(), "{:?}", codes(&output));
    let icon = fs::read_to_string(res.join("mipmap-anydpi-v26/ic_launcher.xml")).unwrap();
    assert!(
        icon.contains("<foreground android:drawable=\"@mipmap/ic_launcher_foreground\"/>"),
        "{icon}"
    );
    assert_eq!(
        fs::read(res.join("mipmap-nodpi/ic_launcher_foreground.webp")).unwrap(),
        fs::read(&inside).unwrap()
    );
    assert!(
        !res.join("drawable-anydpi/ic_launcher_foreground.xml")
            .exists()
    );

    // A foreground is judged the opposite way round to a background.
    assert_eq!(
        codes(&run(
            &["--foreground-image", outside.to_str().unwrap()],
            &temp.path().join("a")
        )),
        ["SVGVD019"]
    );
    assert_eq!(
        codes(&run(
            &["--foreground-image", opaque.to_str().unwrap()],
            &temp.path().join("b")
        )),
        ["SVGVD026"]
    );
    assert_eq!(
        codes(&run(
            &["--foreground-image", empty.to_str().unwrap()],
            &temp.path().join("c")
        )),
        ["SVGVD018"]
    );

    // A JPEG has no alpha, so as a foreground it would hide the background.
    let jpeg = write("fg.jpg", include_bytes!("fixtures/background_square.jpg"));
    let output = run(
        &["--foreground-image", jpeg.to_str().unwrap()],
        &temp.path().join("d"),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("a JPEG has no alpha channel"),
    );

    // --fit places vector artwork; there is none to place here.
    let output = run(
        &[
            "--foreground-image",
            inside.to_str().unwrap(),
            "--fit",
            "66",
        ],
        &temp.path().join("e"),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--fit places vector artwork"),
    );

    // ...but it still applies when a vector monochrome layer is also given.
    let svg = temp.path().join("mono.svg");
    fs::write(
        &svg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
    )
    .unwrap();
    let output = run(
        &[
            "--foreground-image",
            inside.to_str().unwrap(),
            "--monochrome",
            svg.to_str().unwrap(),
            "--fit",
            "66",
        ],
        &temp.path().join("f"),
    );
    assert!(output.status.success(), "{output:?}");

    // A raster monochrome follows the foreground rules and lands beside it.
    let mono = temp.path().join("g");
    let output = run(
        &[
            "--foreground-image",
            inside.to_str().unwrap(),
            "--monochrome-image",
            outside.to_str().unwrap(),
        ],
        &mono,
    );
    assert!(output.status.success(), "{output:?}");
    assert_eq!(codes(&output), ["SVGVD019"]);
    assert!(
        mono.join("mipmap-nodpi/ic_launcher_monochrome.webp")
            .is_file()
    );
    let icon = fs::read_to_string(mono.join("mipmap-anydpi-v26/ic_launcher.xml")).unwrap();
    assert!(
        icon.contains("<monochrome android:drawable=\"@mipmap/ic_launcher_monochrome\"/>"),
        "{icon}"
    );

    // A bitmap cannot be composed into the legacy vector icon.
    let output = run(
        &["--foreground-image", inside.to_str().unwrap(), "--legacy"],
        &temp.path().join("h"),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--legacy composes the foreground into a vector icon"),
    );
}

#[test]
fn cli_adaptive_never_removes_resources_it_did_not_write() {
    let temp = tempfile::tempdir().unwrap();
    let fg = temp.path().join("fg.svg");
    let bg = temp.path().join("bg.svg");
    fs::write(
        &fg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
    )
    .unwrap();
    fs::write(
        &bg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108"><rect width="108" height="108" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    let res = temp.path().join("res");
    let values = res.join("values");
    fs::create_dir_all(&values).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(&fg)
            .args(args)
            .args(["--fit", "56", "-o"])
            .arg(&res)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    };

    // A values file is a container for any resources, so one that merely
    // shares a layer's name is the project's, not vdt's. A foreground or
    // monochrome layer is never written as a values file at all.
    let foreground_values = values.join("ic_launcher_foreground.xml");
    let monochrome_values = values.join("ic_launcher_monochrome.xml");
    let unrelated = "<resources><string name=\"app_name\">Mine</string></resources>\n";
    fs::write(&foreground_values, unrelated).unwrap();
    fs::write(&monochrome_values, unrelated).unwrap();
    // A background values file that holds more than the icon's color is also
    // the project's, even though vdt could have written one of that name.
    let background_values = values.join("ic_launcher_background.xml");
    let mixed = "<resources>\n    <color name=\"ic_launcher_background\">#FFFFFF</color>\n    \
                 <string name=\"tagline\">Kept</string>\n</resources>\n";
    fs::write(&background_values, mixed).unwrap();

    run(&["--background", bg.to_str().unwrap()]);
    assert_eq!(fs::read_to_string(&foreground_values).unwrap(), unrelated);
    assert_eq!(fs::read_to_string(&monochrome_values).unwrap(), unrelated);
    assert_eq!(fs::read_to_string(&background_values).unwrap(), mixed);

    // Even the file Android Studio's Image Asset wizard writes, which holds
    // nothing but the background color, is left in place: vdt cannot tell
    // whether anything else in the project uses that color.
    let studio = "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n    \
                  <color name=\"ic_launcher_background\">#3DDC84</color>\n</resources>\n";
    fs::write(&background_values, studio).unwrap();
    run(&["--background", bg.to_str().unwrap()]);
    assert_eq!(fs::read_to_string(&background_values).unwrap(), studio);
}

#[test]
fn cli_adaptive_clears_mipmap_versions_that_would_shadow_a_raster_layer() {
    let temp = tempfile::tempdir().unwrap();
    let fg = temp.path().join("fg.svg");
    let bg = temp.path().join("bg.svg");
    fs::write(
        &fg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
    )
    .unwrap();
    fs::write(
        &bg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108"><rect width="108" height="108" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    let image = temp.path().join("bg.png");
    fs::write(
        &image,
        vdtoolkit::convert(fs::read(&bg).unwrap().as_slice())
            .unwrap()
            .to_png(432, 432)
            .unwrap(),
    )
    .unwrap();
    let res = temp.path().join("res");
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(&fg)
            .args(args)
            .args(["--fit", "56", "-o"])
            .arg(&res)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stderr).unwrap()
    };
    // What Android Studio's Image Asset wizard leaves for an image background:
    // one version per density, all chosen over mipmap-nodpi on a device of
    // that density, plus a qualified one that wins in dark mode.
    let studio = [
        "mipmap-mdpi/ic_launcher_background.png",
        "mipmap-hdpi/ic_launcher_background.webp",
        "mipmap-xxhdpi/ic_launcher_background.png",
        "mipmap-xxxhdpi/ic_launcher_background.webp",
        "mipmap-night-xhdpi/ic_launcher_background.png",
    ];
    // Things in those folders that are not versions of the layer stay.
    let bystanders = [
        "mipmap-xxhdpi/ic_launcher.png",
        "mipmap-xxhdpi/ic_launcher_foreground_old.png",
        "drawable-xxhdpi/ic_launcher_background.png",
    ];
    let plant = |files: &[&str]| {
        for relative in files {
            let path = res.join(relative);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"stale").unwrap();
        }
    };
    plant(&studio);
    plant(&bystanders);

    // An image background is @mipmap/ic_launcher_background, so every other
    // version of that resource would show instead of it, and is removed.
    let stderr = run(&["--background-image", image.to_str().unwrap()]);
    assert!(
        res.join("mipmap-nodpi/ic_launcher_background.png")
            .is_file()
    );
    for relative in studio {
        assert!(
            !res.join(relative).exists(),
            "{relative} survived\n{stderr}"
        );
        assert!(stderr.contains(relative), "{relative}\n{stderr}");
    }
    for relative in bystanders {
        assert!(res.join(relative).is_file(), "{relative} was removed");
    }

    // A vector background is @drawable/ic_launcher_background, which a
    // mipmap never shadows, so versions left in mipmap folders are only named.
    plant(&studio);
    let stderr = run(&["--background", bg.to_str().unwrap()]);
    for relative in studio {
        assert!(res.join(relative).is_file(), "{relative} was removed");
        assert!(
            stderr.contains(&format!("{relative}; remove it")),
            "{relative}\n{stderr}"
        );
    }
}

#[test]
fn cli_legacy_replaces_the_icon_android_studio_left_behind() {
    let temp = tempfile::tempdir().unwrap();
    // Flat art needs API 21 and gets a vector in mipmap/; a gradient needs
    // API 24 and gets PNGs in every density folder.
    let flat = temp.path().join("flat.svg");
    let gradient = temp.path().join("gradient.svg");
    fs::write(
        &flat,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M8 8H16V16H8Z" fill="#102030"/></svg>"##,
    )
    .unwrap();
    fs::write(
        &gradient,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><defs><linearGradient id="g"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></linearGradient></defs><path d="M8 8H16V16H8Z" fill="url(#g)"/></svg>"##,
    )
    .unwrap();
    let densities = ["mdpi", "hdpi", "xhdpi", "xxhdpi", "xxxhdpi"];
    // What Android Studio writes for a new project: the legacy icon as WebP in
    // every density folder, and, with minSdk 26 or higher, the adaptive icon
    // in an unversioned anydpi folder. Plus a file that is not the icon.
    let studio = |res: &std::path::Path| {
        for density in densities {
            let dir = res.join(format!("mipmap-{density}"));
            fs::create_dir_all(&dir).unwrap();
            fs::write(dir.join("ic_launcher.webp"), b"studio").unwrap();
            fs::write(dir.join("ic_launcher_round.webp"), b"studio").unwrap();
            fs::write(dir.join("ic_launcher_old.webp"), b"keep").unwrap();
        }
        fs::create_dir_all(res.join("mipmap-anydpi")).unwrap();
        fs::write(res.join("mipmap-anydpi/ic_launcher.xml"), b"studio").unwrap();
    };
    let run = |art: &std::path::Path, res: &std::path::Path, legacy: bool| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_vdt"));
        command
            .args(["adaptive", "--foreground"])
            .arg(art)
            .args(["--background-color", "#FFFFFF", "--fit", "56", "-o"])
            .arg(res);
        if legacy {
            command.arg("--legacy");
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stderr).unwrap()
    };
    let studio_left = |res: &std::path::Path| -> Vec<String> {
        let mut left = Vec::new();
        for density in densities {
            for name in ["ic_launcher.webp", "ic_launcher_round.webp"] {
                let relative = format!("mipmap-{density}/{name}");
                if res.join(&relative).exists() {
                    left.push(relative);
                }
            }
        }
        if res.join("mipmap-anydpi/ic_launcher.xml").exists() {
            left.push("mipmap-anydpi/ic_launcher.xml".to_owned());
        }
        left
    };

    for (art, writes) in [
        (
            &flat,
            vec!["mipmap/ic_launcher.xml", "mipmap/ic_launcher_round.xml"],
        ),
        (
            &gradient,
            vec![
                "mipmap-anydpi-v24/ic_launcher.xml",
                "mipmap-mdpi/ic_launcher.png",
                "mipmap-xxxhdpi/ic_launcher_round.png",
            ],
        ),
    ] {
        let res = temp.path().join(art.file_stem().unwrap());
        studio(&res);
        let stderr = run(art, &res, true);
        // Left in place, Studio's copies are the same resource as vdt's:
        // beside the gradient's PNGs they fail the build, and beside the flat
        // art's vector they are chosen over it on older devices.
        assert!(
            studio_left(&res).is_empty(),
            "{:?}\n{stderr}",
            studio_left(&res)
        );
        for relative in writes {
            assert!(res.join(relative).is_file(), "{relative}");
        }
        assert!(res.join("mipmap-anydpi-v26/ic_launcher.xml").is_file());
        for density in densities {
            let other = format!("mipmap-{density}/ic_launcher_old.webp");
            assert!(res.join(&other).is_file(), "{other} was removed");
        }
    }

    // Without --legacy, vdt writes no legacy icon, so Studio's is still the
    // project's icon for older devices and is left alone.
    let res = temp.path().join("no-legacy");
    studio(&res);
    let stderr = run(&flat, &res, false);
    assert_eq!(studio_left(&res).len(), 11, "{stderr}");
    assert!(!stderr.contains("removed stale resource"), "{stderr}");
}

#[test]
fn cli_adaptive_prints_the_safe_zone_note() {
    let temp = tempfile::tempdir().unwrap();
    let round = temp.path().join("round.svg");
    fs::write(
        &round,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><circle cx="12" cy="12" r="12" fill="#fff"/></svg>"##,
    )
    .unwrap();
    // At --fit 72 the round logo reaches about 36dp: a circular mask still
    // shows it, but it is past the 33dp Android asks for, which is a note.
    // The generator is the command people run, so it has to print it.
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&round)
        .args(["--background-color", "#3DDC84", "--fit", "72", "-o"])
        .arg(temp.path().join("res"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let note = stderr
        .lines()
        .find(|line| line.contains("SVGVD019"))
        .unwrap_or_else(|| panic!("no safe-zone finding printed:\n{stderr}"));
    assert!(note.starts_with("note:"), "{note}");
    assert!(note.contains("past the 33dp Android asks"), "{note}");
}

#[test]
fn cli_adaptive_rejects_raster_layers_with_corrupt_image_data() {
    let temp = tempfile::tempdir().unwrap();
    let fg = temp.path().join("fg.svg");
    fs::write(
        &fg,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
    )
    .unwrap();
    let res = temp.path().join("res");
    let run = |image: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(&fg)
            .args(["--fit", "56", "--background-image"])
            .arg(image)
            .arg("-o")
            .arg(&res)
            .output()
            .unwrap()
    };

    // A valid opaque WebP. No alpha channel, which is the path that used to
    // skip decoding the image data entirely.
    let mut opaque = Vec::new();
    image_webp::WebPEncoder::new(std::io::Cursor::new(&mut opaque))
        .encode(
            &[0x3D, 0xDC, 0x84].repeat(432 * 432),
            432,
            432,
            image_webp::ColorType::Rgb8,
        )
        .unwrap();
    assert!(
        !image_webp::WebPDecoder::new(std::io::Cursor::new(&opaque))
            .unwrap()
            .has_alpha(),
        "the fixture must exercise the opaque path"
    );
    let good = temp.path().join("good.webp");
    fs::write(&good, &opaque).unwrap();
    assert!(run(&good).status.success());
    let placed = res.join("mipmap-nodpi/ic_launcher_background.webp");
    assert!(placed.is_file());

    // Headers intact, image data cut short: each format's header still reads.
    let jpeg = include_bytes!("fixtures/background_square.jpg");
    let scan = jpeg
        .windows(2)
        .position(|pair| pair == [0xFF, 0xDA])
        .unwrap();
    let broken_jpeg = temp.path().join("broken.jpg");
    fs::write(&broken_jpeg, &jpeg[..scan + 40]).unwrap();
    let broken_webp = temp.path().join("broken.webp");
    fs::write(&broken_webp, &opaque[..opaque.len() / 2]).unwrap();

    for broken in [&broken_jpeg, &broken_webp] {
        let output = run(broken);
        assert!(!output.status.success(), "{broken:?} was accepted");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("cannot decode"),
            "{broken:?}"
        );
        // Refusing before anything is written also keeps the working layer:
        // it is not cleared away as stale by a run that did not replace it.
        assert_eq!(fs::read(&placed).unwrap(), opaque, "{broken:?}");
        assert!(!res.join("mipmap-nodpi/ic_launcher_background.jpg").exists());
    }
}

#[test]
fn cli_adaptive_places_a_bitmap_background_without_resampling_it() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
    )
    .unwrap();

    // Build the PNGs with vdt's own renderer so the fixtures are exact.
    let png = |svg: &[u8], w: u32, h: u32| vdtoolkit::convert(svg).unwrap().to_png(w, h).unwrap();
    let solid = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect width="100" height="100" fill="#3DDC84"/></svg>"##;
    let holed = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><circle cx="50" cy="50" r="50" fill="#3DDC84"/></svg>"##;

    let write = |name: &str, bytes: &[u8]| {
        let path = temp.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    };
    // Lossless WebP, so the pixels round-trip and the alpha check is exact.
    let webp = |rgba: &[u8], w: u32, h: u32| {
        let mut out = Vec::new();
        image_webp::WebPEncoder::new(std::io::Cursor::new(&mut out))
            .encode(rgba, w, h, image_webp::ColorType::Rgba8)
            .unwrap();
        out
    };
    let fill = |w: u32, h: u32, alpha: u8| [0x3D, 0xDC, 0x84, alpha].repeat((w * h) as usize);

    let sharp = write("sharp.png", &png(solid, 432, 432));
    let soft = write("soft.png", &png(solid, 108, 108));
    let wide = write("wide.png", &png(solid, 600, 400));
    let holes = write("holes.png", &png(holed, 432, 432));

    let run = |image: &std::path::Path, extra: &[&str], out: &std::path::Path| {
        Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(&foreground)
            .arg("--background-image")
            .arg(image)
            .args(["--fit", "66"])
            .args(extra)
            .arg("-o")
            .arg(out)
            .output()
            .unwrap()
    };
    let codes = |output: &std::process::Output| -> Vec<String> {
        String::from_utf8(output.stderr.clone())
            .unwrap()
            .lines()
            .filter_map(|line| {
                line.split_whitespace()
                    .find(|word| word.starts_with("SVGVD"))
            })
            .map(str::to_owned)
            .collect()
    };

    // A square, opaque, 432px image is exactly what the layer wants.
    let res = temp.path().join("res");
    let output = run(&sharp, &[], &res);
    assert!(output.status.success(), "{output:?}");
    assert!(codes(&output).is_empty(), "{:?}", codes(&output));
    let icon = fs::read_to_string(res.join("mipmap-anydpi-v26/ic_launcher.xml")).unwrap();
    assert!(
        icon.contains("<background android:drawable=\"@mipmap/ic_launcher_background\"/>"),
        "{icon}"
    );
    // Copied, not resampled: the bytes written are the bytes given.
    assert_eq!(
        fs::read(res.join("mipmap-nodpi/ic_launcher_background.png")).unwrap(),
        fs::read(&sharp).unwrap()
    );
    assert!(!res.join("values").exists());
    assert!(
        !res.join("drawable-anydpi/ic_launcher_background.xml")
            .exists()
    );

    // What vdt can see about an image it never converts.
    assert_eq!(
        codes(&run(&soft, &[], &temp.path().join("a"))),
        ["SVGVD025"]
    );
    assert_eq!(
        codes(&run(&wide, &[], &temp.path().join("b"))),
        ["SVGVD024", "SVGVD025"]
    );
    assert_eq!(
        codes(&run(&holes, &[], &temp.path().join("c"))),
        ["SVGVD020"]
    );

    // WebP reaches the same findings, and lands under its own extension.
    let webp_sharp = write("sharp.webp", &webp(&fill(432, 432, 0xFF), 432, 432));
    let webp_clear = write("clear.webp", &webp(&fill(432, 432, 0x80), 432, 432));
    let webp_wide = write("wide.webp", &webp(&fill(600, 400, 0xFF), 600, 400));
    let res_webp = temp.path().join("webp");
    let output = run(&webp_sharp, &[], &res_webp);
    assert!(output.status.success(), "{output:?}");
    assert!(codes(&output).is_empty(), "{:?}", codes(&output));
    assert_eq!(
        fs::read(res_webp.join("mipmap-nodpi/ic_launcher_background.webp")).unwrap(),
        fs::read(&webp_sharp).unwrap()
    );
    assert!(
        !res_webp
            .join("mipmap-nodpi/ic_launcher_background.png")
            .exists()
    );
    assert_eq!(
        codes(&run(&webp_clear, &[], &temp.path().join("f"))),
        ["SVGVD020"]
    );
    assert_eq!(
        codes(&run(&webp_wide, &[], &temp.path().join("g"))),
        ["SVGVD024", "SVGVD025"]
    );

    // JPEG carries no alpha, so it can only ever be an opaque background; only
    // its size is in question. `.jpeg` is normalized to the `.jpg` extension.
    let jpeg_sharp = write(
        "sharp.jpeg",
        include_bytes!("fixtures/background_square.jpg"),
    );
    let jpeg_wide = write("wide.jpg", include_bytes!("fixtures/background_wide.jpg"));
    let res_jpeg = temp.path().join("jpeg");
    let output = run(&jpeg_sharp, &[], &res_jpeg);
    assert!(output.status.success(), "{output:?}");
    assert!(codes(&output).is_empty(), "{:?}", codes(&output));
    assert_eq!(
        fs::read(res_jpeg.join("mipmap-nodpi/ic_launcher_background.jpg")).unwrap(),
        fs::read(&jpeg_sharp).unwrap()
    );
    assert_eq!(
        codes(&run(&jpeg_wide, &[], &temp.path().join("h"))),
        ["SVGVD024", "SVGVD025"]
    );

    // The format comes from the file's header, not its name, so a WebP saved
    // as .png is still written as a WebP resource.
    let mislabeled = write("mislabeled.png", &fs::read(&webp_sharp).unwrap());
    let res_sniff = temp.path().join("sniff");
    assert!(run(&mislabeled, &[], &res_sniff).status.success());
    assert!(
        res_sniff
            .join("mipmap-nodpi/ic_launcher_background.webp")
            .is_file()
    );

    // Switching raster format clears the other extension.
    assert!(run(&sharp, &[], &res_sniff).status.success());
    assert!(
        res_sniff
            .join("mipmap-nodpi/ic_launcher_background.png")
            .is_file()
    );
    assert!(
        !res_sniff
            .join("mipmap-nodpi/ic_launcher_background.webp")
            .exists()
    );

    // A bitmap cannot be composed into the legacy vector icon.
    let output = run(&sharp, &["--legacy"], &temp.path().join("d"));
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("--legacy composes the background into a vector icon"),
    );

    // A file in none of the formats is refused rather than copied unseen.
    let fake = write("fake.png", b"GIF89a not an icon");
    let output = run(&fake, &[], &temp.path().join("e"));
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("must be a PNG, a WebP, or a JPEG"),
    );

    // Switching to a color leaves the image in place and names it: a @mipmap
    // and a @color never collide, so keeping it breaks nothing.
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&foreground)
        .args(["--background-color", "#3DDC84", "--fit", "66", "-o"])
        .arg(&res)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        res.join("mipmap-nodpi/ic_launcher_background.png")
            .is_file()
    );
    assert!(res.join("values/ic_launcher_background.xml").is_file());
    assert!(!stderr.contains("removed stale resource"), "{stderr}");
    assert!(
        stderr.contains("mipmap-nodpi/ic_launcher_background.png; remove it"),
        "{stderr}"
    );
}

#[test]
fn cli_adaptive_covers_the_layer_with_a_non_square_background() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    let background = temp.path().join("wide.svg");
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
    )
    .unwrap();
    fs::write(
        &background,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="90"><rect width="160" height="90" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    let res = temp.path().join("res");

    // Cover is the default: the wide background reaches every edge, so there
    // is no gap to warn about, only a note naming what the layer cropped.
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&foreground)
        .arg("--background")
        .arg(&background)
        .args(["--fit", "66", "-o"])
        .arg(&res)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(stderr.contains("SVGVD023"), "{stderr}");
    assert!(
        stderr.contains(
            "scaled the background to cover the 108dp layer, which cropped 44% of the artwork"
        ),
        "{stderr}"
    );
    assert!(!stderr.contains("SVGVD020"), "{stderr}");

    // The layer is 108dp of paint with nothing left transparent.
    let xml = fs::read_to_string(res.join("drawable-anydpi/ic_launcher_background.xml")).unwrap();
    assert!(xml.contains("android:viewportWidth=\"108\""), "{xml}");
    let mut layer = vdtoolkit::convert(
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="90"><rect width="160" height="90" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    layer
        .fit_adaptive_layer(vdtoolkit::Fit::cover(vdtoolkit::ADAPTIVE_ICON_SIZE))
        .unwrap();
    assert!(layer.fills_adaptive_layer());

    // A square background is unchanged by cover, so it gets no note at all.
    let square = temp.path().join("square.svg");
    fs::write(
        &square,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108"><rect width="108" height="108" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&foreground)
        .arg("--background")
        .arg(&square)
        .args(["--fit", "66", "-o"])
        .arg(&res)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(String::from_utf8(output.stderr).unwrap(), "");
}

#[test]
fn cli_adaptive_warns_when_background_does_not_fill_the_layer() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    let background = temp.path().join("wide.svg");
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
    )
    .unwrap();
    fs::write(
        &background,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="90"><rect width="160" height="90" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    let res = temp.path().join("res");

    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&foreground)
        .arg("--background")
        .arg(&background)
        .args(["--fit", "66", "--background-fit", "contain", "-o"])
        .arg(&res)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
    assert!(
        stderr.contains(
            "background content spans 0..108 × 23.6..84.4dp and does not fill the 108dp layer"
        ),
        "{stderr}"
    );
    assert!(
        res.join("drawable-anydpi/ic_launcher_background.xml")
            .is_file()
    );
}

#[test]
fn composes_a_masked_legacy_icon() {
    let logo = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2H22V22H2Z" fill="#102030"/></svg>"##;
    let mut foreground = vdtoolkit::convert(logo).unwrap();
    foreground
        .fit_adaptive_layer(vdtoolkit::Fit::contain(66.0))
        .unwrap();
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
    // The 72dp visible circle lands on the 44dp keyline centered in 48dp.
    assert!(xml.contains("android:width=\"48dp\""));
    assert!(xml.contains("android:viewportWidth=\"48\""));
    assert!(xml.contains("<clip-path\n        android:pathData=\"M24,2 C"));
    // Layers without clips of their own are not wrapped in groups.
    assert!(!xml.contains("<group>"));
    assert_eq!(legacy.analysis.minimum_api, Some(21));
    assert_eq!(legacy.analysis.compatibility, Compatibility::Exact);
    let metrics = &legacy.analysis.metrics;
    assert_eq!((metrics.paths, metrics.clip_paths), (2, 1));
    assert_eq!(metrics.estimated_xml_bytes, xml.len());
    // The foreground spans layer 26.5..81.5, which maps to 7.2..40.8.
    let bounds = metrics.content_bounds.unwrap();
    assert_eq!((bounds.left, bounds.right), (0.0, 48.0));

    let rgba = legacy.render_rgba(48, 48).unwrap();
    let pixel = |x: usize, y: usize| {
        let index = (y * 48 + x) * 4;
        [
            rgba[index],
            rgba[index + 1],
            rgba[index + 2],
            rgba[index + 3],
        ]
    };
    assert_eq!(pixel(24, 24), [0x10, 0x20, 0x30, 255]);
    // Above the foreground, inside the mask: the half-transparent background.
    let [red, green, blue, alpha] = pixel(24, 4);
    assert!(alpha.abs_diff(128) <= 1, "{:?}", pixel(24, 4));
    assert!(red.abs_diff(0x3D) <= 2 && green.abs_diff(0xDC) <= 2 && blue.abs_diff(0x84) <= 2);
    // Outside the mask.
    assert_eq!(pixel(24, 0)[3], 0);
    assert_eq!(pixel(1, 1)[3], 0);

    // A gradient background lifts the legacy icon to API 24.
    let gradient = br##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108">
        <defs><linearGradient id="g" x1="0" y1="0" x2="108" y2="0" gradientUnits="userSpaceOnUse">
            <stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#FFF"/>
        </linearGradient></defs>
        <rect width="108" height="108" fill="url(#g)"/>
    </svg>"##;
    let mut background = vdtoolkit::convert(gradient).unwrap();
    background
        .fit_adaptive_layer(vdtoolkit::Fit::contain(108.0))
        .unwrap();
    let legacy = vdtoolkit::Asset::legacy_launcher_icon(&background, &foreground);
    assert_eq!(legacy.analysis.minimum_api, Some(24));
}

/// Mirrored blue to orange gradient every 54 units, full bleed on the layer.
const MIRRORED_GRADIENT_BACKGROUND: &[u8] =
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108">
    <defs><linearGradient id="g" x1="0" y1="0" x2="54" y2="0"
            gradientUnits="userSpaceOnUse" spreadMethod="reflect">
        <stop offset="0" stop-color="#1267D6"/><stop offset="1" stop-color="#E37A19"/>
    </linearGradient></defs>
    <rect width="108" height="108" fill="url(#g)"/>
</svg>"##;

/// Expected color of the mirrored gradient at layer x.
fn mirrored_gradient_at(x: f32) -> [f32; 3] {
    let t = if x <= 54.0 { x / 54.0 } else { 2.0 - x / 54.0 }.clamp(0.0, 1.0);
    let mix = |a: f32, b: f32| a + (b - a) * t;
    [
        mix(0x12 as f32, 0xE3 as f32),
        mix(0x67 as f32, 0x7A as f32),
        mix(0xD6 as f32, 0x19 as f32),
    ]
}

#[test]
fn renders_legacy_gradients_like_the_vector() {
    let logo = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2H22V22H2Z" fill="#102030"/></svg>"##;
    let mut foreground = vdtoolkit::convert(logo).unwrap();
    foreground
        .fit_adaptive_layer(vdtoolkit::Fit::contain(66.0))
        .unwrap();
    let mut background = vdtoolkit::convert(MIRRORED_GRADIENT_BACKGROUND).unwrap();
    background
        .fit_adaptive_layer(vdtoolkit::Fit::contain(108.0))
        .unwrap();
    let legacy = vdtoolkit::Asset::legacy_launcher_icon(&background, &foreground);
    assert_eq!(legacy.analysis.minimum_api, Some(24));

    let size = 192;
    let rgba = legacy.render_rgba(size, size).unwrap();
    let (scale, offset) = (44.0 / 72.0, -9.0);
    let sample = |layer_x: f32, layer_y: f32| {
        let per_unit = size as f32 / 48.0;
        let x = ((layer_x * scale + offset) * per_unit) as usize;
        let y = ((layer_y * scale + offset) * per_unit) as usize;
        let index = (y * size as usize + x) * 4;
        // Layer coordinate of the pixel center actually sampled.
        let center_x = ((x as f32 + 0.5) / per_unit - offset) / scale;
        (&rgba[index..index + 4], center_x)
    };
    // Above the foreground and inside the mask, on both sides of the mirror
    // axis and at the axis itself.
    for layer_x in [40.0, 54.0, 68.0] {
        let (pixel, center_x) = sample(layer_x, 26.0);
        let expected = mirrored_gradient_at(center_x);
        assert_eq!(pixel[3], 255, "x {layer_x}");
        for channel in 0..3 {
            assert!(
                (pixel[channel] as f32 - expected[channel]).abs() <= 3.0,
                "x {layer_x}: {pixel:?} vs {expected:?}"
            );
        }
    }
    // Reflect, not clamp: equal distances from the axis match.
    let (left, _) = sample(40.0, 26.0);
    let (right, _) = sample(68.0 + 0.5 * 72.0 / 44.0 / 4.0, 26.0);
    assert!(
        left.iter().zip(right).all(|(a, b)| a.abs_diff(*b) <= 2),
        "{left:?} {right:?}"
    );

    let png = legacy.to_png(size, size).unwrap();
    assert!(png.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(&png[16..24], &[0, 0, 0, 192, 0, 0, 0, 192]);
    assert_eq!(png, legacy.to_png(size, size).unwrap());
    assert!(matches!(legacy.to_png(0, 48), Err(Error::InvalidInput(_))));
}

#[test]
fn cli_legacy_removes_the_other_layout_when_art_changes() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    let gradient = temp.path().join("gradient.svg");
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2H22V22H2Z" fill="#102030"/></svg>"##,
    )
    .unwrap();
    fs::write(&gradient, MIRRORED_GRADIENT_BACKGROUND).unwrap();
    let res = temp.path().join("res");
    let run = |background: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(&foreground)
            .args(background)
            .args(["--fit", "66", "--legacy", "-o"])
            .arg(&res)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stderr).unwrap()
    };
    let split = [
        "mipmap-anydpi-v24/ic_launcher.xml",
        "mipmap-anydpi-v24/ic_launcher_round.xml",
        "mipmap-mdpi/ic_launcher.png",
        "mipmap-xxxhdpi/ic_launcher_round.png",
    ];
    let plain = ["mipmap/ic_launcher.xml", "mipmap/ic_launcher_round.xml"];

    // API 24 art, then API 21 art with the same name: the qualified files
    // that Android 24 and 25 would still prefer are removed.
    let stderr = run(&["--background", gradient.to_str().unwrap()]);
    assert!(!stderr.contains("removed"), "{stderr}");
    let stderr = run(&["--background-color", "#3DDC84"]);
    // The twelve legacy files would outrank the new icon, so they go. The
    // drawable the SVG background wrote is a @drawable and the new background
    // a @color, so it collides with nothing: it is named, not deleted.
    assert_eq!(
        stderr.matches("removed stale resource").count(),
        12,
        "{stderr}"
    );
    assert!(
        stderr.contains("drawable-anydpi/ic_launcher_background.xml; remove it"),
        "{stderr}"
    );
    assert!(
        res.join("drawable-anydpi/ic_launcher_background.xml")
            .is_file()
    );
    for relative in split {
        assert!(!res.join(relative).exists(), "{relative} should be gone");
    }
    for relative in plain {
        assert!(res.join(relative).is_file(), "{relative}");
    }

    // And back again.
    let stderr = run(&["--background", gradient.to_str().unwrap()]);
    // The two plain legacy files go; the color resource is named, not deleted.
    assert_eq!(
        stderr.matches("removed stale resource").count(),
        2,
        "{stderr}"
    );
    assert!(
        stderr.contains("values/ic_launcher_background.xml; remove it"),
        "{stderr}"
    );
    assert!(res.join("values/ic_launcher_background.xml").is_file());
    for relative in plain {
        assert!(!res.join(relative).exists(), "{relative} should be gone");
    }
    for relative in split {
        assert!(res.join(relative).is_file(), "{relative}");
    }
}

#[test]
fn cli_legacy_splits_vector_and_pngs_when_art_needs_api_24() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    let background = temp.path().join("gradient.svg");
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M2 2H22V22H2Z" fill="#102030"/></svg>"##,
    )
    .unwrap();
    fs::write(&background, MIRRORED_GRADIENT_BACKGROUND).unwrap();
    let run = |res: &std::path::Path| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(&foreground)
            .arg("--background")
            .arg(&background)
            .args(["--fit", "56", "--legacy", "-o"])
            .arg(res)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    let (first, second) = (temp.path().join("a"), temp.path().join("b"));
    let listed = run(&first);
    run(&second);

    assert!(!first.join("mipmap").exists());
    let vector = fs::read_to_string(first.join("mipmap-anydpi-v24/ic_launcher.xml")).unwrap();
    assert!(vector.contains("<gradient"));
    assert_eq!(
        fs::read_to_string(first.join("mipmap-anydpi-v24/ic_launcher_round.xml")).unwrap(),
        vector
    );
    for (density, pixels) in vdtoolkit::LEGACY_ICON_DENSITIES {
        for name in ["ic_launcher", "ic_launcher_round"] {
            let relative = format!("mipmap-{density}/{name}.png");
            assert!(listed.contains(&relative), "{listed}");
            let png = fs::read(first.join(&relative)).unwrap();
            let size = pixels.to_be_bytes();
            assert_eq!(&png[16..20], &size, "{relative}");
            assert_eq!(&png[20..24], &size, "{relative}");
            assert_eq!(png, fs::read(second.join(&relative)).unwrap(), "{relative}");
        }
    }
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
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z" fill="#3DDC84"/></svg>"##,
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
        fs::read_to_string(res.join("drawable-anydpi/ic_launcher_foreground.xml")).unwrap();
    assert!(foreground_xml.contains("android:width=\"108dp\""));
    assert!(foreground_xml.contains("android:pathData=\"M54,40.25 L40.25,67.75 L67.75,67.75 Z\""));
    let background_xml =
        fs::read_to_string(res.join("drawable-anydpi/ic_launcher_background.xml")).unwrap();
    assert!(background_xml.contains("android:pathData=\"M0,0 L108,0 L108,108 L0,108 Z\""));
    assert_eq!(
        fs::read_to_string(res.join("drawable-anydpi/ic_launcher_monochrome.xml")).unwrap(),
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
        vdtoolkit::compact_xml(&vdtoolkit::color_resource_xml(
            "ic_app_background",
            "#3DDC84"
        ))
    );
    let icon = fs::read_to_string(res.join("mipmap-anydpi-v26/ic_app.xml")).unwrap();
    assert!(icon.contains("<background android:drawable=\"@color/ic_app_background\"/>"));
    assert!(icon.contains("<foreground android:drawable=\"@drawable/ic_app_foreground\"/>"));
    assert!(!icon.contains("monochrome"));
    assert!(res.join("drawable-anydpi/ic_app_foreground.xml").is_file());
    assert!(!res.join("drawable-anydpi/ic_app_background.xml").exists());
    assert!(!res.join("mipmap").exists());

    // The default fit leaves a plain logo outside the safe zone; --legacy
    // adds the masked fallback icon.
    // Drawn edge to edge, so the run also shows the CLI printing the finding.
    let edge = temp.path().join("edge.svg");
    fs::write(
        &edge,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 2L2 22h20z" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["adaptive", "--foreground"])
        .arg(&edge)
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
    assert!(
        stderr.contains("past the 36dp a circular launcher mask shows"),
        "{stderr}"
    );
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
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z"/></svg>"##,
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

#[test]
fn notification_icons_flatten_paint_to_white_and_keep_opacity() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48">
        <path d="M8 8H40V40H8Z" fill="#FF0000" fill-opacity="0.5"/>
        <path d="M16 16H32V32H16Z" fill="none" stroke="#00FF00" stroke-width="4"/>
    </svg>"##;

    let mut asset = vdtoolkit::convert(source).unwrap();
    let flattening = asset
        .to_notification_icon(vdtoolkit::Fit::contain(vdtoolkit::NOTIFICATION_ICON_SIZE))
        .unwrap();
    let xml = asset.to_xml();

    assert_eq!(flattening.colors, 2);
    assert_eq!(flattening.gradients, 0);
    assert!(!flattening.is_empty());
    assert!(xml.contains("android:width=\"24dp\""));
    assert!(xml.contains("android:height=\"24dp\""));
    assert!(xml.contains("android:viewportWidth=\"24\""));
    assert!(!xml.contains("#FF0000"));
    assert!(!xml.contains("#00FF00"));
    assert_eq!(xml.matches("\"#FFFFFF\"").count(), 2);
    assert!(xml.contains("android:fillAlpha=\"0.5\""));
    // 48 units fit into 24dp: scale 0.5, no offset. Stroke width scales too.
    assert!(xml.contains("android:pathData=\"M4,4 L20,4 L20,20 L4,20 Z\""));
    assert!(xml.contains("android:strokeWidth=\"2\""));
    assert_eq!(asset.analysis.metrics.estimated_xml_bytes, xml.len());
    assert_eq!(asset.analysis.minimum_api, Some(21));
}

#[test]
fn notification_icons_fit_smaller_squares_and_reject_other_sizes() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#000000"/>
    </svg>"##;

    let mut asset = vdtoolkit::convert(source).unwrap();
    asset
        .to_notification_icon(vdtoolkit::Fit::contain(
            vdtoolkit::NOTIFICATION_ICON_LIVE_AREA,
        ))
        .unwrap();

    // 24 units into a centered 20dp square: scale 0.833, offset 2 on each side.
    assert!(
        asset
            .to_xml()
            .contains("android:pathData=\"M2,2 L22,2 L22,22 L2,22 Z\"")
    );
    let bounds = asset.analysis.metrics.content_bounds.unwrap();
    assert_eq!((bounds.left, bounds.right), (2.0, 22.0));

    assert!(matches!(
        asset.to_notification_icon(vdtoolkit::Fit::contain(0.0)),
        Err(Error::InvalidInput(_))
    ));
    assert!(matches!(
        asset.to_notification_icon(vdtoolkit::Fit::contain(25.0)),
        Err(Error::InvalidInput(_))
    ));
}

#[test]
fn notification_icons_flatten_gradients_and_keep_varying_opacity() {
    let uniform = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" x1="0" y1="0" x2="24" y2="0" gradientUnits="userSpaceOnUse">
            <stop offset="0" stop-color="#123456"/><stop offset="1" stop-color="#ABCDEF"/>
        </linearGradient></defs>
        <path d="M4 4H20V20H4Z" fill="url(#g)" fill-opacity="0.5"/>
    </svg>"##;

    let mut asset = vdtoolkit::convert(uniform).unwrap();
    assert_eq!(asset.analysis.minimum_api, Some(24));
    let flattening = asset
        .to_notification_icon(vdtoolkit::Fit::contain(vdtoolkit::NOTIFICATION_ICON_SIZE))
        .unwrap();
    let xml = asset.to_xml();

    // Every stop is opaque, so the gradient is only color: it becomes solid
    // white, and the drawable no longer needs API 24.
    assert_eq!((flattening.colors, flattening.gradients), (2, 1));
    assert!(!xml.contains("aapt:attr"));
    assert!(xml.contains("android:fillColor=\"#FFFFFF\""));
    assert!(xml.contains("android:fillAlpha=\"0.5\""));
    assert_eq!(asset.analysis.minimum_api, Some(21));
    assert!(
        !asset
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "SVGVD004"),
        "{:?}",
        asset.analysis.diagnostics
    );

    let fading = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><linearGradient id="g" x1="0" y1="0" x2="24" y2="0" gradientUnits="userSpaceOnUse">
            <stop offset="0" stop-color="#123456" stop-opacity="1"/>
            <stop offset="1" stop-color="#123456" stop-opacity="0"/>
        </linearGradient></defs>
        <path d="M4 4H20V20H4Z" fill="url(#g)"/>
    </svg>"##;

    let mut asset = vdtoolkit::convert(fading).unwrap();
    asset
        .to_notification_icon(vdtoolkit::Fit::contain(vdtoolkit::NOTIFICATION_ICON_SIZE))
        .unwrap();
    let xml = asset.to_xml();

    // The opacity ramp is what the icon is made of, so the gradient stays,
    // with white stops.
    assert!(xml.contains("aapt:attr"));
    assert!(!xml.contains("#123456"));
    assert!(xml.contains("android:color=\"#FFFFFF\""));
    assert!(xml.contains("android:color=\"#00FFFFFF\""));
    assert_eq!(asset.analysis.minimum_api, Some(24));
    let notes: Vec<&str> = asset
        .analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "SVGVD004")
        .map(|diagnostic| diagnostic.message.as_str())
        .collect();
    assert_eq!(notes, ["needs API 24 for gradients"]);
}

#[test]
fn painted_coverage_separates_silhouettes_from_plates() {
    let plate = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#101010"/>
    </svg>"##;
    let silhouette = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M10 10H14V14H10Z" fill="#101010"/>
    </svg>"##;
    let empty = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"></svg>"##;

    assert_eq!(vdtoolkit::convert(plate).unwrap().painted_coverage(), 1.0);
    let coverage = vdtoolkit::convert(silhouette).unwrap().painted_coverage();
    assert!((coverage - 16.0 / 576.0).abs() < 1e-6, "{coverage}");
    assert_eq!(vdtoolkit::convert(empty).unwrap().painted_coverage(), 0.0);
}

#[test]
fn cli_notification_writes_white_icons_and_warns_about_plates() {
    let temp = tempfile::tempdir().unwrap();
    let icons = temp.path().join("icons");
    fs::create_dir(&icons).unwrap();
    fs::write(
        icons.join("bell.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 7L7 17h10z" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    fs::write(
        icons.join("plate.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24" fill="#101010"/></svg>"##,
    )
    .unwrap();
    let drawable = temp.path().join("res/drawable");

    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .arg("notification")
        .arg(&icons)
        .arg("-o")
        .arg(&drawable)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.contains("note: ")
            && stderr.contains("bell.svg: SVGVD021  flattened 1 color to white"),
        "{stderr}"
    );
    assert!(
        stderr.contains("warning: ")
            && stderr.contains("plate.svg: SVGVD017  artwork paints 100% of the 24dp canvas"),
        "{stderr}"
    );
    assert!(!stderr.contains("bell.svg: SVGVD017"), "{stderr}");

    let bell = fs::read_to_string(drawable.join("bell.xml")).unwrap();
    assert!(bell.contains("android:width=\"24dp\""));
    assert!(bell.contains("android:fillColor=\"#FFFFFF\""));
    assert!(!bell.contains("#3DDC84"));

    // A directory of icons needs somewhere to write them.
    let missing = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .arg("notification")
        .arg(&icons)
        .output()
        .unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(
        String::from_utf8(missing.stderr)
            .unwrap()
            .contains("directory conversion requires an output directory")
    );
}

#[test]
fn cli_notification_warns_about_the_output_not_the_source() {
    let temp = tempfile::tempdir().unwrap();
    // Content bounds come before clipping, so only rendering sees that this
    // square is clipped away entirely.
    let clipped = temp.path().join("clipped.svg");
    fs::write(
        &clipped,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><clipPath id="c"><rect x="40" y="40" width="4" height="4"/></clipPath><rect width="24" height="24" fill="#3DDC84" clip-path="url(#c)"/></svg>"##,
    )
    .unwrap();
    // 500dp draws slowly as a source, but the icon is written at 24dp.
    let large = temp.path().join("large.svg");
    fs::write(
        &large,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="500" height="500"><path d="M100 100H400V400H100Z" fill="#101010"/></svg>"##,
    )
    .unwrap();

    let stderr = |icon: &std::path::Path| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .arg("notification")
            .arg(icon)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stderr).unwrap()
    };
    let clipped = stderr(&clipped);
    assert!(
        clipped.contains("no painted content, so the notification icon is invisible"),
        "{clipped}"
    );
    let large = stderr(&large);
    assert!(!large.contains("SVGVD016"), "{large}");
}

#[test]
fn cli_notification_optimize_shortens_numbers_without_moving_the_artwork() {
    let temp = tempfile::tempdir().unwrap();
    let icon = temp.path().join("bell.svg");
    fs::write(
        &icon,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path d="M24.00001 4.333333L4.6666 43.99999h38.66666z" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();

    let run = |extra: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .arg("notification")
            .arg(&icon)
            .args(extra)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    let plain = run(&[]);
    let optimized = run(&["--optimize"]);
    let pretty = run(&["--pretty"]);

    assert!(!plain.contains('\n'));
    assert!(pretty.contains('\n'));
    assert!(
        plain.contains("android:pathData=\"M12.000005,2.166667"),
        "{plain}"
    );
    assert!(
        optimized.contains("android:pathData=\"M12 2.167L2.333 22H21.667Z\""),
        "{optimized}"
    );
    assert!(optimized.len() < plain.len());
    // Optimizing runs after the fit, so the canvas and the white paint stand.
    assert!(optimized.contains("android:width=\"24dp\""));
    assert!(optimized.contains("android:fillColor=\"#FFFFFF\""));
}

#[test]
fn cli_adaptive_optimize_shortens_every_drawable_and_keeps_pngs_identical() {
    let temp = tempfile::tempdir().unwrap();
    let foreground = temp.path().join("fg.svg");
    let background = temp.path().join("bg.svg");
    // Fitting 48 units into 66dp scales by 1.375, which turns these
    // coordinates into long decimals in every layer.
    fs::write(
        &foreground,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48"><path d="M24.00001 4.333333L4.6666 43.99999h38.66666z" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    // A gradient needs API 24, so --legacy renders PNGs from the vector.
    fs::write(
        &background,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
        <defs><linearGradient id="g" x1="0.3333" y1="0" x2="99.6667" y2="100" gradientUnits="userSpaceOnUse">
            <stop offset="0" stop-color="#123456"/><stop offset="1" stop-color="#ABCDEF"/>
        </linearGradient></defs>
        <rect width="100" height="100" fill="url(#g)"/></svg>"##,
    )
    .unwrap();

    let run = |res: &std::path::Path, extra: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(["adaptive", "--foreground"])
            .arg(&foreground)
            .arg("--background")
            .arg(&background)
            .arg("--monochrome")
            .arg(&foreground)
            .args(["--fit", "66", "--legacy"])
            .args(extra)
            .arg("-o")
            .arg(res)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    let plain = temp.path().join("plain");
    let optimized = temp.path().join("optimized");
    let again = temp.path().join("again");
    run(&plain, &[]);
    run(&optimized, &["--optimize"]);
    run(&again, &["--optimize"]);

    let read =
        |res: &std::path::Path, relative: &str| fs::read_to_string(res.join(relative)).unwrap();
    let plain_foreground = read(&plain, "drawable-anydpi/ic_launcher_foreground.xml");
    let foreground_xml = read(&optimized, "drawable-anydpi/ic_launcher_foreground.xml");
    assert!(
        plain_foreground.contains("android:pathData=\"M54.00001,26.958332"),
        "{plain_foreground}"
    );
    assert!(
        foreground_xml.contains("android:pathData=\"M54 26.958L27.417 81.5H80.583Z\""),
        "{foreground_xml}"
    );
    assert!(foreground_xml.contains("android:width=\"108dp\""));
    assert!(foreground_xml.len() < plain_foreground.len());
    assert_eq!(
        read(&optimized, "drawable-anydpi/ic_launcher_monochrome.xml"),
        foreground_xml
    );
    let background_xml = read(&optimized, "drawable-anydpi/ic_launcher_background.xml");
    assert!(
        background_xml.contains("android:startX=\"0.36\""),
        "{background_xml}"
    );
    assert!(
        background_xml.contains("android:endX=\"107.64\""),
        "{background_xml}"
    );
    assert!(background_xml.contains("android:pathData=\"M0 0H108V108H0Z\""));
    let legacy = read(&optimized, "mipmap-anydpi-v24/ic_launcher.xml");
    let plain_legacy = read(&plain, "mipmap-anydpi-v24/ic_launcher.xml");
    assert!(
        plain_legacy.contains("M24.000008,7.474537"),
        "{plain_legacy}"
    );
    assert!(
        plain_legacy.contains("M-9.000002,-9.000002"),
        "{plain_legacy}"
    );
    assert!(legacy.contains("M-9-9H57V57H-9Z"), "{legacy}");
    assert!(
        legacy.contains("M24 7.475L7.755 40.806H40.245Z"),
        "{legacy}"
    );
    assert!(legacy.len() < plain_legacy.len());

    // The PNGs are rendered from the exact legacy vector, so they come out
    // byte-identical with and without --optimize, and the whole run is
    // deterministic.
    for (density, _) in vdtoolkit::LEGACY_ICON_DENSITIES {
        for name in ["ic_launcher.png", "ic_launcher_round.png"] {
            let relative = format!("mipmap-{density}/{name}");
            let bytes = fs::read(optimized.join(&relative)).unwrap();
            assert_eq!(
                bytes,
                fs::read(plain.join(&relative)).unwrap(),
                "{relative}"
            );
            assert_eq!(
                bytes,
                fs::read(again.join(&relative)).unwrap(),
                "{relative}"
            );
        }
    }
    for relative in [
        "drawable-anydpi/ic_launcher_foreground.xml",
        "drawable-anydpi/ic_launcher_background.xml",
        "drawable-anydpi/ic_launcher_monochrome.xml",
        "mipmap-anydpi-v24/ic_launcher.xml",
        "mipmap-anydpi-v24/ic_launcher_round.xml",
        "mipmap-anydpi-v26/ic_launcher.xml",
    ] {
        assert_eq!(
            read(&optimized, relative),
            read(&again, relative),
            "{relative}"
        );
    }
    assert_eq!(
        read(&optimized, "mipmap-anydpi-v26/ic_launcher.xml"),
        read(&plain, "mipmap-anydpi-v26/ic_launcher.xml")
    );
}

#[test]
fn optimizing_a_legacy_icon_only_moves_edge_pixels_by_one_coverage_step() {
    let foreground = br##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48">
        <path d="M24.00001 4.333333L4.6666 43.99999h38.66666z" fill="#3DDC84" fill-opacity="0.7"/>
        <circle cx="24.3333" cy="27.6667" r="6.1111" fill="none" stroke="#123456" stroke-width="1.3333"/>
    </svg>"##;
    let background = br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100">
        <defs><radialGradient id="g" cx="33.3333" cy="66.6667" r="70.7107" gradientUnits="userSpaceOnUse">
            <stop offset="0" stop-color="#FFFFFF"/><stop offset="1" stop-color="#ABCDEF"/>
        </radialGradient></defs>
        <rect width="100" height="100" fill="url(#g)"/></svg>"##;

    let mut foreground = vdtoolkit::convert(foreground).unwrap();
    foreground
        .fit_adaptive_layer(vdtoolkit::Fit::contain(vdtoolkit::ADAPTIVE_ICON_SAFE_ZONE))
        .unwrap();
    let mut background = vdtoolkit::convert(background).unwrap();
    background
        .fit_adaptive_layer(vdtoolkit::Fit::contain(vdtoolkit::ADAPTIVE_ICON_SIZE))
        .unwrap();
    let exact = vdtoolkit::Asset::legacy_launcher_icon(&background, &foreground);
    let mut optimized = exact.clone();
    optimized.optimize();
    assert_ne!(exact.to_xml(), optimized.to_xml());

    // Rounding to a thousandth of a dp is invisible, but it is not nothing at
    // the raster level: from 96px up, an edge that lands within a rounding
    // step of a supersample boundary can move by one coverage step. This is
    // why `adaptive --optimize` renders its PNGs from the exact vector and
    // optimizes only the XML it writes.
    for (_, pixels) in vdtoolkit::LEGACY_ICON_DENSITIES {
        let exact_pixels = exact.render_rgba(pixels, pixels).unwrap();
        let optimized_pixels = optimized.render_rgba(pixels, pixels).unwrap();
        let differences: Vec<i32> = exact_pixels
            .iter()
            .zip(&optimized_pixels)
            .map(|(a, b)| (i32::from(*a) - i32::from(*b)).abs())
            .filter(|difference| *difference != 0)
            .collect();
        let largest = differences.iter().copied().max().unwrap_or(0);
        assert!(largest <= 16, "{pixels}px: a channel moved by {largest}");
        assert!(
            differences.len() * 1000 <= exact_pixels.len(),
            "{pixels}px: {} channels differ",
            differences.len()
        );
    }
}

#[test]
fn icon_kinds_add_coded_findings_without_changing_compatibility() {
    let plate = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#101010"/>
    </svg>"##;
    let mut asset = vdtoolkit::convert(plate).unwrap();
    asset
        .to_icon(
            vdtoolkit::IconKind::Notification,
            vdtoolkit::Fit::contain(24.0),
        )
        .unwrap();
    let codes: Vec<&str> = asset
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, ["SVGVD021", "SVGVD017"], "{codes:?}");
    let plate_warning = &asset.analysis.diagnostics[1];
    assert!(matches!(plate_warning.severity, Severity::Warning));
    assert!(
        plate_warning
            .message
            .starts_with("artwork paints 100% of the 24dp canvas")
    );
    assert!(plate_warning.suggestion.is_some());
    let note = &asset.analysis.diagnostics[0];
    assert!(matches!(note.severity, Severity::Info));
    assert_eq!(
        note.message,
        "flattened 1 color to white; Android tints the alpha channel only"
    );
    assert_eq!(asset.analysis.compatibility, Compatibility::Exact);
    assert_eq!(asset.analysis.minimum_api, Some(21));
    assert!(asset.to_xml().contains("android:fillColor=\"#FFFFFF\""));

    let empty = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"></svg>"##;
    let mut asset = vdtoolkit::convert(empty).unwrap();
    asset
        .to_icon(
            vdtoolkit::IconKind::Notification,
            vdtoolkit::Fit::contain(24.0),
        )
        .unwrap();
    assert!(asset.analysis.diagnostics.iter().any(|diagnostic| {
        diagnostic.code.as_str() == "SVGVD018" && diagnostic.message.contains("no painted content")
    }));

    // A plain logo fitted to the full layer is clipped by a circular mask.
    // --fit 66 is not enough for artwork drawn edge to edge, because the
    // corners of a 66dp box sit 46.7dp out and the mask shows 36dp; the fit
    // the finding suggests is what clears it.
    let logo = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 2L2 22h20z" fill="#3DDC84"/></svg>"##;
    let mut asset = vdtoolkit::convert(logo).unwrap();
    asset
        .to_icon(
            vdtoolkit::IconKind::AdaptiveForeground,
            vdtoolkit::Fit::contain(108.0),
        )
        .unwrap();
    let outside = asset
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "SVGVD019")
        .expect("safe zone warning");
    assert_eq!(
        outside.message,
        "content reaches 63.5dp from the centre, past the 36dp a circular launcher mask shows, \
         so it is clipped"
    );
    assert_eq!(
        outside.suggestion.as_deref(),
        Some("Scale the artwork into the circle with --fit 56 or smaller.")
    );
    let mut asset = vdtoolkit::convert(logo).unwrap();
    asset
        .to_icon(
            vdtoolkit::IconKind::AdaptiveForeground,
            vdtoolkit::Fit::contain(56.0),
        )
        .unwrap();
    assert!(
        asset.analysis.diagnostics.is_empty(),
        "{:?}",
        asset.analysis.diagnostics
    );

    // A non-square background leaves bands; a full square background is fine.
    let wide = br##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="54"><path d="M0 0H108V54H0Z" fill="#FFFFFF"/></svg>"##;
    let mut asset = vdtoolkit::convert(wide).unwrap();
    asset
        .to_icon(
            vdtoolkit::IconKind::AdaptiveBackground,
            vdtoolkit::Fit::contain(108.0),
        )
        .unwrap();
    let gap = asset
        .analysis
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.code.as_str() == "SVGVD020")
        .expect("background warning");
    assert_eq!(
        gap.message,
        "background content spans 0..108 × 27..81dp and does not fill the 108dp layer; \
         uncovered areas show through launcher masks and parallax"
    );
    assert!(
        gap.suggestion
            .as_deref()
            .unwrap()
            .starts_with("Use --background-fit cover to fill the layer"),
        "{:?}",
        gap.suggestion
    );

    // Covering the same artwork fills the layer and reports the crop instead.
    let mut asset = vdtoolkit::convert(wide).unwrap();
    asset
        .to_icon(
            vdtoolkit::IconKind::AdaptiveBackground,
            vdtoolkit::Fit::cover(108.0),
        )
        .unwrap();
    let codes: Vec<&str> = asset
        .analysis
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert_eq!(codes, ["SVGVD023"]);
    assert_eq!(
        asset.analysis.diagnostics[0].message,
        "scaled the background to cover the 108dp layer, which cropped 50% of the artwork"
    );
    assert!(matches!(
        asset.analysis.diagnostics[0].severity,
        Severity::Info
    ));
    let square = br##"<svg xmlns="http://www.w3.org/2000/svg" width="108" height="108"><path d="M0 0H108V108H0Z" fill="#FFFFFF"/></svg>"##;
    let mut asset = vdtoolkit::convert(square).unwrap();
    asset
        .to_icon(
            vdtoolkit::IconKind::AdaptiveBackground,
            vdtoolkit::Fit::contain(108.0),
        )
        .unwrap();
    assert!(asset.analysis.diagnostics.is_empty());

    // The kinds carry their canvas, and the fit is validated against it.
    assert_eq!(vdtoolkit::IconKind::Notification.canvas(), 24.0);
    assert_eq!(
        vdtoolkit::IconKind::AdaptiveBackground.default_fit(),
        vdtoolkit::Fit::cover(108.0)
    );
    assert_eq!(
        vdtoolkit::IconKind::AdaptiveForeground.default_fit(),
        vdtoolkit::Fit::contain(108.0)
    );
    assert!(matches!(
        asset.to_icon(
            vdtoolkit::IconKind::Notification,
            vdtoolkit::Fit::contain(25.0)
        ),
        Err(Error::InvalidInput(_))
    ));
    assert_eq!(
        serde_json::to_string(&vdtoolkit::IconKind::AdaptiveForeground).unwrap(),
        "\"adaptive-foreground\""
    );
}

#[test]
fn analyze_as_reports_icon_findings_without_a_drawable() {
    let plate = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <path d="M0 0H24V24H0Z" fill="#101010"/>
    </svg>"##;
    let analysis = vdtoolkit::analyze_as(
        plate,
        vdtoolkit::IconKind::Notification,
        vdtoolkit::Fit::contain(24.0),
    )
    .unwrap();
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "SVGVD017")
    );
    assert_eq!(analysis.metrics.width, 24.0);

    // An SVG that cannot be converted still gets its compatibility analysis.
    let masked = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
        <defs><mask id="m"><rect width="24" height="24" fill="url(#g)"/></mask>
        <linearGradient id="g"><stop offset="0" stop-color="#fff"/><stop offset="1" stop-color="#000"/></linearGradient></defs>
        <rect width="24" height="24" fill="#f00" mask="url(#m)"/>
    </svg>"##;
    let analysis = vdtoolkit::analyze_as(
        masked,
        vdtoolkit::IconKind::Notification,
        vdtoolkit::Fit::contain(24.0),
    )
    .unwrap();
    assert!(!analysis.compatibility.convertible());
    assert!(
        analysis
            .diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code.as_str() != "SVGVD017")
    );
}

#[test]
fn cli_inspect_as_reports_icon_findings_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    let icons = temp.path().join("icons");
    fs::create_dir(&icons).unwrap();
    let logo = icons.join("logo.svg");
    fs::write(
        &logo,
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 2L2 22h20z" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    fs::write(
        icons.join("plate.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect width="24" height="24" fill="#101010"/></svg>"##,
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["inspect", "--as", "notification", "--format", "json"])
        .arg(&icons)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let entries = report.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    let codes = |entry: &serde_json::Value| -> Vec<String> {
        entry["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|diagnostic| diagnostic["code"].as_str().unwrap().to_owned())
            .collect()
    };
    let (logo_entry, plate_entry) = (&entries[0], &entries[1]);
    assert!(logo_entry["path"].as_str().unwrap().ends_with("logo.svg"));
    assert_eq!(
        logo_entry["icon"],
        serde_json::json!({"kind": "notification", "fit": 24.0, "fit_mode": "contain"})
    );
    assert_eq!(codes(logo_entry), ["SVGVD021"]);
    assert_eq!(codes(plate_entry), ["SVGVD011", "SVGVD021", "SVGVD017"]);
    assert_eq!(plate_entry["diagnostics"][2]["severity"], "warning");
    assert_eq!(plate_entry["metrics"]["width"], 24.0);
    assert_eq!(plate_entry["compatibility"], "exact_with_normalization");
    // Nothing was written next to the inputs.
    assert_eq!(fs::read_dir(&icons).unwrap().count(), 2);

    // The adaptive kinds take the generator's --fit; a plain report has no
    // icon field and no icon findings.
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(args)
            .arg(&logo)
            .output()
            .unwrap();
        (
            output.status.code(),
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    };
    let (code, stdout, _) = run(&["inspect", "--as", "adaptive-foreground"]);
    assert_eq!(code, Some(0));
    assert!(
        stdout.contains("SVGVD019  content reaches 63.5dp from the centre"),
        "{stdout}"
    );
    assert!(stdout.contains("Viewport: 108 × 108"), "{stdout}");
    let (code, stdout, _) = run(&["check", "--as", "adaptive-foreground", "--fit", "56"]);
    assert_eq!(code, Some(0));
    assert!(!stdout.contains("SVGVD019"), "{stdout}");
    let (_, stdout, _) = run(&["inspect", "--format", "json"]);
    assert!(!stdout.contains("\"icon\""), "{stdout}");
    assert!(!stdout.contains("SVGVD02"), "{stdout}");

    // --fit needs a kind, and must fit the kind's canvas.
    let (code, _, stderr) = run(&["inspect", "--fit", "20"]);
    assert_eq!(code, Some(2));
    assert!(stderr.contains("--as"), "{stderr}");
    let (code, _, stderr) = run(&["inspect", "--as", "notification", "--fit", "30"]);
    assert_eq!(code, Some(1));
    assert!(
        stderr.contains("--fit must be between 0 and 24 dp, got 30"),
        "{stderr}"
    );
}

#[test]
fn cli_inspect_as_reports_raster_foreground_findings_without_writing() {
    let temp = tempfile::tempdir().unwrap();
    // Circles of known radius on a 432px layer: the 66dp safe zone is the
    // middle 264px, so r=120 sits inside it and r=200 runs past it.
    let disc = |radius: f32, alpha: u8| {
        let size = 432u32;
        let mut pixels = vec![0u8; (size * size * 4) as usize];
        let center = size as f32 / 2.0;
        for y in 0..size {
            for x in 0..size {
                let (dx, dy) = (x as f32 + 0.5 - center, y as f32 + 0.5 - center);
                if dx * dx + dy * dy <= radius * radius {
                    let at = ((y * size + x) * 4) as usize;
                    pixels[at..at + 4].copy_from_slice(&[0xFF, 0xFF, 0xFF, alpha]);
                }
            }
        }
        let mut out = Vec::new();
        image_webp::WebPEncoder::new(std::io::Cursor::new(&mut out))
            .encode(&pixels, size, size, image_webp::ColorType::Rgba8)
            .unwrap();
        out
    };
    let write = |name: &str, bytes: &[u8]| {
        let path = temp.path().join(name);
        fs::write(&path, bytes).unwrap();
        path
    };
    let inside = write("inside.webp", &disc(120.0, 0xFF));
    let outside = write("outside.webp", &disc(200.0, 0xFF));
    let opaque = write("opaque.webp", &disc(1000.0, 0xFF));
    let empty = write("empty.webp", &disc(0.0, 0xFF));
    let jpeg = write("fg.jpg", include_bytes!("fixtures/background_square.jpg"));

    let inspect = |args: &[&str], input: &std::path::Path| {
        let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args(args)
            .arg(input)
            .output()
            .unwrap();
        (
            output.status.code(),
            String::from_utf8(output.stdout).unwrap(),
            String::from_utf8(output.stderr).unwrap(),
        )
    };
    let json_codes = |stdout: &str| -> Vec<String> {
        let report: serde_json::Value = serde_json::from_str(stdout).unwrap();
        report[0]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|diagnostic| diagnostic["code"].as_str().unwrap().to_owned())
            .collect()
    };

    // Artwork inside the safe zone, with transparency around it, is what a
    // foreground should be. --fit is ignored: a raster layer is placed, never
    // resampled.
    let (code, stdout, _) = inspect(
        &[
            "inspect",
            "--as",
            "adaptive-foreground",
            "--fit",
            "66",
            "--format",
            "json",
        ],
        &inside,
    );
    assert_eq!(code, Some(0), "{stdout}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    assert_eq!(report[0]["image"], "webp");
    assert_eq!(
        report[0]["icon"],
        serde_json::json!({"kind": "adaptive-foreground", "fit": 66.0, "fit_mode": "contain"})
    );
    assert!(json_codes(&stdout).is_empty(), "{stdout}");
    assert_eq!(report[0]["metrics"]["width"], 432.0);
    assert_eq!(report[0]["metrics"]["viewport_width"], 108.0);
    assert_eq!(report[0]["compatibility"], "exact");
    assert!(report[0]["minimum_api"].is_null());
    assert!(!inside.with_extension("xml").exists());

    let (code, stdout, _) = inspect(&["inspect", "--as", "adaptive-foreground"], &outside);
    assert_eq!(code, Some(0), "{stdout}");
    assert!(stdout.contains("Adaptive layer image"), "{stdout}");
    assert!(stdout.contains("SVGVD019"), "{stdout}");
    assert!(stdout.contains("Format: webp"), "{stdout}");
    assert!(stdout.contains("Dimensions: 432 × 432 px"), "{stdout}");
    assert!(stdout.contains("Layer: 108 × 108 dp"), "{stdout}");
    assert!(!stdout.contains("VectorDrawable compatible"), "{stdout}");
    assert!(!stdout.contains("Estimated XML size"), "{stdout}");

    let (code, stdout, _) = inspect(
        &["inspect", "--as", "adaptive-foreground", "--format", "json"],
        &opaque,
    );
    assert_eq!(code, Some(0), "{stdout}");
    assert_eq!(json_codes(&stdout), ["SVGVD026"]);

    let (code, stdout, _) = inspect(
        &["inspect", "--as", "adaptive-foreground", "--format", "json"],
        &empty,
    );
    assert_eq!(code, Some(0), "{stdout}");
    assert_eq!(json_codes(&stdout), ["SVGVD018"]);

    // A JPEG has no alpha, so as a foreground it would hide the background.
    let (code, stdout, stderr) = inspect(
        &["inspect", "--as", "adaptive-foreground", "--format", "json"],
        &jpeg,
    );
    assert_eq!(code, Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("a JPEG has no alpha channel"),
        "{stdout}\n{stderr}"
    );

    // The same JPEG is a valid opaque background. A non-square JPEG still
    // reports the shape and resolution findings a background image gets.
    let wide = write("wide.jpg", include_bytes!("fixtures/background_wide.jpg"));
    let (code, stdout, _) = inspect(
        &["inspect", "--as", "adaptive-background", "--format", "json"],
        &wide,
    );
    assert_eq!(code, Some(0), "{stdout}");
    assert_eq!(json_codes(&stdout), ["SVGVD024", "SVGVD025"]);
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&stdout).unwrap()[0]["image"],
        "jpg"
    );

    // Without --as, a raster file is not an SVG. The --as hint is CLI-only:
    // analyze() itself does not name flags the WASM binding cannot pass.
    assert!(matches!(
        vdtoolkit::analyze(&fs::read(&inside).unwrap()),
        Err(Error::InvalidInput(message)) if message == "this is a raster image, not an SVG"
    ));
    let (code, stdout, stderr) = inspect(&["inspect", "--format", "json"], &inside);
    assert_eq!(code, Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("this is a raster image, not an SVG"),
        "{stdout}\n{stderr}"
    );
    assert!(
        stderr.contains("--as adaptive-foreground"),
        "{stdout}\n{stderr}"
    );

    // A notification icon is always a vector drawable.
    let (code, stdout, stderr) = inspect(
        &["inspect", "--as", "notification", "--format", "json"],
        &inside,
    );
    assert_eq!(code, Some(1), "{stdout}\n{stderr}");
    assert!(
        stderr.contains("notification icons are vector drawables"),
        "{stdout}\n{stderr}"
    );

    // A directory with --as adaptive-foreground keeps both SVGs and layer images.
    let mixed = temp.path().join("mixed");
    fs::create_dir(&mixed).unwrap();
    fs::write(
        mixed.join("logo.svg"),
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M12 2L2 22h20z" fill="#3DDC84"/></svg>"##,
    )
    .unwrap();
    fs::copy(&outside, mixed.join("outside.webp")).unwrap();
    let (code, stdout, _) = inspect(
        &["inspect", "--as", "adaptive-foreground", "--format", "json"],
        &mixed,
    );
    assert_eq!(code, Some(0), "{stdout}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let entries = report.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert!(entries[0]["path"].as_str().unwrap().ends_with("logo.svg"));
    assert!(entries[0].get("image").is_none());
    assert_eq!(entries[1]["image"], "webp");
    assert!(
        entries[1]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| diagnostic["code"] == "SVGVD019")
    );

    // --as notification on a directory still keeps only SVGs; the WebP is not
    // refused, it is skipped.
    let (code, stdout, _) = inspect(
        &["inspect", "--as", "notification", "--format", "json"],
        &mixed,
    );
    assert_eq!(code, Some(0), "{stdout}");
    let report: serde_json::Value = serde_json::from_str(&stdout).unwrap();
    let entries = report.as_array().unwrap();
    assert_eq!(entries.len(), 1);
    assert!(entries[0]["path"].as_str().unwrap().ends_with("logo.svg"));

    // An empty tree with --as adaptive-foreground names layer images, not SVGs.
    let empty_dir = temp.path().join("empty");
    fs::create_dir(&empty_dir).unwrap();
    let (code, _, stderr) = inspect(&["inspect", "--as", "adaptive-foreground"], &empty_dir);
    assert_eq!(code, Some(1), "{stderr}");
    assert!(
        stderr.contains("no SVG or layer image files found"),
        "{stderr}"
    );
}

fn icon_svg(size: u32) -> String {
    format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}"><path d="M0 0H{size}V{size}Z" fill="#000"/></svg>"##
    )
}

#[test]
fn cli_writes_valid_android_resource_names() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icons");
    let output = temp.path().join("drawable");
    fs::create_dir_all(&input).unwrap();
    for name in [
        "Arrow-Left",
        "HTTPServer",
        "2x",
        "switch",
        "_private",
        "ic_ok",
        "c++",
        "c#",
        "Café",
        "100%",
    ] {
        fs::write(input.join(name).with_extension("svg"), icon_svg(24)).unwrap();
    }
    // Sorts after `Arrow-Left` and would overwrite its output.
    fs::write(input.join("arrow_left.svg"), icon_svg(48)).unwrap();

    let result = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args([input.to_str().unwrap(), "-o", output.to_str().unwrap()])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("arrow_left.svg"), "{stderr}");
    assert!(stderr.contains("rename one of them"), "{stderr}");
    assert!(stderr.contains("written as"), "{stderr}");
    assert!(
        !stderr.contains("ic_ok"),
        "valid names are not renamed: {stderr}"
    );

    let mut written: Vec<String> = fs::read_dir(&output)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    written.sort();
    assert_eq!(
        written,
        [
            "_private.xml",
            "arrow_left.xml",
            "c_plus_plus.xml",
            "c_sharp.xml",
            "cafe.xml",
            "http_server.xml",
            "ic_100_percent.xml",
            "ic_2x.xml",
            "ic_ok.xml",
            "ic_switch.xml",
        ]
    );
    let kept = fs::read_to_string(output.join("arrow_left.xml")).unwrap();
    assert_eq!(dimension(&kept, "width"), 24.0);

    let single = temp.path().join("My Icon.xml");
    let result = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args([
            "convert",
            input.join("2x.svg").to_str().unwrap(),
            "-o",
            single.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "{:?}", result.stderr);
    assert!(temp.path().join("my_icon.xml").is_file());
    assert!(!single.exists());

    let adaptive = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args([
            "adaptive",
            "--foreground",
            input.join("2x.svg").to_str().unwrap(),
            "--background-color",
            "#000000",
            "--name",
            "switch",
            "-o",
            temp.path().join("res").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(adaptive.status.code(), Some(1));
}

#[test]
fn cli_size_sets_the_longer_side_and_keeps_the_viewport() {
    let temp = tempfile::tempdir().unwrap();
    let convert = |source: &[u8], extra: &[&str]| {
        let input = temp.path().join("in.svg");
        let output = temp.path().join("out.xml");
        fs::write(&input, source).unwrap();
        let _ = fs::remove_file(&output);
        let result = Command::new(env!("CARGO_BIN_EXE_vdt"))
            .args([
                "convert",
                input.to_str().unwrap(),
                "-o",
                output.to_str().unwrap(),
            ])
            .args(extra)
            .output()
            .unwrap();
        let xml = fs::read_to_string(&output).unwrap_or_default();
        (result, xml)
    };

    let (result, xml) = convert(LARGE, &["--size", "24"]);
    assert!(result.status.success());
    assert_eq!(dimension(&xml, "width"), 24.0);
    assert_eq!(dimension(&xml, "height"), 16.0);
    assert_eq!(attribute(&xml, "viewportWidth"), 480.0);
    assert!(
        !String::from_utf8(result.stderr)
            .unwrap()
            .contains("SVGVD016")
    );

    let (result, xml) = convert(LARGE, &[]);
    assert!(result.status.success());
    assert_eq!(dimension(&xml, "width"), 480.0);
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(
        stderr.contains("SVGVD016") && stderr.contains("--size 24"),
        "{stderr}"
    );

    let symbols = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 -960 960 960"><path d="M80 -80H880V-880Z"/></svg>"##;
    let (result, xml) = convert(symbols, &[]);
    assert!(result.status.success());
    assert_eq!(dimension(&xml, "width"), 24.0);
    assert_eq!(dimension(&xml, "height"), 24.0);
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(
        stderr.contains("note:") && !stderr.contains("SVGVD016"),
        "{stderr}"
    );

    let small = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 48 48"><path d="M0 0H48V48Z"/></svg>"##;
    let (result, xml) = convert(small, &[]);
    assert!(result.status.success());
    assert_eq!(dimension(&xml, "width"), 48.0);
    assert!(result.stderr.is_empty());

    let (result, _) = convert(small, &["--size", "0"]);
    assert_eq!(result.status.code(), Some(1));
}

#[test]
fn set_size_scales_both_ways_and_updates_the_warning() {
    let mut asset = vdtoolkit::convert(LARGE).unwrap();
    assert!(asset.has_declared_size());
    asset.set_size(24.0).unwrap();
    assert_eq!(
        (asset.analysis.metrics.width, asset.analysis.metrics.height),
        (24.0, 16.0)
    );
    assert!(asset.analysis.diagnostics.is_empty());
    asset.set_size(300.0).unwrap();
    assert_eq!(asset.analysis.metrics.height, 200.0);
    assert_eq!(asset.analysis.diagnostics[0].code.as_str(), "SVGVD016");
    assert!(asset.set_size(0.0).is_err());
    assert!(asset.set_size(f32::NAN).is_err());

    let viewbox_only = br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0H24V24Z"/></svg>"##;
    assert!(
        !vdtoolkit::convert(viewbox_only)
            .unwrap()
            .has_declared_size()
    );
}

fn dimension(xml: &str, name: &str) -> f32 {
    let key = format!("android:{name}=\"");
    let start = xml
        .find(&key)
        .unwrap_or_else(|| panic!("missing android:{name} in\n{xml}"))
        + key.len();
    let end = xml[start..].find("dp\"").unwrap() + start;
    xml[start..end].parse().unwrap()
}

#[test]
fn every_error_suggests_a_fix() {
    let inputs: [(&str, &str); 10] = [
        (
            "filter",
            r##"<defs><filter id="f"><feGaussianBlur stdDeviation="1"/></filter></defs><path d="M0 0H24V24Z" filter="url(#f)"/>"##,
        ),
        (
            "pattern",
            r##"<defs><pattern id="p" width="4" height="4"><path d="M0 0H2V2Z"/></pattern></defs><path d="M0 0H24V24Z" fill="url(#p)"/>"##,
        ),
        (
            "dashes",
            r##"<path d="M2 12H22" stroke="#000" stroke-dasharray="2 2"/>"##,
        ),
        (
            "animation",
            r##"<path d="M0 0H24V24Z"><animate attributeName="opacity" from="1" to="0" dur="1s"/></path>"##,
        ),
        ("external", r##"<use href="other.svg#a"/>"##),
        (
            "focal",
            r##"<defs><radialGradient id="g" fx="0.2"><stop offset="0" stop-color="#000"/><stop offset="1" stop-color="#fff"/></radialGradient></defs><path d="M0 0H24V24Z" fill="url(#g)"/>"##,
        ),
        (
            "non_scaling",
            r##"<path d="M2 12H22" stroke="#000" vector-effect="non-scaling-stroke"/>"##,
        ),
        (
            "blend",
            r##"<path d="M0 0H24V24Z" style="mix-blend-mode:multiply"/>"##,
        ),
        (
            "group_opacity",
            r##"<g opacity="0.5"><path d="M0 0H12V12Z"/><path d="M6 6H18V18Z"/></g>"##,
        ),
        ("text", r##"<text x="2" y="12">hi</text>"##),
    ];
    let directory = tempfile::tempdir().unwrap();
    for (name, body) in inputs {
        let svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">{body}</svg>"#
        );
        fs::write(directory.path().join(format!("{name}.svg")), svg).unwrap();
    }

    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .args(["inspect", "--format", "json"])
        .arg(directory.path())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let entries = report.as_array().unwrap();
    assert_eq!(entries.len(), inputs.len());
    for entry in entries {
        let errors: Vec<_> = entry["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|diagnostic| diagnostic["severity"] == "error")
            .collect();
        assert!(!errors.is_empty(), "{entry}");
        for error in errors {
            assert!(error["suggestion"].is_string(), "{error}");
        }
    }

    let output = Command::new(env!("CARGO_BIN_EXE_vdt"))
        .arg("check")
        .arg(directory.path().join("dashes.svg"))
        .output()
        .unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.contains("dashed strokes cannot be represented by VectorDrawable\n  → Convert the dashed stroke to filled outlines.\n"),
        "{stdout}"
    );
}

#[test]
fn wide_gamut_colors_keep_their_paint_instead_of_vanishing_or_turning_black() {
    // Figma writes the sRGB fallback first and the wide-gamut color after it,
    // both in the style attribute, which outranks the presentation attribute.
    // Dropping the color() function it cannot read used to take the stroke
    // with it, leaving a 24dp notification icon with nothing painted.
    let stroked = br##"<svg width="20" height="20" viewBox="0 0 20 20" fill="none" xmlns="http://www.w3.org/2000/svg">
        <path d="M2 2H18V18H2Z" stroke="#6C707E" style="stroke:#6C707E;stroke:color(display-p3 0.4235 0.4392 0.4941);stroke-opacity:1;" stroke-width="1.5"/>
    </svg>"##;
    let filled = br##"<svg width="20" height="20" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
        <path d="M2 2H18V18H2Z" fill="#6C707E" style="fill:#6C707E;fill:color(display-p3 0.4235 0.4392 0.4941);fill-opacity:1;"/>
    </svg>"##;

    let asset = vdtoolkit::convert(stroked).unwrap();
    assert!(
        asset.to_xml().contains("android:strokeColor=\"#6B707F\""),
        "{}",
        asset.to_xml()
    );
    assert!(asset.painted_coverage() > 0.0);

    // An unreadable fill used to fall back to black, the SVG initial value,
    // rather than to the sRGB the design tool wrote beside it.
    let asset = vdtoolkit::convert(filled).unwrap();
    assert!(
        asset.to_xml().contains("android:fillColor=\"#6B707F\""),
        "{}",
        asset.to_xml()
    );

    let mut icon = vdtoolkit::convert(stroked).unwrap();
    icon.to_icon(
        vdtoolkit::IconKind::Notification,
        vdtoolkit::Fit::contain(24.0),
    )
    .unwrap();
    assert!(
        !icon
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "SVGVD018"),
        "{:?}",
        icon.analysis.diagnostics
    );
}

#[test]
fn resolving_a_wide_gamut_color_is_reported_as_an_informational_note() {
    let source = br##"<svg width="20" height="20" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
        <path d="M2 2H18V18H2Z" fill="#6C707E" style="fill:color(display-p3 0.4235 0.4392 0.4941);"/>
        <path d="M4 4H8V8H4Z" stroke="#6C707E" style="stroke:color(display-p3 0.4235 0.4392 0.4941);"/>
    </svg>"##;

    let asset = vdtoolkit::convert(source).unwrap();
    let [note] = asset
        .analysis
        .diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code.as_str() == "SVGVD022")
        .collect::<Vec<_>>()[..]
    else {
        panic!("expected one note: {:?}", asset.analysis.diagnostics);
    };
    assert!(matches!(note.severity, Severity::Info));
    assert_eq!(
        note.message,
        "resolved 2 wide-gamut colors to sRGB; VectorDrawable has no wide-gamut color"
    );
    // A note never moves a drawable out of exact conversion.
    assert_eq!(asset.analysis.compatibility, Compatibility::Exact);

    // One color reads as a singular.
    let single = br##"<svg width="20" height="20" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
        <path d="M2 2H18V18H2Z" fill="#6C707E" style="fill:color(display-p3 0.4235 0.4392 0.4941);"/>
    </svg>"##;
    assert!(
        vdtoolkit::convert(single)
            .unwrap()
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic
                .message
                .starts_with("resolved 1 wide-gamut color to")),
    );

    // An SVG with no color() function says nothing.
    let plain =
        br##"<svg width="20" height="20" viewBox="0 0 20 20" xmlns="http://www.w3.org/2000/svg">
        <path d="M2 2H18V18H2Z" fill="#6C707E"/>
    </svg>"##;
    assert!(
        !vdtoolkit::convert(plain)
            .unwrap()
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "SVGVD022"),
    );
}
