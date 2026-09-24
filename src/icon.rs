//! Icon kinds: what an SVG is being made into, and the findings each kind
//! adds once the artwork sits on its canvas.

use serde::Serialize;

use crate::analysis::{Bounds, Diagnostic, DiagnosticCode, Severity};
use crate::{Asset, Fit, FitMode, Flattening, Result, adaptive, notification};

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
    /// canvas, scaled to fit inside it.
    ///
    /// A background covers the canvas instead. Android crops that layer with
    /// launcher masks and shifts it for parallax either way, so a background
    /// that does not reach every edge shows through; covering is what makes
    /// non-square artwork usable there.
    pub fn default_fit(self) -> Fit {
        match self {
            Self::AdaptiveBackground => Fit::cover(self.canvas()),
            Self::Notification | Self::AdaptiveForeground => Fit::contain(self.canvas()),
        }
    }
}

/// Fraction of the canvas above which a notification icon is more likely a
/// solid plate than a silhouette.
const NOTIFICATION_PLATE_COVERAGE: f32 = 0.9;

/// Fraction of a background below which cropping is rounding, not a decision
/// worth reporting.
const BACKGROUND_CROP_NOTE: f32 = 0.005;

impl Asset {
    /// Turn this asset into an icon of `kind`, scaled uniformly and centered
    /// so the source viewport fits inside a `fit` dp square on the kind's
    /// canvas, and record what that found as diagnostics.
    ///
    /// A notification icon is flattened to white first, as
    /// [`Asset::to_notification_icon`] does, and the flattening is reported
    /// as a `VDT021` note. Artwork that paints almost the whole canvas
    /// gets a `VDT017` warning, and artwork with nothing painted a
    /// `VDT018` warning. An adaptive foreground whose content leaves the
    /// 66dp safe zone gets `VDT019`, an adaptive background that does not
    /// paint every pixel of the layer gets `VDT020`, and an adaptive
    /// background that [`FitMode::Cover`] scaled past the edges of the layer
    /// gets `VDT023` naming how much the layer cropped. None of these change
    /// the compatibility or the minimum API.
    pub fn to_icon(&mut self, kind: IconKind, fit: Fit) -> Result<()> {
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
                if let Some(reach) = self.painted_reach() {
                    let remedy = format!(
                        "Scale the artwork into the circle with --fit {} or smaller.",
                        crate::xml::number(adaptive::fit_for_reach(fit.size, reach))
                    );
                    self.analysis
                        .diagnostics
                        .extend(adaptive::safe_zone_finding(reach, "content", remedy));
                }
            }
            IconKind::AdaptiveBackground => {
                // The fit replaces the source viewport with the canvas, so
                // what it crops has to be measured before it runs.
                let cropped = fit.cropped(
                    self.analysis.metrics.viewport_width,
                    self.analysis.metrics.viewport_height,
                    adaptive::ADAPTIVE_ICON_SIZE,
                );
                self.fit_adaptive_layer(fit)?;
                if cropped > BACKGROUND_CROP_NOTE {
                    self.analysis.diagnostics.push(Diagnostic {
                        code: DiagnosticCode::BackgroundCropped,
                        severity: Severity::Info,
                        message: format!(
                            "scaled the background to cover the {}dp layer, which cropped {:.0}% \
                             of the artwork",
                            crate::xml::number(adaptive::ADAPTIVE_ICON_SIZE),
                            cropped * 100.0
                        ),
                        location: None,
                        suggestion: Some(
                            "Draw the background on a square canvas to control what the layer \
                             keeps, or use --background-fit contain to scale all of it in, which \
                             leaves part of the layer unpainted."
                                .to_owned(),
                        ),
                    });
                }
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
                        suggestion: Some(match fit.mode {
                            FitMode::Contain => "Use --background-fit cover to fill the layer \
                                                 with the artwork, draw it edge to edge on a \
                                                 square canvas with no transparent pixels, or use \
                                                 --background-color."
                                .to_owned(),
                            FitMode::Cover => "Draw the background edge to edge with no \
                                               transparent pixels, or use --background-color."
                                .to_owned(),
                        }),
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
pub(crate) fn span(bounds: Bounds) -> String {
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
