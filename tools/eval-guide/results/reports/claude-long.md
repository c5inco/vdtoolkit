# Report

No build was possible (no SDK or Gradle). I checked every SVG with `vdt inspect` and read the generated XML. Nothing was compiled or run on a device.

## Done

### 1. Launcher icon: `design/logo.svg` on `#1E88E5`
- I inspected the logo as an adaptive foreground. At `--fit 66` it raised VDT019, because the clipped corners are past what a circular mask shows. The tool suggested 50 or smaller, so I used `--fit 50`, which left no warnings.
- I ran `vdt adaptive --foreground design/logo.svg --background-color '#1E88E5' --fit 50 --legacy -o app/src/main/res`.
- `--legacy` is there because `minSdk` is 23, below 26.
- New files in `app/src/main/res`:
  - `drawable-anydpi/ic_launcher_foreground.xml`
  - `values/ic_launcher_background.xml`
  - `mipmap-anydpi-v26/ic_launcher.xml` and `ic_launcher_round.xml`
  - `mipmap/ic_launcher.xml` and `ic_launcher_round.xml`, the vector icon for API 23–25
- `AndroidManifest.xml` already had `android:icon`. I added `android:roundIcon="@mipmap/ic_launcher_round"`.

### 2. Notification small icon
- I ran `vdt notification design/bell.svg -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml`.
- The bell's amber gradient and orange clapper are flattened to white, as Android requires (VDT021, info only).
- `NotificationHelper.kt` now calls `.setSmallIcon(R.drawable.ic_stat_bell)`. The old `ic_dialog_info` placeholder and its TODO are gone.
- `R` is in the same package as the namespace, so no import was needed.

## Not done

### 3. Onboarding hero (`onboarding_hero`): blocked
`design/hero.svg` does not convert. `vdt inspect` reports it as unsupported:
- **VDT002 (error):** the `feDropShadow` filter on the circle.
- **VDT006 (error):** the "Feel the beat" `<text>` needs outlines.
- **VDT013 (error):** unsupported filtering or compositing in the normalized tree, a consequence of the filter.
- **VDT003 (error):** the radial gradient has a focal point (`fx`/`fy`). `--allow-approximate` would fix only this one, by centring the focal point, and it still leaves the three errors above.
- **VDT016 (warning):** 360×240dp is larger than the recommended 200×200dp.

Fixing it means changing the artwork: drawing the shadow as shapes, converting the text to outlines, and likely dropping the focal point. The vdt skill says to get your agreement before that, and you weren't available. I did not edit the SVG or hand-write XML.

Because no `onboarding_hero` drawable exists, I left `OnboardingScreen.kt` unchanged. Referencing a missing resource would break the build. The `// TODO: hero illustration goes here` comment is still there.

Once there is a convertible SVG, the steps are:
1. Run `vdt convert <hero>.svg -o app/src/main/res/drawable-anydpi/onboarding_hero.xml`.
2. Replace the TODO in `OnboardingScreen.kt` with `Image(painter = painterResource(R.drawable.onboarding_hero), contentDescription = null)`, with the needed imports.

### Themed (monochrome) icon: left out on purpose
`logo.svg` is a white rounded square with a blue pulse line on top. Android's monochrome layer uses only alpha. The square and the line are both fully opaque, so the themed icon would be a blank tinted square with no pulse line. I confirmed this in the generated monochrome XML.

Without a monochrome layer, themed launchers show the normal icon untinted, which is better than a blank square. So themed icons do **not** work yet, despite your request.

## Needs your attention
- **Old launcher bitmaps were deleted.** `vdt adaptive` removed the ten template files `mipmap-{m,h,xh,xxh,xxxh}dpi/ic_launcher.webp` and `ic_launcher_round.webp` automatically. Its output says "removed stale resource".
  - The skill says to ask before deleting these. The tool did it unprompted, and the project is not under git, so I can't restore them.
  - The empty `mipmap-*dpi/` directories remain.
  - Without those bitmaps, the new vector icon in `mipmap/` is what API 23–25 devices use. I think that is what you want.
- **Fit of 50.** The logo sits at 50dp inside the 108dp layer, smaller than the usual 66. That is what the tool recommends for a circular mask. A different design could allow a larger fit.

## Questions for you
1. Can I make a monochrome version of the logo for themed icons? It would be the pulse line only, without the white square, as a separate file `design/logo-monochrome.svg`. Then I'd rerun `vdt adaptive` with `--monochrome design/logo-monochrome.svg --fit 50`. Or would you rather supply one?
2. For the hero, which should I do?
   - Redraw `hero.svg` to be VectorDrawable-friendly: shadow as shapes or removed, text outlined, focal gradient centred.
   - Use a raster image (PNG/WebP) for the hero instead.
   - Re-export the SVG from your design tool.
3. Is it fine that the old template launcher bitmaps are gone?
