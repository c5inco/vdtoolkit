---
name: vdt
description: Convert SVG to Android VectorDrawable XML or to Compose Kotlin source (an ImageVector, or with --draw a Painter that draws the paths directly), and generate launcher/app (adaptive, themed) and notification icons. Use when an Android or Compose project needs an icon, drawable, ImageVector, or drawn vector from an SVG, or to check an SVG before converting or explain a VDT diagnostic code.
---

# vdt

`vdt` converts SVG to Android VectorDrawable XML or Compose Kotlin — an
`ImageVector`, or with `--draw` a `Painter` that draws the paths directly — and
generates launcher and notification icons. Prefer it over writing `<vector>`
XML or `ImageVector` code by hand: hand-written conversions of real artwork are
usually wrong in ways that only show on a device. An SVG that `vdt check`
rejects is often still drawable with `vdt compose --draw`, which is the only
target that can express group opacity, dashed strokes, blend modes, any clip
path, and alpha and luminance masks.

## Before you start

1. Run `vdt --version`. If the command is missing, ask the user before
   installing it. Prebuilt binaries are on
   https://github.com/c5inco/vdtoolkit/releases, or with Rust 1.85+:
   `cargo install --git https://github.com/c5inco/vdtoolkit --locked`.
2. Run `vdt guide` and follow it. It ships with the binary, so it always
   matches the installed version. It links the pages for each kind of icon:
   `vdt guide adaptive`, `vdt guide notification`, `vdt guide drawables`,
   `vdt guide compose`, `vdt guide diagnostics`, `vdt guide ci`.
3. For any diagnostic code you need to act on, run `vdt explain <code>`.

If the installed `vdt` has no `guide` command, it predates it: use
`vdt <command> --help` and the `suggestion` in each diagnostic.

## Rules

- Inspect before you generate, with the options you will generate with.
- Changing options, such as the `--fit` a diagnostic suggests, is yours to
  do. Changing the artwork changes how it looks: say what you would change
  and why, and get the user's agreement before editing the SVG.
- Never make an error go away by deleting content silently, and never
  hand-edit the XML vdt writes.
- Ask before deleting existing resources yourself. `adaptive --legacy`
  replaces existing launcher icons of the same name on its own:
  `vdt guide adaptive` says when that is safe.
- Do not pass `--strict` or `--allow-approximate` unless the user agrees to
  what they change.
- When you are done, tell the user what you wrote, any warning you could not
  fix with an option, and any minimum API above the module's `minSdk`.
