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
  numeric values to three decimals and must be treated as a separate, lossy
  policy.
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
