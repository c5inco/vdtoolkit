//! Kotlin source for a Compose `Painter` that draws the artwork's paths
//! directly, written from the same lowered model as the VectorDrawable writer.
//!
//! `compose` writes an `ImageVector`, which is as limited as a VectorDrawable
//! in every way that concerns artwork. This target is the one that lifts those
//! limits, because it does not go through a vector model at all: it draws with
//! `DrawScope`, which can composite a group into a layer of its own, dash a
//! stroke, blend a group, clip to any path, and mask with a layer.
//!
//! The output is a `Painter` subclass behind a `@Composable` factory, the
//! shape `rememberVectorPainter` has. `Painter` is the one of the three
//! candidates that keeps `Icon(painter = …)` and tinting working: `Icon` takes
//! a `Painter` and applies its `ColorFilter` to whatever painter it is given,
//! where a `Modifier.drawBehind` extension or a bare `DrawScope` function
//! would leave the caller to wire the tint through by hand. A `Painter` also
//! carries the intrinsic size the icon is laid out at, and is what
//! `rememberVectorPainter` hands to the same call, so swapping one target for
//! the other changes the type of the `val` and nothing about the call site.
//!
//! The factory is what makes the size right: a `Painter` reports its intrinsic
//! size in pixels, and the only density a non-composable declaration can see is
//! none, so the `24.dp` an `ImageVector` carries has to be converted where a
//! `LocalDensity` exists — exactly as `rememberVectorPainter` does it.
//!
//! A path is written as the `Path().apply { … }` it is, so the file reads as
//! the drawing, and a clip path of several shapes is written as
//! `Path + Path`, which is the `Path.op` call that unions them: filling every
//! contour of one path by the nonzero rule would cancel opposite windings
//! where they overlap rather than union them. The one thing named into a local
//! is a path that is both filled and stroked, because `drawPath` takes one
//! paint at a time and would otherwise write that path's geometry twice. A
//! drawing over [`INLINE_COMMAND_BUDGET`] commands is written as path data
//! strings instead, for the same reason the `ImageVector` target does it.
//!
//! One thing the output needs an API level for: Compose draws a blend mode
//! through `android.graphics.BlendMode` on API 29 and newer and falls back to
//! the `PorterDuff` mode that matches it below, which for every mode Android's
//! `PorterDuff` has no equivalent for — all of them except screen, overlay,
//! darken, and lighten — is `SrcOver`, so the group paints normally. That is
//! what [`crate::vector::draw_api_level`] reports.

use std::fmt::Write;
use std::ops::{Deref, DerefMut};

use crate::kotlin::{Body, Parameter, PathCommands, number, path_data_call, path_data_strings};
use crate::vector::{
    BlendMode, ClipOp, Dash, FillRule, LineCap, LineJoin, MaskKind, Paint, PathData, TileMode,
    VectorClip, VectorDrawable, VectorGroup, VectorMask, VectorNode, VectorPath,
    blends_with_its_backdrop,
};

/// The Kotlin name of a generated `Painter`.
pub struct Painter<'a> {
    /// Name the generated `remember` function and painter class are built
    /// from: `remember<name>` and `<name>Painter`. Must be a valid Kotlin
    /// identifier.
    pub name: &'a str,
    /// `package` declaration to write. Without it the declarations land in the
    /// default package, which no other package can import, so a caller that
    /// knows where the file belongs should pass one.
    pub package: Option<&'a str>,
}

/// The colour matrix that turns a luminance mask's colour into alpha.
///
/// SVG reads a luminance mask as `0.2126 R + 0.7152 G + 0.0722 B` times the
/// mask's own alpha. A `ColorMatrix` can only weight the channels into alpha,
/// so it replaces the content's alpha rather than scaling by it, and every
/// luminance mask gets a second `DstIn` layer to apply the alpha it replaced:
/// even content with no transparent paint is partly transparent along the
/// anti-aliased edges of its shapes, which is why [`Writer::mask`] writes two
/// layers for one.
const LUMINANCE_TO_ALPHA: &str = "\
/// Turns the colour of a luminance mask into the alpha that masks with it.
private val luminanceToAlpha = ColorFilter.colorMatrix(
    ColorMatrix(
        floatArrayOf(
            0f, 0f, 0f, 0f, 0f,
            0f, 0f, 0f, 0f, 0f,
            0f, 0f, 0f, 0f, 0f,
            0.2126f, 0.7152f, 0.0722f, 0f, 0f,
        ),
    ),
)";

