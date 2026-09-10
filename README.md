# svg2vd

`svg2vd` is a standalone native Rust toolchain for checking, inspecting,
converting, and optimizing SVG assets for Android VectorDrawable. It does not
require Android Studio, the Android SDK, Gradle, Java, the JVM, or AOSP.

```sh
cargo install --path .
svg2vd convert icon.svg -o ic_icon.xml
svg2vd check icon.svg
svg2vd inspect icon.svg --format json
svg2vd optimize icon.svg -o ic_icon.xml
svg2vd icon.svg -o ic_icon.xml
```

Directories are accepted by every command. Directory conversion requires `-o`
and preserves the input tree below that output directory. `check` exits 0 for
exactly convertible input, 2 for incompatible input, and 1 for I/O or malformed
input.

## Compatibility profile

The current profile converts dimensions, `viewBox`, paths, primitive shapes,
`<use>`, inherited styles, nested affine transforms, solid fills, fill opacity,
fill rules, solid strokes, stroke opacity/width/caps/joins, conservative
single-path clips, and deterministic XML. Geometry transforms are flattened. A
stroke may only be flattened through a similarity transform (translation,
rotation, reflection, and uniform scale).

The source preflight rejects DTDs, entity declarations, masks, filters, text,
images, patterns, animation, external references, unsupported compositing,
gradients, non-uniformly transformed strokes, and clip semantics VectorDrawable
cannot represent exactly. A clip definition must resolve to one nonzero path;
multi-path unions, even-odd rules, nested clips, and paint effects inside clip
definitions are rejected rather than silently changed.

`Compatibility` has four stable states: `exact`,
`exact_with_normalization`, `approximate`, and `unsupported`. Conversion only
accepts the first two. `--strict` accepts only `exact`.

Plain paths and clip paths target platform API 21. Ordinary paths using
`android:fillType` require API 24. The analysis report derives the minimum API
from the emitted drawable instead of assigning one level to every conversion.

## Security and determinism

SVG is treated as untrusted input. External entity declarations are rejected,
and `usvg` is configured with data and external image resolution disabled. No
network or filesystem resource is loaded from an SVG. Identical bytes and
options produce identical XML.

## Upstream research baseline

The initial behavioral research baseline is AOSP `platform/tools/base`, Android
Studio mirror commit
[`cab65fa60a996079b6f6d9b62025df3d98c135bc`](https://android.googlesource.com/platform/tools/base/+/cab65fa60a996079b6f6d9b62025df3d98c135bc/),
specifically `Svg2Vector.java` and `VectorDrawableGeneratorTest.java`. AOSP is a
reference, not a source or dependency. General SVG normalization uses `usvg`
0.48.1. No AOSP implementation code is copied.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
python3 tools/check-material-symbols.py
python3 tools/check-material-symbols.py --suite twotone-100
```

The core test suite is Rust-only. The optional corpus commands download and
semantically compare pinned, fixed-seed 100-icon outlined and Two Tone corpora
with Google's official Android drawables; those Apache-2.0 assets are not
vendored. Two Tone comparison includes per-path fill alpha. Use `--suite
outlined-all` for the exhaustive pinned outlined corpus. Run `bash
tools/accept-v1.sh` for the complete V1 acceptance and local release packaging
gate. The exhaustive corpus runs manually and before release, not on a timer,
because its pinned contents are immutable. Android renderer/AOSP conformance
remains optional and must not become a normal build dependency.

Licensed under `MIT OR Apache-2.0`.
