# Diagnostic codes

## VDT001: unsupported mask

Error. A `<mask>` that is not a single fully opaque white shape. VectorDrawable
has clip paths but no masks: a clip is all or nothing, so a mask that fades
(alpha or gray), subtracts (black shapes), nests, or carries effects cannot be
drawn exactly. A mask of one opaque white shape is lowered to a clip path and
raises no diagnostic.

Fix in the artwork: replace the mask with outlined geometry, for example by
intersecting or subtracting the shapes in the design tool ("flatten",
"outline", or a path boolean). A soft fade has no exact VectorDrawable
equivalent; a gradient fill may come close and changes the look, so ask first.

## VDT002: unsupported filter

Error. A `<filter>`, a filter primitive (`feGaussianBlur`, `feDropShadow`,
...), or a `filter` attribute or style. VectorDrawable has no filters.

Most filters in icons are drop shadows or blurs. Remove the filter, or draw its
effect as shapes, such as a darker offset copy of the outline behind it. An
Android elevation shadow belongs in the layout, not the drawable. Removing a
shadow changes the look: tell the user.

## VDT003: unsupported gradient

Error, or a warning with `--allow-approximate`. A gradient VectorDrawable
cannot draw exactly: most often a radial gradient whose focal point (`fx`,
`fy`, `fr`) is not its centre, since VectorDrawable radial gradients are
always centered. Also a gradient transform with zero scale, or an elliptical
(skewed or unevenly scaled) radial gradient on a stroke.

For a focal point, `--allow-approximate` centers it, reports `approximate`
compatibility, and converts; the highlight moves, so tell the user. Or remove
`fx`/`fy`/`fr` in the SVG. For a transform or a stroke, fix it in the design
tool, for example by making the stroke gradient circular.

## VDT004: needs API 24

Info. The drawable uses gradients, even-odd fills, or more than one clip path,
which VectorDrawable draws only on API 24 and newer. It mirrors
`minimum_api: 24` and names the features that raised it.

Nothing to do when the module's `minSdk` is 24 or higher. When it is lower,
tell the user: the suggestion says how to stay on API 21, such as flattening
a gradient to a solid fill, converting even-odd fills to nonzero, or combining
clips. `adaptive --legacy` handles this for launcher icons by writing WebPs
for older devices.

## VDT005: external image

Error. An `<image>` that points at a file or URL. vdt never loads external
content, and VectorDrawable cannot draw raster images. Replace the image with
vector paths. For a photographic launcher background, use
`--background-image` instead of an SVG.

## VDT006: text not outlined

Error. `<text>`, `<tspan>`, or `<textPath>`. VectorDrawable has no text, and
the result would depend on fonts the device may not have.

Convert the text to outlines (paths) in the design tool: "Create Outlines" in
Illustrator, "Outline stroke / Flatten" in Figma, "Object to Path" in
Inkscape. If the text should be translatable or editable, keep it out of the
drawable and draw it with a `TextView` or `Text` composable.

## VDT007: embedded image

Error. An `<image>` with a `data:` URI, usually a PNG or JPEG pasted into the
SVG. VectorDrawable cannot draw raster images. Replace it with vector paths,
or ship the raster as its own resource. For a launcher background, pass the
image with `--background-image`.

## VDT008: unsupported pattern

Error. A `<pattern>` fill. VectorDrawable has no repeating fills. Replace the
pattern with the shapes it repeats, expanded in the design tool.

## VDT009: unsupported animation

Error. SMIL animation: `<animate>`, `<animateTransform>`, `<animateMotion>`,
`<set>`. VectorDrawable is static. Remove the animation and animate the
drawable with an `AnimatedVectorDrawable` or Compose animation instead.

## VDT010: external reference

Error. An `href` or `url(...)` that points outside the file, including CSS
`@import`. vdt never loads it. Copy the referenced content into the SVG.

## VDT011: normalization required

Info. The SVG uses constructs vdt rewrites without changing what is drawn:
basic shapes (`rect`, `circle`, ...), `transform`, opacity, `<use>`,
`<style>`, or a missing `width`/`height` with a viewBox. `compatibility` is
`exact_with_normalization` and the file converts. Only `--strict` rejects it;
do not pass `--strict` unless asked.

## VDT012: unsupported stroke transform

Error. A stroke that cannot keep its shape in VectorDrawable:
`vector-effect="non-scaling-stroke"`, or a stroke under a skew or uneven
scale, which VectorDrawable would draw at one uniform width. Convert the
stroke to a filled outline ("Outline stroke") in the design tool, or remove
the skew or uneven scale.

