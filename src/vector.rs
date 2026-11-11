use usvg::tiny_skia_path::{PathSegment, Point, Transform};

use crate::analysis::{Bounds, Compatibility, Diagnostic, DiagnosticCode, Metrics, Severity};

#[derive(Clone, Debug)]
pub struct VectorDrawable {
    pub width_dp: f32,
    pub height_dp: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub children: Vec<VectorNode>,
}

#[derive(Clone, Debug)]
pub enum VectorNode {
    Group(VectorGroup),
    Path(VectorPath),
    ClipPath(VectorClip),
}

#[derive(Clone, Debug)]
pub struct VectorGroup {
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub translate_x: f32,
    pub translate_y: f32,
    /// Opacity of the group as one layer, so overlapping shapes darken
    /// together rather than each fading on its own. Always 1 for a drawable,
    /// which folds group opacity into the alpha of each shape instead.
    pub alpha: f32,
    /// How the group composites into what is already drawn.
    pub blend_mode: BlendMode,
    /// Whether the group must be composited in a layer of its own, so that a
    /// blend mode inside it does not reach the backdrop behind the group.
    pub isolate: bool,
    /// Mask scoping the group's children, in place of the clip paths a
    /// drawable can express instead.
    pub mask: Option<VectorMask>,
    pub children: Vec<VectorNode>,
}

impl VectorGroup {
    pub fn new(children: Vec<VectorNode>) -> Self {
        Self {
            pivot_x: 0.0,
            pivot_y: 0.0,
            rotation: 0.0,
            scale_x: 1.0,
            scale_y: 1.0,
            translate_x: 0.0,
            translate_y: 0.0,
            alpha: 1.0,
            blend_mode: BlendMode::Normal,
            isolate: false,
            mask: None,
            children,
        }
    }

    pub fn is_identity(&self) -> bool {
        self.pivot_x == 0.0
            && self.pivot_y == 0.0
            && self.rotation == 0.0
            && self.scale_x == 1.0
            && self.scale_y == 1.0
            && self.translate_x == 0.0
            && self.translate_y == 0.0
    }

    /// Whether the group's children must be composited in a layer of their
    /// own before they reach the destination.
    pub fn needs_layer(&self) -> bool {
        self.alpha != 1.0
            || self.blend_mode != BlendMode::Normal
            || self.isolate
            || self.mask.is_some()
    }

    pub fn transform(&self) -> Transform {
        if self.is_identity() {
            return Transform::identity();
        }
        Transform::from_translate(
            self.translate_x + self.pivot_x,
            self.translate_y + self.pivot_y,
        )
        .pre_rotate(self.rotation)
        .pre_scale(self.scale_x, self.scale_y)
        .pre_translate(-self.pivot_x, -self.pivot_y)
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct GroupTransform {
    pub pivot_x: f32,
    pub pivot_y: f32,
    pub rotation: f32,
    pub scale_x: f32,
    pub scale_y: f32,
    pub inv_transform: Transform,
}

#[derive(Clone, Debug)]
pub struct VectorPath {
    pub path_data: PathData,
    pub fill: Option<Paint>,
    pub fill_alpha: f32,
    pub fill_rule: FillRule,
    pub stroke: Option<Paint>,
    pub stroke_alpha: f32,
    pub stroke_width: f32,
    pub stroke_cap: LineCap,
    pub stroke_join: LineJoin,
    pub stroke_miter: f32,
    /// Dash pattern of the stroke. Always `None` for a drawable, which has no
    /// dash attribute.
    pub dash: Option<Dash>,
}

/// A stroked path's dash pattern, in viewport units.
#[derive(Clone, Debug)]
pub struct Dash {
    /// Alternating on and off lengths. An odd number of source lengths is
    /// repeated to an even count, as SVG specifies and as a dash effect needs.
    pub intervals: Vec<f32>,
    /// Distance into the pattern the stroke starts at.
    pub offset: f32,
}

/// How a group composites into what is already drawn.
///
/// The variants are the CSS compositing modes `mix-blend-mode` takes, which
/// are the ones a Compose `BlendMode` names one for one.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlendMode {
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

/// A clip path scoping the nodes that follow it.
#[derive(Clone, Debug)]
pub struct VectorClip {
    /// Geometry of the clip, one entry per shape the `clipPath` holds.
    ///
    /// They are unioned, not intersected: a `clipPath` clips to everything its
    /// children cover. Keeping them apart lets each keep the winding rule it
    /// was filled by, which a writer resolves with a path union.
    ///
    /// Never empty: every clip the lowering makes holds at least one shape,
    /// which the drawable and `ImageVector` writers rely on when they write a
    /// clip as its first shape.
    pub shapes: Vec<ClipShape>,
    /// How the geometry combines with the clip already in effect.
    pub op: ClipOp,
}

/// One shape of a clip: geometry and the winding rule it is filled by.
#[derive(Clone, Debug)]
pub struct ClipShape {
    pub data: PathData,
    pub fill_rule: FillRule,
}

impl VectorClip {
    /// A clip that narrows the clip in effect to one shape filled by the
    /// nonzero winding rule, which is what a drawable's `<clip-path>` is.
    pub fn intersect(data: PathData) -> Self {
        Self {
            shapes: vec![ClipShape {
                data,
                fill_rule: FillRule::NonZero,
            }],
            op: ClipOp::Intersect,
        }
    }
}

/// How a clip combines with the clip already in effect.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClipOp {
    Intersect,
    Difference,
}

/// An alpha or luminance mask scoping a group's children.
#[derive(Clone, Debug)]
pub struct VectorMask {
    /// Which channel of the mask's content is the mask.
    pub kind: MaskKind,
    /// The mask's own content, drawn into a layer of its own and then
    /// composited into the group with its alpha.
    pub children: Vec<VectorNode>,
    /// Region the mask is confined to, in viewport units. The mask reads as
    /// fully transparent outside it.
    pub region: Option<PathData>,
}

/// Which channel of a mask's content is the mask.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MaskKind {
    /// The content's alpha channel, which `BlendMode.DstIn` composites with
    /// directly.
    Alpha,
    /// The content's luminance, times its alpha, which needs a colour matrix
    /// before `BlendMode.DstIn` sees it as alpha.
    Luminance,
}

#[derive(Clone, Debug)]
pub struct PathData(pub Vec<PathCommand>);

#[derive(Clone, Debug)]
pub enum PathCommand {
    Move(f32, f32),
    Line(f32, f32),
    Quad(f32, f32, f32, f32),
    Cubic(f32, f32, f32, f32, f32, f32),
    Close,
}

#[derive(Clone, Copy, Debug)]
pub struct Color(pub u8, pub u8, pub u8);

/// Fill or stroke paint.
#[derive(Clone, Debug)]
pub enum Paint {
    Solid(Color),
    Linear(LinearGradient),
    Radial(RadialGradient),
}

impl Paint {
    pub fn is_gradient(&self) -> bool {
        !matches!(self, Self::Solid(_))
    }

    /// Whether this paint can produce any visible pixel. Gradient stops are
    /// serialized with an 8-bit alpha, so a gradient paints nothing when every
    /// stop would round to alpha `00`.
    pub fn paints_pixels(&self) -> bool {
        match self {
            Self::Solid(_) => true,
            Self::Linear(gradient) => gradient.stops.iter().any(GradientStop::is_visible),
            Self::Radial(gradient) => gradient.stops.iter().any(GradientStop::is_visible),
        }
    }
}

impl GradientStop {
    /// The 8-bit alpha this stop is serialized with.
    pub fn alpha_byte(&self) -> u8 {
        alpha_byte(self.alpha)
    }

    fn is_visible(&self) -> bool {
        self.alpha_byte() > 0
    }
}

/// Quantize an alpha to the 8-bit channel used in `#AARRGGBB` colors.
pub fn alpha_byte(alpha: f32) -> u8 {
    (alpha.clamp(0.0, 1.0) * 255.0).round() as u8
}

