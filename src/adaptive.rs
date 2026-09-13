//! Adaptive launcher icon resources built from converted layer drawables.

use std::fmt::Write;

use crate::vector::{Paint, PathCommand, PathData, VectorDrawable, VectorNode};

/// Edge length in dp of every adaptive icon layer.
pub const ADAPTIVE_ICON_SIZE: f32 = 108.0;

/// Edge length in dp of the square that is never masked away by launchers.
pub const ADAPTIVE_ICON_SAFE_ZONE: f32 = 66.0;

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
