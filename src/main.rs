use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use unicode_normalization::UnicodeNormalization;
use vdtoolkit::{
    Analysis, Asset, Bounds, Compatibility, Diagnostic, DiagnosticCode, Error, Fit, FitMode,
    IconKind, ImageFormat, Metrics, Result, Severity,
};
use walkdir::WalkDir;

#[derive(Parser)]
#[command(name = "vdt", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Convert SVG files to deterministic VectorDrawable XML.
    Convert(ConvertArgs),
    /// Report whether SVG files are exactly convertible.
    Check(ReportArgs),
    /// Report compatibility, diagnostics, API level, and metrics.
    Inspect(ReportArgs),
    /// Convert and reduce safe numeric precision for Android.
    Optimize(ConvertArgs),
    /// Generate an adaptive launcher icon and its layer drawables.
    Adaptive(AdaptiveArgs),
    /// Convert SVG files to white 24dp notification icon drawables.
    Notification(NotificationArgs),
}

#[derive(Args)]
struct ConvertArgs {
    /// SVG file or directory to convert.
    input: PathBuf,
    /// Output XML file or directory. Required for directory input.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Size in dp of the drawable's longer side; the other side keeps the
    /// aspect ratio and the drawing is unchanged. Defaults to the SVG's width
    /// and height, or 24 for an SVG with only a viewBox larger than 200.
    #[arg(long, value_name = "DP")]
    size: Option<f32>,
    /// Reject SVGs that require safe normalization.
    #[arg(long)]
    strict: bool,
    /// Allow lossy lowering for constructs VectorDrawable cannot draw exactly.
    #[arg(long)]
    allow_approximate: bool,
    /// Keep readable indentation and line breaks instead of compact XML.
    #[arg(long)]
    pretty: bool,
}

#[derive(Args)]
struct NotificationArgs {
    /// SVG file or directory to convert.
    input: PathBuf,
    /// Output XML file or directory. Required for directory input.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Size in dp of the centered square the artwork is scaled to fit on the
    /// 24dp canvas. 24, the default, keeps artwork that already carries its
    /// own padding, such as a Material system icon, at its drawn size; use 20
    /// for artwork drawn edge to edge.
    #[arg(long, default_value_t = vdtoolkit::NOTIFICATION_ICON_SIZE)]
    fit: f32,
    /// Shorten numbers where it cannot change rendering, as `optimize` does.
    #[arg(long)]
    optimize: bool,
    /// Reject SVGs that require safe normalization.
    #[arg(long)]
    strict: bool,
    /// Allow lossy lowering for constructs VectorDrawable cannot draw exactly.
    #[arg(long)]
    allow_approximate: bool,
    /// Keep readable indentation and line breaks instead of compact XML.
    #[arg(long)]
    pretty: bool,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("background_layer")
        .required(true)
        .args(["background", "background_color", "background_image"])
), group(
    ArgGroup::new("foreground_layer")
        .required(true)
        .args(["foreground", "foreground_image"])
), group(
    ArgGroup::new("monochrome_layer").args(["monochrome", "monochrome_image"])
))]
struct AdaptiveArgs {
    /// Foreground layer SVG.
    #[arg(long)]
    foreground: Option<PathBuf>,
    /// Foreground layer as a PNG or WebP. The file is copied unchanged into
    /// `mipmap-nodpi/` and must already be drawn on the full square layer with
    /// the artwork inside the 66dp safe zone: vdt cannot scale a raster layer
    /// without resampling it. A JPEG is rejected, because a layer with no
    /// alpha channel would hide the background.
    #[arg(long, value_name = "IMAGE")]
    foreground_image: Option<PathBuf>,
    /// Background layer SVG, scaled onto the whole 108dp layer.
    #[arg(long)]
    background: Option<PathBuf>,
    /// How a background SVG is scaled onto the 108dp layer. `cover`, the
    /// default, fills the layer and lets it crop whatever overflows, so
    /// non-square artwork still reaches every edge. `contain` scales all of
    /// the artwork in, which leaves non-square artwork letterboxed and the
    /// bands showing through launcher masks and parallax.
    #[arg(long, value_enum, value_name = "MODE")]
    background_fit: Option<FitModeArg>,
    /// Background layer as a PNG, WebP, or JPEG. The file is copied unchanged into
    /// `mipmap-nodpi/`, where Android draws it onto the layer without scaling
    /// it for density. vdt never resamples the image, but it does decode it to
    /// report what the layer will look like.
    #[arg(long, value_name = "IMAGE")]
    background_image: Option<PathBuf>,
    /// Solid background color as #RRGGBB or #AARRGGBB, written as a color
    /// resource instead of a drawable.
    #[arg(long, value_name = "COLOR")]
    background_color: Option<String>,
    /// Monochrome layer SVG for themed icons on Android 13 and newer.
    #[arg(long)]
    monochrome: Option<PathBuf>,
    /// Monochrome layer as a PNG or WebP, placed like --foreground-image.
    #[arg(long, value_name = "IMAGE")]
    monochrome_image: Option<PathBuf>,
    /// Resource name of the icon and prefix of its layers.
    #[arg(long, default_value = "ic_launcher")]
    name: String,
    /// Size in dp of the centered square that the foreground and monochrome
    /// artwork is scaled to fit. 108 fills the layer. Android recommends 48 to
    /// 66 for a logo; 66 is the safe zone that no launcher mask hides.
    #[arg(long)]
    fit: Option<f32>,
    /// Also write a legacy icon for devices below API 26: the background and
    /// foreground under a circular mask. It is a vector in `mipmap/`, or, when
    /// the art needs API 24, a vector in `mipmap-anydpi-v24/` plus lossless
    /// WebPs in `mipmap-mdpi/` through `mipmap-xxxhdpi/`.
    #[arg(long)]
    legacy: bool,
    /// Shorten numbers in every layer drawable and the legacy vector where it
    /// cannot change rendering, as `optimize` does. It runs after the fit,
    /// which is what introduces long decimals, so placement is unchanged.
    #[arg(long)]
    optimize: bool,
    /// Android `res/` directory to write into.
    #[arg(short, long, value_name = "RES_DIR")]
    output: PathBuf,
    /// Reject SVGs that require safe normalization.
    #[arg(long)]
    strict: bool,
    /// Allow lossy lowering for constructs VectorDrawable cannot draw exactly.
    #[arg(long)]
    allow_approximate: bool,
    /// Keep readable indentation and line breaks instead of compact XML.
    #[arg(long)]
    pretty: bool,
}

