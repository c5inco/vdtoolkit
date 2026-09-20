//! Rasterize a VectorDrawable the way Android draws it, including legacy icons.

use tiny_skia as sk;

use crate::vector::{
    FillRule, LineCap, LineJoin, Paint, PathCommand, PathData, TileMode, VectorDrawable,
    VectorNode, VectorPath,
};

/// Render `drawable` scaled from its viewport to `width` × `height` pixels.
/// `None` when either size is zero.
pub(crate) fn render(drawable: &VectorDrawable, width: u32, height: u32) -> Option<sk::Pixmap> {
    let mut pixmap = sk::Pixmap::new(width, height)?;
    let transform = sk::Transform::from_scale(
        width as f32 / drawable.viewport_width,
        height as f32 / drawable.viewport_height,
    );
    draw_nodes(&mut pixmap, &drawable.children, transform, None);
    Some(pixmap)
}

/// Draw nodes in order. A `<clip-path>` narrows the clip for the siblings
/// after it and their descendants, intersecting any clip already in effect,
/// and the narrowing ends with the enclosing group.
fn draw_nodes(
    pixmap: &mut sk::Pixmap,
    nodes: &[VectorNode],
    transform: sk::Transform,
    inherited: Option<&sk::Mask>,
) {
    let mut local: Option<sk::Mask> = None;
    for node in nodes {
        let clip = local.as_ref().or(inherited);
        match node {
            VectorNode::ClipPath(data) => {
                let mask = narrow(pixmap, clip, data, transform);
                local = Some(mask);
            }
            VectorNode::Group(group) => {
                let group_transform = transform.pre_concat(group.transform());
                draw_nodes(pixmap, &group.children, group_transform, clip);
            }
            VectorNode::Path(path) => draw_path(pixmap, path, transform, clip),
        }
    }
}

fn narrow(
    pixmap: &sk::Pixmap,
    clip: Option<&sk::Mask>,
    data: &PathData,
    transform: sk::Transform,
) -> sk::Mask {
    let empty = || sk::Mask::new(pixmap.width(), pixmap.height()).expect("pixmap size is non-zero");
    // An empty clip path clips everything away. Clip paths have no fill type,
    // so Android always uses nonzero winding.
    let Some(path) = build(data) else {
        return empty();
    };
    match clip {
        Some(existing) => {
            let mut mask = existing.clone();
            mask.intersect_path(&path, sk::FillRule::Winding, true, transform);
            mask
        }
        None => {
            let mut mask = empty();
            mask.fill_path(&path, sk::FillRule::Winding, true, transform);
            mask
        }
    }
}

fn draw_path(
    pixmap: &mut sk::Pixmap,
    path: &VectorPath,
    transform: sk::Transform,
    clip: Option<&sk::Mask>,
) {
    let Some(geometry) = build(&path.path_data) else {
        return;
    };
    // Android fills, then strokes, each with its own alpha.
    if let Some(shader) = path
        .fill
        .as_ref()
        .and_then(|fill| shader(fill, path.fill_alpha))
    {
        let rule = match path.fill_rule {
            FillRule::NonZero => sk::FillRule::Winding,
            FillRule::EvenOdd => sk::FillRule::EvenOdd,
        };
        let paint = sk::Paint {
            shader,
            ..sk::Paint::default()
        };
        pixmap.fill_path(&geometry, &paint, rule, transform, clip);
    }
    if let Some(shader) = path
        .stroke
        .as_ref()
        .and_then(|stroke| shader(stroke, path.stroke_alpha))
    {
        let stroke = sk::Stroke {
            width: path.stroke_width,
            miter_limit: path.stroke_miter,
            line_cap: match path.stroke_cap {
                LineCap::Butt => sk::LineCap::Butt,
                LineCap::Round => sk::LineCap::Round,
                LineCap::Square => sk::LineCap::Square,
            },
            line_join: match path.stroke_join {
                LineJoin::Miter => sk::LineJoin::Miter,
                LineJoin::Round => sk::LineJoin::Round,
                LineJoin::Bevel => sk::LineJoin::Bevel,
            },
            dash: None,
        };
        let paint = sk::Paint {
            shader,
            ..sk::Paint::default()
        };
        pixmap.stroke_path(&geometry, &paint, &stroke, transform, clip);
    }
}

fn shader(paint: &Paint, alpha: f32) -> Option<sk::Shader<'static>> {
    let mode = |tile: TileMode| match tile {
        TileMode::Clamp => sk::SpreadMode::Pad,
        TileMode::Mirror => sk::SpreadMode::Reflect,
        TileMode::Repeat => sk::SpreadMode::Repeat,
    };
    // Stops are written with 8-bit alpha, so render with the same precision.
    let stops = |stops: &[crate::vector::GradientStop]| {
        stops
            .iter()
            .map(|stop| {
                sk::GradientStop::new(
                    stop.offset,
                    sk::Color::from_rgba8(
                        stop.color.0,
                        stop.color.1,
                        stop.color.2,
                        stop.alpha_byte(),
                    ),
                )
            })
            .collect()
    };
    let mut shader = match paint {
        Paint::Solid(color) => {
            sk::Shader::SolidColor(sk::Color::from_rgba8(color.0, color.1, color.2, u8::MAX))
        }
        Paint::Linear(gradient) => sk::LinearGradient::new(
            sk::Point::from_xy(gradient.start_x, gradient.start_y),
            sk::Point::from_xy(gradient.end_x, gradient.end_y),
            stops(&gradient.stops),
            mode(gradient.tile_mode),
            sk::Transform::identity(),
        )?,
        Paint::Radial(gradient) => {
            let center = sk::Point::from_xy(gradient.center_x, gradient.center_y);
            sk::RadialGradient::new(
                center,
                0.0,
                center,
                gradient.radius,
                stops(&gradient.stops),
                mode(gradient.tile_mode),
                sk::Transform::identity(),
            )?
        }
    };
    shader.apply_opacity(alpha.clamp(0.0, 1.0));
    Some(shader)
}

fn build(data: &PathData) -> Option<sk::Path> {
    let mut builder = sk::PathBuilder::new();
    for command in &data.0 {
        match *command {
            PathCommand::Move(x, y) => builder.move_to(x, y),
            PathCommand::Line(x, y) => builder.line_to(x, y),
            PathCommand::Quad(a, b, x, y) => builder.quad_to(a, b, x, y),
            PathCommand::Cubic(a, b, c, d, x, y) => builder.cubic_to(a, b, c, d, x, y),
            PathCommand::Close => builder.close(),
        }
    }
    builder.finish()
}
