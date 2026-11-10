# SVG conversion benchmark

This benchmark compares warm, in-process SVG-to-VectorDrawable conversion in
`vdtoolkit` with [`Ashung/svg2vectordrawable`](https://github.com/Ashung/svg2vectordrawable).
It reports fidelity policy and output size beside timing: a converter that
silently removes unsupported content is not equivalent to one that rejects it.

## Rules

- Use a pinned corpus and record its input hashes.
- Preload SVG bytes before timing. Do not compare the other tool's asynchronous
  folder mode with `vdt` directory conversion.
- Run `vdtoolkit` in its default `exact` mode and in its explicit `optimize`
  mode. The former preserves the normalized `f32` geometry; the latter rounds
  numeric values to three decimals, writes path data in its shortest form, and
  must be treated as a separate, lossy policy.
- Run the reference tool at both its default precision (`2`) and precision `6`.
- Compare successful conversions, rejections, XML validity, determinism, and
  rendered pixels separately from timing. XML byte counts are not fidelity
  scores.

## Reproduce the warm benchmark

The commands below use the 100-asset, pinned Material Symbols corpus already
defined by this repository. They deliberately leave downloaded inputs and the
reference checkout outside the repository.

```sh
git clone https://github.com/Ashung/svg2vectordrawable /tmp/svg2vectordrawable
git -C /tmp/svg2vectordrawable checkout 332f56d488e660c0c2d6c3bbc47fd9bd316006d2
(cd /tmp/svg2vectordrawable && npm install --ignore-scripts --no-audit --no-fund)

python3 - <<'PY'
from pathlib import Path
from urllib.request import urlopen

commit = "0cbb08816df07faaae3dca060d4ebb10b66c214f"
base = f"https://raw.githubusercontent.com/google/material-design-icons/{commit}/"
corpus = Path("/tmp/vdt-benchmark-corpus")
corpus.mkdir(exist_ok=True)
paths = [line.strip() for line in Path("tests/material-symbols.txt").read_text().splitlines()
         if line and not line.startswith("#")]
for index, path in enumerate(paths):
    (corpus / f"{index:03d}-{Path(path).name}").with_suffix(".svg").write_bytes(urlopen(base + path).read())
PY

cargo run --example benchmark_convert --release -- /tmp/vdt-benchmark-corpus
cargo run --example benchmark_convert --release -- /tmp/vdt-benchmark-corpus --optimize
node tools/benchmark-svg2vectordrawable.mjs /tmp/svg2vectordrawable /tmp/vdt-benchmark-corpus 2
node tools/benchmark-svg2vectordrawable.mjs /tmp/svg2vectordrawable /tmp/vdt-benchmark-corpus 6
```

Each command emits one JSON record with ten post-warmup samples in milliseconds.
Report at least the median and p95, along with tool revisions, Node and Rust
versions, CPU information, corpus input bytes, and output bytes.

## Android Studio importer CLI

The original Java baseline is AOSP's `VdCommandLineTool`, the CLI wrapper around
`Svg2Vector`, at the Android Studio source revision cited in
[research.md](research.md). Its source checkout does not build standalone, so
the benchmark compiles that exact CLI source against the corresponding published
`com.android.tools:sdk-common:31.11.0-alpha10` engine artifact. Run it as:

```sh
java -cp "$AOSP_VD_TOOL_CLASSPATH" \
  com.android.ide.common.vectordrawable.VdCommandLineTool \
  -c -in /tmp/vdt-benchmark-corpus -out /tmp/aosp-xml
```

This is a CLI benchmark, so it includes Java VM startup and file I/O. Compare it
only with `vdt convert <directory> -o <directory>`, not the warm library API
results above.

## Initial result

On the 100 Material Symbols corpus at reference commit
`332f56d488e660c0c2d6c3bbc47fd9bd316006d2`, with Node 26.5.1 and Rust 1.98.1:

| Tool and policy | Median | p95 | Output bytes |
| --- | ---: | ---: | ---: |
| vdtoolkit `exact` | 10.45 ms | 10.55 ms | 106,088 |
| vdtoolkit `optimize` | 14.59 ms | 14.68 ms | 99,446 |
| svg2vectordrawable precision 2 | 55.88 ms | 67.06 ms | 63,542 |
| svg2vectordrawable precision 6 | 56.86 ms | 61.15 ms | 63,542 |

The `optimize` row predates shortest path data; see
[Shortest path data in `optimize`](#shortest-path-data-in-optimize) for current
figures, measured on another machine.

On the same 100 assets, the AOSP CLI had a 490.27 ms median and 531.87 ms p95,
compared with 24.55 ms and 32.33 ms for `vdt convert` directory conversion.
That is approximately a 20 times lower end-to-end CLI latency for vdtoolkit.

The default vdtoolkit output is approximately 6.1 times faster in this warm
test than the reference at precision 6. Compact CLI formatting reduced this
corpus from 110,988 to 106,088 bytes without changing geometry or precision.
The remaining output-size difference is not solely formatting: vdtoolkit
normalizes a 960-unit Material Symbol viewBox to a 24-unit viewport, whereas
the reference leaves that viewport at 960. The pixel comparison below is needed
before interpreting either output-size column as a quality result.

## Shortest path data in `optimize`

`optimize` now writes each path command in its shortest form that Android
reads as exactly the same numbers. Measured on 2026-09-14 on an Apple M5 with
Rust 1.98.1, on the same 100-asset corpus, with the previous `main` and this
change built on the same machine (median and p95 of ten warm samples; p95 is the
slowest sample):

| Build and policy | Median | p95 | Output bytes |
| --- | ---: | ---: | ---: |
| vdtoolkit `exact`, before | 5.51 ms | 7.31 ms | 106,088 |
| vdtoolkit `exact`, after | 5.10 ms | 6.31 ms | 106,088 |
| vdtoolkit `optimize`, before | 6.50 ms | 6.93 ms | 99,446 |
| vdtoolkit `optimize`, after | 12.96 ms | 13.13 ms | 85,129 |

`optimize` output is 14.4% smaller and `exact` output is unchanged. Choosing
each command's spelling costs time: `optimize` takes about twice as long as
before, roughly 0.13 ms per asset on this corpus. Rendering 210 icons on Android
API 21 through 36 showed the shortened drawables are pixel-identical to the
previous ones; see the
[Android renderer harness](../tools/android-renderer/README.md#checking-a-path-data-spelling-change).

## Pixel fidelity result

For the same 100 path-only Material Symbols, the source SVG and each generated
VectorDrawable were rasterized at 256 × 256. The VectorDrawable XML was
translated only into equivalent SVG `<path>` elements for this corpus: no
groups, clips, gradients, or strokes occurred in either output. This keeps the
rasterizer identical on both sides of each comparison, but it is not a
substitute for the Android renderer conformance harness.

| Tool and policy | Mean absolute channel difference | Pixels with a channel difference above 32 |
| --- | ---: | ---: |
| vdtoolkit `exact` | 0.39 / 255 | 0.11% |
| svg2vectordrawable precision 6 | 68.39 / 255 | 46.76% |
| AOSP `Svg2Vector` CLI | 68.70 / 255 | 46.90% |

The reference output retains a `0 0 960 960` viewport while the source uses
`0 -960 960 960`; its path coordinates consequently render mostly outside the
VectorDrawable viewport. vdtoolkit normalizes the nonzero source viewBox origin
to Android's zero-origin viewport; both JavaScript and AOSP output preserve
negative path coordinates in a zero-origin VectorDrawable viewport. The small
vdtoolkit difference is anti-aliasing at path edges.

## Unsupported-feature result

Eleven single-feature SVG fixtures were converted with each tool's default CLI
policy. vdtoolkit rejected 10: text, embedded images, filters, patterns,
animation, external references, multi-shape masks, alpha masks, focal radial
gradients, and strokes under non-uniform transforms. The only accepted fixture
used an invalid color name, which both tools converted as black.

svg2vectordrawable rejected only the external reference. It exited successfully
for all remaining unsupported fixtures: text and images became empty drawables;
filters and animation were omitted; patterns, unsupported masks, focal radial
gradients, and non-uniform stroke transforms produced a drawable without a
fidelity diagnostic. These outcomes are intentionally separate from throughput
and pixel results.

The AOSP CLI exits successfully even when conversion has warnings, so its output
must be interpreted with its diagnostics rather than its exit code. In the same
suite it produced no XML for text, images, and external references; emitted XML
with diagnostics for filters, patterns, unsupported masks, and non-uniform
stroke transforms; and emitted XML without diagnostics for animation and focal
radial gradients. vdtoolkit remains the only target in this comparison that
reports unrepresentable input as a failing conversion.

## Compose ImageVector benchmark

This compares `vdt compose` with the other converters that write Compose
`ImageVector` Kotlin. None of them is a TypeScript library. The npm registry
holds one package for this target, `androidboss` (pure JavaScript, no
dependencies); the names usually associated with this conversion are GitHub
projects in other languages, none of them published to npm:

| Project | Language | What it is |
| --- | --- | --- |
| [`DevSrSouza/svg-to-compose`](https://github.com/DevSrSouza/svg-to-compose) | Kotlin | Kotlin script built on AOSP's `Svg2Vector` and a copy of the material-icons generator |
| [`rafaeltonholo/svg-to-compose`](https://github.com/rafaeltonholo/svg-to-compose) | Kotlin | The CLI benchmarked here as `s2c` |
| [`ComposeGears/Valkyrie`](https://github.com/ComposeGears/Valkyrie) | Kotlin | The plugin and CLI benchmarked here |
| [`DevSrSouza/compose-icons`](https://github.com/DevSrSouza/compose-icons), [`composablehorizons/compose-icons`](https://github.com/composablehorizons/compose-icons) | Kotlin | Icon packs with their own generators |
| [`jinghu-moon/compose-icons`](https://github.com/jinghu-moon/compose-icons) | Kotlin | Its `tools/svg2compose` is the `svg2compose` name, a Rust CLI that is not a published crate |
| [`DenisMondon/Svg2Compose`](https://github.com/DenisMondon/Svg2Compose), [`SergioRibera/svg2compose_docker`](https://github.com/SergioRibera/svg2compose_docker) | Kotlin | Smaller `svg2compose` scripts, including a Docker image |
| [`irgaly/compose-vector-plugin`](https://github.com/irgaly/compose-vector-plugin) | Kotlin | Gradle plugin and Java library, Batik-based |
| [`serbelga/compose-vectorize`](https://github.com/serbelga/compose-vectorize) | Kotlin | Library that generates `ImageVector`s on device |
| [`LennartEgb/vec2compose`](https://github.com/LennartEgb/vec2compose) | Kotlin | CLI for vector XML and SVG |
| [`minoism/svg-to-compose-vector`](https://github.com/minoism/svg-to-compose-vector) | Python | Jinja2-templated CLI, `svg2compose`, needs Python 3.13 |

The ones with a standalone CLI are `androidboss`, `s2c`, Valkyrie, and the
Python tool, and all four are benchmarked below; the Python one needs Python
3.13, so it was measured in a container with CPython 3.13.15 and
`svg-to-compose-vector` 0.1.9 rather than on the machine the others ran on. Its
own `svg2compose version` prints `Converter v0.1.8`: the version string in the
package was not bumped with its metadata, which is worth knowing when citing
either. Tools that write VectorDrawable XML instead
(`svg2vectordrawable`, `vd-tool`) are the subject of the benchmark above, not
this one.

### Rules

- Pin every tool's version, and the corpus's commit.
- Run `vdt compose` in its default `exact` mode and in its explicit
  `--optimize` mode, as the VectorDrawable benchmark does, since rounding is a
  separate and lossy policy. `s2c` is run with `-opt=false`, its own SVGO-based
  optimization being the same kind of policy.
- Measure CLI wall time per file, in a fresh process, for every tool, so the
  comparison is end to end: it then includes each tool's startup, which is
  large for a Node package and absent from the warm numbers.
- Report what each tool does with an SVG it cannot draw, because a converter
  that exits zero and writes a vector with nothing in it has silently dropped
  the artwork.
- Report whether the output compiles, and its class bytes. Source bytes measure
  formatting and decimal digits, not the app: a compiled float is four bytes
  however many digits it is written with.

### Reproduce

Use the same 100-asset corpus the VectorDrawable benchmark's Python snippet
downloads into `/tmp/vdt-benchmark-corpus`.

```sh
cargo build --release
cargo run --example benchmark_compose --release -- /tmp/vdt-benchmark-corpus
cargo run --example benchmark_compose --release -- /tmp/vdt-benchmark-corpus --optimize

npm install androidboss@0.4.2 svg2vectordrawable@2.9.1 --no-audit --no-fund
curl -sSL -o s2c.zip \
  https://github.com/rafaeltonholo/svg-to-compose/releases/download/2.2.1/s2c-macosArm64-binaries.zip
unzip -q s2c.zip
brew install ComposeGears/repo/valkyrie

tools/benchmark-compose.py --corpus /tmp/vdt-benchmark-corpus \
  --tool vdt:"$PWD/target/release/vdt compose --package com.example -o {kt} {svg}" \
  --tool androidboss:"npx androidboss convert-vector --input @{svg} \
      --to image-vector --name Icon --package-name com.example" \
  --tool s2c:"./s2c/s2c-macosArm64-binaries/s2c.kexe -p com.example \
      -t com.example.Theme -opt=false -np -o {kt} {svg}" \
  --tool valkyrie:"valkyrie svgxml2imagevector --input-path {svg} \
      --output-path {dir} --package-name com.example"

# The Python tool needs Python 3.13, so it was measured in a container:
uv python install 3.13 && uv venv --python 3.13 /tmp/venv-svg2compose
/tmp/venv-svg2compose/bin/pip install svg-to-compose-vector
tools/benchmark-compose.py --corpus /tmp/vdt-benchmark-corpus \
  --tool svg2compose:"/tmp/venv-svg2compose/bin/svg2compose convert {svg} -o {kt}"
```

`--kotlinc` and `--kotlin-classpath` add a compile check with a Kotlin
compiler and a classpath holding Compose and a platform `android.jar`, as the
`tools/android-renderer` instructions build. Compose's Google Maven coordinates
(`androidx.compose.ui:ui` and friends) are KMP root stubs whose AARs carry no
classes: the classes are in the `-android` variants, so `ui-android`,
`ui-graphics-android`, `ui-geometry-android`, `ui-unit-android`, and
`ui-util-android` are the ones to unpack. The measurements below used Kotlin
2.4.10 against Compose ui 1.7.6 and `android-36` on an Apple M5 Max, with Node
24.21.0, `androidboss` 0.4.2, `s2c` 2.2.1, and Valkyrie 1.2.1.

### Warm in-process conversion

One process, inputs preloaded, ten samples after a discarded warmup round, as
the VectorDrawable benchmark measures `vdtoolkit`.

| Policy | Median | p95 | Output bytes |
| --- | ---: | ---: | ---: |
| `vdt compose` | 5.10 ms | 5.12 ms | 300,596 |
| `vdt compose --optimize` | 9.14 ms | 9.71 ms | 293,954 |

### The 100 Material Symbols, one CLI process per icon

| Tool | Converted | Output bytes | Path commands | Median | p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| `vdt compose` | 100 | 311,014 | 5,374 | 2.53 ms | 2.93 ms |
| `vdt compose --optimize` | 100 | 304,372 | 5,374 | 2.60 ms | 2.88 ms |
| `androidboss` 0.4.2 | 100 | 343,652 | 5,374 | 78.39 ms | 79.90 ms |
| `s2c` 2.2.1 | 100 | 797,433 | 5,374 | 6.29 ms | 7.15 ms |
| Valkyrie 1.2.1 | 100 | 320,940 | 5,374 | 178.25 ms | 188.49 ms |
| `svg-to-compose-vector` 0.1.9 | 100 | 208,242 | 5,374 | 120.10 ms | 166.30 ms |

Two things about the Python tool's default output matter before its bytes are
compared with anything, and both are measured on its own files.

Its default template (`src/generator/templates/default.j2`) is
`{{imports}}

{{build_code}}`, with no declaration around it, so it writes a
builder expression at file scope: not valid Kotlin until it is given one of its
other templates (`-t val_declaration`, `composable_function`, or `icon_object`).

And nothing it writes for this corpus can paint. **None** of the 100 outputs
contains `fill =`, a `Color`, a `Brush`, or a `stroke`; all 100 use the bare
`path { … }` form with no arguments, and all 100 import only `ImageVector`,
`path`, and `dp`. The cause is a documented trade in its parser — `_parse_fill`
returns `None` for an element with no `fill` attribute, "let the element be
stroke-only", and the generator writes `fill = …` only when a fill was parsed —
but SVG's initial fill is black, and all 100 of these icons have no `fill` and
no `stroke`. So its 208,242 bytes are geometry that draws nothing in Compose.
The control is the same tool on a file that declares its paints: for
`features.svg` it writes `fill = Brush.linearGradient(…)`,
`stroke = SolidColor(Color.Green)` with its cap, join, alpha, and width, and
`fill = Brush.radialGradient(…)`, one per source shape, so only the missing
`fill` is mishandled.

The other four converters all apply SVG's default black: for the same corpus
file they write `fill = SolidColor(Color(0xFF000000))`, or the named `Color.Black`
of the tool that prefers names.

All five write the same 5,374 path commands: for this corpus they agree on the
geometry, which is the cross-check that the counting is comparable. The byte
columns are not: `s2c` writes a comment above every command naming the source
spelling, and every tool but `vdt` keeps the 960-unit Material viewport, so
`vdt` writes shorter numbers for the same drawing after normalizing it to 24
units. Every tool but `vdt` is a process-based CLI whose timing therefore
includes interpreter or runtime startup: `androidboss` is a Node package, `s2c`
a Kotlin/Native binary, and Valkyrie and the Python tool run on a JVM and on
CPython, which is what separates 2.5 ms from 120 ms and 178 ms.

Command counting has to know how a tool spells geometry. Only the Python tool
writes `clipPathData` as path node constructors
(`listOf(PathNode.MoveTo(4f, 2f), …, PathNode.Close)`), which the driver counts
alongside builder calls, `addPathNodes` strings, and nothing else; that is 25
nodes over this feature corpus, and none over the Material one.

### What each tool does with an SVG it cannot draw

The repository's feature and stress corpus, with the same three tools:

| Fixture | `vdt compose` | `androidboss` | `s2c` | Valkyrie | `svg-to-compose-vector` |
| --- | --- | --- | --- | --- | --- |
| `<text>` | Exit 1, `VDT006` | Exit 0, a vector with no path in it | Exit 0, a vector with no path in it | Exits 1, `java.io.EOFException: input contained no data` | Exit 0, a vector with no path in it, and a warning |
| A filter | Exit 1, `VDT002` | Exit 0, filter dropped | Exit 0, filter dropped | Exit 0, filter dropped | Exit 0, filter dropped, and a warning |
| A pattern | Exit 1, `VDT008` | Exit 0, pattern dropped | Exit 0, pattern dropped | Exit 0, pattern dropped | Exit 0, pattern dropped, and a warning |
| A mask (two clip paths) | Converts, 15 commands | Converts, 5 commands: the mask is gone | Converts, 10 commands | Converts, 10 commands | Converts, 5 commands: the mask is gone, and a warning |
| Gradient, stroke, clip, even-odd | Converts, compiles | Converts, compiles | Converts, compiles | Converts, compiles | Converts |

`vdt compose` reports 3 of the 13 fixtures as failures with the diagnostic and
the suggestion that `convert` reports. Every other tool exits zero for them,
and four of them write a file with no path geometry in it for the text fixture.
For the mask fixture, only `vdt compose` writes both of its two clip paths,
nested, which is what intersects them: `s2c` and Valkyrie write one clip each —
the mask's region, not its shape — and `androidboss` and the Python tool write
neither, so in all four the mask is not reproduced.

Its missing paint reaches further than the dropped constructs: for the filter
fixture, whose SVG also relies on the default fill, it writes the geometry with
no fill and no stroke. For the pattern fixture the opposite fallback appears:
`fill="url(#p)"` points at an undefined gradient, and its `url()` branch paints
`SolidColor(Color.Black)`, so SVG's default fill shows up there and nowhere
else. That fixture also shows the ignored `<pattern>`'s child geometry leaking
into the output as a second, paintless `path { … }` block — the 8 commands it
reports against the filter fixture's 4 are the inner `<path d="M0 0h4v4z"/>`.

It is the only one that says so about dropped constructs: it names each one it drops
on stderr ("Text element '<text>' is not supported by Compose ImageVector - text
will be ignored", likewise for the filter, the pattern, and the mask) while
still exiting 0. That is a better failure mode than `androidboss` and `s2c`,
which write the same reduced file with no message at all, and a worse one than
`vdt compose`, whose diagnostics are a stable code, a location, and a
suggested fix, and whose exit status is a failure.

### Compiled output and the size of one Kotlin method

Kotlin inlines `path { }`, `group { }`, and `apply { }`, so every command a
converter writes as a `PathBuilder` call becomes bytecode of the one property
getter, and the JVM refuses a method of over 64 KB. The corpus therefore
includes traced-outline-sized drawings of 3,000 and 6,000 cubic commands:

| Drawing | `vdt compose` | `androidboss` | `s2c` | Valkyrie | `svg-to-compose-vector` (`-t val_declaration`) |
| --- | --- | --- | --- | --- | --- |
| 3,000 commands | Compiles, 90,585-byte class | Compiles, 70,530-byte class | Compiles, 155,058-byte class | Compiles, 70,663-byte class | Compiles, 70,344-byte class |
| 6,000 commands | Compiles, 180,010-byte class | **Method too large** | **Method too large** | **Method too large** | **Method too large** |

The Python tool needs that extra step to be judged at all: its generated body is
byte-identical between the default template and `val_declaration` (186,238 and
372,238 bytes for the two drawings), so putting it in a declaration isolates the
JVM question from the invalid-output question, and shows the same
`Method too large: Stress6000Kt.<clinit> ()V` as the other two. Its ceiling is
about 3,600 commands, where 18.0 bytes of bytecode per command reaches the
64 KB a method may hold: at 3,000 commands its `<clinit>` is 54,190 bytes, at
3,600 64,990 bytes, and at 3,700 it fails. `vdt compose`'s budget of a thousand
commands leaves room for forms that cost more per command than that.

`vdt compose` writes a drawing of over a thousand commands as path data strings
(`addPath(pathData = addPathNodes("M0 0h24v24z …"))`), one call per path, so the
getter stays small however long the path is. Measured on its own, a drawing of
2,500 cubic commands written as `PathBuilder` calls compiles and one of 3,000
fails, which is where the budget comes from.

Valkyrie has the same escape hatch, `--use-path-data-string`, which is off by
default: with it on, its 6,000-command drawing compiles, at 192,878 bytes
against 444,830 for its builder calls. Neither `androidboss` nor `s2c` has one,
so for them the row above is the whole story. `vdt compose` switches on its own,
because a file that does not compile is not a conversion the user can decline:
the drawing is what they asked for, and how it is spelled is not.

On eight of the 100 icons, compiled in a fresh process each:

| Tool | Compiled | Class bytes |
| --- | ---: | ---: |
| `vdt compose` | 8 | 46,653 |
| `androidboss` | 8 | 55,566 |
| Valkyrie | 8 | 48,917 |
| `svg-to-compose-vector` | Not compiled | — |
| `s2c` | 0 | — |

`s2c` takes the property name from the file it writes and does not make it a
valid Kotlin identifier: `-o 000-mobile_off_24px.kt` writes
`val 000MobileOff24px`, which the compiler rejects. `vdt compose` derives the
name first and writes the file under it, so `000-mobile_off_24px.svg` becomes
`Ic000MobileOff24px.kt` holding `public val Ic000MobileOff24px`.

### What this benchmark does not measure

It does not compare rendered pixels, and two things are missing from the
compile columns. The Python tool's corpus was not compiled beyond its two large
drawings, which were compiled through its own `val_declaration` template
because its default output is not valid Kotlin. And where a cell says a drawing
compiles, that says nothing about the output for the rest of a tool's corpus.

What the compile columns do say is narrow and useful: against Compose ui 1.7.6,
`vdt compose`, `androidboss`, `s2c`, and Valkyrie emit Kotlin that compiles for
every file they were measured on except the 6,000-command drawing, where all
three of the others exceed the JVM's 64 KB method limit and `vdt compose` does
not.

Whether they draw the same picture is a separate question, and one for a
device: `tools/android-renderer/compose` already loads generated `ImageVector`s
through `rememberVectorPainter`, and each tool's output would have to be
compiled into it and compared against the source SVG. The numbers here say how
fast each converter runs, how large its output is, and what it does with input
it cannot represent; they are not fidelity scores.
