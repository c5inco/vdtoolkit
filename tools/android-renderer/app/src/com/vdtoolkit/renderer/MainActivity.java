package com.vdtoolkit.renderer;

import android.app.Activity;
import android.graphics.Color;
import android.os.Bundle;
import android.view.Gravity;
import android.view.ViewGroup;
import android.webkit.WebView;
import android.widget.GridLayout;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.TextView;

/** Shows 25 source or VectorDrawable icons per screen for screenshot comparison. */
public final class MainActivity extends Activity {
    private static final int PAGE_SIZE = 25;
    private static final int CELL_DP = 70;

    @Override
    public void onCreate(Bundle state) {
        super.onCreate(state);
        String surface = getIntent().getStringExtra("surface");
        int page = getIntent().getIntExtra("page", -1);
        if (surface != null && page >= 0) {
            showPage(surface, page);
            return;
        }
        GridLayout grid = new GridLayout(this);
        grid.setColumnCount(2);
        grid.setBackgroundColor(Color.WHITE);
        addWebView(grid, "SVG source (WebView)");
        addVector(grid, "vdtoolkit", "fidelity_vdt");
        addVector(grid, "svg2vectordrawable", "fidelity_ashung");
        addVector(grid, "AOSP svg2vector", "fidelity_aosp");
        setContentView(grid);
    }

    private void showPage(String surface, int page) {
        GridLayout grid = new GridLayout(this);
        grid.setColumnCount(5);
        grid.setBackgroundColor(Color.WHITE);
        for (int offset = 0; offset < PAGE_SIZE; offset++) {
            int index = page * PAGE_SIZE + offset;
            String number = String.format("%03d", index);
            if ("source".equals(surface)) {
                addWebView(grid, number, "fidelity/" + number + ".svg");
            } else {
                addVector(grid, number, "fidelity_" + surface + "_" + number);
            }
        }
        setContentView(grid);
    }

    private void addWebView(GridLayout grid, String label) {
        addWebView(grid, label, "fidelity_source.svg");
    }

    private void addWebView(GridLayout grid, String label, String asset) {
        WebView source = new WebView(this);
        source.setBackgroundColor(Color.WHITE);
        source.loadDataWithBaseURL("file:///android_asset/", "<!doctype html><style>html,body,img{margin:0;width:100%;height:100%;object-fit:contain}</style><img src=\"" + asset + "\">", "text/html", "UTF-8", null);
        addCell(grid, label, source);
    }

    private void addVector(GridLayout grid, String label, String drawableName) {
        ImageView image = new ImageView(this);
        image.setBackgroundColor(Color.WHITE);
        int drawable = getResources().getIdentifier(drawableName, "drawable", getPackageName());
        if (drawable == 0) throw new IllegalStateException("missing drawable " + drawableName);
        image.setImageResource(drawable);
        image.setScaleType(ImageView.ScaleType.FIT_CENTER);
        addCell(grid, label, image);
    }

    private void addCell(GridLayout grid, String label, android.view.View content) {
        int pixels = (int) (CELL_DP * getResources().getDisplayMetrics().density + 0.5f);
        LinearLayout cell = new LinearLayout(this);
        cell.setOrientation(LinearLayout.VERTICAL);
        TextView title = new TextView(this);
        title.setText(label);
        title.setTextColor(Color.DKGRAY);
        title.setGravity(Gravity.CENTER);
        cell.addView(title, new LinearLayout.LayoutParams(pixels, ViewGroup.LayoutParams.WRAP_CONTENT));
        cell.addView(content, new LinearLayout.LayoutParams(pixels, pixels));
        grid.addView(cell, new ViewGroup.LayoutParams(pixels, ViewGroup.LayoutParams.WRAP_CONTENT));
    }
}