#[derive(Args)]
struct ReportArgs {
    /// SVG file or directory to inspect. With `--as adaptive-foreground` or
    /// `--as adaptive-background`, a PNG, WebP, or JPEG layer image too.
    input: PathBuf,
    /// Report format.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
    /// Also report what making the file into this kind of icon would find,
    /// as `notification` or `adaptive` would, without writing anything.
    /// A monochrome layer follows the adaptive-foreground rules. A PNG,
    /// WebP, or JPEG is inspected as an adaptive layer image.
    #[arg(long = "as", value_enum, value_name = "KIND")]
    as_kind: Option<IconKindArg>,
    /// Size in dp of the centered square the artwork is scaled to fit on the
    /// canvas of the kind given with --as. Defaults to the whole canvas: 24
    /// for a notification icon and 108 for an adaptive layer.
    #[arg(long, requires = "as_kind")]
    fit: Option<f32>,
    /// How the artwork is scaled onto the square given by --fit, for
    /// `--as adaptive-background`. Defaults to what `adaptive` uses: `cover`.
    #[arg(long, value_enum, value_name = "MODE", requires = "as_kind")]
    background_fit: Option<FitModeArg>,
    /// Treat safe normalization as incompatible.
    #[arg(long)]
    strict: bool,
    /// Allow lossy lowering for constructs VectorDrawable cannot draw exactly.
    #[arg(long)]
    allow_approximate: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum IconKindArg {
    /// White 24dp notification icon.
    Notification,
    /// Foreground or monochrome layer of an adaptive launcher icon.
    AdaptiveForeground,
    /// Background layer of an adaptive launcher icon.
    AdaptiveBackground,
}

impl From<IconKindArg> for IconKind {
    fn from(kind: IconKindArg) -> Self {
        match kind {
            IconKindArg::Notification => Self::Notification,
            IconKindArg::AdaptiveForeground => Self::AdaptiveForeground,
            IconKindArg::AdaptiveBackground => Self::AdaptiveBackground,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum FitModeArg {
    /// Scale the artwork until it covers the layer, cropping the overflow.
    Cover,
    /// Scale all of the artwork onto the layer, leaving bands unpainted.
    Contain,
}

impl From<FitModeArg> for FitMode {
    fn from(mode: FitModeArg) -> Self {
        match mode {
            FitModeArg::Cover => Self::Cover,
            FitModeArg::Contain => Self::Contain,
        }
    }
}

/// The icon a report was made for, when `--as` was given.
#[derive(Clone, Copy, Serialize)]
struct IconTarget {
    kind: IconKind,
    fit: f32,
    fit_mode: FitMode,
}

impl IconTarget {
    /// The placement the report's analysis is made with.
    fn placement(self) -> Fit {
        Fit {
            size: self.fit,
            mode: self.fit_mode,
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Json,
}

#[derive(Serialize)]
struct FileAnalysis<'a> {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    icon: Option<ReportIcon>,
    compatibility: Compatibility,
    minimum_api: Option<u32>,
    diagnostics: &'a [Diagnostic],
    metrics: ReportMetrics<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<ImageFormat>,
}

impl<'a> FileAnalysis<'a> {
    fn new(path: String, icon: Option<IconTarget>, analysis: &'a Analysis) -> Self {
        let raster = analysis.image.is_some();
        Self {
            path,
            icon: icon.map(|icon| {
                if raster {
                    ReportIcon::Raster { kind: icon.kind }
                } else {
                    ReportIcon::Vector(icon)
                }
            }),
            compatibility: analysis.compatibility,
            minimum_api: analysis.minimum_api,
            diagnostics: &analysis.diagnostics,
            metrics: if raster {
                ReportMetrics::Raster(RasterMetrics::from(&analysis.metrics))
            } else {
                ReportMetrics::Vector(&analysis.metrics)
            },
            image: analysis.image,
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
enum ReportIcon {
    Vector(IconTarget),
    Raster { kind: IconKind },
}

#[derive(Serialize)]
#[serde(untagged)]
enum ReportMetrics<'a> {
    Vector(&'a Metrics),
    Raster(RasterMetrics<'a>),
}

#[derive(Serialize)]
struct RasterMetrics<'a> {
    width: f32,
    height: f32,
    viewport_width: f32,
    viewport_height: f32,
    content_bounds: &'a Option<Bounds>,
}

impl<'a> From<&'a Metrics> for RasterMetrics<'a> {
    fn from(metrics: &'a Metrics) -> Self {
        Self {
            width: metrics.width,
            height: metrics.height,
            viewport_width: metrics.viewport_width,
            viewport_height: metrics.viewport_height,
            content_bounds: &metrics.content_bounds,
        }
    }
}

#[derive(Serialize)]
struct FileFailure {
    path: String,
    error: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum ReportEntry<'a> {
    Analysis(FileAnalysis<'a>),
    Failed(FileFailure),
}

/// Overall result of a command across every input file.
#[derive(Clone, Copy)]
enum Outcome {
    Passed,
    Incompatible,
    Failed,
}

fn main() -> ExitCode {
    let args = normalized_args();
    let cli = Cli::parse_from(args);
    match run(cli) {
        Ok(Outcome::Passed) => ExitCode::SUCCESS,
        Ok(Outcome::Incompatible) => ExitCode::from(2),
        Ok(Outcome::Failed) => ExitCode::from(1),
        Err(error) => {
            print_error(None, &error);
            ExitCode::from(1)
        }
    }
}

fn normalized_args() -> Vec<OsString> {
    let mut args: Vec<_> = std::env::args_os().collect();
    if args
        .get(1)
        .and_then(|value| value.to_str())
        .is_some_and(|first| {
            !matches!(
                first,
                "convert" | "check" | "inspect" | "optimize" | "adaptive" | "notification"
            ) && !first.starts_with('-')
        })
    {
        args.insert(1, OsString::from("convert"));
    }
    args
}

fn run(cli: Cli) -> Result<Outcome> {
    match cli.command {
        Command::Convert(args) => convert(args, false),
        Command::Optimize(args) => convert(args, true),
        Command::Check(args) => report(args, false),
        Command::Inspect(args) => report(args, true),
        Command::Adaptive(args) => adaptive(args),
        Command::Notification(args) => notification(args),
    }
}

fn notification(args: NotificationArgs) -> Result<Outcome> {
    let inputs = inputs(&args.input)?;
    if args.input.is_dir() && args.output.is_none() {
        return Err(Error::InvalidInput(
            "directory conversion requires an output directory".to_owned(),
        ));
    }
    if !(args.fit > 0.0 && args.fit <= vdtoolkit::NOTIFICATION_ICON_SIZE) {
        return Err(Error::InvalidInput(format!(
            "--fit must be between 0 and {} dp, got {}",
            vdtoolkit::NOTIFICATION_ICON_SIZE,
            args.fit
        )));
    }
    let outputs = plan_outputs(&args.input, &inputs, args.output.as_deref());
    let mut failures = 0;
    for (input, output) in inputs.iter().zip(outputs) {
        if let Err(error) = output.and_then(|output| notification_one(&args, input, output)) {
            print_error(Some(input), &error);
            failures += 1;
        }
    }
    if failures > 0 && inputs.len() > 1 {
        eprintln!("{failures} of {} SVGs failed", inputs.len());
    }
    Ok(if failures == 0 {
        Outcome::Passed
    } else {
        Outcome::Failed
    })
}

fn notification_one(args: &NotificationArgs, input: &Path, output: Option<Output>) -> Result<()> {
    let mut asset = vdtoolkit::convert_file_with_options(input, args.allow_approximate)
        .map_err(hint_raster_convert)?;
    if args.strict && asset.analysis.compatibility != Compatibility::Exact {
        return Err(Error::InvalidInput(
            "requires normalization and was rejected by --strict".to_owned(),
        ));
    }
    asset.to_icon(IconKind::Notification, Fit::contain(args.fit))?;
    print_findings(input, &asset.analysis);
    if args.optimize {
        asset.optimize();
    }
    let xml = asset_xml(&asset, args.pretty);
    match output {
        Some(output) => output.write(input, &xml)?,
        None => print!("{xml}"),
    }
    Ok(())
}

/// Print a converted file's warnings, and the flattening note a notification
/// icon carries, with their codes. Normalization notes stay with `inspect`.
fn print_findings(input: &Path, analysis: &Analysis) {
    print_diagnostics(input, &analysis.diagnostics);
}

/// Print the warnings and the notes that name something vdt changed or chose,
/// for findings that do not come from converting a file.
fn print_diagnostics(input: &Path, diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        // Notes about the icon being generated are printed: what the generator
        // changed about the artwork, and artwork close enough to the mask edge
        // that some launchers clip it. Notes that describe the source SVG in
        // general are left to inspect.
        let about_this_icon = [
            DiagnosticCode::PaintFlattened.as_str(),
            DiagnosticCode::BackgroundCropped.as_str(),
            DiagnosticCode::OutsideSafeZone.as_str(),
        ]
        .contains(&diagnostic.code.as_str());
        let label = match diagnostic.severity {
            Severity::Warning => "warning",
            Severity::Info if about_this_icon => "note",
            Severity::Info | Severity::Error => continue,
        };
        eprintln!(
            "{label}: {}: {}  {}",
            input.display(),
            diagnostic.code.as_str(),
            diagnostic.message
        );
    }
}

fn adaptive(args: AdaptiveArgs) -> Result<Outcome> {
    validate_resource_name(&args.name)?;
    // --fit places vector artwork. A raster layer is copied, never resampled,
    // so there is nothing for it to place.
    if args.fit.is_some() && args.foreground.is_none() && args.monochrome.is_none() {
        return Err(Error::InvalidInput(
            "--fit places vector artwork, and vdt cannot scale a raster layer without resampling \
             it; draw the image with its artwork already inside the 66dp safe zone"
                .to_owned(),
        ));
    }
    let fit = args.fit.unwrap_or(vdtoolkit::ADAPTIVE_ICON_SIZE);
    if !(fit > 0.0 && fit <= vdtoolkit::ADAPTIVE_ICON_SIZE) {
        return Err(Error::InvalidInput(format!(
            "--fit must be between 0 and {} dp, got {fit}",
            vdtoolkit::ADAPTIVE_ICON_SIZE
        )));
    }
    if args.background_fit.is_some() && args.background.is_none() {
        return Err(Error::InvalidInput(
            "--background-fit applies to --background, which is the only background vdt places"
                .to_owned(),
        ));
    }
    if args.foreground_image.is_some() && args.legacy {
        return Err(Error::InvalidInput(
            "--legacy composes the foreground into a vector icon, which it cannot do for \
             --foreground-image; use --foreground, or drop --legacy"
                .to_owned(),
        ));
    }
    if args.background_image.is_some() && args.legacy {
        // The legacy icon is one vector composed from both layers, and vdt
        // cannot draw a bitmap into a vector.
        return Err(Error::InvalidInput(
            "--legacy composes the background into a vector icon, which it cannot do for \
             --background-image; use --background or --background-color, or drop --legacy"
                .to_owned(),
        ));
    }
    let color = args
        .background_color
        .as_deref()
        .map(normalize_color)
        .transpose()?;
    let drawable_dir = args.output.join("drawable-anydpi");
    let mipmap_dir = args.output.join("mipmap-anydpi-v26");

    let foreground_name = format!("{}_foreground", args.name);
    let background_name = format!("{}_background", args.name);
    let monochrome_name = format!("{}_monochrome", args.name);

    let raster_dir = args.output.join("mipmap-nodpi");
    let mut files: Vec<(PathBuf, String)> = Vec::new();
    let mut images: Vec<(PathBuf, Vec<u8>)> = Vec::new();

    // Every layer is read before anything is written, so a failing layer
    // leaves the resource directory untouched.
    let (foreground, foreground_reference) = match (&args.foreground, &args.foreground_image) {
        (Some(path), _) => {
            let Some(layer) = adaptive_layer(
                &args,
                path,
                Fit::contain(fit),
                IconKind::AdaptiveForeground,
                "--foreground-image",
            ) else {
                return Ok(Outcome::Failed);
            };
            files.push((
                drawable_dir.join(&foreground_name).with_extension("xml"),
                drawable_xml(&layer, args.optimize, args.pretty),
            ));
            (Some(layer), format!("@drawable/{foreground_name}"))
        }
        (None, Some(path)) => {
            images.push(raster_layer(
                path,
                &raster_dir,
                &foreground_name,
                vdtoolkit::LayerKind::Foreground,
            )?);
            (None, format!("@mipmap/{foreground_name}"))
        }
        (None, None) => unreachable!("clap requires a foreground layer"),
    };
    let (background, background_reference) = match (&args.background, &args.background_image, color)
    {
        (Some(path), _, _) => {
            let Some(layer) = adaptive_layer(
                &args,
                path,
                Fit {
                    size: vdtoolkit::ADAPTIVE_ICON_SIZE,
                    mode: args.background_fit.map_or(
                        IconKind::AdaptiveBackground.default_fit().mode,
                        FitMode::from,
                    ),
                },
                IconKind::AdaptiveBackground,
                "--background-image",
            ) else {
                return Ok(Outcome::Failed);
            };
            files.push((
                drawable_dir.join(&background_name).with_extension("xml"),
                drawable_xml(&layer, args.optimize, args.pretty),
            ));
            (Some(layer), format!("@drawable/{background_name}"))
        }
        // The image is copied, never resampled, so what is written is the
        // source byte for byte; decoding it only reports what the layer
        // will look like on a device.
        (None, Some(path), _) => {
            images.push(raster_layer(
                path,
                &raster_dir,
                &background_name,
                vdtoolkit::LayerKind::Background,
            )?);
            (None, format!("@mipmap/{background_name}"))
        }
        (None, None, Some(color)) => {
            files.push((
                args.output
                    .join("values")
                    .join(&background_name)
                    .with_extension("xml"),
                format_xml(
                    vdtoolkit::color_resource_xml(&background_name, &color),
                    args.pretty,
                ),
            ));
            (
                Some(vdtoolkit::Asset::solid_adaptive_layer(&color)?),
                format!("@color/{background_name}"),
            )
        }
        (None, None, None) => unreachable!("clap requires a background layer"),
    };
    let monochrome_reference = match (&args.monochrome, &args.monochrome_image) {
        (Some(path), _) => {
            let Some(layer) = adaptive_layer(
                &args,
                path,
                Fit::contain(fit),
                IconKind::AdaptiveForeground,
                "--monochrome-image",
            ) else {
                return Ok(Outcome::Failed);
            };
            files.push((
                drawable_dir.join(&monochrome_name).with_extension("xml"),
                drawable_xml(&layer, args.optimize, args.pretty),
            ));
            Some(format!("@drawable/{monochrome_name}"))
        }
        (None, Some(path)) => {
            images.push(raster_layer(
                path,
                &raster_dir,
                &monochrome_name,
                vdtoolkit::LayerKind::Foreground,
            )?);
            Some(format!("@mipmap/{monochrome_name}"))
        }
        (None, None) => None,
    };
    let icon = format_xml(
        vdtoolkit::adaptive_icon_xml(
            &background_reference,
            &foreground_reference,
            monochrome_reference.as_deref(),
        ),
        args.pretty,
    );
    let round_name = format!("{}_round", args.name);
    files.push((
        mipmap_dir.join(&args.name).with_extension("xml"),
        icon.clone(),
    ));
    files.push((mipmap_dir.join(&round_name).with_extension("xml"), icon));
    // The two legacy layouts are exclusive, and a qualified resource left
    // behind by an earlier run would outrank the new one on API 24 and 25, so
    // whichever layout is not written this time is removed.
    let written: Vec<&PathBuf> = files
        .iter()
        .map(|(path, _)| path)
        .chain(images.iter().map(|(path, _)| path))
        .collect();
    // A file an earlier run left behind is removed only when keeping it would
    // break the build or show the wrong icon. Anything else is merely unused,
    // and vdt cannot tell whether the rest of the project refers to it, so it
    // is named and left alone: deleting a resource something still uses would
    // break the build itself.
    let mut stale: Vec<PathBuf> = Vec::new();
    let mut unused: Vec<PathBuf> = Vec::new();
    let mipmap_dirs = mipmap_dirs(&args.output);
    let drawable_dirs = resource_dirs(&args.output, "drawable");
    for name in [&foreground_name, &background_name, &monochrome_name] {
        // Every file named for the layer in any mipmap folder is a version of
        // the one @mipmap resource a raster layer is written as.
        let alternatives = resource_files(&mipmap_dirs, name);
        // When this run writes that resource into mipmap-nodpi, every other
        // version breaks the icon: one in the same folder is the same resource
        // twice, which aapt2 refuses to build, and one in a density folder,
        // which Android Studio writes for every density, is chosen over
        // mipmap-nodpi on a device of that density, so the old layer keeps
        // showing. They are versions of the resource vdt writes, so nothing
        // else can be using them.
        let wrote_raster = images
            .iter()
            .any(|(path, _)| android_resource_name(path) == Some(name));
        for path in alternatives
            .into_iter()
            .filter(|path| !written.contains(&path))
        {
            if wrote_raster {
                stale.push(path);
            } else {
                unused.push(path);
            }
        }
        // A vector layer is a @drawable and a raster one a @mipmap, so one left
        // behind by the other form never collides with it.
        for path in resource_files(&drawable_dirs, name)
            .into_iter()
            .filter(|path| !written.contains(&path))
        {
            unused.push(path);
        }
    }
    // A color background is a @color, which nothing else vdt writes collides
    // with, and a values file can hold any number of other resources besides.
    let background_values = args
        .output
        .join("values")
        .join(&background_name)
        .with_extension("xml");
    if !written.contains(&&background_values) {
        unused.push(background_values);
    }
    // These adaptive XML resources are always written. Another file with the
    // same resource name beside either one makes aapt2 reject the project,
    // whether or not this run also writes a legacy icon.
    for name in [&args.name, &round_name] {
        stale.extend(
            resource_files(std::slice::from_ref(&mipmap_dir), name)
                .into_iter()
                .filter(|path| !written.contains(&path)),
        );
    }
    if let (true, Some(background), Some(foreground)) = (args.legacy, &background, &foreground) {
        let legacy = vdtoolkit::Asset::legacy_launcher_icon(background, foreground);
        let xml = drawable_xml(&legacy, args.optimize, args.pretty);
        let names = [args.name.as_str(), round_name.as_str()];
        let plain: Vec<PathBuf> = names
            .iter()
            .map(|name| args.output.join("mipmap").join(name).with_extension("xml"))
            .collect();
        let mut split: Vec<PathBuf> = names
            .iter()
            .map(|name| {
                args.output
                    .join("mipmap-anydpi-v24")
                    .join(name)
                    .with_extension("xml")
            })
            .collect();
        for (density, _) in vdtoolkit::LEGACY_ICON_DENSITIES {
            for name in names {
                split.push(
                    args.output
                        .join(format!("mipmap-{density}"))
                        .join(name)
                        .with_extension("webp"),
                );
            }
        }
        if legacy.analysis.minimum_api == Some(21) {
            for path in plain {
                files.push((path, xml.clone()));
            }
        } else {
            // Gradients, even-odd fills, or a second clip need API 24. The exact
            // vector serves API 24 and 25 from an anydpi folder, which outranks
            // density folders, and lossless WebPs rendered from it serve API
            // 21 to 23.
            for path in &split[..2] {
                files.push((path.clone(), xml.clone()));
            }
            for ((density, pixels), pair) in vdtoolkit::LEGACY_ICON_DENSITIES
                .iter()
                .zip(split[2..].chunks(2))
            {
                debug_assert!(pair[0].to_string_lossy().contains(density));
                let webp = legacy.to_webp(*pixels, *pixels)?;
                images.push((pair[0].clone(), webp.clone()));
                images.push((pair[1].clone(), webp));
            }
        }
        // Every other file named for the legacy icon in any mipmap folder is a
        // version of the resource this run writes, and it breaks the icon: in
        // a folder vdt writes to it is the same resource twice, which aapt2
        // refuses to build, and anywhere else it is chosen over vdt's version on
        // some device. Android Studio writes the legacy icon as .webp into every
        // density folder, so a Studio project hits both cases. The adaptive
        // icon directory is owned by the adaptive resources handled above.
        let written: Vec<&PathBuf> = files
            .iter()
            .map(|(path, _)| path)
            .chain(images.iter().map(|(path, _)| path))
            .collect();
        for name in names {
            stale.extend(
                mipmap_dirs
                    .iter()
                    .filter(|dir| *dir != &mipmap_dir)
                    .flat_map(|dir| resource_files(std::slice::from_ref(dir), name))
                    .filter(|path| !written.contains(&path)),
            );
        }
    }

    for (path, contents) in &files {
        write_file(path, contents)?;
        println!("{}", path.display());
    }
    for (path, contents) in &images {
        write_file(path, contents)?;
        println!("{}", path.display());
    }
    for path in stale.iter().filter(|path| path.is_file()) {
        std::fs::remove_file(path).map_err(|source| Error::Write {
            path: path.clone(),
            source,
        })?;
        eprintln!("removed stale resource {}", path.display());
    }
    for path in unused.iter().filter(|path| path.is_file()) {
        eprintln!(
            "note: the icon no longer uses {}; remove it if nothing else in the project refers \
             to it",
            path.display()
        );
    }
    Ok(Outcome::Passed)
}

/// Serialize a drawable, shortening its numbers first when asked.
///
/// The asset itself stays exact: the legacy icon is composed from the fitted
/// layers and would otherwise be rounded twice, and the WebPs are rendered
/// from whichever vector is written.
fn drawable_xml(asset: &Asset, optimize: bool, pretty: bool) -> String {
    if optimize {
        let mut asset = asset.clone();
        asset.optimize();
        asset_xml(&asset, pretty)
    } else {
        asset_xml(asset, pretty)
    }
}

fn asset_xml(asset: &Asset, pretty: bool) -> String {
    if pretty {
        asset.to_xml()
    } else {
        asset.to_compact_xml()
    }
}

fn format_xml(xml: String, pretty: bool) -> String {
    if pretty {
        xml
    } else {
        vdtoolkit::compact_xml(&xml)
    }
}

/// Every mipmap folder in a resource directory, qualified or not, in a stable
/// order.
fn mipmap_dirs(res: &Path) -> Vec<PathBuf> {
    let mut dirs = resource_dirs(res, "mipmap");
    // mipmap-nodpi is where this run writes, and may not exist yet.
    let nodpi = res.join("mipmap-nodpi");
    if !dirs.contains(&nodpi) {
        dirs.push(nodpi);
    }
    dirs.sort();
    dirs
}

/// Every folder for one Android resource type, qualified or not, in a stable
/// order.
fn resource_dirs(res: &Path, resource_type: &str) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(res)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
        .filter(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            name == resource_type
                || name
                    .strip_prefix(resource_type)
                    .is_some_and(|qualifiers| qualifiers.starts_with('-'))
        })
        .map(|entry| entry.path())
        .collect();
    dirs.sort();
    dirs
}

/// Existing files in `dirs` whose Android resource name exactly matches
/// `name`. Android derives a file resource's name from everything before its
/// first dot, so this includes compound extensions such as `.9.png` without
/// treating similarly prefixed resources as alternatives.
fn resource_files(dirs: &[PathBuf], name: &str) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = dirs
        .iter()
        .flat_map(|dir| std::fs::read_dir(dir).into_iter().flatten().flatten())
        .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
        .map(|entry| entry.path())
        .filter(|path| android_resource_name(path) == Some(name))
        .collect();
    files.sort();
    files
}

fn android_resource_name(path: &Path) -> Option<&str> {
    path.file_name()?.to_str()?.split('.').next()
}

/// Read a raster layer, report what it will look like, and return the file to
/// write with the extension the image's own header calls for.
///
/// The bytes are the source's, unchanged: vdt places the image and never
/// resamples it.
fn raster_layer(
    input: &Path,
    dir: &Path,
    name: &str,
    layer: vdtoolkit::LayerKind,
) -> Result<(PathBuf, Vec<u8>)> {
    let data = std::fs::read(input).map_err(|source| Error::Read {
        path: input.to_owned(),
        source,
    })?;
    let (image, diagnostics) = vdtoolkit::analyze_layer_image(&data, layer)?;
    print_diagnostics(input, &diagnostics);
    Ok((
        dir.join(name).with_extension(image.format.extension()),
        data,
    ))
}

/// Convert and fit one layer as `kind`, printing its findings, or report the
/// failure with its path and return `None`.
fn adaptive_layer(
    args: &AdaptiveArgs,
    input: &Path,
    fit: Fit,
    kind: IconKind,
    image_flag: &str,
) -> Option<Asset> {
    let layer = || -> Result<Asset> {
        let mut asset = vdtoolkit::convert_file_with_options(input, args.allow_approximate)
            .map_err(|error| match error {
                Error::InvalidInput(message) if message == "this is a raster image, not an SVG" => {
                    Error::InvalidInput(format!(
                        "{message}; use {image_flag} for a raster adaptive layer"
                    ))
                }
                error => error,
            })?;
        if args.strict && asset.analysis.compatibility != Compatibility::Exact {
            return Err(Error::InvalidInput(
                "requires normalization and was rejected by --strict".to_owned(),
            ));
        }
        asset.to_icon(kind, fit)?;
        Ok(asset)
    };
    match layer() {
        Ok(asset) => {
            print_findings(input, &asset.analysis);
            Some(asset)
        }
        Err(error) => {
            print_error(Some(input), &error);
            None
        }
    }
}

fn validate_resource_name(name: &str) -> Result<()> {
    if is_resource_name(name) {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!(
            "resource name must match [a-z_][a-z0-9_]* and not be a Java keyword, got {name:?}"
        )))
    }
}

/// Java keywords and literals, which aapt2 rejects as resource names because
/// they cannot be fields of the generated `R` class.
const JAVA_KEYWORDS: &[&str] = &[
    "abstract",
    "assert",
    "boolean",
    "break",
    "byte",
    "case",
    "catch",
    "char",
    "class",
    "const",
    "continue",
    "default",
    "do",
    "double",
    "else",
    "enum",
    "extends",
    "false",
    "final",
    "finally",
    "float",
    "for",
    "goto",
    "if",
    "implements",
    "import",
    "instanceof",
    "int",
    "interface",
    "long",
    "native",
    "new",
    "null",
    "package",
    "private",
    "protected",
    "public",
    "return",
    "short",
    "static",
    "strictfp",
    "super",
    "switch",
    "synchronized",
    "this",
    "throw",
    "throws",
    "transient",
    "true",
    "try",
    "void",
    "volatile",
    "while",
];

fn is_resource_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        && name != "_"
        && !JAVA_KEYWORDS.contains(&name)
}

/// Turn a file name into a valid Android resource name. A valid name is kept;
/// otherwise words become lowercase and are joined by underscores, as
/// `Arrow-Left` becomes `arrow_left` and `HTTPServer` becomes `http_server`,
/// and a name that would start with a digit or be a Java keyword gains an
/// `ic_` prefix. Accents are dropped, so `Café` becomes `cafe`, and symbols
/// that carry meaning become words, so `C++` becomes `c_plus_plus` rather than
/// colliding with `C`.
fn resource_name(stem: &str) -> Result<String> {
    if is_resource_name(stem) {
        return Ok(stem.to_owned());
    }
    let mut chars = Vec::with_capacity(stem.len());
    for c in stem.nfkd().filter(|c| !('\u{300}'..='\u{36f}').contains(c)) {
        let word = match c {
            '+' => "plus",
            '#' => "sharp",
            '&' => "and",
            '@' => "at",
            '%' => "percent",
            _ => {
                chars.push(c);
                continue;
            }
        };
        chars.push('_');
        chars.extend(word.chars());
        chars.push('_');
    }
    let mut name = String::with_capacity(stem.len());
    for (index, &c) in chars.iter().enumerate() {
        if c.is_ascii_uppercase() {
            let previous = index.checked_sub(1).map(|previous| chars[previous]);
            let next = chars.get(index + 1);
            // A word starts after a lowercase letter or digit, and at the last
            // capital of an acronym followed by a lowercase word.
            let word_start = previous
                .is_some_and(|previous| previous.is_ascii_lowercase() || previous.is_ascii_digit())
                || (previous.is_some_and(|previous| previous.is_ascii_uppercase())
                    && next.is_some_and(char::is_ascii_lowercase));
            if word_start {
                name.push('_');
            }
            name.push(c.to_ascii_lowercase());
        } else if c.is_ascii_lowercase() || c.is_ascii_digit() {
            name.push(c);
        } else if !name.ends_with('_') {
            name.push('_');
        }
    }
    let name = name.trim_matches('_');
    if name.is_empty() {
        return Err(Error::InvalidInput(format!(
            "cannot make an Android resource name from {stem:?}; use letters or digits in the file name"
        )));
    }
    Ok(
        if name.starts_with(|c: char| c.is_ascii_digit()) || JAVA_KEYWORDS.contains(&name) {
            format!("ic_{name}")
        } else {
            name.to_owned()
        },
    )
}

fn normalize_color(color: &str) -> Result<String> {
    let digits = color.strip_prefix('#').unwrap_or(color);
    if matches!(digits.len(), 6 | 8) && digits.chars().all(|c| c.is_ascii_hexdigit()) {
        Ok(format!("#{}", digits.to_ascii_uppercase()))
    } else {
        Err(Error::InvalidInput(format!(
            "background color must be #RRGGBB or #AARRGGBB, got {color:?}"
        )))
    }
}

fn write_file(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|source| Error::Write {
            path: parent.to_owned(),
            source,
        })?;
    }
    std::fs::write(path, contents).map_err(|source| Error::Write {
        path: path.to_owned(),
        source,
    })
}

