package com.vdtoolkit.renderer.compose

import android.content.res.Resources
import android.os.Build
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.graphics.toPixelMap
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.util.concurrent.atomic.AtomicReference
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import androidx.compose.ui.graphics.Canvas as GraphicsCanvas

/**
 * Renders the shared fixtures through Compose's own VectorDrawable XML parser
 * and checks the same pixels as the platform harness.
 *
 * The vector painter is drawn into an offscreen 240x240 bitmap at density 1,
 * so one viewport unit is 10 px like the platform test. Drawing offscreen
 * avoids window capture, which needs PixelCopy overloads that only exist from
 * API 26, and keeps unpainted pixels transparent.
 */
@RunWith(AndroidJUnit4::class)
class ComposeRendererTest {
    @get:Rule
    val compose = createComposeRule()

    @Test
    fun api21NativeTransformedClipPixels() {
        val pixels = render("transformed_clip")
        assertPixel(pixels, 70, 90, 0xFF1267D6)
        assertPixel(pixels, 130, 160, 0xFF1267D6)
        assertTransparent(pixels, 50, 90)
        assertTransparent(pixels, 70, 180)
    }

    @Test
    fun nestedClipIntersectionAndScopePixels() {
        if (!requireApi24Drawable("nested_clip_scope")) return
        val pixels = render("nested_clip_scope")
        assertPixel(pixels, 70, 30, 0xFFC52A54)
        assertTransparent(pixels, 170, 30)
        assertPixel(pixels, 30, 160, 0xFF159A55)
        assertTransparent(pixels, 220, 160)
    }

    @Test
    fun hardWhiteMaskAndMaskRegionPixels() {
        if (!requireApi24Drawable("hard_white_mask_region")) return
        val pixels = render("hard_white_mask_region")
        assertPixel(pixels, 50, 60, 0xFFE37A19)
        assertPixel(pixels, 150, 140, 0xFFE37A19)
        assertTransparent(pixels, 170, 140)
        assertTransparent(pixels, 80, 160)
    }

    @Test
    fun ordinaryEvenOddPixels() {
        if (!requireApi24Drawable("even_odd")) return
        val pixels = render("even_odd")
        assertPixel(pixels, 30, 30, 0xFF713BC1)
        assertTransparent(pixels, 80, 80)
        assertPixel(pixels, 190, 170, 0xFF713BC1)
        assertTransparent(pixels, 230, 120)
    }

    @Test
    fun skewedLinearGradientPixels() {
        if (!requireApi24Drawable("skewed_linear_gradient")) return
        val pixels = render("skewed_linear_gradient")
        assertPixel(pixels, 40, 40, 0xFF1267D6)
        assertPixel(pixels, 220, 40, 0xFF1267D6)
        assertPixel(pixels, 20, 200, 0xFFE37A19)
        assertPixel(pixels, 200, 200, 0xFFE37A19)
    }

    @Test
    fun scaledCircularRadialGradientPixels() {
        if (!requireApi24Drawable("radial_gradient")) return
        val pixels = render("radial_gradient")
        assertPixel(pixels, 120, 120, 0xFF159A55)
        assertPixel(pixels, 180, 120, 0xFFC52A54)
        assertPixel(pixels, 120, 60, 0xFFC52A54)
        assertPixel(pixels, 20, 20, 0xFFC52A54)
    }

    // Diagnostics: isolate what makes Compose drop an outer clip-path.
    // Both expect the platform result: red clipped at x=21, green clipped at x=21.

    @Test
    fun diagnosticClipThenPlainNestedGroup() {
        if (!requireApi24Drawable("diag_clip_then_plain_group")) return
        val pixels = render("diag_clip_then_plain_group")
        assertPixel(pixels, 100, 50, 0xFFC52A54)   // red inside clip
        assertTransparent(pixels, 225, 50)         // red beyond outer clip
        assertPixel(pixels, 30, 160, 0xFF159A55)   // green inside clip
        assertTransparent(pixels, 220, 160)        // green beyond outer clip
    }

    @Test
    fun diagnosticClipThenSiblingPathsOnly() {
        if (!requireApi24Drawable("diag_clip_then_paths")) return
        val pixels = render("diag_clip_then_paths")
        assertPixel(pixels, 100, 50, 0xFFC52A54)
        assertTransparent(pixels, 225, 50)
        assertPixel(pixels, 30, 160, 0xFF159A55)
        assertTransparent(pixels, 220, 160)
    }

    private val resources: Resources
        get() = InstrumentationRegistry.getInstrumentation().targetContext.resources

    private fun resourceId(name: String): Int {
        val id = resources.getIdentifier(name, "drawable", "com.vdtoolkit.renderer.compose")
        assertTrue("missing drawable resource $name", id != 0)
        return id
    }

    @Suppress("DEPRECATION")
    private fun requireApi24Drawable(name: String): Boolean {
        if (Build.VERSION.SDK_INT >= 24) return true
        try {
            resources.getDrawable(resourceId(name))
            fail("API <24 must not select drawable-v24 resource $name")
        } catch (expected: Resources.NotFoundException) {
            return false
        }
        return false
    }

    private fun render(name: String): ImageBitmap {
        val id = resourceId(name)
        val captured = AtomicReference<ImageBitmap?>(null)
        compose.setContent {
            val painter = painterResource(id)
            Canvas(Modifier.size(1.dp)) {
                val bitmap = ImageBitmap(SIZE, SIZE)
                CanvasDrawScope().draw(
                    Density(1f),
                    LayoutDirection.Ltr,
                    GraphicsCanvas(bitmap),
                    Size(SIZE.toFloat(), SIZE.toFloat()),
                ) {
                    with(painter) { draw(size) }
                }
                captured.set(bitmap)
            }
        }
        // The vector's own composition applies after the first draw and
        // invalidates, so wait until a frame with painted content is captured.
        compose.waitUntil(timeoutMillis = 5_000) {
            captured.get()?.let { hasContent(it) } ?: false
        }
        return requireNotNull(captured.get())
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
        const val SIZE = 240
        const val TOLERANCE = 2
    }
}
