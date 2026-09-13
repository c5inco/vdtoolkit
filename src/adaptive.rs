//! Adaptive launcher icon resources built from converted layer drawables.

use std::fmt::Write;

use crate::vector::{
    Color, FillRule, LineCap, LineJoin, Paint, PathCommand, PathData, VectorDrawable, VectorGroup,
    VectorNode, VectorPath,
};

/// Edge length in dp of every adaptive icon layer.
pub const ADAPTIVE_ICON_SIZE: f32 = 108.0;

/// Edge length in dp of the square that is never masked away by launchers.
pub const ADAPTIVE_ICON_SAFE_ZONE: f32 = 66.0;

/// Diameter in dp of the circular area launchers show; the outer 18dp on each
/// side is reserved for masks and effects.
pub const ADAPTIVE_ICON_VISIBLE_DIAMETER: f32 = 72.0;

/// Edge length in dp of a legacy launcher icon for devices below API 26.
pub const LEGACY_ICON_SIZE: f32 = 48.0;

/// Write an `<adaptive-icon>` resource that references the given drawables.
///
/// Each argument is a resource reference such as `@drawable/ic_launcher_foreground`
/// or `@color/ic_launcher_background`.
pub fn adaptive_icon_xml(background: &str, foreground: &str, monochrome: Option<&str>) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n");
    xml.push_str("<adaptive-icon xmlns:android=\"http://schemas.android.com/apk/res/android\">\n");
    let _ = writeln!(xml, "    <background android:drawable=\"{background}\"/>");
    let _ = writeln!(xml, "    <foreground android:drawable=\"{foreground}\"/>");
    if let Some(monochrome) = monochrome {
        let _ = writeln!(xml, "    <monochrome android:drawable=\"{monochrome}\"/>");
    }
    xml.push_str("</adaptive-icon>\n");
    xml
}

/// Write a values resource declaring one solid color.
pub fn color_resource_xml(name: &str, color: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<resources>\n    <color name=\"{name}\">{color}</color>\n</resources>\n"
    )
}

/// Move the drawable onto a square canvas, scaling its content uniformly so
/// the source viewport fits inside a centered square of `fit` units.
///
/// Uniform scaling and translation are applied to geometry, stroke widths, and
/// gradient coordinates, so rendering is unchanged apart from placement.
pub(crate) fn fit_square(drawable: &mut VectorDrawable, canvas: f32, fit: f32) {
    let extent = drawable.viewport_width.max(drawable.viewport_height);
    let scale = fit / extent;
    let dx = (canvas - drawable.viewport_width * scale) / 2.0;
    let dy = (canvas - drawable.viewport_height * scale) / 2.0;
    transform_nodes(&mut drawable.children, scale, dx, dy);
    drawable.width_dp = canvas;
    drawable.height_dp = canvas;
    drawable.viewport_width = canvas;
    drawable.viewport_height = canvas;
}

fn transform_nodes(nodes: &mut [VectorNode], scale: f32, dx: f32, dy: f32) {
    for node in nodes {
        match node {
            VectorNode::Group(group) => transform_nodes(&mut group.children, scale, dx, dy),
            VectorNode::ClipPath(path_data) => transform_path_data(path_data, scale, dx, dy),
            VectorNode::Path(path) => {
                transform_path_data(&mut path.path_data, scale, dx, dy);
                path.stroke_width *= scale;
                for paint in [&mut path.fill, &mut path.stroke].into_iter().flatten() {
                    transform_paint(paint, scale, dx, dy);
                }
            }
        }
    }
}

fn transform_path_data(path_data: &mut PathData, scale: f32, dx: f32, dy: f32) {
    let point = |x: &mut f32, y: &mut f32| {
        *x = *x * scale + dx;
        *y = *y * scale + dy;
    };
    for command in &mut path_data.0 {
        match command {
            PathCommand::Move(x, y) | PathCommand::Line(x, y) => point(x, y),
            PathCommand::Quad(a, b, x, y) => {
                point(a, b);
                point(x, y);
            }
            PathCommand::Cubic(a, b, c, d, x, y) => {
                point(a, b);
                point(c, d);
                point(x, y);
            }
            PathCommand::Close => {}
        }
    }
}

