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
mod icon;
mod notification;
mod optimize;
mod render;
mod short_path;
mod svg;
mod vector;
mod xml;

use std::path::Path;

pub use adaptive::{
    ADAPTIVE_ICON_SAFE_ZONE, ADAPTIVE_ICON_SIZE, ADAPTIVE_ICON_VISIBLE_DIAMETER,
    LEGACY_ICON_DENSITIES, LEGACY_ICON_KEYLINE, LEGACY_ICON_SIZE, adaptive_icon_xml,
    color_resource_xml,
};
pub use analysis::{
    Analysis, Bounds, Compatibility, Diagnostic, DiagnosticCode, ElementLocation, Metrics, Severity,
};
pub use error::{Error, Result};
pub use icon::IconKind;
pub use notification::{Flattening, NOTIFICATION_ICON_LIVE_AREA, NOTIFICATION_ICON_SIZE};

/// A converted SVG and its compatibility analysis.
#[derive(Clone, Debug)]
pub struct Asset {
    drawable: vector::VectorDrawable,
    /// Compatibility, minimum Android API, diagnostics, and source metrics.
    pub analysis: Analysis,
    declared_size: bool,
    /// Set by `optimize`: path data is written in its shortest form.
    short_paths: bool,
}

impl Asset {
    /// Serialize this asset as deterministic Android VectorDrawable XML.
    pub fn to_xml(&self) -> String {
        xml::write(&self.drawable, self.short_paths)
    }

    /// Serialize this asset as deterministic, whitespace-minimized
    /// VectorDrawable XML.
    pub fn to_compact_xml(&self) -> String {
        xml::compact(&self.to_xml())
    }

    /// Safely reduce numeric precision in the generated drawable, and write
    /// its path data in the shortest form Android reads as the same points.
    pub fn optimize(&mut self) {
        optimize::optimize(&mut self.drawable);
        self.short_paths = true;
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
        self.resized();
        true
    }

    /// Set `android:width` and `android:height` so the longer side is
    /// `size_dp` and the other keeps the aspect ratio exactly. The viewport is
    /// kept, so the drawing is unchanged apart from its size.
    pub fn set_size(&mut self, size_dp: f32) -> Result<()> {
        if !(size_dp.is_finite() && size_dp > 0.0) {
            return Err(Error::InvalidInput(format!(
                "size must be a positive number of dp, got {size_dp}"
            )));
        }
        let drawable = &mut self.drawable;
        let (width, height) = (drawable.width_dp, drawable.height_dp);
        if width >= height {
            drawable.width_dp = size_dp;
            drawable.height_dp = height * size_dp / width;
        } else {
            drawable.width_dp = width * size_dp / height;
            drawable.height_dp = size_dp;
        }
        self.resized();
        Ok(())
    }

    /// Whether the SVG declared its own `width` or `height`. An SVG with only
    /// a `viewBox` has no size of its own, so its viewBox units become dp.
    pub fn has_declared_size(&self) -> bool {
        self.declared_size
    }

    /// Update the metrics and the large-dimensions warning after the drawable
    /// size changed.
    fn resized(&mut self) {
        let (width, height) = (self.drawable.width_dp, self.drawable.height_dp);
        self.analysis.metrics.width = width;
        self.analysis.metrics.height = height;
        self.analysis.diagnostics.retain(|diagnostic| {
            diagnostic.code.as_str() != DiagnosticCode::LargeDimensions.as_str()
        });
        self.analysis
            .diagnostics
            .extend(vector::large_dimensions_warning(width, height));
        self.analysis.metrics.estimated_xml_bytes = self.to_xml().len();
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
        self.fit_canvas(adaptive::ADAPTIVE_ICON_SIZE, fit);
        Ok(())
    }

