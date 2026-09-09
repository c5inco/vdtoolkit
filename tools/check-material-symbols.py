#!/usr/bin/env python3
"""Semantically compare svg2vd output with 100 official Android drawables."""

import concurrent.futures
import pathlib
import re
import shutil
import subprocess
import tempfile
import urllib.request
import xml.etree.ElementTree as ET

COMMIT = "0cbb08816df07faaae3dca060d4ebb10b66c214f"
RAW = f"https://raw.githubusercontent.com/google/material-design-icons/{COMMIT}/"
ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo/bin/cargo")
ANDROID = "{http://schemas.android.com/apk/res/android}"
TOKEN = re.compile(r"[AaCcHhLlMmQqSsTtVvZz]|[-+]?(?:\d*\.\d+|\d+\.?)(?:[eE][-+]?\d+)?")
ARITY = {"M": 2, "L": 2, "H": 1, "V": 1, "Q": 4, "T": 2, "C": 6, "S": 4}


def android_path(svg_path: str) -> str:
    return svg_path.replace("symbols/web/", "symbols/android/")[:-4] + ".xml"


def download(item: tuple[str, pathlib.Path]) -> None:
    source, destination = item
    with urllib.request.urlopen(RAW + source) as response:
        destination.write_bytes(response.read())


def canonical_path(data: str) -> list[tuple[str, tuple[float, ...]]]:
    """Expand relative and shorthand path commands without using svg2vd code."""
    tokens = TOKEN.findall(data.replace(",", " "))
    output: list[tuple[str, tuple[float, ...]]] = []
    index = 0
    command = ""
    current = (0.0, 0.0)
    start = current
    previous = ""
    quad_control: tuple[float, float] | None = None
    cubic_control: tuple[float, float] | None = None

    while index < len(tokens):
        if tokens[index].isalpha():
            command = tokens[index]
            index += 1
            if command.upper() == "Z":
                output.append(("Z", ()))
                current = start
                previous = "Z"
                quad_control = cubic_control = None
                command = ""
                continue
        if not command:
            raise ValueError(f"path data has no command near token {index}: {data}")
        upper = command.upper()
        if upper == "A":
            raise ValueError("arc comparison is not implemented for this corpus")
        arity = ARITY[upper]
        if index + arity > len(tokens) or tokens[index].isalpha():
            raise ValueError(f"incomplete {command} command: {data}")
        values = [float(value) for value in tokens[index : index + arity]]
        index += arity
        relative = command.islower()
        x, y = current

        if upper == "M":
            point = (values[0] + x, values[1] + y) if relative else tuple(values)
            output.append(("M", point))
            current = start = point
            command = "l" if relative else "L"
        elif upper == "L":
            point = (values[0] + x, values[1] + y) if relative else tuple(values)
            output.append(("L", point))
            current = point
        elif upper == "H":
            current = (values[0] + x if relative else values[0], y)
            output.append(("L", current))
        elif upper == "V":
            current = (x, values[0] + y if relative else values[0])
            output.append(("L", current))
        elif upper == "Q":
            control = (values[0] + x, values[1] + y) if relative else tuple(values[:2])
            point = (values[2] + x, values[3] + y) if relative else tuple(values[2:])
            output.append(("Q", control + point))
            quad_control, current = control, point
        elif upper == "T":
            control = (
                (2 * x - quad_control[0], 2 * y - quad_control[1])
                if previous in ("Q", "T") and quad_control is not None
                else current
            )
            point = (values[0] + x, values[1] + y) if relative else tuple(values)
            output.append(("Q", control + point))
            quad_control, current = control, point
        elif upper == "C":
            first = (values[0] + x, values[1] + y) if relative else tuple(values[:2])
            second = (values[2] + x, values[3] + y) if relative else tuple(values[2:4])
            point = (values[4] + x, values[5] + y) if relative else tuple(values[4:])
            output.append(("C", first + second + point))
            cubic_control, current = second, point
        elif upper == "S":
            first = (
                (2 * x - cubic_control[0], 2 * y - cubic_control[1])
                if previous in ("C", "S") and cubic_control is not None
                else current
            )
            second = (values[0] + x, values[1] + y) if relative else tuple(values[:2])
            point = (values[2] + x, values[3] + y) if relative else tuple(values[2:])
            output.append(("C", first + second + point))
            cubic_control, current = second, point

        if upper not in ("Q", "T"):
            quad_control = None
        if upper not in ("C", "S"):
            cubic_control = None
        previous = upper
    return simplify_path(output)