/// Serialize a drawable as Kotlin that draws it with a Compose `Painter`.
pub fn write(drawable: &VectorDrawable, options: &Painter<'_>) -> String {
    // A blend mode that composites into the canvas the painter is drawn on
    // would read whatever the caller drew there, where the SVG composites
    // inside itself. Such a drawing is wrapped in a layer of its own.
    let isolated = blends_with_its_backdrop(&drawable.children);
    // The drawing sits inside the class, the method, `scale { }`, and the
    // `clipRect` that holds it to the viewport: four levels of indentation. The
    // layer that isolates a blended drawing opens at that depth and deepens its
    // children from there, so the writer starts at the same depth whether or
    // not there is a layer to open.
    let mut body = Writer {
        body: Body {
            depth: 3,
            path_data_strings: path_data_strings(&drawable.children),
            path_commands: PathCommands::Path,
            layer_bounds: Some(format!(
                "Rect(Offset.Zero, Size({}f, {}f))",
                number(drawable.viewport_width),
                number(drawable.viewport_height)
            )),
            ..Body::default()
        },
    };
    body.import("androidx.compose.runtime.Composable");
    body.import("androidx.compose.runtime.remember");
    body.import("androidx.compose.ui.geometry.Offset");
    body.import("androidx.compose.ui.geometry.Size");
    body.import("androidx.compose.ui.graphics.drawscope.DrawScope");
    body.import("androidx.compose.ui.graphics.drawscope.scale");
    body.import("androidx.compose.ui.graphics.painter.Painter");
    body.import("androidx.compose.ui.platform.LocalDensity");
    body.import("androidx.compose.ui.unit.Density");
    body.import("androidx.compose.ui.unit.dp");
    // An SVG clips its own viewport, and a drawable clips its bounds, so a
    // drawing whose artwork reaches past the viewport must not paint outside
    // the painter either.
    body.import("androidx.compose.ui.graphics.drawscope.clipRect");
    body.line(&format!(
        "clipRect(0f, 0f, {}f, {}f) {{",
        number(drawable.viewport_width),
        number(drawable.viewport_height)
    ));
    body.depth += 1;
    if isolated {
        body.open_layer(1.0, BlendMode::Normal);
    }
    body.scoped(&drawable.children, None);
    if isolated {
        body.close_layer();
    }
    body.depth -= 1;
    body.line("}");

    let name = options.name;
    let mut kotlin = String::new();
    if let Some(package) = options.package {
        let _ = writeln!(kotlin, "package {package}");
        kotlin.push('\n');
    }
    for import in &body.imports {
        let _ = writeln!(kotlin, "import {import}");
    }
    let _ = writeln!(kotlin);
    let _ = writeln!(kotlin, "/**");
    let _ = writeln!(
        kotlin,
        " * {name}, drawn path by path rather than as an `ImageVector`."
    );
    let _ = writeln!(kotlin, " *");
    let _ = writeln!(
        kotlin,
        " * Drawing the paths directly is what keeps what a vector model cannot hold: a"
    );
    let _ = writeln!(
        kotlin,
        " * group's opacity composited as one layer, a dashed stroke, a blend mode, a"
    );
    let _ = writeln!(
        kotlin,
        " * clip path that is not one nonzero path, and an alpha or luminance mask."
    );
    let _ = writeln!(kotlin, " */");
    let _ = writeln!(kotlin, "@Composable");
    let _ = writeln!(kotlin, "public fun remember{name}(): Painter {{");
    let _ = writeln!(kotlin, "    val density = LocalDensity.current");
    let _ = writeln!(
        kotlin,
        "    return remember(density) {{ {name}Painter(density) }}"
    );
    let _ = writeln!(kotlin, "}}");
    let _ = writeln!(kotlin);
    let _ = writeln!(
        kotlin,
        "private class {name}Painter(private val density: Density) : Painter() {{"
    );
    let _ = writeln!(
        kotlin,
        "    override val intrinsicSize: Size = with(density) {{"
    );
    let _ = writeln!(
        kotlin,
        "        Size({}.dp.toPx(), {}.dp.toPx())",
        number(drawable.width_dp),
        number(drawable.height_dp)
    );
    let _ = writeln!(kotlin, "    }}");
    let _ = writeln!(kotlin);
    let _ = writeln!(kotlin, "    override fun DrawScope.onDraw() {{");
    let _ = writeln!(
        kotlin,
        "        scale(size.width / {}f, size.height / {}f, pivot = Offset.Zero) {{",
        number(drawable.viewport_width),
        number(drawable.viewport_height)
    );
    kotlin.push_str(&body.text);
    let _ = writeln!(kotlin, "        }}");
    let _ = writeln!(kotlin, "    }}");
    let _ = writeln!(kotlin, "}}");
    for declaration in &body.declarations {
        kotlin.push('\n');
        let _ = writeln!(kotlin, "{declaration}");
    }
    kotlin
}

/// The draw target's writer: the shared formatting machinery, plus the calls
/// this target's own Compose types take.
///
/// It derefs to [`Body`], so `self.line(…)`, `self.depth`, and the rest of the
/// shared writer read as they do in the other target, while the methods below
/// name the calls this target makes.
struct Writer {
    body: Body,
}

impl Deref for Writer {
    type Target = Body;

    fn deref(&self) -> &Body {
        &self.body
    }
}

impl DerefMut for Writer {
    fn deref_mut(&mut self) -> &mut Body {
        &mut self.body
    }
}

impl Writer {
    /// Draw a node list: the clip paths that scope it, then what they scope,
    /// inside the layer `group` asks for when there is one.
    fn scoped(&mut self, nodes: &[VectorNode], group: Option<&VectorGroup>) {
        let (clips, children) = split_scope(nodes);
        self.scoped_clips(&clips, &children, group);
    }

