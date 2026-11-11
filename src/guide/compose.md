# vdt guide: Compose

```sh
vdt compose icon.svg --package com.example.icons -o src/main/java/com/example/icons/Icon.kt
vdt compose icons/ --package com.example.icons -o src/main/java/com/example/icons/
vdt compose --draw icon.svg --package com.example.icons -o src/main/java/com/example/icons/Icon.kt
```

## What it writes

Kotlin source that draws the SVG, one file per SVG, named after the SVG in
PascalCase (`arrow-left.svg` becomes `ArrowLeft.kt`). Compose draws it, so the
file goes in your source tree, not in `res/`, and nothing is named like a
resource.

By default the file builds a Compose `ImageVector`, in the shape the Vector
Asset importer and the material-icons generator emit: a top-level
`public val <Name>: ImageVector` over a private backing field.

```kotlin
package com.example.icons

import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

public val Icon: ImageVector
    get() {
        if (_Icon != null) {
            return _Icon!!
        }
        _Icon = ImageVector.Builder(
            name = "Icon",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            path(fill = SolidColor(Color(0xFF3DDC84))) {
                moveTo(12f, 2f)
                lineTo(2f, 22f)
                lineTo(22f, 22f)
                close()
            }
        }.build()
        return _Icon!!
    }

private var _Icon: ImageVector? = null
```

Use the property with `Image(painter = rememberVectorPainter(Icon), ...)`. In a
Material app, `Icon(imageVector = Icon, contentDescription = null)` is the
usual call, and it tints the vector with `LocalContentColor` by default: pass
`tint = Color.Unspecified` to keep the colors the SVG had, which multicolor
artwork needs.

Arguments that would equal the `ImageVector` defaults are left out, which is
the same rule the XML writer follows; anything else is written as it is drawn,
so a reviewed file diff shows what changed in the artwork.

## Naming

The file name and the property are the same name: the SVG's file name in
PascalCase, so `arrow-left.svg` becomes `ArrowLeft.kt` containing `ArrowLeft`.
`--name` sets both for a single file, and `--package` writes the `package`
line. Without `--package` the property lands in the default package, which no
other package can import, so pass it.

`--size <dp>` works exactly as it does for `convert`: the longer side is that
size, the aspect ratio is kept, and the drawing is unchanged. An SVG with only
a viewBox larger than 200, such as a Material Symbols download, becomes 24dp
wide, the size an icon is normally drawn at.

## What differs from `convert`

Compose draws the artwork itself, so the VectorDrawable output format's rules do
not apply: a gradient, an even-odd fill, and a clip path need no API level, and
a gradient is an ordinary `Brush`. One thing in the drawn output does need one.
Compose draws a blend mode through `android.graphics.BlendMode` on API 29 and
newer and falls back to the `PorterDuff` mode that matches it below, and
Android's `PorterDuff` has no equivalent for most of the modes `mix-blend-mode`
names — every one except `screen`, `overlay`, `darken`, and `lighten`. So a
drawing that uses one of the others blends only on API 29 and newer, and paints
its group normally below that: `minimum_api` is 29 for such a drawing, and
`compose --draw` prints a `VDT004` note saying so. The `ImageVector` target
prints no note, because its output needs no level of its own: read
`minimum_api` from the library there, or `vdt explain VDT004` for what the note
means.

Everything else about the SVG is judged the same way, by the same converter:

- `--strict` and `--allow-approximate` mean what they mean for `convert`.
- `--optimize` shortens numbers exactly as `optimize` does.
- Read `vdt inspect icon.svg --format json` first, with the same options.

An icon shipped as code costs a compile and a diff; an icon shipped as a
drawable is a resource that `painterResource` loads. Convert to Kotlin when the
project wants vectors in its source tree, for theming, for `Icon`, or for
generating code from artwork; keep drawables when the icon is content that a
designer will replace without a code change.

## Drawing the paths directly

An `ImageVector` is as limited as a VectorDrawable in every way that concerns
artwork: a group carries no alpha, a path carries no dash or blend effect, a
brush is linear or circular radial without a focal point, and there are no
patterns. `--draw` is the target that lifts those, because it does not go
through a vector model at all: it writes Kotlin that calls `DrawScope`.

```kotlin
@Composable
public fun rememberIcon(): Painter {
    val density = LocalDensity.current
    return remember(density) { IconPainter(density) }
}

private class IconPainter(private val density: Density) : Painter() {
    override val intrinsicSize: Size = with(density) {
        Size(24.dp.toPx(), 24.dp.toPx())
    }

    override fun DrawScope.onDraw() {
        scale(size.width / 24f, size.height / 24f, pivot = Offset.Zero) {
            drawPath(
                path = Path().apply {
                    moveTo(12f, 2f)
                    lineTo(2f, 22f)
                    lineTo(22f, 22f)
                    close()
                },
                color = Color(0xFF3DDC84),
            )
        }
    }
}
```

The output is a `Painter` subclass behind a `@Composable` factory, which is the
shape `rememberVectorPainter` has. `Painter` is the one of the three candidates
that keeps the call site the same: `Icon(painter = …)` takes a `Painter` and
applies its `ColorFilter` to whatever painter it is given, where a
`Modifier.drawBehind` extension or a bare `DrawScope` function would leave the
caller to wire the tint through by hand. A `Painter` also carries the intrinsic
size the icon is laid out at, and the factory is what makes that size right: a
`Painter` reports its intrinsic size in pixels, and the only density a
non-composable declaration can see is none, so the `24.dp` an `ImageVector`
carries has to be converted where a `LocalDensity` exists — exactly as
`rememberVectorPainter` does it.

