# vdtoolkit demo site

A static page that converts an SVG with the `vdtoolkit` WebAssembly build and
draws the resulting VectorDrawable the way three Android versions do:

| View | Ported from | Notable behavior |
| --- | --- | --- |
| API 21 | `VectorDrawable.java` and `PathParser.java`, `android-5.0.0_r1` | A clip replaces the previous one and outlives its group; clips are not antialiased; no gradients or `fillType`; `1.5.5` does not parse |
| API 24 | `libs/hwui/VectorDrawable.cpp`, `android-7.0.0_r1` | Paths transformed to pixels before drawing; strokes scaled by the smaller group axis; clips not antialiased; gradients interpolate unpremultiplied |
| Latest | `libs/hwui/VectorDrawable.cpp`, AOSP main | Groups concatenate onto the canvas; antialiased clips; premultiplied gradients; arcs from `SkPath::arcTo` |

The ports live in `src/android/` and follow the AOSP control flow closely so
quirks carry over. They run on [CanvasKit](https://skia.org/docs/user/modules/canvaskit/),
Skia's WebAssembly build. Shapes, clips, and gradients match devices; edge
antialiasing reflects current Skia rather than each version's own.

## Build and test

Requires Node 22.18 or newer (tests run TypeScript directly), Rust, and
`wasm-pack`.

```sh
npm ci
npm run build      # builds the wasm package, then dist/
npm test           # checks the ports against device-verified pixels
npm run typecheck
```

Serve `dist/` with any static file server, for example
`python3 -m http.server -d dist`. Link to a sample with `#nested-clips`,
`#mask`, `#radial-gradient`, or `#stopwatch`.

The tests reuse the pixel assertions from
`tools/android-renderer/test/src/com/vdtoolkit/renderer/test/RendererConformanceTest.java`
and the API 21 findings in `tools/android-renderer/RESULTS.md`, so a port that
drifts from what devices draw fails here.
