# vdt guide: workflow

vdt turns SVG into Android VectorDrawable XML and generates launcher and
notification icons. It needs no Android SDK, Gradle, or JVM. The same input
always gives the same output, and anything a VectorDrawable cannot draw exactly
is rejected with a diagnostic code instead of being approximated. Use it instead
of writing `<vector>` XML by hand.

Convert, optimize, notification, and inspect without `--as` read SVG only.
`adaptive`, and `inspect` or `check` with `--as adaptive-foreground` or
`--as adaptive-background`, also take PNG, WebP, or JPEG as layer images.
vdt does not read existing VectorDrawable XML and makes no iOS or web assets.

## Before you start

Find the module's `res/` directory, usually `app/src/main/res` next to
`AndroidManifest.xml`, and the module's `minSdk` in `build.gradle.kts` or
`build.gradle`. Both change which options you need.

## Pick the command

| You want | Command | Guide |
| --- | --- | --- |
| A drawable (`R.drawable.x`, `painterResource`) | `vdt convert icon.svg -o res/drawable-anydpi/ic_icon.xml` | `vdt guide drawables` |
| The same, as small as possible | `vdt optimize icon.svg -o res/drawable-anydpi/ic_icon.xml` | `vdt guide drawables` |
| An app or launcher icon | `vdt adaptive --foreground logo.svg --background-color '#FFFFFF' -o res` | `vdt guide adaptive` |
| A notification or status bar icon | `vdt notification bell.svg -o res/drawable-anydpi/ic_stat_bell.xml` | `vdt guide notification` |
| To know whether an SVG converts, without writing | `vdt check icon.svg` | `vdt guide ci` |
| Diagnostics, minimum API, bounds, size | `vdt inspect icon.svg --format json` | `vdt guide diagnostics` |

`vdt icon.svg -o out.xml` is shorthand for `convert`. Every command except
`adaptive` accepts a directory as input; a directory needs `-o`, keeps its
tree, and a file that fails is named on stderr while the rest still convert.

## Inspect, fix, generate, wire up

1. **Inspect first**, as the icon you intend to make, and read the JSON:

   ```sh
   vdt inspect logo.svg --as adaptive-foreground --fit 66 --format json
   vdt inspect bg.svg --as adaptive-background --format json
   vdt inspect bell.svg --as notification --format json
   vdt inspect hero.svg --format json
   ```

   Pass the same `--fit` and `--background-fit` you will give the generator.

2. **Read the report.** `compatibility` is `exact` or
   `exact_with_normalization` when the file converts. Each diagnostic has a
   `code`, a `severity`, a `message`, often a `location`, and for every error
   and warning a `suggestion`. `vdt guide diagnostics` explains the report
   fields and exit codes; `vdt explain <code>` explains one code in depth.

3. **Fix.** Option changes, such as the `--fit` a `VDT019` suggestion names,
   are yours to make. Changes to the artwork change how it looks: say what you
   would change and why, and get the user's agreement before editing the SVG.
   Never make an error go away by deleting content silently, and never
   hand-edit the generated XML.

4. **Generate** with the same options you inspected with.

5. **Wire it up**: the manifest for a launcher icon, `setSmallIcon` for a
   notification icon, the printed resource name for a drawable. If the project
   builds locally, build it to confirm the resources compile.

## Report back

Tell the user what you generated and where, every warning you could not fix
with an option, any `VDT004` note when `minimum_api` is above the module's
`minSdk`, and any `VDT023` note about cropped background artwork.

## Topics

`vdt guide adaptive`, `vdt guide notification`, `vdt guide drawables`,
`vdt guide diagnostics`, `vdt guide ci`. `vdt explain` lists every diagnostic
code. `vdt <command> --help` lists every option.
