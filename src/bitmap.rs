//! Raster icon layers: what vdt can tell about an image it places but never
//! converts.
//!
//! An adaptive icon's layers are drawables, and a drawable can be a raster
//! image. vdt copies that file byte for byte, so nothing it writes is an
//! approximation of the source, but it still decodes the image to report what
//! the layer will look like on a device. PNG, WebP and JPEG are all
//! recognized, from the file header rather than the name.

use serde::Serialize;

use crate::adaptive::{self, ADAPTIVE_ICON_SIZE, PAINTED_ALPHA};
use crate::analysis::{Bounds, Diagnostic, DiagnosticCode, Severity};
use crate::{Error, Result};

/// Pixels an `xxxhdpi` device draws the 108dp layer at, which is the most any
/// density asks of a layer image.
pub const ADAPTIVE_ICON_MAX_PIXELS: u32 = 432;

/// Which layer of the icon an image is for, which decides what is worth
/// reporting about it.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LayerKind {
    /// The background, which sits under everything and must paint every pixel.
    Background,
    /// The foreground, or the monochrome layer, which sits on top and must
    /// leave the background visible around it.
    Foreground,
}

impl LayerKind {
    fn label(self) -> &'static str {
        match self {
            Self::Background => "background",
            Self::Foreground => "foreground",
        }
    }
}

/// Raster formats an icon layer can be written in.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageFormat {
    /// PNG, lossless, alpha optional.
    Png,
    /// WebP, which Android draws from API 14 and the build tools prefer.
    Webp,
    /// JPEG, which has no alpha channel, so it can only ever be an opaque
    /// background.
    Jpeg,
}

impl ImageFormat {
    /// The file extension Android expects the resource to carry.
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Webp => "webp",
            Self::Jpeg => "jpg",
        }
    }

    /// Every extension vdt can write a raster layer under, for clearing the
    /// form a previous run left behind.
    pub const EXTENSIONS: [&'static str; 3] = ["png", "webp", "jpg"];

    /// Recognize the format from the file's own header rather than its name,
    /// so a mislabeled file is written under the extension it really is.
    pub fn sniff(data: &[u8]) -> Option<Self> {
        const PNG: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
        if data.starts_with(&PNG) {
            return Some(Self::Png);
        }
        if data.len() >= 12 && data.starts_with(b"RIFF") && data[8..12] == *b"WEBP" {
            return Some(Self::Webp);
        }
        if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
            return Some(Self::Jpeg);
        }
        None
    }
}

/// What a layer image is, before anything is written.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayerImage {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Whether every pixel is fully opaque.
    pub opaque: bool,
    /// The format the file is in, which decides the extension it is written
    /// under.
    pub format: ImageFormat,
    /// Where the painted pixels sit, in dp on the 108dp layer. `None` when
    /// nothing is painted at all.
    pub content: Option<Bounds>,
    /// How far the painted pixels reach from the layer centre, in dp, which is
    /// what a circular launcher mask judges. `None` when nothing is painted.
    pub reach: Option<f32>,
}

/// Decoded pixels reduced to the three things a layer is judged on.
struct Decoded {
    width: u32,
    height: u32,
    opaque: bool,
    /// Painted pixels as `(left, top, right, bottom)`, exclusive on the far
    /// edges, or `None` when nothing is painted.
    content: Option<(u32, u32, u32, u32)>,
    /// How far painted pixels reach from the layer centre, in dp.
    reach: Option<f32>,
}

/// Decode a raster icon layer and report what it will look like on the 108dp
/// layer, without changing a byte of it.
///
/// A layer that is not square gets `SVGVD024`, because Android stretches it
/// onto the square layer, and one that cannot serve
/// [`ADAPTIVE_ICON_MAX_PIXELS`] sharply, or carries more pixels than that,
/// gets `SVGVD025`.
///
/// A [`LayerKind::Background`] that is not fully opaque gets `SVGVD020`, the
/// same finding a vector background that leaves gaps gets. A
/// [`LayerKind::Foreground`] is judged the other way round: one that is fully
/// opaque hides the background and gets `SVGVD026`, one with nothing painted
/// gets `SVGVD018`, and one whose painted pixels leave the 66dp safe zone gets
/// `SVGVD019`, as a vector foreground does. A JPEG foreground is rejected
/// outright, because a layer with no alpha channel cannot sit over anything.
pub fn analyze_layer_image(data: &[u8], layer: LayerKind) -> Result<(LayerImage, Vec<Diagnostic>)> {
    let format = ImageFormat::sniff(data).ok_or_else(|| {
        Error::InvalidInput(
            "layer image must be a PNG, a WebP, or a JPEG; the file header is none of them"
                .to_owned(),
        )
    })?;
    if layer == LayerKind::Foreground && format == ImageFormat::Jpeg {
        return Err(Error::InvalidInput(
            "a JPEG has no alpha channel, so it would hide the background layer; a foreground or \
             monochrome image must be a PNG or a WebP"
                .to_owned(),
        ));
    }
    let decoded = match format {
        ImageFormat::Png => decode_png(data)?,
        ImageFormat::Webp => decode_webp(data)?,
        ImageFormat::Jpeg => decode_jpeg(data)?,
    };
    let image = LayerImage {
        width: decoded.width,
        height: decoded.height,
        opaque: decoded.opaque,
        format,
        content: decoded.content.map(|content| on_layer(content, &decoded)),
        reach: decoded.reach,
    };
    Ok((image, findings(image, layer)))
}