/// Linear gradient in viewport coordinates.
#[derive(Clone, Debug)]
pub struct LinearGradient {
    pub start_x: f32,
    pub start_y: f32,
    pub end_x: f32,
    pub end_y: f32,
    pub stops: Vec<GradientStop>,
    pub tile_mode: TileMode,
}

/// Circular radial gradient in viewport coordinates.
#[derive(Clone, Debug)]
pub struct RadialGradient {
    pub center_x: f32,
    pub center_y: f32,
    pub radius: f32,
    pub stops: Vec<GradientStop>,
    pub tile_mode: TileMode,
}

#[derive(Clone, Copy, Debug)]
pub struct GradientStop {
    pub offset: f32,
    pub color: Color,
    pub alpha: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TileMode {
    Clamp,
    Mirror,
    Repeat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineCap {
    Butt,
    Round,
    Square,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LineJoin {
    Miter,
    Round,
    Bevel,
}

impl VectorDrawable {
    pub fn minimum_api(&self) -> u32 {
        if self.api_24_features().is_empty() {
            21
        } else {
            24
        }
    }

    /// What the drawable uses that API 21 cannot render, in a stable order.
    fn api_24_features(&self) -> Vec<Api24Feature> {
        let mut features = Vec::new();
        if self.uses_gradients() {
            features.push(Api24Feature::Gradient);
        }
        if contains_even_odd(&self.children) {
            features.push(Api24Feature::EvenOddFill);
        }
        if count_clip_paths(&self.children) > 1 {
            features.push(Api24Feature::MultipleClips);
        }
        features
    }

    pub fn uses_gradients(&self) -> bool {
        contains_gradient(&self.children)
    }
}

fn contains_gradient(nodes: &[VectorNode]) -> bool {
    nodes.iter().any(|node| match node {
        VectorNode::Group(group) => contains_gradient(&group.children),
        VectorNode::Path(path) => [&path.fill, &path.stroke]
            .into_iter()
            .flatten()
            .any(Paint::is_gradient),
        VectorNode::ClipPath(_) => false,
    })
}

fn contains_even_odd(nodes: &[VectorNode]) -> bool {
    nodes.iter().any(|node| match node {
        VectorNode::Group(group) => contains_even_odd(&group.children),
        VectorNode::Path(path) => path.fill_rule == FillRule::EvenOdd,
        VectorNode::ClipPath(_) => false,
    })
}

fn count_clip_paths(nodes: &[VectorNode]) -> usize {
    nodes
        .iter()
        .map(|node| match node {
            VectorNode::Group(group) => count_clip_paths(&group.children),
            VectorNode::ClipPath(_) => 1,
            VectorNode::Path(_) => 0,
        })
        .sum()
}

pub(crate) struct LowerOptions {
    pub allow_approximate: bool,
    pub notification: Option<crate::notification::Whitening>,
    /// What the output can express, which decides what the lowering keeps.
    pub target: Target,
}

/// What an output format can express, which decides what the lowering keeps.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Target {
    /// A VectorDrawable, or a Compose `ImageVector`, which can express the
    /// same set of SVGs.
    Vector,
    /// Kotlin that draws the artwork's paths directly, which can express a
    /// group's opacity, a dashed stroke, a blend mode, any clip path, and an
    /// alpha or luminance mask.
    Draw,
}

pub(crate) fn lower(
    tree: &usvg::Tree,
    options: &mut LowerOptions,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
    metrics: &mut Metrics,
) -> Option<VectorDrawable> {
    let size = tree.size();
    metrics.width = size.width();
    metrics.height = size.height();
    metrics.viewport_width = size.width();
    metrics.viewport_height = size.height();
    metrics.gradients = tree.linear_gradients().len() + tree.radial_gradients().len();
    metrics.clip_paths = tree.clip_paths().len();
    metrics.groups = count_groups(tree.root()).saturating_sub(1);
    metrics.content_bounds = None;

    let mut children = Vec::new();
    Lowering {
        options: &mut *options,
        compatibility: &mut *compatibility,
        diagnostics: &mut *diagnostics,
        metrics: &mut *metrics,
    }
    .visit_group(
        tree.root(),
        &mut children,
        1.0,
        // The root's own transforms are absolute already, so nothing has to be
        // applied on top of them.
        Transform::identity(),
    );

    metrics.content_bounds = metrics
        .content_bounds
        .and_then(|bounds| clamp_bounds(bounds, size.width(), size.height()));
    // Only the draw target writes a mask's content more than once, so only it
    // can expand a drawing past what is worth writing or compiling.
    if options.target == Target::Draw {
        // The commands bound what the drawing expands to and how large the file
        // it writes can be; the bytecode bounds what lands in the one method it
        // is written into, which is what the JVM refuses past 64 KB.
        let commands = crate::kotlin::commands(&children);
        if commands > crate::draw::MAX_COMMANDS {
            unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::TooLargeToCompile,
                "the drawing expands to more path commands than the draw target writes, once its masks are drawn",
                "Flatten the nested masks in the source design tool.",
            );
        } else if crate::draw::on_draw_bytes(&children, crate::kotlin::path_data_strings(&children))
            > crate::draw::MAX_ON_DRAW_BYTES
        {
            // The estimate is an upper bound on the bytecode the method
            // compiles to, which is what makes a drawing under the limit
            // compile; a drawing over it may still have compiled, and is
            // rejected rather than risk one that does not.
            unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::TooLargeToCompile,
                "the drawing writes more code into one method than the JVM compiles",
                "Split the artwork into several icons, or flatten the masks that repeat it.",
            );
        }
    }
    diagnostics.extend(large_dimensions_warning(size.width(), size.height()));
    if compatibility.is_convertible(options.allow_approximate) {
        Some(VectorDrawable {
            width_dp: size.width(),
            height_dp: size.height(),
            viewport_width: size.width(),
            viewport_height: size.height(),
            children,
        })
    } else {
        None
    }
}

fn count_groups(group: &usvg::Group) -> usize {
    1 + group
        .children()
        .iter()
        .filter_map(|node| match node {
            usvg::Node::Group(group) => Some(count_groups(group)),
            _ => None,
        })
        .sum::<usize>()
}

/// The lowering of one SVG: what the target can express, and the findings and
/// measurements collected while the tree is walked.
struct Lowering<'a> {
    options: &'a mut LowerOptions,
    compatibility: &'a mut Compatibility,
    diagnostics: &'a mut Vec<Diagnostic>,
    metrics: &'a mut Metrics,
}