    fn scoped_clips(
        &mut self,
        clips: &[&VectorClip],
        children: &[&VectorNode],
        group: Option<&VectorGroup>,
    ) {
        for clip in clips {
            self.clip(clip);
        }
        let layer = group.filter(|group| group.needs_layer());
        let canvas = layer.map(|group| self.open_layer(group.alpha, group.blend_mode));
        for node in children {
            match node {
                VectorNode::Path(path) => self.path(path),
                VectorNode::Group(group) => self.group(group),
                // Clips are emitted here, never as a node of their own.
                VectorNode::ClipPath(_) => {}
            }
        }
        if let (Some(group), Some(canvas)) = (layer, canvas.as_deref()) {
            if let Some(mask) = &group.mask {
                self.mask(mask, canvas);
            }
        }
        if layer.is_some() {
            self.close_layer();
        }
        for _ in clips {
            self.depth -= 1;
            self.line("}");
        }
    }

    fn group(&mut self, group: &VectorGroup) {
        if group.is_identity() {
            self.scoped(&group.children, Some(group));
            return;
        }
        // A VectorDrawable group means translate(pivot + translate) ∘ rotate ∘
        // scale ∘ translate(-pivot), and a `DrawTransform` applies each call
        // after the last, so the three calls in this order give the same map.
        self.import("androidx.compose.ui.graphics.drawscope.withTransform");
        self.line("withTransform({");
        self.depth += 1;
        if group.translate_x != 0.0 || group.translate_y != 0.0 {
            self.call(
                "translate",
                &[
                    Parameter::plain(format!("{}f", number(group.translate_x))),
                    Parameter::plain(format!("{}f", number(group.translate_y))),
                ],
                "",
            );
        }
        if group.rotation != 0.0 {
            self.call(
                "rotate",
                &[
                    Parameter::plain(format!("{}f", number(group.rotation))),
                    Parameter::plain(format!(
                        "Offset({}f, {}f)",
                        number(group.pivot_x),
                        number(group.pivot_y)
                    )),
                ],
                "",
            );
        }
        if group.scale_x != 1.0 || group.scale_y != 1.0 {
            self.call(
                "scale",
                &[
                    Parameter::plain(format!("{}f", number(group.scale_x))),
                    Parameter::plain(format!("{}f", number(group.scale_y))),
                    Parameter::plain(format!(
                        "Offset({}f, {}f)",
                        number(group.pivot_x),
                        number(group.pivot_y)
                    )),
                ],
                "",
            );
        }
        self.depth -= 1;
        self.line("}) {");
        self.depth += 1;
        self.scoped(&group.children, Some(group));
        self.depth -= 1;
        self.line("}");
    }

    /// Open the layer a group composites in: a canvas layer of the viewport,
    /// composited with the group's opacity and blend mode. The children are
    /// drawn into it, so overlapping shapes darken together rather than each
    /// fading on its own. The canvas it draws into is returned, because the
    /// mask that scopes the group composites into the same layer after the
    /// children are drawn.
    fn open_layer(&mut self, alpha: f32, blend_mode: BlendMode) -> String {
        self.import("androidx.compose.ui.graphics.drawscope.drawIntoCanvas");
        self.import("androidx.compose.ui.graphics.withSaveLayer");
        let canvas = self.local("canvas");
        self.line(&format!("drawIntoCanvas {{ {canvas} ->"));
        self.depth += 1;
        let arguments = self.depth + 1;
        let paint = self.paint(alpha, blend_mode, arguments);
        let bounds = self.layer_bounds();
        self.call(
            &format!("{canvas}.withSaveLayer"),
            &[Parameter::plain(bounds), Parameter::plain(paint)],
            " {",
        );
        self.depth += 1;
        canvas
    }

    /// Close the block a layer opened.
    fn close_layer(&mut self) {
        self.depth -= 1;
        self.line("}");
        self.depth -= 1;
        self.line("}");
    }

    /// Write the mask that scopes a group: one `DstIn` layer of the mask's
    /// content, and a second one for a luminance mask, whose colour matrix
    /// replaces the content's alpha rather than scaling by it.
    fn mask(&mut self, mask: &VectorMask, canvas: &str) {
        // A luminance mask reads its content's luminance times its content's
        // alpha. The colour matrix that moves the luminance into the alpha
        // channel replaces the alpha the content had, so a second layer applies
        // it, and every shape the mask draws has anti-aliased edges whose
        // coverage is that alpha: content with no explicitly transparent paint
        // is still partly transparent along its edges, where dropping the second
        // layer would leave them fully opaque.
        let luminance = mask.kind == MaskKind::Luminance;
        self.mask_layer(mask, canvas, luminance);
        if luminance {
            self.mask_layer(mask, canvas, false);
        }
    }

