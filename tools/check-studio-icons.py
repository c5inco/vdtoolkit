#!/usr/bin/env python3
"""Run a non-gating compatibility and determinism stress test on Studio Icons."""

import collections
import concurrent.futures
import hashlib
import json
import pathlib
import re
import shutil
import subprocess
import tempfile
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
CARGO = shutil.which("cargo") or str(pathlib.Path.home() / ".cargo/bin/cargo")
SITE = "https://studio-icons.web.app"
DATA_ASSET = "/assets/data-Bk27tcYL.js"
DATA_SHA256 = "f83e2c3d71d7feedab59b56c24c81b76196a04ccbd3d3b7e65f1f4f2ed3e75d7"
CORPUS_SHA256 = "110d489ebf80ffc7fbc06e564caca85bca5a997d49a9a4bd57953d95a4af346e"
ICON_COUNT = 812
MINIMUM_CONVERTIBLE = 802


def fetch(url: str) -> bytes:
    with urllib.request.urlopen(url, timeout=30) as response:
        return response.read()


def parse_icons(module: bytes) -> list[dict]:
    text = module.decode()
    match = re.search(r"JSON\.parse\('(.*)'\)", text)
    if match is None:
        raise ValueError("Studio Icons metadata no longer contains its JSON payload")
    icons = json.loads(match.group(1))
    if not isinstance(icons, list):
        raise ValueError("Studio Icons metadata payload is not an icon list")
    return icons


def icon_url(icon: dict) -> str:
    return f'{SITE}/icons/{icon["section"]}/{icon["name"]}.svg'


def download_icon(item: tuple[int, dict]) -> tuple[int, dict, str, bytes]:
    index, icon = item
    url = icon_url(icon)
    return index, icon, url, fetch(url)


def verify_metadata_parser() -> None:
    sample = b'''const icons=JSON.parse('[{"name":"add"}]');export{icons};'''
    assert parse_icons(sample) == [{"name": "add"}]


def main() -> None:
    verify_metadata_parser()
    module = fetch(SITE + DATA_ASSET)
    module_digest = hashlib.sha256(module).hexdigest()
    if module_digest != DATA_SHA256:
        raise SystemExit(
            "Studio Icons metadata snapshot changed; inspect the new source before updating "
            f"DATA_ASSET and its digest (observed {module_digest})"
        )
    icons = parse_icons(module)
    if len(icons) != ICON_COUNT or any(not icon.get("description") for icon in icons):
        raise SystemExit("Studio Icons snapshot no longer has 812 named, described icons")

    with tempfile.TemporaryDirectory(prefix="svg2vd-studio-icons-") as temporary:
        temporary = pathlib.Path(temporary)
        sources = temporary / "svg"
        supported = temporary / "supported"
        first = temporary / "generated"
        second = temporary / "generated-again"
        for directory in (sources, supported, first, second):
            directory.mkdir()

        print(f"Downloading {len(icons)} named Studio Icons SVGs", flush=True)
        with concurrent.futures.ThreadPoolExecutor(max_workers=24) as executor:
            downloads = list(executor.map(download_icon, enumerate(icons)))
        corpus_digest = hashlib.sha256()
        for index, icon, url, source in downloads:
            corpus_digest.update(url.encode())
            corpus_digest.update(b"\0")
            corpus_digest.update(source)
            corpus_digest.update(b"\0")
            (sources / f'{index:03}-{icon["name"]}.svg').write_bytes(source)
        observed_digest = corpus_digest.hexdigest()
        if observed_digest != CORPUS_SHA256:
            raise SystemExit(
                "Studio Icons SVG snapshot changed; inspect upstream before updating its "
                f"digest (observed {observed_digest})"
            )

        subprocess.run([CARGO, "build", "--release", "--locked"], cwd=ROOT, check=True)
        binary = ROOT / "target/release/svg2vd"
        checked = subprocess.run(
            [binary, "check", sources, "--format", "json"],
            check=False,
            stdout=subprocess.PIPE,
        )
        if checked.returncode != 2:
            raise RuntimeError(f"expected mixed compatibility exit 2, got {checked.returncode}")
        reports = json.loads(checked.stdout)
        compatibility = collections.Counter(report["compatibility"] for report in reports)
        convertible = compatibility["exact"] + compatibility["exact_with_normalization"]
        if len(reports) != ICON_COUNT or compatibility["approximate"] != 0:
            raise AssertionError("Studio Icons analysis returned incomplete or approximate results")
        if convertible < MINIMUM_CONVERTIBLE:
            raise AssertionError(
                f"convertible Studio Icons regressed from {MINIMUM_CONVERTIBLE} to {convertible}"
            )

        for report in reports:
            if report["compatibility"] in ("exact", "exact_with_normalization"):
                path = pathlib.Path(report["path"])
                shutil.copyfile(path, supported / path.name)
        subprocess.run([binary, "convert", supported, "-o", first], check=True)
        subprocess.run([binary, "convert", supported, "-o", second], check=True)
        first_paths = sorted(path.name for path in first.glob("*.xml"))
        second_paths = sorted(path.name for path in second.glob("*.xml"))
        if first_paths != second_paths or len(first_paths) != convertible:
            raise AssertionError("Studio Icons conversion produced an incomplete file set")
        for name in first_paths:
            if (first / name).read_bytes() != (second / name).read_bytes():
                raise AssertionError(f"{name}: conversion is not deterministic")

    diagnostics = collections.Counter(
        diagnostic["code"] for report in reports for diagnostic in report["diagnostics"]
    )
    print(f"{convertible} / {ICON_COUNT} Studio Icons are exactly convertible")
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
