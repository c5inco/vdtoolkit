package com.vdtoolkit.renderer.test;

import android.accessibilityservice.AccessibilityService;
import android.app.UiAutomation;
import android.content.res.Resources;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Rect;
import android.graphics.drawable.AdaptiveIconDrawable;
import android.graphics.drawable.BitmapDrawable;
import android.graphics.drawable.ColorDrawable;
import android.graphics.drawable.Drawable;
import android.graphics.drawable.VectorDrawable;
import android.os.Build;
import android.os.SystemClock;
import android.test.InstrumentationTestCase;
import android.util.DisplayMetrics;
import android.view.MotionEvent;
import android.view.accessibility.AccessibilityNodeInfo;

import java.io.File;
import java.io.FileOutputStream;
import java.io.IOException;

/**
 * Loads the icon written by {@code vdt adaptive}, checks its layers pixel by
 * pixel, and on API 26+ verifies that the device launcher composes it.
 */
public final class AdaptiveIconTest extends InstrumentationTestCase {
    private static final String PACKAGE = "com.vdtoolkit.renderer";
    /** Must match {@code android:label} in the app manifest. */
    private static final String LABEL = "vdt icon";
    private static final int LAYER = 1080; // 10 px per dp of the 108dp layer
    private static final int TOLERANCE = 2;
    private static final int LAUNCHER_TOLERANCE = 40;
    private static final int BACKGROUND = 0xFF3DDC84;
    private static final int BACKGROUND_MARK = 0xFF073042;
    private static final int SOLID_BACKGROUND = 0xFF073042;
    private static final int FOREGROUND = 0xFF000000;

    public void testAdaptiveIconLayersRenderAtSafeZone() {
        Drawable icon = drawable(mipmap("ic_launcher"));
        if (Build.VERSION.SDK_INT < 26) {
            // --legacy: both layers under a circular clip, the 72dp visible
            // area mapped onto the 44dp keyline of the 48dp icon.
            assertTrue("API <26 must select the legacy vector, got " + icon,
                    icon instanceof VectorDrawable);
            Bitmap legacy = render(icon);
            assertPixel(legacy, legacyX(54), legacyX(54), FOREGROUND); // star fill
            assertPixel(legacy, legacyX(54), legacyX(22), BACKGROUND); // above the ring, in the mask
            assertTransparent(legacy, legacyX(54), legacyX(16));        // outside the mask
            assertTransparent(legacy, 60, 60);                           // corner mark masked away
            return;
        }
        assertTrue("expected AdaptiveIconDrawable, got " + icon,
                icon instanceof AdaptiveIconDrawable);
        AdaptiveIconDrawable adaptive = (AdaptiveIconDrawable) icon;

        // stars_24px with --fit 66: scale 2.75, offset 21dp. The star fill
        // covers the center and the ring lies 22 to 27.5dp from it.
        Bitmap foreground = render(adaptive.getForeground());
        assertPixel(foreground, 540, 540, FOREGROUND); // star fill at the center
        assertPixel(foreground, 540, 290, FOREGROUND); // ring, 25dp above center
        assertTransparent(foreground, 540, 100);       // above the ring
        assertTransparent(foreground, 100, 100);       // outside the safe zone

        Bitmap background = render(adaptive.getBackground());
        assertPixel(background, 540, 540, BACKGROUND);
        assertPixel(background, 1000, 1000, BACKGROUND);
        assertPixel(background, 60, 60, BACKGROUND_MARK); // 24dp corner mark, hidden by launcher masks

        if (Build.VERSION.SDK_INT >= 33) {
            Drawable monochrome = adaptive.getMonochrome();
            assertNotNull("monochrome layer must be present on API 33+", monochrome);
            Bitmap bitmap = render(monochrome);
            assertTrue("monochrome layer must paint inside the safe zone",
                    countNear(bitmap, new Rect(210, 210, 870, 870), FOREGROUND, TOLERANCE) > 0);
            assertTransparent(bitmap, 100, 100);
        }
    }

    public void testSolidColorBackgroundResolvesToColorResource() {
        int id = mipmap("ic_launcher_solid");
        if (Build.VERSION.SDK_INT < 26) {
            try {
                drawable(id);
                fail("mipmap-anydpi-v26 must be unavailable below API 26");
            } catch (Resources.NotFoundException expected) {
                return;
            }
        }
        Drawable icon = drawable(id);
        assertTrue(icon instanceof AdaptiveIconDrawable);
        Drawable background = ((AdaptiveIconDrawable) icon).getBackground();
        assertTrue("expected ColorDrawable, got " + background, background instanceof ColorDrawable);
        assertEquals(SOLID_BACKGROUND, ((ColorDrawable) background).getColor());
        assertPixel(render(((AdaptiveIconDrawable) icon).getForeground()), 540, 540, FOREGROUND);
    }

