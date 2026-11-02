# Task Report

## What I did

1. **Launcher Icon (`logo.svg`)**: 
   - Inspected `design/logo.svg` and found that the content would be clipped by a circular launcher mask (VDT019).
   - Generated the adaptive launcher icon and legacy icons using `vdt adaptive` with a `#1E88E5` background, `--monochrome` support for themed icons, and `--fit 50` to prevent clipping. 
   - Used `--legacy` to support the project's `minSdk` of 23.
   - Updated `AndroidManifest.xml` to include `android:roundIcon="@mipmap/ic_launcher_round"` so that the generated round icons are correctly applied.

2. **Notification Icon (`bell.svg`)**: 
   - Converted `design/bell.svg` using `vdt notification` into `app/src/main/res/drawable-anydpi/ic_stat_bell.xml` with `--fit 20`, accommodating edge-to-edge content.
   - Updated `NotificationHelper.kt` to replace `android.R.drawable.ic_dialog_info` with `R.drawable.ic_stat_bell`.

3. **Onboarding Screen (`hero.svg`)**:
   - Wired up the code in `OnboardingScreen.kt` using an `Image` composable to display `R.drawable.onboarding_hero`.

## What I did not do and why

- **Convert `hero.svg` into VectorDrawable XML**: 
  - I could not run the conversion for `hero.svg` because the file contains unsupported SVG features that VectorDrawables cannot represent:
    - Filters (`VDT002` on `feDropShadow` and `<circle>` filter)
    - Text (`VDT006` requiring conversion to outlines)
    - Unsupported masking/compositing (`VDT013`)
    - Radial gradients with an off-center focal point (`VDT003`)
  - The skill documentation advises against silently deleting content or making manual changes that alter how the artwork looks without your agreement. Therefore, I left the SVG unconverted so it can be fixed in the source design tool.

## Questions

- How would you like to handle `design/hero.svg`? If you want, you can flatten the text to paths, remove/rasterize the filters and drop shadows, and adjust the radial gradient in your design tool, then the SVG can be successfully converted. Alternatively, if we need it as-is, should we export it to a PNG or WebP and load it as a raster drawable instead?