def simplify_path(
    commands: list[tuple[str, tuple[float, ...]]],
) -> list[tuple[str, tuple[float, ...]]]:
    """Remove rendering-neutral segments usvg is allowed to discard."""
    result: list[tuple[str, tuple[float, ...]]] = []
    subpath: list[tuple[str, tuple[float, ...]]] = []
    current = (0.0, 0.0)

    def flush() -> None:
        nonlocal subpath
        if len(subpath) > 1:
            result.extend(merge_line_segments(subpath))
        subpath = []

    for command, values in commands:
        if command == "M":
            flush()
            current = (values[0], values[1])
            subpath = [(command, values)]
            continue
        if command == "Z":
            if len(subpath) > 1:
                subpath.append((command, values))
            flush()
            continue

        endpoint = (values[-2], values[-1])
        controls = values[:-2]
        is_zero = endpoint == current and all(
            (controls[index], controls[index + 1]) == current
            for index in range(0, len(controls), 2)
        )
        if not is_zero:
            control_points = [
                (controls[index], controls[index + 1])
                for index in range(0, len(controls), 2)
            ]
            if command in ("Q", "C") and all(
                point_on_segment(point, current, endpoint) for point in control_points
            ):
                subpath.append(("L", endpoint))
            else:
                subpath.append((command, values))
        current = endpoint
    flush()
    return result


def merge_line_segments(
    commands: list[tuple[str, tuple[float, ...]]],
) -> list[tuple[str, tuple[float, ...]]]:
    merged: list[tuple[str, tuple[float, ...]]] = []
    for command in commands:
        if command[0] == "L" and len(merged) >= 2 and merged[-1][0] == "L":
            start = merged[-2][1][-2:]
            middle = merged[-1][1]
            end = command[1]
            if point_on_segment(middle, start, end):
                merged[-1] = command
                continue
        merged.append(command)
    return merged


def point_on_segment(
    point: tuple[float, float], start: tuple[float, float], end: tuple[float, float]
) -> bool:
    delta = (end[0] - start[0], end[1] - start[1])
    relative = (point[0] - start[0], point[1] - start[1])
    cross = delta[0] * relative[1] - delta[1] * relative[0]
    dot = relative[0] * delta[0] + relative[1] * delta[1]
    squared_length = delta[0] * delta[0] + delta[1] * delta[1]
    if squared_length == 0.0:
        return point == start
    return abs(cross) <= 1e-9 and 0.0 <= dot <= squared_length


def vector_geometry(path: pathlib.Path) -> tuple[float, float, list[list[tuple[str, tuple[float, ...]]]]]:
    root = ET.parse(path).getroot()
    width = float(root.attrib[ANDROID + "viewportWidth"])
    height = float(root.attrib[ANDROID + "viewportHeight"])
    paths = [
        canonical_path(node.attrib[ANDROID + "pathData"])
        for node in root.iter("path")
        if ANDROID + "pathData" in node.attrib
    ]
    return width, height, paths


def assert_same_geometry(generated: pathlib.Path, official: pathlib.Path) -> None:
    generated_width, generated_height, actual = vector_geometry(generated)
    official_width, official_height, expected = vector_geometry(official)
    if len(actual) != len(expected):
        raise AssertionError(f"{generated.name}: path count {len(actual)} != {len(expected)}")
    for path_index, (actual_path, expected_path) in enumerate(zip(actual, expected)):
        actual_contours = flatten(actual_path, generated_width, generated_height)
        expected_contours = flatten(expected_path, official_width, official_height)
        difference = filled_area_difference(actual_contours, expected_contours)
        if difference > 2e-5:
            raise AssertionError(
                f"{generated.name}: path {path_index} differs by "
                f"{difference:.8f} normalized filled area"
            )


def flatten(
    commands: list[tuple[str, tuple[float, ...]]], width: float, height: float
) -> list[list[tuple[float, float]]]:
    contours: list[list[tuple[float, float]]] = []
    contour: list[tuple[float, float]] = []
    current = (0.0, 0.0)

    def point(x: float, y: float) -> tuple[float, float]:
        return x / width, y / height

    def finish() -> None:
        nonlocal contour
        if len(contour) > 1:
            if contour[-1] != contour[0]:
                contour.append(contour[0])
            contours.append(contour)
        contour = []

    for command, values in commands:
        if command == "M":
            finish()
            current = point(values[0], values[1])
            contour = [current]
        elif command == "L":
            current = point(values[0], values[1])
            contour.append(current)
        elif command == "Q":
            control = point(values[0], values[1])
            end = point(values[2], values[3])
            start = current
            for step in range(1, 33):
                t = step / 32
                inverse = 1 - t
                contour.append(
                    (
                        inverse * inverse * start[0]
                        + 2 * inverse * t * control[0]
                        + t * t * end[0],
                        inverse * inverse * start[1]
                        + 2 * inverse * t * control[1]
                        + t * t * end[1],
                    )
                )
            current = end
        elif command == "C":
            first = point(values[0], values[1])
            second = point(values[2], values[3])
            end = point(values[4], values[5])
            start = current
            for step in range(1, 33):
                t = step / 32
                inverse = 1 - t
                contour.append(
                    (
                        inverse**3 * start[0]
                        + 3 * inverse * inverse * t * first[0]
                        + 3 * inverse * t * t * second[0]
                        + t**3 * end[0],
                        inverse**3 * start[1]
                        + 3 * inverse * inverse * t * first[1]
                        + 3 * inverse * t * t * second[1]
                        + t**3 * end[1],
                    )
                )
            current = end
        elif command == "Z":
            finish()
    finish()
    return contours


