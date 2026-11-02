# vdt guide: notification icons

```sh
vdt inspect bell.svg --as notification --format json
vdt notification bell.svg -o app/src/main/res/drawable-anydpi/ic_stat_bell.xml
vdt notification icons/ --fit 20 -o app/src/main/res/drawable-anydpi/
```

Android draws a notification's small icon from its alpha channel only, so
`notification` makes every color white and keeps opacity. `VDT021` notes what
was flattened; it needs no action.

## `--fit`

The artwork is scaled into a centered square `--fit` dp across on a 24dp
canvas. The default, 24, is right for Material icons and other art that
carries its own padding. Use 20 for art drawn edge to edge, so it gets the
2dp margin status bar icons expect. Look at `metrics.content_bounds` in the
inspect report: content that touches the edges of the viewport is edge to
edge.

## Problems only the artwork can fix

- `VDT017`: the art paints almost the whole canvas, such as a white bell cut out
  of a colored square. Tinted, it becomes a filled square. The icon has to
  be the bell alone on transparency; tell the user, since this changes the
  artwork.
- `VDT018`: nothing is painted, so the icon is invisible.

## Wire it up

Name files `ic_stat_*` by convention and put them in `drawable-anydpi/`
(`minSdk` 21+) or `drawable/`. Reference them with
`setSmallIcon(R.drawable.ic_stat_bell)`, using the resource name vdt printed.
