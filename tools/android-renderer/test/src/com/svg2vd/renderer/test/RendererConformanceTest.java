package com.svg2vd.renderer.test;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.drawable.Drawable;
import android.os.Build;
import android.test.InstrumentationTestCase;

public final class RendererConformanceTest extends InstrumentationTestCase {
    private static final int SIZE = 240;
    private static final int TOLERANCE = 2;

    public void testApi21NativeTransformedClipPixels() {
        Bitmap bitmap = render("transformed_clip");
        assertPixel(bitmap, 70, 90, 0xFF1267D6);   // transformed clip interior
        assertPixel(bitmap, 130, 160, 0xFF1267D6); // asymmetric far interior
        assertTransparent(bitmap, 50, 90);         // content, outside clip
        assertTransparent(bitmap, 70, 180);        // below clip
    }

    public void testNestedClipIntersectionAndScopePixels() {
        if (!requireApi24Drawable("nested_clip_scope")) return;
        Bitmap bitmap = render("nested_clip_scope");
        assertPixel(bitmap, 70, 30, 0xFFC52A54);   // both clips
        assertTransparent(bitmap, 170, 30);        // outer only, outside inner
        assertPixel(bitmap, 30, 160, 0xFF159A55);  // sibling escaped inner clip
        assertTransparent(bitmap, 220, 160);       // sibling still in outer clip
    }

    public void testHardWhiteMaskAndMaskRegionPixels() {
        if (!requireApi24Drawable("hard_white_mask_region")) return;
        Bitmap bitmap = render("hard_white_mask_region");
        assertPixel(bitmap, 50, 60, 0xFFE37A19);   // mask and region interior
        assertPixel(bitmap, 150, 140, 0xFFE37A19); // asymmetric far interior
        assertTransparent(bitmap, 170, 140);       // white shape, outside region
        assertTransparent(bitmap, 80, 160);        // white shape, below region
    }

    public void testOrdinaryEvenOddPixelsAndMinimumApiBoundary() {
        if (!requireApi24Drawable("even_odd")) return;

        Bitmap bitmap = render("even_odd");
        assertPixel(bitmap, 30, 30, 0xFF713BC1);   // outer ring
        assertTransparent(bitmap, 80, 80);         // asymmetric inner hole
        assertPixel(bitmap, 190, 170, 0xFF713BC1); // outside hole, inside outer
        assertTransparent(bitmap, 230, 120);       // outside outer path
    }

    private Bitmap render(String name) {
        return render(resourceId(name));
    }

    private int resourceId(String name) {
        int id = getInstrumentation().getTargetContext().getResources()
                .getIdentifier(name, "drawable", "com.svg2vd.renderer");
        assertTrue("missing drawable resource " + name, id != 0);
        return id;
    }

    @SuppressWarnings("deprecation")
    private boolean requireApi24Drawable(String name) {
        if (Build.VERSION.SDK_INT >= 24) return true;
        try {
            getInstrumentation().getTargetContext().getResources().getDrawable(resourceId(name));
            fail("API <24 must not select drawable-v24 resource " + name);
        } catch (android.content.res.Resources.NotFoundException expected) {
            return false;
        }
        return false;
    }

    @SuppressWarnings("deprecation")
    private Bitmap render(int id) {
        Drawable drawable = getInstrumentation().getTargetContext().getResources().getDrawable(id);
        Bitmap bitmap = Bitmap.createBitmap(SIZE, SIZE, Bitmap.Config.ARGB_8888);
        drawable.setBounds(0, 0, SIZE, SIZE);
        drawable.draw(new Canvas(bitmap));
        return bitmap;
    }

    private void assertPixel(Bitmap bitmap, int x, int y, int expected) {
        int actual = bitmap.getPixel(x, y);
        assertChannel("alpha at " + x + "," + y, Color.alpha(expected), Color.alpha(actual));
        assertChannel("red at " + x + "," + y, Color.red(expected), Color.red(actual));
        assertChannel("green at " + x + "," + y, Color.green(expected), Color.green(actual));
        assertChannel("blue at " + x + "," + y, Color.blue(expected), Color.blue(actual));
    }

    private void assertTransparent(Bitmap bitmap, int x, int y) {
        int alpha = Color.alpha(bitmap.getPixel(x, y));
        assertTrue("expected transparent pixel at " + x + "," + y + ", alpha=" + alpha,
                alpha <= TOLERANCE);
    }

    private void assertChannel(String message, int expected, int actual) {
        assertTrue(message + ": expected " + expected + ", actual " + actual,
                Math.abs(expected - actual) <= TOLERANCE);
    }
}
