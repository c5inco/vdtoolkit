#!/usr/bin/env python3
"""Semantically compare vdtoolkit output with pinned official Android drawables."""

import argparse
import concurrent.futures
import math
import pathlib
import re
import shutil
import struct
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
ARITY = {"M": 2, "L": 2, "H": 1, "V": 1, "Q": 4, "T": 2, "C": 6, "S": 4, "A": 7}
# Adaptive icon layer geometry, as in Android Studio's Image Asset wizard with
# a Material Symbol as the foreground: a 108dp layer with the symbol scaled to
# the 66dp safe zone.
ADAPTIVE_SIZE = 108.0
ADAPTIVE_FIT = 66.0
ADAPTIVE_BACKGROUND = "#3DDC84"


def android_path(svg_path: str) -> str:
    return svg_path.replace("symbols/web/", "symbols/android/")[:-4] + ".xml"


def download(item: tuple[str, pathlib.Path]) -> None:
    source, destination = item
    with urllib.request.urlopen(RAW + source) as response:
        destination.write_bytes(response.read())


def f32(value: float) -> float:
    return struct.unpack("f", struct.pack("f", value))[0]


def canonical_path(
    data: str, simplify: bool = True
) -> list[tuple[str, tuple[float, ...]]]:
    """Expand relative and shorthand path commands without using vdtoolkit code.

    Values are read and relative ones added up in float32, as Android's
    PathParser does: `--optimize` picks relative spellings that land exactly on
    the rounded point in float32, which float64 sums would miss by a few millionths.
    """
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
        arity = ARITY[upper]
        if index + arity > len(tokens) or tokens[index].isalpha():
            raise ValueError(f"incomplete {command} command: {data}")
        values = [f32(float(value)) for value in tokens[index : index + arity]]
        index += arity
        relative = command.islower()
        x, y = current

        if upper == "M":
            point = (f32(values[0] + x), f32(values[1] + y)) if relative else tuple(values)
            output.append(("M", point))
            current = start = point
            command = "l" if relative else "L"
        elif upper == "L":
            point = (f32(values[0] + x), f32(values[1] + y)) if relative else tuple(values)
            output.append(("L", point))
            current = point
        elif upper == "H":
            current = (f32(values[0] + x) if relative else values[0], y)
            output.append(("L", current))
        elif upper == "V":
            current = (x, f32(values[0] + y) if relative else values[0])
            output.append(("L", current))
        elif upper == "Q":
            control = (f32(values[0] + x), f32(values[1] + y)) if relative else tuple(values[:2])
            point = (f32(values[2] + x), f32(values[3] + y)) if relative else tuple(values[2:])
            output.append(("Q", control + point))
            quad_control, current = control, point
        elif upper == "T":
            control = (
                (2 * x - quad_control[0], 2 * y - quad_control[1])
                if previous in ("Q", "T") and quad_control is not None
                else current
            )
            point = (f32(values[0] + x), f32(values[1] + y)) if relative else tuple(values)
            output.append(("Q", control + point))
            quad_control, current = control, point
        elif upper == "C":
            first = (f32(values[0] + x), f32(values[1] + y)) if relative else tuple(values[:2])
            second = (f32(values[2] + x), f32(values[3] + y)) if relative else tuple(values[2:4])
            point = (f32(values[4] + x), f32(values[5] + y)) if relative else tuple(values[4:])
            output.append(("C", first + second + point))
            cubic_control, current = second, point
        elif upper == "S":
            first = (
                (2 * x - cubic_control[0], 2 * y - cubic_control[1])
                if previous in ("C", "S") and cubic_control is not None
                else current
            )
            second = (f32(values[0] + x), f32(values[1] + y)) if relative else tuple(values[:2])
            point = (f32(values[2] + x), f32(values[3] + y)) if relative else tuple(values[2:])
            output.append(("C", first + second + point))
            cubic_control, current = second, point
        elif upper == "A":
            point = (f32(values[5] + x), f32(values[6] + y)) if relative else tuple(values[5:])
            output.append(("A", tuple(values[:5]) + point))
            current = point

        if upper not in ("Q", "T"):
            quad_control = None
        if upper not in ("C", "S"):
            cubic_control = None
        previous = upper
    return simplify_path(output) if simplify else output


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
        if command == "A":
            if endpoint != current:
                subpath.append((command, values))
            current = endpoint
            continue
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


