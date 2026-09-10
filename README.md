# svg2vd

`svg2vd` is a standalone native Rust toolchain for checking, inspecting,
converting, and optimizing SVG assets for Android VectorDrawable. It does not
require Android Studio, the Android SDK, Gradle, Java, the JVM, or AOSP.

## Install

Download a native archive and its SHA-256 checksum from
[GitHub Releases](https://github.com/c5inco/svg2vd/releases), or install from
crates.io:

```sh
cargo install svg2vd --locked
```

To build the current checkout instead:

```sh
cargo install --path . --locked
```

`svg2vd` supports Linux x86-64, macOS Intel and Apple Silicon, and Windows
x86-64 release artifacts. Building from source requires Rust 1.85 or newer.

## Quick start

```sh
svg2vd convert icon.svg -o ic_icon.xml
svg2vd check icon.svg
svg2vd inspect icon.svg --format json
svg2vd optimize icon.svg -o ic_icon.xml
svg2vd icon.svg -o ic_icon.xml
```

Directories are accepted by every command. Directory conversion requires `-o`
and preserves the input tree below that output directory. The first positional
form is shorthand for `convert`.

| Command | Purpose |
| --- | --- |
| `convert` | Produce deterministic VectorDrawable XML. |
| `check` | Test compatibility without writing XML. |
| `inspect` | Report compatibility, diagnostics, minimum Android API, and metrics. |
| `optimize` | Convert and safely round numeric precision. |

`check` and `inspect` exit 0 when every input is convertible, 2 when any input
is incompatible, and 1 for malformed input or I/O errors. `--strict` accepts
only inputs requiring no normalization. Conversion errors exit 1. JSON reports
are intended for automation and include stable diagnostic codes.

## Rust API

Rust programs can embed the same analyzer and converter without launching a
subprocess:

```rust
let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
    <path d="M2 2H22V22H2Z" fill="#123456"/>
</svg>"##;
let asset = svg2vd::convert(source)?;
assert_eq!(asset.analysis.minimum_api, Some(21));
let xml = asset.to_xml();
# Ok::<(), svg2vd::Error>(())
```

The CLI is the stable V1 interface. The deliberately small Rust API exposes
analysis, conversion, XML serialization, and safe optimization, but remains
experimental and may change between `0.x` releases.

## Compatibility profile

The current profile converts dimensions, `viewBox`, paths, primitive shapes,
`<use>`, inherited styles, nested affine transforms, solid fills, fill opacity,
fill rules, solid strokes, stroke opacity/width/caps/joins, conservative
single-path clips, hard white single-shape masks, and deterministic XML. Masks
in that restricted subset are lowered to scoped clip intersections. Geometry
transforms are flattened. A stroke may only be flattened through a similarity
transform (translation, rotation, reflection, and uniform scale).

The source preflight rejects DTDs, entity declarations, filters, text, images,
patterns, animation, external references, unsupported compositing, gradients,
non-uniformly transformed strokes, and clip or mask semantics VectorDrawable
cannot represent exactly. A clip definition must resolve to one nonzero path.
A mask must resolve to one fully opaque white nonzero shape. Multi-path unions,
even-odd rules, alpha or grayscale masks, nested effects, and paint effects
inside definitions are rejected rather than silently changed.

`Compatibility` has four stable states: `exact`,
`exact_with_normalization`, `approximate`, and `unsupported`. Conversion only
accepts the first two. `--strict` accepts only `exact`.

Plain paths and a single clip path target platform API 21. Drawables that need
multiple clips for intersection or scope target API 24 because Android API 21
does not reliably restore/intersect the clip canvas. Ordinary paths using
`android:fillType` also require API 24. The analysis report derives the minimum
API from the emitted drawable instead of assigning one level to every
conversion.

## Security and determinism

SVG is treated as untrusted input. External entity declarations are rejected,
and `usvg` is configured with data and external image resolution disabled. No
network or filesystem resource is loaded from an SVG. Identical bytes and
options produce identical XML.

## Upstream research baseline

The initial behavioral research baseline is AOSP `platform/tools/base`, Android
Studio mirror commit
[`cab65fa60a996079b6f6d9b62025df3d98c135bc`](https://android.googlesource.com/platform/tools/base/+/cab65fa60a996079b6f6d9b62025df3d98c135bc/),
specifically `Svg2Vector.java` and `VectorDrawableGeneratorTest.java`. AOSP is a
reference, not a source or dependency. General SVG normalization uses `usvg`
0.48.1. No AOSP implementation code is copied.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 tools/check-material-symbols.py
python3 tools/check-material-symbols.py --suite twotone-100
python3 tools/check-studio-icons.py
```

The core test suite is Rust-only. The optional corpus commands download and
semantically compare pinned, fixed-seed 100-icon outlined and Two Tone corpora
with Google's official Android drawables; those Apache-2.0 assets are not
vendored. Two Tone comparison includes per-path fill alpha. Use `--suite
outlined-all` for the exhaustive pinned outlined corpus. Run `bash
tools/accept-v1.sh` for the complete V1 acceptance and local release packaging
gate. The exhaustive corpus runs manually and before release, not on a timer,
because its pinned contents are immutable. Android renderer/AOSP conformance
remains optional and must not become a normal build dependency.

The Studio Icons command is a separate, observational SVG stress suite. It
checks a content-hashed public snapshot for compatibility, complete conversion,
and deterministic output, but is not a release gate: the site provides no
immutable asset archive, explicit asset license, or official VectorDrawables.
No Studio Icons metadata or artwork is redistributed by this repository. The
current snapshot converts 802 of 812 icons; all 802 outputs are byte-identical
on repeat.

Android renderer conformance is available as an optional, standalone harness in
[`tools/android-renderer`](https://github.com/c5inco/svg2vd/tree/main/tools/android-renderer).
It converts its SVG fixtures with the current binary and verifies actual
VectorDrawable pixels on API 21, API 24, and a modern Android API. It is not
part of Cargo, CI, packaging, or the V1 acceptance gate.

See the [changelog](CHANGELOG.md) for release notes. Maintainers can follow the
[release checklist](https://github.com/c5inco/svg2vd/blob/main/docs/releasing.md).

Licensed under `MIT OR Apache-2.0`.
