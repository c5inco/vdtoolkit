# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Elliptical and rotated radial gradients (arising from non-square bounding
  boxes or non-uniform `gradientTransform` scales and rotations) are supported
  by lowering into circular VectorDrawable radial gradients wrapped in a
  `<group>` with `scaleY`, `pivotX`, `pivotY`, and `rotation` transforms, with
  the path geometry counter-transformed. When a path has both an elliptical
  radial fill and a stroke, it splits into a filled transformed `<group>` and
  an unscaled stroked sibling path to keep the stroke uniform. Strokes using
  an elliptical gradient remain rejected.
- `--allow-approximate` opt-in flag on `convert`, `optimize`, `notification`,
  `adaptive`, `check`, and `inspect` (`*_with_options` in the Rust API and
  `allowApproximate` in WebAssembly bindings) allows lossy lowering for
  constructs VectorDrawable cannot draw exactly. When enabled, radial gradients
  with off-center focal points (`fx != cx` or `fy != cy`) are centered with
  warning `SVGVD003` and reported as `approximate` compatibility, rather than
  blocking conversion.

### Fixed

- A strongly elliptical radial gradient no longer disappears after `optimize`.
  Group attributes are rounded to three decimals, so a gradient near 2500:1
  produced `android:scaleY="0"`, which collapses the group and erases the
  drawable. The group scale is now clamped to the smallest value that spelling
  can express, and the gradient radius is derived from the clamped scale so the
  ellipse's short axis stays exact rather than being stretched to match.
  Verified on Android across API 21 to 36.

## [0.5.0] - 2026-09-19

### Added

- `inspect --as adaptive-foreground` and `--as adaptive-background` (and
  `check` with the same flags) accept a PNG, WebP, or JPEG layer image and
  report the same findings `adaptive --foreground-image` and
  `--background-image` would, without writing anything. A directory mixes
  SVGs and layer images. `--fit` does not place a raster layer, which is
  copied, never resampled. A JPEG foreground is still rejected (no alpha).
  A raster file argument without
  `--as`, or with `--as notification`, is refused rather than parsed as SVG;
  a directory with `--as notification` still keeps only SVGs. A JPEG in a
  directory scanned as `--as adaptive-foreground` is skipped the same way,
  because a JPEG has no alpha and would fail the whole run; pass the file
  itself to see that rejection. The JSON report gains an `image` field
  (`png`, `webp`, or `jpg`) on those entries, `compatibility` is then
  `not_applicable`, `metrics.width` / `height` are pixels of the image, and
  `content_bounds` / `viewport_*` stay dp on the 108dp layer. Their JSON
  `icon` keeps the layer kind but omits the unused `fit` / `fit_mode`, and
  zeroed vector-only metrics are omitted.

### Fixed

- `convert`, `optimize`, and `notification`, including the default
  `vdt <file>` form, recognize PNG, WebP, and JPEG headers and refuse those
  files as raster images instead of reporting UTF-8 or malformed XML errors.
  The CLI points to adaptive icon image options and `inspect --as`; the Rust
  and WebAssembly conversion APIs keep the flag-free error.

## [0.4.0] - 2026-09-18

### Added

- `adaptive --foreground-image <png|webp>` and `--monochrome-image` take a
  raster foreground or monochrome layer and place it the same way. Because
  `--fit` places vector artwork and vdt will not resample a bitmap, the image
  must already be drawn on the full square layer: `SVGVD019` measures the
  painted pixels against the 66dp safe zone and says when it is not, with half
  a dp of tolerance for antialiased edges. A foreground is judged the opposite
  way round to a background — the new `SVGVD026` reports one with no
  transparent pixels, which covers the background entirely, and `SVGVD018`
  reports one with nothing painted. A JPEG is rejected for these layers, since
  a layer with no alpha channel cannot sit over anything. `--fit` is rejected
  when every layer it would place is a raster image, and `--legacy` alongside
  `--foreground-image`.