## VDT013: unsupported paint

Error. Paint or compositing VectorDrawable cannot reproduce. The message says
which:

- *group opacity* on a group of overlapping shapes. VectorDrawable applies
  alpha per path, so overlaps would get darker. Merge the shapes into one
  path, or move the opacity onto each shape if darker overlaps are fine.
- *dashed strokes* (`stroke-dasharray`). Convert the stroke to outlines.
- *a `miter-clip` stroke join*. Use `miter`, `round`, or `bevel`.
- *stroke drawn below the fill* (`paint-order`). Outline the stroke and place
  it under the fill.
- *blend modes, isolation, filters in styles*, or `foreignObject`, `switch`,
  `script`. Flatten or remove them in the design tool.

## VDT014: unsupported clip path

Error. A clip path VectorDrawable cannot express: several shapes unioned, a
clip nested in a clip, or an even-odd clip rule. Merge the clip into one path
with the nonzero fill rule in the design tool.

## VDT015: unsupported dimensions

Error. The root `<svg>` has percentage `width`/`height`, or is missing one of
them with no viewBox to supply both. A drawable needs a fixed size. Set
absolute `width` and `height`, or a `viewBox`. This is a mechanical edit that
does not change the drawing.

## VDT016: large dimensions

Warning. The drawable is larger than 200×200dp, Android's recommended limit
for vectors: it is redrawn from path data every frame it changes and is slow.
Use `--size` to make it smaller (the viewport is kept, so the drawing scales),
or use a raster image for large illustrations.

## VDT017: notification plate

Warning. A notification icon paints 90% or more of its canvas. Android tints
a small icon's alpha channel white, so a shape on a colored background becomes
a filled square. The icon must be the silhouette alone on transparency. This
changes the artwork: tell the user and get agreement before redrawing it.

## VDT018: empty artwork

Warning. The icon paints nothing visible, so it is invisible. Check that the
artwork has a fill or stroke and that no clip, mask, or `--fit` crop hides it.
For a raster foreground, check it was exported onto the canvas.

## VDT019: outside the safe zone

Warning or info. Adaptive foreground or monochrome content reaches too far
from the centre of the 108dp layer. Launchers mask the layer, most often to a
circle 72dp across, so distance from the centre decides what is clipped:

- warning past 36dp: a circular mask clips it.
- info past 33dp: outside the 66dp circle Android asks key content to stay in;
  a circle shows it but other mask shapes may not.

For SVG art, the suggestion names the largest `--fit` that brings the content
inside. Pass it, inspect again, and generate with it. Art drawn into the
corners of its box needs a much smaller fit than 66 (about 47), round art less
so. For a raster foreground, vdt cannot rescale it: the artwork has to be
redrawn smaller on the same canvas.

## VDT020: background gap

Warning. An adaptive background does not paint every pixel of the layer:
transparent areas, holes, clipping, letterboxing from `--background-fit
contain`, or a background image with non-opaque pixels. Uncovered areas show
the wallpaper through launcher masks and parallax. Use `--background-fit
cover` (the default), draw the background edge to edge with no transparency,
flatten the image onto an opaque color, or use `--background-color`.

## VDT021: paint flattened

Info. A notification icon's colors or gradients were made white, because
Android tints the alpha channel only. Opacity is kept. No action needed.

## VDT022: wide-gamut color

Info. CSS `color()` values in a wide-gamut space (Display P3 and others) were
resolved to the sRGB they render as. VectorDrawable has no wide-gamut color.
No action needed unless exact P3 color matters.

## VDT023: background cropped

Info. `--background-fit cover` scaled a non-square background to cover the
108dp layer, and the layer cropped the stated share of it. Repeat it to the
user. To control what is kept, draw the background on a square canvas;
`--background-fit contain` keeps all of it but leaves unpainted bands
(`VDT020`).

## VDT024: background image shape

Warning. A raster adaptive layer is not square, so Android stretches it onto
the square layer and distorts it. Crop or pad the image to a square first, so
you choose what is kept.

## VDT025: background image resolution

Warning when a raster adaptive layer is smaller than 432px: an xxxhdpi device
draws the 108dp layer at 432px, so it is upscaled and soft. Export it at
432×432px or larger. Info when it is larger than 432px: the extra pixels cost
memory every time the icon is drawn; export it at 432px.

## VDT026: foreground opaque

Warning. A raster foreground has no transparent pixels, so it hides the
background layer entirely and the icon is a flat image without parallax.
Export the foreground on a transparent canvas, or pass the flat image as
`--background-image` with a foreground that is only the mark.
