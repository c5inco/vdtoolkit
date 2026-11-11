# Renderer verification results

Verified with emulator.wtf on 2026-09-10:

| Device | API | Result |
| --- | ---: | --- |
| Pixel 2 | 21 | Passed after API qualification correction |
| Pixel 2 | 24 | 4/4 tests passed |
| Pixel 7 | 34 | 4/4 tests passed |

The initial API 21 run was intentionally diagnostic and found two framework
boundaries: a nested group clip remained active for a following sibling, and a
second sequential clip did not intersect the prior mask-region clip. The single
transformed native clip rendered correctly. API 24 and API 34 passed all pixel
assertions. The converter now conservatively reports API 24 for any drawable
containing multiple clips, while retaining API 21 for one clip.

The final matrix checks 16 independently selected interior/exterior pixels on
API 24 and 34. API 21 checks four transformed-clip pixels and verifies that the
three API-24 resources are not selectable. Opaque expected colors and alpha use
a per-channel tolerance of 2; transparent pixels require alpha <= 2. All sample
coordinates are at least 10 rendered pixels from a geometry edge, so the
tolerance only accommodates renderer variance rather than accepting edge
antialiasing ambiguity.

Gradient fixtures added and verified on 2026-09-11. Verified with emulator.wtf,
plus a local API 36 emulator run:

| Device | API | Result |
| --- | ---: | --- |
| Pixel 2 | 21 | 6/6 tests passed |
| Pixel 2 | 24 | 6/6 tests passed |
| Pixel 7 | 34 | 6/6 tests passed |
| sdk_gphone64_arm64 emulator | 36 | 6/6 tests passed |

This run adds the skewed linear gradient and scaled radial gradient fixtures.
Both report API 24 and are emitted as inline `aapt` gradients, so on API 21 the
two gradient tests verify only that the drawable-v24 resources are not
selectable; they draw no pixels there. API 24 is the first level that renders
them, and the assertions are placed where a naive endpoint mapping of the
gradient would paint the wrong color: two samples on the skewed linear fixture
and the off-center samples on the scaled radial fixture.

## Elliptical radial gradients

Added 2026-09-19. `elliptical_glow.svg` is the realistic case the elliptical
lowering exists for: a soft 2:1 radial glow, rotated 20 degrees, with a smooth
three-stop ramp, behind a rounded icon background. It lowers to a `<group>`
with `scaleY="0.5"`, `rotation="-20"` and a centred pivot. `EllipticalGlowTest`
renders it through the platform loader, checks that the rounded corners clip,
that the centre is the warm inner stop, and that the falloff is faster on the
short axis than the long one, which is what makes it an ellipse rather than a
circle. `docs/images/elliptical-glow.png` is the device render.

It also compares the `convert` and `optimize` spellings, sampling only fully
opaque pixels. `optimize` rounds path coordinates to three decimals, which can
change antialiased coverage on the corner arc. Requiring both pixels to be
opaque compares the interior, where a wrong gradient would show, and nothing
else. The full edge scan below supersedes the initial description of these
differences as just one coverage step.

