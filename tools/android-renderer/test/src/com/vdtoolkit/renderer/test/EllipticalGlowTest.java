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
 * A realistic elliptical gradient: a soft 2:1 glow, rotated slightly, behind a
 * rounded icon background. This is what the elliptical lowering exists for.
 *
 * The rendered drawable is saved as a PNG so it can be compared against the
 * source SVG by eye, and a few pixels are pinned so a regression cannot pass
 * on "it painted something".
 */
public final class EllipticalGlowTest extends InstrumentationTestCase {
    private static final int SIZE = 240;
    private static final int TOLERANCE = 6;

    public void testRealisticGlowRendersAsAnEllipse() throws Exception {
        if (Build.VERSION.SDK_INT < 24) return;

        Bitmap converted = render("elliptical_glow");
        Bitmap optimized = render("elliptical_glow_optimized");
        saveComparison(converted, optimized);

        for (Bitmap bitmap : new Bitmap[] {converted, optimized}) {
            // Rounded corners must be transparent.
            assertTrue("corner should be clipped by rx", Color.alpha(bitmap.getPixel(3, 3)) <= 2);

            // The gradient is centred at the middle and brightest there.
            int centre = bitmap.getPixel(120, 120);
            assertNear("centre is the warm yellow stop", centre, 0xFFFFD54F);

            // A 2:1 ellipse reaches its outer stop sooner vertically than
            // horizontally. Compare equal offsets on both axes: the vertical
            // sample must be further into the ramp than the horizontal one.
            int right = bitmap.getPixel(120 + 80, 120);
            int below = bitmap.getPixel(120, 120 + 80);
            assertTrue("vertical falloff should be faster than horizontal for a 2:1 ellipse; "
                            + "right=#" + Integer.toHexString(right)
                            + " below=#" + Integer.toHexString(below),
                    Color.red(below) < Color.red(right));
        }

        // Both spellings must agree in the interior. `optimize` rounds path
        // coordinates to three decimals, which can shift an antialiased pixel
        // on the rounded corner's arc by one coverage step. Those pixels are
        // exactly the partially transparent ones, so only fully opaque pixels
        // are compared: that is the interior by definition, and it is where a
        // wrong gradient would show.
        int compared = 0;
        for (int y = 0; y < SIZE; y += 5) {
            for (int x = 0; x < SIZE; x += 5) {
                int a = converted.getPixel(x, y);
                int b = optimized.getPixel(x, y);
                if (Color.alpha(a) != 255 || Color.alpha(b) != 255) continue;
                compared++;
                assertTrue("pixel differs at " + x + "," + y
                                + " convert=#" + Integer.toHexString(a)
                                + " optimize=#" + Integer.toHexString(b),
                        channelDistance(a, b) <= TOLERANCE);
            }
        }
        assertTrue("too few opaque pixels compared: " + compared, compared > 1500);
    }

    private void assertNear(String message, int actual, int expected) {
        assertTrue(message + ": expected #" + Integer.toHexString(expected)
                        + ", actual #" + Integer.toHexString(actual),
                channelDistance(actual, expected) <= TOLERANCE);
    }

    private int channelDistance(int a, int b) {
        return Math.max(Math.max(Math.abs(Color.red(a) - Color.red(b)),
                        Math.abs(Color.green(a) - Color.green(b))),
                Math.max(Math.abs(Color.blue(a) - Color.blue(b)),
                        Math.abs(Color.alpha(a) - Color.alpha(b))));
    }

    private void saveComparison(Bitmap converted, Bitmap optimized) throws Exception {
        int gap = 16;
        int labelStrip = 40;
        Bitmap sheet = Bitmap.createBitmap(
                SIZE * 2 + gap * 3, SIZE + gap * 2 + labelStrip, Bitmap.Config.ARGB_8888);
        Canvas canvas = new Canvas(sheet);
        canvas.drawColor(0xFF202124);
        canvas.drawBitmap(converted, gap, gap + labelStrip, null);
        canvas.drawBitmap(optimized, gap * 2 + SIZE, gap + labelStrip, null);

        Paint text = new Paint(Paint.ANTI_ALIAS_FLAG);
        text.setColor(Color.WHITE);
        text.setTextSize(22);
        canvas.drawText("convert", gap, gap + 26, text);
        canvas.drawText("optimize", gap * 2 + SIZE, gap + 26, text);

        File directory = new File(
                getInstrumentation().getTargetContext().getExternalFilesDir(null), "screenshots");
        assertTrue("could not create " + directory, directory.isDirectory() || directory.mkdirs());
        File file = new File(directory, "elliptical-glow-api" + Build.VERSION.SDK_INT + ".png");
        FileOutputStream stream = new FileOutputStream(file);
        try {
            assertTrue(sheet.compress(Bitmap.CompressFormat.PNG, 100, stream));
        } finally {
            stream.close();
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
