# Report

Nothing was built (no SDK or Gradle here), so none of this has been compiled.
Task 1 is done except themed icons. Task 2 is done. Task 3 is not done.

## Done

### 1. Launcher icon (design/logo.svg on #1E88E5)
Command (minSdk is 23, so `--legacy`):
`vdt adaptive --foreground design/logo.svg --background-color '#1E88E5' --fit 50 --legacy --optimize -o app/src/main/res`
- `--fit 50` is the value vdt's VDT019 warning suggested. At the default `--fit 66` the artwork reached 43.3dp from the centre and a circular launcher mask would clip it. After the change inspect reported no warnings, only the VDT011 info note.
- Written:
  - `drawable-anydpi/ic_launcher_foreground.xml`
  - `values/ic_launcher_background.xml`
  - `mipmap-anydpi-v26/ic_launcher.xml` and `ic_launcher_round.xml`
  - `mipmap/ic_launcher.xml` and `ic_launcher_round.xml` (legacy vectors for API 23-25)
- `AndroidManifest.xml`: added `android:roundIcon="@mipmap/ic_launcher_round"`. `android:icon` was already correct.

### 2. Notification small icon (design/bell.svg)
Command: `vdt notification design/bell.svg --fit 20 --optimize -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml`
- I used `--fit 20` because the bell touches all four edges of its 24dp canvas (the guide says to use 20 for edge-to-edge art).
- `NotificationHelper.kt` now calls `.setSmallIcon(R.drawable.ic_stat_bell)`, replacing the placeholder and its TODO. `R` is in the same package, so no import is needed.
- Note VDT021: the colours and gradient were flattened to white. This is expected, because Android only uses the alpha channel for this icon.

## Not done, and why

### 1b. Themed (monochrome) icon: left out on purpose
- logo.svg has an opaque white rounded square behind the blue waveform. A monochrome layer is tinted by alpha only. With the logo as the monochrome layer, a themed launcher would show a solid rounded square and the waveform would disappear.
- Making it work means changing the artwork: a monochrome version of the waveform alone, with no plate. The vdt guide says to get agreement before editing artwork, and I could not ask, so I did not add a `<monochrome>` layer.
- Until then, themed launchers show the normal, untinted icon.
- To finish it, supply or approve a plate-less waveform SVG and re-run the command above with `--monochrome <that svg>` and the same `--fit 50`.

### 3. Onboarding hero: nothing added
`vdt inspect design/hero.svg` reports the file as unsupported. It cannot become a VectorDrawable exactly as drawn:
- VDT002 (error): the `feDropShadow` filter on the circle.
- VDT006 (error): the `<text>` "Feel the beat" must be outlined first.
- VDT013 (error): the filter and compositing it needs.
- VDT003 (error): the radial gradient has a focal point (`fx`/`fy`). `--allow-approximate` would centre it, so the look changes. Even with it, the filter and text errors remain.
- VDT016 (warning): the drawable would be 360x240dp, over the recommended 200dp. This could be fixed with `--size`.

Fixing it needs artwork changes: draw the shadow as shapes, outline the text, and drop or accept a centred gradient focal point. I did not edit the SVG without agreement, did not use `--allow-approximate`, and did not hand-write XML.

I also did not create `onboarding_hero` or touch `OnboardingScreen.kt`. Referencing a drawable that doesn't exist would break the build, so the `// TODO: hero illustration goes here` is still there.

## Something I need to flag
`vdt adaptive` deleted the template bitmaps without asking. Its output said "removed stale resource" for all ten `mipmap-{m,h,xh,xxh,xxxh}dpi/ic_launcher.webp` and `ic_launcher_round.webp` files. Its own guide says to ask before deleting these, and I did not expect the tool to remove them.
- The directory has no git history and I made no backup, so I cannot restore them.
- They were the default template icons, so what was lost is probably the stock placeholder icon. If they were real artwork, restore them from version control.
- The now-empty `mipmap-*dpi/` folders are still there and harmless.
- Removing them was needed anyway: on API 23-25 a density-specific bitmap would have outranked the new legacy vector, so the old icon would have kept showing.

## Questions for you
1. Themed icon: may I make or accept a monochrome SVG of the waveform without the white plate?
2. Hero: which of these do you want?
   - Draw the drop shadow as shapes.
   - Outline the "Feel the beat" text, or put it in a Compose `Text`.
   - Accept a centred gradient focal point, or remove it.
   - Use a raster image instead.
   - Once the SVG is fixed, should the drawable be placed in `drawable-anydpi/onboarding_hero.xml`, shown with `Image(painterResource(R.drawable.onboarding_hero), contentDescription = ...)`, and given a content description?
3. A foreground plate that's a white square shows as a white tile on the blue background (50dp of the 72dp visible area). Is that the intended look? I made it as the SVG is drawn.
4. Unrelated to this task: the manifest references `.MainActivity`, which isn't in the source tree here. I changed nothing about it.