def scanline_intervals(
    contours: list[list[tuple[float, float]]], y: float
) -> list[tuple[float, float]]:
    crossings: list[tuple[float, int]] = []
    for contour in contours:
        for start, end in zip(contour, contour[1:]):
            if start[1] <= y < end[1]:
                direction = 1
            elif end[1] <= y < start[1]:
                direction = -1
            else:
                continue
            ratio = (y - start[1]) / (end[1] - start[1])
            crossings.append((start[0] + ratio * (end[0] - start[0]), direction))
    crossings.sort()
    intervals = []
    winding = 0
    interval_start = 0.0
    for x, direction in crossings:
        before = winding
        winding += direction
        if before == 0 and winding != 0:
            interval_start = x
        elif before != 0 and winding == 0:
            intervals.append((interval_start, x))
    return intervals


def interval_length(intervals: list[tuple[float, float]]) -> float:
    return sum(end - start for start, end in intervals)


def intersection_length(
    left: list[tuple[float, float]], right: list[tuple[float, float]]
) -> float:
    total = 0.0
    left_index = right_index = 0
    while left_index < len(left) and right_index < len(right):
        left_start, left_end = left[left_index]
        right_start, right_end = right[right_index]
        total += max(0.0, min(left_end, right_end) - max(left_start, right_start))
        if left_end < right_end:
            left_index += 1
        else:
            right_index += 1
    return total


def filled_area_difference(
    left: list[list[tuple[float, float]]], right: list[list[tuple[float, float]]]
) -> float:
    rows = 512
    difference = 0.0
    for row in range(rows):
        y = (row + 0.5) / rows
        left_intervals = scanline_intervals(left, y)
        right_intervals = scanline_intervals(right, y)
        difference += (
            interval_length(left_intervals)
            + interval_length(right_intervals)
            - 2 * intersection_length(left_intervals, right_intervals)
        )
    return difference / rows


def verify_oracle() -> None:
    assert canonical_path("M10 10h5v5l-5 0z") == [
        ("M", (10.0, 10.0)),
        ("L", (15.0, 10.0)),
        ("L", (15.0, 15.0)),
        ("L", (10.0, 15.0)),
        ("Z", ()),
    ]
    assert canonical_path("M0 0q10 0 10 10t10 10") == [
        ("M", (0.0, 0.0)),
        ("Q", (10.0, 0.0, 10.0, 10.0)),
        ("Q", (10.0, 20.0, 20.0, 20.0)),
    ]
    square = flatten(canonical_path("M0 0H10V10H0Z"), 10, 10)
    same_square = flatten(canonical_path("m0 0 10 0 0 10-10 0z"), 10, 10)
    triangle = flatten(canonical_path("M0 0L10 0L0 10Z"), 10, 10)
    assert filled_area_difference(square, same_square) == 0.0
    assert filled_area_difference(square, triangle) > 0.4


def main() -> None:
    verify_oracle()
    paths = [
        line
        for line in (ROOT / "tests/material-symbols.txt").read_text().splitlines()
        if line and not line.startswith("#")
    ]
    if len(paths) != 100 or len(set(paths)) != 100:
        raise SystemExit("material-symbols.txt must contain exactly 100 unique paths")

    with tempfile.TemporaryDirectory(prefix="svg2vd-material-") as temporary:
        temporary = pathlib.Path(temporary)
        sources = temporary / "svg"
        official = temporary / "official"
        generated = temporary / "generated"
        second = temporary / "generated-again"
        for directory in (sources, official, generated, second):
            directory.mkdir()
        downloads = []
        for index, path in enumerate(paths):
            name = f"{index:03}-{path.split('/')[2]}"
            downloads.extend(
                [(path, sources / f"{name}.svg"), (android_path(path), official / f"{name}.xml")]
            )
        with concurrent.futures.ThreadPoolExecutor(max_workers=12) as executor:
            list(executor.map(download, downloads))

        subprocess.run([CARGO, "build", "--release", "--locked"], cwd=ROOT, check=True)
        binary = ROOT / "target/release/svg2vd"
        subprocess.run([binary, "convert", sources, "-o", generated], check=True)
        subprocess.run([binary, "convert", sources, "-o", second], check=True)

        for first in sorted(generated.glob("*.xml")):
            if first.read_bytes() != (second / first.name).read_bytes():
                raise AssertionError(f"{first.name}: conversion is not deterministic")
            assert_same_geometry(first, official / first.name)

    print("100 / 100 Material Symbols match official Android geometry")
    print("100 / 100 Material Symbols produce byte-identical repeated output")


if __name__ == "__main__":
    main()
