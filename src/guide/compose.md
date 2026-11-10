# vdt guide: Compose

```sh
vdt compose icon.svg --package com.example.icons -o src/main/java/com/example/icons/Icon.kt
vdt compose icons/ --package com.example.icons -o src/main/java/com/example/icons/
```

## What it writes

Kotlin source that builds a Compose `ImageVector`, one file per SVG, in the
shape the Vector Asset importer and the material-icons generator emit: a
top-level `val <Name>: ImageVector` over a private backing field. Compose draws
it, so the file goes in your source tree, not in `res/`, and nothing is named
like a resource.

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

An `ImageVector` needs no Android API level for a gradient, an even-odd fill,
or a clip path, so `minimum_api` and the `VDT004` note do not describe this
output and `compose` never prints them. Everything else about the SVG is
judged the same way, by the same converter:

- `--strict` and `--allow-approximate` mean what they mean for `convert`.
- `--optimize` shortens numbers exactly as `optimize` does.
- Read `vdt inspect icon.svg --format json` first, with the same options.

An icon shipped as code costs a compile and a diff; an icon shipped as a
drawable is a resource that `painterResource` loads. Both end up as the same
`ImageVector`. Convert to Kotlin when the project wants vectors in its source
tree, for theming, for `Icon`, or for generating code from artwork; keep
drawables when the icon is content that a designer will replace without a code
change.

## Cost

`--optimize` shortens the numbers in the source file, which shrinks the file
and the diffs of it. It does not shrink the app: a compiled `Float` is four
bytes whether it is written `0.299999` or `0.3`. Prefer it for a generated file
someone will read.

Kotlin inlines `path { }` and `group { }`, so every command becomes bytecode of
the one property getter, and a getter over 64 KB does not compile. A drawing
with more than a thousand commands is therefore written as path data strings
(`addPath(pathData = addPathNodes("M0 0h24v24z …"))`) instead of builder calls,
which is one call per path however long the path is. Nothing about the drawing
changes; the file is simply less readable. Compose itself needs `minSdk` 23,
which is above the API 21 a plain drawable can use and below the API 24 a
gradient or an even-odd fill forces on one.

## What does not convert

`ImageVector` is as limited as a drawable in every way that concerns artwork:
a group carries no alpha, a path carries no dash or blend effect, a brush is
linear or circular radial without a focal point, and there are no patterns.
So the SVGs `compose` rejects are the SVGs `convert` rejects, with the same
diagnostic codes and the same suggestions. `vdt explain <code>` explains one,
and nothing about the Kotlin output makes a new construct convertible.

## Report back

Tell the user which file you wrote, the property name in it, and any warning
you could not fix with an option. A warning about a drawable larger than 200dp
(`VDT016`) is a warning here too: a vector that size is slow to draw, so pass
`--size`.
