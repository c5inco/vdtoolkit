//! Render an SVG two ways and measure how far apart they are: the source
//! through resvg, and the converted VectorDrawable through the library's
//! renderer, which follows VectorDrawable semantics.
//!
//! Prints one JSON object per input file: the render size, the mean absolute
//! channel difference over unpremultiplied RGBA, and the fraction of pixels
//! whose largest channel difference is above 32, which is well past the
//! anti-aliasing noise between two rasterizers. Used by
//! `tools/check-illustrations.py`.

use std::path::Path;

use serde::Serialize;

/// Longest side in pixels of every render.
const LONGEST_SIDE: u32 = 256;

/// Channel difference above which a pixel is counted as different.
const PIXEL_THRESHOLD: u8 = 32;

#[derive(Serialize)]
struct Comparison<'a> {
    path: &'a str,
    width: u32,
    height: u32,
    mean_absolute_difference: f64,
    different_pixels: f64,
}

#[derive(Serialize)]
struct Failure<'a> {
    path: &'a str,
    error: String,
}

fn main() {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    if arguments.is_empty() {
        eprintln!("usage: compare <svg>...");
        std::process::exit(2);
    }
    let mut failed = false;
    for argument in &arguments {
        match compare(Path::new(argument)) {
            Ok(comparison) => println!(
                "{}",
                serde_json::to_string(&Comparison {
                    path: argument,
                    ..comparison
                })
                .unwrap()
            ),
            Err(error) => {
                failed = true;
                println!(
                    "{}",
                    serde_json::to_string(&Failure {
                        path: argument,
                        error
                    })
                    .unwrap()
                );
            }
        }
    }
    if failed {
        std::process::exit(1);
    }
}

fn compare(path: &Path) -> Result<Comparison<'static>, String> {
    let source = std::fs::read(path).map_err(|error| error.to_string())?;
    let asset = vdtoolkit::convert(&source).map_err(|error| error.to_string())?;
    let (width, height) = render_size(
        asset.analysis.metrics.viewport_width,
        asset.analysis.metrics.viewport_height,
    );
    let drawable = asset
        .render_rgba(width, height)
        .map_err(|error| error.to_string())?;
    let reference = render_reference(&source, width, height)?;
    let mut total = 0u64;
    let mut different = 0usize;
    for (a, b) in drawable.chunks(4).zip(reference.chunks(4)) {
        let mut largest = 0u8;
        for (x, y) in a.iter().zip(b) {
            let difference = x.abs_diff(*y);
            total += u64::from(difference);
            largest = largest.max(difference);
        }
        if largest > PIXEL_THRESHOLD {
            different += 1;
        }
    }
    let pixels = (width * height) as f64;
    Ok(Comparison {
        path: "",
        width,
        height,
        mean_absolute_difference: total as f64 / (pixels * 4.0),
        different_pixels: different as f64 / pixels,
    })
}

/// Pixel size that puts the longer side on [`LONGEST_SIDE`].
fn render_size(viewport_width: f32, viewport_height: f32) -> (u32, u32) {
    let scale = LONGEST_SIDE as f32 / viewport_width.max(viewport_height);
    let side = |value: f32| ((value * scale).round() as u32).max(1);
    (side(viewport_width), side(viewport_height))
}

/// Render the source SVG with resvg at the same size, as unpremultiplied
/// RGBA, scaling the whole viewBox onto the canvas the way the drawable's
/// viewport is.
fn render_reference(source: &[u8], width: u32, height: u32) -> Result<Vec<u8>, String> {
    let options = usvg::Options::default();
    let tree = usvg::Tree::from_data(source, &options).map_err(|error| error.to_string())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(width, height).ok_or("empty canvas")?;
    let size = tree.size();
    let transform = resvg::tiny_skia::Transform::from_scale(
        width as f32 / size.width(),
        height as f32 / size.height(),
    );
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    Ok(pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect())
}
