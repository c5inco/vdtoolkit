package com.example.icons

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp

/**
 * TransformedClip, drawn path by path rather than as an `ImageVector`.
 *
 * Drawing the paths directly is what keeps what a vector model cannot hold: a
 * group's opacity composited as one layer, a dashed stroke, a blend mode, a
 * clip path that is not one nonzero path, and an alpha or luminance mask.
 */
@Composable
public fun rememberTransformedClip(): Painter {
    val density = LocalDensity.current
    return remember(density) { TransformedClipPainter(density) }
}

private class TransformedClipPainter(private val density: Density) : Painter() {
    override val intrinsicSize: Size = with(density) {
        Size(24.dp.toPx(), 24.dp.toPx())
    }

    override fun DrawScope.onDraw() {
        scale(size.width / 24f, size.height / 24f, pivot = Offset.Zero) {
            clipRect(0f, 0f, 24f, 24f) {
                clipPath(
                    path = Path().apply {
                        moveTo(6f, 8f)
                        lineTo(14f, 8f)
                        lineTo(14f, 17f)
                        lineTo(6f, 17f)
                        close()
                    },
                ) {
                    drawPath(
                        path = Path().apply {
                            moveTo(4f, 5f)
                            lineTo(24f, 5f)
                            lineTo(24f, 25f)
                            lineTo(4f, 25f)
                            close()
                        },
                        color = Color(0xFF1267D6),
                    )
                }
            }
        }
    }
}
