# Report

vdt 0.5.0. The app module's `minSdk` is 23. Resources live in `app/src/main/res`. There is no Android SDK or Gradle here, so nothing was compiled.

## What I did

### Launcher icon

Inspected `design/logo.svg` as an adaptive foreground before writing anything.

- At `--fit 66`, compatibility is `exact_with_normalization`, minimum API 21. `VDT019` (warning) said the artwork reaches 43.3dp from the centre, past the 36dp a circular launcher mask shows, and suggested `--fit 50` or smaller. `VDT011` (info) is safe normalization only.
- At `--fit 50` the clip warning is gone. Only `VDT011` remains.

Generated with that fit, the requested background, a monochrome layer (same SVG) so Android 13 themed icons tint it, and a legacy icon because `minSdk` is below 26:

```sh
vdt adaptive --foreground design/logo.svg --background-color '#1E88E5' \
    --monochrome design/logo.svg --fit 50 --legacy -o app/src/main/res
```

That wrote:

- `@color/ic_launcher_background` = `#1E88E5` in `app/src/main/res/values/ic_launcher_background.xml`
- `@drawable/ic_launcher_foreground` and `@drawable/ic_launcher_monochrome` in `drawable-anydpi/`
- `mipmap-anydpi-v26/ic_launcher.xml` and `ic_launcher_round.xml`, each with background, foreground, and monochrome
- `mipmap/ic_launcher.xml` and `ic_launcher_round.xml`, the legacy circular icon for API 23–25

`AndroidManifest.xml` already had `android:icon="@mipmap/ic_launcher"`. I added `android:roundIcon="@mipmap/ic_launcher_round"`.

While writing the legacy icon, vdt removed the template bitmaps `mipmap-{m,h,xh,xxh,xxxh}dpi/ic_launcher.webp` and the matching `ic_launcher_round.webp` files. Those density bitmaps would have taken precedence over the legacy vector on API 23–25. I left them removed.

The logo SVG is a white rounded square with a `#1E88E5` pulse stroke. That plate is part of the foreground drawable, so the blue background shows in the margin around the plate. I did not edit the SVG.

### Notification icon

Inspected `design/bell.svg` as a notification icon. At the default fit of 24 the painted bounds are the full 24dp canvas, so the art is drawn edge to edge. I used `--fit 20`, which insets it to about 2dp on each side.

```sh
vdt notification design/bell.svg --fit 20 \
    -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml
```

`VDT021` (info): three colors and one linear gradient were flattened to white. Android tints a notification icon from the alpha channel only. Compatibility is `exact_with_normalization`, minimum API 21, which is below `minSdk` 23, so the file is in `drawable-anydpi/`.

`NotificationHelper.showReminder` now calls `setSmallIcon(R.drawable.ic_stat_bell)`.

## What I did not do, and why

### Onboarding hero

`design/hero.svg` is not representable as a VectorDrawable. `vdt inspect` and `vdt check` both report `compatibility: unsupported` and exit 2. `vdt convert --allow-approximate` also failed (exit 1) and wrote no file. I did not edit the SVG, and I did not hand-write `<vector>` XML. `OnboardingScreen.kt` is unchanged: it still has the TODO and does not reference `R.drawable.onboarding_hero`, because that resource does not exist and a reference to it would not compile.

Diagnostics on `design/hero.svg`:

- `VDT002` (error): an `feDropShadow` filter, and the circle that uses it. VectorDrawable cannot draw SVG filters.
- `VDT006` (error): the `<text>` "Feel the beat" has to be converted to outlines first.
- `VDT013` (error): after normalization the tree still needs masking, filtering, or compositing that VectorDrawable cannot draw.
- `VDT003` (error): the radial gradient has a focal point (`fx`/`fy`). `--allow-approximate` would center that focal point; the retry still stopped on the other errors.
- `VDT011` (info): safe normalization would be required before lowering.
- `VDT016` (warning): 360×240dp is larger than Android's recommended 200×200dp for a vector.

Those fixes change the picture (drop the shadow, outline the type, move the gradient focal point). The skill says to get agreement before editing artwork, and to avoid making an error go away by deleting content. You were not available, so I left `design/hero.svg` as it was.

### Build

No Android SDK or Gradle is available, so I did not compile the resources or the Kotlin.

## Questions

1. May I change `design/hero.svg` so it can convert? That means removing the drop-shadow filter (or redrawing the shadow as ordinary shapes), outlining "Feel the beat", and either accepting a centered radial gradient via `--allow-approximate` or removing the focal point in the SVG.
2. If that file can convert, should the drawable use `--size 200` (longer side 200dp, aspect ratio kept) because of `VDT016`? I would write `drawable-anydpi/onboarding_hero.xml` and show it in `OnboardingScreen` with `painterResource`. I would also re-inspect: a gradient often needs API 24, and this module's `minSdk` is 23.
3. The themed layer is the same logo. The white plate and the pulse stroke are both fully opaque, so a themed launcher shows a rounded square and the pulse line does not read as its own shape. Should I add a stroke-only monochrome source so the pulse itself is the themed glyph?
4. vdt deleted the template `ic_launcher.webp` and `ic_launcher_round.webp` files while generating the legacy icon. I left them deleted so API 23–25 show the new icon. Should those bitmaps be restored?
