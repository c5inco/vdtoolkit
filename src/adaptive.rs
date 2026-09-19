//! Adaptive launcher icon resources built from converted layer drawables.

use std::fmt::Write;

use serde::Serialize;

use crate::analysis::{Diagnostic, DiagnosticCode, Severity};
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

/// Radius in dp of the largest circle a launcher mask can show: the 72dp
/// visible window taken as a circle. Painted content past this is clipped.
pub const ADAPTIVE_ICON_MASK_RADIUS: f32 = ADAPTIVE_ICON_VISIBLE_DIAMETER / 2.0;

/// Radius in dp of the circle Android asks key content to stay inside, which
/// leaves room for masks that are not circles and for OEM variation.
pub const ADAPTIVE_ICON_SAFE_RADIUS: f32 = ADAPTIVE_ICON_SAFE_ZONE / 2.0;

/// Alpha above which a pixel counts as painted. An antialiased edge fades out
/// over a pixel or two, and a pixel this faint is not artwork anyone can see.
pub(crate) const PAINTED_ALPHA: u8 = 8;

/// How far past a radius painted content may reach before it is worth
/// reporting, in dp. An antialiased edge fades out past the shape it draws,
/// and a fraction of a dp is not the clipping these findings are about.
const REACH_TOLERANCE: f32 = 0.5;

/// How far a pixel of a `width` × `height` raster of the whole layer sits from
/// the layer's centre, in dp.
///
/// The mask is a circle, so distance from the centre is what decides whether
/// a pixel survives; a bounding box cannot answer that, because its corners
/// are further out than the artwork inside it.
pub(crate) fn reach_dp(x: u32, y: u32, width: u32, height: u32) -> f32 {
    let (width, height) = (width.max(1) as f32, height.max(1) as f32);
    let dx = (x as f32 + 0.5 - width / 2.0) / width * ADAPTIVE_ICON_SIZE;
    let dy = (y as f32 + 0.5 - height / 2.0) / height * ADAPTIVE_ICON_SIZE;
    dx.hypot(dy)
}

/// What to report about a foreground whose painted content reaches `reach` dp
/// from the centre, or `None` when it stays inside the circle Android asks
/// for.
///
/// Past [`ADAPTIVE_ICON_MASK_RADIUS`] the clipping is a fact about rendering,
/// so it is a warning. Between that and [`ADAPTIVE_ICON_SAFE_RADIUS`] it is a
/// note: a circular mask still shows it, but a mask that is not a circle may
/// not.
pub(crate) fn safe_zone_finding(reach: f32, subject: &str, remedy: String) -> Option<Diagnostic> {
    let (severity, message) = if reach > ADAPTIVE_ICON_MASK_RADIUS + REACH_TOLERANCE {
        (
            Severity::Warning,
            format!(
                "{subject} reaches {reach:.1}dp from the centre, past the {}dp a circular \
                 launcher mask shows, so it is clipped",
                crate::xml::number(ADAPTIVE_ICON_MASK_RADIUS)
            ),
        )
    } else if reach > ADAPTIVE_ICON_SAFE_RADIUS + REACH_TOLERANCE {
        (
            Severity::Info,
            format!(
                "{subject} reaches {reach:.1}dp from the centre, past the {}dp Android asks key \
                 content to stay inside; a circular mask still shows it, but other launcher \
                 masks may not",
                crate::xml::number(ADAPTIVE_ICON_SAFE_RADIUS)
            ),
        )
    } else {
        return None;
    };
    Some(Diagnostic {
        code: DiagnosticCode::OutsideSafeZone,
        severity,
        message,
        location: None,
        suggestion: Some(remedy),
    })
}

/// The fit that would bring artwork now reaching `reach` dp inside the safe
/// circle, rounded down to a whole dp.
pub(crate) fn fit_for_reach(fit: f32, reach: f32) -> f32 {
    (fit * ADAPTIVE_ICON_SAFE_RADIUS / reach).floor().max(1.0)
}

/// Edge length in dp of a legacy launcher icon for devices below API 26.
pub const LEGACY_ICON_SIZE: f32 = 48.0;

/// Diameter in dp of the circle keyline a legacy launcher icon fills, centered
/// in [`LEGACY_ICON_SIZE`].
pub const LEGACY_ICON_KEYLINE: f32 = 44.0;

/// Density buckets and pixel sizes for legacy launcher icon PNGs.
pub const LEGACY_ICON_DENSITIES: [(&str, u32); 5] = [
    ("mdpi", 48),
    ("hdpi", 72),
    ("xhdpi", 96),
    ("xxhdpi", 144),
    ("xxxhdpi", 192),
];

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

/// Whether artwork is scaled to fit inside the square it is placed in, or to
/// cover it.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FitMode {
    /// Scale until the whole source fits inside the square. Non-square
    /// artwork keeps all of its content and leaves bands of the square
    /// unpainted along its shorter side.
    #[default]
    Contain,
    /// Scale until the source covers the square. Non-square artwork overflows
    /// the canvas along its longer side, where the canvas crops it.
    Cover,
}