fn transform_paint(paint: &mut Paint, scale: f32, dx: f32, dy: f32) {
    match paint {
        Paint::Solid(_) => {}
        Paint::Linear(gradient) => {
            gradient.start_x = gradient.start_x * scale + dx;
            gradient.start_y = gradient.start_y * scale + dy;
            gradient.end_x = gradient.end_x * scale + dx;
            gradient.end_y = gradient.end_y * scale + dy;
        }
        Paint::Radial(gradient) => {
            gradient.center_x = gradient.center_x * scale + dx;
            gradient.center_y = gradient.center_y * scale + dy;
            gradient.radius *= scale;
        }
    }
}

/// Parse `#RRGGBB` or `#AARRGGBB` into a color and an alpha in `0..=1`.
pub(crate) fn parse_color(text: &str) -> Option<(Color, f32)> {
    let digits = text.strip_prefix('#').unwrap_or(text);
    if !matches!(digits.len(), 6 | 8) || !digits.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let value = u32::from_str_radix(digits, 16).ok()?;
    let (alpha, rgb) = if digits.len() == 8 {
        ((value >> 24) as u8, value & 0x00FF_FFFF)
    } else {
        (u8::MAX, value)
    };
    let color = Color((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8);
    Some((color, f32::from(alpha) / 255.0))
}

/// A full-layer rectangle in one solid color.
pub(crate) fn solid_layer(color: Color, alpha: f32) -> VectorDrawable {
    let size = ADAPTIVE_ICON_SIZE;
    VectorDrawable {
        width_dp: size,
        height_dp: size,
        viewport_width: size,
        viewport_height: size,
        children: vec![VectorNode::Path(VectorPath {
            path_data: PathData(vec![
                PathCommand::Move(0.0, 0.0),
                PathCommand::Line(size, 0.0),
                PathCommand::Line(size, size),
                PathCommand::Line(0.0, size),
                PathCommand::Close,
            ]),
            fill: Some(Paint::Solid(color)),
            fill_alpha: alpha,
            fill_rule: FillRule::NonZero,
            stroke: None,
            stroke_alpha: 1.0,
            stroke_width: 1.0,
            stroke_cap: LineCap::Butt,
            stroke_join: LineJoin::Miter,
            stroke_miter: 4.0,
        })],
    }
}

/// A circle as four cubic Béziers, the closest VectorDrawable can come.
fn circle(cx: f32, cy: f32, r: f32) -> PathData {
    const KAPPA: f32 = 0.552_284_8;
    let k = r * KAPPA;
    PathData(vec![
        PathCommand::Move(cx, cy - r),
        PathCommand::Cubic(cx + k, cy - r, cx + r, cy - k, cx + r, cy),
        PathCommand::Cubic(cx + r, cy + k, cx + k, cy + r, cx, cy + r),
        PathCommand::Cubic(cx - k, cy + r, cx - r, cy + k, cx - r, cy),
        PathCommand::Cubic(cx - r, cy - k, cx - k, cy - r, cx, cy - r),
        PathCommand::Close,
    ])
}

/// Compose fitted background and foreground layers into one legacy icon:
/// both layers under a circular clip of the visible area, the way launchers
/// mask adaptive icons. Each layer is wrapped in a group so its own clip
/// paths cannot leak into the other.
pub(crate) fn legacy_icon(
    background: &VectorDrawable,
    foreground: &VectorDrawable,
) -> VectorDrawable {
    let center = ADAPTIVE_ICON_SIZE / 2.0;
    VectorDrawable {
        width_dp: LEGACY_ICON_SIZE,
        height_dp: LEGACY_ICON_SIZE,
        viewport_width: ADAPTIVE_ICON_SIZE,
        viewport_height: ADAPTIVE_ICON_SIZE,
        children: vec![
            VectorNode::ClipPath(circle(center, center, ADAPTIVE_ICON_VISIBLE_DIAMETER / 2.0)),
            VectorNode::Group(VectorGroup {
                children: background.children.clone(),
            }),
            VectorNode::Group(VectorGroup {
                children: foreground.children.clone(),
            }),
        ],
    }
}
