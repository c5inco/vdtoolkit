//! Experimental Rust embedding API for SVG compatibility analysis and
//! VectorDrawable conversion.
//!
//! The command-line interface is the primary V1 product. This API intentionally
//! exposes only complete analysis and conversion results; its compatibility is
//! not guaranteed across `0.x` releases.
//!
//! # Example
//!
//! ```
//! let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
//!     <path d="M2 2H22V22H2Z" fill="#123456"/>
//! </svg>"##;
//! let asset = vdtoolkit::convert(source)?;
//! assert_eq!(asset.analysis.minimum_api, Some(21));
//! assert!(asset.to_xml().contains("<vector"));
//! # Ok::<(), vdtoolkit::Error>(())
//! ```

mod adaptive;
mod analysis;
mod error;
mod optimize;
mod svg;
mod vector;
mod xml;

use std::path::Path;

pub use adaptive::{
    ADAPTIVE_ICON_SAFE_ZONE, ADAPTIVE_ICON_SIZE, adaptive_icon_xml, color_resource_xml,
};
pub use analysis::{
    Analysis, Bounds, Compatibility, Diagnostic, DiagnosticCode, ElementLocation, Metrics, Severity,
};
pub use error::{Error, Result};

/// A converted SVG and its compatibility analysis.
#[derive(Clone, Debug)]
pub struct Asset {
    drawable: vector::VectorDrawable,
    /// Compatibility, minimum Android API, diagnostics, and source metrics.
    pub analysis: Analysis,
}

impl Asset {
    /// Serialize this asset as deterministic Android VectorDrawable XML.
    pub fn to_xml(&self) -> String {
        xml::write(&self.drawable)
    }

    /// Safely reduce numeric precision in the generated drawable.
    pub fn optimize(&mut self) {
        optimize::optimize(&mut self.drawable);
        self.analysis.metrics.estimated_xml_bytes = self.to_xml().len();
    }

    /// Scale `android:width` and `android:height` down so neither exceeds
    /// `max_dp`, keeping the viewport. Returns whether the size changed.
    ///
    /// The larger side lands exactly on the cap and the other rounds to a whole
    /// dp, so the aspect ratio can shift by at most half a dp.
    pub fn fit_within(&mut self, max_dp: f32) -> bool {
        let drawable = &mut self.drawable;
        let (width, height) = (drawable.width_dp, drawable.height_dp);
        if max_dp.is_nan() || max_dp <= 0.0 || width.max(height) <= max_dp {
            return false;
        }
        let scaled = |side: f32| (side * max_dp / width.max(height)).round().max(1.0);
        if width >= height {
            drawable.width_dp = max_dp;
            drawable.height_dp = scaled(height);
        } else {
            drawable.width_dp = scaled(width);
            drawable.height_dp = max_dp;
        }
        self.analysis.metrics.width = drawable.width_dp;
        self.analysis.metrics.height = drawable.height_dp;
        self.analysis.diagnostics.retain(|diagnostic| {
            diagnostic.code.as_str() != DiagnosticCode::LargeDimensions.as_str()
        });
        self.analysis
            .diagnostics
            .extend(vector::large_dimensions_warning(
                drawable.width_dp,
                drawable.height_dp,
            ));
        self.analysis.metrics.estimated_xml_bytes = self.to_xml().len();
        true
    }

    /// Turn this asset into an adaptive icon layer: a 108dp square drawable
    /// whose content is scaled uniformly and centered so the source viewport
    /// fits inside a `fit` dp square.
    ///
    /// Use [`ADAPTIVE_ICON_SIZE`] to fill the whole layer and
    /// [`ADAPTIVE_ICON_SAFE_ZONE`] to keep artwork inside the area no launcher
    /// mask hides. Rendering is unchanged apart from placement, so the
    /// compatibility and minimum API are preserved.
    pub fn fit_adaptive_layer(&mut self, fit: f32) -> Result<()> {
        if !(fit > 0.0 && fit <= adaptive::ADAPTIVE_ICON_SIZE) {
            return Err(Error::InvalidInput(format!(
                "adaptive layer fit must be between 0 and {} dp, got {fit}",
                adaptive::ADAPTIVE_ICON_SIZE
            )));
        }
        let metrics = &mut self.analysis.metrics;
        let extent = metrics.viewport_width.max(metrics.viewport_height);
        let scale = fit / extent;
        let dx = (adaptive::ADAPTIVE_ICON_SIZE - metrics.viewport_width * scale) / 2.0;
        let dy = (adaptive::ADAPTIVE_ICON_SIZE - metrics.viewport_height * scale) / 2.0;
        adaptive::fit_square(&mut self.drawable, adaptive::ADAPTIVE_ICON_SIZE, fit);
        metrics.width = adaptive::ADAPTIVE_ICON_SIZE;
        metrics.height = adaptive::ADAPTIVE_ICON_SIZE;
        metrics.viewport_width = adaptive::ADAPTIVE_ICON_SIZE;
        metrics.viewport_height = adaptive::ADAPTIVE_ICON_SIZE;
        if let Some(bounds) = &mut metrics.content_bounds {
            bounds.left = bounds.left * scale + dx;
            bounds.top = bounds.top * scale + dy;
            bounds.right = bounds.right * scale + dx;
            bounds.bottom = bounds.bottom * scale + dy;
        }
        self.analysis.metrics.estimated_xml_bytes = self.to_xml().len();
        Ok(())
    }

