# svg2vd

Convert SVG icons to Android VectorDrawable XML without Android Studio, the
Android SDK, Gradle, or a JVM. `svg2vd` is a single native binary and a small
Rust library. Output is deterministic, and anything VectorDrawable cannot render
exactly is rejected with a diagnostic instead of silently approximated.

Compared with the Android Studio importer, it runs anywhere, scripts cleanly
into CI, produces identical output on every machine, and fails loudly instead
of emitting a drawable that renders differently from the source.

## Install

```sh
cargo install svg2vd --locked
```

Prebuilt binaries for macOS, Linux, and Windows are on the
[Releases page](https://github.com/c5inco/svg2vd/releases).

To build from a clone of this repository instead, you need Rust 1.85 or newer:

```sh
cargo install --path . --locked
```

## Usage

```sh
svg2vd icon.svg -o ic_icon.xml            # shorthand for `convert`
svg2vd convert icons/ -o res/drawable/    # directories work; tree is preserved
svg2vd check icon.svg                     # is it convertible? (no output written)
svg2vd inspect icon.svg --format json     # diagnostics, min API, metrics
svg2vd optimize icon.svg -o ic_icon.xml   # convert with smaller numbers, same rendering
```

| Command | What it does |
| --- | --- |
| `convert` | Write VectorDrawable XML. Directory input requires `-o`. |
| `check` | Report compatibility only. |
| `inspect` | Report compatibility, diagnostics, minimum Android API, content bounds, and metrics. |
| `optimize` | Convert, then shorten numbers where it cannot change rendering. |

Every command accepts a file or a directory. In a directory, a file that fails
is named on stderr and the remaining files are still processed. `--format json`
produces a stable report with diagnostic codes for use in CI, and lists a
`path` and `error` for any file that could not be read or parsed. `--strict`
accepts only input that needs no normalization at all.

| Exit code | Meaning |
| --- | --- |
| 0 | Every input converted, or is convertible. |
| 1 | At least one file was malformed, unreadable, or failed to convert. |
| 2 | `check`/`inspect` only: at least one input is incompatible. |

## Example

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
  <path d="M12 2L2 22h20z" fill="#3DDC84"/>
</svg>
```

becomes

```xml
<?xml version="1.0" encoding="utf-8"?>
<vector xmlns:android="http://schemas.android.com/apk/res/android"
    android:width="24dp"
    android:height="24dp"
    android:viewportWidth="24"
    android:viewportHeight="24">
    <path
        android:pathData="M12,2 L2,22 L22,22 Z"
        android:fillColor="#3DDC84"
        />
</vector>
```

## What converts

| Supported | Rejected |
| --- | --- |
| Paths and basic shapes | Patterns |
| `width`/`height` and `viewBox` | Text, images, filters, animation |
| `<use>`, `<defs>`, inherited CSS styles | DTDs, entity declarations, external references |
| Nested affine transforms (flattened into geometry) | Strokes under non-uniform scale or skew |
| Solid fills, fill opacity, fill rules | Alpha or grayscale masks, even-odd or multi-path clips |
| Linear gradients and circular radial gradients, including stop opacity and spread methods | Radial gradients with a focal point, a focal radius, or an elliptical shape |
| Solid strokes: opacity, width, caps, joins | Nested effects or paint effects inside `<clipPath>`/`<mask>` |
| Single-path clips and hard white single-shape masks (lowered to clips) | |

`inspect` reports one of four compatibility states: `exact`,
`exact_with_normalization`, `approximate`, or `unsupported`. Conversion accepts
the first two, and `--strict` accepts only `exact`.

The reported minimum API comes from the emitted drawable, not a fixed constant.
Plain paths and a single clip target API 21. Drawables that need multiple clips,
`android:fillType`, or gradients target API 24. Gradients are written inline
with the `aapt` namespace, which the Android build tools compile into color
resources. See [docs/research.md](docs/research.md)
for the renderer findings behind those levels.

## Rust API (experimental)

Rust programs can embed the same analyzer and converter without launching a
subprocess. API docs are on [docs.rs](https://docs.rs/svg2vd).

```rust
let source = br##"<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24">
    <path d="M2 2H22V22H2Z" fill="#123456"/>
</svg>"##;
let asset = svg2vd::convert(source)?;
assert_eq!(asset.analysis.minimum_api, Some(21));
let xml = asset.to_xml();
# Ok::<(), svg2vd::Error>(())
```

The CLI is the stable interface. The Rust API may change between `0.x`
releases.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Behavior is checked against AOSP's `Svg2Vector` as a reference, not a
dependency; see [docs/research.md](docs/research.md). Optional corpus
comparisons, the Android renderer harness, and the full release gate are
described in [docs/v1-acceptance.md](docs/v1-acceptance.md).

Release notes are in the [changelog](CHANGELOG.md); maintainers follow the
[release checklist](docs/releasing.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
