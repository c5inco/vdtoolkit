//! Rasterize a drawable the way the target draws it, including legacy icons.
//!
//! The model carries everything a drawable cannot express — a group's opacity,
//! a dashed stroke, a blend mode, an arbitrary clip path, and a mask — so this
//! renderer composites groups in layers, as a Compose `Painter` does. A
//! drawable only ever fills in the subset it can write, so it renders exactly
//! as before.

use tiny_skia as sk;

use crate::vector::{
    BlendMode, ClipOp, FillRule, LineCap, LineJoin, MaskKind, Paint, PathCommand, PathData,
    TileMode, VectorClip, VectorDrawable, VectorGroup, VectorMask, VectorNode, VectorPath,
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
            VectorNode::ClipPath(clip_path) => {
                let mask = narrow(pixmap, clip, clip_path, transform);
                local = Some(mask);
            }
            VectorNode::Group(group) => draw_group(pixmap, group, transform, clip),
            VectorNode::Path(path) => draw_path(pixmap, path, transform, clip),
        }
    }
}

/// Draw a group, into a layer of its own when it carries an opacity, a blend
/// mode, an isolation, or a mask, so that its children composite together
/// before they reach what is behind the group.
fn draw_group(
    pixmap: &mut sk::Pixmap,
    group: &VectorGroup,
    transform: sk::Transform,
    clip: Option<&sk::Mask>,
) {
    let transform = transform.pre_concat(group.transform());
    if !group.needs_layer() {
        draw_nodes(pixmap, &group.children, transform, clip);
        return;
    }
    let mut layer = layer_like(pixmap);
    draw_nodes(&mut layer, &group.children, transform, clip);
    if let Some(mask) = &group.mask {
        apply_mask(&mut layer, mask, transform);
    }
    pixmap.draw_pixmap(
        0,
        0,
        layer.as_ref(),
        &sk::PixmapPaint {
            opacity: group.alpha.clamp(0.0, 1.0),
            blend_mode: blend_mode(group.blend_mode),
            ..sk::PixmapPaint::default()
        },
        sk::Transform::identity(),
        None,
    );
}

/// Composite a mask into the layer it scopes, the way `BlendMode.DstIn` does:
/// what the mask leaves transparent is taken away.
fn apply_mask(layer: &mut sk::Pixmap, mask: &VectorMask, transform: sk::Transform) {
    let mut content = layer_like(layer);
    draw_nodes(&mut content, &mask.children, transform, None);
    if let Some(region) = &mask.region {
        let mut region_mask =
            sk::Mask::new(layer.width(), layer.height()).expect("pixmap size is non-zero");
        if let Some(path) = build(region) {
            region_mask.fill_path(&path, sk::FillRule::Winding, true, transform);
        }
        content.apply_mask(&region_mask);
    }
    let mask = sk::Mask::from_pixmap(
        content.as_ref(),
        match mask.kind {
            MaskKind::Alpha => sk::MaskType::Alpha,
            MaskKind::Luminance => sk::MaskType::Luminance,
        },
    );
    layer.apply_mask(&mask);
}

fn layer_like(pixmap: &sk::Pixmap) -> sk::Pixmap {
    sk::Pixmap::new(pixmap.width(), pixmap.height()).expect("pixmap size is non-zero")
}