/// Map a pixel box onto the 108dp layer the image is drawn into, so a bitmap
/// layer is measured in the same units as a vector one.
fn on_layer(content: (u32, u32, u32, u32), decoded: &Decoded) -> Bounds {
    let (left, top, right, bottom) = content;
    let (width, height) = (decoded.width.max(1) as f32, decoded.height.max(1) as f32);
    Bounds {
        left: left as f32 / width * ADAPTIVE_ICON_SIZE,
        top: top as f32 / height * ADAPTIVE_ICON_SIZE,
        right: right as f32 / width * ADAPTIVE_ICON_SIZE,
        bottom: bottom as f32 / height * ADAPTIVE_ICON_SIZE,
    }
}

/// Opacity and the painted box, from a row-major buffer of `stride`-byte
/// pixels whose alpha sits at `alpha_offset`.
fn scan(pixels: &[u8], width: u32, height: u32, stride: usize, alpha_offset: usize) -> Decoded {
    let mut opaque = true;
    let (mut left, mut top, mut right, mut bottom) = (u32::MAX, u32::MAX, 0u32, 0u32);
    let mut reach = 0.0f32;
    for (index, pixel) in pixels.chunks_exact(stride).enumerate() {
        let alpha = pixel[alpha_offset];
        if alpha != u8::MAX {
            opaque = false;
        }
        if alpha <= PAINTED_ALPHA {
            continue;
        }
        let (x, y) = (index as u32 % width, index as u32 / width);
        left = left.min(x);
        top = top.min(y);
        right = right.max(x + 1);
        bottom = bottom.max(y + 1);
        reach = reach.max(adaptive::reach_dp(x, y, width, height));
    }
    let painted = left != u32::MAX;
    Decoded {
        width,
        height,
        opaque,
        content: painted.then_some((left, top, right, bottom)),
        reach: painted.then_some(reach),
    }
}

fn decode_png(data: &[u8]) -> Result<Decoded> {
    let pixmap = tiny_skia::Pixmap::decode_png(data)
        .map_err(|error| Error::InvalidInput(format!("cannot decode PNG: {error}")))?;
    let (width, height) = (pixmap.width(), pixmap.height());
    Ok(scan(pixmap.data(), width, height, 4, 3))
}

fn decode_webp(data: &[u8]) -> Result<Decoded> {
    let mut decoder = image_webp::WebPDecoder::new(std::io::Cursor::new(data))
        .map_err(|error| Error::InvalidInput(format!("cannot decode WebP: {error}")))?;
    if decoder.is_animated() {
        return Err(Error::InvalidInput(
            "layer image is an animated WebP; an icon layer is a still image".to_owned(),
        ));
    }
    let (width, height) = decoder.dimensions();
    // The image data is decoded even when there is no alpha to scan: a file
    // whose header reads but whose data is damaged would otherwise be copied
    // into the project and fail only when Android packages or draws it.
    let size = decoder
        .output_buffer_size()
        .ok_or_else(|| Error::InvalidInput("WebP dimensions are too large to decode".to_owned()))?;
    let mut pixels = vec![0u8; size];
    decoder
        .read_image(&mut pixels)
        .map_err(|error| Error::InvalidInput(format!("cannot decode WebP: {error}")))?;
    // Without an alpha channel the pixels are RGB and every one is painted.
    if !decoder.has_alpha() {
        return Ok(opaque_layer(width, height));
    }
    Ok(scan(&pixels, width, height, 4, 3))
}

/// JPEG carries no alpha channel, so the image is opaque and only its size is
/// in question. The image data is still decoded in full: a file whose header
/// reads but whose data is cut short would otherwise be copied into the
/// project and fail only when Android packages or draws it. JPEG has no
/// checksum, so data damaged into bytes that still decode cannot be caught
/// here or by any other decoder.
fn decode_jpeg(data: &[u8]) -> Result<Decoded> {
    let mut decoder = jpeg_decoder::Decoder::new(std::io::Cursor::new(data));
    decoder
        .decode()
        .map_err(|error| Error::InvalidInput(format!("cannot decode JPEG: {error}")))?;
    let info = decoder
        .info()
        .ok_or_else(|| Error::InvalidInput("JPEG has no frame header".to_owned()))?;
    Ok(opaque_layer(info.width.into(), info.height.into()))
}