- `adaptive --background-image <png|webp|jpg>` takes a raster background from
  anywhere on disk and places it, so a bitmap background no longer needs a res
  folder laid out or the icon XML wired up by hand. The file is copied byte for
  byte into `mipmap-nodpi/<name>_background.<ext>`, where Android draws it onto
  the layer without scaling it for density; vdt never resamples it. The format
  is read from the file's own header, so a mislabeled file is still written
  under the extension it really is; `.jpeg` is normalized to `.jpg`. Every
  image is decoded in full before anything is written, so one whose header
  reads but whose data is cut short or damaged is refused, rather than copied
  into the project to fail when Android packages or draws it. A JPEG carries no
  alpha channel, so it can never raise `SVGVD020`; it has no checksum either, so
  data damaged into bytes that still decode is caught by no decoder. An
  animated WebP and `--legacy`, which composes both layers into one vector, are
  both rejected alongside it.
- `SVGVD024` and `SVGVD025`, which report a background image that is not square
  or does not carry the 432px an xxxhdpi device draws the 108dp layer at.
  `SVGVD020` now also reports a background image that is not fully opaque, the
  same finding a vector background that leaves gaps gets. vdt decodes the image
  without changing it, so the layer it does not convert is still checked.
- `adaptive --background-fit cover|contain` chooses how a background SVG is
  scaled onto the 108dp layer. `cover` fills the layer and lets it crop
  whatever overflows; `contain` is the previous behavior. `inspect` and
  `check` take the same option with `--as adaptive-background`, and their
  JSON `icon` object gained a `fit_mode` field.
- `SVGVD023`, a note naming what percentage of a background the layer cropped
  when covering it. Like `SVGVD021` it is printed by the generator, because it
  names something the generator changed about the artwork rather than
  something the source already had.
- The Figma plugin can batch-export selected frames, components, and instances
  as optimized white 24dp notification icons. It uses the existing review UI,
  reports notification-specific plate and empty-artwork warnings, and bundles
  results as `notification-icons.zip`.
- Optional Hugeicons stress suite, `tools/check-hugeicons.py`, covering all
  6,143 free Stroke Rounded icons from a pinned commit. It checks a coverage
  floor and repeated-output determinism.

### Fixed

- `adaptive --legacy` now replaces the legacy icon Android Studio leaves in a
  project. Studio writes it as `ic_launcher.webp` and `ic_launcher_round.webp`
  in every density folder, for new projects and from its Image Asset wizard,
  and vdt only removed `.png` copies. With art that needs API 24, vdt's PNGs
  landed beside Studio's WebPs and the build failed with a conflicting
  resource; with API 21 art the build succeeded but Studio's icons could be
  chosen over vdt's on devices below API 26. Every other version of the legacy
  icon in any mipmap folder, in any format, is now removed when `--legacy`
  writes one. Without `--legacy`, Studio's icon is still the project's icon
  for older devices and is left alone.
- Wide-gamut `color()` values, which Figma writes into `style` after an sRGB
  fallback, are resolved to sRGB before parsing. The SVG parser dropped the
  declaration it could not read, and the `style` attribute outranks the
  matching presentation attribute, so strokes silently vanished and fills
  silently turned black. Display P3, sRGB, and linear sRGB resolve; other
  color spaces are left untouched. `inspect` reports the resolution as a new
  `SVGVD022` note; like the other informational notes it does not reach
  `convert` output or the Figma export dialog, where every notification icon
  flattens to white anyway.

### Changed

- Output names keep more of the file name. `convert`, `optimize`, and
  `notification` drop accents instead of the accented letters, so `Café.svg`
  becomes `cafe.xml` rather than `caf.xml`. They also spell out `+`, `#`, `&`,
  `@`, and `%`, so `C++.svg` becomes `c_plus_plus.xml` rather than colliding
  with `C.svg`. Inputs with these characters get new output names; delete the
  old outputs and update references. The Figma plugin spells out the same
  symbols.
- `vdtoolkit` is now a CLI only. The experimental Rust API is no longer
  documented or supported: its items are hidden from generated docs and exist
  only so the `vdt` binary, WebAssembly bindings, and tests can share code.
  Use `vdt` instead of depending on the crate.