fn convert(args: ConvertArgs, optimize: bool) -> Result<Outcome> {
    let inputs = inputs(&args.input)?;
    if args.input.is_dir() && args.output.is_none() {
        return Err(Error::InvalidInput(
            "directory conversion requires an output directory".to_owned(),
        ));
    }
    if let Some(size) = args.size.filter(|size| !(size.is_finite() && *size > 0.0)) {
        return Err(Error::InvalidInput(format!(
            "--size must be a positive number of dp, got {size}"
        )));
    }
    let outputs = plan_outputs(&args.input, &inputs, args.output.as_deref());
    let mut failures = 0;
    for (input, output) in inputs.iter().zip(outputs) {
        if let Err(error) = output.and_then(|output| convert_one(&args, input, output, optimize)) {
            print_error(Some(input), &error);
            failures += 1;
        }
    }
    if failures > 0 && inputs.len() > 1 {
        eprintln!("{failures} of {} SVGs failed", inputs.len());
    }
    Ok(if failures == 0 {
        Outcome::Passed
    } else {
        Outcome::Failed
    })
}

fn convert_one(
    args: &ConvertArgs,
    input: &Path,
    output: Option<Output>,
    optimize: bool,
) -> Result<()> {
    let mut asset = vdtoolkit::convert_file_with_options(input, args.allow_approximate)
        .map_err(hint_raster_convert)?;
    if args.strict && asset.analysis.compatibility != Compatibility::Exact {
        return Err(Error::InvalidInput(
            "requires normalization and was rejected by --strict".to_owned(),
        ));
    }
    if let Some(size) = args.size {
        asset.set_size(size)?;
    } else if !asset.has_declared_size() && is_large(&asset.analysis) {
        // Without width and height the viewBox units become dp, and a viewBox
        // this large is a drawing grid, such as Material Symbols' 960, rather
        // than an intended size.
        let metrics = &asset.analysis.metrics;
        let viewbox = (metrics.viewport_width, metrics.viewport_height);
        asset.set_size(DEFAULT_ICON_SIZE)?;
        let metrics = &asset.analysis.metrics;
        eprintln!(
            "note: {}: no width or height and a {}×{} viewBox, so it is drawn at {}×{}dp; pass --size to choose",
            input.display(),
            viewbox.0,
            viewbox.1,
            metrics.width,
            metrics.height
        );
    }
    for diagnostic in &asset.analysis.diagnostics {
        if matches!(diagnostic.severity, Severity::Warning) {
            eprintln!(
                "warning: {}: {}  {}",
                input.display(),
                diagnostic.code.as_str(),
                diagnostic.message
            );
        }
    }
    if args.size.is_none() && is_large(&asset.analysis) {
        eprintln!(
            "hint: pass --size {DEFAULT_ICON_SIZE} to draw it at {DEFAULT_ICON_SIZE}dp; the drawing is unchanged"
        );
    }
    let generated_bytes = asset_xml(&asset, args.pretty).len();
    if optimize {
        asset.optimize();
    }
    let xml = asset_xml(&asset, args.pretty);
    if optimize {
        let original_bytes = std::fs::metadata(input)
            .map(|metadata| metadata.len())
            .unwrap_or_default();
        let reduction = if generated_bytes == 0 {
            0.0
        } else {
            100.0 * (generated_bytes.saturating_sub(xml.len())) as f64 / generated_bytes as f64
        };
        eprintln!("{}", input.display());
        eprintln!("Original SVG: {original_bytes} bytes");
        eprintln!("Generated drawable: {generated_bytes} bytes");
        eprintln!("Optimized drawable: {} bytes", xml.len());
        eprintln!("Reduction: {reduction:.1}%");
    }
    match output {
        Some(output) => output.write(input, &xml)?,
        None => print!("{xml}"),
    }
    Ok(())
}