impl Lowering<'_> {
    fn visit_group(
        &mut self,
        group: &usvg::Group,
        output: &mut Vec<VectorNode>,
        inherited_alpha: f32,
        base: Transform,
    ) {
        let direct = self.options.target == Target::Draw;
        let group_alpha = group.opacity().get();
        // A drawable has no group alpha, so it folds one into each shape's own
        // alpha, which only looks the same while nothing in the group overlaps.
        if !direct
            && group_alpha < 1.0
            && (visible_path_count(group) > 1 || contains_fill_and_stroke(group))
        {
            unsupported(
                self.compatibility,
                self.diagnostics,
                DiagnosticCode::UnsupportedPaint,
                "group opacity cannot be represented without changing overlap semantics",
                "Merge the group's shapes into one path, or set the opacity on each shape if darker overlaps are acceptable.",
            );
        }
        let inherited_alpha = if direct {
            inherited_alpha
        } else {
            inherited_alpha * group_alpha
        };
        if !direct
            && (!group.filters().is_empty()
                || group.blend_mode() != usvg::BlendMode::Normal
                || group.isolate())
        {
            unsupported(
                self.compatibility,
                self.diagnostics,
                DiagnosticCode::UnsupportedPaint,
                "the normalized tree requires unsupported masking, filtering, or compositing",
                "Flatten masks, filters, and blend modes into plain paths in the source design tool.",
            );
        }
        if direct && !group.filters().is_empty() {
            unsupported(
                self.compatibility,
                self.diagnostics,
                DiagnosticCode::UnsupportedFilter,
                "SVG filters cannot be drawn path by path",
                "Remove the filter, or draw its effect, such as a shadow, as shapes.",
            );
        }
        let mut children = Vec::new();
        for node in group.children() {
            match node {
                usvg::Node::Group(group) => {
                    self.visit_group(group, &mut children, inherited_alpha, base)
                }
                usvg::Node::Path(path) => {
                    if !path.is_visible() {
                        continue;
                    }
                    self.metrics.paths += 1;
                    // Geometry is written in output coordinates, so a paint
                    // that is placed by the transform, and a stroke that is
                    // measured by it, take the same transform the geometry
                    // does. It differs from the path's own absolute transform
                    // only inside a mask, whose content is expressed relative
                    // to the element the mask is on.
                    let transform = base.pre_concat(path.abs_transform());
                    let data = transformed_path_with(path, transform);
                    self.metrics.path_commands += data.0.len();
                    let fill = path.fill().and_then(|fill| {
                        lower_paint(
                            fill.paint(),
                            transform,
                            self.options,
                            self.compatibility,
                            self.diagnostics,
                        )
                        .map(|(paint, alpha, group_transform)| {
                            (
                                paint,
                                fill.opacity().get() * inherited_alpha * alpha,
                                map_fill_rule(fill.rule()),
                                group_transform,
                            )
                        })
                    });
                    let stroke = path.stroke().and_then(|stroke| {
                    if stroke.linejoin() == usvg::LineJoin::MiterClip {
                        unsupported(
                            self.compatibility,
                            self.diagnostics,
                            DiagnosticCode::UnsupportedPaint,
                            // Neither output holds the join: a drawable and an
                            // `ImageVector` cannot represent it at all, and the
                            // draw target does not draw it either.
                            if direct {
                                "miter-clip stroke joins are not supported by the draw target"
                            } else {
                                "miter-clip stroke joins cannot be represented by a vector drawable or an ImageVector"
                            },
                            "Use a miter, round, or bevel stroke join.",
                        );
                    }
                    let dash = stroke.dasharray().map(|intervals| Dash {
                        intervals: intervals.to_vec(),
                        offset: stroke.dashoffset(),
                    });
                    // A drawable has no dash attribute at all, so the lowering
                    // drops the pattern rather than keeping one the XML and the
                    // `ImageVector` cannot write, which would also make
                    // `Asset::render_*` disagree with them.
                    let dash = if direct { dash } else { None };
                    if !direct && stroke.dasharray().is_some() {
                        unsupported(
                            self.compatibility,
                            self.diagnostics,
                            DiagnosticCode::UnsupportedPaint,
                            "dashed strokes cannot be represented by a vector drawable or an ImageVector",
                            "Convert the dashed stroke to filled outlines.",
                        );
                    }
                    lower_paint(
                        stroke.paint(),
                        transform,
                        self.options,
                        self.compatibility,
                        self.diagnostics,
                    )
                    .and_then(|(paint, alpha, group_transform)| {
                        if group_transform.is_some() {
                            unsupported(
                                self.compatibility,
                                self.diagnostics,
                                DiagnosticCode::UnsupportedGradient,
                                // A stroke gradient that is not circular is a
                                // gradient inside a counter-scaled group, which
                                // neither a drawable nor an `ImageVector` can
                                // hold, and which the draw target does not
                                // write either.
                                if direct {
                                    "elliptical radial gradients on strokes are not supported by the draw target"
                                } else {
                                    "elliptical radial gradients on strokes cannot be represented by a vector drawable or an ImageVector"
                                },
                                "Make the stroke gradient circular, without skew or uneven scale.",
                            );
                            return None;
                        }
                        let scale = stroke_scale(transform, self.compatibility, self.diagnostics);
                        Some((
                            paint,
                            stroke.opacity().get() * inherited_alpha * alpha,
                            stroke.width().get() * scale,
                            map_cap(stroke.linecap()),
                            map_join(stroke.linejoin()),
                            stroke.miterlimit().get(),
                            // The pattern is in the path's own units, like the
                            // stroke width, and the geometry is written in
                            // output units, so it scales with them.
                            dash.map(|dash| Dash {
                                intervals: dash
                                    .intervals
                                    .iter()
                                    .map(|interval| interval * scale)
                                    .collect(),
                                offset: dash.offset * scale,
                            }),
                        ))
                    })
                });
                    if fill.is_some()
                        && stroke.is_some()
                        && path.paint_order() == usvg::PaintOrder::StrokeAndFill
                    {
                        unsupported(
                            self.compatibility,
                            self.diagnostics,
                            DiagnosticCode::UnsupportedPaint,
                            // Neither output has a paint order: a drawable and
                            // an `ImageVector` cannot hold one at all, and the
                            // draw target draws every fill before its stroke.
                            if direct {
                                "stroke-before-fill paint order is not supported by the draw target, which paints the fill first"
                            } else {
                                "stroke-before-fill paint order cannot be represented by a vector drawable or an ImageVector"
                            },
                            "Convert the stroke to a filled outline placed below the fill.",
                        );
                    }
                    // Only geometry that paints pixels counts toward content bounds:
                    // a fully transparent fill or stroke, or a zero-width stroke,
                    // is emitted for fidelity but renders nothing.
                    let fill_paints = fill.as_ref().is_some_and(|value| {
                        serializes_nonzero(value.1) && value.0.paints_pixels()
                    });
                    let stroke_paints = stroke.as_ref().is_some_and(|value| {
                        serializes_nonzero(value.1)
                            && serializes_nonzero(value.2)
                            && value.0.paints_pixels()
                    });
                    if stroke_paints {
                        include_bounds(
                            &mut self.metrics.content_bounds,
                            path.abs_stroke_bounding_box(),
                        );
                    } else if fill_paints {
                        include_bounds(&mut self.metrics.content_bounds, path.abs_bounding_box());
                    }
                    let (fill, fill_alpha, fill_rule, group_transform) = match fill {
                        Some((paint, alpha, rule, gt)) => (Some(paint), alpha, rule, gt),
                        None => (None, 1.0, FillRule::NonZero, None),
                    };
                    let (
                        stroke,
                        stroke_alpha,
                        stroke_width,
                        stroke_cap,
                        stroke_join,
                        stroke_miter,
                        dash,
                    ) = match stroke {
                        Some((paint, alpha, width, cap, join, miter, dash)) => {
                            (Some(paint), alpha, width, cap, join, miter, dash)
                        }
                        None => (None, 1.0, 0.0, LineCap::Butt, LineJoin::Miter, 4.0, None),
                    };
                    if let Some(gt) = group_transform {
                        let filled_path = VectorPath {
                            path_data: transformed_path_with(
                                path,
                                gt.inv_transform
                                    .pre_concat(base)
                                    .pre_concat(path.abs_transform()),
                            ),
                            fill,
                            fill_alpha,
                            fill_rule,
                            stroke: None,
                            stroke_alpha: 1.0,
                            stroke_width: 0.0,
                            stroke_cap: LineCap::Butt,
                            stroke_join: LineJoin::Miter,
                            stroke_miter: 4.0,
                            dash: None,
                        };
                        children.push(VectorNode::Group(VectorGroup {
                            pivot_x: gt.pivot_x,
                            pivot_y: gt.pivot_y,
                            rotation: gt.rotation,
                            scale_x: gt.scale_x,
                            scale_y: gt.scale_y,
                            translate_x: 0.0,
                            translate_y: 0.0,
                            ..VectorGroup::new(vec![VectorNode::Path(filled_path)])
                        }));
                        self.metrics.groups += 1;
                        if stroke.is_some() {
                            self.metrics.paths += 1;
                            self.metrics.path_commands += data.0.len();
                            children.push(VectorNode::Path(VectorPath {
                                path_data: data,
                                fill: None,
                                fill_alpha: 1.0,
                                fill_rule: FillRule::NonZero,
                                stroke,
                                stroke_alpha,
                                stroke_width,
                                stroke_cap,
                                stroke_join,
                                stroke_miter,
                                dash,
                            }));
                        }
                    } else {
                        children.push(VectorNode::Path(VectorPath {
                            path_data: data,
                            fill,
                            fill_alpha,
                            fill_rule,
                            stroke,
                            stroke_alpha,
                            stroke_width,
                            stroke_cap,
                            stroke_join,
                            stroke_miter,
                            dash,
                        }));
                    }
                }
                usvg::Node::Image(_) | usvg::Node::Text(_) => {
                    unsupported(
                        self.compatibility,
                        self.diagnostics,
                        DiagnosticCode::UnsupportedPaint,
                        "normalized SVG contains a non-vector node",
                        "Convert text to paths and replace images with vector geometry.",
                    );
                }
            }
        }
        let mut clips = Vec::new();
        // Everything this group's own clip path and mask are expressed in: the
        // transform that maps the group into the output, which is not the
        // group's own absolute transform when the group is inside a mask.
        let abs = base.pre_concat(group.abs_transform());
        if let Some(clip_path) = group.clip_path() {
            let lowered = if direct {
                lower_clip_paths(clip_path, abs)
            } else {
                lower_clip_path(clip_path, abs).map(|data| vec![VectorClip::intersect(data)])
            };
            match lowered {
                Some(paths) => {
                    self.metrics.path_commands += paths
                        .iter()
                        .map(|clip| clips_commands(std::slice::from_ref(clip)))
                        .sum::<usize>();
                    clips.extend(paths.into_iter().map(VectorNode::ClipPath));
                }
                None => unsupported(
                    self.compatibility,
                    self.diagnostics,
                    DiagnosticCode::UnsupportedClipPath,
                    // The draw target clips with any winding rule, unions a
                    // clip path's shapes, and nests clips, so what it cannot
                    // clip with is another effect inside the clip path.
                    if direct {
                        "clip path requires a mask, filter, blend mode, image, or nesting the draw target cannot clip with"
                    } else {
                        "clip path requires unsupported union, nesting, or even-odd semantics"
                    },
                    if direct {
                        "Flatten the clip path's contents to plain shapes in the source design tool."
                    } else {
                        "Merge the clip into one path with the nonzero fill rule."
                    },
                ),
            }
        }
        let mut mask = None;
        if let Some(source) = group.mask() {
            if direct {
                match lower_mask_for_draw(
                    source,
                    abs,
                    self.options,
                    self.compatibility,
                    self.diagnostics,
                    self.metrics,
                ) {
                    Some(LoweredMask::Clips(lowered)) => {
                        self.metrics.path_commands += clips_commands(&lowered);
                        clips.extend(lowered.into_iter().map(VectorNode::ClipPath));
                    }
                    Some(LoweredMask::Layer(lowered)) => {
                        // The mask's content is lowered through the visitor
                        // that counts it, paths and commands both; the region it
                        // is confined to is this target's own, and is the only
                        // geometry left to count here.
                        self.metrics.path_commands += lowered
                            .region
                            .iter()
                            .map(|data| data.0.len())
                            .sum::<usize>();
                        mask = Some(lowered);
                    }
                    None => unsupported(
                        self.compatibility,
                        self.diagnostics,
                        DiagnosticCode::UnsupportedMask,
                        "mask requires nesting or effects that cannot be drawn",
                        "Flatten the mask in the source design tool.",
                    ),
                }
            } else {
                match lower_mask(source, abs) {
                    Some(mask_clips) => {
                        self.metrics.path_commands += mask_clips
                            .iter()
                            .map(|path_data| path_data.0.len())
                            .sum::<usize>();
                        clips.extend(
                            mask_clips
                                .into_iter()
                                .map(|data| VectorNode::ClipPath(VectorClip::intersect(data))),
                        );
                    }
                    None => unsupported(
                        self.compatibility,
                        self.diagnostics,
                        DiagnosticCode::UnsupportedMask,
                        "mask requires alpha, luminance, subtraction, nesting, or effects VectorDrawable cannot represent",
                        "Replace alpha, grayscale, or subtractive masking with outlined geometry.",
                    ),
                }
            }
        }
        // SVG isolates a group that has a mask or a clip path: its content
        // composites into a layer of its own, so a blend mode inside it reads
        // the group's own backdrop rather than the artwork the group is drawn
        // over. A mask that lowers to clips leaves the group without a layer of
        // its own, so where a descendant would blend through, the isolation has
        // to be asked for in so many words.
        let clipped = !clips.is_empty();
        let isolate =
            direct && (group.isolate() || (clipped && blends_with_its_backdrop(&children)));
        let mut scoped = clips;
        scoped.extend(children);
        // A drawable scopes a group's children only when it has a clip path. The
        // draw target also needs a group where a drawable would have folded group
        // opacity into the shapes, or where the group blends, isolates, or masks.
        let layer = VectorGroup {
            alpha: if direct { group_alpha } else { 1.0 },
            blend_mode: if direct {
                map_blend_mode(group.blend_mode())
            } else {
                BlendMode::Normal
            },
            isolate,
            mask,
            ..VectorGroup::new(Vec::new())
        };
        // A group that paints nothing paints nothing, mask or not: a mask
        // scopes its own element, so one on an empty group has nothing to take
        // away and the layer would be empty.
        if !scoped.is_empty() {
            if layer.needs_layer() || matches!(scoped.first(), Some(VectorNode::ClipPath(_))) {
                output.push(VectorNode::Group(VectorGroup {
                    children: scoped,
                    ..layer
                }));
            } else {
                output.extend(scoped);
            }
        }
    }
}

