use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};
use serde::Serialize;
use vdtoolkit::{Analysis, Asset, Compatibility, Error, Result, Severity};
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
}

#[derive(Args)]
struct ConvertArgs {
    /// SVG file or directory to convert.
    input: PathBuf,
    /// Output XML file or directory. Required for directory input.
    #[arg(short, long)]
    output: Option<PathBuf>,
    /// Reject SVGs that require safe normalization.
    #[arg(long)]
    strict: bool,
}

#[derive(Args)]
#[command(group(
    ArgGroup::new("background_layer")
        .required(true)
        .args(["background", "background_color"])
))]
struct AdaptiveArgs {
    /// Foreground layer SVG.
    #[arg(long)]
    foreground: PathBuf,
    /// Background layer SVG, scaled to fill the whole 108dp layer.
    #[arg(long)]
    background: Option<PathBuf>,
    /// Solid background color as #RRGGBB or #AARRGGBB, written as a color
    /// resource instead of a drawable.
    #[arg(long, value_name = "COLOR")]
    background_color: Option<String>,
    /// Monochrome layer SVG for themed icons on Android 13 and newer.
    #[arg(long)]
    monochrome: Option<PathBuf>,
    /// Resource name of the icon and prefix of its layers.
    #[arg(long, default_value = "ic_launcher")]
    name: String,
    /// Size in dp of the centered square that the foreground and monochrome
    /// artwork is scaled to fit. 108 fills the layer. Android recommends 48 to
    /// 66 for a logo; 66 is the safe zone that no launcher mask hides.
    #[arg(long, default_value_t = vdtoolkit::ADAPTIVE_ICON_SIZE)]
    fit: f32,
    /// Also write a legacy icon for devices below API 26: the background and
    /// foreground composed under a circular mask, as `mipmap/<name>.xml`.
    #[arg(long)]
    legacy: bool,
    /// Android `res/` directory to write into.
    #[arg(short, long, value_name = "RES_DIR")]
    output: PathBuf,
    /// Reject SVGs that require safe normalization.
    #[arg(long)]
    strict: bool,
}

#[derive(Args)]
struct ReportArgs {
    /// SVG file or directory to inspect.
    input: PathBuf,
    /// Report format.
    #[arg(long, value_enum, default_value_t = Format::Human)]
    format: Format,
    /// Treat safe normalization as incompatible.
    #[arg(long)]
    strict: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Json,
}

#[derive(Serialize)]
struct FileAnalysis<'a> {
    path: String,
    #[serde(flatten)]
    analysis: &'a Analysis,
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
                "convert" | "check" | "inspect" | "optimize" | "adaptive"
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
    }
}

