# Initial research notes

Research was performed on 2026-09-09. The behavioral baseline is AOSP
`platform/tools/base` at Android Studio mirror commit
`cab65fa60a996079b6f6d9b62025df3d98c135bc`, principally `Svg2Vector.java`,
`SvgTree.java`, and `VectorDrawableGeneratorTest.java`.

Observed AOSP behavior includes primitive-to-path conversion, affine
transforms, styles, `<defs>`/`<use>`, solid fills and strokes, clip paths,
gradients, and restricted masks. It rejects animation, filters, fonts, images,
text, patterns, scripts, and foreign objects. AOSP remains a conformance signal,
not the definition of this project's contract.

`usvg` 0.48.1 normalizes shapes, commands, CSS inheritance, units, paint
references, object-bounding-box mappings, markers, and `<use>`. It consumes the
source `viewBox` into a root transform and may drop unsupported or invisible
source constructs. Consequently, source XML is inspected before `usvg`, while
geometry and computed styles come from the normalized tree. Image resolvers are
explicitly disabled because their defaults can read local files.

The initial profile deliberately diagnoses gradients and clip paths instead of
emitting guessed Android XML. AOSP emits these as API-24-era platform resource
constructs but does not itself encode a min-SDK policy. The profile reports API
21 only for the currently emitted plain VectorDrawable subset.

Focused integration fixtures preserve the pinned AOSP baseline's high-value
boundaries without copying its implementation: viewBox-only sizing, affine
matrix normalization, percentage-dimension rejection, and stable rejection of
gradients, clip paths, masks, patterns, and animation. Google Material corpora
provide the paired SVG/VectorDrawable geometry checks. The legacy Two Tone set
is kept separate because its multiple paths and fill alpha exercise compositing
semantics absent from the outlined sample.

The direct dependencies are all available under MIT, Apache-2.0, or a dual
MIT/Apache-2.0 license. No AOSP source code or Material Symbols asset is included
in the package. The optional Material test downloads Apache-2.0 assets from an
exact pinned Google commit.
