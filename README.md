# vdtoolkit

Turn SVGs into Android resources without Android Studio, the Android SDK,
Gradle, or a JVM. `vdt` is a single native command that converts SVGs to
VectorDrawable XML, generates adaptive launcher icons and notification icons,
shrinks drawables without changing how they render, and checks whether an SVG
will convert before anything is written. The same converter also runs as
WebAssembly in a development Figma plugin.

Output is deterministic, so the same SVG gives the same XML on every machine,
and anything VectorDrawable cannot render exactly is rejected with a diagnostic
instead of silently approximated. Compared with the Android Studio importer, it
runs anywhere, scripts cleanly into CI, and fails loudly instead of emitting a
drawable that renders differently from the source.

## Install

> **Note:** `vdtoolkit` is not published to crates.io yet, so
> `cargo install vdtoolkit` will not work. Use a prebuilt binary or install
> from this repository.

Prebuilt binaries for macOS, Linux, and Windows are on the
[Releases page](https://github.com/c5inco/vdtoolkit/releases).

To install with Cargo from GitHub, you need Rust 1.85 or newer:

```sh
cargo install --git https://github.com/c5inco/vdtoolkit --locked
```

The package is `vdtoolkit` and the installed command is `vdt`. Version 0.1.0
was released as `svg2vd`; later releases use the new names.

To build from a clone of this repository instead:

```sh
cargo install --path . --locked
```

## Usage

```sh
vdt icon.svg -o ic_icon.xml            # shorthand for `convert`
vdt convert icons/ -o res/drawable-anydpi/ # directories work; tree is preserved
vdt check icon.svg                     # is it convertible? (no output written)
vdt inspect icon.svg --format json     # diagnostics, min API, metrics
vdt inspect bell.svg --as notification --format json  # would it make a good notification icon?
vdt optimize icon.svg -o ic_icon.xml   # convert with shorter numbers and path data, same rendering
vdt adaptive --foreground fg.svg --background-color '#3DDC84' -o app/src/main/res
vdt adaptive --foreground fg.svg --background-image hero.webp -o app/src/main/res
vdt notification bell.svg -o res/drawable-anydpi/ic_stat_bell.xml
vdt convert icon.svg --pretty          # human-readable XML instead of the default compact format
```

| Command | What it does |
| --- | --- |
| `convert` | Write VectorDrawable XML. Directory input requires `-o`. |
| `check` | Report compatibility only. |
| `inspect` | Report compatibility, diagnostics, minimum Android API, content bounds, and metrics. |
| `optimize` | Convert, then shorten numbers and path data where it cannot change rendering. |
| `adaptive` | Generate an adaptive launcher icon and its layer drawables into a `res/` directory. |
| `notification` | Convert to a white 24dp notification icon drawable. |

Every command except `adaptive` accepts a file or a directory. In a directory, a file that fails
is named on stderr and the remaining files are still processed. `--format json`
produces a stable report with diagnostic codes for use in CI, and lists a
`path` and `error` for any file that could not be read or parsed. `--strict`
accepts only input that needs no normalization at all; `--allow-approximate`
also accepts constructs VectorDrawable cannot draw exactly, such as a radial
gradient's focal point, and lowers them with a warning.

Written XML is compact by default to reduce resource size without changing
geometry or precision. Pass `--pretty` to `convert`, `optimize`, `notification`,
or `adaptive` when reviewing or hand-editing the generated XML.

The examples put vectors in `drawable-anydpi/`, which makes them win resource
selection over same-named density-specific bitmaps. The `anydpi` qualifier and
framework VectorDrawable both require API 21. Use plain `drawable/` instead
when AndroidX VectorDrawableCompat must load the resource below API 21.

Written files get names Android accepts as resources: lowercase letters,
digits, and underscores, not starting with a digit and not a Java keyword. A
valid name is kept. Any other name is rewritten and the new name is printed:
`Arrow-Left.svg` becomes `arrow_left.xml`, `HTTPServer.svg` becomes
`http_server.xml`, and `2x.svg` and `switch.svg` become `ic_2x.xml` and
`ic_switch.xml`. Accents are dropped (`Café.svg` becomes `cafe.xml`), and `+`,
`#`, `&`, `@`, and `%` become words, so `C++.svg` and `C#.svg` become
`c_plus_plus.xml` and `c_sharp.xml` instead of both becoming `c.xml`. Other
punctuation separates words or is dropped. This covers `convert`, `optimize`, and `notification`,
including a name given with `-o`. When two inputs in a directory would get the
same name, the later one fails instead of overwriting the first.

`convert` and `optimize` keep the SVG's `width` and `height` as the drawable's
size. `--size <dp>` sets the longer side instead, and the other side keeps the
aspect ratio; the viewport is unchanged, so the drawing is too. An SVG with only
a `viewBox` has no size of its own, so its viewBox units become dp, except that
a viewBox larger than 200 is drawn at 24dp: a Material Symbols download on its
960-unit grid becomes a 24dp icon, not a 960dp one.

```sh
vdt convert hero.svg --size 24 -o res/drawable-anydpi/ic_hero.xml
```

A launcher mask is a circle inscribed in the 72dp visible window, so what
decides whether artwork survives is its distance from the centre, not its
bounding box. The corners of the 66dp box sit 46.7dp out while the mask shows
36dp, so a square logo filling that box is clipped and a round one filling the
same box is not. vdt renders the layer and measures the painted pixels, which
is why `--fit 66` is safe for some artwork and not for other artwork of the
same size.

`check` and `inspect` take `--as <kind>` to also report what making the file
into that kind of icon would find, exactly as `notification` or `adaptive`
would, without writing anything. The kinds are `notification`,
`adaptive-foreground` (a monochrome layer follows the same rules), and
`adaptive-background`; `--fit <dp>` is the generator's fit and defaults to the
whole canvas, and `--background-fit` mirrors the generator's option for
`--as adaptive-background`. A PNG, WebP, or JPEG is inspected as an adaptive
layer image, the way `--foreground-image` and `--background-image` are, with
`--fit` unused because a raster layer is placed, never resampled.
A raster foreground can raise `SVGVD018`, `SVGVD019`, `SVGVD024`, `SVGVD025`,
and `SVGVD026`; a raster background can raise `SVGVD020`, `SVGVD024`, and
`SVGVD025`. The report's metrics describe the fitted vector or the image on
the 108dp layer. In JSON, a vector entry has an `icon` object naming its kind
and fit. A raster entry's `icon` keeps the kind but omits the unused `fit` and
`fit_mode`, and the entry carries `image`: `png`, `webp`, or `jpg`. Its metrics
contain pixel `width` and `height`, plus `content_bounds` and `viewport_*` in dp
on the 108dp layer; vector-only counts and estimated XML size are omitted. Its
`compatibility` is `not_applicable` because a raster layer is not a
VectorDrawable. A raster *file* without
`--as`, or with `--as notification`, is refused; a directory with
`--as notification` still keeps only SVGs, and a directory with
`--as adaptive-foreground` skips JPEGs the same way. These findings are
warnings and notes.

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

Android draws a status bar and notification icon from its alpha channel alone
and tints it with the system color, so `notification` flattens every fill and
stroke to white, keeps opacity, and writes the artwork onto a 24dp canvas:

```sh
vdt notification bell.svg -o res/drawable-anydpi/ic_stat_bell.xml
vdt notification icons/ --fit 20 -o res/drawable-anydpi/
```

| Option | Meaning |
| --- | --- |
| `--fit <dp>` | Square that the artwork is scaled to fit, centered on the 24dp canvas. Defaults to 24, which keeps artwork that already carries its own padding, such as a Material system icon, at its drawn size. Use 20 for artwork drawn edge to edge, which leaves the 2dp of padding a system icon has. |
| `--optimize` | Shorten numbers and path data where it cannot change rendering, as the `optimize` command does. It runs after the fit, so placement is unchanged. |
| `--strict` | Reject input that needs safe normalization, as elsewhere. |

A gradient whose stops all share one opacity is only color, so it becomes solid
white and the drawable no longer needs API 24; a gradient that fades is the
shape of the icon, so it is kept with white stops. The findings carry
diagnostic codes, so `inspect --as notification --format json` can report them
before anything is written:

| Code | Meaning |
| --- | --- |
| `SVGVD021` | Note naming the colors and gradients that were flattened to white. |
| `SVGVD017` | Warning: the artwork paints almost the whole canvas, so a solid plate tints into a filled square rather than a silhouette. |
| `SVGVD018` | Warning: the artwork has no painted content, so the icon is invisible. |

Reference the result from the notification with
`setSmallIcon(R.drawable.ic_stat_bell)`.

### Adaptive icons

`adaptive` converts each layer SVG into a 108dp VectorDrawable and writes the
`<adaptive-icon>` resource that ties them together:

```sh
vdt adaptive --foreground logo.svg --background bg.svg --monochrome logo.svg \
    --fit 66 --name ic_launcher -o app/src/main/res
```

| Option | Meaning |
| --- | --- |
| `--foreground <svg>` or `--foreground-image <png\|webp>` | Foreground layer, as an SVG or a raster image. Exactly one is required. |
| `--foreground-image <image>`, `--monochrome-image <image>` | Use a PNG or WebP as the layer. It is copied unchanged into `mipmap-nodpi/`, and must already be drawn on the full square layer with its artwork inside the 66dp safe zone: `--fit` places vector artwork, and vdt cannot scale a raster layer without resampling it. A JPEG is rejected here, because a layer with no alpha channel would hide the background. `--legacy` is rejected with `--foreground-image`. |
| `--background <svg>`, `--background-color <#RRGGBB>`, or `--background-image <image>` | Background layer, as a converted drawable, a color resource, or a raster image. Exactly one is required. |
| `--background-image <image>` | Use a PNG, WebP, or JPEG from anywhere on disk as the background. It is copied unchanged into `mipmap-nodpi/<name>_background.<ext>`, where Android draws it onto the layer without scaling it for density, and the icon points at it. The format is read from the file's own header, not its name, so a mislabeled file is still written under the extension it really is. vdt never resamples the image; it decodes it only to report `SVGVD024`, `SVGVD025`, and `SVGVD020`. A JPEG has no alpha channel, so it can never raise `SVGVD020`. An animated WebP and `--legacy`, which composes both layers into one vector, are both rejected alongside it. |
| `--background-fit cover\|contain` | How a background SVG is scaled onto the 108dp layer. `cover`, the default, fills the layer and lets it crop whatever overflows, so non-square artwork still reaches every edge. `contain` scales all of the artwork in, which leaves non-square artwork letterboxed. |
| `--monochrome <svg>` | Optional monochrome layer for themed icons on Android 13 and newer. |
| `--fit <dp>` | Square that the foreground and monochrome *vector* artwork is scaled to fit, centered on the 108dp layer. Defaults to 108 for artwork drawn on the full layer. A launcher mask is a circle, so the fit a logo needs depends on its shape: a round mark can fill 66, while one drawn edge to edge into the corners of the box needs about 47. `SVGVD019` measures the artwork and names the fit that works for it. |
| `--name <name>` | Resource name, `ic_launcher` by default. Layers use it as a prefix. |
| `--legacy` | Also write a legacy icon for devices below API 26, with the 72dp visible area on the 44dp circle keyline of a 48dp icon. It is a vector in `mipmap/`. When the art needs API 24, because of gradients, even-odd fills, or clips, it is a vector in `mipmap-anydpi-v24/` for API 24 and 25 plus PNGs rendered from that vector in `mipmap-mdpi/` through `mipmap-xxxhdpi/` for API 21 to 23. Every other version of the legacy icon in a mipmap folder is removed, in any format, including the `ic_launcher.webp` files Android Studio writes into every density folder, so regenerating a name switches layouts cleanly and replaces Studio's icon. |
| `--optimize` | Shorten numbers and path data in every layer drawable and the legacy vector where it cannot change rendering, as the `optimize` command does. It runs after the fit, which is what introduces long decimals, so placement is unchanged. Legacy PNGs are rendered from the exact vector either way. |

The files written are `mipmap-anydpi-v26/<name>.xml`,
`mipmap-anydpi-v26/<name>_round.xml`, `drawable-anydpi/<name>_foreground.xml`, either
`drawable-anydpi/<name>_background.xml`, `values/<name>_background.xml`, or
`mipmap-nodpi/<name>_background.png`, `.webp`, or `.jpg`, and
`drawable-anydpi/<name>_monochrome.xml` when a monochrome layer is given. Artwork is
scaled uniformly and centered, so rendering is unchanged apart from placement.
A background SVG is scaled to cover the whole layer, because Android crops that
layer with launcher masks and shifts it for parallax, so anything it does not
reach shows through; the layer crops what overflows, and `--background-fit
contain` scales all of the artwork in instead. Placement findings carry
diagnostic codes, so `inspect --as adaptive-foreground --fit 66` or
`--as adaptive-background` can report them before anything is written,
including for a PNG or WebP passed the same way as `--foreground-image`:

| Code | Meaning |
| --- | --- |
| `SVGVD019` | How far the foreground or monochrome artwork reaches from the centre of the layer, measured from the painted pixels. A warning past 36dp, which is what a circular launcher mask shows, so the artwork is clipped; a note past 33dp, the radius Android asks key content to stay inside, which `adaptive` prints as well as `inspect`. The remedy names the `--fit` that would bring it in. |
| `SVGVD020` | Warning: the background leaves part of the 108dp layer unpainted, which shows through launcher masks and parallax. Inset clips, holes, and transparent paint all leave gaps, as does non-square artwork under `--background-fit contain`. |
| `SVGVD023` | Note: the background was scaled to cover the layer, naming what percentage of the artwork the layer cropped. Square artwork is unchanged and gets no note. |
| `SVGVD024` | Warning: a layer image is not square, so Android stretches it onto the square layer and the artwork is distorted. |
| `SVGVD026` | Warning: a `--foreground-image` has no transparent pixels, so it covers the background layer and the icon is flat. |
| `SVGVD025` | A layer image does not carry the 432px an xxxhdpi device draws the layer at: a warning when it is smaller, so the icon is upscaled and soft, and a note when it is larger, so the extra pixels are decoded and scaled away on every draw. |

Every layer is converted before anything is written, and a layer that fails
leaves the directory untouched, and every raster layer is decoded in full, so
an image with damaged data is refused before it can replace a working one.
Regenerating a name removes a file an earlier run left behind only when keeping
it would break the build: a raster layer that changes format, say from `.png`
to `.webp`, would otherwise leave two files with one resource name in one
folder, and a raster layer's versions in density folders such as
`mipmap-xxhdpi/`, which Android Studio writes, would be picked over
`mipmap-nodpi/` and keep the old icon showing. Any other leftover, such as a
vector layer replaced by an image, is a different resource type that collides
with nothing, and vdt cannot tell whether the rest of the project still uses
it, so it is named in a note and left alone.
Add `android:icon="@mipmap/<name>"` and `android:roundIcon="@mipmap/<name>_round"`
to the `<application>` element of the manifest.

## Example

```xml
<svg xmlns="http://www.w3.org/2000/svg" width="24" height="24" viewBox="0 0 24 24">
  <path d="M12 2L2 22h20z" fill="#3DDC84"/>
</svg>
```

becomes, with `--pretty` (by default the same XML is written on one line)

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
| Wide-gamut `color()` values, resolved to the sRGB they render as | Color spaces that are not Display P3, sRGB, or linear sRGB |
| Linear gradients and circular, elliptical, and rotated radial gradients, including stop opacity and spread methods | Radial gradients with a focal radius; strokes painted with an elliptical gradient |
| Radial gradients with an off-center focal point, centered under `--allow-approximate` | |
| Solid strokes: opacity, width, caps, joins | Nested effects or paint effects inside `<clipPath>`/`<mask>` |
| Single-path clips and hard white single-shape masks (lowered to clips) | |

`inspect` reports one of four compatibility states: `exact`,
`exact_with_normalization`, `approximate`, or `unsupported`. Conversion accepts
the first two, `--allow-approximate` also accepts `approximate`, and `--strict`
accepts only `exact`.

The reported minimum API comes from the emitted drawable, not a fixed constant.
Plain paths and a single clip target API 21. Drawables that need multiple clips,
`android:fillType`, or gradients target API 24, and `inspect` then adds an
`SVGVD004` note naming which of those raised it, with how to stay on API 21.
The note is informational: it does not change compatibility, the exit code, or
what `convert` prints. `inspect` adds an `SVGVD022` note in the same way when a
design tool wrote a wide-gamut `color()` value, naming how many were resolved to
sRGB; VectorDrawable has no wide-gamut color, so the resolved value is what
Android draws either way. Gradients are written inline
with the `aapt` namespace, which the Android build tools compile into color
resources. See [docs/research.md](docs/research.md)
for the renderer findings behind those levels.

## Using with AI agents

[`skills/vdt/SKILL.md`](skills/vdt/SKILL.md) is a skill for coding agents such
as Claude Code and Codex. With it installed, an agent asked for an app icon,
notification icon, or drawable reaches for `vdt` instead of writing
VectorDrawable XML by hand, inspects the SVG before generating anything, picks
options such as `--fit` for the kind of icon, and knows how to act on what the
diagnostics report. Install `vdt` itself first.

## WebAssembly and Figma

Browser-oriented `wasm-bindgen` bindings live in `bindings/wasm`. A development
Figma plugin uses them to convert frames, components, and instances, without
network access. In Dev Mode it shows the drawable for a single selection in
Figma's Code panel. In Design Mode it lists every selected layer, shows which
are ready and what to fix in the rest, and exports the chosen layers as `.xml`
files. A second Design Mode command exports the same selection as white 24dp
notification icons with notification-specific warnings. See the
[development plugin guide](docs/figma-codegen.md) for build, import, test, and
plugin-ID setup instructions.

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

When a command or flag changes, update the agent skill in
[skills/vdt/SKILL.md](skills/vdt/SKILL.md); `cargo test` fails while the skill
names anything `vdt` does not have.

Release notes are in the [changelog](CHANGELOG.md); maintainers follow the
[release checklist](docs/releasing.md).

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at
your option.
