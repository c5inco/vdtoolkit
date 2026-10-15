# vdtoolkit

Turn SVGs into Android resources without Android Studio, the Android SDK, Gradle, or a JVM. `vdt` is a native CLI that converts SVGs to VectorDrawable XML, generates adaptive launcher and notification icons, optimizes path and coordinate precision, and checks SVG compatibility. The same converter also runs as WebAssembly in a local Figma plugin.

Output is deterministic across machines, and anything VectorDrawable cannot render faithfully is rejected with a diagnostic code instead of silently approximated.

## Install

> **Note:** `vdtoolkit` is not published to crates.io yet. Use a prebuilt binary from [Releases](https://github.com/c5inco/vdtoolkit/releases) (macOS, Linux, Windows) or install from source with Rust 1.85+.

```sh
cargo install --git https://github.com/c5inco/vdtoolkit --locked
# or from a local clone:
cargo install --path . --locked
```

The package is `vdtoolkit` and the installed command is `vdt` (released as `svg2vd` in v0.1.0).

## Usage

```sh
vdt icon.svg -o ic_icon.xml                           # shorthand for `convert`
vdt convert icons/ -o res/drawable-anydpi/            # directories work; tree is preserved
vdt check icon.svg                                    # is it convertible? (no output written)
vdt inspect icon.svg --format json                    # diagnostics, min API, metrics
vdt inspect bell.svg --as notification --format json  # would it make a good notification icon?
vdt optimize icon.svg -o ic_icon.xml                  # shorter numbers and path data, same rendering
vdt adaptive --foreground fg.svg --background-color '#3DDC84' -o app/src/main/res
vdt adaptive --foreground fg.svg --background-image hero.webp -o app/src/main/res
vdt notification bell.svg -o res/drawable-anydpi/ic_stat_bell.xml
vdt convert icon.svg --pretty                         # indented XML instead of single-line compact
```

| Command | What it does |
| --- | --- |
| `convert` | Write VectorDrawable XML. Directory input requires `-o`. |
| `check` | Report compatibility only. |
| `inspect` | Report compatibility, diagnostics, minimum Android API, content bounds, and metrics. |
| `optimize` | Convert, then shorten numbers and path data where it cannot change rendering. |
| `adaptive` | Generate an adaptive launcher icon and its layer drawables into a `res/` directory. |
| `notification` | Convert to a white 24dp notification icon drawable. |

Every command except `adaptive` accepts a file or a directory (failed files in a directory are reported on stderr while remaining files continue).

- `--format json`: emit a stable report with diagnostic codes for CI.
- `--strict`: accept only inputs that need zero normalization (`exact`).
- `--allow-approximate`: accept constructs VectorDrawable cannot draw exactly (such as an off-center radial gradient focal point) with a warning.
- `--pretty`: format XML across lines instead of the default single-line output.

Output filenames are normalized to valid Android resource names (`lowercase_with_underscores`, prefixed with `ic_` if starting with a digit or matching a Java keyword, and symbols like `+` or `#` expanded to `plus`/`sharp`). Colliding output names in a directory fail rather than overwriting.

`convert` and `optimize` keep the SVG's `width` and `height` in `dp` (or use `viewBox` dimensions, defaulting to `24dp` when a keyless `viewBox` exceeds 200 units, as in Material Symbols). Pass `--size <dp>` to scale the longer side while preserving aspect ratio:

```sh
vdt convert hero.svg --size 24 -o res/drawable-anydpi/ic_hero.xml
```

Pass `--as <notification|adaptive-foreground|adaptive-background>` (plus `--fit` or `--background-fit`) to `check` or `inspect` to run icon-specific placement and compatibility checks without writing files. Adaptive layer inspection also accepts PNG, WebP, and (for backgrounds) JPEG files:

```sh
vdt inspect icons/ --as notification --format json
vdt check logo.svg --as adaptive-foreground --fit 66
vdt inspect fg.webp --as adaptive-foreground --format json
vdt inspect hero.svg --as adaptive-background --background-fit contain
```

| Exit code | Meaning |
| --- | --- |
| 0 | Every input converted, or is convertible. |
| 1 | At least one file was malformed, unreadable, or failed to convert. |
| 2 | `check`/`inspect` only: at least one input is incompatible. |

### Notification icons

Android draws status bar and notification icons from the alpha channel alone, so `notification` flattens fills and strokes to white, keeps opacity, and places artwork onto a 24dp canvas (uniform-opacity gradients flatten to solid white on API 21; alpha-fading gradients keep white stops).

```sh
vdt notification bell.svg -o res/drawable-anydpi/ic_stat_bell.xml
vdt notification icons/ --fit 20 -o res/drawable-anydpi/
```

| Option | Meaning |
| --- | --- |
| `--fit <dp>` | Centered square that artwork is scaled to fit on the 24dp canvas (default `24`; use `20` for edge-to-edge artwork to leave 2dp system-icon padding). |
| `--optimize` | Shorten numbers and path data after fitting. |
| `--strict` | Reject input that needs normalization. |

| Code | Meaning |
| --- | --- |
| `VDT021` | Note naming colors and gradients flattened to white. |
| `VDT017` | Warning: artwork paints almost the entire canvas and will tint into a solid square rather than a silhouette. |
| `VDT018` | Warning: artwork has no painted content. |

Reference the drawable with `setSmallIcon(R.drawable.ic_stat_bell)`.

### Adaptive icons

`adaptive` converts layer SVGs into 108dp VectorDrawables (or copies raster layers into `mipmap-nodpi/`) and writes `mipmap-anydpi-v26/<name>.xml` and `<name>_round.xml`. Reference them in `AndroidManifest.xml` with `android:icon="@mipmap/<name>"` and `android:roundIcon="@mipmap/<name>_round"`.

```sh
vdt adaptive --foreground logo.svg --background bg.svg --monochrome logo.svg \
    --fit 66 --name ic_launcher -o app/src/main/res
```

| Option | Meaning |
| --- | --- |
| `--foreground <svg>` or `--foreground-image <png\|webp>` | Required foreground layer. Raster layers are copied unchanged into `mipmap-nodpi/` without resampling (JPEGs and `--legacy` are rejected). |
| `--background <svg>`, `--background-color <#RRGGBB>`, or `--background-image <png\|webp\|jpg>` | Required background layer (vector drawable, color resource, or raster image in `mipmap-nodpi/`). Animated WebPs and `--legacy` with raster backgrounds are rejected. |
| `--background-fit cover\|contain` | How a background SVG scales onto the 108dp layer (`cover` by default so non-square art fills every edge; `contain` letterboxes). |
| `--monochrome <svg>` or `--monochrome-image <png\|webp>` | Optional monochrome layer for Android 13+ themed icons. |
| `--fit <dp>` | Centered square that *vector* foreground and monochrome artwork is scaled to fit on the 108dp layer (default `108`). Circular launcher masks show a 36dp radius, so a round mark can fill `66` while a full-bleed square mark needs ~`47`. |
| `--name <name>` | Resource name (`ic_launcher` by default). |
| `--legacy` | Also write a pre-API 26 48dp icon (`mipmap/<name>.xml`, or `mipmap-anydpi-v24/` plus density WebPs for API 21–23 when the vector uses API 24 features), removing conflicting legacy mipmap files. |
| `--optimize` | Shorten numbers and path data in generated vectors after fitting. |

| Code | Meaning |
| --- | --- |
| `VDT019` | Foreground/monochrome painted pixels extend past the 33dp keyline radius (note) or 36dp circular mask radius (warning); names the `--fit` that brings it inside. |
| `VDT020` | Warning: background leaves part of the 108dp layer unpainted. |
| `VDT023` | Note: non-square background SVG was cropped by `--background-fit cover` (with crop percentage). |
| `VDT024` | Warning: raster layer image is not square and will be stretched by Android. |
| `VDT025` | Raster layer image differs from the 432px xxxhdpi target (warning when smaller, note when larger). |
| `VDT026` | Warning: `--foreground-image` has no transparent pixels and hides the background layer. |

Every layer is validated before any file is written, leaving the output directory untouched on error.

## Example

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
  <path d="M12 2L2 22h20z" fill="#3DDC84"/>
</svg>
```

becomes, with `--pretty` (by default the same XML is written on one line):

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
| Wide-gamut `color()` values, resolved to the sRGB they render as (`VDT022`) | Color spaces that are not Display P3, sRGB, or linear sRGB |
| Linear gradients and circular, elliptical, and rotated radial gradients, including stop opacity and spread methods | Radial gradients with a focal radius; strokes painted with an elliptical gradient |
| Radial gradients with an off-center focal point, centered under `--allow-approximate` | |
| Solid strokes: opacity, width, caps, joins | Nested effects or paint effects inside `<clipPath>`/`<mask>` |
| Single-path clips and hard white single-shape masks (lowered to clips) | |

`inspect` reports `exact`, `exact_with_normalization`, `approximate`, or `unsupported`. Plain paths and a single clip target API 21; multiple clips, `android:fillType`, or inline `aapt` gradients raise the minimum to API 24 (`VDT004`). See [docs/research.md](docs/research.md) for renderer details.

## Using with AI agents

[`skills/vdt/SKILL.md`](skills/vdt/SKILL.md) is an agent skill. With `vdt` and the skill installed, agents inspect SVGs, choose `--fit` from diagnostics, and generate drawables with `vdt` instead of writing VectorDrawable XML by hand.

## WebAssembly and Figma

`bindings/wasm` provides browser `wasm-bindgen` bindings used by the local Figma plugin in `figma-plugin/`. In Dev Mode it shows VectorDrawable XML for a selection; in Design Mode it batch-inspects and exports layers as drawables or 24dp notification icons. See [docs/figma-codegen.md](docs/figma-codegen.md).

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Behavior is checked against AOSP's `Svg2Vector` ([docs/research.md](docs/research.md)); see [docs/v1-acceptance.md](docs/v1-acceptance.md) for the Android renderer harness and [docs/releasing.md](docs/releasing.md) for the release checklist. Update [skills/vdt/SKILL.md](skills/vdt/SKILL.md) whenever CLI flags change (`cargo test` verifies skill coverage).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