def vector_geometry(
    path: pathlib.Path,
) -> tuple[
    float,
    float,
    list[tuple[float, list[tuple[str, tuple[float, ...]]]]],
]:
    root = ET.parse(path).getroot()
    width = float(root.attrib[ANDROID + "viewportWidth"])
    height = float(root.attrib[ANDROID + "viewportHeight"])
    paths = [
        (
            float(node.attrib.get(ANDROID + "fillAlpha", "1")),
            canonical_path(node.attrib[ANDROID + "pathData"]),
        )
        for node in root.iter("path")
        if ANDROID + "pathData" in node.attrib
    ]
    return width, height, paths


def assert_same_geometry(
    generated: pathlib.Path, official: pathlib.Path, compare_alpha: bool
) -> None:
    generated_width, generated_height, actual = vector_geometry(generated)
    official_width, official_height, expected = vector_geometry(official)
    if len(actual) != len(expected):
        raise AssertionError(f"{generated.name}: path count {len(actual)} != {len(expected)}")
    for path_index, ((actual_alpha, actual_path), (expected_alpha, expected_path)) in enumerate(
        zip(actual, expected)
    ):
        if compare_alpha and abs(actual_alpha - expected_alpha) > 1e-6:
            raise AssertionError(
                f"{generated.name}: path {path_index} fill alpha "
                f"{actual_alpha} != {expected_alpha}"
            )
        if paths_numerically_equal(actual_path, expected_path):
            continue
        actual_contours = flatten(actual_path, generated_width, generated_height)
        expected_contours = flatten(expected_path, official_width, official_height)
        difference = filled_area_difference(actual_contours, expected_contours)
        # SVG circles normalize to cubic Béziers while official Android assets
        # retain arcs; allow only their sub-pixel approximation error.
        if difference > 5e-5:
            raise AssertionError(
                f"{generated.name}: path {path_index} differs by "
                f"{difference:.8f} normalized filled area"
            )


def transform_path(
    commands: list[tuple[str, tuple[float, ...]]], scale: float, dx: float, dy: float
) -> list[tuple[str, tuple[float, ...]]]:
    """Apply a uniform scale and translation to absolute path coordinates."""
    result = []
    for command, values in commands:
        if command == "A":
            radii = (values[0] * scale, values[1] * scale)
            values = radii + values[2:5] + (values[5] * scale + dx, values[6] * scale + dy)
        else:
            values = tuple(
                value * scale + (dx if index % 2 == 0 else dy)
                for index, value in enumerate(values)
            )
        result.append((command, values))
    return result