- **`SVGVD019` now measures distance from the centre of the layer, not a
  bounding box, and reports against the circle a launcher mask actually shows.**
  The old check asked whether content stayed inside a 66dp *square*, but a mask
  is a circle: the corners of that square sit 46.7dp from the centre while a
  circular mask shows 36dp, so artwork drawn edge to edge at the recommended
  `--fit 66` passed the check and was visibly clipped on a device. vdt now
  renders the layer and measures the painted pixels, which also removes the
  false positive a box test would have: a round logo filling the 66dp box
  reaches only 33dp and is fine. Past 36dp is a warning, because the clipping is
  certain; between 33 and 36dp is a note, because a circular mask still shows it
  but another mask may not. `adaptive` prints the note as well as `inspect`. The finding names the `--fit` that would bring the
  artwork in. Expect new warnings on unchanged input: artwork that was certified
  before and is genuinely clipped now says so.
- Regenerating an adaptive icon removes a file an earlier run left behind only
  when keeping it would break the build, the way switching legacy layouts
  already did to avoid showing the old icon: changing a raster layer's format,
  say from `.png` to `.webp`, leaves two files with one resource name in one
  folder, which `aapt2` refuses to build. Writing a raster layer also removes
  the versions of it in density folders such as `mipmap-xxhdpi/`, which
  Android Studio's Image Asset wizard writes for every density: Android picks
  those over `mipmap-nodpi/` on a device of that density, so a project moved
  from Studio to vdt kept showing its old icon. Verified on an emulator at API
  36. Any other leftover, such as a vector layer replaced by an image or a
  color background replaced by an SVG, is a different resource type that
  collides with nothing. vdt cannot tell whether the rest of the project still
  refers to it, and deleting a resource in use would break the build itself,
  so it names the file in a note and leaves it.
- **Adaptive backgrounds now cover the layer by default.** Android crops the
  background layer with launcher masks and shifts it for parallax, so a
  background that does not reach every edge shows through. Non-square
  background artwork was scaled to fit and left letterboxed, which earned an
  `SVGVD020` warning that nothing but redrawing the artwork could clear;
  it now fills the layer. Regenerating an icon from non-square background
  artwork produces a different drawable than 0.3.0 did, and part of that
  artwork is cropped; pass `--background-fit contain` to restore the old
  result. Square backgrounds are unaffected.
- The Figma export dialog closes once the zip is handed to the browser, and
  reports the count as it closes instead of leaving itself open behind the
  save prompt. Enter now runs Export, the way a dialog's default button does,
  except while a button or the filter box has focus.
- Vector drawables packaged by the Figma exporter and adaptive icon layers
  now use `drawable-anydpi/`. CLI examples recommend the same directory for
  caller-selected `convert`, `optimize`, and `notification` outputs.

## [0.3.0] - 2026-09-15

### Added

- An agent skill, `skills/vdt/SKILL.md`, teaches coding agents such as Claude
  Code and Codex when to use `vdt`, which order to run its commands in, and how
  to act on its diagnostics. A test fails when the skill names a command or
  flag that `vdt` does not have.
- Every error and warning diagnostic carries a suggestion naming the fix, such
  as converting a dashed stroke to filled outlines or copying referenced content
  into the SVG. `check` and `inspect` print it under the diagnostic, as the JSON
  report and the Figma plugin already did.
- A drawable that needs API 24 carries an `SVGVD004` note naming why:
  gradients, even-odd fills (`android:fillType`), or more than one clip path,
  with a suggestion for staying on API 21. `inspect` and `check` list it and
  the JSON report includes it; it is informational, so compatibility, exit
  codes, and `convert` output are unchanged. The code was defined but never
  emitted before. A notification icon whose gradients flatten to white drops
  the note along with the API 24 requirement.

## [0.2.0] - 2026-09-14

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

- The project is renamed from `svg2vd` to `vdtoolkit`. The installed command is
  now `vdt`, and release archives are named `vdtoolkit-v<version>-<target>`.
  Scripts that call `svg2vd` need to call `vdt` instead; its commands and
  options are otherwise unchanged.
- The macOS release binaries are signed with a Developer ID certificate and
  notarized by Apple.
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

[Unreleased]: https://github.com/c5inco/vdtoolkit/compare/v0.5.0...HEAD
[0.5.0]: https://github.com/c5inco/vdtoolkit/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/c5inco/vdtoolkit/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/c5inco/vdtoolkit/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/c5inco/vdtoolkit/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/c5inco/vdtoolkit/releases/tag/v0.1.0