    /// Draw a mask's content into a layer of its own and composite it into the
    /// layer it scopes with `BlendMode.DstIn`, which keeps the group where the
    /// mask is opaque and takes it away where the mask is not.
    ///
    /// This comes after the group's children, because a canvas layer is
    /// composited into its destination when it is restored: a `DstIn` layer
    /// restored before the content was drawn would multiply an empty layer and
    /// leave the content unmasked.
    ///
    /// `luminance` turns the content's colour into the alpha that masks with it
    /// first, which is what a luminance mask reads; without it the content's
    /// own alpha is the mask, which is what an alpha mask reads.
    fn mask_layer(&mut self, mask: &VectorMask, canvas: &str, luminance: bool) {
        self.import("androidx.compose.ui.graphics.BlendMode");
        self.import("androidx.compose.ui.graphics.withSaveLayer");
        let paint = self.apply_expression("Paint", self.depth + 1, |body| {
            body.line("blendMode = BlendMode.DstIn");
            if luminance {
                body.import("androidx.compose.ui.graphics.ColorFilter");
                body.import("androidx.compose.ui.graphics.ColorMatrix");
                body.declare(LUMINANCE_TO_ALPHA);
                body.line("colorFilter = luminanceToAlpha");
            }
        });
        let bounds = self.layer_bounds();
        // The canvas of the layer this mask is applied to is still in scope:
        // `withSaveLayer` is inline and its block has no receiver of its own.
        self.call(
            &format!("{canvas}.withSaveLayer"),
            &[Parameter::plain(bounds), Parameter::plain(paint)],
            " {",
        );
        self.depth += 1;
        let region = mask.region.clone().map(VectorClip::intersect);
        let clips = region.iter().collect::<Vec<_>>();
        let children = mask.children.iter().collect::<Vec<_>>();
        self.scoped_clips(&clips, &children, None);
        self.depth -= 1;
        self.line("}");
    }

    /// `Name().apply { … }` as the value of a named parameter, written at
    /// `depth`, with `statements` as the body.
    fn apply_expression(
        &mut self,
        name: &str,
        depth: usize,
        statements: impl FnOnce(&mut Body),
    ) -> String {
        let body = self.scratch(depth + 1, statements);
        format!("{name}().apply {{\n{body}{}}}", "    ".repeat(depth))
    }

    /// Narrow the clip in effect to `clip`, and open the block it scopes.
    fn clip(&mut self, clip: &VectorClip) {
        self.import("androidx.compose.ui.graphics.drawscope.clipPath");
        let arguments = self.depth + 1;
        let mut parameters = vec![Parameter::named(
            "path",
            self.clip_expression(clip, arguments),
        )];
        if clip.op == ClipOp::Difference {
            self.import("androidx.compose.ui.graphics.ClipOp");
            parameters.push(Parameter::named("clipOp", "ClipOp.Difference".to_owned()));
        }
        self.call("clipPath", &parameters, " {");
        self.depth += 1;
    }

    /// A clip path as the value `clipPath` takes, written at `depth`.
    ///
    /// A clip path of one shape is the path it is. One of several is the union
    /// of them, written as `Path + Path`, which is `Path.op` with
    /// `PathOperation.Union`: filling every contour of one path by the nonzero
    /// rule would cancel opposite windings where they overlap rather than
    /// union them.
    fn clip_expression(&mut self, clip: &VectorClip, depth: usize) -> String {
        let mut shapes = clip.shapes.iter();
        let Some(first) = shapes.next() else {
            // An empty clip path clips everything away, which is what an empty
            // `clipPath` means.
            self.import("androidx.compose.ui.graphics.Path");
            return "Path()".to_owned();
        };
        let mut expression = self.path_expression(&first.data, first.fill_rule, depth);
        for shape in shapes {
            let _ = write!(
                expression,
                " + {}",
                self.path_expression(&shape.data, shape.fill_rule, depth)
            );
        }
        expression
    }

    fn path(&mut self, path: &VectorPath) {
        let filled = path.fill.is_some();
        // A stroke whose width the writer would print as zero is left out the
        // way a zero width is: SVG draws nothing for it, while Compose draws a
        // width of zero as a hairline, one pixel of the rasterized drawing.
        let stroked = path.stroke.is_some() && stroke_draws(path.stroke_width);
        if !filled && !stroked {
            return;
        }
        // `drawPath` takes one paint at a time, so a path that is both filled
        // and stroked draws twice; naming its geometry keeps the file from
        // carrying it twice.
        let name = (filled && stroked).then(|| {
            let name = self.local("path");
            let expression = self.path_expression(&path.path_data, path.fill_rule, self.depth);
            self.line(&format!("val {name} = {expression}"));
            name
        });
        if let Some(fill) = &path.fill {
            self.draw_path(path, fill, path.fill_alpha, None, name.as_deref());
        }
        if stroked {
            let stroke = path.stroke.as_ref().expect("a stroke was checked for");
            let style = self.stroke_style(path);
            self.draw_path(
                path,
                stroke,
                path.stroke_alpha,
                Some(style),
                name.as_deref(),
            );
        }
    }