/// Size in dp of a Material icon, used for an SVG whose only size is a large
/// viewBox.
const DEFAULT_ICON_SIZE: f32 = 24.0;

/// Whether the analysis carries the warning for a drawable larger than
/// Android recommends.
fn is_large(analysis: &Analysis) -> bool {
    analysis
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code.as_str() == DiagnosticCode::LargeDimensions.as_str())
}

fn report(args: ReportArgs, inspect: bool) -> Result<Outcome> {
    let adaptive_layer = matches!(
        args.as_kind,
        Some(IconKindArg::AdaptiveForeground | IconKindArg::AdaptiveBackground)
    );
    let inputs = collect_inputs(
        &args.input,
        |path| {
            is_svg(path)
                || (adaptive_layer
                    && is_layer_image(path)
                    && !(matches!(args.as_kind, Some(IconKindArg::AdaptiveForeground))
                        && is_jpeg(path)))
        },
        if adaptive_layer {
            "SVG or layer image files"
        } else {
            "SVG files"
        },
    )?;
    if args.background_fit.is_some()
        && !matches!(args.as_kind, Some(IconKindArg::AdaptiveBackground))
    {
        return Err(Error::InvalidInput(
            "--background-fit applies to --as adaptive-background".to_owned(),
        ));
    }
    let icon = args.as_kind.map(|kind| {
        let default = IconKind::from(kind).default_fit();
        IconTarget {
            kind: kind.into(),
            fit: args.fit.unwrap_or(default.size),
            fit_mode: args.background_fit.map_or(default.mode, FitMode::from),
        }
    });
    if let Some(icon) = icon.filter(|icon| !(icon.fit > 0.0 && icon.fit <= icon.kind.canvas())) {
        return Err(Error::InvalidInput(format!(
            "--fit must be between 0 and {} dp, got {}",
            icon.kind.canvas(),
            icon.fit
        )));
    }
    let mut reports = Vec::new();
    let mut passed = true;
    let mut failed = 0;
    for input in &inputs {
        let result = match icon {
            Some(icon) => vdtoolkit::analyze_file_as_with_options(
                input,
                icon.kind,
                icon.placement(),
                args.allow_approximate,
            ),
            None => vdtoolkit::analyze_file_with_options(input, args.allow_approximate)
                .map_err(hint_raster_inspect),
        };
        match &result {
            Ok(analysis) => {
                if analysis.image.is_none() {
                    passed &= if args.strict {
                        analysis.compatibility == Compatibility::Exact
                    } else {
                        analysis
                            .compatibility
                            .is_convertible(args.allow_approximate)
                    };
                }
            }
            Err(_) => failed += 1,
        }
        reports.push((input, result));
    }
    match args.format {
        Format::Json => {
            let values: Vec<_> = reports
                .iter()
                .map(|(path, result)| match result {
                    Ok(analysis) => ReportEntry::Analysis(FileAnalysis::new(
                        path.display().to_string(),
                        icon,
                        analysis,
                    )),
                    Err(error) => ReportEntry::Failed(FileFailure {
                        path: path.display().to_string(),
                        error: error.to_string(),
                    }),
                })
                .collect();
            println!("{}", serde_json::to_string_pretty(&values).unwrap());
            for (path, result) in &reports {
                if let Err(error) = result {
                    print_error(Some(path.as_path()), error);
                }
            }
        }
        Format::Human => {
            for (index, (path, result)) in reports.iter().enumerate() {
                if reports.len() > 1 {
                    if index > 0 {
                        println!();
                    }
                    println!("{}", path.display());
                }
                match result {
                    Ok(analysis) => print_human(analysis, inspect, args.allow_approximate),
                    Err(error) => println!("✗ Could not analyze: {error}"),
                }
            }
            if reports.len() > 1 {
                print_summary(&reports);
            }
        }
    }
    Ok(if failed > 0 {
        Outcome::Failed
    } else if passed {
        Outcome::Passed
    } else {
        Outcome::Incompatible
    })
}