    /// Content bounds of a fitted adaptive layer that reach outside the
    /// centered 66dp safe zone, which launcher masks may hide. `None` when
    /// the content stays inside or there is no painted content.
    pub fn outside_adaptive_safe_zone(&self) -> Option<Bounds> {
        let bounds = self.analysis.metrics.content_bounds?;
        let inset = (adaptive::ADAPTIVE_ICON_SIZE - adaptive::ADAPTIVE_ICON_SAFE_ZONE) / 2.0;
        let (low, high) = (inset - 1e-3, adaptive::ADAPTIVE_ICON_SIZE - inset + 1e-3);
        let outside =
            bounds.left < low || bounds.top < low || bounds.right > high || bounds.bottom > high;
        outside.then_some(bounds)
    }

    /// A 108dp adaptive icon layer filled with one solid `#RRGGBB` or
    /// `#AARRGGBB` color, for composing a legacy icon over a color background.
    pub fn solid_adaptive_layer(color: &str) -> Result<Asset> {
        let (rgb, alpha) = adaptive::parse_color(color).ok_or_else(|| {
            Error::InvalidInput(format!("color must be #RRGGBB or #AARRGGBB, got {color:?}"))
        })?;
        Ok(Self::synthesized(adaptive::solid_layer(rgb, alpha), &[]))
    }

    /// A legacy launcher icon for devices below API 26: the fitted
    /// `background` and `foreground` layers composed under a circular clip of
    /// the 72dp area launchers show, at 48dp. Fit both layers with
    /// [`Asset::fit_adaptive_layer`] first.
    pub fn legacy_launcher_icon(background: &Asset, foreground: &Asset) -> Asset {
        Self::synthesized(
            adaptive::legacy_icon(&background.drawable, &foreground.drawable),
            &[background, foreground],
        )
    }

    fn synthesized(drawable: vector::VectorDrawable, layers: &[&Asset]) -> Asset {
        let mut metrics = Metrics {
            width: drawable.width_dp,
            height: drawable.height_dp,
            viewport_width: drawable.viewport_width,
            viewport_height: drawable.viewport_height,
            ..Metrics::default()
        };
        let mut compatibility = Compatibility::Exact;
        for layer in layers {
            let source = &layer.analysis.metrics;
            metrics.paths += source.paths;
            metrics.path_commands += source.path_commands;
            metrics.groups += source.groups + 1;
            metrics.gradients += source.gradients;
            metrics.clip_paths += source.clip_paths;
            if let Some(bounds) = source.content_bounds {
                metrics.content_bounds = Some(match metrics.content_bounds {
                    None => bounds,
                    Some(union) => Bounds {
                        left: union.left.min(bounds.left),
                        top: union.top.min(bounds.top),
                        right: union.right.max(bounds.right),
                        bottom: union.bottom.max(bounds.bottom),
                    },
                });
            }
            compatibility.worsen(layer.analysis.compatibility);
        }
        if layers.is_empty() {
            metrics.paths = 1;
            metrics.path_commands = 5;
            metrics.content_bounds = Some(Bounds {
                left: 0.0,
                top: 0.0,
                right: drawable.viewport_width,
                bottom: drawable.viewport_height,
            });
        } else {
            metrics.clip_paths += 1;
            metrics.path_commands += 6;
        }
        let minimum_api = Some(drawable.minimum_api());
        let mut asset = Asset {
            drawable,
            analysis: Analysis {
                compatibility,
                minimum_api,
                diagnostics: Vec::new(),
                metrics,
            },
        };
        asset.analysis.metrics.estimated_xml_bytes = asset.to_xml().len();
        asset
    }
}

/// Analyze an SVG file without converting it.
pub fn analyze_file(path: &Path) -> Result<Analysis> {
    let source = std::fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    analyze(&source)
}

/// Analyze SVG bytes without converting them.
pub fn analyze(source: &[u8]) -> Result<Analysis> {
    Ok(svg::process(source, false)?.analysis)
}

/// Convert an SVG file exactly or with safe normalization.
pub fn convert_file(path: &Path) -> Result<Asset> {
    let source = std::fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    convert(&source)
}

/// Convert SVG bytes exactly or with safe normalization.
pub fn convert(source: &[u8]) -> Result<Asset> {
    let processed = svg::process(source, true)?;
    if !processed.analysis.compatibility.convertible() {
        return Err(Error::Incompatible(Box::new(processed.analysis)));
    }
    Ok(Asset {
        drawable: processed
            .drawable
            .expect("conversion requested for a compatible SVG"),
        analysis: processed.analysis,
    })
}
