package com.example.icons

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.SolidColor
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.PathData
import androidx.compose.ui.graphics.vector.group
import androidx.compose.ui.graphics.vector.path
import androidx.compose.ui.unit.dp

public val Features: ImageVector
    get() {
        if (_Features != null) {
            return _Features!!
        }
        _Features = ImageVector.Builder(
            name = "Features",
            defaultWidth = 24.dp,
            defaultHeight = 24.dp,
            viewportWidth = 24f,
            viewportHeight = 24f,
        ).apply {
            group(
                clipPathData = PathData {
                    moveTo(4f, 2f)
                    lineTo(20f, 2f)
                    curveTo(21.10457f, 2f, 22f, 2.895431f, 22f, 4f)
                    lineTo(22f, 20f)
                    curveTo(22f, 21.10457f, 21.10457f, 22f, 20f, 22f)
                    lineTo(4f, 22f)
                    curveTo(2.895431f, 22f, 2f, 21.10457f, 2f, 20f)
                    lineTo(2f, 4f)
                    curveTo(2f, 2.895431f, 2.895431f, 2f, 4f, 2f)
                    close()
                },
            ) {
                path(
                    fill = Brush.linearGradient(
                        colorStops = arrayOf(
                            0f to Color(0xFFFF0000),
                            1f to Color(0x800000FF),
                        ),
                        start = Offset(0f, 0f),
                        end = Offset(23.999998f, 0f),
                        tileMode = TileMode.Repeated,
                    ),
                    pathFillType = PathFillType.EvenOdd,
                ) {
                    moveTo(0f, 0f)
                    lineTo(24f, 0f)
                    lineTo(24f, 24f)
                    lineTo(0f, 24f)
                    close()
                    moveTo(6f, 6f)
                    lineTo(18f, 6f)
                    lineTo(18f, 18f)
                    lineTo(6f, 18f)
                    close()
                }
                path(
                    stroke = SolidColor(Color(0xFF00FF00)),
                    strokeAlpha = 0.75f,
                    strokeLineWidth = 2.5f,
                    strokeLineCap = StrokeCap.Round,
                    strokeLineJoin = StrokeJoin.Bevel,
                    strokeLineMiter = 6f,
                ) {
                    moveTo(3f, 20f)
                    lineTo(12f, 4f)
                    lineTo(21f, 20f)
                }
                group(rotate = 20.000002f, pivotX = 9.224191f, pivotY = 9.742397f, scaleY = 0.5f) {
                    path(
                        fill = Brush.radialGradient(
                            colorStops = arrayOf(
                                0f to Color(0xFFFFD54F),
                                1f to Color(0xFF4A148C),
                            ),
                            center = Offset(9.224191f, 9.742397f),
                            radius = 10f,
                        ),
                        fillAlpha = 0.5f,
                    ) {
                        moveTo(17.303205f, 8.666336f)
                        curveTo(18.24767f, 13.856115f, 16.909733f, 19.594536f, 14.314845f, 21.483463f)
                        curveTo(11.719954f, 23.372387f, 8.850743f, 20.696518f, 7.906281f, 15.506739f)
                        curveTo(6.961818f, 10.316957f, 8.299753f, 4.578536f, 10.894644f, 2.689611f)
                        curveTo(13.489533f, 0.800685f, 16.358744f, 3.476555f, 17.303205f, 8.666336f)
                        close()
                    }
                }
            }
        }.build()
        return _Features!!
    }

private var _Features: ImageVector? = null
