#!/usr/bin/env python3
"""Run a non-gating compatibility and determinism stress test on Codicons."""

import collections
import json
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo/bin/cargo")
REPOSITORY = "https://github.com/microsoft/vscode-codicons.git"
COMMIT = "6200d186bea39354a18737cfff73fe19dbc5fcff"
ICON_COUNT = 659
MINIMUM_CONVERTIBLE = 659


def git(*arguments: object, **options) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *map(str, arguments)], check=True, **options)


def fetch_corpus(checkout: pathlib.Path) -> list[pathlib.Path]:
    print(f"Fetching Codicons at {COMMIT} with a sparse checkout", flush=True)
    git("init", "--quiet", checkout)
    git("-C", checkout, "remote", "add", "origin", REPOSITORY)
    git("-C", checkout, "config", "core.sparseCheckout", "true")
    git("-C", checkout, "config", "core.sparseCheckoutCone", "false")
    checkout.joinpath(".git/info/sparse-checkout").write_text("/src/icons/\n")
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
    return sorted((checkout / "src" / "icons").glob("*.svg"))


def resource_name(stem: str) -> str:
    """The Android resource name a file name maps to."""
    return stem.replace("-", "_").lower()


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="vdtoolkit-codicons-") as temporary:
        temporary = pathlib.Path(temporary)
        checkout = temporary / "upstream"
        supported = temporary / "supported"
        first = temporary / "generated"
        second = temporary / "generated-again"
        for directory in (supported, first, second):
            directory.mkdir()

        paths = fetch_corpus(checkout)
        if len(paths) != ICON_COUNT:
            raise SystemExit(
                f"Codicons corpus at {COMMIT} has {len(paths)} icons, "
                f"expected {ICON_COUNT}"
            )

        subprocess.run([CARGO, "build", "--release", "--locked"], cwd=ROOT, check=True)
        binary = ROOT / "target" / "release" / "vdt"
        checked = subprocess.run(
            [binary, "check", checkout / "src" / "icons", "--format", "json"],
            check=False,
            stdout=subprocess.PIPE,
        )
        if checked.returncode not in (0, 2):
            raise RuntimeError(f"unexpected compatibility exit {checked.returncode}")
        reports = json.loads(checked.stdout)
        if len(reports) != ICON_COUNT:
            raise AssertionError("Codicons analysis returned incomplete results")
        compatibility = collections.Counter(report["compatibility"] for report in reports)
        if compatibility["approximate"] != 0:
            raise AssertionError("Codicons analysis returned approximate results")

        convertible = compatibility["exact"] + compatibility["exact_with_normalization"]
        if convertible < MINIMUM_CONVERTIBLE:
            raise AssertionError(
                f"convertible Codicons regressed from {MINIMUM_CONVERTIBLE} "
                f"to {convertible}"
            )
        names = {
            pathlib.Path(report["path"]).stem
            for report in reports
            if report["compatibility"] in ("exact", "exact_with_normalization")
        }
        if len({resource_name(name) for name in names}) != len(names):
            # Upstream names that collide would overwrite each other here, and
            # the count below would report the loss.
            raise AssertionError("Codicons names collide as Android resource names")
        for report in reports:
            if report["compatibility"] in ("exact", "exact_with_normalization"):
                source = pathlib.Path(report["path"])
                # Upstream names are dashed, which Android resource names cannot
                # use; converting them as they are would report a rename for
                # every icon. Copy them under the name Android would give them.
                shutil.copyfile(source, supported / f"{resource_name(source.stem)}.svg")

        subprocess.run([binary, "convert", supported, "-o", first], check=True)
        subprocess.run([binary, "convert", supported, "-o", second], check=True)
        first_paths = sorted(path.name for path in first.glob("*.xml"))
        second_paths = sorted(path.name for path in second.glob("*.xml"))
        if first_paths != second_paths or len(first_paths) != convertible:
            raise AssertionError("Codicons conversion produced an incomplete file set")
        for name in first_paths:
            if (first / name).read_bytes() != (second / name).read_bytes():
                raise AssertionError(f"{name}: conversion is not deterministic")

    diagnostics = collections.Counter(
        diagnostic["code"] for report in reports for diagnostic in report["diagnostics"]
    )
    api_24 = sum(
        1
        for report in reports
        if any(diagnostic["code"] == "VDT004" for diagnostic in report["diagnostics"])
    )
    print(f"{convertible} / {ICON_COUNT} Codicons convert, all byte-identical on repeat")
    print(f"{api_24} / {convertible} need API 24 for their even-odd fills")
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