fn opaque_layer(width: u32, height: u32) -> Decoded {
    Decoded {
        width,
        height,
        opaque: true,
        content: Some((0, 0, width, height)),
        // Every pixel is painted, so content reaches the layer's corner.
        reach: Some(adaptive::reach_dp(0, 0, width, height)),
    }
}

/// What the image will look like once Android draws it onto the layer.
fn findings(image: LayerImage, layer: LayerKind) -> Vec<Diagnostic> {
    let LayerImage {
        width,
        height,
        opaque,
        content,
        ..
    } = image;
    let (label, size) = (layer.label(), crate::xml::number(ADAPTIVE_ICON_SIZE));
    let mut diagnostics = Vec::new();

    if width != height {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::BackgroundImageShape,
            severity: Severity::Warning,
            message: format!(
                "{label} image is {width}×{height}px; Android stretches it onto the square \
                 {size}dp layer, so the artwork is distorted"
            ),
            location: None,
            suggestion: Some(
                "Crop or pad the image to a square before passing it, so you choose what the \
                 layer keeps rather than having it stretched."
                    .to_owned(),
            ),
        });
    }

    match layer {
        LayerKind::Background if !opaque => diagnostics.push(Diagnostic {
            code: DiagnosticCode::BackgroundGap,
            severity: Severity::Warning,
            message: format!(
                "background image has pixels that are not fully opaque; uncovered areas show \
                 through launcher masks and parallax on the {size}dp layer"
            ),
            location: None,
            suggestion: Some(
                "Flatten the image onto an opaque background, so no wallpaper shows through the \
                 icon."
                    .to_owned(),
            ),
        }),
        LayerKind::Background => {}
        LayerKind::Foreground if opaque => diagnostics.push(Diagnostic {
            code: DiagnosticCode::ForegroundOpaque,
            severity: Severity::Warning,
            message: format!(
                "foreground image has no transparent pixels, so it covers the background layer \
                 and the icon is a flat {width}×{height}px image"
            ),
            location: None,
            suggestion: Some(
                "Export the foreground with a transparent canvas, so only the artwork sits over \
                 the background."
                    .to_owned(),
            ),
        }),
        LayerKind::Foreground => match content {
            None => diagnostics.push(Diagnostic {
                code: DiagnosticCode::EmptyArtwork,
                severity: Severity::Warning,
                message: "foreground image has no painted content, so the icon is invisible"
                    .to_owned(),
                location: None,
                suggestion: Some(
                    "Check that the artwork was exported onto the canvas and not clipped away."
                        .to_owned(),
                ),
            }),
            Some(_) => diagnostics.extend(image.reach.and_then(|reach| {
                adaptive::safe_zone_finding(
                    reach,
                    "painted content",
                    format!(
                        "vdt cannot scale a raster layer without resampling it, so redraw the \
                         artwork smaller on the same canvas: it should sit inside a centred \
                         circle {}dp across on the {size}dp layer.",
                        crate::xml::number(crate::adaptive::ADAPTIVE_ICON_SAFE_ZONE)
                    ),
                )
            })),
        },
    }

    let shortest = width.min(height);
    if shortest < ADAPTIVE_ICON_MAX_PIXELS {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::BackgroundImageResolution,
            severity: Severity::Warning,
            message: format!(
                "{label} image is {width}×{height}px; an xxxhdpi device draws the {size}dp layer \
                 at {ADAPTIVE_ICON_MAX_PIXELS}px, so it is upscaled and looks soft"
            ),
            location: None,
            suggestion: Some(format!(
                "Export the image at {ADAPTIVE_ICON_MAX_PIXELS}×{ADAPTIVE_ICON_MAX_PIXELS}px or \
                 larger."
            )),
        });
    } else if shortest > ADAPTIVE_ICON_MAX_PIXELS {
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::BackgroundImageResolution,
            severity: Severity::Info,
            message: format!(
                "{label} image is {width}×{height}px, larger than the \
                 {ADAPTIVE_ICON_MAX_PIXELS}px an xxxhdpi device draws; the extra pixels are \
                 decoded into memory and scaled away every time the icon is drawn"
            ),
            location: None,
            suggestion: Some(format!(
                "Export the image at {ADAPTIVE_ICON_MAX_PIXELS}×{ADAPTIVE_ICON_MAX_PIXELS}px to \
                 ship the smallest file that still draws sharply."
            )),
        });
    }

    diagnostics
}
