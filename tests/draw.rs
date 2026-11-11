//! The Compose draw target: the Kotlin `vdt compose --draw` writes, the
//! features only it can express, and the pixels those features draw.
//!
//! One drawing that uses every feature this target writes is checked in, so a
//! change to the shape of the output is a reviewable diff. The lowering's own
//! rendering of that drawing is compared against the source SVG through
//! `resvg`, which is what says the artwork survives the lowering; the Kotlin's
//! own pixels are verified on a device by `tools/android-renderer`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use vdtoolkit::Compatibility;

/// A generated file of every feature, from `fixtures/draw/features.svg`.
const FEATURES: &str = include_str!("fixtures/draw/features.kt");

#[test]
fn draw_writes_a_painter_behind_a_composable_factory() {
    let asset = vdtoolkit::convert_draw(&triangle()).unwrap();
    let kotlin = asset.to_draw_kotlin("Triangle", Some("com.example.icons"));

    assert!(kotlin.starts_with("package com.example.icons\n\nimport "));
    for import in [
        "import androidx.compose.runtime.Composable\n",
        "import androidx.compose.runtime.remember\n",
        "import androidx.compose.ui.geometry.Offset\n",
        "import androidx.compose.ui.geometry.Size\n",
        "import androidx.compose.ui.graphics.Color\n",
        "import androidx.compose.ui.graphics.Path\n",
        "import androidx.compose.ui.graphics.drawscope.DrawScope\n",
        "import androidx.compose.ui.graphics.drawscope.scale\n",
        "import androidx.compose.ui.graphics.painter.Painter\n",
        "import androidx.compose.ui.platform.LocalDensity\n",
        "import androidx.compose.ui.unit.Density\n",
        "import androidx.compose.ui.unit.dp\n",
    ] {
        assert!(kotlin.contains(import), "missing {import} in\n{kotlin}");
    }
    // `Icon(painter = …)` needs a `Painter`, and tinting needs the intrinsic
    // size the icon is laid out at, which only a density can turn into pixels.
    assert!(kotlin.contains("public fun rememberTriangle(): Painter {\n"));
    assert!(
        kotlin.contains(
            "private class TrianglePainter(private val density: Density) : Painter() {\n"
        )
    );
    assert!(kotlin.contains("    override val intrinsicSize: Size = with(density) {\n"));
    assert!(kotlin.contains("        Size(24.dp.toPx(), 24.dp.toPx())\n"));
    assert!(kotlin.contains("    override fun DrawScope.onDraw() {\n"));
    // The viewport is mapped onto whatever the layout gives the painter, as a
    // `VectorPainter` maps its own.
    assert!(
        kotlin.contains(
            "        scale(size.width / 24f, size.height / 24f, pivot = Offset.Zero) {\n"
        )
    );
    assert!(kotlin.contains("                path = Path().apply {\n"));
    assert!(kotlin.contains("                    moveTo(12f, 2f)\n"));
    assert!(kotlin.contains("                    close()\n"));
    assert!(kotlin.contains("                color = Color(0xFF3DDC84),\n"));

    // Nothing about the drawing needs a layer, and nothing may claim to.
    for absent in [
        "saveLayer",
        "drawIntoCanvas",
        "Paint()",
        "BlendMode",
        "ColorMatrix",
    ] {
        assert!(!kotlin.contains(absent), "{absent} should be left out");
    }
}

#[test]
fn draw_composites_group_opacity_as_one_layer() {
    // Folding the group's opacity into each shape would fade the two circles
    // apart and leave the overlap lighter than the SVG draws it.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <g opacity="0.5">
            <circle cx="8" cy="12" r="6" fill="#ff0000" fill-opacity="0.5"/>
            <circle cx="14" cy="12" r="6" fill="#ff0000" fill-opacity="0.5"/>
        </g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Overlap", None);
    for line in [
        "            drawIntoCanvas { canvas1 ->\n",
        "                canvas1.withSaveLayer(\n",
        "                    Rect(Offset.Zero, Size(24f, 24f)),\n",
        "                    Paint().apply {\n",
        "                        alpha = 0.5f\n",
        "                ) {\n",
        "                        alpha = 0.5f,\n",
    ] {
        assert!(kotlin.contains(line), "missing {line:?} in\n{kotlin}");
    }
    // The shapes keep their own alpha inside the layer, so the overlap darkens
    // to 0.5 x (1 - 0.5 x 0.5) instead of the 0.5 x 0.5 each shape would.
    assert_eq!(
        sample(source, 4.0, 12.0),
        [0xff, 0x00, 0x00, 0x40],
        "one circle"
    );
    assert_eq!(
        sample(source, 11.0, 12.0),
        [0xff, 0x00, 0x00, 0x60],
        "the overlap"
    );
}

#[test]
fn draw_writes_a_dashed_stroke_with_its_dash_array_and_offset() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path d="M2 12H22" fill="none" stroke="#0000ff" stroke-width="2"
            stroke-dasharray="4 2" stroke-dashoffset="1"/>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Dashes", None);
    assert!(
        kotlin.contains(
            "                    style = Stroke(\n                        width = 2f,\n                        pathEffect = PathEffect.dashPathEffect(floatArrayOf(4f, 2f), 1f),\n                    ),\n"
        ),
        "{kotlin}"
    );
    assert!(
        kotlin.contains("import androidx.compose.ui.graphics.PathEffect\n"),
        "{kotlin}"
    );
    // An odd number of lengths is repeated to an even count, as SVG specifies
    // and as a dash effect needs, so a single `4` is `4 4`.
    let single =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path d="M2 12H22" fill="none" stroke="#0000ff" stroke-width="2" stroke-dasharray="4"/>
    </svg>"##;
    let kotlin = vdtoolkit::convert_draw(single)
        .unwrap()
        .to_draw_kotlin("Dashes", None);
    assert!(
        kotlin.contains("pathEffect = PathEffect.dashPathEffect(floatArrayOf(4f, 4f)),\n"),
        "{kotlin}"
    );
    // The dash pattern is drawn, not just written: the stroke is painted where
    // the pattern is on and absent where it is off.
    assert_eq!(sample(source, 9.0, 12.0), [0x00, 0x00, 0xff, 0xff], "on");
    assert_eq!(sample(source, 6.0, 12.0)[3], 0x00, "off");
}

#[test]
fn draw_writes_a_blend_mode_on_the_layer_it_blends() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <rect x="2" y="6" width="20" height="12" fill="#3366cc"/>
        <g style="mix-blend-mode:multiply">
            <rect x="4" y="8" width="8" height="8" fill="#ffcc00"/>
        </g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Blended", None);
    for line in [
        "import androidx.compose.ui.graphics.BlendMode\n",
        // The layer's paint carries the mode, at the depth the layer's own
        // `Paint().apply { }` argument puts it: the isolating layer the drawing
        // opens first, then the layer that blends.
        "                                Paint().apply {\n                                    blendMode = BlendMode.Multiply\n",
    ] {
        assert!(kotlin.contains(line), "missing {line:?} in\n{kotlin}");
    }
    // The blend reads the drawing's own backdrop, so the whole drawing is
    // composited in a layer of its own: without it the group's layer would read
    // whatever the painter was drawn on, and the icon would darken the view
    // behind it.
    assert!(
        kotlin.contains(
            "                drawIntoCanvas { canvas1 ->\n                    canvas1.withSaveLayer(Rect(Offset.Zero, Size(24f, 24f)), Paint()) {\n"
        ),
        "{kotlin}"
    );
    // The group blends with the backdrop, so the rectangle multiplies the blue
    // under it rather than covering it.
    assert_eq!(sample(source, 8.0, 12.0), [0x33, 0x52, 0x00, 0xff]);
}

