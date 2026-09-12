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

latest_dir() {
  for dir in "$1"/*/; do
    basename "$dir"
  done | sort -V | tail -1
}

PLATFORM_VERSION="${PLATFORM_VERSION:-$(latest_dir "$SDK/platforms")}"
BUILD_TOOLS_VERSION="${BUILD_TOOLS_VERSION:-$(latest_dir "$SDK/build-tools")}"
ANDROID_JAR="$SDK/platforms/$PLATFORM_VERSION/android.jar"
ANDROID_TEST_BASE="$SDK/platforms/$PLATFORM_VERSION/optional/android.test.base.jar"
BUILD_TOOLS="$SDK/build-tools/$BUILD_TOOLS_VERSION"

for tool in cargo javac keytool zip; do
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
  report=$("$ROOT/target/debug/svg2vd" inspect "$fixture" --format json)
  minimum_api=$(python3 -c 'import json,sys; print(json.load(sys.stdin)[0]["minimum_api"])' <<<"$report")
  case "$minimum_api" in
    21) qualifier=drawable ;;
    24) qualifier=drawable-v24 ;;
    *) echo "unexpected minimum API $minimum_api for $fixture" >&2; exit 1 ;;
  esac
  "$ROOT/target/debug/svg2vd" convert "$fixture" \
    --output "$BUILD/generated/res/$qualifier/$name.xml"
done

while IFS= read -r -d '' resource; do
  "$BUILD_TOOLS/aapt2" compile "$resource" -o "$BUILD/compiled"
done < <(find "$BUILD/generated/res" -type f -name '*.xml' -print0)

AAPT_RESOURCES=()
while IFS= read -r -d '' flat; do
  AAPT_RESOURCES+=(-R "$flat")
done < <(find "$BUILD/compiled" -type f -name '*.flat' -print0)
"$BUILD_TOOLS/aapt2" link -o "$BUILD/app-unsigned.apk" \
  --manifest "$HARNESS/app/AndroidManifest.xml" -I "$ANDROID_JAR" \
  --min-sdk-version 21 --target-sdk-version 35 "${AAPT_RESOURCES[@]}"
javac -source 8 -target 8 -Xlint:-options -classpath "$ANDROID_JAR" \
  -d "$BUILD/app-classes" "$HARNESS/app/src/com/svg2vd/renderer/Marker.java"
"$BUILD_TOOLS/d8" --lib "$ANDROID_JAR" --min-api 21 --output "$BUILD/app-dex" \
  $(find "$BUILD/app-classes" -type f -name '*.class' -print)
zip -q -j "$BUILD/app-unsigned.apk" "$BUILD/app-dex/classes.dex"

javac -source 8 -target 8 -Xlint:-options -classpath "$ANDROID_JAR:$ANDROID_TEST_BASE" \
  -d "$BUILD/test-classes" \
  "$HARNESS/test/src/com/svg2vd/renderer/test/RendererConformanceTest.java"
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
  output="$BUILD/svg2vd-renderer.apk"
  [[ "$apk" == test ]] && output="$BUILD/svg2vd-renderer-test.apk"
  "$BUILD_TOOLS/apksigner" sign --ks "$BUILD/debug.keystore" \
    --ks-pass pass:android --key-pass pass:android --min-sdk-version 21 \
    --out "$output" "$BUILD/$apk-unsigned.apk"
  "$BUILD_TOOLS/apksigner" verify --verbose "$output" >/dev/null
done

printf 'Built:\n  %s\n  %s\n' \
  "$BUILD/svg2vd-renderer.apk" "$BUILD/svg2vd-renderer-test.apk"