    /// Turn this asset into a notification icon: every fill and stroke
    /// flattened to white, keeping opacity, on a 24dp square canvas whose
    /// content is scaled uniformly and centered to fit inside a `fit` dp
    /// square. Returns what the flattening changed.
    ///
    /// Android draws a status bar and notification icon from its alpha channel
    /// alone and tints it with the system color, so color in the source cannot
    /// survive. Use [`NOTIFICATION_ICON_SIZE`] as the fit for artwork that
    /// already carries its own padding, such as a Material system icon, and
    /// [`NOTIFICATION_ICON_LIVE_AREA`] for artwork drawn edge to edge.
    pub fn to_notification_icon(&mut self, fit: f32) -> Result<Flattening> {
        if !(fit > 0.0 && fit <= notification::NOTIFICATION_ICON_SIZE) {
            return Err(Error::InvalidInput(format!(
                "notification icon fit must be between 0 and {} dp, got {fit}",
                notification::NOTIFICATION_ICON_SIZE
            )));
        }
        let flattening = notification::whiten(&mut self.drawable);
        self.fit_canvas(notification::NOTIFICATION_ICON_SIZE, fit);
        // Flattening a gradient to solid white can lower the minimum API, and
        // the fit never raises it.
        self.analysis.minimum_api = self
            .analysis
            .minimum_api
            .map(|_| self.drawable.minimum_api());
        self.analysis.diagnostics.retain(|diagnostic| {
            diagnostic.code.as_str() != DiagnosticCode::ApiLevelRequirement.as_str()
        });
        if self.analysis.minimum_api.is_some() {
            self.analysis
                .diagnostics
                .extend(vector::api_level_note(&self.drawable));
        }
        self.analysis.metrics.estimated_xml_bytes = self.to_xml().len();
        Ok(flattening)
    }

