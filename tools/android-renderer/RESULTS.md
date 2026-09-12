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