#[test]
fn draw_writes_clips_that_are_not_one_nonzero_path() {
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="36" height="12" viewBox="0 0 36 12">
        <defs>
            <clipPath id="hole"><path clip-rule="evenodd" d="M1 1H11V11H1Z M4 4H8V8H4Z"/></clipPath>
            <clipPath id="pair"><rect x="1" y="1" width="6" height="6"/><rect x="5" y="5" width="6" height="6"/></clipPath>
            <mask id="cut"><rect x="1" y="1" width="10" height="10" fill="#ffffff"/><circle cx="6" cy="6" r="2" fill="#000000"/></mask>
        </defs>
        <g transform="translate(0 0)" clip-path="url(#hole)"><rect width="12" height="12" fill="#00ffff"/></g>
        <g transform="translate(12 0)" clip-path="url(#pair)"><rect width="12" height="12" fill="#ff00ff"/></g>
        <g transform="translate(24 0)" mask="url(#cut)"><rect width="12" height="12" fill="#ffff00"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Clipped", None);
    for line in [
        // An even-odd clip keeps its hole by the winding rule of its path.
        "import androidx.compose.ui.graphics.PathFillType\n",
        "fillType = PathFillType.EvenOdd\n",
        // A clip path of several shapes is a union, which is `Path.op`.
        "} + Path().apply {\n",
        // A luminance mask with a black shape subtracts it.
        "import androidx.compose.ui.graphics.ClipOp\n",
        "clipOp = ClipOp.Difference,\n",
    ] {
        assert!(kotlin.contains(line), "missing {line:?} in\n{kotlin}");
    }
    // `Path.op` is what `+` calls, and the mask needs no layer of its own.
    assert!(!kotlin.contains("saveLayer"), "{kotlin}");
    // The even-odd clip keeps its hole, and the union clip covers the overlap
    // of the shapes it unions, and the mask that subtracts a shape leaves
    // nothing where the shape was.
    assert_eq!(
        sample(source, 2.0, 2.0),
        [0x00, 0xff, 0xff, 0xff],
        "outside the hole"
    );
    assert_eq!(sample(source, 6.0, 6.0)[3], 0x00, "the hole");
    assert_eq!(
        sample(source, 17.5, 5.5),
        [0xff, 0x00, 0xff, 0xff],
        "the overlap of the union"
    );
    assert_eq!(
        sample(source, 14.0, 3.0),
        [0xff, 0x00, 0xff, 0xff],
        "the first shape of the union alone"
    );
    assert_eq!(
        sample(source, 21.0, 9.0),
        [0xff, 0x00, 0xff, 0xff],
        "the second shape of the union alone"
    );
    assert_eq!(sample(source, 11.5, 6.0)[3], 0x00, "outside the union");
    assert_eq!(sample(source, 30.0, 6.0)[3], 0x00, "the subtracted shape");
    assert_eq!(
        sample(source, 26.0, 2.0),
        [0xff, 0xff, 0x00, 0xff],
        "the mask"
    );
}

#[test]
fn draw_writes_alpha_and_luminance_masks_as_a_dst_in_layer() {
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs>
            <mask id="alpha" mask-type="alpha">
                <rect x="0" y="0" width="12" height="24" fill="#ffffff"/>
                <circle cx="6" cy="12" r="3" fill="#000000"/>
            </mask>
            <mask id="fade"><rect x="12" y="0" width="12" height="24" fill="#808080"/></mask>
        </defs>
        <g mask="url(#alpha)"><rect width="24" height="24" fill="#00ff00"/></g>
        <g mask="url(#fade)"><rect width="24" height="24" fill="#0000ff"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Masked", None);
    for line in [
        "import androidx.compose.ui.graphics.BlendMode\n",
        // The mask composites into the layer the group drew into, so its paint
        // sits at the depth of that layer's own argument.
        "                            Paint().apply {\n                                blendMode = BlendMode.DstIn\n",
        "import androidx.compose.ui.graphics.ColorFilter\n",
        "import androidx.compose.ui.graphics.ColorMatrix\n",
        "                                colorFilter = luminanceToAlpha\n",
        "private val luminanceToAlpha = ColorFilter.colorMatrix(\n",
    ] {
        assert!(kotlin.contains(line), "missing {line:?} in\n{kotlin}");
    }
    // An alpha mask reads alpha, so an opaque black shape keeps what it covers.
    assert_eq!(
        sample(source, 6.0, 12.0),
        [0x00, 0xff, 0x00, 0xff],
        "black keeps"
    );
    // A luminance mask reads luminance, so the grey half fades what it covers.
    assert_eq!(sample(source, 18.0, 12.0), [0x00, 0x00, 0xff, 0x80]);
}

#[test]
fn draw_matches_the_checked_in_file_for_a_drawing_with_every_feature() {
    let asset = vdtoolkit::convert_draw(&source_of(&fixture("features.svg"))).unwrap();
    assert_eq!(
        asset.to_draw_kotlin("Features", Some("com.example.icons")),
        FEATURES
    );
}

#[test]
fn draw_lowers_the_drawable_subset_exactly_as_the_drawable_target_does() {
    // An SVG both targets accept must come out the same drawing, so that the
    // draw target is a way to keep more, never a way to draw the same SVG
    // differently.
    let mut checked = 0;
    for path in fixtures() {
        let source = source_of(&path);
        let Ok(exact) = vdtoolkit::convert(&source) else {
            continue;
        };
        let drawn = vdtoolkit::convert_draw(&source).unwrap();
        let (width, height) = render_size(&exact);
        assert_drawn_alike(
            &drawn.render_rgba(width, height).unwrap(),
            &exact.render_rgba(width, height).unwrap(),
            &path.display().to_string(),
        );
        checked += 1;
    }
    assert!(checked >= 8, "only {checked} fixtures were compared");
}

#[test]
fn draw_keeps_the_artwork_the_source_svg_draws() {
    // The one drawing that needs the draw target's own features is rasterized
    // and compared with the source SVG through `resvg`, which is what says the
    // artwork survives the lowering. What the Kotlin then draws is checked
    // against the same fixture on a device.
    let path = fixture("features.svg");
    let source = source_of(&path);
    let asset = vdtoolkit::convert_draw(&source).unwrap();
    let (width, height) = render_size(&asset);
    let drawn = asset.render_rgba(width, height).unwrap();
    let reference = render_reference(&source, width, height);
    assert_drawn_alike(&drawn, &reference, &path.display().to_string());
}