/// Number of path commands a lowered mask's clips carry, for the metrics.
fn clips_commands(clips: &[VectorClip]) -> usize {
    clips
        .iter()
        .flat_map(|clip| &clip.shapes)
        .map(|shape| shape.data.0.len())
        .sum()
}

fn map_blend_mode(blend_mode: usvg::BlendMode) -> BlendMode {
    match blend_mode {
        usvg::BlendMode::Normal => BlendMode::Normal,
        usvg::BlendMode::Multiply => BlendMode::Multiply,
        usvg::BlendMode::Screen => BlendMode::Screen,
        usvg::BlendMode::Overlay => BlendMode::Overlay,
        usvg::BlendMode::Darken => BlendMode::Darken,
        usvg::BlendMode::Lighten => BlendMode::Lighten,
        usvg::BlendMode::ColorDodge => BlendMode::ColorDodge,
        usvg::BlendMode::ColorBurn => BlendMode::ColorBurn,
        usvg::BlendMode::HardLight => BlendMode::HardLight,
        usvg::BlendMode::SoftLight => BlendMode::SoftLight,
        usvg::BlendMode::Difference => BlendMode::Difference,
        usvg::BlendMode::Exclusion => BlendMode::Exclusion,
        usvg::BlendMode::Hue => BlendMode::Hue,
        usvg::BlendMode::Saturation => BlendMode::Saturation,
        usvg::BlendMode::Color => BlendMode::Color,
        usvg::BlendMode::Luminosity => BlendMode::Luminosity,
    }
}

fn lower_clip_path(clip_path: &usvg::ClipPath, target_transform: Transform) -> Option<PathData> {
    if clip_path.clip_path().is_some() {
        return None;
    }
    let mut paths = Vec::new();
    collect_clip_paths(clip_path.root(), &mut paths)?;
    if paths.len() != 1 {
        return None;
    }
    let path = paths[0];
    if path
        .fill()
        .is_some_and(|fill| fill.rule() == usvg::FillRule::EvenOdd)
    {
        return None;
    }
    let transform = target_transform
        .pre_concat(clip_path.transform())
        .pre_concat(path.abs_transform());
    Some(transformed_path_with(path, transform))
}

fn collect_clip_paths<'a>(group: &'a usvg::Group, paths: &mut Vec<&'a usvg::Path>) -> Option<()> {
    if group.clip_path().is_some()
        || group.mask().is_some()
        || !group.filters().is_empty()
        || group.blend_mode() != usvg::BlendMode::Normal
        || group.isolate()
    {
        return None;
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_clip_paths(group, paths)?,
            usvg::Node::Path(path) if path.is_visible() => paths.push(path),
            usvg::Node::Path(_) => {}
            usvg::Node::Image(_) | usvg::Node::Text(_) => return None,
        }
    }
    Some(())
}

