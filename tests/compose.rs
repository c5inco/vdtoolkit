//! The Compose `ImageVector` target: the Kotlin `vdt compose` writes, the
//! names it derives, and the numbers that Kotlin draws.
//!
//! One drawing that uses every feature this target writes is checked in, so a
//! change to the shape of the output is a reviewable diff. Everything else
//! compares the Kotlin's numbers with the drawable's own numbers: what those
//! numbers draw is verified on a device by `tools/android-renderer`, and
//! reading them back out of the Kotlin is what ties this target to that
//! evidence. Compiling the output against Compose itself is a one-off check
//! rather than a cargo test; `docs/benchmarks.md` records it.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use vdtoolkit::{Compatibility, DiagnosticCode};

/// A generated file of every feature, from `fixtures/compose/features.svg`.
const FEATURES: &str = include_str!("fixtures/compose/features.kt");

#[test]
fn compose_writes_a_lazy_image_vector_without_arguments_that_equal_defaults() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path d="M12 2L2 22h20z" fill="#3DDC84"/>
    </svg>"##;
    let asset = vdtoolkit::convert(source).unwrap();
    let kotlin = asset.to_kotlin("Triangle", Some("com.example.icons"));

    assert!(kotlin.starts_with("package com.example.icons\n\nimport "));
    for import in [
        "import androidx.compose.ui.graphics.Color\n",
        "import androidx.compose.ui.graphics.SolidColor\n",
        "import androidx.compose.ui.graphics.vector.ImageVector\n",
        "import androidx.compose.ui.graphics.vector.path\n",
        "import androidx.compose.ui.unit.dp\n",
    ] {
        assert!(kotlin.contains(import), "missing {import}");
    }
    assert!(kotlin.contains("\npublic val Triangle: ImageVector\n"));
    assert!(kotlin.contains("            defaultWidth = 24.dp,\n"));
    assert!(kotlin.contains("            viewportWidth = 24f,\n"));
    assert!(kotlin.contains("            path(fill = SolidColor(Color(0xFF3DDC84))) {\n"));
    assert!(kotlin.contains("                moveTo(12f, 2f)\n"));
    assert!(kotlin.contains("                lineTo(22f, 22f)\n"));
    assert!(kotlin.contains("                close()\n"));
    assert!(kotlin.ends_with("private var _Triangle: ImageVector? = null\n"));

    // Compose already assumes these, so writing them would only be noise.
    for argument in [
        "fillAlpha",
        "stroke = null",
        "strokeAlpha",
        "strokeLineWidth",
        "strokeLineCap",
        "strokeLineJoin",
        "strokeLineMiter",
        "pathFillType = PathFillType.NonZero",
        "autoMirror",
        "tintColor",
    ] {
        assert!(!kotlin.contains(argument), "{argument} should be left out");
    }
}