fn adaptive(args: AdaptiveArgs) -> Result<Outcome> {
    validate_resource_name(&args.name)?;
    if !(args.fit > 0.0 && args.fit <= vdtoolkit::ADAPTIVE_ICON_SIZE) {
        return Err(Error::InvalidInput(format!(
            "--fit must be between 0 and {} dp, got {}",
            vdtoolkit::ADAPTIVE_ICON_SIZE,
            args.fit
        )));
    }
    let color = args
        .background_color
        .as_deref()
        .map(normalize_color)
        .transpose()?;
    let drawable_dir = args.output.join("drawable");
    let mipmap_dir = args.output.join("mipmap-anydpi-v26");

    let foreground_name = format!("{}_foreground", args.name);
    let background_name = format!("{}_background", args.name);
    let monochrome_name = format!("{}_monochrome", args.name);

    // Every layer is converted before anything is written, so a failing
    // layer leaves the resource directory untouched.
    let Some(foreground) = adaptive_layer(&args.foreground, args.fit, args.strict, true) else {
        return Ok(Outcome::Failed);
    };
    let mut files = vec![(
        drawable_dir.join(&foreground_name).with_extension("xml"),
        foreground.to_xml(),
    )];
    let (background, background_reference) = match (&args.background, color) {
        (Some(path), _) => {
            let Some(layer) =
                adaptive_layer(path, vdtoolkit::ADAPTIVE_ICON_SIZE, args.strict, false)
            else {
                return Ok(Outcome::Failed);
            };
            files.push((
                drawable_dir.join(&background_name).with_extension("xml"),
                layer.to_xml(),
            ));
            (layer, format!("@drawable/{background_name}"))
        }
        (None, Some(color)) => {
            files.push((
                args.output
                    .join("values")
                    .join(&background_name)
                    .with_extension("xml"),
                vdtoolkit::color_resource_xml(&background_name, &color),
            ));
            (
                vdtoolkit::Asset::solid_adaptive_layer(&color)?,
                format!("@color/{background_name}"),
            )
        }
        (None, None) => unreachable!("clap requires a background layer"),
    };
    let monochrome_reference = match &args.monochrome {
        Some(monochrome) => {
            let Some(layer) = adaptive_layer(monochrome, args.fit, args.strict, true) else {
                return Ok(Outcome::Failed);
            };
            files.push((
                drawable_dir.join(&monochrome_name).with_extension("xml"),
                layer.to_xml(),
            ));
            Some(format!("@drawable/{monochrome_name}"))
        }
        None => None,
    };
    let icon = vdtoolkit::adaptive_icon_xml(
        &background_reference,
        &format!("@drawable/{foreground_name}"),
        monochrome_reference.as_deref(),
    );
    let round_name = format!("{}_round", args.name);
    files.push((
        mipmap_dir.join(&args.name).with_extension("xml"),
        icon.clone(),
    ));
    files.push((mipmap_dir.join(&round_name).with_extension("xml"), icon));
    if args.legacy {
        let legacy = vdtoolkit::Asset::legacy_launcher_icon(&background, &foreground);
        if let Some(api) = legacy.analysis.minimum_api.filter(|api| *api > 21) {
            eprintln!(
                "warning: legacy icon needs API {api} because a layer uses clips or gradients \
                 and cannot be inflated on earlier devices"
            );
        }
        let legacy_dir = args.output.join("mipmap");
        let xml = legacy.to_xml();
        files.push((
            legacy_dir.join(&args.name).with_extension("xml"),
            xml.clone(),
        ));
        files.push((legacy_dir.join(&round_name).with_extension("xml"), xml));
    }

    for (path, contents) in &files {
        write_file(path, contents)?;
        println!("{}", path.display());
    }
    Ok(Outcome::Passed)
}

/// Convert and fit one layer, or report the failure with its path and return
/// `None`. Foreground and monochrome layers warn when content leaves the safe
/// zone; a background is meant to fill the layer.
fn adaptive_layer(input: &Path, fit: f32, strict: bool, check_safe_zone: bool) -> Option<Asset> {
    let layer = || -> Result<Asset> {
        let mut asset = vdtoolkit::convert_file(input)?;
        if strict && asset.analysis.compatibility != Compatibility::Exact {
            return Err(Error::InvalidInput(
                "requires normalization and was rejected by --strict".to_owned(),
            ));
        }
        asset.fit_adaptive_layer(fit)?;
        Ok(asset)
    };
    match layer() {
        Ok(asset) => {
            if check_safe_zone {
                if let Some(bounds) = asset.outside_adaptive_safe_zone() {
                    eprintln!(
                        "warning: {}: content spans {}..{} × {}..{}dp, outside the {}dp safe zone; \
                         launcher masks may hide it (see --fit)",
                        input.display(),
                        vdtoolkit_number(bounds.left),
                        vdtoolkit_number(bounds.right),
                        vdtoolkit_number(bounds.top),
                        vdtoolkit_number(bounds.bottom),
                        vdtoolkit::ADAPTIVE_ICON_SAFE_ZONE
                    );
                }
            }
            Some(asset)
        }
        Err(error) => {
            print_error(Some(input), &error);
            None
        }
    }
}

/// Short decimal for dp values in warnings.
fn vdtoolkit_number(value: f32) -> String {
    let text = format!("{value:.1}");
    text.strip_suffix(".0").map(str::to_owned).unwrap_or(text)
}

fn validate_resource_name(name: &str) -> Result<()> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first.is_ascii_lowercase() || first == '_')
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidInput(format!(
            "resource name must match [a-z_][a-z0-9_]*, got {name:?}"
        )))
    }
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