/// Lower a clip path for a target that can clip with more than one nonzero
/// path: every child keeps its own winding rule, and a clip path nested in
/// another narrows the clip the outer one establishes.
fn lower_clip_paths(
    clip_path: &usvg::ClipPath,
    target_transform: Transform,
) -> Option<Vec<VectorClip>> {
    let mut paths = Vec::new();
    collect_clip_paths(clip_path.root(), &mut paths)?;
    if paths.is_empty() {
        return None;
    }
    let transform = target_transform.pre_concat(clip_path.transform());
    let mut clips = vec![VectorClip {
        shapes: paths
            .iter()
            .map(|path| ClipShape {
                data: transformed_path_with(path, transform.pre_concat(path.abs_transform())),
                fill_rule: path
                    .fill()
                    .map_or(FillRule::NonZero, |fill| map_fill_rule(fill.rule())),
            })
            .collect(),
        op: ClipOp::Intersect,
    }];
    // A clip path's own `clip-path` narrows it further.
    if let Some(inner) = clip_path.clip_path() {
        clips.extend(lower_clip_paths(inner, target_transform)?);
    }
    Some(clips)
}

/// How a mask lowers for a target that can draw it.
enum LoweredMask {
    /// A mask every shape of which is opaque and, when it is a luminance mask,
    /// black or white. Such a mask has no value between transparent and
    /// opaque, so it is exactly the region its shapes cover: clipping to it
    /// draws the SVG and needs no layer. Black shapes subtract, which is what
    /// `ClipOp.Difference` is for.
    Clips(Vec<VectorClip>),
    /// Anything else: drawn into a layer of its own and composited with
    /// `BlendMode.DstIn`, so that partial values survive.
    Layer(VectorMask),
}

/// Lower a mask for the draw target.
fn lower_mask_for_draw(
    mask: &usvg::Mask,
    target_transform: Transform,
    options: &mut LowerOptions,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
    metrics: &mut Metrics,
) -> Option<LoweredMask> {
    // A mask inside a mask would have to be composited into the mask layer
    // before the mask is applied, which nothing here expresses.
    if mask.mask().is_some() {
        return None;
    }
    let region = transformed_rect(
        mask.rect().left(),
        mask.rect().top(),
        mask.rect().right(),
        mask.rect().bottom(),
        target_transform,
    );
    let kind = match mask.kind() {
        usvg::MaskType::Alpha => MaskKind::Alpha,
        usvg::MaskType::Luminance => MaskKind::Luminance,
    };
    if let Some(shapes) = opaque_mask_shapes(mask, target_transform, kind) {
        let mut clips = vec![VectorClip::intersect(region)];
        for shape in shapes {
            clips.push(shape);
        }
        return Some(LoweredMask::Clips(clips));
    }
    let mut children = Vec::new();
    // The mask's content is drawn into a mask layer and read back as the mask,
    // so it is not painted content: its paths and commands are emitted and are
    // counted as such, but its geometry does not belong to the bounds of what
    // the drawing paints, which the paths it scopes already give.
    let painted = metrics.content_bounds.take();
    Lowering {
        options: &mut *options,
        compatibility: &mut *compatibility,
        diagnostics: &mut *diagnostics,
        metrics: &mut *metrics,
    }
    .visit_group(mask.root(), &mut children, 1.0, target_transform);
    metrics.content_bounds = painted;
    Some(LoweredMask::Layer(VectorMask {
        kind,
        children,
        region: Some(region),
    }))
}

/// Whether any layer of a node list composites with a mode that reads the
/// canvas under it, rather than only the content of the list itself.
///
/// A layer restored with a blend mode reads whatever is beneath it: for the
/// outermost list that is the caller's canvas, and for the content of a group
/// that is isolated it is the group's own backdrop. Both need a layer that
/// nothing blends through, which is what this says. A layer nested in another
/// layer is contained already, because its backdrop is that layer's content, so
/// only a blend mode whose enclosing groups are all plain counts.
pub(crate) fn blends_with_its_backdrop(nodes: &[VectorNode]) -> bool {
    nodes.iter().any(|node| match node {
        VectorNode::Group(group) if group.needs_layer() => group.blend_mode != BlendMode::Normal,
        VectorNode::Group(group) => blends_with_its_backdrop(&group.children),
        VectorNode::Path(_) | VectorNode::ClipPath(_) => false,
    })
}

/// The clip shapes a mask is exactly, when every shape in it is opaque.
///
/// An alpha mask reads its content's alpha, so an opaque shape keeps whatever
/// it covers. A luminance mask reads luminance, so a white shape keeps and a
/// black one hides; a shape drawn after a black one would cover it, and
/// clipping cannot put back what a subtraction took away, so a mask with white
/// over black is left to the layer. Any other colour, any partial opacity, a
/// stroke, or a group that composites on its own is not a mask of shapes at
/// all.
fn opaque_mask_shapes(
    mask: &usvg::Mask,
    target_transform: Transform,
    kind: MaskKind,
) -> Option<Vec<VectorClip>> {
    let mut paths = Vec::new();
    collect_mask_paths(mask.root(), &mut paths)?;
    if paths.is_empty() {
        return None;
    }
    let mut keeps = Vec::new();
    let mut cuts = Vec::new();
    let mut subtracted = false;
    for path in paths {
        let fill = path.fill()?;
        if path.stroke().is_some() || fill.opacity().get() != 1.0 {
            return None;
        }
        let shape = ClipShape {
            data: transformed_path_with(path, target_transform.pre_concat(path.abs_transform())),
            fill_rule: map_fill_rule(fill.rule()),
        };
        let keep = match (kind, fill.paint()) {
            // An alpha mask keeps whatever its content covers, so content that
            // is opaque everywhere is exactly a clip. A gradient that fades
            // through its stops' opacity has values between transparent and
            // opaque, which a clip cannot express, so only content whose stops
            // all paint at full opacity is a mask of shapes.
            (MaskKind::Alpha, usvg::Paint::Color(_)) => true,
            (MaskKind::Alpha, usvg::Paint::LinearGradient(gradient)) => gradient
                .stops()
                .iter()
                .all(|stop| stop.opacity().get() == 1.0),
            (MaskKind::Alpha, usvg::Paint::RadialGradient(gradient)) => gradient
                .stops()
                .iter()
                .all(|stop| stop.opacity().get() == 1.0),
            (MaskKind::Alpha, _) => return None,
            (MaskKind::Luminance, usvg::Paint::Color(color)) => {
                if *color == usvg::Color::white() {
                    true
                } else if *color == usvg::Color::black() {
                    false
                } else {
                    return None;
                }
            }
            (MaskKind::Luminance, _) => return None,
        };
        if keep {
            if subtracted {
                // A white shape over a black one is opaque where they overlap,
                // which a clip that has already subtracted cannot restore.
                return None;
            }
            keeps.push(shape);
        } else {
            subtracted = true;
            cuts.push(shape);
        }
    }
    if keeps.is_empty() {
        return None;
    }
    let mut clips = vec![VectorClip {
        shapes: keeps,
        op: ClipOp::Intersect,
    }];
    clips.extend(cuts.into_iter().map(|shape| VectorClip {
        shapes: vec![shape],
        op: ClipOp::Difference,
    }));
    Some(clips)
}

fn lower_mask(mask: &usvg::Mask, target_transform: Transform) -> Option<Vec<PathData>> {
    if mask.mask().is_some() || mask.kind() != usvg::MaskType::Luminance {
        return None;
    }
    let mut paths = Vec::new();
    collect_mask_paths(mask.root(), &mut paths)?;
    if paths.len() != 1 {
        return None;
    }
    let path = paths[0];
    let fill = path.fill()?;
    if fill.rule() == usvg::FillRule::EvenOdd
        || fill.opacity().get() != 1.0
        || path.stroke().is_some()
        || !matches!(fill.paint(), usvg::Paint::Color(color) if *color == usvg::Color::white())
    {
        return None;
    }

    let rect = mask.rect();
    let region = transformed_rect(
        rect.left(),
        rect.top(),
        rect.right(),
        rect.bottom(),
        target_transform,
    );
    let geometry = transformed_path_with(path, target_transform.pre_concat(path.abs_transform()));
    Some(vec![region, geometry])
}