fn print_human(analysis: &Analysis, inspect: bool, allow_approximate: bool) {
    let raster = analysis.image.is_some();
    let compatible = analysis.compatibility.is_convertible(allow_approximate);
    let verdict = if raster {
        "Adaptive layer image"
    } else if !compatible {
        "Not exactly representable as VectorDrawable"
    } else if analysis.compatibility == Compatibility::Approximate {
        "VectorDrawable compatible with --allow-approximate"
    } else {
        "VectorDrawable compatible"
    };
    println!(
        "{} {}",
        if raster || compatible { "✓" } else { "✗" },
        verdict
    );
    for diagnostic in &analysis.diagnostics {
        let location = diagnostic
            .location
            .as_ref()
            .map(|value| format!(" ({}:{}:{})", value.element, value.line, value.column))
            .unwrap_or_default();
        println!(
            "{}  {}{}",
            diagnostic.code.as_str(),
            diagnostic.message,
            location
        );
        if let Some(suggestion) = &diagnostic.suggestion {
            println!("  → {suggestion}");
        }
    }
    if inspect {
        let metrics = &analysis.metrics;
        if raster {
            println!(
                "Dimensions: {} × {} px",
                metrics.width as u32, metrics.height as u32
            );
            println!(
                "Layer: {} × {} dp",
                metrics.viewport_width, metrics.viewport_height
            );
        } else {
            println!("Dimensions: {} × {}", metrics.width, metrics.height);
            println!(
                "Viewport: {} × {}",
                metrics.viewport_width, metrics.viewport_height
            );
            println!("Paths: {}", metrics.paths);
            println!("Path commands: {}", metrics.path_commands);
            println!("Groups: {}", metrics.groups);
            println!("Clip paths: {}", metrics.clip_paths);
            println!("Gradients: {}", metrics.gradients);
        }
        match metrics.content_bounds {
            Some(bounds) => println!(
                "Content bounds: left {}, top {}, right {}, bottom {}{}",
                bounds.left,
                bounds.top,
                bounds.right,
                bounds.bottom,
                if raster { " dp" } else { "" }
            ),
            None => println!("Content bounds: none"),
        }
        if !raster {
            println!("Estimated XML size: {} bytes", metrics.estimated_xml_bytes);
        }
    }
    if raster {
        if let Some(format) = analysis.image {
            println!("Format: {}", format.extension());
        }
    } else {
        println!("Compatibility: {:?}", analysis.compatibility);
        match analysis.minimum_api {
            Some(api) => println!("Minimum API for exact rendering: {api}"),
            None => println!("Minimum API for exact rendering: n/a"),
        }
    }
}

