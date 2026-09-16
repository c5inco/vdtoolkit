//! Browser-oriented WebAssembly adapter for the `vdtoolkit` Rust API.

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OperationResult {
    Success {
        ok: bool,
        analysis: vdtoolkit::Analysis,
        #[serde(skip_serializing_if = "Option::is_none")]
        xml: Option<String>,
    },
    Failure {
        ok: bool,
        error: AdapterError,
    },
}

#[derive(Debug, Serialize)]
struct AdapterError {
    kind: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    analysis: Option<vdtoolkit::Analysis>,
}

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_TYPES: &'static str = r#"
export type Compatibility = "exact" | "exact_with_normalization" | "approximate" | "unsupported";
export type Severity = "error" | "warning" | "info";

export interface ElementLocation {
  element: string;
  line: number;
  column: number;
}

export interface Diagnostic {
  code: string;
  severity: Severity;
  message: string;
  location?: ElementLocation;
  suggestion?: string;
}

export interface Bounds {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

export interface Metrics {
  width: number;
  height: number;
  viewport_width: number;
  viewport_height: number;
  paths: number;
  path_commands: number;
  groups: number;
  gradients: number;
  clip_paths: number;
  content_bounds: Bounds | null;
  estimated_xml_bytes: number;
}

export interface Analysis {
  compatibility: Compatibility;
  minimum_api: number | null;
  diagnostics: Diagnostic[];
  metrics: Metrics;
}

export interface VdtoolkitError {
  kind: "utf8" | "unsafe_xml" | "xml" | "svg" | "unsupported" | "io" | "invalid_input";
  message: string;
  analysis?: Analysis;
}

export type AnalyzeResult =
  | { ok: true; analysis: Analysis }
  | { ok: false; error: VdtoolkitError };

export type ConvertResult =
  | { ok: true; analysis: Analysis; xml: string }
  | { ok: false; error: VdtoolkitError };
"#;

/// Analyze SVG bytes and return a structured JavaScript result.
#[wasm_bindgen(js_name = analyzeSvg, skip_typescript)]
pub fn analyze_svg(source: &[u8]) -> JsValue {
    serialize(&analyze_result(source))
}

/// Convert SVG bytes to VectorDrawable XML, optionally optimizing numbers and
/// path data and scaling the drawable down so neither side exceeds
/// `max_size_dp`. The XML is compact, like the command-line interface's,
/// unless `pretty` asks for readable indentation and line breaks.
#[wasm_bindgen(js_name = convertSvg, skip_typescript)]
pub fn convert_svg(
    source: &[u8],
    optimize: bool,
    max_size_dp: Option<f32>,
    pretty: Option<bool>,
) -> JsValue {
    serialize(&convert_result(
        source,
        optimize,
        max_size_dp,
        pretty.unwrap_or(false),
    ))
}

/// Convert SVG bytes to a white 24dp Android notification icon. Artwork is
/// scaled and centered inside `fit_dp`, which defaults to the full canvas.
#[wasm_bindgen(js_name = convertNotificationSvg, skip_typescript)]
pub fn convert_notification_svg(
    source: &[u8],
    optimize: bool,
    fit_dp: Option<f32>,
    pretty: Option<bool>,
) -> JsValue {
    serialize(&notification_result(
        source,
        optimize,
        fit_dp.unwrap_or(vdtoolkit::NOTIFICATION_ICON_SIZE),
        pretty.unwrap_or(false),
    ))
}

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_FUNCTIONS: &'static str = r#"
export function analyzeSvg(source: Uint8Array): AnalyzeResult;
export function convertSvg(source: Uint8Array, optimize: boolean, maxSizeDp?: number, pretty?: boolean): ConvertResult;
export function convertNotificationSvg(source: Uint8Array, optimize: boolean, fitDp?: number, pretty?: boolean): ConvertResult;
"#;

fn analyze_result(source: &[u8]) -> OperationResult {
    match vdtoolkit::analyze(source) {
        Ok(analysis) => OperationResult::Success {
            ok: true,
            analysis,
            xml: None,
        },
        Err(error) => failure(error),
    }
}

fn convert_result(
    source: &[u8],
    optimize: bool,
    max_size_dp: Option<f32>,
    pretty: bool,
) -> OperationResult {
    match vdtoolkit::convert(source) {
        Ok(mut asset) => {
            if optimize {
                asset.optimize();
            }
            if let Some(max_dp) = max_size_dp {
                asset.fit_within(max_dp);
            }
            OperationResult::Success {
                ok: true,
                xml: Some(if pretty {
                    asset.to_xml()
                } else {
                    asset.to_compact_xml()
                }),
                analysis: asset.analysis,
            }
        }
        Err(error) => failure(error),
    }
}

fn notification_result(
    source: &[u8],
    optimize: bool,
    fit_dp: f32,
    pretty: bool,
) -> OperationResult {
    match vdtoolkit::convert(source).and_then(|mut asset| {
        asset.to_icon(vdtoolkit::IconKind::Notification, vdtoolkit::Fit::contain(fit_dp))?;
        if optimize {
            asset.optimize();
        }
        Ok(asset)
    }) {
        Ok(asset) => OperationResult::Success {
            ok: true,
            xml: Some(if pretty {
                asset.to_xml()
            } else {
                asset.to_compact_xml()
            }),
            analysis: asset.analysis,
        },
        Err(error) => failure(error),
    }
}

fn failure(error: vdtoolkit::Error) -> OperationResult {
    let kind = match &error {
        vdtoolkit::Error::Read { .. } | vdtoolkit::Error::Write { .. } => "io",
        vdtoolkit::Error::Utf8(_) => "utf8",
        vdtoolkit::Error::UnsafeXml(_) => "unsafe_xml",
        vdtoolkit::Error::Xml(_) => "xml",
        vdtoolkit::Error::Svg(_) => "svg",
        vdtoolkit::Error::Incompatible(_) => "unsupported",
        vdtoolkit::Error::InvalidInput(_) => "invalid_input",
    };
    let message = error.to_string();
    let analysis = match error {
        vdtoolkit::Error::Incompatible(analysis) => Some(*analysis),
        _ => None,
    };
    OperationResult::Failure {
        ok: false,
        error: AdapterError {
            kind,
            message,
            analysis,
        },
    }
}

fn serialize(result: &OperationResult) -> JsValue {
    result
        .serialize(&serde_wasm_bindgen::Serializer::new().serialize_maps_as_objects(true))
        .expect("adapter result contains only JavaScript-compatible values")
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXACT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="10"><path d="M0 0H20V10Z" fill="#123456"/></svg>"##;
    const NORMALIZED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><rect x="1" y="2" width="3" height="4" fill="#123456"/></svg>"##;
    const UNSUPPORTED: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><text>hello</text></svg>"##;
    const MALFORMED: &[u8] = br#"<svg><path></svg>"#;
    const DECIMALS: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24"><path d="M1.234567 2.345678L20.987654 21.876543" fill="#123456"/></svg>"##;

    const LARGE: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="480" height="320"><path d="M0 0H480V320Z" fill="#123456"/></svg>"##;

    fn json(result: &OperationResult) -> serde_json::Value {
        serde_json::to_value(result).unwrap()
    }

    #[test]
    fn exact_and_normalized_results_match_the_native_api() {
        for source in [EXACT, NORMALIZED] {
            let native_analysis = vdtoolkit::analyze(source).unwrap();
            let native_asset = vdtoolkit::convert(source).unwrap();

            let analyzed = json(&analyze_result(source));
            let converted = json(&convert_result(source, false, None, false));

            assert_eq!(
                analyzed["analysis"],
                serde_json::to_value(native_analysis).unwrap()
            );
            assert_eq!(
                converted["analysis"],
                serde_json::to_value(&native_asset.analysis).unwrap()
            );
            assert_eq!(converted["xml"], native_asset.to_compact_xml());
        }
    }

    #[test]
    fn unsupported_and_malformed_errors_preserve_native_information() {
        let unsupported_analysis = vdtoolkit::analyze(UNSUPPORTED).unwrap();
        let unsupported = json(&convert_result(UNSUPPORTED, false, None, false));
        assert_eq!(unsupported["error"]["kind"], "unsupported");
        assert_eq!(
            unsupported["error"]["analysis"],
            serde_json::to_value(unsupported_analysis).unwrap()
        );

        let malformed = json(&convert_result(MALFORMED, false, None, false));
        assert_eq!(malformed["error"]["kind"], "xml");
        assert!(malformed["error"].get("analysis").is_none());
    }

    #[test]
    fn optimization_and_determinism_match_the_native_api() {
        let mut native = vdtoolkit::convert(DECIMALS).unwrap();
        native.optimize();
        let optimized = json(&convert_result(DECIMALS, true, None, false));
        assert_eq!(optimized["xml"], native.to_compact_xml());
        assert_eq!(
            optimized["analysis"],
            serde_json::to_value(native.analysis).unwrap()
        );

        assert_eq!(
            json(&convert_result(EXACT, false, None, false)),
            json(&convert_result(EXACT, false, None, false))
        );
    }

    #[test]
    fn pretty_output_matches_the_readable_native_xml() {
        let mut native = vdtoolkit::convert(DECIMALS).unwrap();
        native.optimize();
        let pretty = json(&convert_result(DECIMALS, true, None, true));
        assert_eq!(pretty["xml"], native.to_xml());
        assert_eq!(
            json(&convert_result(DECIMALS, true, None, false))["xml"],
            vdtoolkit::compact_xml(&native.to_xml())
        );
    }

    #[test]
    fn size_cap_matches_the_native_api() {
        let uncapped = json(&convert_result(LARGE, false, None, false));
        assert_eq!(uncapped["analysis"]["diagnostics"][0]["code"], "SVGVD016");
        assert_eq!(uncapped["analysis"]["diagnostics"][0]["severity"], "warning");

        let mut native = vdtoolkit::convert(LARGE).unwrap();
        assert!(native.fit_within(200.0));
        let capped = json(&convert_result(LARGE, false, Some(200.0), false));
        assert_eq!(capped["xml"], native.to_compact_xml());
        assert_eq!(
            capped["analysis"],
            serde_json::to_value(&native.analysis).unwrap()
        );
        assert_eq!(capped["analysis"]["diagnostics"], serde_json::json!([]));
    }

    #[test]
    fn notification_conversion_matches_the_native_api() {
        let mut native = vdtoolkit::convert(EXACT).unwrap();
        native
            .to_icon(vdtoolkit::IconKind::Notification, vdtoolkit::Fit::contain(20.0))
            .unwrap();
        native.optimize();

        let converted = json(&notification_result(EXACT, true, 20.0, false));
        assert_eq!(converted["xml"], native.to_compact_xml());
        assert_eq!(
            converted["analysis"],
            serde_json::to_value(native.analysis).unwrap()
        );
        assert_eq!(converted["analysis"]["metrics"]["width"], 24.0);
        assert!(converted["xml"].as_str().unwrap().contains("#FFFFFF"));
    }
}