/// Where artwork is placed on an icon canvas: the centered square it is
/// scaled to, and whether it fits inside that square or covers it.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Fit {
    /// Edge length in dp of the centered square.
    pub size: f32,
    /// Whether the artwork fits inside the square or covers it.
    pub mode: FitMode,
}

impl Fit {
    /// Scale the whole source into a centered `size` dp square.
    pub const fn contain(size: f32) -> Self {
        Self {
            size,
            mode: FitMode::Contain,
        }
    }

    /// Scale the source until it covers a centered `size` dp square.
    pub const fn cover(size: f32) -> Self {
        Self {
            size,
            mode: FitMode::Cover,
        }
    }

    /// Uniform scale and the offsets that center the scaled source on a
    /// `canvas` dp square. Under [`FitMode::Cover`] the offsets are negative
    /// on the axis that overflows.
    pub(crate) fn placement(
        self,
        viewport_width: f32,
        viewport_height: f32,
        canvas: f32,
    ) -> (f32, f32, f32) {
        let extent = match self.mode {
            FitMode::Contain => viewport_width.max(viewport_height),
            FitMode::Cover => viewport_width.min(viewport_height),
        };
        let scale = self.size / extent;
        (
            scale,
            (canvas - viewport_width * scale) / 2.0,
            (canvas - viewport_height * scale) / 2.0,
        )
    }

    /// Fraction of the source's area that falls outside a `canvas` dp square,
    /// which the canvas crops away. Zero unless the placement overflows.
    pub(crate) fn cropped(self, viewport_width: f32, viewport_height: f32, canvas: f32) -> f32 {
        let (scale, ..) = self.placement(viewport_width, viewport_height, canvas);
        let (width, height) = (viewport_width * scale, viewport_height * scale);
        if !(width > 0.0 && height > 0.0) {
            return 0.0;
        }
        1.0 - (width.min(canvas) * height.min(canvas)) / (width * height)
    }
}

/// Move the drawable onto a square canvas, scaling its content uniformly so
/// the source viewport fits inside, or covers, a centered square of `fit`
/// units.
///
/// Uniform scaling and translation are applied to geometry, stroke widths, and
/// gradient coordinates, so rendering is unchanged apart from placement. Under
/// [`FitMode::Cover`] the canvas also crops whatever overflows it.
pub(crate) fn fit_square(drawable: &mut VectorDrawable, canvas: f32, fit: Fit) {
    let (scale, dx, dy) = fit.placement(drawable.viewport_width, drawable.viewport_height, canvas);
    transform_nodes(&mut drawable.children, scale, dx, dy);
    drawable.width_dp = canvas;
    drawable.height_dp = canvas;
    drawable.viewport_width = canvas;
    drawable.viewport_height = canvas;
}

fn transform_nodes(nodes: &mut [VectorNode], scale: f32, dx: f32, dy: f32) {
    for node in nodes {
        match node {
            VectorNode::Group(group) => {
                group.pivot_x = group.pivot_x * scale + dx;
                group.pivot_y = group.pivot_y * scale + dy;
                group.translate_x *= scale;
                group.translate_y *= scale;
                transform_nodes(&mut group.children, scale, dx, dy);
            }
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

/// Scale and offset from adaptive layer units to legacy icon units, which put
/// the 72dp visible circle on the 44dp keyline centered in 48dp.
pub(crate) fn legacy_mapping() -> (f32, f32) {
    let scale = LEGACY_ICON_KEYLINE / ADAPTIVE_ICON_VISIBLE_DIAMETER;
    (scale, (LEGACY_ICON_SIZE - ADAPTIVE_ICON_SIZE * scale) / 2.0)
}

/// Compose fitted background and foreground layers into one legacy icon under
/// a circular clip, the way launchers mask adaptive icons.
///
/// A layer with clip paths of its own is wrapped in a group so its clips
/// cannot reach the layer after it. Layers without clips are not wrapped,
/// which keeps the drawable at the single clip API 21 applies reliably.
pub(crate) fn legacy_icon(
    background: &VectorDrawable,
    foreground: &VectorDrawable,
) -> VectorDrawable {
    let (scale, offset) = legacy_mapping();
    let center = LEGACY_ICON_SIZE / 2.0;
    let mut children = vec![VectorNode::ClipPath(circle(
        center,
        center,
        LEGACY_ICON_KEYLINE / 2.0,
    ))];
    for layer in [background, foreground] {
        let mut nodes = layer.children.clone();
        transform_nodes(&mut nodes, scale, offset, offset);
        if contains_clip(&nodes) {
            children.push(VectorNode::Group(VectorGroup::new(nodes)));
        } else {
            children.extend(nodes);
        }
    }
    VectorDrawable {
        width_dp: LEGACY_ICON_SIZE,
        height_dp: LEGACY_ICON_SIZE,
        viewport_width: LEGACY_ICON_SIZE,
        viewport_height: LEGACY_ICON_SIZE,
        children,
    }
}

fn contains_clip(nodes: &[VectorNode]) -> bool {
    nodes.iter().any(|node| match node {
        VectorNode::Group(group) => contains_clip(&group.children),
        VectorNode::ClipPath(_) => true,
        VectorNode::Path(_) => false,
    })
}
