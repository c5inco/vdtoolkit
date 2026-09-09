use std::path::PathBuf;

use thiserror::Error;

use crate::Analysis;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("input is not UTF-8 XML")]
    Utf8(#[from] std::str::Utf8Error),
    #[error("unsafe XML is not accepted: {0}")]
    UnsafeXml(&'static str),
    #[error("malformed SVG XML: {0}")]
    Xml(#[from] roxmltree::Error),
    #[error("SVG normalization failed: {0}")]
    Svg(#[from] usvg::Error),
    #[error("SVG is not exactly representable as VectorDrawable")]
    Incompatible(Analysis),
    #[error("{0}")]
    InvalidInput(String),
}