def assert_adaptive_layer(
    layer: pathlib.Path, plain: pathlib.Path, abs_tol: float = 1e-5
) -> None:
    """The foreground layer must be the plain drawable fitted to the safe zone.

    `abs_tol` is the distance in dp each coordinate may sit from the fitted
    geometry; an optimized layer is rounded to a thousandth.
    """
    root = ET.parse(layer).getroot()
    for attribute, expected in (
        ("width", "108dp"),
        ("height", "108dp"),
        ("viewportWidth", "108"),
        ("viewportHeight", "108"),
    ):
        actual = root.attrib.get(ANDROID + attribute)
        if actual != expected:
            raise AssertionError(f"{layer.name}: {attribute} {actual!r} != {expected!r}")
    plain_root = ET.parse(plain).getroot()
    width, height, _ = vector_geometry(plain)
    scale = ADAPTIVE_FIT / max(width, height)
    dx = (ADAPTIVE_SIZE - width * scale) / 2
    dy = (ADAPTIVE_SIZE - height * scale) / 2
    layer_paths = [node for node in root.iter("path")]
    plain_paths = [node for node in plain_root.iter("path")]
    if len(layer_paths) != len(plain_paths):
        raise AssertionError(
            f"{layer.name}: path count {len(layer_paths)} != {len(plain_paths)}"
        )
    for index, (actual, expected) in enumerate(zip(layer_paths, plain_paths)):
        # Only geometry and stroke width may change; every other attribute,
        # and therefore the minimum API, must be untouched.
        scaled = {ANDROID + "pathData", ANDROID + "strokeWidth"}
        actual_style = {k: v for k, v in actual.attrib.items() if k not in scaled}
        expected_style = {k: v for k, v in expected.attrib.items() if k not in scaled}
        if actual_style != expected_style:
            raise AssertionError(f"{layer.name}: path {index} attributes changed")
        # Compare raw commands: the simplifier's absolute tolerance would
        # treat the two scales differently.
        fitted = transform_path(
            canonical_path(expected.attrib[ANDROID + "pathData"], simplify=False),
            scale,
            dx,
            dy,
        )
        if not paths_numerically_equal(
            canonical_path(actual.attrib[ANDROID + "pathData"], simplify=False),
            fitted,
            abs_tol=abs_tol,
        ):
            raise AssertionError(f"{layer.name}: path {index} is not the fitted geometry")
        if ANDROID + "strokeWidth" in expected.attrib and not math.isclose(
            float(actual.attrib[ANDROID + "strokeWidth"]),
            float(expected.attrib[ANDROID + "strokeWidth"]) * scale,
            rel_tol=1e-6,
            abs_tol=abs_tol,
        ):
            raise AssertionError(f"{layer.name}: path {index} stroke width is not scaled")


# Half a thousandth of a dp, the most `--optimize` moves a coordinate, plus
# the f32 allowance the plain check already has.
OPTIMIZED_TOLERANCE = 5e-4 + 1e-5


def check_adaptive_icons(
    binary: pathlib.Path, sources: pathlib.Path, generated: pathlib.Path, temporary: pathlib.Path
) -> int:
    """Generate an adaptive icon from every symbol; check layers and determinism.

    Each symbol is generated twice plain and twice with `--optimize`, so both
    paths are checked for byte-identical repeated output.
    """
    first = temporary / "adaptive"
    second = temporary / "adaptive-again"
    optimized = temporary / "adaptive-optimized"
    optimized_again = temporary / "adaptive-optimized-again"
    count = 0
    for svg in sorted(sources.glob("*.svg")):
        name = "ic_" + re.sub(r"[^a-z0-9_]", "_", svg.stem.lower())
        for output, extra in (
            (first, []),
            (second, []),
            (optimized, ["--optimize"]),
            (optimized_again, ["--optimize"]),
        ):
            subprocess.run(
                [
                    binary,
                    "adaptive",
                    "--foreground",
                    svg,
                    "--background-color",
                    ADAPTIVE_BACKGROUND,
                    "--fit",
                    str(ADAPTIVE_FIT),
                    "--name",
                    name,
                    "--output",
                    output,
                    *extra,
                ],
                check=True,
                stdout=subprocess.DEVNULL,
            )
        written = [
            f"drawable/{name}_foreground.xml",
            f"values/{name}_background.xml",
            f"mipmap-anydpi-v26/{name}.xml",
            f"mipmap-anydpi-v26/{name}_round.xml",
        ]
        for relative in written:
            if (first / relative).read_bytes() != (second / relative).read_bytes():
                raise AssertionError(f"{relative}: adaptive output is not deterministic")
            if (optimized / relative).read_bytes() != (
                optimized_again / relative
            ).read_bytes():
                raise AssertionError(
                    f"{relative}: optimized adaptive output is not deterministic"
                )
        # Only the layer drawable carries numbers to shorten.
        for relative in written[1:]:
            if (optimized / relative).read_bytes() != (first / relative).read_bytes():
                raise AssertionError(f"{relative}: --optimize changed a non-drawable")
        if len((optimized / written[0]).read_bytes()) > len(
            (first / written[0]).read_bytes()
        ):
            raise AssertionError(f"{written[0]}: --optimize made the layer larger")
        icon = ET.parse(first / written[2]).getroot()
        references = {
            child.tag: child.attrib.get(ANDROID + "drawable") for child in icon
        }
        if icon.tag != "adaptive-icon" or references != {
            "background": f"@color/{name}_background",
            "foreground": f"@drawable/{name}_foreground",
        }:
            raise AssertionError(f"{name}: unexpected adaptive-icon resource {references}")
        color = ET.parse(first / written[1]).getroot().find("color")
        if color is None or color.attrib.get("name") != f"{name}_background" or (
            color.text != ADAPTIVE_BACKGROUND
        ):
            raise AssertionError(f"{name}: unexpected color resource")
        assert_adaptive_layer(first / written[0], generated / f"{svg.stem}.xml")
        assert_adaptive_layer(
            optimized / written[0],
            generated / f"{svg.stem}.xml",
            abs_tol=OPTIMIZED_TOLERANCE,
        )
        count += 1
    return count


