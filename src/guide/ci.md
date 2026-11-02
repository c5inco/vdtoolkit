# vdt guide: CI and batch checks

```sh
vdt check res-src/ --format json
vdt check res-src/notifications/ --as notification --fit 20
```

`check` reads every SVG under a directory and writes nothing. It exits:

- `0` when every file converts,
- `2` when any file is incompatible, listing each problem with its code,
- `1` when a file is malformed or unreadable.

So it can gate a pull request on source SVGs. With `--as`, it also fails on
nothing but errors: icon warnings such as `VDT019` are reported but do not
change the exit code. Read the JSON if warnings should block a merge.

To regenerate checked-in resources reproducibly, run the same `convert`,
`optimize`, `notification`, or `adaptive` command in CI and fail when
`git diff --exit-code` reports changes: vdt's output is deterministic.
