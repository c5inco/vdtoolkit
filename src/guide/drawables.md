# vdt guide: drawables

```sh
vdt inspect icon.svg --format json
vdt convert icon.svg -o app/src/main/res/drawable-anydpi/ic_icon.xml
vdt optimize icons/ -o app/src/main/res/drawable-anydpi/
```

## Where to write

Use `drawable-anydpi/` when `minSdk` is 21 or newer, so vectors outrank
same-named density-specific bitmaps. Use plain `drawable/` when AndroidX
VectorDrawableCompat must load them below API 21, where `anydpi` is not
available.

## Size

The SVG's `width` and `height` become the drawable's size in dp. `--size <dp>`
sets the longer side and keeps the aspect ratio; the drawing is unchanged. An
SVG with only a viewBox larger than 200, such as a Material Symbols download,
becomes 24dp. `VDT016` warns about drawables larger than 200dp, which are
slow to draw; use `--size`, or a raster image for large artwork.

## `convert` or `optimize`

`optimize` writes the same drawing with shorter numbers and path data. Prefer
it for shipped resources. `notification` and `adaptive` take `--optimize` for
the same effect. Output is one-line XML; pass `--pretty` only when someone
will read or review it.

## Names

File names are made valid Android resource names and the new name is printed:
`Arrow-Left.svg` becomes `arrow_left.xml`. Use the printed name in code.

## Minimum API

`minimum_api` is 24 when the drawable uses gradients, even-odd fills, or more
than one clip, and a `VDT004` note names which. If that is above the module's
`minSdk`, tell the user and pass on the note's suggestion: raise `minSdk` to
24, or change the artwork as the note describes.

## Options that change what converts

- `--strict` accepts only `exact`, rejecting files that need safe
  normalization (`VDT011`). Do not pass it unless the user asks.
- `--allow-approximate` accepts `approximate` lowering; today that is
  centering a radial gradient's off-center focal point (`VDT003`). The result
  looks different from the SVG, so tell the user before you rely on it.
