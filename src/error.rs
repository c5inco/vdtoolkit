use std::path::PathBuf;

use thiserror::Error;

use crate::Analysis;

/// Result type returned by the embedding API.
pub type Result<T> = std::result::Result<T, Error>;

/// Failure while reading, parsing, analyzing, converting, or writing an SVG.
#[derive(Debug, Error)]
pub enum Error {
    /// An input file could not be read.
    #[error("cannot read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    /// An output file or directory could not be written.
    #[error("cannot write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Input bytes are not UTF-8 XML.
    #[error("input is not UTF-8 XML")]
    Utf8(#[from] std::str::Utf8Error),
    /// Input contains a forbidden XML construct such as a DTD.
    #[error("unsafe XML is not accepted: {0}")]
    UnsafeXml(&'static str),
    /// Input is not well-formed XML.
    #[error("malformed SVG XML: {0}")]
    Xml(#[from] roxmltree::Error),
    /// SVG normalization failed.
    #[error("SVG normalization failed: {0}")]
    Svg(#[from] usvg::Error),
    /// The SVG cannot be converted exactly or with safe normalization.
    #[error("SVG is not exactly representable as VectorDrawable")]
    Incompatible(Box<Analysis>),
    /// CLI input arguments describe an unsupported operation.
    #[error("{0}")]
    InvalidInput(String),
}