#[test]
fn draw_output_is_deterministic() {
    let source = source_of(&fixture("features.svg"));
    assert_eq!(
        vdtoolkit::convert_draw(&source)
            .unwrap()
            .to_draw_kotlin("Features", None),
        vdtoolkit::convert_draw(&source)
            .unwrap()
            .to_draw_kotlin("Features", None)
    );
}

#[test]
fn draw_writes_a_large_drawing_as_path_data_strings() {
    // `apply { }` is inlined, so every command lands in one method and a method
    // of over 64 KB does not compile; a drawing over the budget is written as
    // path data strings, which is one call per path.
    let asset = vdtoolkit::convert_draw(&large_svg(1_500)).unwrap();
    let kotlin = asset.to_draw_kotlin("Large", None);
    assert!(
        kotlin.contains("path = addPathNodes("),
        "no path data string in\n{kotlin}"
    );
    assert!(!kotlin.contains("cubicTo("), "{kotlin}");
    assert!(
        kotlin.contains("import androidx.compose.ui.graphics.vector.addPathNodes\n"),
        "{kotlin}"
    );
    assert!(
        kotlin.contains("import androidx.compose.ui.graphics.vector.toPath\n"),
        "{kotlin}"
    );
}

#[test]
fn draw_isolates_a_masked_group_whose_content_blends() {
    // SVG isolates a group that has a mask or a clip path: its content
    // composites into a layer of its own, so a blend mode inside it reads the
    // group's own backdrop and not the artwork the group is drawn over. A mask
    // of one opaque white shape lowers to a clip, which would leave the group
    // without that layer, and the multiply below would reach the blue rect
    // behind it and paint black.
    for (scope, definition) in [
        (
            "mask=\"url(#m)\"",
            "<mask id=\"m\"><rect width=\"24\" height=\"24\" fill=\"#ffffff\"/></mask>",
        ),
        (
            "clip-path=\"url(#c)\"",
            "<clipPath id=\"c\"><rect width=\"24\" height=\"24\"/></clipPath>",
        ),
    ] {
        let source = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
                <defs>{definition}</defs>
                <rect width="24" height="24" fill="#0000ff"/>
                <g {scope}>
                    <rect width="24" height="24" fill="#ffff00" style="mix-blend-mode:multiply"/>
                </g>
            </svg>"##
        )
        .into_bytes();
        let asset = vdtoolkit::convert_draw(&source).unwrap();
        let kotlin = asset.to_draw_kotlin("Isolated", None);
        // The blue backdrop is drawn before the group's layer opens, and the
        // group's clip and its multiply are both inside that layer.
        let layer = kotlin.find(".withSaveLayer(").expect("the group's layer");
        assert_eq!(
            kotlin.matches(".withSaveLayer(").count(),
            2,
            "the drawing needs the group's layer and the blend's, no more\n{kotlin}"
        );
        assert!(
            kotlin.find("Color(0xFF0000FF)").unwrap() < layer,
            "the group's layer must not hold the backdrop it blends against\n{kotlin}"
        );
        let clip = kotlin.find("clipPath(\n").expect("the group's clip");
        let multiply = kotlin
            .find("blendMode = BlendMode.Multiply")
            .expect("the group's blend");
        assert!(
            clip < layer && layer < multiply,
            "the group's layer must scope its blend and sit inside its clip\n{kotlin}"
        );
        // Yellow over blue, multiplied, is black; the same multiply against the
        // group's own empty backdrop leaves the yellow.
        assert_eq!(sample(&source, 12.0, 12.0), [0xff, 0xff, 0x00, 0xff]);
    }
}

#[test]
fn draw_applies_a_luminance_masks_alpha_through_a_grey_nested_mask() {
    // An opaque #808080 shape inside a luminance mask masks by half, so the
    // group it scopes is half transparent even though every shape in it is
    // opaque. The outer mask is a luminance mask of that group, so its own
    // alpha has to be applied as well: two layers for it, and the inner mask's
    // two again in the second pass that re-draws the content.
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs>
            <mask id="inner" mask-type="luminance"><rect width="24" height="24" fill="#808080"/></mask>
            <mask id="outer" mask-type="luminance">
                <g mask="url(#inner)"><rect width="24" height="24" fill="#ffffff"/></g>
            </mask>
        </defs>
        <g mask="url(#outer)"><rect width="24" height="24" fill="#ff0000"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Nested", None);
    assert_eq!(
        kotlin.matches("blendMode = BlendMode.DstIn").count(),
        6,
        "the outer mask's alpha layer is missing\n{kotlin}"
    );
    // The luminance is read once per pass that reads a colour, and the inner
    // mask is drawn in both of the outer mask's passes.
    assert_eq!(
        kotlin.matches("colorFilter = luminanceToAlpha").count(),
        3,
        "{kotlin}"
    );
    // The lowering masks by half, which is what the Kotlin has to draw.
    assert_eq!(sample(source, 12.0, 12.0), [0xff, 0x00, 0x00, 0x80]);
}

#[test]
fn draw_applies_a_luminance_masks_alpha_where_its_content_is_opaque() {
    // A luminance mask is its content's luminance times its content's alpha.
    // Content with no transparent paint is still partly transparent along the
    // anti-aliased edges of its shapes, so the second layer that applies that
    // alpha is written whether or not anything in the content fades: without it
    // the edge of an opaque shape would be drawn fully opaque, where the SVG
    // draws it faded by its coverage.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs>
            <mask id="m"><rect x="1" y="1" width="10" height="10" fill="#808080"/></mask>
        </defs>
        <g mask="url(#m)"><rect width="24" height="24" fill="#0000ff"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Opaque", None);
    assert_eq!(
        kotlin.matches("blendMode = BlendMode.DstIn").count(),
        2,
        "the luminance layer and the alpha layer\n{kotlin}"
    );
    assert_eq!(
        kotlin.matches("colorFilter = luminanceToAlpha").count(),
        1,
        "{kotlin}"
    );
    // The grey masks by half where it paints.
    assert_eq!(sample(source, 6.0, 6.0), [0x00, 0x00, 0xff, 0x80]);
}

#[test]
fn draw_counts_a_mask_the_number_of_times_it_is_drawn() {
    // A luminance mask whose content can be partly transparent is drawn twice,
    // and a mask inside that content doubles it again, so a path inside three
    // nested masks lands in `onDraw` eight times. The budget that decides
    // whether the drawing is written as builder calls has to count the emitted
    // copies: 800 commands is under the budget once and over it eight times.
    let segments = "c0.01 0.01 0.02 0.02 0.03 0.03 ".repeat(800);
    let source = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
            <defs>
                <mask id="m3"><path d="M0 0 {segments}" fill="#ffffff" opacity="0.5"/></mask>
                <mask id="m2"><g mask="url(#m3)"><rect width="24" height="24" fill="#ffffff"/></g></mask>
                <mask id="m1"><g mask="url(#m2)"><rect width="24" height="24" fill="#ffffff"/></g></mask>
            </defs>
            <g mask="url(#m1)"><rect width="24" height="24" fill="#0000ff"/></g>
        </svg>"##
    )
    .into_bytes();
    let asset = vdtoolkit::convert_draw(&source).unwrap();
    let kotlin = asset.to_draw_kotlin("Nested", None);
    assert!(
        kotlin.contains("path = addPathNodes("),
        "the mask's content is drawn eight times and has to be written as path data\n{kotlin}"
    );
}

