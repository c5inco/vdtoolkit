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
5. **Determinism:** unit fixtures and all 100 corpus assets are converted twice
   and compared byte-for-byte.
6. **100 Material Symbols:** a fixed-seed manifest pinned to Google's commit
   `0cbb08816df07faaae3dca060d4ebb10b66c214f` downloads both official SVGs and
   official Android drawables. An independent path parser, curve flattener, and
   nonzero-fill scan converter compare normalized filled geometry rather than
   XML strings. The oracle has separate shorthand, curve, and filled-area
   self-tests.
7. **Core edge cases:** tests cover non-zero viewBox origins, primitive shapes,
   arcs, relative/shorthand paths through the corpus, nested translation,
   scaling and rotation, style inheritance, local `<use>`, fills, fill opacity,
   even-odd rules, strokes, stroke opacity/width/caps/joins, and unsafe stroke
   transforms.
8. **No silent loss:** source preflight and normalized-tree checks reject known
   unsupported or lossy constructs before output.
9. **Actionable errors:** stable diagnostic codes, source locations, optional
   suggestions, JSON reports, and nonzero CI exit codes are tested.
10. **Rust quality gates:** formatting, Clippy with warnings denied, the locked
    complete test suite, and a release build all run in the gate.

The generated local package is written below `target/release-dist/`. Publishing
or signing a release is intentionally separate because it changes external
state and is not necessary to validate the V1 implementation.