#[test]
fn compose_writes_brushes_strokes_and_even_odd_fills() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="48" viewBox="0 0 48 48">
        <defs>
            <linearGradient id="g" x1="0" y1="0" x2="48" y2="0" gradientUnits="userSpaceOnUse" spreadMethod="repeat">
                <stop offset="0" stop-color="#ff0000"/>
                <stop offset="1" stop-color="#0000ff" stop-opacity="0.5"/>
            </linearGradient>
        </defs>
        <path d="M0 0H48V48H0Z M8 8H40V40H8Z" fill="url(#g)" fill-rule="evenodd"/>
        <path d="M4 44C8 20 40 20 44 44" fill="none" stroke="#00ff00" stroke-width="3"
            stroke-linecap="round" stroke-linejoin="bevel" stroke-miterlimit="6" stroke-opacity="0.5"/>
    </svg>"##;
    let asset = vdtoolkit::convert(source).unwrap();
    let kotlin = asset.to_kotlin("Gradient", None);
    assert_eq!(asset.analysis.compatibility, Compatibility::Exact);

    // A package line is optional, and nothing else may claim to be one.
    assert!(kotlin.starts_with("import "));
    for line in [
        "                fill = Brush.linearGradient(\n",
        "                    colorStops = arrayOf(\n",
        "                        0f to Color(0xFFFF0000),\n",
        "                        1f to Color(0x800000FF),\n",
        "                    ),\n",
        "                    start = Offset(0f, 0f),\n",
        "                    tileMode = TileMode.Repeated,\n",
        "                ),\n",
        "                pathFillType = PathFillType.EvenOdd,\n",
        "                stroke = SolidColor(Color(0xFF00FF00)),\n",
        "                strokeAlpha = 0.5f,\n",
        "                strokeLineWidth = 3f,\n",
        "                strokeLineCap = StrokeCap.Round,\n",
        "                strokeLineJoin = StrokeJoin.Bevel,\n",
        "                strokeLineMiter = 6f,\n",
        "                curveTo(",
    ] {
        assert!(kotlin.contains(line), "missing {line:?} in\n{kotlin}");
    }
    // The lowering re-expresses a gradient axis and lands within a float step
    // of the source, exactly as the XML target's `android:endX` does.
    let (end_x, end_y) = offset_line(&kotlin, "end");
    assert!((end_x - 48.0).abs() < 0.001, "{end_x}");
    assert_eq!(end_y, 0.0);
}

/// The `name = Offset(x, y)` line of a brush.
fn offset_line(kotlin: &str, name: &str) -> (f32, f32) {
    let prefix = format!("{name} = Offset(");
    let line = kotlin
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with(&prefix))
        .unwrap_or_else(|| panic!("no {name} in\n{kotlin}"));
    let inner = line.trim_start_matches(&prefix).trim_end_matches(',');
    let (x, y) = inner
        .trim_end_matches(')')
        .split_once(", ")
        .unwrap_or_else(|| panic!("{line} is not an Offset"));
    (
        x.trim_end_matches('f').parse().unwrap(),
        y.trim_end_matches('f').parse().unwrap(),
    )
}

#[test]
fn compose_writes_a_radial_gradient_as_a_radial_brush() {
    let kotlin = compose(&source_of(&fixture("radial_gradient.svg")), "Radial");
    assert!(
        kotlin.contains("fill = Brush.radialGradient(\n"),
        "{kotlin}"
    );
    assert!(kotlin.contains("center = Offset(12f, 12f),\n"), "{kotlin}");
    assert!(kotlin.contains("radius = 8f,\n"), "{kotlin}");
    assert!(kotlin.contains("import androidx.compose.ui.geometry.Offset\n"));
    assert!(kotlin.contains("import androidx.compose.ui.graphics.Brush\n"));
}

#[test]
fn compose_nests_groups_for_a_mask_that_lowers_to_two_clip_paths() {
    let kotlin = compose(&source_of(&fixture("hard_white_mask_region.svg")), "Mask");
    // One clip per group: Compose fills a group's `clipPathData` as one path,
    // which unions it, so a second clip has to nest to intersect the first.
    assert_eq!(kotlin.matches("group(\n").count(), 2, "{kotlin}");
    assert_eq!(kotlin.matches("clipPathData = PathData {").count(), 2);
    assert_eq!(
        kotlin
            .matches("import androidx.compose.ui.graphics.vector.PathData\n")
            .count(),
        1
    );
    assert!(
        kotlin.contains("            group(\n                clipPathData"),
        "{kotlin}"
    );
    assert!(
        kotlin.contains("                group(\n                    clipPathData"),
        "{kotlin}"
    );
}

#[test]
fn compose_writes_group_transforms_that_a_drawable_group_uses() {
    let kotlin = compose(&source_of(&fixture("elliptical_glow.svg")), "Glow");
    assert!(
        kotlin.contains(
            "group(rotate = -20.000002f, pivotX = 12.000001f, pivotY = 12f, scaleY = 0.5f) {\n"
        ),
        "{kotlin}"
    );
}

