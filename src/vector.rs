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
    ClipPath(PathData),
}

#[derive(Clone, Debug)]
pub struct VectorGroup {
    pub children: Vec<VectorNode>,
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
        if contains_even_odd(&self.children)
            || count_clip_paths(&self.children) > 1
            || self.uses_gradients()
        {
            24
        } else {
            21
        }
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

pub(crate) fn lower(
    tree: &usvg::Tree,
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
    visit_group(
        tree.root(),
        &mut children,
        1.0,
        compatibility,
        diagnostics,
        metrics,
    );
    metrics.content_bounds = metrics
        .content_bounds
        .and_then(|bounds| clamp_bounds(bounds, size.width(), size.height()));
    if compatibility.convertible() {
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

fn visit_group(
    group: &usvg::Group,
    output: &mut Vec<VectorNode>,
    inherited_alpha: f32,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
    metrics: &mut Metrics,
) {
    let group_alpha = group.opacity().get();
    if group_alpha < 1.0 && (visible_path_count(group) > 1 || contains_fill_and_stroke(group)) {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedPaint,
            "group opacity cannot be represented without changing overlap semantics",
        );
    }
    let inherited_alpha = inherited_alpha * group_alpha;
    if !group.filters().is_empty()
        || group.blend_mode() != usvg::BlendMode::Normal
        || group.isolate()
    {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedPaint,
            "the normalized tree requires unsupported masking, filtering, or compositing",
        );
    }
    let mut children = Vec::new();
    for node in group.children() {
        match node {
            usvg::Node::Group(group) => visit_group(
                group,
                &mut children,
                inherited_alpha,
                compatibility,
                diagnostics,
                metrics,
            ),
            usvg::Node::Path(path) => {
                if !path.is_visible() {
                    continue;
                }
                metrics.paths += 1;
                let data = transformed_path(path);
                metrics.path_commands += data.0.len();
                let fill = path.fill().and_then(|fill| {
                    lower_paint(
                        fill.paint(),
                        path.abs_transform(),
                        compatibility,
                        diagnostics,
                    )
                    .map(|(paint, alpha)| {
                        (
                            paint,
                            fill.opacity().get() * inherited_alpha * alpha,
                            map_fill_rule(fill.rule()),
                        )
                    })
                });
                let stroke = path.stroke().and_then(|stroke| {
                    if stroke.dasharray().is_some() {
                        unsupported(
                            compatibility,
                            diagnostics,
                            DiagnosticCode::UnsupportedPaint,
                            "dashed strokes cannot be represented by VectorDrawable",
                        );
                    }
                    if stroke.linejoin() == usvg::LineJoin::MiterClip {
                        unsupported(
                            compatibility,
                            diagnostics,
                            DiagnosticCode::UnsupportedPaint,
                            "miter-clip stroke joins cannot be represented by VectorDrawable",
                        );
                    }
                    lower_paint(
                        stroke.paint(),
                        path.abs_transform(),
                        compatibility,
                        diagnostics,
                    )
                    .map(|(paint, alpha)| {
                        let scale = stroke_scale(path.abs_transform(), compatibility, diagnostics);
                        (
                            paint,
                            stroke.opacity().get() * inherited_alpha * alpha,
                            stroke.width().get() * scale,
                            map_cap(stroke.linecap()),
                            map_join(stroke.linejoin()),
                            stroke.miterlimit().get(),
                        )
                    })
                });
                if fill.is_some()
                    && stroke.is_some()
                    && path.paint_order() == usvg::PaintOrder::StrokeAndFill
                {
                    unsupported(
                        compatibility,
                        diagnostics,
                        DiagnosticCode::UnsupportedPaint,
                        "stroke-before-fill paint order cannot be represented by VectorDrawable",
                    );
                }
                // Only geometry that paints pixels counts toward content bounds:
                // a fully transparent fill or stroke, or a zero-width stroke,
                // is emitted for fidelity but renders nothing.
                let fill_paints = fill.as_ref().is_some_and(|value| value.1 > 0.0);
                let stroke_paints = stroke
                    .as_ref()
                    .is_some_and(|value| value.1 > 0.0 && value.2 > 0.0);
                if stroke_paints {
                    include_bounds(&mut metrics.content_bounds, path.abs_stroke_bounding_box());
                } else if fill_paints {
                    include_bounds(&mut metrics.content_bounds, path.abs_bounding_box());
                }
                let (fill, fill_alpha, fill_rule) = match fill {
                    Some((paint, alpha, rule)) => (Some(paint), alpha, rule),
                    None => (None, 1.0, FillRule::NonZero),
                };
                let (stroke, stroke_alpha, stroke_width, stroke_cap, stroke_join, stroke_miter) =
                    match stroke {
                        Some((paint, alpha, width, cap, join, miter)) => {
                            (Some(paint), alpha, width, cap, join, miter)
                        }
                        None => (None, 1.0, 0.0, LineCap::Butt, LineJoin::Miter, 4.0),
                    };
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
                }));
            }
            usvg::Node::Image(_) | usvg::Node::Text(_) => {
                unsupported(
                    compatibility,
                    diagnostics,
                    DiagnosticCode::UnsupportedPaint,
                    "normalized SVG contains a non-vector node",
                );
            }
        }
    }
    let mut clips = Vec::new();
    if let Some(clip_path) = group.clip_path() {
        match lower_clip_path(clip_path, group.abs_transform()) {
            Some(path_data) => {
                metrics.path_commands += path_data.0.len();
                clips.push(path_data);
            }
            None => unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::UnsupportedClipPath,
                "clip path requires unsupported union, nesting, or even-odd semantics",
            ),
        }
    }
    if let Some(mask) = group.mask() {
        match lower_mask(mask, group.abs_transform()) {
            Some(mask_clips) => {
                metrics.path_commands += mask_clips
                    .iter()
                    .map(|path_data| path_data.0.len())
                    .sum::<usize>();
                clips.extend(mask_clips);
            }
            None => unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::UnsupportedMask,
                "mask requires alpha, luminance, subtraction, nesting, or effects VectorDrawable cannot represent",
            ),
        }
    }
    if clips.is_empty() {
        output.extend(children);
    } else {
        let mut scoped = clips
            .into_iter()
            .map(VectorNode::ClipPath)
            .collect::<Vec<_>>();
        scoped.extend(children);
        output.push(VectorNode::Group(VectorGroup { children: scoped }));
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
/// Returns the paint and an alpha multiplier the paint contributes to the path.
fn lower_paint(
    paint: &usvg::Paint,
    transform: Transform,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Paint, f32)> {
    match paint {
        usvg::Paint::Color(color) => Some((Paint::Solid(rgb(*color)), 1.0)),
        usvg::Paint::LinearGradient(gradient) => {
            lower_linear_gradient(gradient, transform, compatibility, diagnostics)
        }
        usvg::Paint::RadialGradient(gradient) => {
            lower_radial_gradient(gradient, transform, compatibility, diagnostics)
        }
        usvg::Paint::Pattern(_) => {
            unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::UnsupportedPaint,
                "pattern paint cannot be represented by VectorDrawable",
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
    if length_squared <= f32::EPSILON {
        // SVG paints a zero-length linear gradient with its last stop.
        return solid_from_last_stop(&stops);
    }
    let Some(inverse) = transform.invert() else {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedGradient,
            "gradient transform is not invertible",
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
    Some((
        Paint::Linear(LinearGradient {
            start_x: start.x,
            start_y: start.y,
            end_x: start.x + direction_x / direction_squared,
            end_y: start.y + direction_y / direction_squared,
            stops,
            tile_mode: tile_mode(gradient.spread_method()),
        }),
        1.0,
    ))
}

/// Re-express a radial gradient in viewport coordinates when Android can draw it.
///
/// VectorDrawable radial gradients are circles with a center and radius only,
/// so a focal point, a focal radius, or a transform that would turn the circle
/// into an ellipse is rejected.
fn lower_radial_gradient(
    gradient: &usvg::RadialGradient,
    transform: Transform,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<(Paint, f32)> {
    let transform = transform.pre_concat(gradient.transform());
    let stops = gradient_stops(gradient.stops());
    if stops.len() < 2 {
        return solid_from_last_stop(&stops);
    }
    let radius = gradient.r().get();
    let focal_offset = (gradient.fx() - gradient.cx()).hypot(gradient.fy() - gradient.cy());
    if focal_offset > 1.0e-4 * radius || gradient.fr().get() > 0.0 {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedGradient,
            "radial gradients with a focal point cannot be represented by VectorDrawable",
        );
        return None;
    }
    let Some(scale) = similarity_scale(transform) else {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedGradient,
            "elliptical or skewed radial gradients cannot be represented by VectorDrawable",
        );
        return None;
    };
    if !transform.is_identity() {
        require_normalization(compatibility, diagnostics);
    }
    let center = mapped(transform, Point::from_xy(gradient.cx(), gradient.cy()));
    Some((
        Paint::Radial(RadialGradient {
            center_x: center.x,
            center_y: center.y,
            radius: radius * scale,
            stops,
            tile_mode: tile_mode(gradient.spread_method()),
        }),
        1.0,
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

fn transformed_path(path: &usvg::Path) -> PathData {
    transformed_path_with(path, path.abs_transform())
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
    let epsilon = 1.0e-4;
    if (x - y).abs() > epsilon || dot.abs() > epsilon {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedStrokeTransform,
            "a stroked path has a non-uniform or skew transform",
        );
    }
    x
}

fn unsupported(
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
    code: DiagnosticCode,
    message: &str,
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
            suggestion: None,
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