#[test]
fn draw_reports_the_api_level_note_it_needs() {
    // Only the draw target's output needs an API level of its own, so only
    // `compose --draw` reports the note for it. The `ImageVector` target's asset
    // carries the drawable's level, which its Kotlin does not need, and it
    // prints no note at all.
    let multiply =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <g style="mix-blend-mode:multiply"><rect width="12" height="12" fill="#ffcc00"/></g>
    </svg>"##;
    let gradient =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs><linearGradient id="g"><stop offset="0" stop-color="#ff0000"/><stop offset="1" stop-color="#0000ff"/></linearGradient></defs>
        <rect width="24" height="24" fill="url(#g)"/>
    </svg>"##;
    let temp = tempfile::tempdir().unwrap();
    let blend = temp.path().join("blend.svg");
    let fade = temp.path().join("fade.svg");
    fs::write(&blend, multiply).unwrap();
    fs::write(&fade, gradient).unwrap();
    let run = |args: &[&str], input: &Path| {
        let output = cli()
            .args(args)
            .arg(input)
            .arg("--name")
            .arg("Icon")
            .output()
            .unwrap();
        String::from_utf8(output.stderr).unwrap()
    };
    let stderr = run(&["compose", "--draw"], &blend);
    assert!(
        stderr.contains("VDT004  needs API 29 for mix-blend-mode"),
        "{stderr}"
    );
    // A gradient needs no API level as drawn Kotlin, and the note a drawable
    // would carry is not printed for the `ImageVector` target.
    assert!(!run(&["compose", "--draw"], &fade).contains("VDT004"));
    assert!(!run(&["compose"], &fade).contains("VDT004"));
}

#[test]
fn draw_rejects_a_stroke_before_its_fill_in_its_own_words() {
    // A paint order that puts the stroke under the fill is rejected by both
    // targets, but not for the same reason: a drawable and an `ImageVector`
    // cannot hold a paint order at all, where the draw target draws every fill
    // before its stroke.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path d="M4 12H20" fill="#ff0000" stroke="#0000ff" stroke-width="4" paint-order="stroke"/>
    </svg>"##;
    let messages = |result: Result<vdtoolkit::Asset, vdtoolkit::Error>| match result {
        Err(vdtoolkit::Error::Incompatible(analysis)) => analysis
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.clone())
            .collect::<Vec<_>>(),
        other => panic!("expected a rejection, got {other:?}"),
    };
    let drawn = messages(vdtoolkit::convert_draw(source));
    assert!(
        drawn
            .iter()
            .any(|message| message.contains("not supported by the draw target")),
        "{drawn:?}"
    );
    let drawable = messages(vdtoolkit::convert(source));
    assert!(
        drawable
            .iter()
            .any(|message| message.contains("cannot be represented by a vector drawable")),
        "{drawable:?}"
    );
}

#[test]
fn draw_draws_nothing_for_a_masked_group_with_no_content() {
    // A mask scopes its own element, so a mask on an empty group has nothing to
    // take away and paints nothing: no layer, and the mask's own geometry is
    // left out rather than drawn for nothing.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs><mask id="m"><rect width="24" height="24" fill="#ffffff"/></mask></defs>
        <rect width="24" height="24" fill="#0000ff"/>
        <g mask="url(#m)"></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Empty", None);
    for absent in ["clipPath", "withSaveLayer", "DstIn"] {
        assert!(!kotlin.contains(absent), "{absent} should be left out");
    }
    assert_eq!(kotlin.matches("drawPath(").count(), 1, "{kotlin}");
    assert_eq!(sample(source, 12.0, 12.0), [0x00, 0x00, 0xff, 0xff]);
}

#[test]
fn draw_rejects_a_drawing_whose_masks_expand_it_without_bound() {
    // A luminance mask is written twice, so each nested mask doubles the content
    // it holds: sixty of them around one path would emit a billion copies of it.
    // The drawing is rejected instead, which is what keeps both the count and
    // the file it would write bounded, and it is rejected without writing any of
    // it.
    let masks: String = (1..=60)
        .map(|level| match level {
            1 => "<mask id=\"m1\"><rect width=\"24\" height=\"24\" fill=\"#808080\"/></mask>".to_owned(),
            _ => format!(
                "<mask id=\"m{level}\"><g mask=\"url(#m{})\"><rect width=\"24\" height=\"24\" fill=\"#808080\"/></g></mask>",
                level - 1
            ),
        })
        .collect();
    let source = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">\
         <defs>{masks}</defs>\
         <g mask=\"url(#m60)\"><rect width=\"24\" height=\"24\" fill=\"#ff0000\"/></g></svg>"
    )
    .into_bytes();
    let analysis = match vdtoolkit::convert_draw(&source) {
        Err(vdtoolkit::Error::Incompatible(analysis)) => analysis,
        other => panic!("expected a rejection, got {other:?}"),
    };
    assert!(
        analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.message.contains("expands to more path commands")),
        "{:?}",
        analysis.diagnostics
    );
}

#[test]
fn draw_rejects_a_drawing_that_fills_one_method() {
    // Every draw call, clip, and layer a drawing writes lands in the one method
    // the painter draws in, which the JVM refuses past 64 KB, so the drawing is
    // bounded by the bytecode that method compiles to rather than by its path
    // data. A group at half opacity is the most a statement costs: a layer is
    // `drawIntoCanvas`, a `withSaveLayer` whose block is inlined, the bounds and
    // the paint, and the path inside it is one more draw call. 106 such groups
    // sit on the limit of 60,000 bytes and 107 is past it. Both numbers are
    // measured: 106 groups compile with the harness in `tools/android-renderer`
    // and 300 do not, and the largest drawing in the repository costs 25,152
    // bytes.
    let drawing = |groups: usize| {
        let body: String = (0..groups)
            .map(|index| {
                format!(
                    "<g opacity=\"0.5\"><rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"#123456\"/></g>",
                    index % 20,
                    (index / 20) % 20
                )
            })
            .collect();
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">{body}</svg>"
        )
        .into_bytes()
    };
    let accepted = vdtoolkit::convert_draw(&drawing(106)).unwrap();
    let kotlin = accepted.to_draw_kotlin("Full", None);
    assert_eq!(
        kotlin.matches("withSaveLayer(").count(),
        106,
        "one layer per group"
    );
    let analysis = match vdtoolkit::convert_draw(&drawing(107)) {
        Err(vdtoolkit::Error::Incompatible(analysis)) => analysis,
        other => panic!("expected a rejection, got {other:?}"),
    };
    assert!(
        analysis.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("writes more code into one method")),
        "{:?}",
        analysis.diagnostics
    );
}