fn print_summary(reports: &[(&PathBuf, Result<Analysis>)]) {
    let rasters = reports
        .iter()
        .filter(|(_, result)| {
            result
                .as_ref()
                .is_ok_and(|analysis| analysis.image.is_some())
        })
        .count();
    let count = |compatibility: Compatibility| {
        reports
            .iter()
            .filter(|(_, result)| {
                result.as_ref().is_ok_and(|analysis| {
                    analysis.image.is_none() && analysis.compatibility == compatibility
                })
            })
            .count()
    };
    let failed = reports.iter().filter(|(_, result)| result.is_err()).count();
    println!("\n{} files checked", reports.len());
    println!("{} exact", count(Compatibility::Exact));
    println!(
        "{} exact with normalization",
        count(Compatibility::ExactWithNormalization)
    );
    println!("{} approximate", count(Compatibility::Approximate));
    println!("{} unsupported", count(Compatibility::Unsupported));
    if rasters > 0 {
        println!("{rasters} adaptive layer images");
    }
    if failed > 0 {
        println!("{failed} could not be analyzed");
    }
}

fn inputs(path: &Path) -> Result<Vec<PathBuf>> {
    collect_inputs(path, is_svg, "SVG files")
}

fn collect_inputs(path: &Path, keep: impl Fn(&Path) -> bool, what: &str) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_owned()]);
    }
    if !path.is_dir() {
        return Err(Error::InvalidInput(format!(
            "input does not exist: {}",
            path.display()
        )));
    }
    let mut files: Vec<_> = WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .filter(|path| keep(path))
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(Error::InvalidInput(format!(
            "no {what} found in {}",
            path.display()
        )));
    }
    Ok(files)
}