#[test]
fn compose_leaves_out_a_stroke_that_compose_would_draw_as_a_hairline() {
    // `usvg` drops a `stroke-width="0"` stroke the way SVG renders it, so the
    // case to guard is a width that `optimize` rounds to zero: a drawable
    // draws nothing for `android:strokeWidth="0"`, while Compose draws one
    // pixel of the rasterized vector.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path d="M2 2L22 22" fill="#123456" stroke="#FF0000" stroke-width="0.0004"/>
    </svg>"##;
    let mut asset = vdtoolkit::convert(source).unwrap();
    let plain = asset.to_kotlin("Tiny", None);
    assert!(plain.contains("strokeLineWidth = 0.0004f"), "{plain}");

    asset.optimize();
    let optimized = asset.to_kotlin("Tiny", None);
    assert!(!optimized.contains("stroke = "), "{optimized}");
    assert!(
        asset.to_xml().contains("android:strokeWidth=\"0\""),
        "the drawable should keep the stroke the Kotlin leaves out"
    );
}

#[test]
fn compose_output_is_deterministic() {
    let source = source_of(&fixture("elliptical_glow.svg"));
    assert_eq!(compose(&source, "Glow"), compose(&source, "Glow"));
}

#[test]
fn compose_matches_the_checked_in_file_for_a_drawing_with_every_feature() {
    let asset = vdtoolkit::convert(&source_of(&fixture_path("features.svg"))).unwrap();
    assert_eq!(
        asset.to_kotlin("Features", Some("com.example.icons")),
        FEATURES
    );
}

#[test]
fn compose_writes_the_numbers_the_drawable_it_comes_from_writes() {
    let mut checked = 0;
    for path in fixtures() {
        let asset = vdtoolkit::convert(&source_of(&path)).unwrap();
        assert_same_numbers(
            &kotlin_numbers(&asset.to_kotlin("Icon", None)),
            &xml_numbers(&asset.to_xml()),
            &path.display().to_string(),
        );
        checked += 1;
    }
    assert!(checked >= 7, "only {checked} fixtures were compared");
}

#[test]
fn compose_writes_a_large_drawing_as_path_data_strings() {
    // Kotlin inlines `path { }`, so every command becomes bytecode of the one
    // getter, and a getter of over 64 KB does not compile. Measured with
    // Kotlin 2.4.10 against Compose ui 1.7.6, 2,500 cubic commands compile and
    // 3,000 fail, so a drawing of this size has to be written another way.
    let asset = vdtoolkit::convert(&large_svg(1_500)).unwrap();
    let kotlin = asset.to_kotlin("Large", None);
    assert!(kotlin.contains("addPath(\n"), "{kotlin}");
    assert!(kotlin.contains("pathData = addPathNodes(\""), "{kotlin}");
    assert!(!kotlin.contains("\" +\n"), "{kotlin}");
    assert!(!kotlin.contains("curveTo("), "{kotlin}");
    assert!(
        kotlin.contains("import androidx.compose.ui.graphics.vector.addPathNodes\n"),
        "{kotlin}"
    );
    let mut drawable = asset.clone();
    drawable.optimize();
    assert_same_numbers(
        &kotlin_numbers_of_strings(&kotlin),
        &xml_numbers(&drawable.to_xml()),
        "the path data string",
    );
}

#[test]
fn compose_concatenates_path_data_longer_than_a_string_constant() {
    // A JVM string constant holds at most 64 KB, which about 3,000 commands of
    // this shape reach, so longer path data is several literals concatenated.
    let asset = vdtoolkit::convert(&large_svg(6_000)).unwrap();
    let kotlin = asset.to_kotlin("Huge", None);
    assert!(kotlin.contains("\" +\n"), "no concatenation in\n{kotlin}");
    let mut drawable = asset.clone();
    drawable.optimize();
    assert_same_numbers(
        &kotlin_numbers_of_strings(&kotlin),
        &xml_numbers(&drawable.to_xml()),
        "the concatenated path data",
    );
}