fn write_file(path: &Path, contents: &str) -> Result<()> {
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
    let mut failures = 0;
    for input in &inputs {
        if let Err(error) = convert_one(&args, input, optimize) {
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

fn convert_one(args: &ConvertArgs, input: &Path, optimize: bool) -> Result<()> {
    let mut asset = vdtoolkit::convert_file(input)?;
    if args.strict && asset.analysis.compatibility != Compatibility::Exact {
        return Err(Error::InvalidInput(
            "requires normalization and was rejected by --strict".to_owned(),
        ));
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
    let generated_bytes = asset.to_xml().len();
    if optimize {
        asset.optimize();
    }
    let xml = asset.to_xml();
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
    let output = output_path(&args.input, input, args.output.as_deref());
    if let Some(output) = output {
        write_file(&output, &xml)?;
    } else {
        print!("{xml}");
    }
    Ok(())
}

fn report(args: ReportArgs, inspect: bool) -> Result<Outcome> {
    let inputs = inputs(&args.input)?;
    let mut reports = Vec::new();
    let mut passed = true;
    let mut failed = 0;
    for input in &inputs {
        let result = vdtoolkit::analyze_file(input);
        match &result {
            Ok(analysis) => {
                passed &= if args.strict {
                    analysis.compatibility == Compatibility::Exact
                } else {
                    analysis.compatibility.convertible()
                };
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
                    Ok(analysis) => ReportEntry::Analysis(FileAnalysis {
                        path: path.display().to_string(),
                        analysis,
                    }),
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
                    Ok(analysis) => print_human(analysis, inspect),
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

fn print_human(analysis: &Analysis, inspect: bool) {
    let compatible = analysis.compatibility.convertible();
    println!(
        "{} {}",
        if compatible { "✓" } else { "✗" },
        if compatible {
            "VectorDrawable compatible"
        } else {
            "Not exactly representable as VectorDrawable"
        }
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
    }
    if inspect {
        let metrics = &analysis.metrics;
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
        match metrics.content_bounds {
            Some(bounds) => println!(
                "Content bounds: left {}, top {}, right {}, bottom {}",
                bounds.left, bounds.top, bounds.right, bounds.bottom
            ),
            None => println!("Content bounds: none"),
        }
        println!("Estimated XML size: {} bytes", metrics.estimated_xml_bytes);
    }
    println!("Compatibility: {:?}", analysis.compatibility);
    match analysis.minimum_api {
        Some(api) => println!("Minimum API for exact rendering: {api}"),
        None => println!("Minimum API for exact rendering: n/a"),
    }
}

fn print_summary(reports: &[(&PathBuf, Result<Analysis>)]) {
    let count = |compatibility: Compatibility| {
        reports
            .iter()
            .filter(|(_, result)| {
                result
                    .as_ref()
                    .is_ok_and(|analysis| analysis.compatibility == compatibility)
            })
            .count()
    };
    let failed = reports.iter().filter(|(_, result)| result.is_err()).count();
    println!("\n{} SVGs checked", reports.len());
    println!("{} exact", count(Compatibility::Exact));
    println!(
        "{} exact with normalization",
        count(Compatibility::ExactWithNormalization)
    );
    println!("{} approximate", count(Compatibility::Approximate));
    println!("{} unsupported", count(Compatibility::Unsupported));
    if failed > 0 {
        println!("{failed} could not be analyzed");
    }
}

fn inputs(path: &Path) -> Result<Vec<PathBuf>> {
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
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("svg"))
        })
        .collect();
    files.sort();
    if files.is_empty() {
        return Err(Error::InvalidInput(format!(
            "no SVG files found in {}",
            path.display()
        )));
    }
    Ok(files)
}

fn output_path(root: &Path, input: &Path, output: Option<&Path>) -> Option<PathBuf> {
    let output = output?;
    if root.is_file() {
        return Some(output.to_owned());
    }
    let relative = input.strip_prefix(root).unwrap_or(input);
    Some(output.join(relative).with_extension("xml"))
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
