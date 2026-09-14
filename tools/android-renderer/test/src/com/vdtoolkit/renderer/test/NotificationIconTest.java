package com.vdtoolkit.renderer.test;

import android.accessibilityservice.AccessibilityService;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.UiAutomation;
import android.content.Context;
import android.content.res.Resources;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.VectorDrawable;
import android.os.Build;
import android.os.ParcelFileDescriptor;
import android.os.SystemClock;
import android.test.InstrumentationTestCase;
import android.view.accessibility.AccessibilityNodeInfo;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;
import java.io.InputStream;

/**
 * Loads the drawables written by {@code vdt notification} and checks the one
 * claim the command rests on: the icon carries its shape in the alpha channel
 * alone, is pure white everywhere it paints, and takes the system tint.
 */
public final class NotificationIconTest extends InstrumentationTestCase {
    private static final String PACKAGE = "com.vdtoolkit.renderer";
    /** 10 px per dp of the 24dp icon. */
    private static final int ICON = 240;
    private static final int TOLERANCE = 2;
    private static final int WHITE = 0xFFFFFFFF;
    /** Half of 255 lands on 127 or 128 depending on the platform's rounding. */
    private static final int HALF_WHITE = 0x80FFFFFF;
    private static final int TINT = 0xFFE37A19;
    private static final String CHANNEL = "vdtoolkit";
    private static final String TITLE = "vdt notification icon";

    /**
     * bell.svg is a #3DDC84 square at 4..12dp and a half-opaque #073042 square
     * at 12..20dp. Both colors must be gone, both shapes and the opacity kept.
     */
    public void testNotificationIconIsAWhiteSilhouette() {
        Drawable icon = drawable("ic_stat_bell");
        assertTrue("a notification icon must stay a vector at every API, got " + icon,
                icon instanceof VectorDrawable);
        Bitmap bitmap = render(icon);

        assertPixel(bitmap, 80, 80, WHITE);            // 8dp: opaque square
        assertPixel(bitmap, 160, 160, HALF_WHITE);     // 16dp: opacity survives
        assertTransparent(bitmap, 20, 220);            // 2,22dp: never painted
        assertTransparent(bitmap, 220, 20);
        assertAllPaintedPixelsAreWhite(bitmap);
    }

    /**
     * The system tints a small icon, so color in the source would fight the
     * tint. Every painted pixel must take the tint at its own opacity.
     */
    public void testTintReplacesColorAndKeepsAlpha() {
        Drawable icon = drawable("ic_stat_bell");
        icon.setTint(TINT);
        Bitmap bitmap = render(icon);

        assertPixel(bitmap, 80, 80, TINT);
        assertPixel(bitmap, 160, 160, (0x80 << 24) | (TINT & 0x00FFFFFF));
        assertTransparent(bitmap, 20, 220);
    }

    /**
     * fade.svg fades from opaque to transparent across 4..20dp. The ramp is
     * the shape of the icon, so the gradient is kept with white stops, which
     * needs API 24: below that the drawable-v24 resource must be unavailable.
     */
    public void testFadingGradientKeepsItsAlphaRamp() {
        if (Build.VERSION.SDK_INT < 24) {
            try {
                drawable("ic_stat_fade");
                fail("drawable-v24 must be unavailable below API 24");
            } catch (Resources.NotFoundException expected) {
                return;
            }
        }
        Bitmap bitmap = render(drawable("ic_stat_fade"));

        // Alpha falls from 255 at 4dp to 0 at 20dp. Gradient interpolation
        // varies between platforms, so the ramp is checked by range and by
        // being monotonic, not pixel for pixel.
        int left = Color.alpha(bitmap.getPixel(45, 120));    // 4.5dp
        int middle = Color.alpha(bitmap.getPixel(120, 120)); // 12dp
        int right = Color.alpha(bitmap.getPixel(195, 120));  // 19.5dp
        assertTrue("left of the ramp should be nearly opaque, was " + left, left > 200);
        assertTrue("middle of the ramp should be near half, was " + middle,
                middle > 96 && middle < 160);
        assertTrue("right of the ramp should be nearly clear, was " + right, right < 56);
        assertTrue("the ramp must fall left to right: " + left + ", " + middle + ", " + right,
                left > middle && middle > right);
        assertAllPaintedPixelsAreWhite(bitmap);
    }