fn collect_mask_paths<'a>(group: &'a usvg::Group, paths: &mut Vec<&'a usvg::Path>) -> Option<()> {
    if group.opacity().get() != 1.0
        || group.clip_path().is_some()
        || group.mask().is_some()
        || !group.filters().is_empty()
        || group.blend_mode() != usvg::BlendMode::Normal
        || group.isolate()
    {
        return None;
    }
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => collect_mask_paths(group, paths)?,
            usvg::Node::Path(path) if path.is_visible() => paths.push(path),
            usvg::Node::Path(_) => {}
            usvg::Node::Image(_) | usvg::Node::Text(_) => return None,
        }
    }
    Some(())
}

fn transformed_rect(
    left: f32,
    top: f32,
    right: f32,
    bottom: f32,
    transform: Transform,
) -> PathData {
    let top_left = mapped(transform, Point::from_xy(left, top));
    let top_right = mapped(transform, Point::from_xy(right, top));
    let bottom_right = mapped(transform, Point::from_xy(right, bottom));
    let bottom_left = mapped(transform, Point::from_xy(left, bottom));
    PathData(vec![
        PathCommand::Move(top_left.x, top_left.y),
        PathCommand::Line(top_right.x, top_right.y),
        PathCommand::Line(bottom_right.x, bottom_right.y),
        PathCommand::Line(bottom_left.x, bottom_left.y),
        PathCommand::Close,
    ])
}

fn visible_path_count(group: &usvg::Group) -> usize {
    group
        .children()
        .iter()
        .map(|node| match node {
            usvg::Node::Group(group) => visible_path_count(group),
            usvg::Node::Path(path) => usize::from(path.is_visible()),
            usvg::Node::Image(_) | usvg::Node::Text(_) => 1,
        })
        .sum()
}

fn contains_fill_and_stroke(group: &usvg::Group) -> bool {
    group.children().iter().any(|node| match node {
        usvg::Node::Group(group) => contains_fill_and_stroke(group),
        usvg::Node::Path(path) => path.fill().is_some() && path.stroke().is_some(),
        usvg::Node::Image(_) | usvg::Node::Text(_) => false,
    })
}

/// Lower an SVG paint for a path whose geometry is mapped by `transform`.
///
/// Returns the paint, an alpha multiplier the paint contributes to the path,
/// and an optional group transform required for elliptical radial gradients.
fn lower_paint(
    paint: &usvg::Paint,
    transform: Transform,
    options: &mut LowerOptions,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Paint, f32, Option<GroupTransform>)> {
    // Uniform alpha carries no gradient geometry in a notification silhouette.
    // Collapse before focal/transform checks, sharing the final whitening report.
    if let Some(whitening) = &mut options.notification {
        let gradient = match paint {
            usvg::Paint::LinearGradient(gradient) => Some((gradient.stops(), gradient.transform())),
            usvg::Paint::RadialGradient(gradient) => Some((gradient.stops(), gradient.transform())),
            _ => None,
        };
        if let Some((stops, gradient_transform)) = gradient {
            if let Some((paint, alpha)) = whitening.collapse(&gradient_stops(stops)) {
                return Some((paint, alpha, None));
            }
            // Ordinary radial lowering can fall back to its last stop for a
            // singular transform. That would erase a notification's alpha shape.
            if transform.pre_concat(gradient_transform).invert().is_none() {
                unsupported(
                    compatibility,
                    diagnostics,
                    DiagnosticCode::UnsupportedGradient,
                    "gradient transform is not invertible",
                    "Fix the gradient transform; a zero scale collapses the gradient.",
                );
                return None;
            }
        }
    }
    match paint {
        usvg::Paint::Color(color) => Some((Paint::Solid(rgb(*color)), 1.0, None)),
        usvg::Paint::LinearGradient(gradient) => {
            lower_linear_gradient(gradient, transform, compatibility, diagnostics)
                .map(|(paint, alpha)| (paint, alpha, None))
        }
        usvg::Paint::RadialGradient(gradient) => lower_radial_gradient(
            gradient,
            transform,
            options.allow_approximate,
            options.target == Target::Draw,
            compatibility,
            diagnostics,
        ),
        usvg::Paint::Pattern(_) => {
            unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::UnsupportedPaint,
                if options.target == Target::Draw {
                    "pattern paint is not supported by the draw target"
                } else {
                    "pattern paint cannot be represented by a vector drawable or an ImageVector"
                },
                "Replace the pattern with the shapes it repeats.",
            );
            None
        }
    }
}

fn rgb(color: usvg::Color) -> Color {
    Color(color.red, color.green, color.blue)
}

/// Re-express a linear gradient in viewport coordinates.
///
/// An affine map keeps a linear gradient linear, but the new end point is not
/// the mapped `x2, y2`. Under skew or non-uniform scale the gradient direction
/// follows the inverse transpose, so every line of constant color lands where
/// the SVG renders it.
fn lower_linear_gradient(
    gradient: &usvg::LinearGradient,
    transform: Transform,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Paint, f32)> {
    let transform = transform.pre_concat(gradient.transform());
    let stops = gradient_stops(gradient.stops());
    if stops.len() < 2 {
        return solid_from_last_stop(&stops);
    }
    let dx = gradient.x2() - gradient.x1();
    let dy = gradient.y2() - gradient.y1();
    let length_squared = dx * dx + dy * dy;
    if length_squared == 0.0 {
        // SVG paints a zero-length linear gradient with its last stop.
        return solid_from_last_stop(&stops);
    }
    let Some(inverse) = transform.invert() else {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedGradient,
            "gradient transform is not invertible",
            "Fix the gradient transform; a zero scale collapses the gradient.",
        );
        return None;
    };
    if !transform.is_identity() {
        require_normalization(compatibility, diagnostics);
    }
    let direction_x = (inverse.sx * dx + inverse.ky * dy) / length_squared;
    let direction_y = (inverse.kx * dx + inverse.sy * dy) / length_squared;
    let direction_squared = direction_x * direction_x + direction_y * direction_y;
    let start = mapped(transform, Point::from_xy(gradient.x1(), gradient.y1()));
    let mut axis_x = direction_x / direction_squared;
    let mut axis_y = direction_y / direction_squared;
    let length = axis_x.hypot(axis_y);
    if !length.is_finite() || length == 0.0 {
        // The transform is numerically singular for this axis; nothing better
        // than the last stop can be expressed.
        return solid_from_last_stop(&stops);
    }
    if length < MINIMUM_AXIS {
        // A nonzero axis shorter than the writer can express is still a hard
        // edge between the first and last stop colors. Lengthen it along its
        // own direction to the shortest distinctly serializable axis rather
        // than collapse it; the edge moves by less than a millionth of a unit.
        axis_x *= MINIMUM_AXIS / length;
        axis_y *= MINIMUM_AXIS / length;
    }
    let end_x = start.x + axis_x;
    let end_y = start.y + axis_y;
    Some((
        Paint::Linear(LinearGradient {
            start_x: start.x,
            start_y: start.y,
            end_x,
            end_y,
            stops,
            tile_mode: tile_mode(gradient.spread_method()),
        }),
        1.0,
    ))
}

/// Smallest gradient extent, in viewport units, that the XML writer's six
/// decimal places can express. Anything below it would serialize as zero.
pub const MINIMUM_EXTENT: f32 = 0.000_001;

/// Shortest linear-gradient axis whose end points serialize distinctly in
/// every direction: at this length the larger component is at least one
/// writer unit even on a diagonal.
const MINIMUM_AXIS: f32 = 2.0 * MINIMUM_EXTENT;

/// Smallest group scale that survives `optimize`'s three-decimal rounding as
/// nonzero. A group scaled to zero draws nothing at all.
const MINIMUM_GROUP_SCALE: f32 = 0.001;

