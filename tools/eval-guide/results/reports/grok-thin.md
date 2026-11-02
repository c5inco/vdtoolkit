# Report

`vdt` 0.5.0. Module `minSdk` is 23 (`app/build.gradle.kts`). Resources live in `app/src/main/res`. I inspected each SVG with the options used to generate it. I did not build: there is no Android SDK or Gradle here. I did not hand-edit any XML `vdt` wrote, and I did not pass `--strict` or `--allow-approximate`.

## What I did

### Launcher icon (`design/logo.svg`)

Inspected as an adaptive foreground at `--fit 66`. Compatibility was `exact_with_normalization` (info `VDT011`: the rounded `rect` is normalized; the drawing is unchanged). Warning `VDT019`: content reached 43.3dp from the centre, past the 36dp circular mask. The suggestion was `--fit 50` or smaller. Re-inspected at `--fit 50`: `VDT019` was gone, `minimum_api` is 21 (below `minSdk`, so no `VDT004`).

Generated with that fit, a `#1E88E5` background, the same SVG as the monochrome layer (themed icons), `--legacy` because `minSdk` is below 26, and `--optimize`:

```sh
vdt adaptive --foreground design/logo.svg --background-color '#1E88E5' \
    --monochrome design/logo.svg --fit 50 --legacy --optimize \
    -o app/src/main/res
```

Wrote:

- `app/src/main/res/drawable-anydpi/ic_launcher_foreground.xml`
- `app/src/main/res/drawable-anydpi/ic_launcher_monochrome.xml`
- `app/src/main/res/values/ic_launcher_background.xml` (`@color/ic_launcher_background` = `#1E88E5`)
- `app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml`
- `app/src/main/res/mipmap-anydpi-v26/ic_launcher_round.xml`
- `app/src/main/res/mipmap/ic_launcher.xml` (legacy, API 23–25)
- `app/src/main/res/mipmap/ic_launcher_round.xml`

The manifest already had `android:icon="@mipmap/ic_launcher"`. I added `android:roundIcon="@mipmap/ic_launcher_round"`.

While writing the legacy icons, `vdt` deleted the template bitmaps as stale resources: `ic_launcher.webp` and `ic_launcher_round.webp` in `mipmap-mdpi`, `mipmap-hdpi`, `mipmap-xhdpi`, `mipmap-xxhdpi`, and `mipmap-xxxhdpi`. Those density bitmaps would have outranked the legacy vector on API 23–25. This directory is not a git repo, so I cannot put the files back. The empty density directories are still there.

### Notification icon (`design/bell.svg`)

Inspected as a notification icon. At the default `--fit 24`, painted content touched all four edges of the 24dp canvas, so I used `--fit 20` (2dp margin). Re-inspected at 20: `exact_with_normalization`, `minimum_api` 21. Info only: `VDT011` (the `circle` is normalized) and `VDT021` (3 colors and 1 gradient flattened to white; Android tints the alpha channel). No `VDT017` or `VDT018`.

```sh
vdt notification design/bell.svg --fit 20 --optimize \
    -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml
```

`NotificationHelper.showReminder` now calls `setSmallIcon(R.drawable.ic_stat_bell)`.

## What I did not do

### Onboarding hero (`design/hero.svg`)

I did not convert it, did not add `onboarding_hero`, and did not change `OnboardingScreen.kt`. Inspect reports `compatibility: unsupported`, so generation would fail. The errors are in the artwork, and changing them changes how it looks. I did not edit the SVG.

- `VDT002` (error): `<feDropShadow>` filter on the glow circle. VectorDrawable has no filters. Removing the shadow, or redrawing it as shapes, changes the look.
- `VDT013` (error): the normalized tree still needs unsupported masking, filtering, or compositing. The file has that drop-shadow filter and no mask or blend mode.
- `VDT006` (error): `<text>` "Feel the beat" (Roboto, 22px). VectorDrawable has no text. It has to be outlined in a design tool, or drawn as a `Text` composable instead of baked into the drawable.
- `VDT003` (error): radial gradient `glow` has a focal point (`fx`/`fy` = 0.3). `--allow-approximate` would center that highlight and the result would look different. I did not pass it.
- `VDT016` (warning): 360×240dp is above the 200dp guidance. After the errors above are resolved, `--size` can shrink the longer side without changing the drawing. I would use `--size 200` unless you want a different size.

`OnboardingScreen.kt` still has the TODO where the illustration should go. Pointing it at a drawable that does not exist would not compile.

### Other

No build or install. No hand-written `<vector>` XML.

## Warnings I could not fix with an option

Themed icons are wired (`ic_launcher_monochrome`), but the logo does not read as a one-color glyph. The white rounded tile and the pulse stroke are both fully opaque, so Android 13+ tints a solid rounded square. The waveform does not show. No `vdt` flag separates those shapes.

`VDT021` on the bell is expected and needs no change. No `VDT004` (`minimum_api` 21, `minSdk` 23). No `VDT023` (the launcher background is a flat color, not cropped artwork).

## Questions

1. Is it all right that `vdt adaptive --legacy` deleted the template `mipmap-*/ic_launcher.webp` and `ic_launcher_round.webp` files? I would have asked before removing them. They are gone, and there is no copy in this directory to restore. If you have the originals and want the old bitmap on API 23–25, they need to be put back (they would then hide the legacy vector).
2. May I change the monochrome artwork so themed launchers show the pulse instead of a solid rounded square? That would be a separate silhouette (pulse only, or the pulse knocked out of the tile), not an option flag.
3. May I edit `design/hero.svg` so it can become `R.drawable.onboarding_hero`? Specifically: drop or redraw the shadow (`VDT002` / `VDT013`), outline "Feel the beat" or move that string into `OnboardingScreen` (`VDT006`), and either remove the radial focal point or accept `--allow-approximate` (`VDT003`, highlight moves). I would then generate with `vdt optimize --size 200` into `app/src/main/res/drawable-anydpi/onboarding_hero.xml` and show it from `OnboardingScreen`.
