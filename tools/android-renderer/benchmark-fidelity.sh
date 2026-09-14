#!/usr/bin/env bash
# Prepare the pinned three-converter Android screenshot benchmark.
# Set ANDROID_SERIAL to also install and capture the four 25-icon pages.
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
WORK="${FIDELITY_WORKDIR:-/tmp/vdtoolkit-android-fidelity}"
CORPUS="$WORK/corpus"
ASHUNG="$WORK/svg2vectordrawable"
ASHUNG_XML="$WORK/ashung-xml"
AOSP="$WORK/aosp-cli"
AOSP_XML="$WORK/aosp-xml"
AOSP_SOURCE="$WORK/aosp-tools-base"
MATERIAL_COMMIT=0cbb08816df07faaae3dca060d4ebb10b66c214f
ASHUNG_COMMIT=332f56d488e660c0c2d6c3bbc47fd9bd316006d2
AOSP_COMMIT=cab65fa60a996079b6f6d9b62025df3d98c135bc

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
for tool in cargo node npm java mvn python3 curl; do
  command -v "$tool" >/dev/null || { echo "missing $tool; run .agents/setup first" >&2; exit 1; }
done
mkdir -p "$WORK"

python3 - "$ROOT" "$CORPUS" "$MATERIAL_COMMIT" <<'PY'
from pathlib import Path
from urllib.request import urlopen
import sys
root, corpus = map(Path, sys.argv[1:3])
commit = sys.argv[3]
corpus.mkdir(parents=True, exist_ok=True)
base = f"https://raw.githubusercontent.com/google/material-design-icons/{commit}/"
paths = [line.strip() for line in (root / "tests/material-symbols.txt").read_text().splitlines()
         if line.strip() and not line.startswith("#")]
for index, path in enumerate(paths):
    output = corpus / f"{index:03d}-{Path(path).name}"
    output = output.with_suffix(".svg")
    if not output.exists(): output.write_bytes(urlopen(base + path).read())
PY

if [[ ! -d "$ASHUNG/.git" ]]; then git clone -q https://github.com/Ashung/svg2vectordrawable "$ASHUNG"; fi
git -C "$ASHUNG" fetch -q --depth 1 origin "$ASHUNG_COMMIT"
git -C "$ASHUNG" checkout -q --detach "$ASHUNG_COMMIT"
(cd "$ASHUNG" && npm ci --ignore-scripts --no-audit --no-fund)
mkdir -p "$ASHUNG_XML"
node - "$ASHUNG" "$CORPUS" "$ASHUNG_XML" <<'NODE'
const fs = require('fs/promises'); const path = require('path');
(async () => {
  const [tool, corpus, output] = process.argv.slice(2); const convert = require(tool);
  for (const name of (await fs.readdir(corpus)).filter(n => n.endsWith('.svg')).sort()) {
    const svg = await fs.readFile(path.join(corpus, name), 'utf8');
    const xml = await convert(svg, {floatPrecision: 6, strict: false, fillBlack: false, xmlTag: false});
    await fs.writeFile(path.join(output, name.replace(/\.svg$/, '.xml')), xml);
  }
})().catch(error => { console.error(error); process.exit(1); });
NODE

if [[ ! -d "$AOSP_SOURCE/.git" ]]; then git init -q "$AOSP_SOURCE"; fi
git -C "$AOSP_SOURCE" fetch -q --depth 1 https://android.googlesource.com/platform/tools/base "$AOSP_COMMIT"
mkdir -p "$AOSP/src/main/java/com/android/ide/common/vectordrawable"
for file in VdCommandLineTool VdCommandLineOptions; do
  git -C "$AOSP_SOURCE" show "$AOSP_COMMIT:vector-drawable-tool/src/main/java/com/android/ide/common/vectordrawable/$file.java" \
    >"$AOSP/src/main/java/com/android/ide/common/vectordrawable/$file.java"
done
cat >"$AOSP/pom.xml" <<'XML'
<project xmlns="http://maven.apache.org/POM/4.0.0"><modelVersion>4.0.0</modelVersion><groupId>benchmark</groupId><artifactId>aosp-svg2vector</artifactId><version>1</version><properties><maven.compiler.release>17</maven.compiler.release></properties><repositories><repository><id>google</id><url>https://maven.google.com</url></repository></repositories><dependencies><dependency><groupId>com.android.tools</groupId><artifactId>sdk-common</artifactId><version>31.11.0-alpha10</version></dependency><dependency><groupId>com.android.tools</groupId><artifactId>common</artifactId><version>31.11.0-alpha10</version></dependency><dependency><groupId>com.android.tools</groupId><artifactId>annotations</artifactId><version>31.11.0-alpha10</version></dependency><dependency><groupId>com.google.guava</groupId><artifactId>guava</artifactId><version>33.3.1-jre</version></dependency></dependencies><build><plugins><plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-compiler-plugin</artifactId><version>3.11.0</version><configuration><release>17</release></configuration></plugin><plugin><groupId>org.apache.maven.plugins</groupId><artifactId>maven-dependency-plugin</artifactId><version>3.6.1</version></plugin></plugins></build></project>
XML
(cd "$AOSP" && mvn -q package dependency:build-classpath -Dmdep.outputFile=classpath)
rm -rf "$AOSP_XML" && mkdir -p "$AOSP_XML"
java -cp "$AOSP/target/classes:$(cat "$AOSP/classpath")" com.android.ide.common.vectordrawable.VdCommandLineTool -c -in "$CORPUS" -out "$AOSP_XML"

FIDELITY_CORPUS="$CORPUS" FIDELITY_ASHUNG_OUTPUTS="$ASHUNG_XML" FIDELITY_AOSP_OUTPUTS="$AOSP_XML" \
  "$ROOT/tools/android-renderer/build.sh"

if [[ -n "${ANDROID_SERIAL:-}" ]]; then
  adb -s "$ANDROID_SERIAL" install -r "$ROOT/tools/android-renderer/build/vdtoolkit-renderer.apk" >/dev/null
  output="$WORK/screenshots"; mkdir -p "$output"
  for page in 0 1 2 3; do for surface in source vdt ashung aosp; do
    adb -s "$ANDROID_SERIAL" shell am force-stop com.vdtoolkit.renderer
    adb -s "$ANDROID_SERIAL" shell am start -n com.vdtoolkit.renderer/.MainActivity --es surface "$surface" --ei page "$page" >/dev/null
    sleep 2; adb -s "$ANDROID_SERIAL" exec-out screencap -p >"$output/$surface-$page.png"
  done; done
  echo "Captured 16 screenshots in $output"
else
  echo "APK built. Set ANDROID_SERIAL and rerun to capture the Android surfaces."
fi