#[test]
fn compose_names_files_and_properties_in_pascal_case() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icons");
    let output = temp.path().join("kotlin");
    fs::create_dir_all(&input).unwrap();
    for name in ["arrow_left.svg", "chevron-down.svg", "24px Café.svg"] {
        fs::write(input.join(name), sample_svg()).unwrap();
    }

    let result = cli()
        .args([
            "compose",
            input.to_str().unwrap(),
            "--package",
            "com.example.icons",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(result.status.success(), "{stderr}");

    for (file, property) in [
        ("ArrowLeft.kt", "ArrowLeft"),
        ("ChevronDown.kt", "ChevronDown"),
        ("Ic24pxCafe.kt", "Ic24pxCafe"),
    ] {
        let kotlin = fs::read_to_string(output.join(file)).unwrap_or_else(|error| {
            panic!("{file}: {error}; wrote {:?}", written(&output));
        });
        assert!(kotlin.contains(&format!("public val {property}: ImageVector\n")));
        assert!(kotlin.starts_with("package com.example.icons\n"));
    }
    // A stem that is already a valid Kotlin identifier is only recased, so
    // there is nothing to report about the name; a stem that is not says so.
    assert!(
        !stderr.contains("arrow_left"),
        "a valid identifier was reported as renamed: {stderr}"
    );
    for reported in ["chevron-down", "24px Café"] {
        assert!(
            stderr.contains(&format!("because {reported:?} is not a valid Kotlin name")),
            "{reported} was not reported: {stderr}"
        );
    }
}

fn written(directory: &Path) -> Vec<String> {
    fs::read_dir(directory)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn compose_names_stdout_after_the_svg_unless_told_otherwise() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("my-icon.svg");
    fs::write(&input, sample_svg()).unwrap();

    let default = cli()
        .args(["compose", input.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(default.status.success());
    let stdout = String::from_utf8(default.stdout).unwrap();
    assert!(
        stdout.contains("public val MyIcon: ImageVector\n"),
        "{stdout}"
    );
    assert!(stdout.ends_with("private var _MyIcon: ImageVector? = null\n"));

    // `--name` renames the property and, with no file to write, nothing else.
    let named = cli()
        .args(["compose", input.to_str().unwrap(), "--name", "Other"])
        .output()
        .unwrap();
    assert!(
        String::from_utf8(named.stdout)
            .unwrap()
            .contains("public val Other: ImageVector\n")
    );
}

#[test]
fn compose_rejects_a_name_that_cannot_be_a_kotlin_property() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icon.svg");
    let directory = temp.path().join("icons");
    fs::create_dir_all(&directory).unwrap();
    fs::write(directory.join("icon.svg"), sample_svg()).unwrap();
    fs::write(&input, sample_svg()).unwrap();

    for name in ["class", "9lives", "arrow-left"] {
        let result = cli()
            .args(["compose", input.to_str().unwrap(), "--name", name])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1), "{name}");
        let stderr = String::from_utf8(result.stderr).unwrap();
        assert!(
            stderr.contains("must match [A-Za-z_][A-Za-z0-9_]*"),
            "{name}: {stderr}"
        );
    }

    // One `--name` cannot name a directory of vectors.
    let result = cli()
        .args([
            "compose",
            directory.to_str().unwrap(),
            "--name",
            "Icon",
            "-o",
            temp.path().join("out").to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("--name names the one declaration a file holds")
    );
}

#[test]
fn compose_rejects_a_package_that_is_not_kotlin_identifiers() {
    for package in ["com.example.object", "com.ex ample", ""] {
        let result = cli()
            .args([
                "compose",
                fixture("even_odd.svg").to_str().unwrap(),
                "--package",
                package,
            ])
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1), "{package:?}");
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("--package must be dot-separated Kotlin identifiers"),
            "{package:?}"
        );
    }
}

