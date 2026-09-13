#!/usr/bin/env bash
set -euo pipefail

ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
HARNESS="$ROOT/tools/android-renderer"
BUILD="$HARNESS/build"
SDK="${ANDROID_HOME:-${ANDROID_SDK_ROOT:-$HOME/Android/Sdk}}"

if [[ -f "$HOME/.cargo/env" ]]; then
  # shellcheck disable=SC1091
  source "$HOME/.cargo/env"
fi
if ! command -v cargo >/dev/null && command -v rustup >/dev/null; then
  PATH="$(dirname "$(rustup which cargo)"):$PATH"
fi

# Newest directory whose name matches a glob, ignoring aliases such as `latest`.
latest_dir() {
  for dir in "$1"/$2/; do
    basename "$dir"
  done | sort -V | tail -1
}

PLATFORM_VERSION="${PLATFORM_VERSION:-$(latest_dir "$SDK/platforms" 'android-*')}"
BUILD_TOOLS_VERSION="${BUILD_TOOLS_VERSION:-$(latest_dir "$SDK/build-tools" '[0-9]*')}"

# Material Symbols used as adaptive icon layers, fetched from the pinned
# conformance corpus commit; see fixtures/adaptive/SOURCES.md.
MATERIAL_COMMIT=0cbb08816df07faaae3dca060d4ebb10b66c214f
MATERIAL_RAW="https://raw.githubusercontent.com/google/material-design-icons/$MATERIAL_COMMIT"
ADAPTIVE_FOREGROUND=symbols/web/stars/materialsymbolsoutlined/stars_24px.svg
ADAPTIVE_MONOCHROME=symbols/web/rocket_launch/materialsymbolsoutlined/rocket_launch_24px.svg
ANDROID_JAR="$SDK/platforms/$PLATFORM_VERSION/android.jar"
ANDROID_TEST_BASE="$SDK/platforms/$PLATFORM_VERSION/optional/android.test.base.jar"
BUILD_TOOLS="$SDK/build-tools/$BUILD_TOOLS_VERSION"

for tool in cargo javac keytool zip curl; do
  command -v "$tool" >/dev/null || { echo "missing required tool: $tool" >&2; exit 1; }
done
for file in "$ANDROID_JAR" "$ANDROID_TEST_BASE" "$BUILD_TOOLS/aapt2" "$BUILD_TOOLS/d8" "$BUILD_TOOLS/apksigner"; do
  [[ -e "$file" ]] || { echo "missing Android SDK file: $file" >&2; exit 1; }
done

rm -rf "$BUILD"
mkdir -p "$BUILD/generated/res/drawable" "$BUILD/generated/res/drawable-v24" \
  "$BUILD/compiled" "$BUILD/app-classes" "$BUILD/app-dex" \
  "$BUILD/test-classes" "$BUILD/test-dex"

