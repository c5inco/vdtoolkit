# V1 acceptance

Run the complete acceptance gate with:

```sh
bash tools/accept-v1.sh
```

The gate maps to the V1 product criteria as follows:

1. **One native executable:** builds a release executable, installs it into an
   empty temporary prefix, runs the installed binary, and creates a versioned
   archive plus SHA-256 checksum.
2. **No Android/JVM/AOSP tooling:** the build graph is Rust-only and the release
   executable is checked for Android, Java, or JVM dynamic libraries where
   `ldd` is available.
3. **Valid VectorDrawable output:** integration tests parse generated XML and
   verify Android dimensions, viewport, path geometry, and paint attributes.
4. **File and directory conversion:** CLI integration tests cover shorthand
   file conversion, recursive directory discovery, and preserved output paths.
5. **Determinism:** unit fixtures and all 200 sampled corpus assets are converted
   twice and compared byte-for-byte.
6. **Paired Google corpora:** fixed-seed manifests pinned to Google's commit
   `0cbb08816df07faaae3dca060d4ebb10b66c214f` downloads both official SVGs and
   official Android drawables for 100 outlined Material Symbols and 100 legacy
   Two Tone Material Icons. An independent path parser, arc/curve flattener,
   and nonzero-fill scan converter compare normalized filled geometry rather
   than XML strings. Two Tone also verifies per-path fill alpha. The oracle has
   separate shorthand, curve, and filled-area self-tests.
7. **Core and AOSP-derived edge cases:** tests cover non-zero viewBox origins,
   viewBox-only dimensions, percentage-dimension rejection, matrix transforms,
   single-path clips with transformed and object-bounding-box geometry, nested
   clip intersection and scope, hard white masks with transformed and
   object-bounding-box geometry, stable rejection of unrepresentable clips,
   masks, and gradient/pattern/animation features, primitive shapes, arcs,
   relative/shorthand paths through the corpus, nested translation, scaling and
   rotation, style inheritance, local `<use>`, fills, fill opacity, even-odd
   rules, strokes, stroke opacity/width/caps/joins, and unsafe stroke transforms.
8. **No silent loss:** source preflight and normalized-tree checks reject known
   unsupported or lossy constructs before output.
9. **Actionable errors:** stable diagnostic codes, source locations, optional
   suggestions, JSON reports, and nonzero CI exit codes are tested.
10. **Rust quality gates:** formatting, Clippy with warnings denied, the locked
    complete test suite, and a release build all run in the gate.

The manual conformance workflow adds every outlined Material Symbol with an
official Android pair at the pinned commit. The same exhaustive suite gates
release builds. It is intentionally not scheduled: upstream bytes cannot change
without changing the pinned commit.

## Optional checks

These run manually and are not Cargo, CI, or packaging dependencies.

```sh
python3 tools/check-material-symbols.py                       # 100 outlined icons vs Google drawables
python3 tools/check-material-symbols.py --suite twotone-100   # 100 Two Tone icons, includes fill alpha
python3 tools/check-material-symbols.py --suite outlined-all  # exhaustive pinned outlined corpus
python3 tools/check-studio-icons.py                           # Studio Icons stress suite
python3 tools/check-runeicons.py                              # Rune Icons stress suite
python3 tools/check-illustrations.py                          # complex-illustration corpus, with visual comparison
```

The Material corpora download Apache-2.0 assets from the pinned Google commit
and compare geometry against the official Android drawables. Each suite also
runs `vdt adaptive` on every icon the way Android Studio's Image Asset wizard
uses a Material Symbol as a launcher foreground, and checks that the output is
deterministic, that the color and `adaptive-icon` resources are well formed,
and that the foreground layer is exactly the plain drawable scaled into the
66dp safe zone with every other attribute unchanged. Nothing is
vendored. The Studio Icons suite is observational only: it checks a
content-hashed public snapshot for compatibility and deterministic output but is
not a release gate, because the site publishes no immutable archive, asset
license, or official VectorDrawables. See
[research.md](research.md#studio-icons-stress-corpus) for current figures.
The Rune Icons suite is also observational: it fetches Apache-2.0 assets from a
pinned GitHub commit and checks per-style coverage floors and deterministic
output, but has no official VectorDrawables to compare against. See
[research.md](research.md#rune-icons-stress-corpus).

[`tools/android-renderer`](../tools/android-renderer) is a standalone harness
that converts its SVG fixtures with the current binary and verifies rendered
VectorDrawable pixels on API 21, API 24, and a modern Android API. Its
`compose/` module repeats the pixel checks through Jetpack Compose's own
VectorDrawable parser from API 23.

The generated local package is written below `target/release-dist/`. Publishing
or signing a release is intentionally separate because it changes external
state and is not necessary to validate the V1 implementation.