#[test]
fn compose_rejects_two_svgs_that_share_a_kotlin_name() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icons");
    let output = temp.path().join("kotlin");
    for directory in ["a", "b"] {
        fs::create_dir_all(input.join(directory)).unwrap();
        fs::write(input.join(directory).join("home.svg"), sample_svg()).unwrap();
    }
    // Two properties with one name in one package is a duplicate declaration
    // wherever the files land, so the run fails before writing either.
    let result = cli()
        .args([
            "compose",
            input.to_str().unwrap(),
            "--package",
            "com.example",
            "-o",
            output.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("is already the Kotlin name of"),
    );
    assert!(!output.exists(), "a name collision left output behind");
}

fn sample_svg() -> &'static str {
    r##"<svg xmlns="http://www.w3.org/2000/svg" width="10" height="10"><path d="M0 0L10 10" fill="#123456"/></svg>"##
}

#[test]
fn compose_judges_an_svg_exactly_as_convert_does() {
    // Text, filters, and patterns are as unrepresentable in an ImageVector as
    // in a drawable: it has no text, no filter, and no pattern either.
    for (source, code) in [
        (
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><text x="0" y="0">hi</text></svg>"##,
            DiagnosticCode::TextNotOutlined,
        ),
        (
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><filter id="f"><feGaussianBlur stdDeviation="2"/></filter><path d="M0 0h1v1z" filter="url(#f)"/></svg>"##,
            DiagnosticCode::UnsupportedFilter,
        ),
        (
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><pattern id="p" width="4" height="4"><path d="M0 0h4v4z"/></pattern><path d="M0 0h24v24z" fill="url(#p)"/></svg>"##,
            DiagnosticCode::UnsupportedPattern,
        ),
    ] {
        let codes = match vdtoolkit::convert(source.as_bytes()) {
            Err(vdtoolkit::Error::Incompatible(analysis)) => analysis
                .diagnostics
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            other => panic!("expected a rejection, got {other:?}"),
        };
        assert!(codes.contains(&code), "{source}");
    }

    // An even-odd fill needs API 24 as a drawable and nothing at all as an
    // ImageVector, which is why `minimum_api` does not describe this output.
    let source = source_of(&fixture("even_odd.svg"));
    let asset = vdtoolkit::convert(&source).unwrap();
    assert_eq!(asset.analysis.minimum_api, Some(24));
    assert_eq!(asset.analysis.compatibility, Compatibility::Exact);
    assert!(
        asset
            .to_kotlin("EvenOdd", None)
            .contains("pathFillType = PathFillType.EvenOdd")
    );
}

#[test]
fn compose_applies_size_and_optimize_as_convert_does() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50" viewBox="0 0 100 50">
        <path d="M1.234567 2.345678L90.987654 21.876543" fill="#123456"/>
    </svg>"##;
    let mut asset = vdtoolkit::convert(source).unwrap();
    asset.set_size(24.0).unwrap();
    let before = asset.to_kotlin("Icon", None);
    assert!(before.contains("defaultWidth = 24.dp,\n"));
    assert!(before.contains("defaultHeight = 12.dp,\n"));
    assert!(before.contains("viewportWidth = 100f,\n"));
    assert!(
        before.contains("moveTo(1.234567f, 2.345678f)\n"),
        "{before}"
    );

    asset.optimize();
    let after = asset.to_kotlin("Icon", None);
    assert!(after.contains("moveTo(1.235f, 2.346f)\n"), "{after}");
    assert!(after.len() < before.len());
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tools/android-renderer/fixtures")
        .join(name)
}

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/compose")
        .join(name)
}

fn fixtures() -> Vec<PathBuf> {
    let mut paths = vec![fixture_path("features.svg")];
    paths.extend(
        fs::read_dir(fixture("."))
            .into_iter()
            .flatten()
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "svg")),
    );
    paths.sort();
    paths
}

fn source_of(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
}

fn compose(source: &[u8], name: &str) -> String {
    vdtoolkit::convert(source).unwrap().to_kotlin(name, None)
}

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_vdt"))
}

