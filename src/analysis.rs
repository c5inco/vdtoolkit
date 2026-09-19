use serde::Serialize;

use crate::bitmap::ImageFormat;

/// How faithfully an SVG can be represented as a VectorDrawable.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Exact,
    ExactWithNormalization,
    Approximate,
    Unsupported,
}

impl Compatibility {
    /// Whether conversion is allowed without approximation or data loss.
    pub fn convertible(self) -> bool {
        matches!(self, Self::Exact | Self::ExactWithNormalization)
    }

    pub(crate) fn worsen(&mut self, other: Self) {
        *self = (*self).max(other);
    }
}

/// Compatibility and output requirements discovered for one SVG.
#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    /// The strongest compatibility classification encountered.
    pub compatibility: Compatibility,
    /// The minimum verified Android API for converted output, when convertible.
    pub minimum_api: Option<u32>,
    /// Actionable compatibility and normalization findings.
    pub diagnostics: Vec<Diagnostic>,
    /// Geometry and output-size measurements.
    pub metrics: Metrics,
    /// Raster format when this analysis is of a layer image rather than an SVG.
    ///
    /// `"png"`, `"webp"`, or `"jpg"`. Omitted from JSON for SVG input. When
    /// present, [`Metrics::width`] and [`Metrics::height`] are pixels.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<ImageFormat>,
}

/// Measurements collected while analyzing an SVG.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Metrics {
    /// Normalized drawable width in dp, or pixel width when [`Analysis::image`]
    /// is set.
    pub width: f32,
    /// Normalized drawable height in dp, or pixel height when
    /// [`Analysis::image`] is set.
    pub height: f32,
    /// VectorDrawable viewport width.
    pub viewport_width: f32,
    /// VectorDrawable viewport height.
    pub viewport_height: f32,
    /// Number of visible painted paths.
    pub paths: usize,
    /// Number of emitted path commands, including clip geometry.
    pub path_commands: usize,
    /// Number of normalized SVG groups.
    pub groups: usize,
    /// Number of source gradients.
    pub gradients: usize,
    /// Number of source clip paths.
    pub clip_paths: usize,
    /// Bounds of the painted paths that are emitted, including strokes,
    /// clamped to the viewport.
    ///
    /// Hidden, unpainted, and fully transparent geometry is excluded, and a
    /// transparent stroke does not widen the bounds of a visible fill.
    /// Clipping is not applied, so
    /// the bounds can be larger than what is visible. `None` when no painted
    /// path lies inside the viewport.
    pub content_bounds: Option<Bounds>,
    /// Serialized VectorDrawable size before optional optimization.
    pub estimated_xml_bytes: usize,
}

/// Axis-aligned bounds in VectorDrawable viewport units.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct Bounds {
    /// Left edge.
    pub left: f32,
    /// Top edge.
    pub top: f32,
    /// Right edge.
    pub right: f32,
    /// Bottom edge.
    pub bottom: f32,
}

/// One compatibility, safety, or normalization finding.
#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    /// Stable machine-readable diagnostic identifier.
    pub code: DiagnosticCode,
    /// Diagnostic severity.
    pub severity: Severity,
    /// Human-readable explanation.
    pub message: String,
    /// Source element location when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ElementLocation>,
    /// Suggested remediation when available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

/// Diagnostic severity.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

/// Location of an SVG element in the source document.
#[derive(Clone, Debug, Serialize)]
pub struct ElementLocation {
    /// Local SVG element name.
    pub element: String,
    /// One-based source line.
    pub line: u32,
    /// One-based source column.
    pub column: u32,
}

