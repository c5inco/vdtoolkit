---
name: vdt
description: Convert SVG to Android VectorDrawable XML and generate launcher/app (adaptive, themed) and notification icons. Use when an Android project needs an icon or drawable from an SVG, or to check an SVG before converting or explain an SVGVD diagnostic code.
---

# vdt

`vdt` converts SVG to Android VectorDrawable XML and generates launcher and
notification icons from SVG. It needs no Android SDK, Gradle, or JVM. The same
input always gives the same output, and anything a VectorDrawable cannot draw
exactly is rejected with a diagnostic code instead of being approximated.

Prefer it over writing `<vector>` XML by hand: hand-written conversions of real
artwork are usually wrong in ways that only show on a device.

It only reads SVG. It does not take PNG, WebP, or JPEG, read existing
VectorDrawable XML, or make iOS or web assets.

## Before you start

1. Run `vdt --version`. If the command is missing, ask the user before
   installing it. Prebuilt binaries are on
   https://github.com/c5inco/vdtoolkit/releases, or with Rust 1.85+:
   `cargo install --git https://github.com/c5inco/vdtoolkit --locked`.
2. `vdt <command> --help` is the source of truth. If it disagrees with this
   file, follow `--help`.
3. Find the module's `res/` directory, usually `app/src/main/res` next to
   `AndroidManifest.xml`, and the module's `minSdk` in `build.gradle.kts` or
   `build.gradle`. Both change which options you need.

## Pick the command

| The user wants | Command |
| --- | --- |
| A drawable from an SVG (`R.drawable.x`, `painterResource`) | `vdt convert icon.svg -o res/drawable-anydpi/ic_icon.xml` |
| The same, as small as possible | `vdt optimize icon.svg -o res/drawable-anydpi/ic_icon.xml` |
| An app or launcher icon | `vdt adaptive --foreground logo.svg --background-color '#FFFFFF' -o res` |
| A notification or status bar icon | `vdt notification bell.svg -o res/drawable-anydpi/ic_stat_bell.xml` |
| To know if an SVG will convert, without writing | `vdt check icon.svg` |
| Details: diagnostics, minimum API, bounds, size | `vdt inspect icon.svg --format json` |

`vdt icon.svg -o out.xml` is shorthand for `convert`. Every command except
`adaptive` accepts a directory as input; a directory needs `-o`, keeps its
tree, and a file that fails is named on stderr while the rest still convert.
Use `drawable-anydpi/` when `minSdk` is 21 or newer so vectors outrank
same-named density-specific bitmaps. Use plain `drawable/` when AndroidX
VectorDrawableCompat must load them below API 21, where `anydpi` is unavailable.

## Workflow: inspect, fix, generate, wire up

1. **Inspect first**, as the icon you intend to make, and read the JSON:

   ```sh
   vdt inspect logo.svg --as adaptive-foreground --fit 66 --format json
   vdt inspect bg.svg --as adaptive-background --format json
   vdt inspect bell.svg --as notification --format json
   ```

   `--as` takes `notification`, `adaptive-foreground` (also used for a
   monochrome layer), or `adaptive-background`. `--fit`, and
   `--background-fit` for a background, must match the values you will pass to
   the generator.

2. **Read the report.** Each file gives:

   ```json
   {
     "path": "logo.svg",
     "icon": { "kind": "adaptive-foreground", "fit": 66.0, "fit_mode": "contain" },
     "compatibility": "exact",
     "minimum_api": 21,
     "diagnostics": [
       { "code": "SVGVD019", "severity": "warning", "message": "...", "suggestion": "..." }
     ],
     "metrics": { "width": 24.0, "height": 24.0, "content_bounds": { "left": 2.0, "top": 2.0, "right": 22.0, "bottom": 22.0 } }
   }
   ```

   `compatibility` is `exact`, `exact_with_normalization`, `approximate`, or
   `unsupported`. The first two convert; `--allow-approximate` also allows
   `approximate` (e.g. centering radial gradients with focal offsets). `--strict`
   accepts only `exact`, so do not pass it unless the user asks. A diagnostic
   with severity `error` blocks conversion; `warning` and `info` do not, but
   warnings about icons mean the result will look wrong on a device. A file
   that could not be read or parsed appears as `{ "path", "error" }` instead.

   Exit codes: `0` everything converted or is convertible, `1` a file was
   malformed, unreadable, or failed, `2` (`check` and `inspect` only) an input
   is incompatible.