    public void testLauncherComposesAdaptiveIcon() throws Exception {
        if (Build.VERSION.SDK_INT < 26) return;
        UiAutomation automation = getInstrumentation().getUiAutomation();
        assertTrue(automation.performGlobalAction(AccessibilityService.GLOBAL_ACTION_HOME));
        SystemClock.sleep(2500);
        save("home", automation.takeScreenshot());

        AccessibilityNodeInfo icon = findLabel(automation.getRootInActiveWindow());
        for (int attempt = 0; icon == null && attempt < 4; attempt++) {
            swipeUp(automation);
            SystemClock.sleep(2000);
            icon = findLabel(automation.getRootInActiveWindow());
        }
        Bitmap screen = automation.takeScreenshot();
        save("app_drawer", screen);
        assertNotNull("launcher does not list an app labelled " + LABEL, icon);

        Rect bounds = new Rect();
        icon.getBoundsInScreen(bounds);
        bounds.intersect(0, 0, screen.getWidth(), screen.getHeight());
        assertFalse("icon bounds are off screen: " + bounds, bounds.isEmpty());
        save("icon", Bitmap.createBitmap(screen, bounds.left, bounds.top,
                bounds.width(), bounds.height()));

        int foreground = countNear(screen, bounds, FOREGROUND, LAUNCHER_TOLERANCE);
        int background = countNear(screen, bounds, BACKGROUND, LAUNCHER_TOLERANCE);
        assertTrue("launcher shows no foreground pixels in " + bounds, foreground > 0);
        assertTrue("launcher shows no background pixels in " + bounds, background > 0);
    }

    /**
     * A mirrored gradient background needs API 24: API 26+ uses the adaptive
     * icon, API 24 and 25 the anydpi-v24 legacy vector, and API 21 to 23 the
     * lossless WebP rendered from it. Every path must show the same gradient.
     */
    @SuppressWarnings("deprecation")
    public void testGradientIconMatchesAcrossVectorAndWebp() {
        Drawable icon = drawable(mipmap("ic_launcher_gradient"));
        Bitmap bitmap;
        boolean adaptive = Build.VERSION.SDK_INT >= 26;
        if (adaptive) {
            assertTrue("expected AdaptiveIconDrawable, got " + icon,
                    icon instanceof AdaptiveIconDrawable);
            bitmap = render(((AdaptiveIconDrawable) icon).getBackground());
        } else if (Build.VERSION.SDK_INT >= 24) {
            assertTrue("API 24 and 25 must select the anydpi-v24 vector, got " + icon,
                    icon instanceof VectorDrawable);
            bitmap = render(icon);
        } else {
            assertTrue("API <24 must select the density WebP, got " + icon,
                    icon instanceof BitmapDrawable);
            bitmap = render(icon);
        }
        // Rendered WebPs are scaled by density, so allow more than the vector.
        int tolerance = icon instanceof BitmapDrawable ? 10 : 4;
        // Outside the star ring and inside the mask: both sides of the mirror
        // axis at x 54, and the axis itself.
        double[][] points = {{40, 26}, {68, 26}, {54, 23}};
        for (double[] point : points) {
            int x = adaptive ? (int) Math.round(point[0] * 10) : legacyX(point[0]);
            int y = adaptive ? (int) Math.round(point[1] * 10) : legacyX(point[1]);
            assertNear(bitmap, x, y, mirroredGradientAt(point[0]), tolerance);
        }
        if (!adaptive) {
            assertTransparent(bitmap, 60, 60); // outside the mask
        }
    }

    /** Bitmap pixel for a layer coordinate in a legacy icon rendered at LAYER px. */
    private static int legacyX(double layer) {
        return (int) Math.round((24 + (layer - 54) * 44.0 / 72.0) / 48.0 * LAYER);
    }

    /** Blue to orange every 54 units, reflected, as in gradient_background.svg. */
    private static int mirroredGradientAt(double x) {
        double t = x <= 54 ? x / 54 : 2 - x / 54;
        t = Math.max(0, Math.min(1, t));
        int red = (int) Math.round(0x12 + (0xE3 - 0x12) * t);
        int green = (int) Math.round(0x67 + (0x7A - 0x67) * t);
        int blue = (int) Math.round(0xD6 + (0x19 - 0xD6) * t);
        return Color.argb(255, red, green, blue);
    }

