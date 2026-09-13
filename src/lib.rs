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

mod analysis;
mod error;
mod optimize;
mod svg;
mod vector;
mod xml;

use std::path::Path;

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
