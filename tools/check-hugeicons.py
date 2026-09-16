#!/usr/bin/env python3
"""Run a non-gating compatibility and determinism stress test on Hugeicons."""

import collections
import json
import pathlib
import shutil
import subprocess
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo/bin/cargo")
REPOSITORY = "https://github.com/hugeicons/hugeicons.git"
COMMIT = "3e93f5d38c3ffb38319b1b13b128ad4bd45291c1"
ICON_COUNT = 6143
MINIMUM_CONVERTIBLE = 6137


def git(*arguments: object, **options) -> subprocess.CompletedProcess:
    return subprocess.run(["git", *map(str, arguments)], check=True, **options)


def fetch_corpus(checkout: pathlib.Path) -> list[pathlib.Path]:
    print(f"Fetching Hugeicons at {COMMIT} with a sparse checkout", flush=True)
    git("init", "--quiet", checkout)
    git("-C", checkout, "remote", "add", "origin", REPOSITORY)
    git("-C", checkout, "config", "core.sparseCheckout", "true")
    git("-C", checkout, "config", "core.sparseCheckoutCone", "false")
    checkout.joinpath(".git/info/sparse-checkout").write_text("/icons/\n")
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
    return sorted((checkout / "icons").glob("*.svg"))


def main() -> None:
    with tempfile.TemporaryDirectory(prefix="vdtoolkit-hugeicons-") as temporary:
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
                f"Hugeicons corpus at {COMMIT} has {len(paths)} icons, expected {ICON_COUNT}"
            )

        subprocess.run([CARGO, "build", "--release", "--locked"], cwd=ROOT, check=True)
        binary = ROOT / "target/release/vdt"
        checked = subprocess.run(
            [binary, "check", checkout / "icons", "--format", "json"],
            check=False,
            stdout=subprocess.PIPE,
        )
        if checked.returncode not in (0, 2):
            raise RuntimeError(f"unexpected compatibility exit {checked.returncode}")
        reports = json.loads(checked.stdout)
        if len(reports) != ICON_COUNT:
            raise AssertionError("Hugeicons analysis returned incomplete results")
        compatibility = collections.Counter(report["compatibility"] for report in reports)
        if compatibility["approximate"] != 0:
            raise AssertionError("Hugeicons analysis returned approximate results")

        for index, report in enumerate(reports):
            if report["compatibility"] in ("exact", "exact_with_normalization"):
                # Upstream names such as "re.svg" and "re:.svg" map to the same
                # resource name, so give every copy a distinct, valid one.
                shutil.copyfile(report["path"], supported / f"icon_{index:04d}.svg")
        convertible = compatibility["exact"] + compatibility["exact_with_normalization"]
        if convertible < MINIMUM_CONVERTIBLE:
            raise AssertionError(
                f"convertible Hugeicons regressed from {MINIMUM_CONVERTIBLE} to {convertible}"
            )

        subprocess.run([binary, "convert", supported, "-o", first], check=True)
        subprocess.run([binary, "convert", supported, "-o", second], check=True)
        first_paths = sorted(path.name for path in first.glob("*.xml"))
        second_paths = sorted(path.name for path in second.glob("*.xml"))
        if first_paths != second_paths or len(first_paths) != convertible:
            raise AssertionError("Hugeicons conversion produced an incomplete file set")
        for name in first_paths:
            if (first / name).read_bytes() != (second / name).read_bytes():
                raise AssertionError(f"{name}: conversion is not deterministic")

    diagnostics = collections.Counter(
        diagnostic["code"] for report in reports for diagnostic in report["diagnostics"]
    )
    print(f"{convertible} / {ICON_COUNT} Hugeicons are exactly convertible")
    print(f"{convertible} / {convertible} convertible icons are byte-identical on repeat")
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
