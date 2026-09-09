use crate::vector::{PathCommand, VectorDrawable};

pub fn optimize(drawable: &mut VectorDrawable) {
    for path in &mut drawable.children {
        for command in &mut path.path_data.0 {
            match command {
                PathCommand::Move(a, b) | PathCommand::Line(a, b) => round_all(&mut [a, b]),
                PathCommand::Quad(a, b, c, d) => round_all(&mut [a, b, c, d]),
                PathCommand::Cubic(a, b, c, d, e, f) => round_all(&mut [a, b, c, d, e, f]),
                PathCommand::Close => {}
            }
        }
        path.fill_alpha = round(path.fill_alpha);
        path.stroke_alpha = round(path.stroke_alpha);
        path.stroke_width = round(path.stroke_width);
        path.stroke_miter = round(path.stroke_miter);
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
