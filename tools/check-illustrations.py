#!/usr/bin/env python3
"""Check vdtoolkit against a pinned corpus of complex SVG illustrations.

The icon corpora establish the typical use case. This one exercises what
they cannot: many paths, nested groups with clips and masks, gradients on
fills and strokes, opacity on groups, and assets exported from Figma or
Illustrator. It reports, per source, how much converts, why the rest is
rejected, that repeated output is byte-identical, and how closely the
converted drawable renders to the source SVG.

The corpus is `tests/illustrations.txt`: files pinned by repository commit
and content hash, downloaded on demand. Compatibility floors keep a
regression from passing silently, and `--worst` lists the files the visual
comparison likes least.
"""

import argparse
import collections
import concurrent.futures
import hashlib
import http.client
import json
import pathlib
import shutil
import subprocess
import tempfile
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo/bin/cargo")
MANIFEST = ROOT / "tests/illustrations.txt"

# Source key -> (repository, commit, license, what the files are).
SOURCES = {
    "illlustrations": (
        "realvjy/illlustrations",
        "b6d6f3286995e6efbbf397eff21b572245c336fd",
        "MIT",
        "illlustrations.co, flat scene illustrations exported from Figma",
    ),
    "flowbite": (
        "themesberg/flowbite-illustrations",
        "8e5c17ff01b9f1a31bc3ad1c18d0d8a439eb4b81",
        "MIT",
        "Flowbite 3D-style illustrations, light and dark variants",
    ),
    "fluent": (
        "microsoft/fluentui-emoji",
        "1ffb34c752ecf5d402f04cfb4b392c77f57c54bc",
        "MIT",
        "Fluent Emoji color style, gradient-heavy Illustrator exports",
    ),
    "noto": (
        "googlefonts/noto-emoji",
        "8998f5dd683424a73e2314a8c1f1e359c19e8742",
        "OFL-1.1",
        "Noto Color Emoji, gradients and clip paths",
    ),
}

# Convertible files per source at the pinned commit. A drop is a regression.
MINIMUM_CONVERTIBLE = {
    "illlustrations": 50,
    "flowbite": 86,
    "fluent": 1,
    "noto": 118,
}

# Fraction of pixels that may differ from the resvg render of the source
# before a converted file counts as a visual mismatch. Anti-aliasing between
# two rasterizers stays far below this.
VISUAL_TOLERANCE = 0.005

CONVERTIBLE = ("exact", "exact_with_normalization")


def read_manifest() -> list[tuple[str, str, str]]:
    entries = []
    for line in MANIFEST.read_text().splitlines():
        if not line or line.startswith("#"):
            continue
        source, digest, path = line.split(" ", 2)
        entries.append((source, digest, path))
    return entries


def local_name(source: str, path: str) -> str:
    """Flatten a repository path into one file name without collisions."""
    return f"{source}--" + path.replace("/", "--").replace(" ", "_")


def download(item: tuple[str, str, str, pathlib.Path]) -> None:
    source, digest, path, destination = item
    repository, commit, _, _ = SOURCES[source]
    url = f"https://raw.githubusercontent.com/{repository}/{commit}/" + urllib.parse.quote(path)
    data = None
    for attempt in range(3):
        try:
            with urllib.request.urlopen(url, timeout=60) as response:
                data = response.read()
            break
        except (OSError, http.client.HTTPException):
            if attempt == 2:
                raise
    if hashlib.sha256(data).hexdigest() != digest:
        raise AssertionError(f"{source}: {path} does not match its pinned hash")
    destination.write_bytes(data)


def fetch_corpus(sources: pathlib.Path) -> dict[str, list[pathlib.Path]]:
    entries = read_manifest()
    print(f"Fetching {len(entries)} pinned illustrations", flush=True)
    items = []
    by_source: dict[str, list[pathlib.Path]] = collections.defaultdict(list)
    for source, digest, path in entries:
        directory = sources / source
        directory.mkdir(exist_ok=True)
        destination = directory / local_name(source, path)
        items.append((source, digest, path, destination))
        by_source[source].append(destination)
    with concurrent.futures.ThreadPoolExecutor(max_workers=16) as executor:
        list(executor.map(download, items))
    return by_source


