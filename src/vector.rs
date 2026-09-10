use usvg::tiny_skia_path::{PathSegment, Point, Transform};

use crate::analysis::{Compatibility, Diagnostic, DiagnosticCode, Metrics, Severity};

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
    pub fill: Option<Color>,
    pub fill_alpha: f32,
    pub fill_rule: FillRule,
    pub stroke: Option<Color>,
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
        if contains_even_odd(&self.children) {
            24
        } else {
            21
        }
    }
}

fn contains_even_odd(nodes: &[VectorNode]) -> bool {
    nodes.iter().any(|node| match node {
        VectorNode::Group(group) => contains_even_odd(&group.children),
        VectorNode::Path(path) => path.fill_rule == FillRule::EvenOdd,
        VectorNode::ClipPath(_) => false,
    })
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

    if metrics.gradients > 0 {
        unsupported(
            compatibility,
            diagnostics,
            DiagnosticCode::UnsupportedGradient,
            "gradients require Android complex-color lowering, which is not yet supported",
        );
    }
    let mut children = Vec::new();
    visit_group(
        tree.root(),
        &mut children,
        1.0,
        compatibility,
        diagnostics,
        metrics,
    );
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
                    color(fill.paint(), compatibility, diagnostics).map(|color| {
                        (
                            color,
                            fill.opacity().get() * inherited_alpha,
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
                    color(stroke.paint(), compatibility, diagnostics).map(|color| {
                        let scale = stroke_scale(path.abs_transform(), compatibility, diagnostics);
                        (
                            color,
                            stroke.opacity().get() * inherited_alpha,
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
                children.push(VectorNode::Path(VectorPath {
                    path_data: data,
                    fill: fill.map(|value| value.0),
                    fill_alpha: fill.map_or(1.0, |value| value.1),
                    fill_rule: fill.map_or(FillRule::NonZero, |value| value.2),
                    stroke: stroke.map(|value| value.0),
                    stroke_alpha: stroke.map_or(1.0, |value| value.1),
                    stroke_width: stroke.map_or(0.0, |value| value.2),
                    stroke_cap: stroke.map_or(LineCap::Butt, |value| value.3),
                    stroke_join: stroke.map_or(LineJoin::Miter, |value| value.4),
                    stroke_miter: stroke.map_or(4.0, |value| value.5),
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

fn color(
    paint: &usvg::Paint,
    compatibility: &mut Compatibility,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<Color> {
    match paint {
        usvg::Paint::Color(color) => Some(Color(color.red, color.green, color.blue)),
        _ => {
            unsupported(
                compatibility,
                diagnostics,
                DiagnosticCode::UnsupportedPaint,
                "non-solid paint cannot be represented by the current lowering profile",
            );
            None
        }
    }
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