Issue [#25](https://github.com/c5inco/vdtoolkit/issues/25) separates this
rounding from short-path serialization. The fixture's suspicious relative
control `-1.133001` is exact: adding it to `21.224` in `f32` gives `20.091`.
Using `-1.133` instead misses that absolute control by one float step. The
raw-delta fallback therefore preserves precision rather than discarding it.
The `short_path` fixture regression checks every decoded control and endpoint
against the rounded absolute spelling, then renders both decoded paths at
240 × 240: their host-rendered pixels are identical. Replacing only the
original path geometry with rounded geometry, retaining the original group
and gradient attributes, changes edge alpha even with absolute spelling.
The writer's exactness contract applies **after** optimization's intentional
rounding; it does not promise pixel identity between `convert` and `optimize`.

On 2026-09-20, emulator.wtf verified the isolation on Pixel 7 / API 24, 26,
and 33. `build.sh` independently rounds the converted absolute path to three
decimals and substitutes it into the optimized XML, retaining all optimized
group and paint attributes. This control does not decode the short path, which
could copy a relative-spelling error into both sides of the comparison.
`testRoundedAbsoluteAndShortPathsMatchEveryPixel` compares all 57,600 pixels
exactly, including transparent and antialiased pixels. It also checks that
the absolute control still differs in alpha from `convert`.

| API | Glow tests | Absolute vs short differing pixels | Convert vs rounded differing alpha pixels | Maximum alpha difference |
| ---: | --- | ---: | ---: | ---: |
| 24 | 2 passed | 0 | 20 | 16 |
| 26 | 2 passed | 0 | 428 | 38 |
| 33 | 2 passed | 0 | 427 | 38 |

All runs had zero failures, errors, and skips. Thus disabling relative spelling
does not remove the edge changes on these APIs. The maximum of 38 on API 26
and 33 shows why the original sampled edge pixels did not establish a bound.
These measurements apply to this fixture at 240 × 240, not every drawable or
Android version. The spelling comparison has zero tolerance; the existing
convert-versus-optimize opaque-interior tolerance remains unchanged. No general
edge tolerance is introduced. Any future color comparison at edges should use
premultiplied color, since straight RGB is unstable near zero alpha.

The earlier multi-API shape and interior checks were:

| Device | API | Result |
| --- | ---: | --- |
| Pixel 7 | 21 | passed (verifies the v24 resource is not selectable) |
| Pixel 7 | 24 | passed |
| Pixel 7 | 26 | passed |
| Pixel 7 | 33 | passed |
| Pixel 7 | 36 | passed |

## Elliptical gradient collapse under `optimize`

Added 2026-09-19 while reviewing elliptical radial gradient support.
`elliptical_collapse.svg` is a 2500:1 ellipse, converted twice: as `convert`
emits it, and as `optimize` respells it. `EllipticalCollapseTest` renders both
through the platform loader and requires identical pixels.

The fixture pre-divides `cx` by the transform's scale so the ellipse lands on
the canvas rather than far off to the right, and uses two hard stops so the
banding is assertable. Without that compensation the transform also multiplies
`cx`, putting the gradient centre at x=30000: every visible pixel then sits
beyond the radius and clamps to one flat colour, which still reproduces the
collapse but shows nothing.

The first run reproduced the defect on Pixel 7 / API 33: the plain conversion
painted all 57600 pixels, the optimized spelling painted 0. `optimize` rounds
group attributes to three decimals, and this ellipse's `scaleY` of 0.0004
rounded to `0`, which collapses the group and erases the drawable. The failure
window is bounded: below roughly 2010:1 the value survives rounding, and above
roughly 4090:1 the existing `MINIMUM_EXTENT` guard falls back to a solid color.

Clamping `scale_y` alone is not sufficient, and is itself a defect: it stretches
the ellipse's short axis by the same factor, which is the axis all visible
banding runs along. The drawable then fills every pixel with the inner band, so
a painted-pixel assertion still passes while the geometry is wrong. The radius
is therefore derived from the clamped scale (`b / MINIMUM_GROUP_SCALE`), which
holds the short axis at exactly `b` and instead shortens the long axis, which
at these eccentricities already extends far beyond the viewport. The test pins
the banding down the centre line so that error cannot pass again.

Re-verified with emulator.wtf:

| Device | API | Result |
| --- | ---: | --- |
| Pixel 7 | 21 | 16/16 tests passed |
| Pixel 7 | 24 | 16/16 tests passed |
| Pixel 7 | 26 | 16/16 tests passed |
| Pixel 7 | 33 | 16/16 tests passed |
| Pixel 7 | 36 | 16/16 tests passed |

The test writes a side-by-side PNG to the screenshots directory, so the two
spellings can be compared directly rather than by reading `scaleY` out of XML.
Both states are kept in `docs/images/`: `elliptical-collapse-before.png` shows
the optimized panel empty, and `elliptical-collapse-fixed.png` shows the two
panels banding identically.

## Compose renderer

The `compose/` module renders the same generated drawables through Jetpack
Compose's VectorDrawable XML parser (`painterResource`) instead of the platform
`VectorDrawable`. Verified on 2026-09-11 with Compose BOM 2026.01.01:

| Device | API | Result |
| --- | ---: | --- |
| Pixel 2 | 23 | 8/8 passed (only `transformed_clip` draws; the rest verify non-selection) |
| Pixel 2 | 24 | 6/8 passed |
| Pixel 7 | 34 | 6/8 passed |
| sdk_gphone64_arm64 emulator | 36 | 6/8 passed |

Both gradient fixtures, the hard white mask, ordinary even-odd fill, and the
single transformed clip render identically in Compose and on the platform.
Compose's minimum is API 23, so API 21 is covered by the platform harness only.

The two failures are the same pixel, (220, 160), in `nested_clip_scope` and in
the diagnostic `diag_clip_then_plain_group`. Both expect the outer clip to
still apply to a path that follows a nested `<group>`. The platform renders
that correctly on every API level above; Compose does not. Compose's parser
turns each `<clip-path>` into an implicit group and closes every open clip
group at any `</group>`, so a sibling after a nested group escapes its
enclosing clip. The companion diagnostic `diag_clip_then_paths`, a clip
followed only by paths, passes, which isolates the nested group as the
trigger. The generated XML is correct VectorDrawable; this is a Compose
divergence from the platform renderer, recorded in `docs/research.md`.

Pixels are read from an offscreen bitmap drawn with `CanvasDrawScope` at
density 1. Compose's `captureToImage` was tried first and fails below API 26,
where it calls a `PixelCopy.request` overload that does not exist yet.

### Draw target

`DrawTargetRendererTest` draws the Kotlin `vdt compose --draw` writes, at the
same ten pixels per viewport unit, and checks the pixels the source SVG draws.
Verified on 2026-11-10 with Compose BOM 2026.01.01:

| Device | API | Result |
| --- | ---: | --- |
| sdk_gphone64_arm64 emulator | 36 | 15/15 passed |

The drawing is `tests/fixtures/draw/features.svg`, twelve 12x12 cells, one per
feature only this target can express: a group at opacity 0.5 over two shapes at
`fill-opacity` 0.5, whose overlap proves the group is one layer (0x60 where
folding its opacity into each shape would give 0x70); a dashed stroke, sampled
where the pattern is on and where it is off; a `mix-blend-mode:multiply` group
over a backdrop; an even-odd clip, whose hole is transparent; a clip path of two
shapes, unioned; a luminance mask that subtracts its black shape; an alpha mask,
where the same black shape keeps; a luminance mask that fades to half; a
luminance mask whose content is white at half opacity, which masks by half
rather than keeping everything (0x80 where a colour matrix that replaced the
alpha with the luminance would give 0xFF); an alpha mask whose content is a
gradient from opaque white to transparent white, which reads the content's alpha
and so fades the artwork out across the cell (0xE6 at the left, 0x80 in the
middle, 0x1A at the right, where converting a colour into alpha would keep it
whole); a group whose mask is one opaque white rect, which lowers to a clip and
is isolated all the same, so the multiply inside it reads the group's own empty
backdrop and the yellow stays yellow (black, where the multiply reached the blue
rect behind it); and a luminance mask whose content is a white rect masked by a
luminance mask of `#808080`, which masks by half twice, so the outer mask has to
apply that alpha as well as the luminance (0x80 where dropping the second layer
would keep the red opaque). One more test draws the same drawing at 10.5 pixels
per viewport unit, so that shape edges land inside a pixel: the fade cell's mask
edge keeps its coverage as well as its luminance (0x40 where dropping the alpha
layer would leave the fully covered 0x80), which is what `resvg` draws there.
Two more tests draw `even_odd` and `transformed_clip` and assert the same pixels
`ComposeRendererTest` asserts for their drawables. Rendering the same fixture
with `resvg` and compositing both over white differs only in anti-aliasing at
shape edges: 288 of 172,800 pixels, worst channel delta 31, all of them in the
group-opacity and luminance-subtract cells. The two cells added last match
`resvg` exactly, pixel for pixel, and so does the mask edge at the fractional
scale; at that scale 75 of 190,512 pixels differ by more than eight levels,
worst 29, again the edges of those two cells' circles.

### The draw target's method budget

`compose --draw` rejects a drawing whose statements would compile to more than
the JVM's 64 KB method with `VDT027`, so what it accepts compiles. It estimates
the bytecode from the lowered drawing rather than counting statements, because
the constructs differ by an order of magnitude: a `Color` is one constant, while
a gradient brush builds an array of boxed `Pair`s and a layer is an inlined
`withSaveLayer` block around everything inside it.

Each weight in `src/draw.rs` is the cost of the construct with about 1.6 times
that on top, which is the margin in the table below: the limit is 60,000
estimated bytes, so a drawing it accepts compiles to roughly 37 KB. The weights
that scale with the artwork — one gradient stop, one builder command, one dash
interval, one `Path + Path` union — are derived from the *widest* encoding of
that item rather than a typical one, because a drawing made only of them has
nothing else to average against: a stop is `aload`, `sipush` for the array
index, `ldc_w` for the offset, `Float.valueOf`, `ldc2_w`, `ColorKt.Color`,
`Color.box-impl`, `TuplesKt.to`, and `aastore`, which is 24 bytes where a
two-stop gradient measures 21; a `cubicTo` is six `ldc_w` floats and an
`invokeinterface`, 25 bytes at worst where the `lineTo` of a rect measures 13.

Measured on 2026-11-11 by compiling one probe per family with this project
(`tools/android-renderer/compose`, Compose BOM 2026.01.01, JDK 21,
`:app:compileDebugKotlin`), each probe N copies of one construct in a 240-wide
viewBox. "vdt" is the largest N `compose --draw` accepts, "compiles" an N that
compiles, and "fails" an N that does not, with the Kotlin backend reporting
`Method too large: …Painter.onDraw`.

| One construct per copy | vdt | compiles | fails |
| --- | ---: | ---: | ---: |
| solid-filled 5×5 rect | 1,250 | 2,000 | 2,500 |
| rect filled with a 2-stop linear gradient | 108 | 320 | 350 |
| rect filled with a 6-stop radial gradient | 84 | 250 | 300 |
| rect filled and stroked from a 3-stop gradient, dashed | 41 | 100 | 125 |
| group at opacity 0.5 around a rect (a layer) | 106 | 300 | 350 |
| group with a clip path around a rect | 164 | 300 | 350 |
| transformed group around a stroked rect | 625 | 1,200 | 1,500 |
| group with a luminance mask around a rect | 22 | 70 | 85 |
| rect that is both filled and stroked | 394 | 800 | 1,000 |
| rect filled with a 32-stop linear gradient | 34 | 58 | 75 |
| rect filled with a 100-stop linear gradient | 13 | 25 | 30 |
| rect filled with a 300-stop linear gradient | 4 | 8 | 9 |
| rect filled with a 1,000-stop linear gradient | 1 | 2 | 3 |

The largest drawing in the repository, `tests/fixtures/draw/features.svg`,
costs 25,152 estimated bytes, and the largest in the 102-icon corpus under
`.amp/in/corpus` costs 9,648, so no checked-in artwork is close to the limit.
The limit is a safety margin rather than a measurement of the artwork: a drawing
just under it is roughly half the size that would still have compiled.