#[test]
fn draw_bounds_a_drawing_by_the_bytecode_its_paints_write() {
    // What a statement costs is the bytecode it compiles to, not how much of the
    // file it takes: a gradient brush builds an array of boxed `Pair`s where a
    // solid `Color` is one constant, so a drawing of gradient-filled shapes
    // fills the method about three times sooner than one of plain fills. 108
    // rects each filled with a two-stop linear gradient sit on the limit and 109
    // is past it; 320 of them compile with the harness in
    // `tools/android-renderer` and 350 do not, where 1,250 solid-filled rects
    // are accepted and 2,500 of those do not compile. Counting draw calls alone
    // cannot see that difference, which is what this pins.
    let drawing = |rects: usize| {
        let defs: String = (0..rects)
            .map(|index| {
                format!(
                    "<linearGradient id=\"g{index}\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">\
                     <stop offset=\"0\" stop-color=\"#ff0000\"/><stop offset=\"1\" stop-color=\"#0000ff\"/>\
                     </linearGradient>"
                )
            })
            .collect();
        let body: String = (0..rects)
            .map(|index| {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"1\" height=\"1\" fill=\"url(#g{index})\"/>",
                    index % 20,
                    (index / 20) % 20
                )
            })
            .collect();
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">\
             <defs>{defs}</defs>{body}</svg>"
        )
        .into_bytes()
    };
    let accepted = vdtoolkit::convert_draw(&drawing(108)).unwrap();
    let kotlin = accepted.to_draw_kotlin("Gradients", None);
    assert_eq!(
        kotlin.matches("Brush.linearGradient(").count(),
        108,
        "one brush per rect"
    );
    let analysis = match vdtoolkit::convert_draw(&drawing(109)) {
        Err(vdtoolkit::Error::Incompatible(analysis)) => analysis,
        other => panic!("expected a rejection, got {other:?}"),
    };
    assert!(
        analysis.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("writes more code into one method")),
        "{:?}",
        analysis.diagnostics
    );
}

#[test]
fn draw_bounds_a_drawing_by_the_stops_its_brushes_write() {
    // A stop is written as `0f to Color(0x…)`, which the compiler turns into an
    // index, a boxed offset, a boxed `Color`, a `Pair`, and an array store:
    // about 24 bytes each, and a gradient of many stops is a brush whose fixed
    // cost no longer dominates. Four rects each filled with a 300-stop gradient
    // sit on the limit and five is past it; four of them compile with the
    // harness in `tools/android-renderer` and nine do not, which is the case
    // this weight was found on. A weight derived from a two-stop gradient would
    // have accepted nine.
    let drawing = |rects: usize| {
        let defs: String = (0..rects)
            .map(|index| {
                let stops: String = (0..300)
                    .map(|stop| {
                        format!(
                            "<stop offset=\"{:.5}\" stop-color=\"#{:02x}{:02x}{:02x}\"/>",
                            stop as f32 / 299.0,
                            (index * 13 + stop * 7) % 256,
                            (index * 5 + stop * 11) % 256,
                            (stop * 3 + index) % 256
                        )
                    })
                    .collect();
                format!(
                    "<linearGradient id=\"g{index}\" x1=\"0\" y1=\"0\" x2=\"1\" y2=\"1\">{stops}</linearGradient>"
                )
            })
            .collect();
        let body: String = (0..rects)
            .map(|index| {
                format!(
                    "<rect x=\"{}\" y=\"{}\" width=\"5\" height=\"5\" fill=\"url(#g{index})\"/>",
                    (index % 40) * 6,
                    (index / 40) * 6
                )
            })
            .collect();
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"240\" height=\"6\" viewBox=\"0 0 240 6\">\
             <defs>{defs}</defs>{body}</svg>"
        )
        .into_bytes()
    };
    let accepted = vdtoolkit::convert_draw(&drawing(4)).unwrap();
    let kotlin = accepted.to_draw_kotlin("Stops", None);
    assert_eq!(
        kotlin.matches(" to Color(").count(),
        // Every stop of every brush, written once each.
        4 * 300,
        "one stop per gradient stop"
    );
    let analysis = match vdtoolkit::convert_draw(&drawing(5)) {
        Err(vdtoolkit::Error::Incompatible(analysis)) => analysis,
        other => panic!("expected a rejection, got {other:?}"),
    };
    assert!(
        analysis.diagnostics.iter().any(|diagnostic| diagnostic
            .message
            .contains("writes more code into one method")),
        "{:?}",
        analysis.diagnostics
    );
}

#[test]
fn draw_leaves_mask_geometry_out_of_the_content_bounds() {
    // Artwork and a grey mask both at local x 0-10 inside translate(40 0). The
    // mask's content is read back as the mask rather than painted, so the bounds
    // are the artwork's at 40-50, and not pulled back to 0 by the mask's own
    // untransformed bounding box.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="60" height="24" viewBox="0 0 60 24">
        <defs><mask id="m"><rect x="0" y="0" width="10" height="10" fill="#808080"/></mask></defs>
        <g transform="translate(40 0)" mask="url(#m)">
            <rect x="0" y="0" width="10" height="10" fill="#0000ff"/>
        </g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let bounds = asset.analysis.metrics.content_bounds.unwrap();
    assert_eq!((bounds.left, bounds.right), (40.0, 50.0));
}

#[test]
fn draw_counts_a_masks_geometry_once() {
    // A mask drawn as a layer is lowered through the visitor that counts paths
    // and commands, so the metrics count its content once, and only the region
    // the mask is confined to is counted beside it: two paths (the group's and
    // the mask's), and five commands for each of them plus five for the region.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs><mask id="m"><rect width="24" height="24" fill="#808080"/></mask></defs>
        <g mask="url(#m)"><rect width="24" height="24" fill="#0000ff"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let metrics = &asset.analysis.metrics;
    assert_eq!(
        (metrics.paths, metrics.path_commands),
        (2, 15),
        "the mask's own geometry is counted twice"
    );
}

#[test]
fn draw_nests_a_clip_path_the_way_the_reference_draws_it() {
    // A clip path's own `clip-path` narrows it, and that nested clip is
    // expressed where the outer clip path is used rather than in the outer clip
    // path's own transformed space: the transform a clip path carries does not
    // reach the clip it nests. That is what `resvg` does, so the outer clip's
    // rect moves by the translate and the inner one does not, and the drawing
    // is compared against `resvg` rather than described.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs>
            <clipPath id="outer" transform="translate(4 0)" clip-path="url(#inner)">
                <rect x="0" y="0" width="12" height="12"/>
            </clipPath>
            <clipPath id="inner"><rect x="1" y="1" width="6" height="6"/></clipPath>
        </defs>
        <g clip-path="url(#outer)"><rect width="24" height="24" fill="#C52A54"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let (width, height) = (240, 240);
    assert_drawn_alike(
        &asset.render_rgba(width, height).unwrap(),
        &render_reference(source, width, height),
        "a clip path that nests another through its own clip-path",
    );
}

