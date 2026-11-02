# Report

`design/logo.svg` is the launcher icon on `#1E88E5`, with a monochrome layer for themed icons. `design/bell.svg` is the notification small icon. `design/hero.svg` is not a drawable yet: VectorDrawable cannot represent it, and changing the artwork needs a decision.

No Android SDK or Gradle is available here, so nothing was built.

## What I did

### Launcher icon

Inspected `design/logo.svg` as an adaptive foreground. At the usual `--fit 66` it was `exact_with_normalization` (minimum API 21) with warning `VDT019`: the rounded rectangle reaches 43.3dp from the centre, past the 36dp a circular mask shows. The suggestion was `--fit 50` or smaller. Re-inspected at `--fit 50`: only info `VDT011` (the `<rect>` is normalized; the drawing is unchanged). `minSdk` is 23, which is below 26, so the legacy icon is included.

```sh
vdt adaptive --foreground design/logo.svg --background-color '#1E88E5' \
    --monochrome design/logo.svg --fit 50 --legacy --optimize \
    -o app/src/main/res
```

That wrote:

- `app/src/main/res/values/ic_launcher_background.xml` — color `ic_launcher_background` = `#1E88E5`
- `app/src/main/res/drawable-anydpi/ic_launcher_foreground.xml`
- `app/src/main/res/drawable-anydpi/ic_launcher_monochrome.xml`
- `app/src/main/res/mipmap-anydpi-v26/ic_launcher.xml` and `ic_launcher_round.xml` (background, foreground, and monochrome)
- `app/src/main/res/mipmap/ic_launcher.xml` and `ic_launcher_round.xml` — legacy vectors, because the art does not need API 24

`AndroidManifest.xml` already had `android:icon="@mipmap/ic_launcher"`. I added `android:roundIcon="@mipmap/ic_launcher_round"`.

`vdt adaptive` removed the template density bitmaps, because on API 23–25 they would outrank a legacy vector in `mipmap/`:

- `mipmap-mdpi` through `mipmap-xxxhdpi`: `ic_launcher.webp` and `ic_launcher_round.webp`

The empty `mipmap-mdpi` through `mipmap-xxxhdpi` directories are still there. I did not edit any generated XML, and I did not edit `design/logo.svg`.

`VDT011` needs no change. There is no `VDT004`: `minimum_api` is 21 and `minSdk` is 23. There is no `VDT023`: the background is a solid color, not cropped artwork.

The monochrome layer is the same logo. Both shapes are fully opaque (white rounded rectangle, blue pulse stroke on top). Android 13 themed icons tint the alpha channel, so the tinted glyph is that rounded rectangle. The pulse does not read as a separate cutout.

### Notification icon

Inspected `design/bell.svg` as a notification icon. It converts as `exact_with_normalization`, minimum API 21. At the default `--fit 24`, `content_bounds` touch all four edges of the 24dp canvas, so the art is edge to edge. Re-inspected at `--fit 20`: the artwork sits in a 20dp square with a 2dp margin. Info `VDT021`: 3 colors and 1 gradient were flattened to white, which is what a small icon needs. Info `VDT011`: the `<circle>` is normalized.

```sh
vdt notification design/bell.svg --fit 20 --optimize \
    -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml
```

`drawable-anydpi` is the right directory because `minSdk` is 23 (21 or newer). `NotificationHelper.showReminder` now calls `setSmallIcon(R.drawable.ic_stat_bell)`.

`VDT021` and `VDT011` need no change. No `VDT004`. `design/bell.svg` was not edited.

## What I did not do, and why

### Onboarding hero

`design/hero.svg` did not become `R.drawable.onboarding_hero`, and `OnboardingScreen.kt` is unchanged. The TODO is still there. A reference to a missing drawable would not compile.

`vdt inspect design/hero.svg` reports `unsupported`:

- `VDT002` (error): `<feDropShadow>` and the `filter` on the circle. VectorDrawable has no filters.
- `VDT006` (error): the `<text>` “Feel the beat” must be outlined first. VectorDrawable has no text.
- `VDT013` (error): the normalized tree still needs that filter.
- `VDT003` (error): the radial gradient’s focal point (`fx`/`fy`) is not its centre. `--allow-approximate` would center it and turn this into a warning, and the highlight would move. The filter and the text would still block conversion. I did not pass `--allow-approximate`.
- `VDT016` (warning): 360×240dp is above the 200×200dp vector recommendation.
- `VDT011` (info): the background `<rect>` would be normalized.

I did not edit `design/hero.svg` or hand-write VectorDrawable XML. Removing the shadow, outlining the text, or moving the focal point changes the picture, and those fixes belong in the design tool. I did not pass `--strict`.

### Build

I did not compile the app. There is no Android SDK or Gradle in this environment.

## Questions

1. May I change `design/hero.svg` so it can convert? The picture would change:
   - Remove the drop shadow, or replace it with plain shapes.
   - Outline “Feel the beat” in the design tool, or take that string out of the SVG and draw it with a `Text` composable.
   - Center the radial gradient (delete `fx`/`fy`), or accept `--allow-approximate`, which moves the highlight to the centre.
   - Set the long side with `--size` so it is at most 200dp (`VDT016`). The source is 360×240.
   - If the gradient stays, the drawable needs API 24. `minSdk` is 23. Raise `minSdk` to 24, or flatten the gradient to a solid fill?

   After that I would write `app/src/main/res/drawable-anydpi/onboarding_hero.xml` and show it in `OnboardingScreen` with `painterResource(R.drawable.onboarding_hero)`.

2. Themed icons are wired up, and the tinted glyph is the logo’s rounded rectangle, not the pulse line. Should the monochrome layer stay as this logo, or do you want a stroke-only silhouette so the pulse is the themed glyph?
