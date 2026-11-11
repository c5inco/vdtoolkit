use crate::vector::{Paint, PathCommand, PathData, VectorDrawable, VectorNode};

pub fn optimize(drawable: &mut VectorDrawable) {
    optimize_nodes(&mut drawable.children);
}

fn optimize_nodes(nodes: &mut [VectorNode]) {
    for node in nodes {
        match node {
            VectorNode::Group(group) => {
                // A group's opacity is one of the reasons it composites in a
                // layer of its own, and that layer is what a blend mode inside
                // the group reads its backdrop from. Rounding the last of an
                // opacity away would drop the layer and let the blend mode reach
                // the artwork behind the group, so a group whose opacity rounds
                // to one stays isolated: the number is shortened, and the group
                // still composites where it did.
                let alpha = round(group.alpha);
                if alpha == 1.0 && group.alpha != 1.0 {
                    group.isolate = true;
                }
                group.alpha = alpha;
                round_all(&mut [
                    &mut group.pivot_x,
                    &mut group.pivot_y,
                    &mut group.rotation,
                    &mut group.scale_x,
                    &mut group.scale_y,
                    &mut group.translate_x,
                    &mut group.translate_y,
                ]);
                if let Some(mask) = &mut group.mask {
                    if let Some(region) = &mut mask.region {
                        optimize_path_data(region);
                    }
                    optimize_nodes(&mut mask.children);
                }
                optimize_nodes(&mut group.children);
            }
            VectorNode::ClipPath(clip) => {
                for shape in &mut clip.shapes {
                    optimize_path_data(&mut shape.data);
                }
            }
            VectorNode::Path(path) => {
                optimize_path_data(&mut path.path_data);
                path.fill_alpha = round(path.fill_alpha);
                path.stroke_alpha = round(path.stroke_alpha);
                path.stroke_width = round(path.stroke_width);
                path.stroke_miter = round(path.stroke_miter);
                if let Some(dash) = &mut path.dash {
                    // Rounding must not change which intervals of the pattern are
                    // zero. A dash whose only "on" interval rounds to zero draws
                    // nothing where the SVG draws a faint dash, and one whose
                    // "off" interval does draws solid, so a pattern with an
                    // interval the rounding cannot express keeps its values, as a
                    // gradient's axis keeps its.
                    let rounded = dash
                        .intervals
                        .iter()
                        .map(|interval| round(*interval))
                        .collect::<Vec<f32>>();
                    let keeps_its_zeros = dash
                        .intervals
                        .iter()
                        .zip(&rounded)
                        .all(|(interval, rounded)| *interval == 0.0 || *rounded > 0.0);
                    if keeps_its_zeros {
                        dash.intervals = rounded;
                    }
                    dash.offset = round(dash.offset);
                }
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