    fn draw_path(
        &mut self,
        path: &VectorPath,
        paint: &Paint,
        alpha: f32,
        style: Option<String>,
        name: Option<&str>,
    ) {
        let arguments = self.depth + 1;
        let geometry = match name {
            Some(name) => name.to_owned(),
            None => self.path_expression(&path.path_data, path.fill_rule, arguments),
        };
        let mut parameters = vec![Parameter::named("path", geometry)];
        match paint {
            Paint::Solid(color) => {
                parameters.push(Parameter::named("color", self.color(*color, 1.0)));
            }
            gradient => parameters.push(Parameter::named("brush", self.brush(gradient, arguments))),
        }
        if alpha != 1.0 {
            parameters.push(Parameter::named("alpha", format!("{}f", number(alpha))));
        }
        if let Some(style) = style {
            parameters.push(Parameter::named("style", style));
        }
        self.call("drawPath", &parameters, "");
    }

    fn stroke_style(&mut self, path: &VectorPath) -> String {
        self.import("androidx.compose.ui.graphics.drawscope.Stroke");
        let mut parameters = vec![Parameter::named(
            "width",
            format!("{}f", number(path.stroke_width)),
        )];
        if path.stroke_miter != 4.0 {
            parameters.push(Parameter::named(
                "miter",
                format!("{}f", number(path.stroke_miter)),
            ));
        }
        match path.stroke_cap {
            LineCap::Butt => {}
            cap => {
                self.import("androidx.compose.ui.graphics.StrokeCap");
                parameters.push(Parameter::named(
                    "cap",
                    match cap {
                        LineCap::Round => "StrokeCap.Round",
                        _ => "StrokeCap.Square",
                    }
                    .to_owned(),
                ));
            }
        }
        match path.stroke_join {
            LineJoin::Miter => {}
            join => {
                self.import("androidx.compose.ui.graphics.StrokeJoin");
                parameters.push(Parameter::named(
                    "join",
                    match join {
                        LineJoin::Round => "StrokeJoin.Round",
                        _ => "StrokeJoin.Bevel",
                    }
                    .to_owned(),
                ));
            }
        }
        if let Some(dash) = &path.dash {
            parameters.push(Parameter::named("pathEffect", self.dash_effect(dash)));
        }
        let depth = self.depth + 1;
        let columns = depth * 4 + "style = ".len();
        self.expression("Stroke", &parameters, depth, columns)
    }

    fn dash_effect(&mut self, dash: &Dash) -> String {
        self.import("androidx.compose.ui.graphics.PathEffect");
        let intervals = dash
            .intervals
            .iter()
            .map(|value| format!("{}f", number(*value)))
            .collect::<Vec<_>>()
            .join(", ");
        let mut effect = format!("PathEffect.dashPathEffect(floatArrayOf({intervals})");
        if dash.offset != 0.0 {
            let _ = write!(effect, ", {}f", number(dash.offset));
        }
        effect.push(')');
        effect
    }

    /// A path as the value it is, written at `depth`.
    fn path_expression(&mut self, data: &PathData, fill_rule: FillRule, depth: usize) -> String {
        let even_odd = fill_rule == FillRule::EvenOdd;
        if self.path_data_strings {
            // One call per path however many commands it carries, which is
            // what keeps a large drawing inside the JVM's method limit.
            self.import("androidx.compose.ui.graphics.vector.addPathNodes");
            self.import("androidx.compose.ui.graphics.vector.toPath");
            let text = crate::short_path::write(&data.0);
            let mut expression = format!("{}.toPath()", path_data_call(&text, depth));
            if even_odd {
                self.import("androidx.compose.ui.graphics.PathFillType");
                expression.push_str(".apply { fillType = PathFillType.EvenOdd }");
            }
            return expression;
        }
        self.import("androidx.compose.ui.graphics.Path");
        let body = self.scratch(depth + 1, |body| {
            if even_odd {
                body.import("androidx.compose.ui.graphics.PathFillType");
                body.line("fillType = PathFillType.EvenOdd");
            }
            for command in &data.0 {
                body.command(command);
            }
        });
        format!("Path().apply {{\n{body}{}}}", "    ".repeat(depth))
    }

    fn paint(&mut self, alpha: f32, blend_mode: BlendMode, depth: usize) -> String {
        self.import("androidx.compose.ui.graphics.Paint");
        if alpha == 1.0 && blend_mode == BlendMode::Normal {
            return "Paint()".to_owned();
        }
        self.apply_expression("Paint", depth, |body| {
            if alpha != 1.0 {
                body.line(&format!("alpha = {}f", number(alpha)));
            }
            if blend_mode != BlendMode::Normal {
                body.import("androidx.compose.ui.graphics.BlendMode");
                body.line(&format!("blendMode = {}", blend_mode_name(blend_mode)));
            }
        })
    }

    /// The rectangle a canvas layer has to cover. Only the viewport has to be
    /// covered: the lowering keeps artwork inside it, and a canvas layer clips
    /// whatever falls outside.
    fn layer_bounds(&mut self) -> String {
        self.import("androidx.compose.ui.geometry.Offset");
        self.import("androidx.compose.ui.geometry.Rect");
        self.import("androidx.compose.ui.geometry.Size");
        self.layer_bounds
            .clone()
            .expect("the draw target sets the layer bounds")
    }
}