/// Whether a path-level number survives XML serialization as nonzero. Path
/// alphas and stroke widths are floats in Android, so the writer's own
/// formatting is the only quantization that applies.
fn serializes_nonzero(value: f32) -> bool {
    crate::xml::number(value) != "0"
}

/// Re-express a radial gradient in viewport coordinates when Android can draw it.
///
/// Circular radial gradients are lowered directly. Elliptical radial gradients
/// (from non-uniform scale or skew) are emitted as a circular gradient inside
/// a group scaled by the radii ratio and rotated to match the ellipse, with
/// the path geometry counter-scaled.
fn lower_radial_gradient(
    gradient: &usvg::RadialGradient,
    transform: Transform,
    allow_approximate: bool,
    direct: bool,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Paint, f32, Option<GroupTransform>)> {
    let transform = transform.pre_concat(gradient.transform());
    let stops = gradient_stops(gradient.stops());
    if stops.len() < 2 {
        return solid_from_last_stop(&stops).map(|(p, a)| (p, a, None));
    }
    let radius = gradient.r().get();
    let focal_offset = (gradient.fx() - gradient.cx()).hypot(gradient.fy() - gradient.cy());
    let has_focal = focal_offset > 1.0e-4 * radius || gradient.fr().get() > 0.0;
    if has_focal {
        if allow_approximate {
            compatibility.worsen(Compatibility::Approximate);
            diagnostics.push(Diagnostic {
                code: DiagnosticCode::UnsupportedGradient,
                severity: Severity::Warning,
                message: if direct {
                    "radial gradient focal point or focal radius is not supported by the draw target; centered at (cx, cy)".to_owned()
                } else {
                    "radial gradient focal point or focal radius cannot be represented by a vector drawable or an ImageVector; centered at (cx, cy)".to_owned()
                },
                location: None,
                suggestion: Some("Keep the centered gradient or remove the focal point in the SVG.".to_owned()),
            });
        } else {
            unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::UnsupportedGradient,
                if direct {
                    "radial gradients with a focal point are not supported by the draw target"
                } else {
                    "radial gradients with a focal point cannot be represented by a vector drawable or an ImageVector"
                },
                "Pass --allow-approximate to center the focal point, or remove the focal point and focal radius so the gradient is centered.",
            );
            return None;
        }
    }
    if let Some(scale) = similarity_scale(transform) {
        if !transform.is_identity() {
            require_normalization(compatibility, diagnostics);
        }
        let center = mapped(transform, Point::from_xy(gradient.cx(), gradient.cy()));
        // Android requires a strictly positive radius; keep it representable.
        let radius = (radius * scale).max(MINIMUM_EXTENT);
        return Some((
            Paint::Radial(RadialGradient {
                center_x: center.x,
                center_y: center.y,
                radius,
                stops,
                tile_mode: tile_mode(gradient.spread_method()),
            }),
            1.0,
            None,
        ));
    }

    // Elliptical radial gradient: decompose into group scale, rotation, and pivot.
    require_normalization(compatibility, diagnostics);

    let center = mapped(transform, Point::from_xy(gradient.cx(), gradient.cy()));
    let sx = transform.sx;
    let ky = transform.ky;
    let kx = transform.kx;
    let sy = transform.sy;

    let e = sx * sx + kx * kx;
    let f = sx * ky + kx * sy;
    let g = ky * ky + sy * sy;

    let delta = ((e - g) * (e - g) + 4.0 * f * f).sqrt();
    let lambda1 = 0.5 * (e + g + delta);
    let lambda2 = (0.5 * (e + g - delta)).max(0.0);

    let sigma1 = lambda1.sqrt();
    let sigma2 = lambda2.sqrt();

    let a = (radius * sigma1).max(MINIMUM_EXTENT);
    let b = (radius * sigma2).max(MINIMUM_EXTENT);

    if a <= MINIMUM_EXTENT || b <= MINIMUM_EXTENT {
        return solid_from_last_stop(&stops).map(|(p, a)| (p, a, None));
    }

    let theta = 0.5 * (2.0 * f).atan2(e - g);
    let rotation_deg = theta.to_degrees();

    // `optimize` rounds group attributes to three decimals, so a ratio below
    // MINIMUM_GROUP_SCALE would round to zero, collapsing the group and
    // erasing the drawable.
    //
    // Clamping the scale alone would stretch the ellipse's short axis, which
    // is the axis all visible banding runs along. Deriving the radius from the
    // clamped scale keeps that short axis exactly `b` and instead shortens the
    // long axis, which at these eccentricities already extends far beyond the
    // viewport.
    let (scale_y, radius) = if b / a < MINIMUM_GROUP_SCALE {
        (MINIMUM_GROUP_SCALE, b / MINIMUM_GROUP_SCALE)
    } else {
        ((b / a).min(1.0), a)
    };

    let group_transform = Transform::from_translate(center.x, center.y)
        .pre_rotate(rotation_deg)
        .pre_scale(1.0, scale_y)
        .pre_translate(-center.x, -center.y);

    let Some(inv_transform) = group_transform.invert() else {
        return solid_from_last_stop(&stops).map(|(p, a)| (p, a, None));
    };

    Some((
        Paint::Radial(RadialGradient {
            center_x: center.x,
            center_y: center.y,
            radius,
            stops,
            tile_mode: tile_mode(gradient.spread_method()),
        }),
        1.0,
        Some(GroupTransform {
            pivot_x: center.x,
            pivot_y: center.y,
            rotation: rotation_deg,
            scale_x: 1.0,
            scale_y,
            inv_transform,
        }),
    ))
}

/// Android gradients need at least two colors; a gradient with fewer stops is
/// painted as the solid color of its last stop, as SVG specifies. `usvg`
/// already lowers these before they reach us, so this is a guard.
fn solid_from_last_stop(stops: &[GradientStop]) -> Option<(Paint, f32)> {
    let last = *stops.last()?;
    Some((Paint::Solid(last.color), last.alpha))
}

fn gradient_stops(stops: &[usvg::Stop]) -> Vec<GradientStop> {
    stops
        .iter()
        .map(|stop| GradientStop {
            offset: stop.offset().get(),
            color: rgb(stop.color()),
            alpha: stop.opacity().get(),
        })
        .collect()
}

fn tile_mode(spread: usvg::SpreadMethod) -> TileMode {
    match spread {
        usvg::SpreadMethod::Pad => TileMode::Clamp,
        usvg::SpreadMethod::Reflect => TileMode::Mirror,
        usvg::SpreadMethod::Repeat => TileMode::Repeat,
    }
}

/// Uniform scale factor of a transform made of rotation, reflection,
/// translation, and uniform scale. `None` for skew or non-uniform scale.
fn similarity_scale(transform: Transform) -> Option<f32> {
    let x = transform.sx.hypot(transform.ky);
    let y = transform.kx.hypot(transform.sy);
    let dot = transform.sx * transform.kx + transform.ky * transform.sy;
    let relative = 1.0e-4;
    (x > 0.0 && (x - y).abs() <= relative * x.max(y) && dot.abs() <= relative * x * y).then_some(x)
}

fn require_normalization(compatibility: &mut Compatibility, diagnostics: &mut Vec<Diagnostic>) {
    compatibility.worsen(Compatibility::ExactWithNormalization);
    if !diagnostics
        .iter()
        .any(|item| item.code.as_str() == DiagnosticCode::NormalizationRequired.as_str())
    {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::NormalizationRequired,
            severity: Severity::Info,
            message: "safe SVG normalization is required before lowering".to_owned(),
            location: None,
            suggestion: None,
        });
    }
}

/// Largest `android:width` or `android:height` Android lint accepts for a
/// vector icon before warning that drawing it is slow.
pub(crate) const RECOMMENDED_MAX_DP: f32 = 200.0;

