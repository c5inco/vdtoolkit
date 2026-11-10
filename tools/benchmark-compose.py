#!/usr/bin/env python3
"""Benchmark SVG-to-Compose-ImageVector converters over one SVG corpus.

Each tool is run once per file, in a fresh process, so the wall-clock numbers
include process startup: they are an end-to-end CLI measurement, not a parser
measurement. Pair them with `cargo run --example benchmark_compose --release`,
which times `vdtoolkit` warm and in process.

Besides throughput the report carries the numbers that decide whether two
converters are comparable at all:

* `converted`, `blank`, and `failed`, because a converter that exits zero and
  writes a vector with no path in it has silently dropped the artwork.
* `bytes` and `commands`, which measure the source file, not the app: a
  compiled float is four bytes however it is spelled. Treat the columns as a
  review-size and diff-size measurement.
* `compiles` and `class_bytes`, from a Kotlin compiler given a Compose
  classpath with `--classpath`, because a file that does not compile is not a
  conversion. Output that needs a tool's own runtime cannot be judged this way
  and is reported as `not_checked`.

    tools/benchmark-compose.py --corpus /tmp/corpus \
        --tool vdt:'./target/release/vdt compose --package com.example -o {kt} {svg}' \
        --tool androidboss:'npx androidboss convert-vector --input @{svg} \
            --to image-vector --name Icon --package-name com.example' \
        --tool s2c:'./s2c.kexe -p com.example -t com.example.Theme -opt=false \
            -np -o {kt} {svg}'

`{svg}` is replaced by the input path, `{kt}` by an output path inside the run
directory, and `{dir}` by that path's directory for a tool that takes an output
directory. A tool may ignore both and print the code instead, as `androidboss`
does, or rename the file, as `vdt` does to the Kotlin name of the source; the
driver reads whatever the run left behind.
"""

import argparse
import json
import re
import shlex
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

COMMANDS = re.compile(
    r"\b("
    r"close|moveTo|moveToRelative|relativeMoveTo|lineTo|lineToRelative|relativeLineTo|"
    r"horizontalLineTo|horizontalLineToRelative|verticalLineTo|verticalLineToRelative|"
    r"curveTo|curveToRelative|reflectiveCurveTo|reflectiveCurveToRelative|"
    r"quadTo|quadToRelative|reflectiveQuadTo|reflectiveQuadToRelative|arcTo|arcToRelative"
    r")\("
)


# Numbers each path command takes, for counting a path data string.
ARITY = {"M": 2, "L": 2, "H": 1, "V": 1, "C": 6, "S": 4, "Q": 4, "T": 2, "A": 7, "Z": 0}