/// Point inspect/check at `--as` when a raster file is given without a kind.
fn hint_raster_inspect(error: Error) -> Error {
    match error {
        Error::InvalidInput(message) if message == "this is a raster image, not an SVG" => {
            Error::InvalidInput(format!(
                "{message}; pass --as adaptive-foreground or --as adaptive-background to inspect \
                 it as an adaptive layer"
            ))
        }
        error => error,
    }
}

/// Point conversion commands at the adaptive image options and at inspect.
fn hint_raster_convert(error: Error) -> Error {
    match error {
        Error::InvalidInput(message) if message == "this is a raster image, not an SVG" => {
            Error::InvalidInput(format!(
                "{message}; pass it to adaptive with --foreground-image or --background-image, \
                 or use inspect --as adaptive-foreground or --as adaptive-background"
            ))
        }
        error => error,
    }
}

fn is_svg(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
}

fn is_layer_image(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        ["png", "webp", "jpg", "jpeg"]
            .iter()
            .any(|name| extension.eq_ignore_ascii_case(name))
    })
}

fn is_jpeg(path: &Path) -> bool {
    path.extension().is_some_and(|extension| {
        extension.eq_ignore_ascii_case("jpg") || extension.eq_ignore_ascii_case("jpeg")
    })
}

/// Where a converted file is written, and the name it would have had when
/// that is not a valid Android resource name.
struct Output {
    path: PathBuf,
    renamed_from: Option<String>,
}