/// Most bytecode a drawing's `onDraw` may compile to, in the bytes
/// [`on_draw_bytes`] estimates.
///
/// Kotlin inlines every block the emitter writes — `clipRect`, `clipPath`, and
/// the `drawIntoCanvas` around `withSaveLayer` — so a drawing lands in one
/// method however deeply it nests, and the JVM refuses a method of over 64 KB,
/// which the Kotlin backend reports as `Method too large`. The estimate is an
/// upper bound on that bytecode rather than a count of it: every construct is
/// charged about 1.6 times the cost of its widest encoding, so a drawing at this
/// limit compiles to roughly 37 KB and has room for the constructs the
/// calibration did not cover. `tools/android-renderer/RESULTS.md` records the
/// measurement.
pub(crate) const MAX_ON_DRAW_BYTES: usize = 60_000;

/// Most path commands a drawn drawing may expand to, counting the copies a
/// luminance mask makes of its content.
///
/// A luminance mask is written twice, and each nested mask doubles the content
/// it holds, so a chain of them multiplies without bound: thirty masks around
/// an ordinary path would emit a billion copies of it. A drawing that expands
/// past this is rejected rather than written, which also keeps the counts
/// themselves inside a `usize`. The limit is far above any artwork: the largest
/// drawings here carry a few thousand commands.
pub(crate) const MAX_COMMANDS: usize = 1_000_000;

/// Bytes of bytecode one `drawPath` call costs, before its arguments.
const DRAW_PATH: usize = 16;
/// Bytes a path written as path data costs: `addPathNodes("…").toPath()`.
const PATH_DATA: usize = 16;
/// Bytes one command of a path written as builder calls costs.
///
/// This is the weight that scales with the artwork, so it is derived from the
/// widest encoding of a command rather than from a typical one: `cubicTo` takes
/// six floats, and a `Path().apply { … }` calls it through the `PathBuilder`
/// interface, which is `aload` (2), six `ldc_w` (3 each), and `invokeinterface`
/// (5) — 25 bytes, or 27 if the receiver's local slot needs `wide`. Measured
/// against a 200-segment path in `tools/android-renderer/compose`: 22 to 24
/// bytes per command.
const PATH_COMMAND: usize = 40;
/// Bytes a `Color(0x…)` argument costs.
const COLOR: usize = 16;
/// Bytes a gradient brush costs before its stops: the `Brush.linearGradient` or
/// `Brush.radialGradient` call and the `arrayOf` it is given.
const BRUSH: usize = 160;
/// Bytes one `0f to Color(0x…)` stop of a gradient brush costs.
///
/// Like [`PATH_COMMAND`] this scales with the artwork, so it is the widest
/// encoding of a stop rather than a typical one: `aload`, the index of the array
/// slot (`sipush` once there are more than 127 stops), the offset (`ldc_w` once
/// the constant pool passes 255), `Float.valueOf`, the colour (`ldc2_w`,
/// `ColorKt.Color`, `Color.box-impl`), `TuplesKt.to`, and `aastore` — 24 bytes,
/// or 26 if the array's local slot needs `wide`. Measured against gradients of
/// 100 and 300 stops: 21 to 24 bytes per stop.
const BRUSH_STOP: usize = 40;
/// Bytes the `Offset`s a linear gradient is placed with, or the `Offset` and
/// radius a radial gradient is placed with, cost.
const BRUSH_POINT: usize = 40;
/// Bytes an `alpha = …f`, `blendMode = …`, or `clipOp = …` argument costs.
const PAINT_ARGUMENT: usize = 16;
/// Bytes a `Stroke(…)` style costs, before its arguments.
const STROKE: usize = 48;
/// Bytes one `Stroke` argument costs: `miter`, `cap`, `join`, or `pathEffect`.
const STROKE_ARGUMENT: usize = 24;
/// Bytes a `PathEffect.dashPathEffect(floatArrayOf(…), …)` costs before its
/// intervals.
const DASH: usize = 60;
/// Bytes one interval of a dash array costs.
///
/// An interval is one store into a primitive `float` array: `aload`, the index
/// (`sipush` at worst), the value (`ldc_w` at worst), and `fastore` — 9 bytes.
/// Measured against a four-interval dash: 5 bytes each.
const DASH_INTERVAL: usize = 24;
/// Bytes naming a path into a local costs, which a path that is both filled and
/// stroked does so that its geometry is written once.
const LOCAL: usize = 8;
/// Bytes a canvas layer costs: `drawIntoCanvas`, the `withSaveLayer` inside it,
/// the bounds, and the `Paint`.
const LAYER: usize = 300;
/// Bytes a `clipPath(…)` call costs, before its path.
const CLIP: usize = 300;
/// Bytes one `translate`, `rotate`, or `scale` call inside `withTransform`
/// costs.
const TRANSFORM_CALL: usize = 48;
/// Bytes one `Path + Path` costs, which is the `Path.op` that unions the shapes
/// of a clip path.
///
/// The union is `Path.plus`, called on the receiver with the next shape as its
/// argument: two `aload`s and an `invokeinterface`, 9 bytes. Measured in the
/// harness fixture, which unions two shapes: 9 bytes.
const PATH_OP: usize = 24;