def count_path_data(data: str) -> int:
    """Path commands in path data, where repeated commands drop their letter."""
    count = 0
    numbers = 0
    arity = 0
    for token in re.findall(r"[A-Za-z]|-?\d*\.?\d+(?:[eE][-+]?\d+)?", data):
        if token.isalpha():
            if arity:
                count += -(-numbers // arity)
            arity = ARITY.get(token.upper(), 0)
            numbers = 0
        else:
            numbers += 1
    if arity:
        count += -(-numbers // arity)
    return count


# Path nodes written as constructors, as `PathNode.MoveTo(4f, 2f)` and
# `PathNode.Close` in a `clipPathData` list, which is how some converters
# spell clip geometry.
PATH_NODES = re.compile(r"\bPathNode\.[A-Za-z]+")


def count_commands(text: str) -> int:
    """Path commands, however a converter spells them.

    Builder calls (`moveTo(…)`, `lineToRelative(…)`), path data strings parsed
    by `addPathNodes`, and path node constructors (`PathNode.MoveTo(…)`).
    """
    count = len(COMMANDS.findall(text)) + len(PATH_NODES.findall(text))
    for call in re.findall(r"addPathNodes\((.*?)\)", text, re.S):
        count += count_path_data("".join(re.findall(r'"([^"]*)"', call)))
    return count


def parse_tool(specification: str) -> tuple[str, list[str]]:
    name, _, command = specification.partition(":")
    if not name or not command:
        raise argparse.ArgumentTypeError(
            f"expected name:'command ... {{svg}} ...', got {specification!r}"
        )
    return name, shlex.split(command)


def measure(name: str, command: list[str], corpus: list[Path], work: Path,
            kotlinc: list[str], kotlin_classpath: str, details: bool) -> dict:
    results = [measure_one(name, command, svg, work, kotlinc, kotlin_classpath) for svg in corpus]
    timings = [result["ms"] for result in results if result["converted"]]
    if details:
        for result in results:
            print(json.dumps({"tool": name, **result}), flush=True)
    return {
        "tool": name,
        "assets": len(corpus),
        "converted": sum(result["converted"] for result in results),
        "blank": sum(result["blank"] for result in results),
        "failed": sum(result["failed"] for result in results),
        "bytes": sum(result["bytes"] for result in results),
        "commands": sum(result["commands"] for result in results),
        "compiles": sum(result["compiles"] for result in results),
        "class_bytes": sum(result["class_bytes"] for result in results),
        "median_ms": round(statistics.median(timings), 3) if timings else None,
        "p95_ms": round(sorted(timings)[max(0, int(len(timings) * 0.95) - 1)], 3) if timings else None,
        "total_ms": round(sum(timings), 3),
        "failures": [result["path"] for result in results if result["failed"]][:10],
        "error": next(
            (result["error"] for result in results if result["failed"] and result["error"]), ""
        ),
    }


def measure_one(name: str, command: list[str], svg: Path, work: Path,
                kotlinc: list[str], kotlin_classpath: str) -> dict:
    # One directory per file, so a tool that renames its output or writes
    # beside it cannot be confused with another file's output.
    output = work / name / svg.stem / (svg.stem + ".kt")
    output.parent.mkdir(parents=True, exist_ok=True)
    if output.exists():
        output.unlink()
    arguments = [
        argument.replace("{svg}", str(svg))
        .replace("{kt}", str(output))
        .replace("{dir}", str(output.parent))
        for argument in command
    ]
    started = time.perf_counter()
    completed = subprocess.run(arguments, capture_output=True, text=True)
    elapsed = (time.perf_counter() - started) * 1000
    # A tool may write the file it was given, rename it, as `vdt` does when the
    # source name is not a valid Kotlin name, or print the code to stdout.
    written = output.read_text() if output.exists() else ""
    written_from = output if written.strip() else None
    if written_from is None:
        for candidate in sorted(output.parent.glob("*.kt")):
            written = candidate.read_text()
            written_from = candidate
            break
    if written_from is None and completed.stdout.strip():
        # A tool that prints its output, as `androidboss` does, has it written
        # here so that the compile check has a file to read.
        written = completed.stdout
        output.write_text(written)
        written_from = output
    result = {
        "path": svg.name,
        "ms": round(elapsed, 3),
        "converted": completed.returncode == 0 and bool(written.strip()),
        "blank": completed.returncode == 0 and not count_commands(written),
        "failed": completed.returncode != 0,
        "bytes": len(written.encode()),
        "commands": count_commands(written),
        "compiles": 0,
        "class_bytes": 0,
        "error": completed.stderr.strip().splitlines()[0][:160] if completed.stderr.strip() else "",
    }
    if kotlinc and result["converted"] and written_from is not None:
        classes = output.parent / f"{written_from.stem}-classes"
        compiled = subprocess.run(
            kotlinc
            + ["-classpath", kotlin_classpath, "-d", str(classes), str(written_from)],
            capture_output=True,
            text=True,
        )
        result["compiles"] = 1 if compiled.returncode == 0 else 0
        if compiled.returncode == 0:
            result["class_bytes"] = sum(
                path.stat().st_size for path in classes.rglob("*.class")
            )
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--corpus", required=True, type=Path)
    parser.add_argument(
        "--details",
        action="store_true",
        help="also print one line per file, to see what a tool did with each one",
    )
    parser.add_argument("--tool", required=True, action="append", type=parse_tool)
    parser.add_argument(
        "--kotlinc",
        default="",
        help="Kotlin compiler command prefix, such as the probe's `java -cp <jars> K2JVMCompiler`",
    )
    parser.add_argument(
        "--kotlin-classpath",
        default="",
        help="classpath for that compiler, holding Compose and a platform android.jar",
    )
    arguments = parser.parse_args()

    corpus = sorted(arguments.corpus.glob("*.svg"))
    if not corpus:
        parser.error(f"no SVG files in {arguments.corpus}")

    with tempfile.TemporaryDirectory(prefix="compose-benchmark-") as temporary:
        work = Path(temporary)
        kotlinc = shlex.split(arguments.kotlinc)
        for name, command in arguments.tool:
            print(
                json.dumps(
                    measure(
                        name,
                        command,
                        corpus,
                        work,
                        kotlinc,
                        arguments.kotlin_classpath,
                        arguments.details,
                    )
                ),
                flush=True,
            )
    return 0


if __name__ == "__main__":
    sys.exit(main())
