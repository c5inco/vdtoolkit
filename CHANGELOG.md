# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `convert`, `optimize`, and `notification` write valid Android resource names.
  A file name Android would reject, such as `Arrow-Left.svg`, is written as
  `arrow_left.xml` with a note on stderr; a name starting with a digit or equal
  to a Java keyword gains an `ic_` prefix. An input whose name is already taken
  by another input's output fails instead of overwriting it. `adaptive --name`
  also rejects Java keywords.
- `convert --size <dp>` and `optimize --size <dp>` set the drawable's longer
  side, keeping the aspect ratio and the viewport. An SVG with only a viewBox
  larger than 200 is drawn at 24dp by default, and a drawable with a declared
  size over 200dp gets a hint to pass `--size`. The Rust API gains
  `Asset::set_size` and `Asset::has_declared_size`.
- The findings of the generator commands carry diagnostic codes. `SVGVD017`
  warns that a notification icon paints almost the whole canvas, `SVGVD018`
  that it has no painted content, `SVGVD019` that adaptive foreground or
  monochrome content leaves the 66dp safe zone, `SVGVD020` that an adaptive
  background does not fill the layer, and the `SVGVD021` note names what was
  flattened to white. `notification` and `adaptive` print them with their
  codes on stderr, as `convert` prints its warnings.
- `check --as <kind>` and `inspect --as <kind>` report those findings without
  writing anything, for `notification`, `adaptive-foreground`, and
  `adaptive-background`, with `--fit` as the generator's fit. The metrics
  describe the fitted result, and the JSON report gains an `icon` object with
  the kind and fit. The Rust API gains `IconKind`, `Asset::to_icon`,
  `analyze_as`, and `analyze_file_as`.
- `adaptive --optimize` shortens numbers in the foreground, background, and
  monochrome layer drawables and the legacy vector the way the `optimize`
  command does. Fitting a layer scales every coordinate by a non-round factor,
  which is exactly what produces long decimals, so the pass runs after the
  fit. Legacy PNGs are rendered from the exact vector either way, since a
  thousandth of a dp can still move an anti-aliased edge by one coverage step
  at 96px and above.
- `notification` command that converts SVGs to white 24dp notification icon
  drawables: every fill and stroke is flattened to white with its opacity kept,
  since Android tints the alpha channel alone, and the artwork is scaled
  uniformly into a centered `--fit` square on the 24dp canvas. A gradient with
  uniform stop opacity collapses to solid white, which drops the minimum API
  back to 21; a fading gradient is kept with white stops. Stderr names what was
  flattened, and warns about artwork that covers almost the whole canvas or
  paints nothing. `--optimize` shortens numbers the way the `optimize` command
  does, after the fit. The Rust API gains `Asset::to_notification_icon`,
  `Asset::painted_coverage`, `Flattening`, `NOTIFICATION_ICON_SIZE`, and
  `NOTIFICATION_ICON_LIVE_AREA`.
- The Android renderer harness generates notification icons with
  `vdt notification` and verifies on device that the artwork is pure white,
  keeps its opacity, takes `setTint` at every painted pixel, and keeps a fading
  gradient's alpha ramp on API 24+, then posts a notification and checks the
  shade shows it.
- `SVGVD016` warning for drawables larger than Android's recommended 200×200dp
  vector icon size. It does not change compatibility; `convert` and `optimize`
  print it to stderr, and `check` and `inspect` list it with other diagnostics.
- `Asset::fit_within` scales `android:width` and `android:height` down to a
  maximum size in dp while keeping the viewport.
- `adaptive` command that generates an Android adaptive launcher icon from
  layer SVGs: 108dp foreground, background, and optional monochrome drawables,
  a solid background as a color resource, and the `mipmap-anydpi-v26` icon
  resources. `--fit` scales artwork uniformly into a centered square such as
  the 66dp safe zone, with warnings when foreground content leaves it or a
  background does not fill the layer, and
  `--legacy` writes a masked fallback icon for devices below API 26: a vector,
  or, when the art needs API 24, an API 24 vector plus PNGs at every density.
  The Rust API gains `Asset::fit_adaptive_layer`, `outside_adaptive_safe_zone`,
  `fills_adaptive_layer`,
  `solid_adaptive_layer`, `legacy_launcher_icon`, `render_rgba`, `to_png`, `adaptive_icon_xml`, and
  `color_resource_xml`.
