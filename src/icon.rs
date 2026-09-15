//! Icon kinds: what an SVG is being made into, and the findings each kind
//! adds once the artwork sits on its canvas.

use serde::Serialize;

use crate::analysis::{Bounds, Diagnostic, DiagnosticCode, Severity};
use crate::{Asset, Flattening, Result, adaptive, notification};

/// What an SVG is being turned into, which decides the canvas it is fitted
/// to and the findings that apply to the result.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IconKind {
    /// A white 24dp notification icon.
    Notification,
    /// The foreground or monochrome layer of a 108dp adaptive launcher icon,
    /// whose artwork should stay inside the 66dp safe zone.
    AdaptiveForeground,
    /// The background layer of a 108dp adaptive launcher icon, which should
    /// paint every pixel.
    AdaptiveBackground,
}

impl IconKind {
    /// Edge length in dp of the square canvas the kind is drawn on.
    pub fn canvas(self) -> f32 {
        match self {
            Self::Notification => notification::NOTIFICATION_ICON_SIZE,
            Self::AdaptiveForeground | Self::AdaptiveBackground => adaptive::ADAPTIVE_ICON_SIZE,
        }
    }

    /// The fit the generator commands use unless told otherwise: the whole
    /// canvas.
    pub fn default_fit(self) -> f32 {
        self.canvas()
    }
}

/// Fraction of the canvas above which a notification icon is more likely a
/// solid plate than a silhouette.
const NOTIFICATION_PLATE_COVERAGE: f32 = 0.9;

impl Asset {
    /// Turn this asset into an icon of `kind`, scaled uniformly and centered
    /// so the source viewport fits inside a `fit` dp square on the kind's
    /// canvas, and record what that found as diagnostics.
    ///
    /// A notification icon is flattened to white first, as
    /// [`Asset::to_notification_icon`] does, and the flattening is reported
    /// as an `SVGVD021` note. Artwork that paints almost the whole canvas
    /// gets an `SVGVD017` warning, and artwork with nothing painted an
    /// `SVGVD018` warning. An adaptive foreground whose content leaves the
    /// 66dp safe zone gets `SVGVD019`, and an adaptive background that does
    /// not paint every pixel of the layer gets `SVGVD020`. None of these
    /// change the compatibility or the minimum API.
    pub fn to_icon(&mut self, kind: IconKind, fit: f32) -> Result<()> {
        match kind {
            IconKind::Notification => {
                let flattening = self.to_notification_icon(fit)?;
                if !flattening.is_empty() {
                    self.analysis.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::PaintFlattened,
                        severity: Severity::Info,
                        message: format!(
                            "flattened {} to white; Android tints the alpha channel only",
                            flattened(flattening)
                        ),
                        location: None,
                        suggestion: None,
                    });
                }
                let coverage = self.painted_coverage();
                if coverage >= NOTIFICATION_PLATE_COVERAGE {
                    self.analysis.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::NotificationPlate,
                        severity: Severity::Warning,
                        message: format!(
                            "artwork paints {:.0}% of the {}dp canvas; a notification icon \
                             should be a silhouette on transparency, or the system tints it \
                             into a filled square",
                            coverage * 100.0,
                            crate::xml::number(notification::NOTIFICATION_ICON_SIZE)
                        ),
                        location: None,
                        suggestion: Some(
                            "Draw the icon as a silhouette on a transparent background; a \
                             plate cannot be tinted into a shape."
                                .to_owned(),
                        ),
                    });
                } else if coverage == 0.0 || self.analysis.metrics.content_bounds.is_none() {
                    // Content bounds are measured before clipping, so artwork
                    // clipped away entirely still has bounds; only rendering
                    // shows it is empty.
                    self.analysis.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::EmptyArtwork,
                        severity: Severity::Warning,
                        message: "no painted content, so the notification icon is invisible"
                            .to_owned(),
                        location: None,
                        suggestion: Some(
                            "Give the artwork a visible fill or stroke, and check that no clip or \
                             mask hides it."
                                .to_owned(),
                        ),
                    });
                }
            }
            IconKind::AdaptiveForeground => {
                self.fit_adaptive_layer(fit)?;
                if let Some(bounds) = self.outside_adaptive_safe_zone() {
                    self.analysis.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::OutsideSafeZone,
                        severity: Severity::Warning,
                        message: format!(
                            "content spans {}, outside the {}dp safe zone; launcher masks may \
                             hide it",
                            span(bounds),
                            crate::xml::number(adaptive::ADAPTIVE_ICON_SAFE_ZONE)
                        ),
                        location: None,
                        suggestion: Some(format!(
                            "Scale the artwork into the safe zone with --fit {} or smaller.",
                            crate::xml::number(adaptive::ADAPTIVE_ICON_SAFE_ZONE)
                        )),
                    });
                }
            }
            IconKind::AdaptiveBackground => {
                self.fit_adaptive_layer(fit)?;
                if !self.fills_adaptive_layer() {
                    let detail = match self.short_of_layer() {
                        Some(bounds) => {
                            format!("content spans {} and does not fill", span(bounds))
                        }
                        None if self.analysis.metrics.content_bounds.is_none() => {
                            "has no painted content, so it does not fill".to_owned()
                        }
                        None => "leaves unpainted pixels, from clipping, holes, or \
                                 transparent paint, in"
                            .to_owned(),
                    };
                    self.analysis.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::BackgroundGap,
                        severity: Severity::Warning,
                        message: format!(
                            "background {detail} the {}dp layer; uncovered areas show through \
                             launcher masks and parallax",
                            crate::xml::number(adaptive::ADAPTIVE_ICON_SIZE)
                        ),
                        location: None,
                        suggestion: Some(
                            "Draw the background edge to edge on a square canvas with no \
                             transparent pixels, or use --background-color."
                                .to_owned(),
                        ),
                    });
                }
            }
        }
        Ok(())
    }

    /// Content bounds when they stop short of some edge of the 108dp layer.
    fn short_of_layer(&self) -> Option<Bounds> {
        let far = adaptive::ADAPTIVE_ICON_SIZE - 1e-3;
        self.analysis.metrics.content_bounds.filter(|bounds| {
            bounds.left > 1e-3 || bounds.top > 1e-3 || bounds.right < far || bounds.bottom < far
        })
    }
}

/// What flattening changed, as `2 colors and 1 gradient`.
fn flattened(flattening: Flattening) -> String {
    let plural =
        |count: usize, noun: &str| format!("{count} {noun}{}", if count == 1 { "" } else { "s" });
    match (flattening.colors, flattening.gradients) {
        (colors, 0) => plural(colors, "color"),
        (0, gradients) => plural(gradients, "gradient"),
        (colors, gradients) => format!(
            "{} and {}",
            plural(colors, "color"),
            plural(gradients, "gradient")
        ),
    }
}

/// Bounds as `left..right × top..bottom` in dp.
fn span(bounds: Bounds) -> String {
    format!(
        "{}..{} × {}..{}dp",
        short(bounds.left),
        short(bounds.right),
        short(bounds.top),
        short(bounds.bottom)
    )
}

/// One-decimal dp value for messages.
fn short(value: f32) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").map(str::to_owned).unwrap_or(text)
}
