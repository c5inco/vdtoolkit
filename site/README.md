# vdtoolkit demo site

A static page with two tools, both running `vdtoolkit`'s WebAssembly build
in the browser.

**Drawable** converts an SVG and draws the resulting VectorDrawable the way
three Android versions do:

| View | Ported from | Notable behavior |
| --- | --- | --- |
| API 21 | `VectorDrawable.java` and `PathParser.java`, `android-5.0.0_r1` | A clip replaces the previous one and outlives its group; clips are not antialiased; no gradients or `fillType`; `1.5.5` does not parse |
| API 24 | `libs/hwui/VectorDrawable.cpp`, `android-7.0.0_r1` | Paths transformed to pixels before drawing; strokes scaled by the smaller group axis; clips not antialiased; gradients interpolate unpremultiplied |
| Latest | `libs/hwui/VectorDrawable.cpp`, AOSP main | Groups concatenate onto the canvas; antialiased clips; premultiplied gradients; arcs from `SkPath::arcTo` |

The ports live in `src/android/` and follow the AOSP control flow closely so
quirks carry over. They run on [CanvasKit](https://skia.org/docs/user/modules/canvaskit/),
Skia's WebAssembly build. Shapes, clips, and gradients match devices; edge
antialiasing reflects current Skia rather than each version's own. Hovering a
render opens a loupe on the same pixels in all three.

**Launcher icon** is `vdt adaptive` in the browser: foreground, background
(SVG or color), optional monochrome layer, and the `--fit`, `--legacy`,
`--optimize`, and `--name` flags. It shows the equivalent command line, the
files the command writes under `res/`, the icon under the launcher masks
(`src/icon-geometry.ts`, from `AdaptiveIconDrawable` and `config_icon_mask`),
and, with `--legacy`, the 48dp fallback drawn by the three Android ports. The
wasm binding `adaptiveIcon` in `bindings/wasm` produces the same file list as
the command; legacy PNGs are rendered in the page from the legacy vector.

## Build and test

Requires Node 22.18 or newer (tests run TypeScript directly), Rust, and
`wasm-pack`.

```sh
npm ci
npm run build      # builds the wasm package, then dist/
npm test           # renderer ports against device-verified pixels; icon geometry
npm run typecheck
```

Serve `dist/` with any static file server, for example
`python3 -m http.server -d dist`. Link to the icon tool with `#icon`, or to a
drawable sample with `#nested-clips`, `#mask`, `#radial-gradient`, or
`#stopwatch`.

The renderer tests reuse the pixel assertions from
`tools/android-renderer/test/src/com/vdtoolkit/renderer/test/RendererConformanceTest.java`
and the API 21 findings in `tools/android-renderer/RESULTS.md`, so a port that
drifts from what devices draw fails here. The geometry test checks the icon
constants against `src/adaptive.rs`.
