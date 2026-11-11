#!/usr/bin/env bash
# Run one vdt command over every input on standard input, one at a time, with a
# wall-clock and a memory bound on each run.
#
# A sweep is how a whole corpus gets checked, and it is also how one bad input
# takes a machine down. `find ... | xargs -P8 vdt ...` starts eight processes
# that grow together, and an input that never finishes never releases what it
# holds: on 2026-11-10 three runaway `vdt compose --draw` processes outlived
# the tool call that started them, filled the compressor on a 64 GB Mac, and
# the kernel watchdog panicked it. A run here is serial and bounded, and an
# input that hits a bound is killed and named rather than left running.
#
#   find corpus -name '*.svg' -print0 | tools/sweep.sh --null compose --draw --name X
#
# Options before the vdt arguments are read by this script; everything after
# them is passed to vdt with the input path appended, so the line above runs
# `vdt compose --draw --name X <input>` for each file. vdt's standard output is
# discarded, because a sweep asks whether each input converts; pass `-o` in the
# arguments to keep the output of a run.
#
# Options:
#   --timeout SECONDS  wall clock one run may take (default 60)
#   --memory-mb MB     resident memory one run may reach (default 2048)
#   --binary PATH      the vdt to run (default target/release/vdt)
#   --null             read inputs NUL-separated, as `find -print0` writes them
#   --quiet            list only the inputs that failed or hit a bound
#   -h, --help         print this usage
#
# Exits 1 if any input hit a bound. An input that fails to convert is reported
# and counted but does not fail the sweep, because a sweep over artwork is
# expected to find some.
set -euo pipefail

usage() {
  sed -n '2,/^set -euo/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//; $d'
}

root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
timeout_seconds=60
memory_mb=2048
binary=target/release/vdt
null_separated=0
quiet=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --timeout) timeout_seconds=$2; shift 2 ;;
    --memory-mb) memory_mb=$2; shift 2 ;;
    --binary) binary=$2; shift 2 ;;
    --null) null_separated=1; shift ;;
    --quiet) quiet=1; shift ;;
    -h | --help) usage; exit 0 ;;
    --) shift; break ;;
    *) break ;;
  esac
done

if [[ $# -eq 0 ]]; then
  echo "sweep: no vdt arguments given" >&2
  usage >&2
  exit 2
fi

[[ "$binary" == /* ]] || binary="$root/$binary"
if [[ ! -x "$binary" ]]; then
  echo "sweep: $binary is not executable; run 'cargo build --release --locked'" >&2
  exit 2
fi

scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
: >"$scratch/failures"
: >"$scratch/killed"

ok=0
total=0

# Read one input path, NUL-separated when the caller asked for that.
read_input() {
  if (( null_separated )); then
    IFS= read -r -d '' "$@"
  else
    IFS= read -r "$@"
  fi
}

# Run one input under both bounds, and record how it ended.
sweep_one() {
  local input=$1
  shift
  local started=$SECONDS
  local reason=""

  "$binary" "$@" "$input" >/dev/null 2>"$scratch/error" &
  local pid=$!

  # Poll rather than trust a limit the platform may not enforce: `ulimit -m` is
  # a no-op on macOS, and RLIMIT_AS does not bound what a process can touch.
  while kill -0 "$pid" 2>/dev/null; do
    local rss_kb=""
    rss_kb=$(ps -o rss= -p "$pid" 2>/dev/null | tr -d ' ' || true)
    if [[ -n "$rss_kb" ]] && (( rss_kb > memory_mb * 1024 )); then
      reason="memory over ${memory_mb} MB"
      break
    fi
    if (( SECONDS - started >= timeout_seconds )); then
      reason="timeout over ${timeout_seconds}s"
      break
    fi
    sleep 0.1
  done

  if [[ -n "$reason" ]]; then
    kill "$pid" 2>/dev/null || true
    wait "$pid" 2>/dev/null || true
    printf '%s: %s\n' "$reason" "$input" >>"$scratch/killed"
    [[ $quiet -eq 1 ]] || printf 'killed (%s): %s\n' "$reason" "$input"
    return
  fi

  local status=0
  wait "$pid" || status=$?
  if (( status != 0 )); then
    local message
    message=$(tr '\n' ' ' <"$scratch/error" | cut -c1-200)
    printf '%s: %s\n' "$input" "$message" >>"$scratch/failures"
    [[ $quiet -eq 1 ]] || printf 'failed (%s): %s\n' "$status" "$input"
    return
  fi

  ok=$((ok + 1))
  [[ $quiet -eq 1 ]] || printf 'ok: %s\n' "$input"
}

while read_input input; do
  [[ -n "$input" ]] || continue
  total=$((total + 1))
  sweep_one "$input" "$@"
done

killed=$(wc -l <"$scratch/killed" | tr -d ' ')
failed=$(wc -l <"$scratch/failures" | tr -d ' ')

if (( failed > 0 )); then
  echo
  echo "failed inputs:"
  sed 's/^/  /' "$scratch/failures"
fi
if (( killed > 0 )); then
  echo
  echo "inputs that hit a bound:"
  sed 's/^/  /' "$scratch/killed"
fi

echo
echo "sweep: $total inputs, $ok ok, $failed failed, $killed hit a bound"

(( killed == 0 ))