/// An SVG of `count` cubic segments, as a traced outline is.
fn large_svg(count: usize) -> Vec<u8> {
    let segments = "c0.01 0.01 0.02 0.02 0.03 0.03 ".repeat(count);
    format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">\
         <path d=\"M0 0 {segments}\" fill=\"#123456\"/></svg>"
    )
    .into_bytes()
}

/// The letters and numbers of SVG path data, in order.
fn path_data_numbers(data: &str) -> Vec<String> {
    let mut numbers = Vec::new();
    for token in data.split([' ', ',']) {
        if token.is_empty() {
            continue;
        }
        let mut rest = token;
        if let Some(letter) = token.chars().next().filter(char::is_ascii_alphabetic) {
            numbers.push(letter.to_ascii_uppercase().to_string());
            rest = &token[letter.len_utf8()..];
        }
        if !rest.is_empty() {
            numbers.push(rest.to_owned());
        }
    }
    numbers
}

/// The same, from the `PathBuilder` calls of generated Kotlin, wherever the
/// drawing nests them.
fn kotlin_numbers(kotlin: &str) -> Vec<String> {
    let body = kotlin
        .split_once(".apply {\n")
        .expect("generated Kotlin applies a builder")
        .1
        .split_once("\n        }.build()")
        .expect("generated Kotlin builds the vector")
        .0;
    let mut numbers = Vec::new();
    let mut rest = body;
    while let Some((at, call)) = ["moveTo(", "lineTo(", "quadTo(", "curveTo(", "close()"]
        .into_iter()
        .filter_map(|call| rest.find(call).map(|at| (at, call)))
        .min_by_key(|(at, _)| *at)
    {
        rest = &rest[at + call.len()..];
        if call == "close()" {
            numbers.push("Z".to_owned());
            continue;
        }
        numbers.push(
            match call {
                "moveTo(" => "M",
                "lineTo(" => "L",
                "quadTo(" => "Q",
                _ => "C",
            }
            .to_owned(),
        );
        let (arguments, after) = rest
            .split_once(')')
            .unwrap_or_else(|| panic!("unbalanced call at {rest:?}"));
        for argument in arguments.split(", ") {
            numbers.push(argument.trim_end_matches('f').to_owned());
        }
        rest = after;
    }
    numbers
}

/// The same, from the path data strings of Kotlin written as `addPathNodes`
/// literals, which may be several literals concatenated.
fn kotlin_numbers_of_strings(kotlin: &str) -> Vec<String> {
    let mut numbers = Vec::new();
    let mut rest = kotlin;
    while let Some(at) = rest.find("addPathNodes(") {
        rest = &rest[at + "addPathNodes(".len()..];
        let (expression, after) = rest
            .split_once(')')
            .unwrap_or_else(|| panic!("unbalanced addPathNodes at {rest:?}"));
        let data = expression.split('"').skip(1).step_by(2).collect::<String>();
        numbers.extend(path_data_numbers(&data));
        rest = after;
    }
    numbers
}

/// The same, from every `android:pathData` a drawable carries.
fn xml_numbers(xml: &str) -> Vec<String> {
    xml.split("android:pathData=\"")
        .skip(1)
        .flat_map(|attribute| path_data_numbers(attribute.split('"').next().unwrap()))
        .collect()
}

/// Compare two flattened drawings, reporting the first difference rather than
/// both of them.
fn assert_same_numbers(kotlin: &[String], drawable: &[String], what: &str) {
    let tokens = kotlin.len().max(drawable.len());
    for (index, (left, right)) in kotlin.iter().zip(drawable).enumerate() {
        assert_eq!(left, right, "{what}: token {index} of {tokens} differs");
    }
    assert_eq!(
        kotlin.len(),
        drawable.len(),
        "{what}: {} tokens in the Kotlin, {} in the drawable",
        kotlin.len(),
        drawable.len()
    );
}
