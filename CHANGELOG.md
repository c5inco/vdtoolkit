# Changelog

All notable changes to this project are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and version numbers
follow [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- Linear gradients and circular radial gradients on fills and strokes, written
  as inline `aapt` gradients. Stop opacity and all spread methods are kept, and
  gradients report API 24 as the minimum. Radial gradients with a focal point,
  a focal radius, or an elliptical shape are still rejected.
- `inspect` reports content bounds: the painted area including strokes,
  clamped to the viewport.

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

[Unreleased]: https://github.com/c5inco/svg2vd/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/c5inco/svg2vd/releases/tag/v0.1.0
