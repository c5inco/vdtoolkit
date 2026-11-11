package com.example.icons

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp

/**
 * EvenOdd, drawn path by path rather than as an `ImageVector`.
 *
 * Drawing the paths directly is what keeps what a vector model cannot hold: a
 * group's opacity composited as one layer, a dashed stroke, a blend mode, a
 * clip path that is not one nonzero path, and an alpha or luminance mask.
 */
@Composable
public fun rememberEvenOdd(): Painter {
    val density = LocalDensity.current
    return remember(density) { EvenOddPainter(density) }
}

private class EvenOddPainter(private val density: Density) : Painter() {
    override val intrinsicSize: Size = with(density) {
        Size(24.dp.toPx(), 24.dp.toPx())
    }

    override fun DrawScope.onDraw() {
        scale(size.width / 24f, size.height / 24f, pivot = Offset.Zero) {
            clipRect(0f, 0f, 24f, 24f) {
                drawPath(
                    path = Path().apply {
                        fillType = PathFillType.EvenOdd
                        moveTo(2f, 2f)
                        lineTo(22f, 2f)
                        lineTo(22f, 22f)
                        lineTo(2f, 22f)
                        close()
                        moveTo(7f, 6f)
                        lineTo(18f, 6f)
                        lineTo(18f, 16f)
                        lineTo(7f, 16f)
                        close()
                    },
                    color = Color(0xFF713BC1),
                )
            }
        }
    }
}
