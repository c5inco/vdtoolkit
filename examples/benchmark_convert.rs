//! Measure warm in-process SVG-to-VectorDrawable conversion.
//!
//! Use this with `tools/benchmark-svg2vectordrawable.mjs`, which measures the
//! competing Node API against the same corpus and reports comparable JSON.

use std::{
    env, fs,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

fn main() {
    let (directory, optimize) = arguments();
    let sources = read_sources(&directory);
    let mut samples = Vec::new();
    let mut output_bytes = 0;

    for _ in 0..11 {
        let started = Instant::now();
        output_bytes = sources
            .iter()
            .map(|source| {
                let mut asset = vdtoolkit::convert(source).expect("corpus SVG must convert");
                if optimize {
                    asset.optimize();
                }
                asset.to_compact_xml().len()
            })
            .sum();
        samples.push(started.elapsed());
    }

    // The first round warms allocator and code caches; it is deliberately not
    // part of the reported distribution.
    samples.remove(0);
    println!(
        "{{\"tool\":\"vdtoolkit\",\"policy\":\"{}\",\"assets\":{},\"output_bytes\":{},\"samples_ms\":[{}]}}",
        if optimize { "optimize" } else { "exact" },
        sources.len(),
        output_bytes,
        samples
            .iter()
            .map(|sample| format_duration(*sample))
            .collect::<Vec<_>>()
            .join(",")
    );
}

fn arguments() -> (PathBuf, bool) {
    let mut arguments = env::args_os().skip(1);
    let directory = arguments.next().map(PathBuf::from).expect(
        "usage: cargo run --example benchmark_convert --release -- <svg-directory> [--optimize]",
    );
    let optimize = arguments
        .next()
        .is_some_and(|argument| argument == "--optimize");
    assert!(arguments.next().is_none(), "unexpected argument");
    (directory, optimize)
}

fn read_sources(directory: &Path) -> Vec<Vec<u8>> {
    let mut paths = fs::read_dir(directory)
        .expect("read SVG directory")
        .map(|entry| entry.expect("read directory entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "svg"))
        .collect::<Vec<_>>();
    paths.sort();
    assert!(!paths.is_empty(), "SVG directory is empty");
    paths
        .iter()
        .map(|path| fs::read(path).expect("read SVG"))
        .collect()
}

fn format_duration(duration: Duration) -> String {
    format!("{:.3}", duration.as_secs_f64() * 1_000.0)
}
