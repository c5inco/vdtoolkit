# Report

## Done

### 1. Launcher icon (design/logo.svg on #1E88E5), partly done
Inspected with `--as adaptive-foreground`. At `--fit 66` it raised VDT019 (clipped by a circular mask), so I used the suggested `--fit 50`, re-inspected (no warnings), then ran:

    vdt adaptive --foreground design/logo.svg --background-color '#1E88E5' --monochrome design/logo.svg --fit 50 --legacy --optimize -o app/src/main/res

(`--legacy` because minSdk is 23.)

Written:
- `drawable-anydpi/ic_launcher_foreground.xml`
- `drawable-anydpi/ic_launcher_monochrome.xml`
- `values/ic_launcher_background.xml`
- `mipmap-anydpi-v26/ic_launcher.xml` and `ic_launcher_round.xml`
- `mipmap/ic_launcher.xml` and `ic_launcher_round.xml` (legacy vectors)

I added `android:roundIcon="@mipmap/ic_launcher_round"` to the manifest. `android:icon` was already correct.

### 2. Notification icon, done
`vdt notification design/bell.svg --fit 20 --optimize -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml`. The bell touches all four edges of its 24dp canvas, so I used `--fit 20` for the status-bar margin. The only diagnostics were info notes (VDT011, VDT021: colors and gradient flattened to white, as expected).
`NotificationHelper.kt` now calls `.setSmallIcon(R.drawable.ic_stat_bell)`. I removed the TODO comment. `R` is in the same package, so no import is needed.

## Not done
### 3. Hero drawable (`onboarding_hero`), not created, `OnboardingScreen.kt` not changed
`vdt inspect design/hero.svg` reports errors that options cannot fix:
- VDT002 / VDT013: the `feDropShadow` filter on the circle.
- VDT006: the `<text>` "Feel the beat" is not outlined.
- VDT003: the radial gradient has a focal point (fx/fy). `--allow-approximate` would center it, which changes how it looks.
- VDT016 (warning): it is 360×240dp, above the recommended 200dp. `--size` would handle this.

Fixing these means changing the artwork: drawing the shadow as shapes, converting the text to outlines, and dropping the gradient focal point. The vdt rules say to get your agreement before that, and I couldn't ask. I did not delete content, pass `--allow-approximate`, or hand-write XML.
I also left `OnboardingScreen.kt` alone. Referencing a missing `R.drawable.onboarding_hero` would break the build, so the "TODO: hero illustration" is still there.

## Problems you should know about
1. **vdt deleted the template launcher bitmaps.** `vdt adaptive` printed "removed stale resource" for all ten `mipmap-*dpi/ic_launcher.webp` and `ic_launcher_round.webp` files. The vdt guide says to ask before deleting those, and I did not know the command would do it. This directory is not a git repo, so I cannot restore them. They were the stock template icons. I removed the empty `mipmap-*dpi` directories afterwards. The new `mipmap/ic_launcher.xml` legacy vectors replace them for API 23–25.
2. **The themed (monochrome) icon will probably look wrong.** The logo is a white rounded square with a blue line on top. Android tints only the alpha channel, so the monochrome layer will be a solid tinted rounded square with no visible line. The same artwork on the launcher background shows as a white tile on blue. A themed icon that reads well needs the line as a cutout, or as a line without the tile, which means editing the SVG or supplying a separate monochrome SVG. I didn't do that without your agreement.
3. No minimum-API notes: all generated resources are API 21 or lower, under minSdk 23. Nothing could be built or run (no SDK/Gradle), so none of this is compile-checked.

## Questions
1. May I change `hero.svg`: draw the shadow as shapes, outline the text, and remove the gradient's focal point? Or may I use `--allow-approximate` for the gradient? Which would you prefer for the shadow and text? Once you've answered, I'd convert it with `--size`, e.g. 200, name it `onboarding_hero`, and show it with `Image(painterResource(R.drawable.onboarding_hero), contentDescription = ...)`.
2. Do you want a separate monochrome SVG (line only, no white tile) for the themed icon? Do you want the logo's white tile kept on the blue background?
3. I'd have asked before deleting the old template launcher bitmaps. Please confirm they should stay gone, or tell me what to restore.