def paths_numerically_equal(
    left: list[tuple[str, tuple[float, ...]]],
    right: list[tuple[str, tuple[float, ...]]],
    abs_tol: float = 1e-5,
) -> bool:
    return len(left) == len(right) and all(
        left_command == right_command
        and len(left_values) == len(right_values)
        and all(
            math.isclose(left_value, right_value, rel_tol=1e-7, abs_tol=abs_tol)
            for left_value, right_value in zip(left_values, right_values)
        )
        for (left_command, left_values), (right_command, right_values) in zip(left, right)
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
            for step in range(1, 65):
                t = step / 64
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
            for step in range(1, 65):
                t = step / 64
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
        elif command == "A":
            end = point(values[5], values[6])
            contour.extend(
                arc_points(
                    current,
                    end,
                    values[0] / width,
                    values[1] / height,
                    values[2],
                    bool(values[3]),
                    bool(values[4]),
                )
            )
            current = end
        elif command == "Z":
            finish()
    finish()
    return contours


def arc_points(
    start: tuple[float, float],
    end: tuple[float, float],
    radius_x: float,
    radius_y: float,
    rotation_degrees: float,
    large_arc: bool,
    sweep: bool,
) -> list[tuple[float, float]]:
    """Sample an SVG endpoint-parameterized elliptical arc independently."""
    radius_x, radius_y = abs(radius_x), abs(radius_y)
    if radius_x == 0 or radius_y == 0 or start == end:
        return [end]
    rotation = math.radians(rotation_degrees % 360)
    cosine, sine = math.cos(rotation), math.sin(rotation)
    half_x = (start[0] - end[0]) / 2
    half_y = (start[1] - end[1]) / 2
    transformed_x = cosine * half_x + sine * half_y
    transformed_y = -sine * half_x + cosine * half_y
    scale = (transformed_x / radius_x) ** 2 + (transformed_y / radius_y) ** 2
    if scale > 1:
        scale = math.sqrt(scale)
        radius_x *= scale
        radius_y *= scale
    numerator = max(
        0.0,
        radius_x**2 * radius_y**2
        - radius_x**2 * transformed_y**2
        - radius_y**2 * transformed_x**2,
    )
    denominator = (
        radius_x**2 * transformed_y**2 + radius_y**2 * transformed_x**2
    )
    factor = math.sqrt(numerator / denominator) if denominator else 0.0
    if large_arc == sweep:
        factor = -factor
    center_x_prime = factor * radius_x * transformed_y / radius_y
    center_y_prime = -factor * radius_y * transformed_x / radius_x
    center_x = (
        cosine * center_x_prime
        - sine * center_y_prime
        + (start[0] + end[0]) / 2
    )
    center_y = (
        sine * center_x_prime
        + cosine * center_y_prime
        + (start[1] + end[1]) / 2
    )

    def angle(first: tuple[float, float], second: tuple[float, float]) -> float:
        return math.atan2(
            first[0] * second[1] - first[1] * second[0],
            first[0] * second[0] + first[1] * second[1],
        )

    unit_start = (
        (transformed_x - center_x_prime) / radius_x,
        (transformed_y - center_y_prime) / radius_y,
    )
    unit_end = (
        (-transformed_x - center_x_prime) / radius_x,
        (-transformed_y - center_y_prime) / radius_y,
    )
    start_angle = angle((1.0, 0.0), unit_start)
    delta = angle(unit_start, unit_end)
    if sweep and delta < 0:
        delta += 2 * math.pi
    elif not sweep and delta > 0:
        delta -= 2 * math.pi
    steps = max(32, math.ceil(abs(delta) / (math.pi / 2)) * 64)
    points = []
    for step in range(1, steps + 1):
        theta = start_angle + delta * step / steps
        points.append(
            (
                center_x
                + cosine * radius_x * math.cos(theta)
                - sine * radius_y * math.sin(theta),
                center_y
                + sine * radius_x * math.cos(theta)
                + cosine * radius_y * math.sin(theta),
            )
        )
    points[-1] = end
    return points


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
    semicircle = arc_points((0.0, 0.0), (2.0, 0.0), 1.0, 1.0, 0.0, False, True)
    assert semicircle[-1] == (2.0, 0.0)
    assert max(abs(y) for _, y in semicircle) > 0.99
    assert transform_path([("M", (2.0, 4.0)), ("A", (1.0, 2.0, 0.0, 0.0, 1.0, 3.0, 5.0))], 2.0, 1.0, 3.0) == [
        ("M", (5.0, 11.0)),
        ("A", (2.0, 4.0, 0.0, 0.0, 1.0, 7.0, 13.0)),
    ]


def manifest_lines(path: pathlib.Path) -> list[str]:
    return [
        line
        for line in path.read_text().splitlines()
        if line and not line.startswith("#")
    ]


def sample_entries(suite: str) -> list[tuple[str, str]]:
    if suite == "outlined-100":
        paths = manifest_lines(ROOT / "tests/material-symbols.txt")
        entries = [(path, android_path(path)) for path in paths]
    else:
        lines = manifest_lines(ROOT / "tests/material-icons-twotone.txt")
        entries = [
            (fields[1], fields[2])
            for line in lines
            if len(fields := line.split("\t")) == 3
        ]
    if len(entries) != 100 or len(set(entries)) != 100:
        raise SystemExit(f"{suite} manifest must contain exactly 100 unique pairs")
    return entries


def prepare_full_corpus(
    sources: pathlib.Path, official: pathlib.Path, temporary: pathlib.Path
) -> int:
    print(f"Fetching outlined corpus at {COMMIT} with a sparse checkout", flush=True)
    checkout = temporary / "upstream"
    subprocess.run(["git", "init", "--quiet", checkout], check=True)
    subprocess.run(
        [
            "git",
            "-C",
            checkout,
            "remote",
            "add",
            "origin",
            "https://github.com/google/material-design-icons.git",
        ],
        check=True,
    )
    subprocess.run(
        ["git", "-C", checkout, "config", "core.sparseCheckout", "true"], check=True
    )
    subprocess.run(
        ["git", "-C", checkout, "config", "core.sparseCheckoutCone", "false"],
        check=True,
    )
    subprocess.run(
        [
            "git",
            "-C",
            checkout,
            "fetch",
            "--quiet",
            "--depth=1",
            "--filter=blob:none",
            "origin",
            COMMIT,
        ],
        check=True,
    )
    tree_paths = subprocess.run(
        ["git", "-C", checkout, "ls-tree", "-r", "--name-only", "FETCH_HEAD"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.splitlines()
    available = set(tree_paths)
    svg_paths = sorted(
        path
        for path in tree_paths
        if path.startswith("symbols/web/")
        and "/materialsymbolsoutlined/" in path
        and path.endswith("_24px.svg")
        and pathlib.PurePosixPath(path).name
        == f"{pathlib.PurePosixPath(path).parts[-3]}_24px.svg"
    )
    pairs = [(path, android_path(path)) for path in svg_paths]
    missing = [android for _, android in pairs if android not in available]
    if missing:
        raise RuntimeError(f"{len(missing)} outlined SVGs lack official Android pairs")
    checkout.joinpath(".git/info/sparse-checkout").write_text(
        "".join(f"/{path}\n/{android}\n" for path, android in pairs)
    )
    subprocess.run(
        ["git", "-C", checkout, "checkout", "--quiet", "--detach", "FETCH_HEAD"],
        check=True,
    )
    print(f"Preparing {len(pairs)} paired outlined assets", flush=True)
    for index, (svg_path, android) in enumerate(pairs):
        name = f"ic_{index:05}_{pathlib.PurePosixPath(svg_path).parts[-3]}"
        shutil.copyfile(checkout / svg_path, sources / f"{name}.svg")
        shutil.copyfile(checkout / android, official / f"{name}.xml")
    return len(pairs)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--suite",
        choices=("outlined-100", "twotone-100", "outlined-all"),
        default="outlined-100",
    )
    suite = parser.parse_args().suite
    verify_oracle()

    with tempfile.TemporaryDirectory(prefix="vdtoolkit-material-") as temporary:
        temporary = pathlib.Path(temporary)
        sources = temporary / "svg"
        official = temporary / "official"
        generated = temporary / "generated"
        second = temporary / "generated-again"
        for directory in (sources, official, generated, second):
            directory.mkdir()
        if suite == "outlined-all":
            count = prepare_full_corpus(sources, official, temporary)
        else:
            entries = sample_entries(suite)
            downloads = []
            for index, (svg_path, android) in enumerate(entries):
                # A valid Android resource name, so `convert` keeps it and the
                # generated drawable pairs with the official one by file name.
                name = f"ic_{index:03}_{svg_path.split('/')[2]}"
                downloads.extend(
                    [
                        (svg_path, sources / f"{name}.svg"),
                        (android, official / f"{name}.xml"),
                    ]
                )
            with concurrent.futures.ThreadPoolExecutor(max_workers=12) as executor:
                list(executor.map(download, downloads))
            count = len(entries)

        subprocess.run([CARGO, "build", "--release", "--locked"], cwd=ROOT, check=True)
        binary = ROOT / "target/release/vdt"
        subprocess.run([binary, "convert", sources, "-o", generated], check=True)
        subprocess.run([binary, "convert", sources, "-o", second], check=True)

        for first in sorted(generated.glob("*.xml")):
            if first.read_bytes() != (second / first.name).read_bytes():
                raise AssertionError(f"{first.name}: conversion is not deterministic")
            assert_same_geometry(
                first, official / first.name, compare_alpha=suite == "twotone-100"
            )
        adaptive_count = check_adaptive_icons(binary, sources, generated, temporary)

    label = {
        "outlined-100": "sampled outlined Material Symbols",
        "twotone-100": "sampled legacy Two Tone Material Icons",
        "outlined-all": "pinned outlined Material Symbols",
    }[suite]
    semantics = "geometry and fill alpha" if suite == "twotone-100" else "geometry"
    print(f"{count} / {count} {label} match official Android {semantics}")
    print(f"{count} / {count} {label} produce byte-identical repeated output")
    print(
        f"{adaptive_count} / {count} {label} generate deterministic adaptive icons, "
        f"plain and with --optimize, "
        f"whose foreground is the drawable fitted to the {ADAPTIVE_FIT:g}dp safe zone"
    )


if __name__ == "__main__":
    main()