#[test]
fn draw_writes_a_split_path_data_string_as_an_argument_list() {
    // Path data over the JVM's 64 KB string constant is written as several
    // literals joined by `+` into the one argument of `addPathNodes`, whose
    // argument list keeps a trailing comma: Kotlin allows that there where it
    // rejects one inside a parenthesized expression, which `addPathNodes((…))`
    // would be.
    let asset = vdtoolkit::convert_draw(&large_svg(5_000)).unwrap();
    let kotlin = asset.to_draw_kotlin("Large", None);
    assert!(
        kotlin.contains("                    path = addPathNodes(\n"),
        "no split path data string in\n{kotlin}"
    );
    assert!(!kotlin.contains("addPathNodes(("), "{kotlin}");
    // More than one literal, joined by `+`, and the last one ends the argument
    // list with its own comma.
    assert!(kotlin.contains(" +\n"), "{kotlin}");
    assert!(
        kotlin.contains("\",\n                    ).toPath(),\n"),
        "{kotlin}"
    );
}

#[test]
fn draw_scales_a_dashed_stroke_with_its_transform() {
    // The geometry is written in output coordinates and the stroke width is
    // scaled to match, so the dash pattern, which is in the path's own units,
    // has to be scaled with them: under `scale(2)`, `4 2` at offset 1 is 8, 4
    // at offset 2.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <g transform="scale(2)">
            <path d="M1 1H5" fill="none" stroke="#0000ff" stroke-width="1"
                stroke-dasharray="4 2" stroke-dashoffset="1"/>
        </g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Scaled", None);
    for line in [
        "                        width = 2f,\n",
        "                        pathEffect = PathEffect.dashPathEffect(floatArrayOf(8f, 4f), 2f),\n",
    ] {
        assert!(kotlin.contains(line), "missing {line:?} in\n{kotlin}");
    }
}

#[test]
fn draw_keeps_an_alpha_mask_of_a_gradient_as_a_layer() {
    // An alpha mask keeps what its content covers only while the content is
    // opaque: a gradient that fades through its stops' opacity has values
    // between transparent and opaque, which a clip path cannot express.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs>
            <linearGradient id="fade">
                <stop offset="0" stop-color="#ffffff"/>
                <stop offset="1" stop-color="#ffffff" stop-opacity="0"/>
            </linearGradient>
            <mask id="m" mask-type="alpha"><rect width="24" height="24" fill="url(#fade)"/></mask>
        </defs>
        <g mask="url(#m)"><rect width="24" height="24" fill="#0000ff"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Faded", None);
    assert!(kotlin.contains("blendMode = BlendMode.DstIn"), "{kotlin}");
    // An alpha mask reads the alpha channel, so one layer is the whole mask and
    // nothing converts a colour into alpha.
    assert_eq!(
        kotlin.matches("blendMode = BlendMode.DstIn").count(),
        1,
        "{kotlin}"
    );
    assert!(!kotlin.contains("luminanceToAlpha"), "{kotlin}");
}

#[test]
fn draw_applies_a_luminance_masks_alpha_through_a_nested_mask() {
    // The outer mask's content is a white rect whose own mask is half
    // transparent, so the outer mask is partly transparent too: its own alpha
    // has to be applied as well, which is two layers for it, two for the inner
    // mask, and the inner mask's two again in the second pass that re-draws the
    // content.
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs>
            <mask id="inner"><rect width="24" height="24" fill="#ffffff" opacity="0.5"/></mask>
            <mask id="outer"><g mask="url(#inner)"><rect width="24" height="24" fill="#ffffff"/></g></mask>
        </defs>
        <g mask="url(#outer)"><rect width="24" height="24" fill="#0000ff"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Nested", None);
    assert_eq!(
        kotlin.matches("blendMode = BlendMode.DstIn").count(),
        6,
        "the outer mask's alpha layer is missing\n{kotlin}"
    );
}

#[test]
fn draw_reports_the_api_level_a_blend_mode_needs() {
    // Compose draws a blend mode through `android.graphics.BlendMode` from API
    // 29 and falls back to the `PorterDuff` mode that matches it below, and
    // `PorterDuff` has no equivalent for most of them: only screen, overlay,
    // darken, and lighten, which therefore need no level of their own.
    let source = |mode: &str| {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
                <g style="mix-blend-mode:{mode}"><rect width="12" height="12" fill="#ffcc00"/></g>
            </svg>"##
        )
        .into_bytes()
    };
    for mode in ["multiply", "difference", "hue"] {
        let asset = vdtoolkit::convert_draw(&source(mode)).unwrap();
        assert_eq!(asset.analysis.minimum_api, Some(29), "{mode}");
        assert!(
            asset
                .analysis
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_str() == "VDT004"),
            "{mode}: {:?}",
            asset.analysis.diagnostics
        );
    }
    for mode in ["screen", "overlay", "darken", "lighten"] {
        let asset = vdtoolkit::convert_draw(&source(mode)).unwrap();
        assert_eq!(asset.analysis.minimum_api, None, "{mode}");
        assert!(
            asset
                .analysis
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code.as_str() != "VDT004"),
            "{mode}: {:?}",
            asset.analysis.diagnostics
        );
    }
}

#[test]
fn draw_applies_a_luminance_masks_own_alpha_as_well_as_its_luminance() {
    // A luminance mask is its content's luminance times its content's alpha.
    // The colour matrix that moves the luminance into the alpha channel
    // replaces the alpha the content had, so a second layer applies it: white
    // at half opacity masks by half rather than keeping everything.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <defs>
            <mask id="m"><rect width="24" height="24" fill="#ffffff" opacity="0.5"/></mask>
        </defs>
        <g mask="url(#m)"><rect width="24" height="24" fill="#0000ff"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Half", None);
    assert_eq!(
        kotlin.matches("blendMode = BlendMode.DstIn").count(),
        2,
        "{kotlin}"
    );
    assert_eq!(
        kotlin.matches("colorFilter = luminanceToAlpha").count(),
        1,
        "the luminance is applied once and the alpha once\n{kotlin}"
    );
    assert_eq!(sample(source, 12.0, 12.0), [0x00, 0x00, 0xff, 0x80]);
}

#[test]
fn draw_transforms_a_clip_inside_a_mask() {
    // A mask's content is in the user space of the element the mask is on, so
    // everything in it — a clip path on a group as much as the shapes it scopes
    // — is expressed there and moves with the element.
    let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="48" height="24" viewBox="0 0 48 24">
        <defs>
            <clipPath id="c"><rect x="1" y="1" width="10" height="10"/></clipPath>
            <mask id="m"><g clip-path="url(#c)"><rect width="12" height="12" fill="#ffffff"/></g></mask>
        </defs>
        <g transform="translate(36 12)" mask="url(#m)"><rect width="12" height="12" fill="#0000ff"/></g>
    </svg>"##;
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let kotlin = asset.to_draw_kotlin("Clipped", None);
    // The clip is the (1,1)-(11,11) rect translated by (36,12).
    assert!(
        kotlin.contains("                                    moveTo(37f, 13f)\n"),
        "the mask's clip path did not move with the element\n{kotlin}"
    );
    assert_eq!(sample(source, 42.0, 18.0), [0x00, 0x00, 0xff, 0xff]);
    assert_eq!(sample(source, 37.5, 12.5)[3], 0x00, "outside the clip");
}

