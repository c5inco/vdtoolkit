use serde::Serialize;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Compatibility {
    Exact,
    ExactWithNormalization,
    Approximate,
    Unsupported,
}

impl Compatibility {
    pub fn convertible(self) -> bool {
        matches!(self, Self::Exact | Self::ExactWithNormalization)
    }

    pub(crate) fn worsen(&mut self, other: Self) {
        *self = (*self).max(other);
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Analysis {
    pub compatibility: Compatibility,
    pub minimum_api: Option<u32>,
    pub diagnostics: Vec<Diagnostic>,
    pub metrics: Metrics,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Metrics {
    pub width: f32,
    pub height: f32,
    pub viewport_width: f32,
    pub viewport_height: f32,
    pub paths: usize,
    pub path_commands: usize,
    pub groups: usize,
    pub gradients: usize,
    pub clip_paths: usize,
    pub estimated_xml_bytes: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub severity: Severity,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub location: Option<ElementLocation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suggestion: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Error,
    Warning,
    Info,
}

#[derive(Clone, Debug, Serialize)]
pub struct ElementLocation {
    pub element: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Serialize)]
pub enum DiagnosticCode {
    #[serde(rename = "SVGVD001")]
    UnsupportedMask,
    #[serde(rename = "SVGVD002")]
    UnsupportedFilter,
    #[serde(rename = "SVGVD003")]
    UnsupportedGradient,
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
}

impl DiagnosticCode {
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
        }
    }
}
