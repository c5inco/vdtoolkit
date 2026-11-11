package com.vdtoolkit.renderer.compose

import android.graphics.Bitmap
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.example.icons.rememberEvenOdd
import com.example.icons.rememberFeatures
import com.example.icons.rememberTransformedClip
import java.io.File
import java.util.concurrent.atomic.AtomicReference
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import androidx.compose.ui.graphics.Canvas as GraphicsCanvas

/**
 * Renders the Kotlin `vdt compose --draw` writes, on Compose, and checks the
 * pixels the source SVG draws.
 *
 * The painter comes from its `@Composable` factory inside a composition, which
 * is the only way to get one: the painter class is private. It is then drawn
 * into an offscreen bitmap at ten pixels per viewport unit, the same scale the
 * platform and `ComposeRendererTest` harnesses use, so one viewport unit is
 * 10 px and every sample below is a whole number of pixels.
 *
 * `tests/fixtures/draw/features.svg` is a 72 x 24 viewBox in twelve 12 x 12
 * cells, so no cell can paint over another; each test below is one cell and the
 * feature only the draw target can express. The source is rendered with `resvg`
 * and the lowered model by `tests/draw.rs`, which is what says the expected
 * values are the SVG's; what this checks is that the generated Kotlin draws the
 * same thing on Compose.
 */