/// Stable diagnostic identifier.
#[derive(Clone, Copy, Debug, Serialize)]
pub enum DiagnosticCode {
    #[serde(rename = "SVGVD001")]
    UnsupportedMask,
    #[serde(rename = "SVGVD002")]
    UnsupportedFilter,
    #[serde(rename = "SVGVD003")]
    UnsupportedGradient,
    /// A convertible drawable needs API 24 for gradients, even-odd fills, or
    /// more than one clip path. Informational: it mirrors `minimum_api` and
    /// names what raised it.
    #[serde(rename = "SVGVD004")]
    ApiLevelRequirement,
    #[serde(rename = "SVGVD005")]
    ExternalImage,
    #[serde(rename = "SVGVD006")]
    TextNotOutlined,
    #[serde(rename = "SVGVD007")]
    EmbeddedImage,
    #[serde(rename = "SVGVD008")]
    UnsupportedPattern,
    #[serde(rename = "SVGVD009")]
    UnsupportedAnimation,
    #[serde(rename = "SVGVD010")]
    ExternalReference,
    #[serde(rename = "SVGVD011")]
    NormalizationRequired,
    #[serde(rename = "SVGVD012")]
    UnsupportedStrokeTransform,
    #[serde(rename = "SVGVD013")]
    UnsupportedPaint,
    #[serde(rename = "SVGVD014")]
    UnsupportedClipPath,
    #[serde(rename = "SVGVD015")]
    UnsupportedDimensions,
    #[serde(rename = "SVGVD016")]
    LargeDimensions,
    /// A notification icon paints almost the whole canvas, so it tints into
    /// a filled square rather than a silhouette.
    #[serde(rename = "SVGVD017")]
    NotificationPlate,
    /// A notification icon has no painted content.
    #[serde(rename = "SVGVD018")]
    EmptyArtwork,
    /// Adaptive foreground or monochrome content leaves the 66dp safe zone.
    #[serde(rename = "SVGVD019")]
    OutsideSafeZone,
    /// An adaptive background does not paint every pixel of the 108dp layer.
    #[serde(rename = "SVGVD020")]
    BackgroundGap,
    /// Colors or gradients were flattened to white for a notification icon.
    #[serde(rename = "SVGVD021")]
    PaintFlattened,
    /// Wide-gamut `color()` values were resolved to the sRGB they render as.
    #[serde(rename = "SVGVD022")]
    WideGamutColor,
    /// An adaptive background was scaled to cover the 108dp layer, so the
    /// layer cropped what overflowed it.
    #[serde(rename = "SVGVD023")]
    BackgroundCropped,
    /// A bitmap background layer is not square, so Android stretches it onto
    /// the square layer.
    #[serde(rename = "SVGVD024")]
    BackgroundImageShape,
    /// A bitmap background layer carries fewer or more pixels than the
    /// densest screen draws the layer at.
    #[serde(rename = "SVGVD025")]
    BackgroundImageResolution,
    /// A bitmap foreground layer has no transparent pixels, so it covers the
    /// background layer entirely.
    #[serde(rename = "SVGVD026")]
    ForegroundOpaque,
}

impl DiagnosticCode {
    /// Return the stable `SVGVDnnn` identifier.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedMask => "SVGVD001",
            Self::UnsupportedFilter => "SVGVD002",
            Self::UnsupportedGradient => "SVGVD003",
            Self::ApiLevelRequirement => "SVGVD004",
            Self::ExternalImage => "SVGVD005",
            Self::TextNotOutlined => "SVGVD006",
            Self::EmbeddedImage => "SVGVD007",
            Self::UnsupportedPattern => "SVGVD008",
            Self::UnsupportedAnimation => "SVGVD009",
            Self::ExternalReference => "SVGVD010",
            Self::NormalizationRequired => "SVGVD011",
            Self::UnsupportedStrokeTransform => "SVGVD012",
            Self::UnsupportedPaint => "SVGVD013",
            Self::UnsupportedClipPath => "SVGVD014",
            Self::UnsupportedDimensions => "SVGVD015",
            Self::LargeDimensions => "SVGVD016",
            Self::NotificationPlate => "SVGVD017",
            Self::EmptyArtwork => "SVGVD018",
            Self::OutsideSafeZone => "SVGVD019",
            Self::BackgroundGap => "SVGVD020",
            Self::PaintFlattened => "SVGVD021",
            Self::WideGamutColor => "SVGVD022",
            Self::BackgroundCropped => "SVGVD023",
            Self::BackgroundImageShape => "SVGVD024",
            Self::BackgroundImageResolution => "SVGVD025",
            Self::ForegroundOpaque => "SVGVD026",
        }
    }
}
