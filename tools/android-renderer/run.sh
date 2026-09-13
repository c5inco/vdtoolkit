#!/usr/bin/env bash
# Run the renderer harness APKs built by build.sh.
#
# On emulator.wtf when EW_API_TOKEN is set (override with RUNNER=adb or
# RUNNER=ew), otherwise on the adb device selected by ANDROID_SERIAL or the
# only connected device. Results, logs, and launcher screenshots are written
# below build/run/.
#
#   EW_DEVICES   semicolon-separated ew-cli device specs
#                (default: Pixel7 on API 21, 24, 26, 33, and 36)
set -euo pipefail

HARNESS=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
BUILD="$HARNESS/build"
APP="$BUILD/vdtoolkit-renderer.apk"
TEST="$BUILD/vdtoolkit-renderer-test.apk"
PACKAGE=com.vdtoolkit.renderer
RUNNER_CLASS=android.test.InstrumentationTestRunner
SCREENSHOTS="/sdcard/Android/data/$PACKAGE/files/screenshots"
OUT="$BUILD/run"
EW_DEVICES="${EW_DEVICES:-model=Pixel7,version=21;model=Pixel7,version=24;model=Pixel7,version=26;model=Pixel7,version=33;model=Pixel7,version=36}"

for apk in "$APP" "$TEST"; do
  [[ -f "$apk" ]] || { echo "missing $apk; run build.sh first" >&2; exit 1; }
done
rm -rf "$OUT"
mkdir -p "$OUT"

RUNNER="${RUNNER:-}"
if [[ -z "$RUNNER" ]]; then
  if [[ -n "${EW_API_TOKEN:-}" ]] && command -v ew-cli >/dev/null; then
    RUNNER=ew
  else
    RUNNER=adb
  fi
fi

case "$RUNNER" in
  ew)
    DEVICE_ARGS=()
    IFS=';' read -ra devices <<<"$EW_DEVICES"
    for device in "${devices[@]}"; do
      DEVICE_ARGS+=(--device "$device")
    done
    echo "Running on emulator.wtf: ${devices[*]}"
    ew-cli --app "$APP" --test "$TEST" "${DEVICE_ARGS[@]}" \
      --display-name "vdtoolkit renderer harness" \
      --directories-to-pull "$SCREENSHOTS" \
      --outputs summary,merged_results_xml,results_xml,logcat,pulled_dirs \
      --outputs-dir "$OUT" --no-test-cache --record-video
    ;;
  adb)
    if [[ -z "${ANDROID_SERIAL:-}" ]]; then
      mapfile -t serials < <(adb devices | awk 'NR > 1 && $2 == "device" { print $1 }')
      if [[ ${#serials[@]} -ne 1 ]]; then
        echo "set ANDROID_SERIAL; connected devices: ${serials[*]:-none}" >&2
        exit 1
      fi
      export ANDROID_SERIAL="${serials[0]}"
    fi
    api=$(adb shell getprop ro.build.version.sdk | tr -d '\r')
    echo "Running on $ANDROID_SERIAL (API $api)"
    # build.sh signs with a fresh key each time, so replace rather than update.
    adb uninstall "$PACKAGE.test" >/dev/null 2>&1 || true
    adb uninstall "$PACKAGE" >/dev/null 2>&1 || true
    adb install -r "$APP" >/dev/null
    adb install -r "$TEST" >/dev/null
    adb shell am instrument -w "$PACKAGE.test/$RUNNER_CLASS" | tee "$OUT/instrument-api$api.txt"
    adb pull "$SCREENSHOTS" "$OUT/" >/dev/null 2>&1 || echo "no screenshots pulled from $SCREENSHOTS"
    grep -q '^OK (' "$OUT/instrument-api$api.txt"
    ;;
  *)
    echo "unknown RUNNER=$RUNNER (expected ew or adb)" >&2
    exit 1
    ;;
esac

echo "Outputs in $OUT"
find "$OUT" -name '*.png' | sort
