# Agent guidance eval

Measures whether coding agents do better with guidance in `vdt guide` than
with the long agent skill it replaced. It follows the approach of
[cav](https://github.com/rock3r/cav), which moved its guidance from a skill
into a `guide` command.

## Method

[fixture/](fixture) is a small Android project (`minSdk` 23, template launcher
bitmaps in `mipmap-*dpi/`) and three SVGs, each with a trap:

1. `logo.svg` as the launcher icon with a themed layer. The mark fills its box,
   so `--fit 66` is clipped (`VDT019` suggests 50), `minSdk` 23 needs
   `--legacy`, and `--legacy` replaces the template bitmaps.
2. `bell.svg` as the notification icon. It is drawn edge to edge, so the guide
   says `--fit 20`.
3. `hero.svg` as an onboarding drawable. It has text, a drop shadow, and a
   focal radial gradient, so it cannot convert without changing the artwork,
   which the agent must not do without asking.

Each agent gets the same prompt (`prompt.md`) under three conditions:

| Condition | Binary | Skill |
| --- | --- | --- |
| `long` | baseline, without `vdt guide` | the baseline's 226-line `SKILL.md` |
| `thin` | current | the current 40-line `SKILL.md` |
| `none` | current | none |

The skill is offered the way a harness lists one: its name, description, and
path, for the agent to read if it wants. The agent cannot ask questions and
must write `REPORT.md`. A shim logs every `vdt` command.

```sh
tools/eval-guide/setup.sh a16b795 claude codex grok pi
# give each agent its runs/<agent>-<condition>.prompt.txt
tools/eval-guide/grade.py
```

`grade.py` checks the files left behind and the logged commands: icon
generated with a fit that clears `VDT019`, `--legacy`, themed layer,
background color, `roundIcon` in the manifest, template bitmaps mentioned in
the report, notification icon generated at `--fit 20` and wired to
`setSmallIcon`, artwork unchanged, no hero drawable faked or referenced, and
how many of the hero's four problems the report names. 16 points.

## Results, 2026-11-01

Claude Sonnet 5.5, Codex GPT-6.1 Sol (medium), Grok 4.7, and Gemini Pro
through Pi, one run per cell. Reports and command logs are in
[results/](results).

| | `long` | `thin` | `none` |
| --- | --- | --- | --- |
| Claude Sonnet 5.5 | 14 | 16 | 15 |
| Codex GPT-6.1 Sol | 14.75 | 16 | 16 |
| Grok 4.7 | 16 | 16 | 16 |
| Gemini Pro (Pi) | 14.75 | 16 | 16 |
| **Mean** | **14.9** | **16.0** | **15.75** |
| Perfect runs | 1/4 | 4/4 | 3/4 |
| Mean `vdt` calls | 11.5 | 18.2 | 19.8 |

- Every `long` miss is something the guide covers: a themed layer left out,
  the default fit on edge-to-edge notification art, a screen pointed at a
  drawable that does not exist, an incomplete report.
- With no skill at all, every agent found `vdt guide` from the hint at the end
  of `vdt --help` and read it.
- No run edited the artwork, hand-wrote XML, or faked the hero drawable.
- Guide runs make more `vdt` calls, since reading the guide is a call. Token
  use was not measured.
- The eval found that the long skill was wrong: it said to ask before deleting
  the template bitmaps, but `adaptive --legacy` replaces them itself. The
  guide now says so.

One fixture and one run per cell is a small sample. The agents were told, not
forced, to stay inside their run directory.
