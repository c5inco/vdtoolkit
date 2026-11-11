package com.example.icons

import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.geometry.Rect
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.ClipOp
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.ColorFilter
import androidx.compose.ui.graphics.ColorMatrix
import androidx.compose.ui.graphics.Paint
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathEffect
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.clipPath
import androidx.compose.ui.graphics.drawscope.clipRect
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.withSaveLayer
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.dp

/**
 * Features, drawn path by path rather than as an `ImageVector`.
 *
 * Drawing the paths directly is what keeps what a vector model cannot hold: a
 * group's opacity composited as one layer, a dashed stroke, a blend mode, a
 * clip path that is not one nonzero path, and an alpha or luminance mask.
 */
@Composable
public fun rememberFeatures(): Painter {
    val density = LocalDensity.current
    return remember(density) { FeaturesPainter(density) }
}

private class FeaturesPainter(private val density: Density) : Painter() {
    override val intrinsicSize: Size = with(density) {
        Size(72.dp.toPx(), 24.dp.toPx())
    }

    override fun DrawScope.onDraw() {
        scale(size.width / 72f, size.height / 24f, pivot = Offset.Zero) {
            clipRect(0f, 0f, 72f, 24f) {
                drawIntoCanvas { canvas1 ->
                    canvas1.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                        drawIntoCanvas { canvas2 ->
                            canvas2.withSaveLayer(
                                Rect(Offset.Zero, Size(72f, 24f)),
                                Paint().apply {
                                    alpha = 0.5f
                                },
                            ) {
                                drawPath(
                                    path = Path().apply {
                                        moveTo(7f, 6f)
                                        cubicTo(7f, 7.656854f, 5.656854f, 9f, 4f, 9f)
                                        cubicTo(2.343146f, 9f, 1f, 7.656854f, 1f, 6f)
                                        cubicTo(1f, 4.343146f, 2.343146f, 3f, 4f, 3f)
                                        cubicTo(5.656854f, 3f, 7f, 4.343146f, 7f, 6f)
                                        close()
                                    },
                                    color = Color(0xFFFF0000),
                                    alpha = 0.5f,
                                )
                                drawPath(
                                    path = Path().apply {
                                        moveTo(11f, 6f)
                                        cubicTo(11f, 7.656854f, 9.656855f, 9f, 8f, 9f)
                                        cubicTo(6.343146f, 9f, 5f, 7.656854f, 5f, 6f)
                                        cubicTo(5f, 4.343146f, 6.343146f, 3f, 8f, 3f)
                                        cubicTo(9.656855f, 3f, 11f, 4.343146f, 11f, 6f)
                                        close()
                                    },
                                    color = Color(0xFFFF0000),
                                    alpha = 0.5f,
                                )
                            }
                        }
                        drawPath(
                            path = Path().apply {
                                moveTo(13f, 6f)
                                lineTo(23f, 6f)
                            },
                            color = Color(0xFF0000FF),
                            style = Stroke(
                                width = 1f,
                                pathEffect = PathEffect.dashPathEffect(floatArrayOf(2f, 1f), 1f),
                            ),
                        )
                        drawPath(
                            path = Path().apply {
                                moveTo(25f, 2f)
                                lineTo(35f, 2f)
                                lineTo(35f, 10f)
                                lineTo(25f, 10f)
                                close()
                            },
                            color = Color(0xFF3366CC),
                        )
                        drawIntoCanvas { canvas3 ->
                            canvas3.withSaveLayer(
                                Rect(Offset.Zero, Size(72f, 24f)),
                                Paint().apply {
                                    blendMode = BlendMode.Multiply
                                },
                            ) {
                                drawPath(
                                    path = Path().apply {
                                        moveTo(26f, 3f)
                                        lineTo(31f, 3f)
                                        lineTo(31f, 8f)
                                        lineTo(26f, 8f)
                                        close()
                                    },
                                    color = Color(0xFFFFCC00),
                                )
                            }
                        }
                        clipPath(
                            path = Path().apply {
                                fillType = PathFillType.EvenOdd
                                moveTo(37f, 1f)
                                lineTo(47f, 1f)
                                lineTo(47f, 11f)
                                lineTo(37f, 11f)
                                close()
                                moveTo(40f, 4f)
                                lineTo(44f, 4f)
                                lineTo(44f, 8f)
                                lineTo(40f, 8f)
                                close()
                            },
                        ) {
                            drawPath(
                                path = Path().apply {
                                    moveTo(36f, 0f)
                                    lineTo(48f, 0f)
                                    lineTo(48f, 12f)
                                    lineTo(36f, 12f)
                                    close()
                                },
                                color = Color(0xFF00FFFF),
                            )
                        }
                        clipPath(
                            path = Path().apply {
                                moveTo(1f, 13f)
                                lineTo(6f, 13f)
                                lineTo(6f, 18f)
                                lineTo(1f, 18f)
                                close()
                            } + Path().apply {
                                moveTo(4f, 16f)
                                lineTo(9f, 16f)
                                lineTo(9f, 21f)
                                lineTo(4f, 21f)
                                close()
                            },
                        ) {
                            drawPath(
                                path = Path().apply {
                                    moveTo(0f, 12f)
                                    lineTo(12f, 12f)
                                    lineTo(12f, 24f)
                                    lineTo(0f, 24f)
                                    close()
                                },
                                color = Color(0xFFFF00FF),
                            )
                        }
                        clipPath(
                            path = Path().apply {
                                moveTo(10.8f, 10.8f)
                                lineTo(25.2f, 10.8f)
                                lineTo(25.2f, 25.2f)
                                lineTo(10.8f, 25.2f)
                                close()
                            },
                        ) {
                            clipPath(
                                path = Path().apply {
                                    moveTo(13f, 13f)
                                    lineTo(23f, 13f)
                                    lineTo(23f, 23f)
                                    lineTo(13f, 23f)
                                    close()
                                },
                            ) {
                                clipPath(
                                    path = Path().apply {
                                        moveTo(20f, 18f)
                                        cubicTo(20f, 19.104568f, 19.104568f, 20f, 18f, 20f)
                                        cubicTo(16.895432f, 20f, 16f, 19.104568f, 16f, 18f)
                                        cubicTo(16f, 16.895432f, 16.895432f, 16f, 18f, 16f)
                                        cubicTo(19.104568f, 16f, 20f, 16.895432f, 20f, 18f)
                                        close()
                                    },
                                    clipOp = ClipOp.Difference,
                                ) {
                                    drawPath(
                                        path = Path().apply {
                                            moveTo(12f, 12f)
                                            lineTo(24f, 12f)
                                            lineTo(24f, 24f)
                                            lineTo(12f, 24f)
                                            close()
                                        },
                                        color = Color(0xFFFFFF00),
                                    )
                                }
                            }
                        }
                        clipPath(
                            path = Path().apply {
                                moveTo(22.8f, 10.8f)
                                lineTo(37.2f, 10.8f)
                                lineTo(37.2f, 25.2f)
                                lineTo(22.8f, 25.2f)
                                close()
                            },
                        ) {
                            clipPath(
                                path = Path().apply {
                                    moveTo(25f, 13f)
                                    lineTo(35f, 13f)
                                    lineTo(35f, 18f)
                                    lineTo(25f, 18f)
                                    close()
                                } + Path().apply {
                                    moveTo(31.5f, 15f)
                                    cubicTo(31.5f, 15.828427f, 30.828426f, 16.5f, 30f, 16.5f)
                                    cubicTo(29.171574f, 16.5f, 28.5f, 15.828427f, 28.5f, 15f)
                                    cubicTo(28.5f, 14.171573f, 29.171574f, 13.5f, 30f, 13.5f)
                                    cubicTo(30.828426f, 13.5f, 31.5f, 14.171573f, 31.5f, 15f)
                                    close()
                                },
                            ) {
                                drawPath(
                                    path = Path().apply {
                                        moveTo(24f, 12f)
                                        lineTo(36f, 12f)
                                        lineTo(36f, 24f)
                                        lineTo(24f, 24f)
                                        close()
                                    },
                                    color = Color(0xFF00FF00),
                                )
                            }
                        }
                        drawIntoCanvas { canvas4 ->
                            canvas4.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                                drawPath(
                                    path = Path().apply {
                                        moveTo(36f, 12f)
                                        lineTo(48f, 12f)
                                        lineTo(48f, 24f)
                                        lineTo(36f, 24f)
                                        close()
                                    },
                                    color = Color(0xFF0000FF),
                                )
                                canvas4.withSaveLayer(
                                    Rect(Offset.Zero, Size(72f, 24f)),
                                    Paint().apply {
                                        blendMode = BlendMode.DstIn
                                        colorFilter = luminanceToAlpha
                                    },
                                ) {
                                    clipPath(
                                        path = Path().apply {
                                            moveTo(34.8f, 10.8f)
                                            lineTo(49.2f, 10.8f)
                                            lineTo(49.2f, 25.2f)
                                            lineTo(34.8f, 25.2f)
                                            close()
                                        },
                                    ) {
                                        drawPath(
                                            path = Path().apply {
                                                moveTo(37f, 13f)
                                                lineTo(47f, 13f)
                                                lineTo(47f, 23f)
                                                lineTo(37f, 23f)
                                                close()
                                            },
                                            color = Color(0xFF808080),
                                        )
                                    }
                                }
                                canvas4.withSaveLayer(
                                    Rect(Offset.Zero, Size(72f, 24f)),
                                    Paint().apply {
                                        blendMode = BlendMode.DstIn
                                    },
                                ) {
                                    clipPath(
                                        path = Path().apply {
                                            moveTo(34.8f, 10.8f)
                                            lineTo(49.2f, 10.8f)
                                            lineTo(49.2f, 25.2f)
                                            lineTo(34.8f, 25.2f)
                                            close()
                                        },
                                    ) {
                                        drawPath(
                                            path = Path().apply {
                                                moveTo(37f, 13f)
                                                lineTo(47f, 13f)
                                                lineTo(47f, 23f)
                                                lineTo(37f, 23f)
                                                close()
                                            },
                                            color = Color(0xFF808080),
                                        )
                                    }
                                }
                            }
                        }
                        drawIntoCanvas { canvas5 ->
                            canvas5.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                                drawPath(
                                    path = Path().apply {
                                        moveTo(48f, 12f)
                                        lineTo(60f, 12f)
                                        lineTo(60f, 24f)
                                        lineTo(48f, 24f)
                                        close()
                                    },
                                    color = Color(0xFF0000FF),
                                )
                                canvas5.withSaveLayer(
                                    Rect(Offset.Zero, Size(72f, 24f)),
                                    Paint().apply {
                                        blendMode = BlendMode.DstIn
                                        colorFilter = luminanceToAlpha
                                    },
                                ) {
                                    clipPath(
                                        path = Path().apply {
                                            moveTo(46.8f, 10.8f)
                                            lineTo(61.2f, 10.8f)
                                            lineTo(61.2f, 25.2f)
                                            lineTo(46.8f, 25.2f)
                                            close()
                                        },
                                    ) {
                                        drawIntoCanvas { canvas6 ->
                                            canvas6.withSaveLayer(
                                                Rect(Offset.Zero, Size(72f, 24f)),
                                                Paint().apply {
                                                    alpha = 0.5f
                                                },
                                            ) {
                                                drawPath(
                                                    path = Path().apply {
                                                        moveTo(49f, 13f)
                                                        lineTo(59f, 13f)
                                                        lineTo(59f, 23f)
                                                        lineTo(49f, 23f)
                                                        close()
                                                    },
                                                    color = Color(0xFFFFFFFF),
                                                )
                                            }
                                        }
                                    }
                                }
                                canvas5.withSaveLayer(
                                    Rect(Offset.Zero, Size(72f, 24f)),
                                    Paint().apply {
                                        blendMode = BlendMode.DstIn
                                    },
                                ) {
                                    clipPath(
                                        path = Path().apply {
                                            moveTo(46.8f, 10.8f)
                                            lineTo(61.2f, 10.8f)
                                            lineTo(61.2f, 25.2f)
                                            lineTo(46.8f, 25.2f)
                                            close()
                                        },
                                    ) {
                                        drawIntoCanvas { canvas7 ->
                                            canvas7.withSaveLayer(
                                                Rect(Offset.Zero, Size(72f, 24f)),
                                                Paint().apply {
                                                    alpha = 0.5f
                                                },
                                            ) {
                                                drawPath(
                                                    path = Path().apply {
                                                        moveTo(49f, 13f)
                                                        lineTo(59f, 13f)
                                                        lineTo(59f, 23f)
                                                        lineTo(49f, 23f)
                                                        close()
                                                    },
                                                    color = Color(0xFFFFFFFF),
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        drawIntoCanvas { canvas8 ->
                            canvas8.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                                drawPath(
                                    path = Path().apply {
                                        moveTo(60f, 0f)
                                        lineTo(72f, 0f)
                                        lineTo(72f, 12f)
                                        lineTo(60f, 12f)
                                        close()
                                    },
                                    color = Color(0xFFFF00FF),
                                )
                                canvas8.withSaveLayer(
                                    Rect(Offset.Zero, Size(72f, 24f)),
                                    Paint().apply {
                                        blendMode = BlendMode.DstIn
                                    },
                                ) {
                                    clipPath(
                                        path = Path().apply {
                                            moveTo(58.8f, -1.2f)
                                            lineTo(73.2f, -1.2f)
                                            lineTo(73.2f, 13.200001f)
                                            lineTo(58.8f, 13.200001f)
                                            close()
                                        },
                                    ) {
                                        drawPath(
                                            path = Path().apply {
                                                moveTo(61f, 1f)
                                                lineTo(71f, 1f)
                                                lineTo(71f, 11f)
                                                lineTo(61f, 11f)
                                                close()
                                            },
                                            brush = Brush.linearGradient(
                                                colorStops = arrayOf(
                                                    0f to Color(0xFFFFFFFF),
                                                    1f to Color(0x00FFFFFF),
                                                ),
                                                start = Offset(61f, 1f),
                                                end = Offset(71f, 1f),
                                            ),
                                        )
                                    }
                                }
                            }
                        }
                        drawPath(
                            path = Path().apply {
                                moveTo(48f, 0f)
                                lineTo(60f, 0f)
                                lineTo(60f, 12f)
                                lineTo(48f, 12f)
                                close()
                            },
                            color = Color(0xFF0000FF),
                        )
                        clipPath(
                            path = Path().apply {
                                moveTo(46.8f, -1.2f)
                                lineTo(61.2f, -1.2f)
                                lineTo(61.2f, 13.200001f)
                                lineTo(46.8f, 13.200001f)
                                close()
                            },
                        ) {
                            clipPath(
                                path = Path().apply {
                                    moveTo(49f, 1f)
                                    lineTo(59f, 1f)
                                    lineTo(59f, 11f)
                                    lineTo(49f, 11f)
                                    close()
                                },
                            ) {
                                drawIntoCanvas { canvas9 ->
                                    canvas9.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                                        drawIntoCanvas { canvas10 ->
                                            canvas10.withSaveLayer(
                                                Rect(Offset.Zero, Size(72f, 24f)),
                                                Paint().apply {
                                                    blendMode = BlendMode.Multiply
                                                },
                                            ) {
                                                drawPath(
                                                    path = Path().apply {
                                                        moveTo(48f, 0f)
                                                        lineTo(60f, 0f)
                                                        lineTo(60f, 12f)
                                                        lineTo(48f, 12f)
                                                        close()
                                                    },
                                                    color = Color(0xFFFFFF00),
                                                )
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        drawIntoCanvas { canvas11 ->
                            canvas11.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                                drawPath(
                                    path = Path().apply {
                                        moveTo(60f, 12f)
                                        lineTo(72f, 12f)
                                        lineTo(72f, 24f)
                                        lineTo(60f, 24f)
                                        close()
                                    },
                                    color = Color(0xFFFF0000),
                                )
                                canvas11.withSaveLayer(
                                    Rect(Offset.Zero, Size(72f, 24f)),
                                    Paint().apply {
                                        blendMode = BlendMode.DstIn
                                        colorFilter = luminanceToAlpha
                                    },
                                ) {
                                    clipPath(
                                        path = Path().apply {
                                            moveTo(58.8f, 10.8f)
                                            lineTo(73.2f, 10.8f)
                                            lineTo(73.2f, 25.2f)
                                            lineTo(58.8f, 25.2f)
                                            close()
                                        },
                                    ) {
                                        drawIntoCanvas { canvas12 ->
                                            canvas12.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                                                drawPath(
                                                    path = Path().apply {
                                                        moveTo(61f, 13f)
                                                        lineTo(71f, 13f)
                                                        lineTo(71f, 23f)
                                                        lineTo(61f, 23f)
                                                        close()
                                                    },
                                                    color = Color(0xFFFFFFFF),
                                                )
                                                canvas12.withSaveLayer(
                                                    Rect(Offset.Zero, Size(72f, 24f)),
                                                    Paint().apply {
                                                        blendMode = BlendMode.DstIn
                                                        colorFilter = luminanceToAlpha
                                                    },
                                                ) {
                                                    clipPath(
                                                        path = Path().apply {
                                                            moveTo(60f, 12f)
                                                            lineTo(72f, 12f)
                                                            lineTo(72f, 24f)
                                                            lineTo(60f, 24f)
                                                            close()
                                                        },
                                                    ) {
                                                        drawPath(
                                                            path = Path().apply {
                                                                moveTo(61f, 13f)
                                                                lineTo(71f, 13f)
                                                                lineTo(71f, 23f)
                                                                lineTo(61f, 23f)
                                                                close()
                                                            },
                                                            color = Color(0xFF808080),
                                                        )
                                                    }
                                                }
                                                canvas12.withSaveLayer(
                                                    Rect(Offset.Zero, Size(72f, 24f)),
                                                    Paint().apply {
                                                        blendMode = BlendMode.DstIn
                                                    },
                                                ) {
                                                    clipPath(
                                                        path = Path().apply {
                                                            moveTo(60f, 12f)
                                                            lineTo(72f, 12f)
                                                            lineTo(72f, 24f)
                                                            lineTo(60f, 24f)
                                                            close()
                                                        },
                                                    ) {
                                                        drawPath(
                                                            path = Path().apply {
                                                                moveTo(61f, 13f)
                                                                lineTo(71f, 13f)
                                                                lineTo(71f, 23f)
                                                                lineTo(61f, 23f)
                                                                close()
                                                            },
                                                            color = Color(0xFF808080),
                                                        )
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                canvas11.withSaveLayer(
                                    Rect(Offset.Zero, Size(72f, 24f)),
                                    Paint().apply {
                                        blendMode = BlendMode.DstIn
                                    },
                                ) {
                                    clipPath(
                                        path = Path().apply {
                                            moveTo(58.8f, 10.8f)
                                            lineTo(73.2f, 10.8f)
                                            lineTo(73.2f, 25.2f)
                                            lineTo(58.8f, 25.2f)
                                            close()
                                        },
                                    ) {
                                        drawIntoCanvas { canvas13 ->
                                            canvas13.withSaveLayer(Rect(Offset.Zero, Size(72f, 24f)), Paint()) {
                                                drawPath(
                                                    path = Path().apply {
                                                        moveTo(61f, 13f)
                                                        lineTo(71f, 13f)
                                                        lineTo(71f, 23f)
                                                        lineTo(61f, 23f)
                                                        close()
                                                    },
                                                    color = Color(0xFFFFFFFF),
                                                )
                                                canvas13.withSaveLayer(
                                                    Rect(Offset.Zero, Size(72f, 24f)),
                                                    Paint().apply {
                                                        blendMode = BlendMode.DstIn
                                                        colorFilter = luminanceToAlpha
                                                    },
                                                ) {
                                                    clipPath(
                                                        path = Path().apply {
                                                            moveTo(60f, 12f)
                                                            lineTo(72f, 12f)
                                                            lineTo(72f, 24f)
                                                            lineTo(60f, 24f)
                                                            close()
                                                        },
                                                    ) {
                                                        drawPath(
                                                            path = Path().apply {
                                                                moveTo(61f, 13f)
                                                                lineTo(71f, 13f)
                                                                lineTo(71f, 23f)
                                                                lineTo(61f, 23f)
                                                                close()
                                                            },
                                                            color = Color(0xFF808080),
                                                        )
                                                    }
                                                }
                                                canvas13.withSaveLayer(
                                                    Rect(Offset.Zero, Size(72f, 24f)),
                                                    Paint().apply {
                                                        blendMode = BlendMode.DstIn
                                                    },
                                                ) {
                                                    clipPath(
                                                        path = Path().apply {
                                                            moveTo(60f, 12f)
                                                            lineTo(72f, 12f)
                                                            lineTo(72f, 24f)
                                                            lineTo(60f, 24f)
                                                            close()
                                                        },
                                                    ) {
                                                        drawPath(
                                                            path = Path().apply {
                                                                moveTo(61f, 13f)
                                                                lineTo(71f, 13f)
                                                                lineTo(71f, 23f)
                                                                lineTo(61f, 23f)
                                                                close()
                                                            },
                                                            color = Color(0xFF808080),
                                                        )
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Turns the colour of a luminance mask into the alpha that masks with it.
private val luminanceToAlpha = ColorFilter.colorMatrix(
    ColorMatrix(
        floatArrayOf(
            0f, 0f, 0f, 0f, 0f,
            0f, 0f, 0f, 0f, 0f,
            0f, 0f, 0f, 0f, 0f,
            0.2126f, 0.7152f, 0.0722f, 0f, 0f,
        ),
    ),
)
