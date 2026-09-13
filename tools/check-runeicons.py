#!/usr/bin/env python3
"""Run a non-gating compatibility and determinism stress test on Rune Icons."""

import collections
import json
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo/bin/cargo")
REPOSITORY = "https://github.com/Nexvyn/runeicons.git"
COMMIT = "f649e467d1bc9f272aae3f8daa329d4c924e7340"
STYLES = ("normal", "duotone", "fill", "pixelated", "glass-icons")
STYLE_COUNTS = {
    "normal": 217,
    "duotone": 215,
    "fill": 126,
    "pixelated": 215,
    "glass-icons": 135,
}
ICON_COUNT = sum(STYLE_COUNTS.values())
MINIMUM_CONVERTIBLE = {
    "normal": 217,
    "duotone": 215,
    "fill": 126,
    "pixelated": 215,
    "glass-icons": 2,
}


def git(*arguments: object, **options) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *map(str, arguments)], check=True, **options)


def fetch_corpus(checkout: pathlib.Path) -> list[str]:
    print(f"Fetching Rune Icons at {COMMIT} with a sparse checkout", flush=True)
    git("init", "--quiet", checkout)
    git("-C", checkout, "remote", "add", "origin", REPOSITORY)
    git("-C", checkout, "config", "core.sparseCheckout", "true")
    git("-C", checkout, "config", "core.sparseCheckoutCone", "false")
    checkout.joinpath(".git/info/sparse-checkout").write_text(
        "".join(f"/public/{style}/\n" for style in STYLES)
    )
    git(
        "-C",
        checkout,
        "fetch",
        "--quiet",
        "--depth=1",
        "--filter=blob:none",
        "origin",
        COMMIT,
    )
    git("-C", checkout, "checkout", "--quiet", "--detach", "FETCH_HEAD")
    return sorted(
        path.relative_to(checkout / "public").as_posix()
        for style in STYLES
        for path in (checkout / "public" / style).rglob("*.svg")
    )


def style_of(name: str) -> str:
    return next(style for style in STYLES if name.startswith(f"{style}--"))


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="vdtoolkit-runeicons-") as temporary:
        temporary = pathlib.Path(temporary)
        checkout = temporary / "upstream"
        sources = temporary / "svg"
        supported = temporary / "supported"
        first = temporary / "generated"
        second = temporary / "generated-again"
        for directory in (sources, supported, first, second):
            directory.mkdir()

        paths = fetch_corpus(checkout)
        counts = collections.Counter(path.split("/")[0] for path in paths)
        if dict(counts) != STYLE_COUNTS:
            raise SystemExit(f"Rune Icons corpus at {COMMIT} has unexpected counts {counts}")
        for path in paths:
            # Flatten style/category/name into one directory without collisions.
            shutil.copyfile(
                checkout / "public" / path, sources / path.replace("/", "--")
            )

        subprocess.run([CARGO, "build", "--release", "--locked"], cwd=ROOT, check=True)
        binary = ROOT / "target/release/vdt"
        checked = subprocess.run(
            [binary, "check", sources, "--format", "json"],
            check=False,
            stdout=subprocess.PIPE,
        )
        if checked.returncode != 2:
            raise RuntimeError(f"expected mixed compatibility exit 2, got {checked.returncode}")
        reports = json.loads(checked.stdout)
        if len(reports) != ICON_COUNT:
            raise AssertionError("Rune Icons analysis returned incomplete results")
        compatibility = collections.Counter(report["compatibility"] for report in reports)
        if compatibility["approximate"] != 0:
            raise AssertionError("Rune Icons analysis returned approximate results")

        by_style = collections.defaultdict(collections.Counter)
        for report in reports:
            path = pathlib.Path(report["path"])
            by_style[style_of(path.name)][report["compatibility"]] += 1
            if report["compatibility"] in ("exact", "exact_with_normalization"):
                shutil.copyfile(path, supported / path.name)
        convertible = compatibility["exact"] + compatibility["exact_with_normalization"]
        for style in STYLES:
            style_convertible = (
                by_style[style]["exact"] + by_style[style]["exact_with_normalization"]
            )
            if style_convertible < MINIMUM_CONVERTIBLE[style]:
                raise AssertionError(
                    f"convertible Rune Icons {style} regressed from "
                    f"{MINIMUM_CONVERTIBLE[style]} to {style_convertible}"
                )

        subprocess.run([binary, "convert", supported, "-o", first], check=True)
        subprocess.run([binary, "convert", supported, "-o", second], check=True)
        first_paths = sorted(path.name for path in first.glob("*.xml"))
        second_paths = sorted(path.name for path in second.glob("*.xml"))
        if first_paths != second_paths or len(first_paths) != convertible:
            raise AssertionError("Rune Icons conversion produced an incomplete file set")
        for name in first_paths:
            if (first / name).read_bytes() != (second / name).read_bytes():
                raise AssertionError(f"{name}: conversion is not deterministic")

    diagnostics = collections.Counter(
        diagnostic["code"] for report in reports for diagnostic in report["diagnostics"]
    )
    print(f"{convertible} / {ICON_COUNT} Rune Icons are exactly convertible")
    print(f"{convertible} / {convertible} convertible icons are byte-identical on repeat")
    for style in STYLES:
        print(
            f"  {style}: "
            + ", ".join(f"{name}={count}" for name, count in sorted(by_style[style].items()))
        )
    print(
        "Compatibility: "
        + ", ".join(f"{name}={count}" for name, count in sorted(compatibility.items()))
    )
    print(
        "Diagnostics: "
        + ", ".join(f"{code}={count}" for code, count in sorted(diagnostics.items()))
    )


if __name__ == "__main__":
    main()
