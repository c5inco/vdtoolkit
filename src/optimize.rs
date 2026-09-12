use crate::vector::{Paint, PathCommand, PathData, VectorDrawable, VectorNode};

pub fn optimize(drawable: &mut VectorDrawable) {
    optimize_nodes(&mut drawable.children);
}

fn optimize_nodes(nodes: &mut [VectorNode]) {
    for node in nodes {
        match node {
            VectorNode::Group(group) => optimize_nodes(&mut group.children),
            VectorNode::ClipPath(path_data) => optimize_path_data(path_data),
            VectorNode::Path(path) => {
                optimize_path_data(&mut path.path_data);
                path.fill_alpha = round(path.fill_alpha);
                path.stroke_alpha = round(path.stroke_alpha);
                path.stroke_width = round(path.stroke_width);
                path.stroke_miter = round(path.stroke_miter);
                for paint in [&mut path.fill, &mut path.stroke].into_iter().flatten() {
                    optimize_paint(paint);
                }
            }
        }
    }
}

fn optimize_path_data(path_data: &mut PathData) {
    for command in &mut path_data.0 {
        match command {
            PathCommand::Move(a, b) | PathCommand::Line(a, b) => round_all(&mut [a, b]),
            PathCommand::Quad(a, b, c, d) => round_all(&mut [a, b, c, d]),
            PathCommand::Cubic(a, b, c, d, e, f) => round_all(&mut [a, b, c, d, e, f]),
            PathCommand::Close => {}
        }
    }
}

fn optimize_paint(paint: &mut Paint) {
    match paint {
        Paint::Solid(_) => {}
        Paint::Linear(gradient) => round_all(&mut [
            &mut gradient.start_x,
            &mut gradient.start_y,
            &mut gradient.end_x,
            &mut gradient.end_y,
        ]),
        Paint::Radial(gradient) => round_all(&mut [
            &mut gradient.center_x,
            &mut gradient.center_y,
            &mut gradient.radius,
        ]),
    }
}

fn round_all(values: &mut [&mut f32]) {
    for value in values {
        **value = round(**value);
    }
}

fn round(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0
}
