# vdt guide: diagnostics

`vdt inspect <file> --format json` writes one entry per file:

```json
{
  "path": "logo.svg",
  "icon": { "kind": "adaptive-foreground", "fit": 66.0, "fit_mode": "contain" },
  "compatibility": "exact",
  "minimum_api": 21,
  "diagnostics": [
    { "code": "VDT019", "severity": "warning", "message": "...", "suggestion": "..." }
  ],
  "metrics": { "width": 24.0, "height": 24.0, "content_bounds": { "left": 2.0, "top": 2.0, "right": 22.0, "bottom": 22.0 } }
}
```

- `icon` is present when `--as` was given.
- `compatibility` is `exact`, `exact_with_normalization`, `approximate`, or
  `unsupported`. The first two convert. `approximate` converts only with
  `--allow-approximate`; `--strict` accepts only `exact`.
- `minimum_api` is the lowest Android API level that draws the result.
- `metrics` gives the size, the viewport, and `content_bounds`, the painted
  area in viewport units. For an icon, `viewport_*` and `content_bounds` are dp
  on the icon's canvas.
- A raster layer image instead has `"image": "png"`, `"webp"`, or `"jpg"` and
  `"compatibility": "not_applicable"`. `metrics.width` and `metrics.height`
  are its pixels.
- A file that could not be read or parsed appears as `{ "path", "error" }`.

## Severity

- `error` blocks conversion. The SVG uses something VectorDrawable cannot
  draw: text, filters, masks, patterns, images, dashed strokes, animation. The
  fix is in the artwork, usually in the design tool that exported it.
- `warning` means the file converts but will look wrong on a device, such as
  an icon outside the safe zone. These are usually fixed with an option such
  as `--fit` or `--size`.
- `info` needs no action, except `VDT004` when `minimum_api` is above the
  module's `minSdk`, and `VDT023`, which says part of the background artwork
  was cropped and is worth repeating to the user.

Every error and warning carries a `suggestion`; follow it. When the fix is a
mechanical SVG edit that does not change the look, such as setting a viewBox,
you can make it. Otherwise explain the trade-off and let the user decide.
`vdt explain <code>` explains a code in depth; `vdt explain` lists them all.

## Exit codes

`0` everything converted or is convertible; `1` a file was malformed,
unreadable, or failed; `2` (`check` and `inspect` only) an input is
incompatible.