3. **Fix** using the table in [Diagnostics](#diagnostics). Flag changes such as
   `--fit` are yours to make. Changes to the artwork itself change how the icon
   looks: tell the user what you would change and why, and get agreement before
   editing the SVG. Never make an error go away by deleting content silently or
   by hand-editing the generated XML.

4. **Generate** with the same options you inspected with.

5. **Wire it up** (see each section below) and, if the project builds locally,
   build it to confirm the resources compile.

## Launcher icons: `adaptive`

```sh
vdt adaptive --foreground logo.svg --background-color '#3DDC84' \
    --monochrome logo.svg --fit 66 -o app/src/main/res
```

- Give exactly one foreground: `--foreground <svg>` or `--foreground-image
  <png|webp>`. A raster foreground must already be drawn on the full square
  layer with its artwork inside the 66dp safe zone, because `--fit` places
  vector artwork only; `SVGVD019` measures the painted pixels and says when it
  is not. A JPEG is rejected for this layer, and for `--monochrome-image`,
  because a layer with no alpha would hide the background — `SVGVD026` reports
  a PNG or WebP with the same problem. `--legacy` is rejected with
  `--foreground-image`.
- Give exactly one of `--background <svg>` (art
  that fills the layer), `--background-color <#RRGGBB>` (a flat color), or
  `--background-image <png|webp|jpg>` (a raster image at any path).
- `--background-image` is the answer when the background is a photo, a render,
  or anything else that is not an SVG. Pass the file wherever it sits; vdt
  copies it byte for byte into `mipmap-nodpi/` and points the icon at it. It
  never resamples the image, but it does decode it, so `SVGVD024`, `SVGVD025`,
  and `SVGVD020` still report a background that is not square, too small, or
  not fully opaque. PNG, WebP, and JPEG, recognized from the file's own header
  rather than its name; a JPEG has no alpha, so it can never raise `SVGVD020`.
  An animated WebP and `--legacy` are both rejected.
- `--background-fit` is how a background SVG is scaled onto the 108dp layer.
  The default `cover` fills it and crops whatever overflows, which is what a
  background wants: Android masks and shifts that layer, so anything it does
  not reach shows through. Note `SVGVD023` names how much was cropped. Use
  `contain` only when the user says the whole background image must stay
  visible; it letterboxes non-square art and then earns warning `SVGVD020`.
- `--fit` is the square, in dp, the foreground and monochrome art is scaled to,
  centered on the 108dp layer. Keep the default 108 only when the SVG was drawn
  on the full 108dp adaptive layer with its padding built in; otherwise start at
  66 and let `SVGVD019` correct you. Do not assume 66 is safe: a launcher mask
  is a *circle*, so the fit a logo needs depends on its shape. A round mark can
  fill 66, while one drawn into the corners of the box needs about 47, because
  the corners of a 66dp box sit 46.7dp from the centre and the mask shows only
  36dp. `SVGVD019` measures the painted pixels and names the fit that works —
  warning when the artwork is clipped, note when it is past the 33dp Android
  recommends. Pass the fit it suggests rather than guessing.
- `--monochrome` adds the layer Android 13+ uses for themed icons. Pass the
  foreground again if the logo is a single shape that reads well as a
  silhouette; recommend it, since without it themed launchers show the icon
  untinted.
- `--legacy` is needed when `minSdk` is below 26; it writes an icon for older
  devices, including PNGs when the art needs API 24.
- `--name` defaults to `ic_launcher`. Before generating, list
  `res/mipmap-*/<name>.*`. A new project template leaves bitmaps such as
  `mipmap-hdpi/ic_launcher.webp`; on devices below API 26 they can take
  precedence over what `vdt` writes. Ask the user before deleting them.
- Every layer is converted before anything is written, so a failure leaves
  `res/` untouched.

Then make sure the `<application>` element in `AndroidManifest.xml` has
`android:icon="@mipmap/ic_launcher"` and
`android:roundIcon="@mipmap/ic_launcher_round"` (using `--name` if changed).

## Notification icons: `notification`

```sh
vdt notification bell.svg -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml
vdt notification icons/ --fit 20 -o app/src/main/res/drawable-anydpi/
```

Android draws these from the alpha channel only, so every color becomes white
and opacity is kept. `--fit` defaults to 24, right for Material icons that carry
their own padding; use 20 for art drawn edge to edge. Name files `ic_stat_*` by
convention and reference them with `setSmallIcon(R.drawable.ic_stat_bell)`.

## Plain drawables: `convert` and `optimize`

- The SVG's `width` and `height` become the drawable size. `--size <dp>` sets
  the longer side and keeps the aspect ratio. An SVG with only a viewBox larger
  than 200, such as a Material Symbols download, becomes 24dp.
- `optimize` writes the same drawing with shorter numbers and path data.
  `notification` and `adaptive` take `--optimize` for the same effect.
- Output is compact, one-line XML. Pass `--pretty` only when someone will read
  or review the XML.
- File names are made valid Android resource names and the new name is
  printed: `Arrow-Left.svg` becomes `arrow_left.xml`. Use the printed name when
  you reference the resource in code.
- `minimum_api` is 24 when the drawable uses gradients, even-odd fills, or
  several clips, and an `SVGVD004` note names which. If it is above the
  module's `minSdk`, tell the user and pass on the note's suggestion.

## Diagnostics

Each diagnostic in the report says what is wrong and where: a `code`, a
`severity`, a specific `message`, and usually a `location` naming the element
and line. Every error and warning also carries a `suggestion` naming the fix;
follow it. As a rule:

- `error` blocks conversion. The cause is an SVG feature VectorDrawable cannot
  draw, such as text, filters, masks, patterns, images, dashed strokes, or
  animation. The fix is in the artwork.
- `warning` means the file converts but will look wrong, for example an icon
  outside the safe zone or a notification icon that tints into a solid square.
  These are usually fixed with an option such as `--fit` or `--size`.
- `info` needs no action, except `SVGVD004` when the module's `minSdk` is
  below 24, and `SVGVD023`, which is worth repeating to the user because it
  says part of their background artwork was cropped away.

Most errors are fixed in the design tool that produced the SVG. When the fix
is a mechanical SVG edit that does not change the look (inlining a `<use>`,
setting a viewBox), you can make it; otherwise explain the trade-off and let
the user decide. For radial gradients with off-center focal points (`SVGVD003`),
`--allow-approximate` allows lossy conversion by centering the focal point.

## In CI

`vdt check res-src/ --format json` exits `2` when any SVG is incompatible and
lists every problem with its code, so it can gate a pull request without
writing files.
