//! Browser-oriented WebAssembly adapter for the `svg2vd` Rust API.

use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum OperationResult {
    Success {
        ok: bool,
        analysis: svg2vd::Analysis,
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
    analysis: Option<svg2vd::Analysis>,
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
  estimated_xml_bytes: number;
}

export interface Analysis {
  compatibility: Compatibility;
  minimum_api: number | null;
  diagnostics: Diagnostic[];
  metrics: Metrics;
}

export interface Svg2vdError {
  kind: "utf8" | "unsafe_xml" | "xml" | "svg" | "unsupported" | "io" | "invalid_input";
  message: string;
  analysis?: Analysis;
}

export type AnalyzeResult =
  | { ok: true; analysis: Analysis }
  | { ok: false; error: Svg2vdError };

export type ConvertResult =
  | { ok: true; analysis: Analysis; xml: string }
  | { ok: false; error: Svg2vdError };
"#;

/// Analyze SVG bytes and return a structured JavaScript result.
#[wasm_bindgen(js_name = analyzeSvg, skip_typescript)]
pub fn analyze_svg(source: &[u8]) -> JsValue {
    serialize(&analyze_result(source))
}

/// Convert SVG bytes to VectorDrawable XML, optionally optimizing path data.
#[wasm_bindgen(js_name = convertSvg, skip_typescript)]
pub fn convert_svg(source: &[u8], optimize: bool) -> JsValue {
    serialize(&convert_result(source, optimize))
}

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_FUNCTIONS: &'static str = r#"
export function analyzeSvg(source: Uint8Array): AnalyzeResult;
export function convertSvg(source: Uint8Array, optimize: boolean): ConvertResult;
"#;

fn analyze_result(source: &[u8]) -> OperationResult {
    match svg2vd::analyze(source) {
        Ok(analysis) => OperationResult::Success {
            ok: true,
            analysis,
            xml: None,
        },
        Err(error) => failure(error),
    }
}

fn convert_result(source: &[u8], optimize: bool) -> OperationResult {
    match svg2vd::convert(source) {
        Ok(mut asset) => {
            if optimize {
                asset.optimize();
            }
            OperationResult::Success {
                ok: true,
                xml: Some(asset.to_xml()),
                analysis: asset.analysis,
            }
        }
        Err(error) => failure(error),
    }
}

fn failure(error: svg2vd::Error) -> OperationResult {
    let kind = match &error {
        svg2vd::Error::Read { .. } | svg2vd::Error::Write { .. } => "io",
        svg2vd::Error::Utf8(_) => "utf8",
        svg2vd::Error::UnsafeXml(_) => "unsafe_xml",
        svg2vd::Error::Xml(_) => "xml",
        svg2vd::Error::Svg(_) => "svg",
        svg2vd::Error::Incompatible(_) => "unsupported",
        svg2vd::Error::InvalidInput(_) => "invalid_input",
    };
    let message = error.to_string();
    let analysis = match error {
        svg2vd::Error::Incompatible(analysis) => Some(analysis),
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

    fn json(result: &OperationResult) -> serde_json::Value {
        serde_json::to_value(result).unwrap()
    }

    #[test]
    fn exact_and_normalized_results_match_the_native_api() {
        for source in [EXACT, NORMALIZED] {
            let native_analysis = svg2vd::analyze(source).unwrap();
            let native_asset = svg2vd::convert(source).unwrap();

            let analyzed = json(&analyze_result(source));
            let converted = json(&convert_result(source, false));

            assert_eq!(
                analyzed["analysis"],
                serde_json::to_value(native_analysis).unwrap()
            );
            assert_eq!(
                converted["analysis"],
                serde_json::to_value(&native_asset.analysis).unwrap()
            );
            assert_eq!(converted["xml"], native_asset.to_xml());
        }
    }

    #[test]
    fn unsupported_and_malformed_errors_preserve_native_information() {
        let unsupported_analysis = svg2vd::analyze(UNSUPPORTED).unwrap();
        let unsupported = json(&convert_result(UNSUPPORTED, false));
        assert_eq!(unsupported["error"]["kind"], "unsupported");
        assert_eq!(
            unsupported["error"]["analysis"],
            serde_json::to_value(unsupported_analysis).unwrap()
        );

        let malformed = json(&convert_result(MALFORMED, false));
        assert_eq!(malformed["error"]["kind"], "xml");
        assert!(malformed["error"].get("analysis").is_none());
    }

    #[test]
    fn optimization_and_determinism_match_the_native_api() {
        let mut native = svg2vd::convert(DECIMALS).unwrap();
        native.optimize();
        let optimized = json(&convert_result(DECIMALS, true));
        assert_eq!(optimized["xml"], native.to_xml());
        assert_eq!(
            optimized["analysis"],
            serde_json::to_value(native.analysis).unwrap()
        );

        assert_eq!(
            json(&convert_result(EXACT, false)),
            json(&convert_result(EXACT, false))
        );
    }
}
