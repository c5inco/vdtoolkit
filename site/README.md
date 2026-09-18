# vdtoolkit demo site

A static page with two tools, both running `vdtoolkit`'s WebAssembly build
in the browser.

**Drawable** converts an SVG and draws the resulting VectorDrawable the way
two Android versions do, side by side:

| View | Ported from | Notable behavior |
| --- | --- | --- |
| Android 7 | `libs/hwui/VectorDrawable.cpp`, `android-7.0.0_r1` | Paths transformed to pixels before drawing; strokes scaled by the smaller group axis; clips not antialiased; gradients interpolate unpremultiplied |
| Latest | `libs/hwui/VectorDrawable.cpp`, AOSP main | Groups concatenate onto the canvas; antialiased clips; premultiplied gradients; arcs from `SkPath::arcTo` |

Android 5 (API 21) is off the page — AndroidX's minSdk is 23, so the practical
question is 24 vs latest. The API 21 port still lives in `src/android/api21.ts`
and is covered by the tests; its quirks (a clip replaces the previous one and
outlives its group; no gradients or `fillType`; `1.5.5` does not parse) are in
`tools/android-renderer/RESULTS.md`.

The ports live in `src/android/` and follow the AOSP control flow closely so
quirks carry over. They run on [CanvasKit](https://skia.org/docs/user/modules/canvaskit/),
Skia's WebAssembly build. Shapes, clips, and gradients match devices; edge
antialiasing reflects current Skia rather than each version's own. When the two
renders disagree, the Android 7 column quantifies it ("N% of pixels differ")
and the samples explain why in one sentence each.

The page is laid out as a specimen sheet: a ruled source strip over a grid of
evidence — Android 7 and latest share one half, the VectorDrawable XML takes
the other. The XML is highlighted by [gpu-lexer](https://gpu-lexer.vercel.app/),
a tiny WebGPU lexer; where WebGPU is unavailable a hand-rolled quote-aware
scanner in `src/xml-pane.ts` produces the same palette.

**Launcher icon** is `vdt adaptive` in the browser: foreground, background
(SVG or color), optional monochrome layer, and the `--fit`, `--legacy`,
`--optimize`, and `--name` flags. It shows the equivalent command line, the
files the command writes under `res/`, the icon under the launcher masks
(`src/icon-geometry.ts`, from `AdaptiveIconDrawable` and `config_icon_mask`),
and, with `--legacy`, the 48dp fallback drawn by the Android 7 and latest
ports. The
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
drawable sample with `#alpha-gradient`, `#angled-clip`, `#scaled-stroke`, or
`#stopwatch`.

The renderer tests reuse the pixel assertions from
`tools/android-renderer/test/src/com/vdtoolkit/renderer/test/RendererConformanceTest.java`
and the API 21 findings in `tools/android-renderer/RESULTS.md`, so a port that
drifts from what devices draw fails here. The geometry test checks the icon
constants against `src/adaptive.rs`.