/// Non-blocking warning for drawables larger than [`RECOMMENDED_MAX_DP`].
pub(crate) fn large_dimensions_warning(width_dp: f32, height_dp: f32) -> Option<Diagnostic> {
    (width_dp > RECOMMENDED_MAX_DP || height_dp > RECOMMENDED_MAX_DP).then(|| Diagnostic {
        code: DiagnosticCode::LargeDimensions,
        severity: Severity::Warning,
        message: format!(
            "{}×{}dp is larger than Android's recommended 200×200dp for vector icons, which slows drawing",
            crate::xml::number(width_dp),
            crate::xml::number(height_dp)
        ),
        location: None,
        suggestion: Some(
            "Reduce android:width and android:height and keep the viewport, or use a raster image for large artwork."
                .to_owned(),
        ),
    })
}

#[derive(Clone, Copy)]
enum Api24Feature {
    Gradient,
    EvenOddFill,
    MultipleClips,
}

/// The API level drawn Kotlin needs above Compose's own floor, when the artwork
/// asks for one.
///
/// Compose draws a `BlendMode` through `android.graphics.BlendMode` on API 29
/// and newer, and falls back to the `PorterDuff` mode that matches it below
/// that. Android's `PorterDuff` has no equivalent for most of the modes
/// `mix-blend-mode` names — only screen, overlay, darken, and lighten — so a
/// drawing that uses one of the others paints its group normally on API 28 and
/// below instead of blending it.
pub(crate) fn draw_api_level(drawable: &VectorDrawable) -> Option<u32> {
    blends_needing_api_29(&drawable.children).then_some(29)
}

/// The note that says which API level drawn Kotlin needs and what falls back.
pub(crate) fn draw_api_level_note(level: u32) -> Diagnostic {
    Diagnostic {
        code: DiagnosticCode::ApiLevelRequirement,
        severity: Severity::Info,
        message: format!("needs API {level} for mix-blend-mode"),
        location: None,
        suggestion: Some(format!(
            "Use it with minSdk {level} or higher; below that Compose draws the \
             group normally instead of blending it."
        )),
    }
}

fn blends_needing_api_29(nodes: &[VectorNode]) -> bool {
    nodes.iter().any(|node| match node {
        VectorNode::Group(group) => {
            falls_back_below_api_29(group.blend_mode)
                || group
                    .mask
                    .as_ref()
                    .is_some_and(|mask| blends_needing_api_29(&mask.children))
                || blends_needing_api_29(&group.children)
        }
        VectorNode::Path(_) | VectorNode::ClipPath(_) => false,
    })
}

/// Whether Compose draws a mode by falling back to `SrcOver` below API 29.
fn falls_back_below_api_29(blend_mode: BlendMode) -> bool {
    !matches!(
        blend_mode,
        BlendMode::Normal
            | BlendMode::Screen
            | BlendMode::Overlay
            | BlendMode::Darken
            | BlendMode::Lighten
    )
}

/// `VDT004` note naming what raises a drawable's minimum API to 24, or
/// `None` when it renders on API 21.
pub(crate) fn api_level_note(drawable: &VectorDrawable) -> Option<Diagnostic> {
    let features = drawable.api_24_features();
    if features.is_empty() {
        return None;
    }
    let (causes, fixes): (Vec<&str>, Vec<&str>) = features
        .iter()
        .map(|feature| match feature {
            Api24Feature::Gradient => ("gradients", "use solid colors instead of gradients"),
            Api24Feature::EvenOddFill => (
                "even-odd fills (android:fillType)",
                "draw holes with reversed path direction under the nonzero fill rule",
            ),
            Api24Feature::MultipleClips => (
                "more than one clip path",
                "combine the clips into a single clip path",
            ),
        })
        .unzip();
    Some(Diagnostic {
        code: DiagnosticCode::ApiLevelRequirement,
        severity: Severity::Info,
        message: format!("needs API 24 for {}", causes.join(", ")),
        location: None,
        suggestion: Some(format!(
            "Use it with minSdk 24 or higher; to support API 21, {}.",
            fixes.join("; ")
        )),
    })
}

/// Grow `bounds` to include the stroke bounding box of an emitted painted path.
fn include_bounds(bounds: &mut Option<Bounds>, rect: usvg::tiny_skia_path::Rect) {
    let next = Bounds {
        left: rect.left(),
        top: rect.top(),
        right: rect.right(),
        bottom: rect.bottom(),
    };
    *bounds = Some(match *bounds {
        None => next,
        Some(current) => Bounds {
            left: current.left.min(next.left),
            top: current.top.min(next.top),
            right: current.right.max(next.right),
            bottom: current.bottom.max(next.bottom),
        },
    });
}

/// Clamp accumulated bounds to the viewport; `None` when nothing remains.
fn clamp_bounds(bounds: Bounds, width: f32, height: f32) -> Option<Bounds> {
    let round = |value: f32| (value * 10_000.0).round() / 10_000.0;
    let left = round(bounds.left.max(0.0));
    let top = round(bounds.top.max(0.0));
    let right = round(bounds.right.min(width));
    let bottom = round(bounds.bottom.min(height));
    (right > left && bottom > top).then_some(Bounds {
        left,
        top,
        right,
        bottom,
    })
}

fn transformed_path_with(path: &usvg::Path, transform: Transform) -> PathData {
    PathData(
        path.data()
            .segments()
            .map(|segment| match segment {
                PathSegment::MoveTo(point) => {
                    let point = mapped(transform, point);
                    PathCommand::Move(point.x, point.y)
                }
                PathSegment::LineTo(point) => {
                    let point = mapped(transform, point);
                    PathCommand::Line(point.x, point.y)
                }
                PathSegment::QuadTo(control, point) => {
                    let control = mapped(transform, control);
                    let point = mapped(transform, point);
                    PathCommand::Quad(control.x, control.y, point.x, point.y)
                }
                PathSegment::CubicTo(a, b, point) => {
                    let a = mapped(transform, a);
                    let b = mapped(transform, b);
                    let point = mapped(transform, point);
                    PathCommand::Cubic(a.x, a.y, b.x, b.y, point.x, point.y)
                }
                PathSegment::Close => PathCommand::Close,
            })
            .collect(),
    )
}

fn mapped(transform: Transform, mut point: Point) -> Point {
    transform.map_point(&mut point);
    point
}

fn stroke_scale(
    transform: Transform,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
) -> f32 {
    let x = transform.sx.hypot(transform.ky);
    let y = transform.kx.hypot(transform.sy);
    let dot = transform.sx * transform.kx + transform.ky * transform.sy;
    // Relative to the scale, as in `similarity_scale`: an absolute tolerance
    // lets a skew or uneven scale through once a large viewBox is scaled
    // down to dp, such as a 960-unit icon drawn at 24dp.
    let relative = 1.0e-4;
    if (x - y).abs() > relative * x.max(y) || dot.abs() > relative * x * y {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedStrokeTransform,
            "a stroked path has a non-uniform or skew transform",
            "Convert the stroke to a filled outline, or remove the skew or uneven scale.",
        );
    }
    x
}

fn unsupported(
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagnosticCode,
    message: &str,
    suggestion: &str,
) {
    compatibility.worsen(Compatibility::Unsupported);
    if !diagnostics
        .iter()
        .any(|item| item.code.as_str() == code.as_str())
    {
        diagnostics.push(Diagnostic {
            code,
            severity: Severity::Error,
            message: message.to_owned(),
            location: None,
            suggestion: Some(suggestion.to_owned()),
        });
    }
}

fn map_fill_rule(rule: usvg::FillRule) -> FillRule {
    match rule {
        usvg::FillRule::NonZero => FillRule::NonZero,
        usvg::FillRule::EvenOdd => FillRule::EvenOdd,
    }
}

fn map_cap(cap: usvg::LineCap) -> LineCap {
    match cap {
        usvg::LineCap::Butt => LineCap::Butt,
        usvg::LineCap::Round => LineCap::Round,
        usvg::LineCap::Square => LineCap::Square,
    }
}

fn map_join(join: usvg::LineJoin) -> LineJoin {
    match join {
        usvg::LineJoin::Miter | usvg::LineJoin::MiterClip => LineJoin::Miter,
        usvg::LineJoin::Round => LineJoin::Round,
        usvg::LineJoin::Bevel => LineJoin::Bevel,
    }
}