impl Output {
    fn write(&self, input: &Path, contents: &str) -> Result<()> {
        write_file(&self.path, contents)?;
        if let Some(requested) = &self.renamed_from {
            eprintln!(
                "note: {}: written as {} because {requested:?} is not a valid Android resource name",
                input.display(),
                self.path.display()
            );
        }
        Ok(())
    }
}

/// Plan every input's output up front with a valid Android resource name, so
/// an input whose name is already taken fails instead of overwriting the
/// earlier file. `None` writes to stdout.
fn plan_outputs(
    root: &Path,
    inputs: &[PathBuf],
    output: Option<&Path>,
) -> Vec<Result<Option<Output>>> {
    let mut taken: HashMap<PathBuf, &Path> = HashMap::new();
    inputs
        .iter()
        .map(|input| {
            let Some(output) = output else {
                return Ok(None);
            };
            let (directory, stem, extension) = if root.is_file() {
                (
                    output.parent().unwrap_or(Path::new("")).to_owned(),
                    output.file_stem(),
                    output.extension(),
                )
            } else {
                let relative = input.strip_prefix(root).unwrap_or(input);
                let destination = output.join(relative);
                (
                    destination.parent().unwrap_or(output).to_owned(),
                    input.file_stem(),
                    Some(OsStr::new("xml")),
                )
            };
            let requested = stem
                .map(|stem| stem.to_string_lossy().into_owned())
                .unwrap_or_default();
            let name = resource_name(&requested)?;
            let mut path = directory.join(&name);
            if let Some(extension) = extension {
                path.set_extension(extension);
            }
            if let Some(first) = taken.get(&path) {
                return Err(Error::InvalidInput(format!(
                    "{} is already the output of {}; rename one of them",
                    path.display(),
                    first.display()
                )));
            }
            taken.insert(path.clone(), input);
            Ok(Some(Output {
                renamed_from: (name != requested).then_some(requested),
                path,
            }))
        })
        .collect()
}

fn print_error(path: Option<&Path>, error: &Error) {
    match path {
        Some(path) => eprintln!("error: {}: {error}", path.display()),
        None => eprintln!("error: {error}"),
    }
    if let Error::Incompatible(analysis) = error {
        for diagnostic in &analysis.diagnostics {
            eprintln!("{}  {}", diagnostic.code.as_str(), diagnostic.message);
        }
    }
}
