//! Browser-oriented WebAssembly adapter for the `vdtoolkit` Rust API.

use serde::{Deserialize, Serialize};
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
export type Compatibility = "not_applicable" | "exact" | "exact_with_normalization" | "approximate" | "unsupported";
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
  image?: "png" | "webp" | "jpg";
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
pub fn analyze_svg(source: &[u8], allow_approximate: Option<bool>) -> JsValue {
    serialize(&analyze_result(
        source,
        allow_approximate.unwrap_or(false),
    ))
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
    allow_approximate: Option<bool>,
) -> JsValue {
    serialize(&convert_result(
        source,
        optimize,
        max_size_dp,
        pretty.unwrap_or(false),
        allow_approximate.unwrap_or(false),
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
    allow_approximate: Option<bool>,
) -> JsValue {
    serialize(&notification_result(
        source,
        optimize,
        fit_dp.unwrap_or(vdtoolkit::NOTIFICATION_ICON_SIZE),
        pretty.unwrap_or(false),
        allow_approximate.unwrap_or(false),
    ))
}

/// Generate an adaptive launcher icon from SVG layers: the same files, in the
/// same order, that `vdt adaptive` writes into `res/`. Exactly one of
/// `background` (an SVG scaled to cover the layer) and `background_color`
/// (`#RRGGBB` or `#AARRGGBB`) must be given. Legacy PNGs are listed by path
/// only; render them from the legacy vector they name.
#[wasm_bindgen(js_name = adaptiveIcon, skip_typescript)]
pub fn adaptive_icon(
    foreground: &[u8],
    background: Option<Vec<u8>>,
    background_color: Option<String>,
    monochrome: Option<Vec<u8>>,
    options: JsValue,
) -> JsValue {
    let options: AdaptiveOptions = match serde_wasm_bindgen::from_value(options) {
        Ok(options) => options,
        Err(error) => {
            return serialize(&AdaptiveResult::Failure {
                ok: false,
                error: adapter_error(vdtoolkit::Error::InvalidInput(format!(
                    "invalid options: {error}"
                ))),
                layer: None,
            });
        }
    };
    serialize(&adaptive_result(
        foreground,
        background.as_deref(),
        background_color.as_deref(),
        monochrome.as_deref(),
        &options,
    ))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct AdaptiveOptions {
    name: String,
    fit: f32,
    legacy: bool,
    optimize: bool,
    pretty: bool,
}

impl Default for AdaptiveOptions {
    fn default() -> Self {
        Self {
            name: "ic_launcher".to_owned(),
            fit: vdtoolkit::ADAPTIVE_ICON_SIZE,
            legacy: false,
            optimize: false,
            pretty: false,
        }
    }
}

#[derive(Debug, Serialize)]
struct AdaptiveFile {
    /// Path under `res/`, with forward slashes.
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    xml: Option<String>,
    /// For a PNG: the path of the vector it is rendered from.
    #[serde(skip_serializing_if = "Option::is_none")]
    rendered_from: Option<String>,
}

#[derive(Debug, Serialize)]
struct AdaptiveLayers {
    foreground: vdtoolkit::Analysis,
    #[serde(skip_serializing_if = "Option::is_none")]
    background: Option<vdtoolkit::Analysis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    monochrome: Option<vdtoolkit::Analysis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    legacy: Option<vdtoolkit::Analysis>,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
enum AdaptiveResult {
    Success {
        ok: bool,
        files: Vec<AdaptiveFile>,
        layers: Box<AdaptiveLayers>,
    },
    Failure {
        ok: bool,
        error: AdapterError,
        /// Which layer failed to convert, when one did.
        #[serde(skip_serializing_if = "Option::is_none")]
        layer: Option<&'static str>,
    },
}

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_FUNCTIONS: &'static str = r#"
export interface AdaptiveIconOptions {
  /** Resource name of the icon and prefix of its layers; default ic_launcher. */
  name?: string;
  /** Size in dp of the centered square the foreground and monochrome artwork fit; default 108. */
  fit?: number;
  /** Also write the icon for devices below API 26; default false. */
  legacy?: boolean;
  optimize?: boolean;
  pretty?: boolean;
}

export interface AdaptiveIconFile {
  /** Path under res/, such as mipmap-anydpi-v26/ic_launcher.xml. */
  path: string;
  xml?: string;
  /** For a PNG: the path of the vector it is rendered from. */
  rendered_from?: string;
}

export interface AdaptiveIconLayers {
  foreground: Analysis;
  background?: Analysis;
  monochrome?: Analysis;
  legacy?: Analysis;
}

export type AdaptiveIconResult =
  | { ok: true; files: AdaptiveIconFile[]; layers: AdaptiveIconLayers }
  | { ok: false; error: VdtoolkitError; layer?: "foreground" | "background" | "monochrome" };

export function analyzeSvg(source: Uint8Array, allowApproximate?: boolean): AnalyzeResult;
export function convertSvg(source: Uint8Array, optimize: boolean, maxSizeDp?: number, pretty?: boolean, allowApproximate?: boolean): ConvertResult;
export function convertNotificationSvg(source: Uint8Array, optimize: boolean, fitDp?: number, pretty?: boolean, allowApproximate?: boolean): ConvertResult;
export function adaptiveIcon(foreground: Uint8Array, background: Uint8Array | undefined, backgroundColor: string | undefined, monochrome: Uint8Array | undefined, options?: AdaptiveIconOptions): AdaptiveIconResult;
"#;

fn adaptive_result(
    foreground: &[u8],
    background: Option<&[u8]>,
    background_color: Option<&str>,
    monochrome: Option<&[u8]>,
    options: &AdaptiveOptions,
) -> AdaptiveResult {
    use vdtoolkit::{Asset, Fit, IconKind};

    let invalid = |message: String| AdaptiveResult::Failure {
        ok: false,
        error: adapter_error(vdtoolkit::Error::InvalidInput(message)),
        layer: None,
    };
    if !is_resource_name(&options.name) {
        return invalid(format!(
            "resource name must match [a-z_][a-z0-9_]*, got {:?}",
            options.name
        ));
    }
    if !(options.fit > 0.0 && options.fit <= vdtoolkit::ADAPTIVE_ICON_SIZE) {
        return invalid(format!(
            "fit must be between 0 and {} dp, got {}",
            vdtoolkit::ADAPTIVE_ICON_SIZE,
            options.fit
        ));
    }
    let color = match (background, background_color) {
        (Some(_), None) => None,
        (None, Some(color)) => match normalize_color(color) {
            Ok(color) => Some(color),
            Err(error) => return layer_failure(error, "background"),
        },
        _ => return invalid("give exactly one of background and backgroundColor".to_owned()),
    };

    let layer = |source: &[u8], fit: Fit, kind: IconKind, name: &'static str| {
        vdtoolkit::convert(source)
            .and_then(|mut asset| {
                asset.to_icon(kind, fit)?;
                Ok(asset)
            })
            .map_err(|error| (error, name))
    };
    let xml = |asset: &Asset| {
        let mut asset = asset.clone();
        if options.optimize {
            asset.optimize();
        }
        if options.pretty {
            asset.to_xml()
        } else {
            asset.to_compact_xml()
        }
    };
    let format = |xml: String| {
        if options.pretty {
            xml
        } else {
            vdtoolkit::compact_xml(&xml)
        }
    };
    let file = |path: String, xml: String| AdaptiveFile {
        path,
        xml: Some(xml),
        rendered_from: None,
    };

    let name = &options.name;
    let foreground_name = format!("{name}_foreground");
    let background_name = format!("{name}_background");
    let monochrome_name = format!("{name}_monochrome");
    let round_name = format!("{name}_round");

    let foreground = match layer(
        foreground,
        Fit::contain(options.fit),
        IconKind::AdaptiveForeground,
        "foreground",
    ) {
        Ok(asset) => asset,
        Err((error, name)) => return layer_failure(error, name),
    };
    let mut files = vec![file(
        format!("drawable-anydpi/{foreground_name}.xml"),
        xml(&foreground),
    )];
    let (background, background_reference, background_analysis) = match (background, color) {
        (Some(source), _) => {
            let asset = match layer(
                source,
                IconKind::AdaptiveBackground.default_fit(),
                IconKind::AdaptiveBackground,
                "background",
            ) {
                Ok(asset) => asset,
                Err((error, name)) => return layer_failure(error, name),
            };
            files.push(file(
                format!("drawable-anydpi/{background_name}.xml"),
                xml(&asset),
            ));
            let analysis = asset.analysis.clone();
            (
                asset,
                format!("@drawable/{background_name}"),
                Some(analysis),
            )
        }
        (None, Some(color)) => {
            files.push(file(
                format!("values/{background_name}.xml"),
                format(vdtoolkit::color_resource_xml(&background_name, &color)),
            ));
            let asset = match Asset::solid_adaptive_layer(&color) {
                Ok(asset) => asset,
                Err(error) => return layer_failure(error, "background"),
            };
            (asset, format!("@color/{background_name}"), None)
        }
        (None, None) => unreachable!("one background is required above"),
    };
    let mut monochrome_analysis = None;
    let monochrome_reference = match monochrome {
        Some(source) => {
            let asset = match layer(
                source,
                Fit::contain(options.fit),
                IconKind::AdaptiveForeground,
                "monochrome",
            ) {
                Ok(asset) => asset,
                Err((error, name)) => return layer_failure(error, name),
            };
            files.push(file(
                format!("drawable-anydpi/{monochrome_name}.xml"),
                xml(&asset),
            ));
            monochrome_analysis = Some(asset.analysis);
            Some(format!("@drawable/{monochrome_name}"))
        }
        None => None,
    };
    let icon = format(vdtoolkit::adaptive_icon_xml(
        &background_reference,
        &format!("@drawable/{foreground_name}"),
        monochrome_reference.as_deref(),
    ));
    files.push(file(format!("mipmap-anydpi-v26/{name}.xml"), icon.clone()));
    files.push(file(format!("mipmap-anydpi-v26/{round_name}.xml"), icon));

    let mut legacy_analysis = None;
    if options.legacy {
        let legacy = Asset::legacy_launcher_icon(&background, &foreground);
        let legacy_xml = xml(&legacy);
        if legacy.analysis.minimum_api == Some(21) {
            for icon_name in [name.as_str(), round_name.as_str()] {
                files.push(file(format!("mipmap/{icon_name}.xml"), legacy_xml.clone()));
            }
        } else {
            for icon_name in [name.as_str(), round_name.as_str()] {
                files.push(file(
                    format!("mipmap-anydpi-v24/{icon_name}.xml"),
                    legacy_xml.clone(),
                ));
            }
            for (density, _) in vdtoolkit::LEGACY_ICON_DENSITIES {
                for icon_name in [name.as_str(), round_name.as_str()] {
                    files.push(AdaptiveFile {
                        path: format!("mipmap-{density}/{icon_name}.png"),
                        xml: None,
                        rendered_from: Some(format!("mipmap-anydpi-v24/{icon_name}.xml")),
                    });
                }
            }
        }
        legacy_analysis = Some(legacy.analysis);
    }

    AdaptiveResult::Success {
        ok: true,
        files,
        layers: Box::new(AdaptiveLayers {
            foreground: foreground.analysis,
            background: background_analysis,
            monochrome: monochrome_analysis,
            legacy: legacy_analysis,
        }),
    }
}

fn layer_failure(error: vdtoolkit::Error, layer: &'static str) -> AdaptiveResult {
    AdaptiveResult::Failure {
        ok: false,
        error: adapter_error(error),
        layer: Some(layer),
    }
}

/// `[a-z_][a-z0-9_]*`, as the command line requires. Java keywords, which
/// aapt2 also rejects, are left to the build.
fn is_resource_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c == '_' || c.is_ascii_lowercase())
        && chars.all(|c| c == '_' || c.is_ascii_lowercase() || c.is_ascii_digit())
}

fn normalize_color(color: &str) -> vdtoolkit::Result<String> {
    let digits = color.strip_prefix('#').unwrap_or(color);
    if matches!(digits.len(), 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(format!("#{}", digits.to_ascii_uppercase()))
    } else {
        Err(vdtoolkit::Error::InvalidInput(format!(
            "background color must be #RRGGBB or #AARRGGBB, got {color:?}"
        )))
    }
}

fn analyze_result(source: &[u8], allow_approximate: bool) -> OperationResult {
    match vdtoolkit::analyze_with_options(source, allow_approximate) {
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
    allow_approximate: bool,
) -> OperationResult {
    match vdtoolkit::convert_with_options(source, allow_approximate) {
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
    allow_approximate: bool,
) -> OperationResult {
    match vdtoolkit::convert_with_options(source, allow_approximate).and_then(|mut asset| {
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
    OperationResult::Failure {
        ok: false,
        error: adapter_error(error),
    }
}

fn adapter_error(error: vdtoolkit::Error) -> AdapterError {
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
    AdapterError {
        kind,
        message,
        analysis,
    }
}

fn serialize<T: Serialize>(result: &T) -> JsValue {
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

            let analyzed = json(&analyze_result(source, false));
            let converted = json(&convert_result(source, false, None, false, false));

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
        let unsupported = json(&convert_result(UNSUPPORTED, false, None, false, false));
        assert_eq!(unsupported["error"]["kind"], "unsupported");
        assert_eq!(
            unsupported["error"]["analysis"],
            serde_json::to_value(unsupported_analysis).unwrap()
        );

        let malformed = json(&convert_result(MALFORMED, false, None, false, false));
        assert_eq!(malformed["error"]["kind"], "xml");
        assert!(malformed["error"].get("analysis").is_none());
    }

    #[test]
    fn raster_conversion_errors_do_not_name_cli_flags() {
        let png = b"\x89PNG\r\n\x1a\n";
        for result in [
            convert_result(png, false, None, false, false),
            notification_result(png, false, 24.0, false, false),
        ] {
            let result = json(&result);
            assert_eq!(result["error"]["kind"], "invalid_input");
            assert_eq!(
                result["error"]["message"],
                "this is a raster image, not an SVG"
            );
            assert!(!result["error"]["message"]
                .as_str()
                .unwrap()
                .contains("--"));
        }
    }

    #[test]
    fn optimization_and_determinism_match_the_native_api() {
        let mut native = vdtoolkit::convert(DECIMALS).unwrap();
        native.optimize();
        let optimized = json(&convert_result(DECIMALS, true, None, false, false));
        assert_eq!(optimized["xml"], native.to_compact_xml());
        assert_eq!(
            optimized["analysis"],
            serde_json::to_value(native.analysis).unwrap()
        );

        assert_eq!(
            json(&convert_result(EXACT, false, None, false, false)),
            json(&convert_result(EXACT, false, None, false, false))
        );
    }

    #[test]
    fn pretty_output_matches_the_readable_native_xml() {
        let mut native = vdtoolkit::convert(DECIMALS).unwrap();
        native.optimize();
        let pretty = json(&convert_result(DECIMALS, true, None, true, false));
        assert_eq!(pretty["xml"], native.to_xml());
        assert_eq!(
            json(&convert_result(DECIMALS, true, None, false, false))["xml"],
            vdtoolkit::compact_xml(&native.to_xml())
        );
    }

    #[test]
    fn size_cap_matches_the_native_api() {
        let uncapped = json(&convert_result(LARGE, false, None, false, false));
        assert_eq!(uncapped["analysis"]["diagnostics"][0]["code"], "SVGVD016");
        assert_eq!(uncapped["analysis"]["diagnostics"][0]["severity"], "warning");

        let mut native = vdtoolkit::convert(LARGE).unwrap();
        assert!(native.fit_within(200.0));
        let capped = json(&convert_result(LARGE, false, Some(200.0), false, false));
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

        let converted = json(&notification_result(EXACT, true, 20.0, false, false));
        assert_eq!(converted["xml"], native.to_compact_xml());
        assert_eq!(
            converted["analysis"],
            serde_json::to_value(native.analysis).unwrap()
        );
        assert_eq!(converted["analysis"]["metrics"]["width"], 24.0);
        assert!(converted["xml"].as_str().unwrap().contains("#FFFFFF"));
    }

    #[test]
    fn allow_approximate_enables_approximating_radial_gradients() {
        const FOCAL_OFFSET_GRADIENT: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
            <defs>
                <radialGradient id="g" cx="12" cy="12" r="10" fx="10" fy="10">
                    <stop offset="0%" stop-color="#fff"/>
                    <stop offset="100%" stop-color="#000"/>
                </radialGradient>
            </defs>
            <rect width="24" height="24" fill="url(#g)"/>
        </svg>"##;

        let rejected = json(&convert_result(FOCAL_OFFSET_GRADIENT, false, None, false, false));
        assert_eq!(rejected["error"]["kind"], "unsupported");

        let allowed = json(&convert_result(FOCAL_OFFSET_GRADIENT, false, None, false, true));
        assert_eq!(allowed["ok"], true);
        assert_eq!(allowed["analysis"]["compatibility"], "approximate");
        assert!(allowed["analysis"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "SVGVD003"));
    }
}