fn narrow(
    pixmap: &sk::Pixmap,
    clip: Option<&sk::Mask>,
    clip_path: &VectorClip,
    transform: sk::Transform,
) -> sk::Mask {
    let empty = || sk::Mask::new(pixmap.width(), pixmap.height()).expect("pixmap size is non-zero");
    // A clip path's shapes are unioned, not intersected: it clips to
    // everything its children cover. Filling them into one mask composites
    // them onto each other, which is that union.
    let mut shapes = empty();
    for shape in &clip_path.shapes {
        // A shape that cannot be built covers nothing, so it adds nothing to
        // the union of the shapes; a clip path whose every shape is like that
        // is empty, which clips everything away.
        let Some(path) = build(&shape.data) else {
            continue;
        };
        let rule = match shape.fill_rule {
            FillRule::NonZero => sk::FillRule::Winding,
            FillRule::EvenOdd => sk::FillRule::EvenOdd,
        };
        shapes.fill_path(&path, rule, true, transform);
    }
    let mut mask = match clip {
        Some(clip) => clip.clone(),
        // With no clip in effect yet, the clip path is the whole clip.
        None if clip_path.op == ClipOp::Intersect => return shapes,
        None => {
            let mut full = empty();
            full.invert();
            full
        }
    };
    let difference = clip_path.op == ClipOp::Difference;
    for (alpha, covered) in mask.data_mut().iter_mut().zip(shapes.data()) {
        *alpha = if difference {
            multiply_u8(*alpha, 255 - covered)
        } else {
            multiply_u8(*alpha, *covered)
        };
    }
    mask
}

/// `a * b / 255`, rounded, as the premultiplication tiny-skia does internally.
fn multiply_u8(a: u8, b: u8) -> u8 {
    let product = u32::from(a) * u32::from(b) + 128;
    ((product + (product >> 8)) >> 8) as u8
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
            dash: path
                .dash
                .as_ref()
                .and_then(|dash| sk::StrokeDash::new(dash.intervals.clone(), dash.offset)),
        };
        let paint = sk::Paint {
            shader,
            ..sk::Paint::default()
        };
        pixmap.stroke_path(&geometry, &paint, &stroke, transform, clip);
    }
}

/// A CSS compositing mode as the blend mode that composites with it.
fn blend_mode(mode: BlendMode) -> sk::BlendMode {
    match mode {
        BlendMode::Normal => sk::BlendMode::SourceOver,
        BlendMode::Multiply => sk::BlendMode::Multiply,
        BlendMode::Screen => sk::BlendMode::Screen,
        BlendMode::Overlay => sk::BlendMode::Overlay,
        BlendMode::Darken => sk::BlendMode::Darken,
        BlendMode::Lighten => sk::BlendMode::Lighten,
        BlendMode::ColorDodge => sk::BlendMode::ColorDodge,
        BlendMode::ColorBurn => sk::BlendMode::ColorBurn,
        BlendMode::HardLight => sk::BlendMode::HardLight,
        BlendMode::SoftLight => sk::BlendMode::SoftLight,
        BlendMode::Difference => sk::BlendMode::Difference,
        BlendMode::Exclusion => sk::BlendMode::Exclusion,
        BlendMode::Hue => sk::BlendMode::Hue,
        BlendMode::Saturation => sk::BlendMode::Saturation,
        BlendMode::Color => sk::BlendMode::Color,
        BlendMode::Luminosity => sk::BlendMode::Luminosity,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vector::ClipShape;

    #[test]
    fn a_shape_a_clip_cannot_build_does_not_take_the_union_with_it() {
        // A clip path clips to everything its shapes cover, so a shape that
        // cannot be built adds nothing to that union; a clip path whose every
        // shape is like that is empty, which clips everything away.
        let unbuildable = ClipShape {
            data: PathData(Vec::new()),
            fill_rule: FillRule::NonZero,
        };
        let rect = ClipShape {
            data: PathData(vec![
                PathCommand::Move(0.0, 0.0),
                PathCommand::Line(4.0, 0.0),
                PathCommand::Line(4.0, 4.0),
                PathCommand::Line(0.0, 4.0),
                PathCommand::Close,
            ]),
            fill_rule: FillRule::NonZero,
        };
        let clip = VectorClip {
            shapes: vec![unbuildable, rect],
            op: ClipOp::Intersect,
        };
        let pixmap = sk::Pixmap::new(8, 8).unwrap();
        let mask = narrow(&pixmap, None, &clip, sk::Transform::identity());
        assert_eq!(mask.data()[0], 255, "inside the shape that builds");
        assert_eq!(mask.data()[7], 0, "outside it");
    }
}
