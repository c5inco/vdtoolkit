use crate::vector::{Paint, PathCommand, PathData, VectorDrawable, VectorNode};

pub fn optimize(drawable: &mut VectorDrawable) {
    optimize_nodes(&mut drawable.children);
}

fn optimize_nodes(nodes: &mut [VectorNode]) {
    for node in nodes {
        match node {
            VectorNode::Group(group) => {
                round_all(&mut [
                    &mut group.pivot_x,
                    &mut group.pivot_y,
                    &mut group.rotation,
                    &mut group.scale_x,
                    &mut group.scale_y,
                    &mut group.translate_x,
                    &mut group.translate_y,
                ]);
                optimize_nodes(&mut group.children);
            }
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
        Paint::Linear(gradient) => {
            // Rounding must not collapse the axis to a point, which would
            // change how Android paints the gradient.
            let (start_x, start_y) = (round(gradient.start_x), round(gradient.start_y));
            let (end_x, end_y) = (round(gradient.end_x), round(gradient.end_y));
            if (start_x, start_y) != (end_x, end_y) {
                gradient.start_x = start_x;
                gradient.start_y = start_y;
                gradient.end_x = end_x;
                gradient.end_y = end_y;
            }
        }
        Paint::Radial(gradient) => {
            round_all(&mut [&mut gradient.center_x, &mut gradient.center_y]);
            // Android requires a strictly positive radius.
            gradient.radius = round(gradient.radius).max(MINIMUM_RADIUS);
        }
    }
}

/// Smallest radius the three-decimal rounding can express.
const MINIMUM_RADIUS: f32 = 0.001;

fn round_all(values: &mut [&mut f32]) {
    for value in values {
        **value = round(**value);
    }
}

fn round(value: f32) -> f32 {
    (value * 1000.0).round() / 1000.0
}