    /**
     * End to end: post a notification that uses the generated icon and confirm
     * the shade shows it. The status bar icon itself is a few dp on a themed
     * background, so this asserts presence and saves screenshots rather than
     * checking pixels; the pixel truth is in the tests above.
     */
    public void testShadeShowsANotificationUsingTheIcon() throws Exception {
        Context context = getInstrumentation().getTargetContext();
        NotificationManager manager =
                (NotificationManager) context.getSystemService(Context.NOTIFICATION_SERVICE);
        UiAutomation automation = getInstrumentation().getUiAutomation();
        if (Build.VERSION.SDK_INT >= 33) {
            // Posting needs a runtime permission from API 33.
            shell(automation, "pm grant " + PACKAGE + " android.permission.POST_NOTIFICATIONS");
        }
        if (Build.VERSION.SDK_INT >= 26) {
            manager.createNotificationChannel(new NotificationChannel(
                    CHANNEL, "vdtoolkit", NotificationManager.IMPORTANCE_DEFAULT));
        }
        try {
            manager.notify(1, build(context));
            SystemClock.sleep(1500);
            assertTrue(automation.performGlobalAction(
                    AccessibilityService.GLOBAL_ACTION_NOTIFICATIONS));
            SystemClock.sleep(2500);
            save("shade", automation.takeScreenshot());
            assertNotNull("the shade shows no notification titled " + TITLE,
                    findText(automation.getRootInActiveWindow()));
        } finally {
            manager.cancel(1);
            automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_BACK);
        }
    }

    @SuppressWarnings("deprecation")
    private Notification build(Context context) {
        Notification.Builder builder = Build.VERSION.SDK_INT >= 26
                ? new Notification.Builder(context, CHANNEL)
                : new Notification.Builder(context);
        return builder
                .setSmallIcon(resourceId("ic_stat_bell"))
                .setContentTitle(TITLE)
                .setContentText("generated by vdt notification")
                .build();
    }

    private static void shell(UiAutomation automation, String command) throws IOException {
        ParcelFileDescriptor descriptor = automation.executeShellCommand(command);
        // The command runs only while its output is drained.
        InputStream stream = new ParcelFileDescriptor.AutoCloseInputStream(descriptor);
        try {
            byte[] buffer = new byte[256];
            while (stream.read(buffer) >= 0) {
                // discard
            }
        } finally {
            stream.close();
        }
    }

    private int resourceId(String name) {
        int id = resources().getIdentifier(name, "drawable", PACKAGE);
        assertTrue("missing drawable resource " + name, id != 0);
        return id;
    }

    private Resources resources() {
        return getInstrumentation().getTargetContext().getResources();
    }

    @SuppressWarnings("deprecation")
    private Drawable drawable(String name) {
        return resources().getDrawable(resourceId(name));
    }

    private static Bitmap render(Drawable drawable) {
        Bitmap bitmap = Bitmap.createBitmap(ICON, ICON, Bitmap.Config.ARGB_8888);
        drawable.setBounds(0, 0, ICON, ICON);
        drawable.draw(new Canvas(bitmap));
        return bitmap;
    }

    /** Any color left in the artwork would survive the tint as a stain. */
    private static void assertAllPaintedPixelsAreWhite(Bitmap bitmap) {
        for (int y = 0; y < bitmap.getHeight(); y++) {
            for (int x = 0; x < bitmap.getWidth(); x++) {
                int pixel = bitmap.getPixel(x, y);
                if (Color.alpha(pixel) <= TOLERANCE) continue;
                assertTrue(String.format("painted pixel at %d,%d is not white: %08X",
                                x, y, pixel),
                        Color.red(pixel) >= 255 - TOLERANCE
                                && Color.green(pixel) >= 255 - TOLERANCE
                                && Color.blue(pixel) >= 255 - TOLERANCE);
            }
        }
    }

    private static AccessibilityNodeInfo findText(AccessibilityNodeInfo node) {
        if (node == null) return null;
        if (matches(node.getText()) || matches(node.getContentDescription())) return node;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo found = findText(node.getChild(i));
            if (found != null) return found;
        }
        return null;
    }

    private static boolean matches(CharSequence text) {
        return text != null && text.toString().contains(TITLE);
    }

    private void save(String name, Bitmap bitmap) throws IOException {
        File dir = new File(getInstrumentation().getTargetContext()
                .getExternalFilesDir(null), "screenshots");
        assertTrue("cannot create " + dir, dir.isDirectory() || dir.mkdirs());
        FileOutputStream stream = new FileOutputStream(
                new File(dir, "api" + Build.VERSION.SDK_INT + "_" + name + ".png"));
        try {
            assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream));
        } finally {
            stream.close();
        }
    }

    private static boolean near(int actual, int expected) {
        return Math.abs(Color.alpha(actual) - Color.alpha(expected)) <= TOLERANCE
                && Math.abs(Color.red(actual) - Color.red(expected)) <= TOLERANCE
                && Math.abs(Color.green(actual) - Color.green(expected)) <= TOLERANCE
                && Math.abs(Color.blue(actual) - Color.blue(expected)) <= TOLERANCE;
    }

    private static void assertPixel(Bitmap bitmap, int x, int y, int expected) {
        int actual = bitmap.getPixel(x, y);
        assertTrue(String.format("pixel at %d,%d: expected %08X, actual %08X",
                x, y, expected, actual), near(actual, expected));
    }

    private static void assertTransparent(Bitmap bitmap, int x, int y) {
        int alpha = Color.alpha(bitmap.getPixel(x, y));
        assertTrue("expected transparent pixel at " + x + "," + y + ", alpha=" + alpha,
                alpha <= TOLERANCE);
    }
}
