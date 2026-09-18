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

## Rune Icons stress corpus

[Nexvyn/runeicons](https://github.com/Nexvyn/runeicons) publishes Apache-2.0
SVG icons under `public/`. The suite sparse-checks out commit `f649e467` and
converts all 908 icons in five styles: normal (217), duotone (215), fill (126),
pixelated (215, built only from axis-aligned rectangles in roughly 40 to 42
unit viewports), and glass (135). The
per-style sprite sheets are excluded. Because the commit is pinned, the bytes
cannot drift; there are no paired VectorDrawables, so the suite measures
compatibility and repeated-output determinism only and is not a release gate.

At that commit 775 of 908 icons convert (767 exact and 8 exact with
normalization), all byte-identical on repeat. Every normal, duotone, fill, and
pixelated icon converts, and the suite fails if any of those styles loses an
icon. Only 2 of 135 glass icons convert: 132 blur their gradient layers with an
`feGaussianBlur` / `feFlood` / `feBlend` filter chain and 119 clip them with
alpha masks, neither of which VectorDrawable can express.

## Hugeicons stress corpus

[hugeicons/hugeicons](https://github.com/hugeicons/hugeicons) publishes its
free Stroke Rounded set as MIT-licensed SVGs under `icons/`. The suite
sparse-checks out commit `3e93f5d3` and converts all 6,143 icons: 24-unit
viewports of 1.5-unit round-capped strokes built from paths, circles, ellipses,
and a few rectangles. There are no paired VectorDrawables, so the suite
measures compatibility and repeated-output determinism only and is not a
release gate.

At that commit 6,137 of 6,143 icons convert (5,602 exact and 535 exact with
normalization), all byte-identical on repeat, and the suite fails if that count
drops. The 6 rejections all use dashed strokes, which VectorDrawable cannot
express. Six pairs of upstream names, such as `rubber-duck.svg` and
`rubber--duck.svg`, map to the same Android resource name, so the suite renames
its copies before converting them.

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

## Legacy launcher icons

`vdt adaptive --legacy` maps the 72dp visible circle of an adaptive icon onto
the 44dp circle keyline of a 48dp legacy icon. A composite that needs API 24,
because a layer uses gradients, even-odd fills, or its own clip, is written as
a vector in `mipmap-anydpi-v24` plus PNGs in the five density folders. The
`anydpi` qualifier outranks density qualifiers, so API 24 and 25 select the
vector while API 21 to 23, which have no `-v24` match, fall back to the PNGs; a
plain `mipmap-v24` folder would lose to the density folders. The PNGs are
rendered from the emitted vector, not the source SVG, so both share geometry,
mask, and gradients. The renderer harness verified the selection and the
gradient colors on emulator.wtf Pixel 7 at API 21, 23, 24, 25, 26, and 36.

## Adaptive icon safe zone

`SVGVD019` used to ask whether foreground content stayed inside a 66dp square
centered on the 108dp layer. A launcher mask is a circle, though: the 72dp
visible window inscribed as a circle shows 36dp from the center, while the
corners of the 66dp square sit 46.7dp out. On an API 36 emulator, a
logo drawn edge to edge at `--fit 66` passed that check and lost its
lower corners to the round mask. The check now renders the layer at 432px and
measures how far the painted pixels reach from the center: a warning past 36dp,
where clipping is certain, and a note past 33dp, the radius Android asks key
content to stay inside, each with half a dp of tolerance for antialiased edges.
Measuring pixels rather than a bounding box keeps a round logo that fills the
66dp box, which reaches only 33dp, free of findings. The same logo on the same
emulator rendered without clipping at the `--fit 54` the warning suggests.

At commit `8059d26`, the 200 sampled Material icons that `tools/accept-v1.sh`
turns into adaptive icons at `--fit 66`, 100 outlined Material Symbols and 100
legacy Two Tone icons, give 48 warnings, reaching 36.6 to 46.5dp, and 45 notes.
Material glyphs are drawn on a 24-unit grid with about 2 units of padding, so a
squarish glyph filling its 20-unit live area becomes a 55dp box at `--fit 66`,
whose corners reach 38.9dp, the most common value among the warnings. Those
icons really are clipped by a round mask; glyphs that are round or leave their
corners empty stay inside. The suite checks conversion and determinism, not
findings, so these counts are recorded here rather than gated.

## Complex-illustration corpus

_Figures below are from the pinned commits in `tests/illustrations.txt` on
2026-09-14 and do not drift._

The icon corpora say nothing about larger illustrations: many paths, nested
groups with clips and masks, gradients on fills and strokes, opacity on groups,
and assets exported from Figma or Illustrator. `tools/check-illustrations.py`
covers that with 637 files from four openly licensed sources, each pinned by
repository commit and per-file content hash and downloaded on demand:

| Source | License | Files | What it is |
| --- | --- | --- | --- |
| `realvjy/illlustrations` | MIT | 130, all | Flat scene illustrations exported from Figma |
| `themesberg/flowbite-illustrations` | MIT | 107, all | 3D-style illustrations, light and dark variants, gradient on nearly every path |
| `microsoft/fluentui-emoji` (Color) | MIT | 200 of 3145, by hash | Gradient-heavy Illustrator exports with blur filters |
| `googlefonts/noto-emoji` | OFL-1.1 | 200 of 3731, by hash | Gradients and clip paths |

The suite reports compatibility, the rejection codes and reasons, which code
combinations block each file, repeated-output determinism, and a visual
comparison: `examples/compare.rs` renders the source through resvg and the
converted drawable through the library's VectorDrawable renderer at 256px. A
file fails when more than 0.5% of pixels have a channel difference above 32, or
when the mean channel difference over painted pixels exceeds 1 / 255, which
catches a small shift in color or opacity across all the artwork.

| Source | Convertible | Byte-identical on repeat | Renders like resvg |
| --- | --- | --- | --- |
| illlustrations | 50 / 130 | 50 / 50 | 50 / 50 |
| flowbite | 86 / 107 | 86 / 86 | 86 / 86 |
| fluent | 1 / 200 | 1 / 1 | 1 / 1 |
| noto | 118 / 200 | 118 / 118 | 118 / 118 |

Nothing is approximate, and every converted file renders like its source: the
mean channel difference over painted pixels is at most 0.22 / 255 for any
single file, from anti-aliasing. The
converted flowbite set alone carries 1366 gradients across 5339 paths, so the
gradient lowering from #1 is now exercised on real Figma-exported artwork
rather than fixtures, and the 33 clip paths and 529 groups in the converted
Noto set cover the clip-scope lowering at illustration scale. The converted
illlustrations average 69 KiB of XML and 140 paths each, well past icon size,
and every flowbite file trips the `SVGVD016` large-dimensions warning at
400 to 770dp.

What blocks the rest, by the constructs that reject a file on their own:

| Construct | Code | Files it alone blocks | Where |
| --- | --- | --- | --- |
| Group opacity over overlapping children | `SVGVD013` | 69 | illlustrations (69 of 80 rejected), 1 flowbite, 1 noto |
| Elliptical or skewed radial gradients | `SVGVD003` | 95 | noto (81 of 82 rejected), 9 fluent, 5 flowbite |
| Masks that are not one opaque white shape | `SVGVD001` | 13 | flowbite (13 of 21 rejected); a further 6 illlustrations alongside filters |
| Filters | `SVGVD002` | 190 with the two above | fluent (190 of 199 rejected), 6 illlustrations |
| Text, patterns, images, dashes, `mix-blend-mode` | `SVGVD006`, `SVGVD008`, `SVGVD007`, `SVGVD013` | 2 to 11 each | scattered |

Decisions from the data:

- **Elliptical radial gradients are worth lowering next.** They are the sole
  blocker for 95 files and the whole of the Noto gap. VectorDrawable's radial
  gradient is circular, but `<group>` takes `scaleX` and `scaleY`, so a
  gradient with distinct radii can be emitted as a circular gradient inside a
  group scaled by the radii ratio, with the path geometry counter-scaled. The
  cost is a group per elliptical gradient and API 24 for the gradient itself,
  which the file already needs.
- **Group opacity is the largest single gap on flat illustrations**, 69 of
  the 80 rejected illlustrations, and it is the construct Figma exports for
  any layer with an opacity slider. It cannot be lowered exactly where
  children overlap, which is why it is rejected today. Two partial routes are
  worth measuring on this corpus before choosing: lowering when the children
  provably do not overlap, and an explicit opt-in approximation that pushes
  the opacity onto each child. Neither is implemented here.
- **Masks are the flowbite gap**, 13 files, each a soft or multi-shape mask.
  Only the one-opaque-white-shape case is exact, and these are not that.
- **Fluent Emoji is out of reach by design.** 190 of the 200 files use blur
  filters, which no VectorDrawable can draw, and 184 of those also use
  elliptical gradients and `mix-blend-mode`. It stays in the corpus as the
  ceiling: a rejected-by-filter file must never become approximate.

`tests/illustrations-convertible.txt` pins which files convert, file by file,
so a file that stops converting fails the run even when another in the same
source starts. A visual mismatch or an approximate result also fails it, so a
lowering regression on illustration-scale input fails the suite rather than
passing quietly. A file that newly converts is reported, and
`--update-convertible` records it. The suite is not a release gate: the `Material conformance` workflow
runs it on demand with the full scope. It does add `resvg` as a
dev-dependency and `examples/compare.rs`, which ordinary CI compiles.