#[test]
fn draw_judges_an_svg_as_the_drawable_target_does_until_it_can_draw_more() {
    // A dashed stroke, group opacity, a blend mode, a mask that is not one
    // opaque white shape, and a clip path that unions or clips by even-odd are
    // all rejected by `convert` and all drawn here.
    let source = source_of(&fixture("features.svg"));
    assert_eq!(
        vdtoolkit::convert(&source).unwrap_err().to_string(),
        "SVG is not exactly representable as VectorDrawable"
    );
    let asset = vdtoolkit::convert_draw(&source).unwrap();
    assert!(asset.analysis.compatibility <= Compatibility::ExactWithNormalization);
    // The drawing blends, so it needs API 29: Compose draws a blend mode through
    // `android.graphics.BlendMode` from API 29 and falls back to `PorterDuff`
    // below, which has no equivalent for multiply.
    assert_eq!(asset.analysis.minimum_api, Some(29));
    assert!(
        asset
            .analysis
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.as_str() == "VDT004"),
        "{:?}",
        asset.analysis.diagnostics
    );
}

#[test]
fn draw_names_files_and_functions_in_pascal_case() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icons");
    let output = temp.path().join("kotlin");
    fs::create_dir_all(&input).unwrap();
    fs::write(input.join("arrow_left.svg"), triangle()).unwrap();

    let result = cli()
        .args([
            "compose",
            "--draw",
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
    let kotlin = fs::read_to_string(output.join("ArrowLeft.kt")).unwrap();
    assert!(
        kotlin.contains("public fun rememberArrowLeft(): Painter {\n"),
        "{kotlin}"
    );
}

#[test]
fn draw_applies_size_and_optimize_as_the_other_targets_do() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icon.svg");
    fs::write(&input, triangle()).unwrap();

    let result = cli()
        .args([
            "compose",
            "--draw",
            input.to_str().unwrap(),
            "--size",
            "48",
            "--optimize",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8(result.stdout).unwrap();
    assert!(result.status.success(), "{stdout}");
    assert!(
        stdout.contains("Size(48.dp.toPx(), 48.dp.toPx())\n"),
        "{stdout}"
    );
    // The viewport is kept, so the drawing is unchanged apart from its size.
    assert!(stdout.contains("scale(size.width / 24f, size.height / 24f, pivot = Offset.Zero) {\n"));
}

#[test]
fn draw_optimize_keeps_a_group_layer_and_its_isolation() {
    // Rounding a group's opacity to one must not take the layer away, because
    // the layer is the group's own backdrop: the multiply inside it multiplies
    // an empty layer and the rect stays yellow, where without the layer it would
    // multiply the blue rect behind it and come out black.
    let source =
        br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24"><rect width="24" height="24" fill="#00f"/><g opacity="0.9999"><rect x="4" y="4" width="16" height="16" fill="#ff0" style="mix-blend-mode:multiply"/></g></svg>"##;
    let plain = vdtoolkit::convert_draw(source).unwrap();
    let mut optimized = vdtoolkit::convert_draw(source).unwrap();
    optimized.optimize();

    let plain_kotlin = plain.to_draw_kotlin("Isolated", None);
    let optimized_kotlin = optimized.to_draw_kotlin("Isolated", None);
    assert!(
        plain_kotlin.contains("alpha = 0.9999f"),
        "the group's opacity is written as it is:\n{plain_kotlin}"
    );
    assert_eq!(
        plain_kotlin.matches("withSaveLayer(").count(),
        optimized_kotlin.matches("withSaveLayer(").count(),
        "optimize must not take a layer away:\n{optimized_kotlin}"
    );
    // The number is still shortened, and the blend still has its own layer.
    assert!(!optimized_kotlin.contains("0.9999f"), "{optimized_kotlin}");
    assert!(
        optimized_kotlin.contains("blendMode = BlendMode.Multiply"),
        "{optimized_kotlin}"
    );

    let (width, height) = (240, 240);
    let plain_pixels = plain.render_rgba(width, height).unwrap();
    let optimized_pixels = optimized.render_rgba(width, height).unwrap();
    assert_eq!(
        plain_pixels, optimized_pixels,
        "optimize must not change the drawing"
    );
    // Inside the group's rect, where the multiply reads the group's own
    // backdrop and stays yellow rather than turning black against the blue.
    let at = ((120 * width + 120) * 4) as usize;
    assert_eq!(&plain_pixels[at..at + 4], &[0xff, 0xff, 0x00, 0xff]);
}

#[test]
fn draw_optimize_keeps_a_dash_the_rounding_cannot_express() {
    // Rounding must not change which intervals of the pattern are zero. A dash
    // whose only "on" interval rounds to zero draws nothing where the SVG draws
    // a faint dash, and one whose "off" interval does draws solid, so a pattern
    // with an interval the rounding cannot express keeps its values.
    let drawing = |dasharray: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">\
             <path d=\"M2 12H22\" stroke=\"#000000\" stroke-width=\"2\" stroke-dasharray=\"{dasharray}\"/></svg>"
        )
        .into_bytes()
    };
    let optimized = |dasharray: &str| {
        let mut asset = vdtoolkit::convert_draw(&drawing(dasharray)).unwrap();
        asset.optimize();
        asset
    };
    let kotlin = |dasharray: &str| optimized(dasharray).to_draw_kotlin("Dashed", None);

    // Both intervals round to nothing, which is not a dash at all: Skia drops
    // the effect and the renderer drops it too, so the stroke would draw solid.
    // The third is the one the sum alone does not catch: its total stays
    // positive, but its only "on" interval would round to zero.
    for (dasharray, written) in [
        ("0.0004 0.0004", "floatArrayOf(0.0004f, 0.0004f)"),
        ("0.0004 0.0006", "floatArrayOf(0.0004f, 0.0006f)"),
        ("0.0006 0.0004", "floatArrayOf(0.0006f, 0.0004f)"),
    ] {
        let kotlin = kotlin(dasharray);
        assert!(
            kotlin.contains(written),
            "the intervals of {dasharray} keep their values:\n{kotlin}"
        );
        let (width, height) = (240, 240);
        assert_eq!(
            vdtoolkit::convert_draw(&drawing(dasharray))
                .unwrap()
                .render_rgba(width, height)
                .unwrap(),
            optimized(dasharray).render_rgba(width, height).unwrap(),
            "optimize must not change the drawing of {dasharray}"
        );
    }

    // A dash every interval of which rounds to a positive number is shortened.
    let written = kotlin("1.0004 2.0004");
    assert!(
        written.contains("floatArrayOf(1f, 2f)"),
        "a dash that stays a dash is shortened:\n{written}"
    );
}