@RunWith(AndroidJUnit4::class)
class DrawTargetRendererTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun groupOpacityCompositesAsOneLayer() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (0,0): two red circles at (4,6) and (8,6) of radius 3, each with
        // fill-opacity 0.5, inside one group at opacity 0.5. Compositing the
        // group as one layer keeps 0.5 x 0.5 = 0x40 where one circle paints and
        // 0.5 x (0.5 + 0.5 - 0.5 x 0.5) = 0x60 where they overlap. Folding the
        // group's opacity into each shape would give 0x40 and 0x70 instead, so
        // the overlap is the proof that the layer is one.
        assertPixel(pixels, px(4f), px(6f), 0x40FF0000)
        assertPixel(pixels, px(10f), px(6f), 0x40FF0000)
        assertPixel(pixels, px(6f), px(6f), 0x60FF0000)
    }

    @Test
    fun dashedStrokeFollowsItsDashArrayAndOffset() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (1,0): `M13 6H23`, stroke width 1, dasharray "2 1", dashoffset 1,
        // so the stroke is on from x 13 to 14, off 14 to 15, on 15 to 17, ...
        assertPixel(pixels, px(16.5f), px(6f), 0xFF0000FF)
        assertTransparent(pixels, px(14.5f), px(6f))
    }

    @Test
    fun blendModeReachesTheBackdrop() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (2,0): a #3366cc backdrop with a group `mix-blend-mode:multiply`
        // holding a #ffcc00 rect. The layer must blend with the backdrop rather
        // than cover it, so the yellow multiplies the blue under it.
        assertPixel(pixels, px(28f), px(6f), 0xFF335200)
    }

    @Test
    fun evenOddClipKeepsItsHole() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (3,0): an even-odd clip path, so the inner square is a hole.
        assertPixel(pixels, px(38f), px(2f), 0xFF00FFFF)
        assertTransparent(pixels, px(42f), px(6f))
    }

    @Test
    fun clipPathOfTwoShapesIsTheirUnion() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (0,1): a clip path of two overlapping rects, (1,1,5,5) and
        // (4,4,5,5), unioned; nothing outside them paints. The three samples are
        // the overlap, the first shape alone, and the second shape alone, so a
        // union that kept only one of them would fail.
        assertPixel(pixels, px(5f), px(17f), 0xFFFF00FF)
        assertPixel(pixels, px(3f), px(15f), 0xFFFF00FF)
        assertPixel(pixels, px(8f), px(20f), 0xFFFF00FF)
        assertTransparent(pixels, px(2f), px(20f))
    }

    @Test
    fun luminanceMaskSubtractsItsBlackShape() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (1,1): a luminance mask of a white rect (1,1,10,10) and a black
        // circle at (6,6) radius 2. Luminance reads the black as zero, so the
        // circle subtracts and the rect keeps.
        assertTransparent(pixels, px(18f), px(18f))
        assertPixel(pixels, px(14f), px(14f), 0xFFFFFF00)
    }

    @Test
    fun alphaMaskReadsAlphaSoBlackKeeps() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (2,1): an alpha mask of a white rect (1,1,10,5) and a black
        // circle at (6,3) radius 1.5. Alpha reads the black as opaque, so the
        // circle keeps what it covers, and outside the mask rect nothing does.
        assertPixel(pixels, px(30f), px(15f), 0xFF00FF00)
        assertTransparent(pixels, px(30f), px(21f))
    }

    @Test
    fun luminanceMaskFadesToItsOwnLuminance() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (3,1): a luminance mask of a #808080 rect over #0000ff content.
        // Luminance reads the grey as half, so the content is half opaque.
        assertPixel(pixels, px(42f), px(18f), 0x800000FF)
    }

    @Test
    fun gradientAlphaMaskFadesRatherThanKeepingEverything() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (5,0): an alpha mask whose content is a gradient from opaque
        // white to transparent white over #ff00ff content. An alpha mask reads
        // the content's alpha, so the artwork fades out across the mask rather
        // than being kept whole: the mask rect spans x 61 to 71, so the fade is
        // 0.9 at x 62, 0.5 at 66, and 0.1 at 70.
        assertPixel(pixels, px(62f), px(6f), 0xE6FF00FF)
        assertPixel(pixels, px(66f), px(6f), 0x80FF00FF)
        assertPixel(pixels, px(70f), px(6f), 0x1AFF00FF)
        assertTransparent(pixels, px(60.5f), px(0.5f))
    }

    @Test
    fun luminanceMaskScalesByItsOwnAlpha() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (4,1): a luminance mask of a #ffffff rect at opacity 0.5 over
        // #0000ff content. Luminance reads the white as one and the mask's own
        // alpha as half, so the content is half opaque rather than kept whole.
        assertPixel(pixels, px(54f), px(18f), 0x800000FF)
    }

    @Test
    fun evenOddIsDrawnAsTheDrawableTargetDrawsIt() {
        // The same pixels `ComposeRendererTest.ordinaryEvenOddPixels` asserts
        // for the `even_odd` VectorDrawable, from the same SVG through `--draw`.
        val pixels = render(240, 240, "even_odd_draw") { rememberEvenOdd() }
        assertPixel(pixels, 30, 30, 0xFF713BC1)
        assertTransparent(pixels, 80, 80)
        assertPixel(pixels, 190, 170, 0xFF713BC1)
        assertTransparent(pixels, 230, 120)
    }

    @Test
    fun transformedClipIsDrawnAsTheDrawableTargetDrawsIt() {
        // The same pixels `ComposeRendererTest.api21NativeTransformedClipPixels`
        // asserts for the `transformed_clip` VectorDrawable, from the same SVG
        // through `--draw`.
        val pixels = render(240, 240, "transformed_clip_draw") { rememberTransformedClip() }
        assertPixel(pixels, 70, 90, 0xFF1267D6)
        assertPixel(pixels, 130, 160, 0xFF1267D6)
        assertTransparent(pixels, 50, 90)
        assertTransparent(pixels, 70, 180)
    }

    @Test
    fun maskedGroupIsolatesTheBlendInsideIt() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (4,0): a #0000ff backdrop with a group whose mask is one opaque
        // white rect, so the mask lowers to a clip, and whose content multiplies
        // a #ffff00 rect. SVG isolates a group that has a mask, so the multiply
        // reads the group's own empty backdrop and the yellow stays yellow; were
        // the group not isolated, the multiply would reach the blue behind it
        // and paint black.
        assertPixel(pixels, px(52f), px(6f), 0xFFFFFF00)
    }

    @Test
    fun luminanceMaskAppliesTheAlphaOfAGreyNestedMask() {
        val pixels = render(WIDTH, HEIGHT, "features") { rememberFeatures() }
        // Cell (5,1): a #ff0000 rect under a luminance mask whose content is a
        // white rect masked by a luminance mask of #808080. The inner mask masks
        // by half even though its content is opaque, so the outer mask is half
        // transparent as well and has to apply that alpha on top of the
        // luminance: without its second layer the red would come out opaque.
        assertPixel(pixels, px(66f), px(18f), 0x80FF0000)
        assertTransparent(pixels, px(60.5f), px(12.5f))
    }

    @Test
    fun luminanceMaskScalesItsAlphaAtAnAntiAliasedEdge() {
        // The same cell (3,1) at 10.5 pixels per viewport unit, so that the mask
        // rect's left edge at x 37 falls inside pixel 388. A luminance mask is
        // its content's luminance times its content's alpha, and there the edge
        // of the #808080 rect is half covered: 0.502 x 0.5 x 255 is 64, where a
        // colour matrix that replaced the alpha with the luminance would leave
        // the fully covered 128. The interior pixel keeps 128 either way, so it
        // is the edge that says which happened.
        val pixels =
            render(FRACTIONAL_WIDTH, FRACTIONAL_HEIGHT, "features-fractional") { rememberFeatures() }
        assertPixel(pixels, 388, 189, 0x400000FF)
        assertPixel(pixels, 389, 189, 0x800000FF)
        assertTransparent(pixels, 387, 189)
    }

    /** A viewport coordinate in pixels, at ten pixels per viewport unit. */
    private fun px(units: Float): Int = Math.round(units * PIXELS_PER_UNIT)

    /**
     * Draws the painter `content` returns into an offscreen bitmap and writes it
     * to the app's external files directory, so a failing run still leaves the
     * whole render behind to look at.
     */
    private fun render(
        width: Int,
        height: Int,
        name: String,
        content: @Composable () -> Painter,
    ): ImageBitmap {
        val captured = AtomicReference<ImageBitmap?>(null)
        compose.setContent {
            val painter = content()
            Canvas(Modifier.size(1.dp)) {
                val bitmap = ImageBitmap(width, height)
                CanvasDrawScope().draw(
                    Density(1f),
                    LayoutDirection.Ltr,
                    GraphicsCanvas(bitmap),
                    Size(width.toFloat(), height.toFloat()),
                ) {
                    with(painter) { draw(size) }
                }
                captured.set(bitmap)
            }
        }
        // A painter draws on the frame after it is composed, so wait until a
        // frame with painted content is captured.
        compose.waitUntil(timeoutMillis = 5_000) {
            captured.get()?.let { hasContent(it) } ?: false
        }
        return requireNotNull(captured.get()).also { dump(name, it) }
    }

    private fun hasContent(image: ImageBitmap): Boolean {
        val map = image.toPixelMap()
        for (y in 0 until image.height step 8) {
            for (x in 0 until image.width step 8) {
                if (map[x, y].alpha > 0f) return true
            }
        }
        return false
    }

    private fun dump(name: String, image: ImageBitmap) {
        val directory = InstrumentationRegistry.getInstrumentation()
            .targetContext.getExternalFilesDir(null) ?: return
        directory.mkdirs()
        File(directory, "$name.png").outputStream().use { stream ->
            image.asAndroidBitmap().compress(Bitmap.CompressFormat.PNG, 100, stream)
        }
    }

    private fun assertPixel(image: ImageBitmap, x: Int, y: Int, expected: Long) {
        val actual = image.toPixelMap()[x, y].toArgb()
        val e = expected.toInt()
        for (shift in intArrayOf(24, 16, 8, 0)) {
            val a = (actual shr shift) and 0xFF
            val b = (e shr shift) and 0xFF
            assertTrue(
                "pixel ($x, $y) expected ${Integer.toHexString(e)} got ${Integer.toHexString(actual)}",
                Math.abs(a - b) <= TOLERANCE,
            )
        }
    }

    private fun assertTransparent(image: ImageBitmap, x: Int, y: Int) {
        val alpha = (image.toPixelMap()[x, y].toArgb() ushr 24) and 0xFF
        assertTrue("pixel ($x, $y) expected transparent, alpha was $alpha", alpha <= TOLERANCE)
    }

    private companion object {
        const val WIDTH = 720
        const val HEIGHT = 240
        const val PIXELS_PER_UNIT = 10f
        /** 10.5 pixels per viewport unit, so shape edges land inside a pixel. */
        const val FRACTIONAL_WIDTH = 756
        const val FRACTIONAL_HEIGHT = 252
        const val TOLERANCE = 2
    }
}