/// Bytecode a node list's statements in `onDraw` compile to, in bytes.
///
/// This is what [`MAX_ON_DRAW_BYTES`] bounds. Counting statements is not enough,
/// because the JVM's limit is on the bytecode a statement compiles to and the
/// constructs differ by an order of magnitude: a `Color` is a `ldc2_w` and a
/// call, while a gradient brush builds an array of boxed `Pair`s, and a layer is
/// an inlined `withSaveLayer` block around everything inside it. Each weight
/// above is the cost of that construct with about 1.6 times that on top, and the
/// weights that scale with the artwork are its widest encoding rather than a
/// typical one, so the estimate is an upper bound on the bytecode and not a
/// count of it.
///
/// A path written as builder calls counts one weight per command; the geometry
/// of a path that is both filled and stroked is written once, into a local.
///
/// Saturating, because a luminance mask is written twice and each nested mask
/// doubles the cost under it, which a deep chain multiplies past what a `usize`
/// holds. The chain is rejected for it, which is what the command count in
/// [`crate::kotlin::commands`] does too.
pub(crate) fn on_draw_bytes(nodes: &[VectorNode], strings: bool) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            VectorNode::Path(path) => {
                let filled = path.fill.is_some();
                // A stroke the writer would print a width of zero for is left
                // out rather than written as one.
                let stroked = path.stroke.is_some() && stroke_draws(path.stroke_width);
                let geometry = geometry_bytes(&path.path_data, strings);
                // An even-odd path writes its fill type beside its geometry.
                let fill_type = fill_type_bytes(path.fill_rule);
                let mut bytes = usize::from(filled && stroked).saturating_mul(LOCAL);
                if filled {
                    bytes = bytes
                        .saturating_add(DRAW_PATH)
                        .saturating_add(geometry)
                        .saturating_add(fill_type)
                        .saturating_add(paint_bytes(path.fill.as_ref()));
                }
                if stroked {
                    bytes = bytes
                        .saturating_add(DRAW_PATH)
                        .saturating_add(geometry)
                        .saturating_add(fill_type)
                        .saturating_add(paint_bytes(path.stroke.as_ref()))
                        .saturating_add(stroke_bytes(path));
                }
                bytes
            }
            // A clip path is one `clipPath` call over its shapes, and a clip
            // path of several shapes unions them with a `Path + Path` each.
            VectorNode::ClipPath(clip) => {
                let mut bytes = CLIP
                    .saturating_add(usize::from(clip.op == ClipOp::Difference) * PAINT_ARGUMENT);
                for shape in &clip.shapes {
                    bytes = bytes
                        .saturating_add(geometry_bytes(&shape.data, strings))
                        .saturating_add(fill_type_bytes(shape.fill_rule));
                }
                bytes.saturating_add(PATH_OP.saturating_mul(clip.shapes.len().saturating_sub(1)))
            }
            VectorNode::Group(group) => {
                // `withTransform` and the calls inside it.
                let transform = if group.is_identity() {
                    0
                } else {
                    let calls = 1
                        + usize::from(group.translate_x != 0.0 || group.translate_y != 0.0)
                        + usize::from(group.rotation != 0.0)
                        + usize::from(group.scale_x != 1.0 || group.scale_y != 1.0);
                    TRANSFORM_CALL.saturating_mul(calls)
                };
                // `drawIntoCanvas` and `withSaveLayer`, whose block is inlined.
                let layer = if group.needs_layer() {
                    LAYER.saturating_add(layer_paint_bytes(group))
                } else {
                    0
                };
                let mask = group.mask.as_ref().map_or(0, |mask| {
                    // Every pass writes the region clip, the layer, and the
                    // whole content, and each nested mask inside the content
                    // doubles it again, which the recursion has already counted.
                    let passes: usize = if mask.kind == MaskKind::Luminance {
                        2
                    } else {
                        1
                    };
                    let region = match &mask.region {
                        Some(region) => CLIP.saturating_add(geometry_bytes(region, strings)),
                        None => 0,
                    };
                    passes.saturating_mul(
                        LAYER
                            .saturating_add(PAINT_ARGUMENT)
                            .saturating_add(region)
                            .saturating_add(on_draw_bytes(&mask.children, strings)),
                    )
                });
                transform
                    .saturating_add(layer)
                    .saturating_add(mask)
                    .saturating_add(on_draw_bytes(&group.children, strings))
            }
        })
        .fold(0, usize::saturating_add)
}

/// Whether a stroke of `width` draws anything.
///
/// The width has to print as a positive number, not merely be positive: the
/// writer prints every width below its six decimals as `0`, and Compose draws a
/// width of zero as a hairline, one pixel of the rasterized drawing, where SVG
/// draws nothing at all. A stroke whose width would be written as zero is left
/// out instead.
fn stroke_draws(width: f32) -> bool {
    number(width) != "0"
}

