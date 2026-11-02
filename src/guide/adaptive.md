# vdt guide: launcher icons

```sh
vdt inspect logo.svg --as adaptive-foreground --fit 66 --format json
vdt adaptive --foreground logo.svg --background-color '#3DDC84' \
    --monochrome logo.svg --fit 66 -o app/src/main/res
```

`adaptive` writes an adaptive icon (API 26+) into a `res/` directory: the
layers as drawables, `mipmap-anydpi-v26/<name>.xml` and `<name>_round.xml`,
and, with `--legacy`, an icon for older devices. Every layer is converted
before anything is written, so a failure leaves `res/` untouched.

## Layers

- **Foreground**, exactly one: `--foreground <svg>` or
  `--foreground-image <png|webp>`. A raster foreground is copied byte for byte
  into `mipmap-nodpi/`, so it must already be drawn on the full square layer
  with its artwork inside the safe zone: `--fit` places vector artwork only.
  A JPEG is refused for this layer, because a layer with no alpha would hide
  the background; `VDT026` reports a PNG or WebP with the same problem.
- **Background**, exactly one: `--background <svg>` (art that fills the
  layer), `--background-color <#RRGGBB>` (a flat color), or
  `--background-image <png|webp|jpg>` (a photo, a render, anything that is not
  an SVG). A background image is copied byte for byte into `mipmap-nodpi/` and
  never resampled; `VDT024`, `VDT025`, and `VDT020` report one that is not
  square, too small, or not fully opaque.
- **Monochrome**, optional: `--monochrome <svg>` or `--monochrome-image`. This
  is the layer Android 13+ tints for themed icons. Without it, themed
  launchers show the icon untinted, so recommend it. Pass the foreground again
  when the logo reads well as a single-color silhouette. Android tints the
  layer by alpha only, so a mark drawn on an opaque plate or tile becomes a
  solid tinted shape: it needs a version without the plate, which is an
  artwork change to raise with the user.

## Fitting the foreground: `--fit`

`--fit` is the square, in dp, that foreground and monochrome vector art is
scaled to, centered on the 108dp layer.

- Keep the default 108 only when the SVG was drawn on the full 108dp adaptive
  layer with its padding built in.
- Otherwise start at 66 and let `VDT019` correct you. 66 is not always safe:
  a launcher mask is a *circle* 72dp across, so what matters is how far the
  painted pixels reach from the centre. A round mark can fill 66. A mark drawn
  into the corners of its box needs about 47, because the corners of a 66dp
  box sit 46.7dp from the centre and the mask shows only 36dp.
- `VDT019` is a warning past 36dp (clipped by a circular mask) and a note past
  33dp (outside the radius Android asks key content to stay inside). Its
  suggestion names the fit that works. Pass that fit, inspect again, and
  generate with it.

## Fitting a background SVG: `--background-fit`

`cover`, the default, fills the layer and crops whatever overflows. That is
what a background wants, because Android masks and shifts the layer and
anything it does not reach shows through. `VDT023` says how much was cropped;
repeat it to the user. Use `contain` only when the user says the whole
background must stay visible: it letterboxes non-square art and earns
warning `VDT020`.

## Older devices: `--legacy`

Pass `--legacy` when the module's `minSdk` is below 26. It writes the
background and foreground under a circular mask as a vector in `mipmap/`, or,
when the art needs API 24, a vector in `mipmap-anydpi-v24/` plus lossless
WebPs in `mipmap-mdpi/` through `mipmap-xxxhdpi/`. `--legacy` is refused with
`--foreground-image` and `--background-image`.

## Existing launcher icons

`--name` defaults to `ic_launcher`. Before generating, list
`res/mipmap-*/<name>.*` and `res/mipmap-*/<name>_round.*`. A new project
template leaves bitmaps such as `mipmap-hdpi/ic_launcher.webp`.

- With `--legacy`, vdt replaces every other version of `<name>` and
  `<name>_round` in the `mipmap-*` folders, deleting each one and printing
  `removed stale resource` for it: on devices below API 26 a density-specific
  bitmap would otherwise outrank the legacy icon vdt writes. That is what you
  want for template placeholders. If they could be the user's real icon and
  the project is not under version control, ask first, or pass a new `--name`
  and point the manifest at it, which leaves them in place.
- Without `--legacy`, vdt leaves them, and devices below API 26 keep showing
  them. Tell the user, and ask before deleting them.
- vdt overwrites its own files from an earlier run in place, and names any it
  no longer uses in a `note:` without deleting them.

## Wire it up

The `<application>` element in `AndroidManifest.xml` needs
`android:icon="@mipmap/ic_launcher"` and
`android:roundIcon="@mipmap/ic_launcher_round"`, using `--name` if you changed
it. Add `roundIcon` if it is missing.