    private int mipmap(String name) {
        int id = resourceId(name, "mipmap");
        assertTrue("missing mipmap resource " + name, id != 0);
        return id;
    }

    private int resourceId(String name, String type) {
        return resources().getIdentifier(name, type, PACKAGE);
    }

    private Resources resources() {
        return getInstrumentation().getTargetContext().getResources();
    }

    @SuppressWarnings("deprecation")
    private Drawable drawable(int id) {
        return resources().getDrawable(id);
    }

    private static Bitmap render(Drawable drawable) {
        Bitmap bitmap = Bitmap.createBitmap(LAYER, LAYER, Bitmap.Config.ARGB_8888);
        drawable.setBounds(0, 0, LAYER, LAYER);
        drawable.draw(new Canvas(bitmap));
        return bitmap;
    }

    private static AccessibilityNodeInfo findLabel(AccessibilityNodeInfo node) {
        if (node == null) return null;
        if (matches(node.getText()) || matches(node.getContentDescription())) return node;
        for (int i = 0; i < node.getChildCount(); i++) {
            AccessibilityNodeInfo found = findLabel(node.getChild(i));
            if (found != null) return found;
        }
        return null;
    }

    private static boolean matches(CharSequence text) {
        return text != null && text.toString().trim().startsWith(LABEL);
    }

    private void swipeUp(UiAutomation automation) {
        DisplayMetrics metrics = resources().getDisplayMetrics();
        float x = metrics.widthPixels / 2f;
        float from = metrics.heightPixels * 0.85f;
        float to = metrics.heightPixels * 0.25f;
        long down = SystemClock.uptimeMillis();
        inject(automation, down, down, MotionEvent.ACTION_DOWN, x, from);
        int steps = 20;
        for (int i = 1; i <= steps; i++) {
            SystemClock.sleep(15);
            inject(automation, down, SystemClock.uptimeMillis(), MotionEvent.ACTION_MOVE,
                    x, from + (to - from) * i / steps);
        }
        inject(automation, down, SystemClock.uptimeMillis(), MotionEvent.ACTION_UP, x, to);
    }

    private static void inject(UiAutomation automation, long down, long now, int action,
            float x, float y) {
        MotionEvent event = MotionEvent.obtain(down, now, action, x, y, 0);
        event.setSource(android.view.InputDevice.SOURCE_TOUCHSCREEN);
        assertTrue("input injection failed", automation.injectInputEvent(event, true));
        event.recycle();
    }

    private void save(String name, Bitmap bitmap) throws IOException {
        File dir = new File(getInstrumentation().getTargetContext()
                .getExternalFilesDir(null), "screenshots");
        assertTrue("cannot create " + dir, dir.isDirectory() || dir.mkdirs());
        File file = new File(dir, "api" + Build.VERSION.SDK_INT + "_" + name + ".png");
        FileOutputStream stream = new FileOutputStream(file);
        try {
            assertTrue(bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream));
        } finally {
            stream.close();
        }
    }

    private static int countNear(Bitmap bitmap, Rect region, int expected, int tolerance) {
        int count = 0;
        for (int y = region.top; y < region.bottom; y++) {
            for (int x = region.left; x < region.right; x++) {
                if (near(bitmap.getPixel(x, y), expected, tolerance)) count++;
            }
        }
        return count;
    }

    private static boolean near(int actual, int expected, int tolerance) {
        return Math.abs(Color.alpha(actual) - Color.alpha(expected)) <= tolerance
                && Math.abs(Color.red(actual) - Color.red(expected)) <= tolerance
                && Math.abs(Color.green(actual) - Color.green(expected)) <= tolerance
                && Math.abs(Color.blue(actual) - Color.blue(expected)) <= tolerance;
    }

    private static void assertPixel(Bitmap bitmap, int x, int y, int expected) {
        int actual = bitmap.getPixel(x, y);
        assertTrue(String.format("pixel at %d,%d: expected %08X, actual %08X", x, y, expected, actual),
                near(actual, expected, TOLERANCE));
    }

    private static void assertNear(Bitmap bitmap, int x, int y, int expected, int tolerance) {
        int actual = bitmap.getPixel(x, y);
        assertTrue(String.format("pixel at %d,%d: expected %08X, actual %08X (tolerance %d)",
                x, y, expected, actual, tolerance), near(actual, expected, tolerance));
    }

    private static void assertTransparent(Bitmap bitmap, int x, int y) {
        int alpha = Color.alpha(bitmap.getPixel(x, y));
        assertTrue("expected transparent pixel at " + x + "," + y + ", alpha=" + alpha,
                alpha <= TOLERANCE);
    }
}
