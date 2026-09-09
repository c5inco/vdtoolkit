pub mod analysis;
pub mod error;
pub mod optimize;
pub mod svg;
pub mod vector;
pub mod xml;

use std::path::Path;

pub use analysis::{Analysis, Compatibility, Diagnostic, DiagnosticCode, Metrics, Severity};
pub use error::{Error, Result};
pub use vector::VectorDrawable;

pub struct Asset {
    pub drawable: VectorDrawable,
    pub analysis: Analysis,
}

pub fn analyze_file(path: &Path) -> Result<Analysis> {
    let source = std::fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    analyze(&source)
}

pub fn analyze(source: &[u8]) -> Result<Analysis> {
    Ok(svg::process(source, false)?.analysis)
}

pub fn convert_file(path: &Path) -> Result<Asset> {
    let source = std::fs::read(path).map_err(|source| Error::Read {
        path: path.to_owned(),
        source,
    })?;
    convert(&source)
}

pub fn convert(source: &[u8]) -> Result<Asset> {
    let processed = svg::process(source, true)?;
    if !processed.analysis.compatibility.convertible() {
        return Err(Error::Incompatible(processed.analysis));
    }
    Ok(Asset {
        drawable: processed
            .drawable
            .expect("conversion requested for a compatible SVG"),
        analysis: processed.analysis,
    })
}
