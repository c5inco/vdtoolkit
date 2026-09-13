# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- `notification` command that converts SVGs to white 24dp notification icon
  drawables: every fill and stroke is flattened to white with its opacity kept,
  since Android tints the alpha channel alone, and the artwork is scaled
  uniformly into a centered `--fit` square on the 24dp canvas. A gradient with
  uniform stop opacity collapses to solid white, which drops the minimum API
  back to 21; a fading gradient is kept with white stops. Stderr names what was
  flattened, and warns about artwork that covers almost the whole canvas or
  paints nothing. The Rust API gains `Asset::to_notification_icon`,
  `Asset::painted_coverage`, `Flattening`, `NOTIFICATION_ICON_SIZE`, and
  `NOTIFICATION_ICON_LIVE_AREA`.
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
- Optional Rune Icons stress suite, `tools/check-runeicons.py`, covering 908
  icons in five styles from a pinned commit. It checks per-style coverage
  floors and repeated-output determinism.

### Fixed

- `convert`, `check`, and `inspect` no longer stop at the first failing file in
  a directory. Each failure is reported with its path, the remaining files are
  processed, and the command exits 1. JSON reports include an entry with the
  path and error for files that could not be read or parsed.

### Changed

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