Use `Icon(painter = rememberIcon(), contentDescription = null)`, or
`Image(painter = rememberIcon(), ...)`. Tinting works as it does for an
`ImageVector`: `Icon` tints by default, and `tint = Color.Unspecified` keeps
the SVG's own colors.

### What it can draw that the other target cannot

- **Group opacity**, composited as one layer, so overlapping shapes darken
  together the way the SVG draws them rather than each fading on its own. A
  drawable has no group alpha at all, and folding one into each shape is what
  `convert` does when it can.
- **Dashed strokes**, with the SVG's dash array and offset, as
  `PathEffect.dashPathEffect`.
- **Blend modes**, as `mix-blend-mode` means them: the group is drawn into a
  layer and the layer composites with the mode. An SVG composites inside
  itself, so a drawing that uses one is drawn into a layer of its own first,
  and the blend reaches the artwork's own backdrop rather than whatever the
  painter happens to be drawn on.
- **Clip paths that are not one nonzero path**: an even-odd clip keeps its hole
  through its path's fill type, a clip path of several shapes is unioned
  (`Path + Path`, which is the `Path.op` call that unions them), and a
  luminance mask that subtracts a shape becomes `ClipOp.Difference`.
- **Alpha and luminance masks**, as a layer the group is composited through
  with `BlendMode.DstIn`. An alpha mask reads alpha, so an opaque black shape
  keeps what it covers; a luminance mask reads luminance, so the same shape
  hides it.

A mask every shape of which is opaque — and, for a luminance mask, black or
white — has no value between transparent and opaque, so it is exactly the
region its shapes cover, and it is written as clips rather than as a layer. The
group is isolated all the same: SVG draws a masked group into a layer of its
own, so a blend mode inside it reaches the group's own backdrop rather than the
artwork behind the group, and the drawing opens that layer wherever something
inside blends through. That clipping applies each shape's coverage as it is
drawn rather than to the group as a whole, so where two shapes of the group
overlap on a partly covered edge the drawing is slightly darker than the SVG's:
a layer per masked group would fix it and cost a layer each, which is the same
approximation every Compose `clipPath` and every VectorDrawable clip makes.
Anything else, such as a grey fade, is written as the
layer above, and a luminance mask gets a second layer that applies its content's
alpha: `BlendMode.DstIn` multiplies by the content's alpha, and the colour matrix
that moves its luminance into the alpha channel replaces that alpha rather than
scaling it. That second layer is written for every luminance mask, because even
content with no transparent paint is partly transparent along the anti-aliased
edges of its shapes.

The painter draws inside a `clipRect` of its viewport, so artwork that reaches
past the viewBox is cut off the way an SVG cuts it off, rather than painting
over what is next to the icon.

### Cost

Kotlin inlines `apply { }`, so every command lands in one method and a method
of over 64 KB does not compile; a drawing with more than a thousand commands is
therefore written as path data strings, exactly as the `ImageVector` target
does it. The file is simply less readable, and nothing about the drawing
changes. A mask whose content is drawn twice counts twice, because the budget
is a size of the method the drawing lands in.

`--draw` inlines its own blocks the same way — `clipRect`, `clipPath`, and the
`withSaveLayer` a layer or a mask draws through — so its whole drawing lands in
one method however deeply it nests. What that method costs is the bytecode the
statements compile to, which is not what they look like: a gradient brush builds
an array of boxed pairs where a `Color` is one constant, and a layer costs
several times a draw call. `compose --draw` estimates that bytecode and rejects
a drawing that would pass 64 KB with `VDT027`, so what it accepts compiles. The
estimate is an upper bound with about 1.6 times the cost of each construct on
top, and the weights that scale with the artwork — a gradient stop, a path
command, a dash interval — are the widest encoding of that item rather than a
typical one, so a drawing it accepts is between a third and two thirds of the
size that would still have compiled; `tools/android-renderer/RESULTS.md` records
the measurement.

The painter rebuilds its paths every time it draws, where a `VectorPainter`
caches a bitmap. That is the cost of drawing directly, and it is the reason to
keep an icon on the `ImageVector` target when nothing needs the extra
features. Tinting costs one more layer: `Icon` applies its `ColorFilter`
through `Painter`'s own layer, because the generated painter does not implement
`applyColorFilter` — applying a tint to the finished drawing rather than to
each path is what keeps it correct through the layers a mask or a blend mode
draws.

### What still does not convert

`--draw` lifts the artwork limits an `ImageVector` shares with a drawable. It
does not lift the ones neither can express: a radial gradient's focal point, a
pattern paint, an SVG filter, a non-uniform scale on a stroked path, and text
that is not outlined are rejected by `compose` with or without `--draw`, with
the same diagnostic codes and the same suggestions. `vdt explain <code>`
explains one.

## Report back

Tell the user which file you wrote, the name in it, and any warning you could
not fix with an option. A warning about a drawable larger than 200dp (`VDT016`)
is a warning here too: a vector that size is slow to draw, so pass `--size`.