/// Bytes a path's geometry costs. A path written as path data is one call
/// however many commands it carries; one written as builder calls is a call per
/// command.
fn geometry_bytes(data: &PathData, strings: bool) -> usize {
    if strings {
        PATH_DATA
    } else {
        PATH_DATA.saturating_add(PATH_COMMAND.saturating_mul(data.0.len()))
    }
}

/// Bytes the `fillType = PathFillType.EvenOdd` a path's geometry is given costs,
/// which only an even-odd path writes.
fn fill_type_bytes(fill_rule: FillRule) -> usize {
    match fill_rule {
        FillRule::NonZero => 0,
        FillRule::EvenOdd => PAINT_ARGUMENT,
    }
}

/// Bytes a paint argument costs: a `Color`, or a gradient brush with its stops
/// and the `Offset`s or radius it is placed with.
fn paint_bytes(paint: Option<&Paint>) -> usize {
    match paint {
        None => 0,
        Some(Paint::Solid(_)) => COLOR,
        Some(Paint::Linear(gradient)) => BRUSH
            .saturating_add(BRUSH_STOP.saturating_mul(gradient.stops.len()))
            .saturating_add(BRUSH_POINT.saturating_mul(2))
            .saturating_add(tile_mode_bytes(gradient.tile_mode)),
        Some(Paint::Radial(gradient)) => BRUSH
            .saturating_add(BRUSH_STOP.saturating_mul(gradient.stops.len()))
            .saturating_add(BRUSH_POINT.saturating_mul(2))
            .saturating_add(tile_mode_bytes(gradient.tile_mode)),
    }
}

/// Bytes a `tileMode = …` argument costs, which a gradient only writes when it
/// is not the default.
fn tile_mode_bytes(tile_mode: TileMode) -> usize {
    match tile_mode {
        TileMode::Clamp => 0,
        _ => PAINT_ARGUMENT,
    }
}

/// Bytes a `Stroke(…)` style costs: its width, the arguments it is given beyond
/// it, and the dash effect if it has one.
fn stroke_bytes(path: &VectorPath) -> usize {
    let arguments = usize::from(path.stroke_miter != 4.0)
        + usize::from(path.stroke_cap != LineCap::Butt)
        + usize::from(path.stroke_join != LineJoin::Miter);
    let mut bytes = STROKE.saturating_add(STROKE_ARGUMENT.saturating_mul(arguments));
    if let Some(dash) = &path.dash {
        bytes = bytes
            .saturating_add(DASH)
            .saturating_add(DASH_INTERVAL.saturating_mul(dash.intervals.len()))
            // A dash with an offset writes one more argument.
            .saturating_add(usize::from(dash.offset != 0.0) * PAINT_ARGUMENT);
    }
    bytes
}

/// Bytes the `Paint` of a group's layer costs: its opacity and blend mode when
/// they are not the defaults.
fn layer_paint_bytes(group: &VectorGroup) -> usize {
    PAINT_ARGUMENT.saturating_mul(
        usize::from(group.alpha != 1.0) + usize::from(group.blend_mode != BlendMode::Normal),
    )
}

/// Split a node list into the clip paths that scope it and what they scope.
///
/// The lowering writes a group's clip paths before anything they scope, and a
/// clip scopes the siblings that follow it, where `clipPath` scopes its whole
/// block. A clip path that is not in that prefix would therefore scope more
/// through Compose than it does as an SVG.
fn split_scope(nodes: &[VectorNode]) -> (Vec<&VectorClip>, Vec<&VectorNode>) {
    let clips = nodes
        .iter()
        .take_while(|node| matches!(node, VectorNode::ClipPath(_)))
        .filter_map(|node| match node {
            VectorNode::ClipPath(clip) => Some(clip),
            _ => None,
        })
        .collect::<Vec<&VectorClip>>();
    debug_assert_eq!(
        clips.len(),
        nodes
            .iter()
            .filter(|node| matches!(node, VectorNode::ClipPath(_)))
            .count(),
        "the lowering puts every clip path of a group before its paths"
    );
    let children = nodes.iter().skip(clips.len()).collect();
    (clips, children)
}

/// The Compose name of a compositing mode.
fn blend_mode_name(blend_mode: BlendMode) -> &'static str {
    match blend_mode {
        BlendMode::Normal => "BlendMode.SrcOver",
        BlendMode::Multiply => "BlendMode.Multiply",
        BlendMode::Screen => "BlendMode.Screen",
        BlendMode::Overlay => "BlendMode.Overlay",
        BlendMode::Darken => "BlendMode.Darken",
        BlendMode::Lighten => "BlendMode.Lighten",
        BlendMode::ColorDodge => "BlendMode.ColorDodge",
        BlendMode::ColorBurn => "BlendMode.ColorBurn",
        BlendMode::HardLight => "BlendMode.Hardlight",
        BlendMode::SoftLight => "BlendMode.Softlight",
        BlendMode::Difference => "BlendMode.Difference",
        BlendMode::Exclusion => "BlendMode.Exclusion",
        BlendMode::Hue => "BlendMode.Hue",
        BlendMode::Saturation => "BlendMode.Saturation",
        BlendMode::Color => "BlendMode.Color",
        BlendMode::Luminosity => "BlendMode.Luminosity",
    }
}