#[test]
fn draw_leaves_out_a_stroke_the_writer_would_print_as_zero() {
    // The writer prints every width below its six decimals as `0`, and Compose
    // draws a width of zero as a hairline, one pixel of the rasterized drawing,
    // where the SVG draws nothing at all. Such a stroke is left out, the way a
    // width of zero already is, so no drawing ever writes `Stroke(width = 0f)`.
    let drawing = |body: &str| {
        format!(
            "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"24\" height=\"24\" viewBox=\"0 0 24 24\">{body}</svg>"
        )
        .into_bytes()
    };
    let kotlin = |body: &str| {
        vdtoolkit::convert_draw(&drawing(body))
            .unwrap()
            .to_draw_kotlin("Thin", None)
    };

    let written =
        kotlin(r##"<path d="M2 12H22" fill="none" stroke="#000000" stroke-width="0.0000001"/>"##);
    assert!(!written.contains("Stroke("), "{written}");
    assert!(
        !written.contains("drawPath("),
        "a stroke is all this path has, so nothing is drawn:\n{written}"
    );

    // A filled path keeps its fill and loses only the stroke.
    let written = kotlin(
        r##"<path d="M2 2H22V22H2Z" fill="#123456" stroke="#ff0000" stroke-width="0.0000001"/>"##,
    );
    assert!(written.contains("color = Color(0xFF123456)"), "{written}");
    assert!(!written.contains("Stroke("), "{written}");

    // A width the writer can print is kept, however small it is.
    let written =
        kotlin(r##"<path d="M2 12H22" fill="none" stroke="#000000" stroke-width="0.000001"/>"##);
    assert!(written.contains("width = 0.000001f"), "{written}");
}

#[test]
fn draw_rejects_a_name_that_cannot_be_a_kotlin_declaration() {
    let temp = tempfile::tempdir().unwrap();
    let input = temp.path().join("icon.svg");
    fs::write(&input, triangle()).unwrap();
    let result = cli()
        .args([
            "compose",
            "--draw",
            input.to_str().unwrap(),
            "--name",
            "class",
        ])
        .output()
        .unwrap();
    assert!(!result.status.success());
    let stderr = String::from_utf8(result.stderr).unwrap();
    assert!(stderr.contains("--name must match"), "{stderr}");
}

/// The solid green triangle every simple test uses.
fn triangle() -> Vec<u8> {
    br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
        <path d="M12 2L2 22h20z" fill="#3DDC84"/>
    </svg>"##
        .to_vec()
}

/// One unpremultiplied RGBA pixel of the drawing the draw target lowers, at
/// `x`, `y` in viewport units, rendered at ten pixels per unit.
fn sample(source: &[u8], x: f32, y: f32) -> [u8; 4] {
    let asset = vdtoolkit::convert_draw(source).unwrap();
    let metrics = &asset.analysis.metrics;
    let width = (metrics.viewport_width * PIXELS_PER_UNIT).round() as u32;
    let height = (metrics.viewport_height * PIXELS_PER_UNIT).round() as u32;
    let pixels = asset.render_rgba(width, height).unwrap();
    let x = (x * PIXELS_PER_UNIT).round() as u32;
    let y = (y * PIXELS_PER_UNIT).round() as u32;
    let at = ((y * width + x) * 4) as usize;
    pixels[at..at + 4].try_into().unwrap()
}

/// Pixels per viewport unit every sample is taken at, which keeps a sample
/// well inside the shape it is meant to land in.
const PIXELS_PER_UNIT: f32 = 10.0;

/// Pixel size that puts the longer side on [`LONGEST_SIDE`].
fn render_size(asset: &vdtoolkit::Asset) -> (u32, u32) {
    let metrics = &asset.analysis.metrics;
    let scale = LONGEST_SIDE as f32 / metrics.viewport_width.max(metrics.viewport_height);
    let side = |value: f32| ((value * scale).round() as u32).max(1);
    (side(metrics.viewport_width), side(metrics.viewport_height))
}

/// Longest side in pixels of every comparison render.
const LONGEST_SIDE: u32 = 240;

/// Render the source SVG with `resvg` at the same size, as unpremultiplied
/// RGBA, scaling the whole viewBox onto the canvas the way the drawable's
/// viewport is.
fn render_reference(source: &[u8], width: u32, height: u32) -> Vec<u8> {
    let tree = usvg::Tree::from_data(source, &usvg::Options::default()).unwrap();
    let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
    let size = tree.size();
    resvg::render(
        &tree,
        tiny_skia::Transform::from_scale(
            width as f32 / size.width(),
            height as f32 / size.height(),
        ),
        &mut pixmap.as_mut(),
    );
    pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect()
}

/// Compare the drawing the target lowers with the source SVG.
///
/// Both are rasterized by `tiny_skia`, so the geometry and its anti-aliasing
/// agree to within a channel step wherever the lowering is exact; what this
/// catches is a construct the lowering drops or misplaces, which moves whole
/// regions of the canvas.
fn assert_drawn_alike(drawn: &[u8], reference: &[u8], what: &str) {
    assert_eq!(drawn.len(), reference.len(), "{what}: render sizes differ");
    let channels = drawn.len();
    let mut total: u64 = 0;
    let mut painted: u64 = 0;
    let mut painted_channels: u64 = 0;
    let mut far = 0u64;
    let mut worst = 0u8;
    for (left, right) in drawn.chunks_exact(4).zip(reference.chunks_exact(4)) {
        let difference = left
            .iter()
            .zip(right)
            .map(|(left, right)| left.abs_diff(*right))
            .max()
            .unwrap_or(0);
        total += u64::from(difference);
        if left[3] > 8 || right[3] > 8 {
            painted += u64::from(difference);
            painted_channels += 4;
        }
        if difference > 32 {
            far += 1;
        }
        worst = worst.max(difference);
    }
    let mean = total as f64 / channels as f64;
    let painted_mean = painted as f64 / painted_channels.max(1) as f64;
    let far_fraction = far as f64 / (channels / 4) as f64;
    assert!(
        mean < 1.0 && painted_mean < 2.0 && far_fraction < 0.002,
        "{what}: mean {mean:.4}, painted mean {painted_mean:.4}, \
         {far_fraction:.4} of pixels past 32, worst {worst}"
    );
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/draw")
        .join(name)
}

/// Every SVG in the repository that the draw target accepts.
fn fixtures() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut paths = vec![fixture("features.svg")];
    for directory in ["tools/android-renderer/fixtures", "tests/fixtures/compose"] {
        let directory = root.join(directory);
        paths.extend(
            fs::read_dir(&directory)
                .into_iter()
                .flatten()
                .flatten()
                .map(|entry| entry.path())
                .filter(|path| path.extension().is_some_and(|extension| extension == "svg")),
        );
    }
    paths.sort();
    paths
}

fn source_of(path: &Path) -> Vec<u8> {
    fs::read(path).unwrap()
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
