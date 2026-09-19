package com.vdtoolkit.renderer.test;

import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.drawable.Drawable;
import android.os.Build;
import android.test.InstrumentationTestCase;

import java.io.File;
import java.io.FileOutputStream;

/**
 * Renders a strongly elliptical radial gradient twice -- as converted, and as
 * the optimized spelling that actually ships -- and compares them on Android's
 * own renderer.
 *
 * Rounding the group's scaleY to three decimals sends any ratio near 2500:1 to
 * zero, and a zero scale collapses the group. This makes that visible as
 * pixels rather than as a number in the XML.
 */
public final class EllipticalCollapseTest extends InstrumentationTestCase {
    private static final int SIZE = 240;

    public void testOptimizedSpellingKeepsTheArtwork() throws Exception {
        if (Build.VERSION.SDK_INT < 24) return;

        Bitmap converted = render("elliptical_collapse");
        Bitmap optimized = render("elliptical_collapse_optimized");
        saveComparison(converted, optimized);

        int convertedPainted = paintedPixels(converted);
        assertTrue("conversion should paint the drawable, painted=" + convertedPainted,
                convertedPainted > 0);
        assertEquals("the optimized spelling must paint what the conversion painted",
                convertedPainted, paintedPixels(optimized));

        // Pixel counts alone cannot tell a correct ellipse from a stretched
        // one: both fill the canvas. Assert the banding down the centre line,
        // which is the axis the group scale controls.
        assertBanding("converted", converted);
        assertBanding("optimized", optimized);

        // The two spellings must agree pixel for pixel, not merely both paint.
        for (int y = 0; y < SIZE; y++) {
            for (int x = 0; x < SIZE; x++) {
                assertEquals("pixel differs at " + x + "," + y,
                        converted.getPixel(x, y), optimized.getPixel(x, y));
            }
        }
    }

    /**
     * The ellipse is centred, so its short axis runs vertically: the middle of
     * the canvas is the inner band and the top and bottom edges are the outer
     * band. A group scale that is too large stretches the inner band over the
     * whole canvas and loses the edges.
     */
    private void assertBanding(String label, Bitmap bitmap) {
        int x = SIZE / 2;
        assertColor(label + " top edge", bitmap.getPixel(x, 10), 0xFFC52A54);
        assertColor(label + " centre", bitmap.getPixel(x, SIZE / 2), 0xFF159A55);
        assertColor(label + " bottom edge", bitmap.getPixel(x, SIZE - 10), 0xFFC52A54);
    }

    private void assertColor(String message, int actual, int expected) {
        assertTrue(message + ": expected #" + Integer.toHexString(expected)
                        + ", actual #" + Integer.toHexString(actual),
                Math.abs(Color.red(expected) - Color.red(actual)) <= 2
                        && Math.abs(Color.green(expected) - Color.green(actual)) <= 2
                        && Math.abs(Color.blue(expected) - Color.blue(actual)) <= 2
                        && Math.abs(Color.alpha(expected) - Color.alpha(actual)) <= 2);
    }

    private int paintedPixels(Bitmap bitmap) {
        int painted = 0;
        for (int y = 0; y < bitmap.getHeight(); y++) {
            for (int x = 0; x < bitmap.getWidth(); x++) {
                if (Color.alpha(bitmap.getPixel(x, y)) > 2) painted++;
            }
        }
        return painted;
    }

    /** Writes the two renders side by side so the difference can be seen. */
    private void saveComparison(Bitmap converted, Bitmap optimized) throws Exception {
        int gap = 16;
        int labelStrip = 40;
        Bitmap sheet = Bitmap.createBitmap(
                SIZE * 2 + gap * 3, SIZE + gap * 2 + labelStrip, Bitmap.Config.ARGB_8888);
        Canvas canvas = new Canvas(sheet);
        canvas.drawColor(0xFF202124);

        // A checkerboard makes "nothing was painted" legible instead of a
        // black square that could be mistaken for dark artwork.
        drawCheckerboard(canvas, gap, gap + labelStrip);
        drawCheckerboard(canvas, gap * 2 + SIZE, gap + labelStrip);

        canvas.drawBitmap(converted, gap, gap + labelStrip, null);
        canvas.drawBitmap(optimized, gap * 2 + SIZE, gap + labelStrip, null);

        Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        text.setColor(Color.WHITE);
        text.setTextSize(22);
        canvas.drawText("convert", gap, gap + 26, text);
        canvas.drawText("convert --optimize", gap * 2 + SIZE, gap + 26, text);

        File directory = new File(
                getInstrumentation().getTargetContext().getExternalFilesDir(null), "screenshots");
        assertTrue("could not create " + directory, directory.isDirectory() || directory.mkdirs());
        File file = new File(directory, "elliptical-collapse-api" + Build.VERSION.SDK_INT + ".png");
        FileOutputStream stream = new FileOutputStream(file);
        try {
            assertTrue(sheet.compress(Bitmap.CompressFormat.PNG, 100, stream));
        } finally {
            stream.close();
        }
    }

    private void drawCheckerboard(Canvas canvas, int left, int top) {
        Paint light = new Paint();
        light.setColor(0xFF5F6368);
        Paint dark = new Paint();
        dark.setColor(0xFF3C4043);
        int cell = 15;
        for (int y = 0; y < SIZE; y += cell) {
            for (int x = 0; x < SIZE; x += cell) {
                Paint paint = ((x / cell + y / cell) % 2 == 0) ? light : dark;
                canvas.drawRect(left + x, top + y, left + x + cell, top + y + cell, paint);
            }
        }
    }

    @SuppressWarnings("deprecation")
    private Bitmap render(String name) {
        int id = getInstrumentation().getTargetContext().getResources()
                .getIdentifier(name, "drawable", "com.vdtoolkit.renderer");
        assertTrue("missing drawable resource " + name, id != 0);
        Drawable drawable = getInstrumentation().getTargetContext().getResources().getDrawable(id);
        Bitmap bitmap = Bitmap.createBitmap(SIZE, SIZE, Bitmap.Config.ARGB_8888);
        drawable.setBounds(0, 0, SIZE, SIZE);
        drawable.draw(new Canvas(bitmap));
        return bitmap;
    }
}