def inspect(binary: pathlib.Path, directory: pathlib.Path) -> list[dict]:
    # Exit 2 means some files are incompatible and 1 that some failed to
    # parse; both still carry a full report. Anything else, or no report at
    # all, is a problem with the run itself, and stderr says which.
    result = subprocess.run(
        [binary, "inspect", directory, "--format", "json"],
        check=False,
        stdout=subprocess.PIPE,
    )
    if result.returncode not in (0, 1, 2) or not result.stdout.strip():
        raise RuntimeError(f"{directory.name}: inspect exited {result.returncode} without a report")
    return json.loads(result.stdout)


def compare(example: pathlib.Path, files: list[pathlib.Path]) -> list[dict]:
    comparisons = []
    for start in range(0, len(files), 50):
        # A file that cannot be compared is reported on its own line, and
        # the exit status only repeats that; the caller judges the lines.
        result = subprocess.run(
            [example, *files[start : start + 50]],
            check=False,
            stdout=subprocess.PIPE,
        )
        comparisons.extend(json.loads(line) for line in result.stdout.splitlines())
    return comparisons


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--worst", type=int, default=5, help="visual mismatches to list per source"
    )
    parser.add_argument(
        "--examples", type=int, default=3, help="rejected files to name per diagnostic code"
    )
    arguments = parser.parse_args()

    with tempfile.TemporaryDirectory(prefix="vdtoolkit-illustrations-") as temporary:
        temporary = pathlib.Path(temporary)
        sources = temporary / "svg"
        sources.mkdir()
        by_source = fetch_corpus(sources)

        subprocess.run(
            [CARGO, "build", "--release", "--locked", "--bins", "--example", "compare"],
            cwd=ROOT,
            check=True,
        )
        binary = ROOT / "target/release/vdt"
        example = ROOT / "target/release/examples/compare"

        summary = {}
        for source, files in by_source.items():
            directory = sources / source
            reports = inspect(binary, directory)
            if len(reports) != len(files):
                raise AssertionError(f"{source}: inspect returned {len(reports)} of {len(files)}")
            compatibility = collections.Counter()
            rejected_by_code: dict[str, list[str]] = collections.defaultdict(list)
            failed = []
            convertible = []
            metrics = collections.Counter()
            reasons: collections.Counter = collections.Counter()
            blockers: collections.Counter = collections.Counter()
            for report in reports:
                name = pathlib.Path(report["path"]).name
                if "error" in report:
                    failed.append((name, report["error"]))
                    continue
                compatibility[report["compatibility"]] += 1
                if report["compatibility"] in CONVERTIBLE:
                    convertible.append(pathlib.Path(report["path"]))
                    for key in ("paths", "gradients", "clip_paths", "groups"):
                        metrics[key] += report["metrics"][key]
                    metrics["xml_bytes"] += report["metrics"]["estimated_xml_bytes"]
                    continue
                errors = [d for d in report["diagnostics"] if d["severity"] == "error"]
                codes = sorted({d["code"] for d in errors})
                for code in codes:
                    rejected_by_code[code].append(name)
                # Which constructs block the file, and which files a single
                # construct would free: the data for deciding what to lower next.
                blockers["+".join(codes)] += 1
                for reason in {(d["code"], d["message"]) for d in errors}:
                    reasons[reason] += 1
            # Every pinned file must stay analyzable. A parse failure on a file
            # that was already unsupported would otherwise drop out of every
            # later check while the convertible floor still passes.
            if failed:
                raise AssertionError(
                    f"{source}: {len(failed)} files could not be analyzed: "
                    + "; ".join(f"{name}: {error}" for name, error in failed)
                )
            # Nothing may be approximate: a file that is rejected today, such
            # as one with a blur filter, must never quietly become lossy.
            if compatibility["approximate"] != 0:
                raise AssertionError(
                    f"{source}: {compatibility['approximate']} files became approximate"
                )
            if len(convertible) < MINIMUM_CONVERTIBLE[source]:
                raise AssertionError(
                    f"{source}: convertible files regressed from "
                    f"{MINIMUM_CONVERTIBLE[source]} to {len(convertible)}"
                )

            first = temporary / f"{source}-generated"
            second = temporary / f"{source}-generated-again"
            supported = temporary / f"{source}-supported"
            for path in (first, second, supported):
                path.mkdir()
            for path in convertible:
                shutil.copyfile(path, supported / path.name)
            if convertible:
                for output in (first, second):
                    subprocess.run(
                        [binary, "convert", supported, "-o", output],
                        check=True,
                        stderr=subprocess.DEVNULL,
                    )
            for path in convertible:
                name = path.with_suffix(".xml").name
                if (first / name).read_bytes() != (second / name).read_bytes():
                    raise AssertionError(f"{source}/{name}: conversion is not deterministic")

            comparisons = compare(example, convertible)
            if len(comparisons) != len(convertible):
                raise AssertionError(f"{source}: compare returned {len(comparisons)} of {len(convertible)}")
            mismatches = sorted(
                (c for c in comparisons if "error" in c or c["different_pixels"] > VISUAL_TOLERANCE),
                key=lambda c: -c.get("different_pixels", 1.0),
            )
            compared = [c for c in comparisons if "error" not in c]
            summary[source] = {
                "total": len(files),
                "compatibility": dict(compatibility),
                "failed": failed,
                "convertible": len(convertible),
                "rejected_by_code": {code: names for code, names in sorted(rejected_by_code.items())},
                "reasons": reasons.most_common(),
                "blockers": blockers.most_common(),
                "metrics": dict(metrics),
                "visual_matches": len(comparisons) - len(mismatches),
                "mismatches": mismatches,
                "mean_difference": (
                    sum(c["mean_absolute_difference"] for c in compared) / max(1, len(compared))
                ),
            }

    # Every check above raises on its own; the visual comparison is judged
    # here, after the report, so the worst files are still listed.
    total = sum(s["total"] for s in summary.values())
    convertible = sum(s["convertible"] for s in summary.values())
    matches = sum(s["visual_matches"] for s in summary.values())
    print()
    print(f"{convertible} / {total} illustrations are exactly convertible")
    print(f"{convertible} / {convertible} convertible illustrations are byte-identical on repeat")
    print(
        f"{matches} / {convertible} converted drawables render within "
        f"{VISUAL_TOLERANCE:.1%} of pixels of the resvg source render"
    )
    for source, s in summary.items():
        repository, commit, license, description = SOURCES[source]
        print()
        print(f"{source}: {repository} @ {commit[:12]} ({license}), {description}")
        print(
            f"  {s['convertible']} / {s['total']} convertible: "
            + ", ".join(f"{name}={count}" for name, count in sorted(s["compatibility"].items()))
        )
        if s["failed"]:
            print(f"  {len(s['failed'])} failed to parse:")
            for name, error in s["failed"][: arguments.examples]:
                print(f"    {name}: {error}")
        for code, names in s["rejected_by_code"].items():
            parts = names[: arguments.examples]
            if parts and len(names) > len(parts):
                parts.append(f"+{len(names) - len(parts)} more")
            shown = f" ({', '.join(parts)})" if parts else ""
            print(f"  rejected {code}: {len(names)}{shown}")
        for (code, message), count in s["reasons"]:
            print(f"    {count:4} {code} {message}")
        if s["blockers"]:
            print(
                "  blocked by: "
                + ", ".join(f"{codes}={count}" for codes, count in s["blockers"])
            )
        m = s["metrics"]
        if s["convertible"]:
            print(
                f"  converted: {m['paths']} paths, {m['groups']} groups, "
                f"{m['gradients']} gradients, {m['clip_paths']} clip paths, "
                f"{m['xml_bytes'] / s['convertible'] / 1024:.1f} KiB per drawable"
            )
            print(
                f"  visual: {s['visual_matches']} / {s['convertible']} within tolerance, "
                f"mean channel difference {s['mean_difference']:.2f} / 255"
            )
        for c in s["mismatches"][: arguments.worst]:
            name = pathlib.Path(c["path"]).name
            if "error" in c:
                print(f"    {name}: {c['error']}")
            else:
                print(
                    f"    {name}: {c['different_pixels']:.1%} of pixels differ, "
                    f"mean {c['mean_absolute_difference']:.2f}"
                )

    mismatched = sum(len(s["mismatches"]) for s in summary.values())
    if mismatched:
        raise AssertionError(
            f"{mismatched} converted drawables do not render like their source"
        )


if __name__ == "__main__":
    main()