cargo build --locked --manifest-path "$ROOT/Cargo.toml"
for fixture in "$HARNESS"/fixtures/*.svg; do
  name=$(basename "$fixture" .svg)
  report=$("$ROOT/target/debug/vdt" inspect "$fixture" --format json)
  minimum_api=$(python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["minimum_api"])' <<<"$report")
  case "$minimum_api" in
    21) qualifier=drawable ;;
    24) qualifier=drawable-v24 ;;
    *) echo "unexpected minimum API $minimum_api for $fixture" >&2; exit 1 ;;
  esac
  "$ROOT/target/debug/vdt" convert "$fixture" \
    --output "$BUILD/generated/res/$qualifier/$name.xml"
done

# Adaptive launcher icon: Material Symbols layers fitted to the 66dp safe zone
# over a drawable background, plus a variant with a solid color background.
mkdir -p "$BUILD/adaptive-src"
for layer in "$ADAPTIVE_FOREGROUND" "$ADAPTIVE_MONOCHROME"; do
  curl -sSfL "$MATERIAL_RAW/$layer" -o "$BUILD/adaptive-src/$(basename "$layer")"
done
"$ROOT/target/debug/vdt" adaptive \
  --foreground "$BUILD/adaptive-src/$(basename "$ADAPTIVE_FOREGROUND")" \
  --background "$HARNESS/fixtures/adaptive/background.svg" \
  --monochrome "$BUILD/adaptive-src/$(basename "$ADAPTIVE_MONOCHROME")" \
  --fit 66 --name ic_launcher --legacy --output "$BUILD/generated/res"
"$ROOT/target/debug/vdt" adaptive \
  --foreground "$BUILD/adaptive-src/$(basename "$ADAPTIVE_FOREGROUND")" \
  --background-color '#073042' \
  --fit 66 --name ic_launcher_solid --output "$BUILD/generated/res"

while IFS= read -r -d '' resource; do
  "$BUILD_TOOLS/aapt2" compile "$resource" -o "$BUILD/compiled"
done < <(find "$BUILD/generated/res" -type f -name '*.xml' -print0)

AAPT_RESOURCES=()
while IFS= read -r -d '' flat; do
  AAPT_RESOURCES+=("$flat")
done < <(find "$BUILD/compiled" -type f -name '*.flat' -print0)
"$BUILD_TOOLS/aapt2" link -o "$BUILD/app-unsigned.apk" \
  --manifest "$HARNESS/app/AndroidManifest.xml" -I "$ANDROID_JAR" \
  --min-sdk-version 21 --target-sdk-version 35 "${AAPT_RESOURCES[@]}"
javac -source 8 -target 8 -Xlint:-options -classpath "$ANDROID_JAR" \
  -d "$BUILD/app-classes" $(find "$HARNESS/app/src" -type f -name '*.java' -print)
"$BUILD_TOOLS/d8" --lib "$ANDROID_JAR" --min-api 21 --output "$BUILD/app-dex" \
  $(find "$BUILD/app-classes" -type f -name '*.class' -print)
zip -q -j "$BUILD/app-unsigned.apk" "$BUILD/app-dex/classes.dex"

javac -source 8 -target 8 -Xlint:-options -classpath "$ANDROID_JAR:$ANDROID_TEST_BASE" \
  -d "$BUILD/test-classes" $(find "$HARNESS/test/src" -type f -name '*.java' -print)
"$BUILD_TOOLS/d8" --lib "$ANDROID_JAR" --lib "$ANDROID_TEST_BASE" \
  --min-api 21 --output "$BUILD/test-dex" \
  $(find "$BUILD/test-classes" -type f -name '*.class' -print)
"$BUILD_TOOLS/aapt2" link -o "$BUILD/test-unsigned.apk" \
  --manifest "$HARNESS/test/AndroidManifest.xml" -I "$ANDROID_JAR" \
  --min-sdk-version 21 --target-sdk-version 35
zip -q -j "$BUILD/test-unsigned.apk" "$BUILD/test-dex/classes.dex"

keytool -genkeypair -keystore "$BUILD/debug.keystore" -storepass android \
  -keypass android -alias androiddebugkey -dname 'CN=Android Debug,O=Android,C=US' \
  -keyalg RSA -keysize 2048 -validity 10000 >/dev/null 2>&1
for apk in app test; do
  output="$BUILD/vdtoolkit-renderer.apk"
  [[ "$apk" == test ]] && output="$BUILD/vdtoolkit-renderer-test.apk"
  "$BUILD_TOOLS/apksigner" sign --ks "$BUILD/debug.keystore" \
    --ks-pass pass:android --key-pass pass:android --min-sdk-version 21 \
    --out "$output" "$BUILD/$apk-unsigned.apk"
  "$BUILD_TOOLS/apksigner" verify --verbose "$output" >/dev/null
done

printf 'Built:\n  %s\n  %s\n' \
  "$BUILD/vdtoolkit-renderer.apk" "$BUILD/vdtoolkit-renderer-test.apk"
