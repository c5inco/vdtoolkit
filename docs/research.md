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

The initial profile deliberately diagnosed gradients and clip paths instead of
emitting guessed Android XML. Renderer verification established that
`<clip-path>` and `android:pathData` are API 21 and a single transformed clip
renders correctly there. Android API 21 does not, however, reliably restore a
clip after a nested group or intersect sequential clips; the same fixtures
render as intended on API 24 and 34. Drawables containing multiple clips
therefore report API 24. Clip paths do not support `android:fillType`; that
attribute is also available for ordinary paths from API 24. The converter emits
one nonzero path per clip definition and uses nested VectorDrawable groups to
preserve clip scope and intersection on the supported boundary. It still
rejects multi-path unions, even-odd clips, nested clip definitions, and effects
inside clip definitions rather than changing their semantics.

Focused integration fixtures preserve the pinned AOSP baseline's high-value
boundaries without copying its implementation: viewBox-only sizing, affine
matrix normalization, percentage-dimension rejection, and stable rejection of
gradients, complex clips, masks, patterns, and animation. Google Material
corpora provide the paired SVG/VectorDrawable geometry checks. The legacy Two
Tone set is kept separate because its multiple paths and fill alpha exercise
compositing semantics absent from the outlined sample.

The direct dependencies are all available under MIT, Apache-2.0, or a dual
MIT/Apache-2.0 license. No AOSP source code or Material Symbols asset is included
in the package. The optional Material test downloads Apache-2.0 assets from an
exact pinned Google commit.

## Studio Icons stress corpus

_Snapshot figures below are as of 2026-09-10 and drift as the site changes._

`studio-icons.web.app` currently publishes 812 light-theme SVG icons. Its
hashed metadata module gives every icon a section, name, description, declared
size, and public SVG URL. A content-hashed observational suite downloads those
assets transiently and measures compatibility and repeated-output determinism.

The site does not identify a public source repository, explicit asset license,
or immutable artwork archive. It also provides no paired VectorDrawables.
Therefore these assets are not vendored and the suite is intentionally excluded
from CI and release gates. Snapshot drift stops with an actionable digest rather
than being interpreted as a converter regression.

The pinned snapshot currently converts 771 of 812 icons (426 exact and 345
exact with normalization), with all 771 byte-identical on repeat. Conservative
clip-path lowering increased coverage from 622 to 771; the remaining 41 assets
use masks, filters, a gradient, or other unsupported paint semantics.

Mask triage found 58 definitions across 39 icons. Fifty-six definitions contain
one opaque white shape; the other two use a white base plus black subtraction.
Six of the nominally single-shape masks use even-odd geometry, which Android
clip paths cannot encode because `<clip-path>` has no `android:fillType`.
Lowering the remaining hard nonzero masks to a mask-region clip followed by a
geometry clip raises coverage to 802 of 812 icons (426 exact and 376 exact with
normalization), all deterministic. The eight remaining mask diagnostics are the
six even-odd definitions and two subtractive definitions; no lossy mask is
accepted.

## Gradient lowering

VectorDrawable gradients are inline complex colors from API 24. Linear
gradients support any start and end point; radial gradients support only a
center and a radius. The converter maps each gradient into viewport
coordinates with the path's absolute transform combined with the gradient
transform, where `usvg` has already folded object-bounding-box units into that
transform.

A linear gradient stays linear under every affine map, but its end point is
not simply the mapped `x2, y2`. Under skew or non-uniform scale the gradient
direction is carried by the inverse transpose of the transform, which keeps
every line of constant color where the SVG renders it. A zero-length linear
gradient is painted with its last stop, as SVG specifies.

Radial gradients convert only when the combined transform is a similarity and
the focal point coincides with the center. Everything else would become an
ellipse or a focal gradient, which Android cannot draw, so it is rejected with
`SVGVD003`. The renderer harness includes a skewed linear fixture and a scaled
radial fixture whose assertions sit where a naive endpoint mapping would paint
the wrong color.

## Compose renderer divergence

Jetpack Compose parses VectorDrawable XML itself for `painterResource` and
`ImageVector.vectorResource`; it does not use the platform `VectorDrawable`.
Verified with Compose BOM 2026.01.01 on API 24, 34, and 36: gradients, the
hard white mask lowering, even-odd fills, and a single clip render identically
to the platform. Nested clip scope does not. Compose turns each `<clip-path>`
into an implicit group and closes every open clip group at any `</group>`, so
a path that follows a nested group escapes the enclosing clip. A clip followed
only by sibling paths is unaffected. The platform renderer handles the same
XML correctly on API 21 through 36, so the converter output is correct
VectorDrawable and the difference is on the Compose side.

The converter keeps emitting platform-exact structure. If Compose parity for
nested clip scope becomes a requirement, the lowering would need to re-emit
the enclosing clip after each nested group or wrap trailing siblings in their
own group, both of which change output for every affected drawable and should
be an explicit option rather than a silent change.