- The Material corpus suites in `tools/check-material-symbols.py` also
  generate an adaptive icon from every icon and verify determinism, the
  generated resources, and that the foreground layer is the plain drawable
  fitted to the safe zone.
- The Android renderer harness builds an adaptive icon from Material Symbols
  layers with `vdt adaptive`, checks the layers and the `@color` background on
  device, and drives the launcher through `UiAutomation` to confirm the icon
  is composed, saving screenshots. A new `tools/android-renderer/run.sh` runs
  the harness on emulator.wtf across a device matrix or on a local adb device.
- Linear gradients and circular radial gradients on fills and strokes, written
  as inline `aapt` gradients. Stop opacity and all spread methods are kept, and
  gradients report API 24 as the minimum. Radial gradients with a focal point,
  a focal radius, or an elliptical shape are still rejected.
- `inspect` reports content bounds: the painted area including strokes,
  clamped to the viewport.
- Optional Compose renderer harness under `tools/android-renderer/compose`
  that verifies generated drawables, including gradients, through Jetpack
  Compose's VectorDrawable parser. It records one Compose divergence: a
  `</group>` closes enclosing clip paths, so a sibling after a nested group
  renders unclipped in Compose while the platform clips it correctly.
- Optional complex-illustration corpus, `tools/check-illustrations.py`: 637
  SVGs from four openly licensed sources, pinned by commit and content hash in
  `tests/illustrations.txt`, covering many-path scenes, nested clips, masks,
  gradients on most paths, and group opacity. It reports compatibility and
  the rejection reasons per source, checks determinism and that every file
  pinned in `tests/illustrations-convertible.txt` still converts, and compares each converted drawable against a resvg
  render of its source, by differing pixels and by mean channel difference
  over painted pixels, through the new `examples/compare.rs`, which adds
  `resvg` as a dev-dependency. Findings are in `docs/research.md`.
- Optional Rune Icons stress suite, `tools/check-runeicons.py`, covering 908
  icons in five styles from a pinned commit. It checks per-style coverage
  floors and repeated-output determinism.

### Fixed

- Numbers above about 64 in generated drawables no longer spell out binary
  noise: `107.64` printed as `107.639999`, because six fixed decimals is finer
  than an `f32` can resolve there. The writer now uses the shortest decimal
  that reads back as the same value whenever that is shorter, which is what
  made `optimize` ineffective on 108dp adaptive layers.
- `convert`, `check`, and `inspect` no longer stop at the first failing file in
  a directory. Each failure is reported with its path, the remaining files are
  processed, and the command exits 1. JSON reports include an entry with the
  path and error for files that could not be read or parsed.

### Changed

- `optimize`, and `--optimize` on `notification` and `adaptive`, write path data
  in its shortest form: each command absolute or relative, whichever is shorter,
  `H` and `V` for horizontal and vertical lines, and no repeated command letters,
  unneeded separators, or leading zeros. A relative spelling is used only where
  Android's parser, adding it up in floats, reaches exactly the number the
  absolute spelling gives, so drawables render as they did with absolute path
  data. `convert` output is unchanged; `Asset::optimize` does the same in the
  Rust API.
- The experimental Rust API now boxes the analysis inside
  `Error::Incompatible`. Field access and pattern matches are unchanged; code
  that moves the analysis out needs to dereference the box.

## [0.1.0] - 2026-09-10

### Added

- Native `convert`, `check`, `inspect`, and `optimize` commands for files and
  directory trees.
- Deterministic lowering of paths, primitive shapes, local uses, inherited
  styles, affine transforms, solid fills and strokes, opacity, and fill rules.
- Conservative single-path clip support and hard white mask lowering with
  Android API requirements derived from generated output.
- Stable diagnostics, JSON reports, strict compatibility checks, and rejection
  of unsupported or lossy SVG constructs.
- A small experimental Rust API for embedding analysis and conversion during
  the `0.x` series.
- Paired Material corpus conformance, Studio Icons stress coverage, release
  packaging, and optional Android pixel-renderer verification.

[Unreleased]: https://github.com/c5inco/vdtoolkit/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/c5inco/vdtoolkit/releases/tag/v0.1.0
