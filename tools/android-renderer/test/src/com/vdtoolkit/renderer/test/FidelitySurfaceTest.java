package com.vdtoolkit.renderer.test;

import android.content.Intent;
import android.graphics.Bitmap;
import android.test.InstrumentationTestCase;

import java.io.File;
import java.io.FileOutputStream;

/** Captures the independent SVG and native VectorDrawable surfaces on-device. */
public final class FidelitySurfaceTest extends InstrumentationTestCase {
    public void testCaptureFidelitySurfaces() throws Exception {
        Intent intent = new Intent();
        intent.setClassName("com.vdtoolkit.renderer", "com.vdtoolkit.renderer.MainActivity");
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        getInstrumentation().startActivitySync(intent);
        getInstrumentation().waitForIdleSync();
        Thread.sleep(1_000);
        Bitmap screen = getInstrumentation().getUiAutomation().takeScreenshot();
        assertNotNull("could not capture Android display", screen);
        File directory = getInstrumentation().getTargetContext().getExternalFilesDir("screenshots");
        assertNotNull("external files directory unavailable", directory);
        assertTrue(directory.exists() || directory.mkdirs());
        File output = new File(directory, "api" + android.os.Build.VERSION.SDK_INT + "_fidelity_surfaces.png");
        FileOutputStream stream = new FileOutputStream(output);
        try {
            assertTrue(screen.compress(Bitmap.CompressFormat.PNG, 100, stream));
        } finally {
            stream.close();
        }
    }
}