    /// Scale the content uniformly into a centered `fit` dp square on a
    /// `canvas` dp square, updating the metrics that placement changes.
    fn fit_canvas(&mut self, canvas: f32, fit: f32) {
        let metrics = &mut self.analysis.metrics;
        let extent = metrics.viewport_width.max(metrics.viewport_height);
        let scale = fit / extent;
        let dx = (canvas - metrics.viewport_width * scale) / 2.0;
        let dy = (canvas - metrics.viewport_height * scale) / 2.0;
        adaptive::fit_square(&mut self.drawable, canvas, fit);
        // The canvas replaces the source size, so a large-dimensions warning
        // about the source no longer describes the output.
        self.analysis.diagnostics.retain(|diagnostic| {
            diagnostic.code.as_str() != DiagnosticCode::LargeDimensions.as_str()
        });
        let metrics = &mut self.analysis.metrics;
        metrics.width = canvas;
        metrics.height = canvas;
        metrics.viewport_width = canvas;
        metrics.viewport_height = canvas;
        if let Some(bounds) = &mut metrics.content_bounds {
            bounds.left = bounds.left * scale + dx;
            bounds.top = bounds.top * scale + dy;
            bounds.right = bounds.right * scale + dx;
            bounds.bottom = bounds.bottom * scale + dy;
        }
        self.analysis.metrics.estimated_xml_bytes = self.to_xml().len();
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

    /// Whether a fitted adaptive layer paints every pixel of the 108dp layer.
    ///
    /// The layer is rendered at one pixel per dp with clipping, fill rules,
    /// and alpha applied, so letterboxed non-square artwork, inset clips,
    /// holes, and transparent paint all count as not filling. Uncovered areas
    /// in a background show through launcher masks and parallax.
    pub fn fills_adaptive_layer(&self) -> bool {
        let size = adaptive::ADAPTIVE_ICON_SIZE as u32;
        render::render(&self.drawable, size, size)
            .is_some_and(|pixmap| pixmap.pixels().iter().all(|pixel| pixel.alpha() > 0))
    }

    /// Fraction of the drawable's pixels that any paint reaches, from 0 for
    /// empty artwork to 1 for artwork that covers every pixel.
    ///
    /// The drawable is rendered at one pixel per dp with clipping, fill rules,
    /// and alpha applied. A notification icon close to 1 is likely a solid
    /// plate rather than a silhouette, which the system tints into a filled
    /// square.
    pub fn painted_coverage(&self) -> f32 {
        let width = self.drawable.width_dp.round().max(1.0) as u32;
        let height = self.drawable.height_dp.round().max(1.0) as u32;
        let Some(pixmap) = render::render(&self.drawable, width, height) else {
            return 0.0;
        };
        let pixels = pixmap.pixels();
        if pixels.is_empty() {
            return 0.0;
        }
        let painted = pixels.iter().filter(|pixel| pixel.alpha() > 0).count();
        painted as f32 / pixels.len() as f32
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
    /// `background` and `foreground` layers composed under a circular clip,
    /// with the 72dp area launchers show mapped onto the 44dp circle keyline
    /// of a 48dp icon. Fit both layers with [`Asset::fit_adaptive_layer`]
    /// first.
    ///
    /// The result needs API 24 when a layer uses gradients, even-odd fills, or
    /// clip paths of its own. [`Asset::to_png`] renders it for earlier devices.
    pub fn legacy_launcher_icon(background: &Asset, foreground: &Asset) -> Asset {
        let (scale, offset) = adaptive::legacy_mapping();
        let mut asset = Self::synthesized(
            adaptive::legacy_icon(&background.drawable, &foreground.drawable),
            &[background, foreground],
        );
        if let Some(bounds) = &mut asset.analysis.metrics.content_bounds {
            let map = |value: f32| (value * scale + offset).clamp(0.0, adaptive::LEGACY_ICON_SIZE);
            *bounds = Bounds {
                left: map(bounds.left),
                top: map(bounds.top),
                right: map(bounds.right),
                bottom: map(bounds.bottom),
            };
        }
        asset
    }

    /// Render the drawable at `width` × `height` pixels and return
    /// unpremultiplied RGBA8 pixels, row by row from the top left.
    ///
    /// Geometry, fill rules, strokes, gradients, alpha, and clip scope follow
    /// VectorDrawable semantics, with anti-aliasing.
    pub fn render_rgba(&self, width: u32, height: u32) -> Result<Vec<u8>> {
        let pixmap = self.render(width, height)?;
        Ok(pixmap
            .pixels()
            .iter()
            .flat_map(|pixel| {
                let color = pixel.demultiply();
                [color.red(), color.green(), color.blue(), color.alpha()]
            })
            .collect())
    }

    /// Render the drawable at `width` × `height` pixels as a PNG, as
    /// [`Asset::render_rgba`] does.
    pub fn to_png(&self, width: u32, height: u32) -> Result<Vec<u8>> {
        self.render(width, height)?
            .encode_png()
            .map_err(|error| Error::InvalidInput(format!("cannot encode PNG: {error}")))
    }

    fn render(&self, width: u32, height: u32) -> Result<tiny_skia::Pixmap> {
        render::render(&self.drawable, width, height)
            .ok_or_else(|| Error::InvalidInput(format!("cannot render at {width} × {height} px")))
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
            metrics.groups += source.groups;
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
        metrics.groups += drawable
            .children
            .iter()
            .filter(|node| matches!(node, vector::VectorNode::Group(_)))
            .count();
        let minimum_api = Some(drawable.minimum_api());
        let diagnostics = vector::api_level_note(&drawable).into_iter().collect();
        let mut asset = Asset {
            drawable,
            analysis: Analysis {
                compatibility,
                minimum_api,
                diagnostics,
                metrics,
            },
            declared_size: true,
            short_paths: false,
        };
        asset.analysis.metrics.estimated_xml_bytes = asset.to_xml().len();
        asset
    }
}

/// Remove formatting whitespace from XML that has no meaningful text nodes.
///
/// This is useful for the adaptive icon and values resources generated by the
/// command-line interface. Do not use it on arbitrary XML containing text.
pub fn compact_xml(xml: &str) -> String {
    xml::compact(xml)
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

/// Analyze an SVG file as an icon of `kind` fitted to `fit` dp, without
/// writing anything, as [`analyze_as`] does.
pub fn analyze_file_as(path: &Path, kind: IconKind, fit: f32) -> Result<Analysis> {
    let source = std::fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    analyze_as(&source, kind, fit)
}

/// Analyze SVG bytes as an icon of `kind` fitted to `fit` dp, without
/// keeping the drawable.
///
/// A convertible SVG is converted and turned into the icon with
/// [`Asset::to_icon`], so the analysis carries the kind's findings and the
/// metrics of the fitted result. An SVG that cannot be converted yields its
/// compatibility analysis alone, as [`analyze`] would.
pub fn analyze_as(source: &[u8], kind: IconKind, fit: f32) -> Result<Analysis> {
    match convert(source) {
        Ok(mut asset) => {
            asset.to_icon(kind, fit)?;
            Ok(asset.analysis)
        }
        Err(Error::Incompatible(analysis)) => Ok(*analysis),
        Err(error) => Err(error),
    }
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
        declared_size: processed.declared_size,
        short_paths: false,
    })
}
