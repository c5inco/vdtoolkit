//! Notification icon drawables: a white silhouette on a 24dp canvas.
//!
//! Android draws a status bar and notification icon from its alpha channel
//! alone and tints it with the system color, so every paint is flattened to
//! opaque white and only opacity is kept.

use crate::vector::{Color, Paint, VectorDrawable, VectorNode};

/// Edge length in dp of a notification icon.
pub const NOTIFICATION_ICON_SIZE: f32 = 24.0;

/// Edge length in dp of the live area inside a 24dp system icon, leaving the
/// 2dp of padding Material system icons are drawn with.
pub const NOTIFICATION_ICON_LIVE_AREA: f32 = 20.0;

/// White, the only color a notification icon is drawn in.
const WHITE: Color = Color(255, 255, 255);

/// What flattening an icon to white changed.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Flattening {
    /// Distinct non-white colors that were replaced with white.
    pub colors: usize,
    /// Gradients that were flattened, either to white stops that keep their
    /// opacity or, when the opacity was uniform, to solid white.
    pub gradients: usize,
}

impl Flattening {
    /// Whether any paint was recolored.
    pub fn is_empty(self) -> bool {
        self.colors == 0 && self.gradients == 0
    }
}

/// Repaint every fill and stroke white, keeping opacity.
///
/// A gradient whose stops all share one opacity becomes solid white with that
/// opacity folded into the path's alpha, so an icon that only used gradients
/// for color no longer needs API 24.
pub(crate) fn whiten(drawable: &mut VectorDrawable) -> Flattening {
    let mut seen: Vec<Color> = Vec::new();
    let mut flattening = Flattening::default();
    whiten_nodes(&mut drawable.children, &mut seen, &mut flattening);
    flattening.colors = seen.len();
    flattening
}

fn whiten_nodes(nodes: &mut [VectorNode], seen: &mut Vec<Color>, flattening: &mut Flattening) {
    for node in nodes {
        match node {
            VectorNode::Group(group) => whiten_nodes(&mut group.children, seen, flattening),
            VectorNode::ClipPath(_) => {}
            VectorNode::Path(path) => {
                for (paint, alpha) in [
                    (&mut path.fill, &mut path.fill_alpha),
                    (&mut path.stroke, &mut path.stroke_alpha),
                ] {
                    if let Some(paint) = paint {
                        whiten_paint(paint, alpha, seen, flattening);
                    }
                }
            }
        }
    }
}

fn whiten_paint(
    paint: &mut Paint,
    alpha: &mut f32,
    seen: &mut Vec<Color>,
    flattening: &mut Flattening,
) {
    let stops = match paint {
        Paint::Solid(color) => {
            record(*color, seen);
            *paint = Paint::Solid(WHITE);
            return;
        }
        Paint::Linear(gradient) => &mut gradient.stops,
        Paint::Radial(gradient) => &mut gradient.stops,
    };
    flattening.gradients += 1;
    for stop in stops.iter() {
        record(stop.color, seen);
    }
    // Stops are serialized with an 8-bit alpha, so comparing the bytes is
    // exactly the test for a gradient that renders as one flat opacity.
    let uniform = stops.first().map(|first| first.alpha).filter(|_| {
        let byte = stops[0].alpha_byte();
        stops.iter().all(|stop| stop.alpha_byte() == byte)
    });
    match uniform {
        Some(opacity) => {
            *alpha = (*alpha * opacity).clamp(0.0, 1.0);
            *paint = Paint::Solid(WHITE);
        }
        None => {
            for stop in stops {
                stop.color = WHITE;
            }
        }
    }
}

fn record(color: Color, seen: &mut Vec<Color>) {
    let white = (color.0, color.1, color.2) == (WHITE.0, WHITE.1, WHITE.2);
    if !white && !seen.iter().any(|other| rgb(*other) == rgb(color)) {
        seen.push(color);
    }
}

fn rgb(color: Color) -> (u8, u8, u8) {
    (color.0, color.1, color.2)
}
