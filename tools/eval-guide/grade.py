#!/usr/bin/env python3
"""Grade the runs setup.sh prepared, from the files each agent left behind
and the `vdt` commands its shim logged.

usage: tools/eval-guide/grade.py [RUN...]

Writes $VDT_EVAL_DIR/results.json and prints a score per run and a mean per
condition. Every check but the report check is pass or fail; the report check
scores how many of the hero's four problems REPORT.md names.
"""
import glob, hashlib, json, os, re, shlex, statistics, subprocess, sys

E = os.environ.get("VDT_EVAL_DIR", "/tmp/vdt-eval")
VDT = f"{E}/bin-new/vdt"
FIXTURE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "fixture")
RES = "app/src/main/res"


def sha(path):
    try:
        return hashlib.sha256(open(path, "rb").read()).hexdigest()
    except FileNotFoundError:
        return None


def read(path):
    try:
        return open(path, encoding="utf-8", errors="replace").read()
    except FileNotFoundError:
        return ""


def last(log, command):
    lines = [l for l in log if l.split()[:1] == [command] and "--help" not in l.split()]
    return lines[-1] if lines else None


def opt(line, flag, default=None):
    words = shlex.split(line)
    for i, w in enumerate(words):
        if w == flag and i + 1 < len(words):
            return words[i + 1]
        if w.startswith(flag + "="):
            return w.split("=", 1)[1]
    return default


def fit_is_safe(fit):
    out = subprocess.run(
        [VDT, "inspect", f"{FIXTURE}/design/logo.svg", "--as", "adaptive-foreground",
         "--fit", str(fit), "--format", "json"], capture_output=True, text=True).stdout
    report = json.loads(out)[0]
    return not any(d["code"] == "VDT019" and d["severity"] == "warning"
                   for d in report["diagnostics"])


def grade(run):
    R = f"{E}/runs/{run}"
    log = [l.strip() for l in read(f"{R}/.vdt-log").splitlines() if l.strip()]
    report = read(f"{R}/REPORT.md")
    res = f"{R}/{RES}"
    g = {}

    # Task 1: launcher icon
    adaptive = last(log, "adaptive")
    name = opt(adaptive, "--name", "ic_launcher") if adaptive else "ic_launcher"
    anydpi = read(f"{res}/mipmap-anydpi-v26/{name}.xml")
    g["t1_generated"] = adaptive is not None and "<adaptive-icon" in anydpi
    g["t1_themed"] = "monochrome" in anydpi
    g["t1_background"] = bool(re.search(r"1e88e5", "".join(
        read(p) for p in glob.glob(f"{res}/**/*.xml", recursive=True)), re.I))
    g["t1_fit_safe"] = bool(adaptive) and fit_is_safe(float(opt(adaptive, "--fit", "108")))
    g["t1_legacy"] = bool(adaptive) and "--legacy" in adaptive.split()
    webps = [p.replace(FIXTURE, R) for p in glob.glob(f"{FIXTURE}/{RES}/mipmap-*/*.webp")]
    kept = all(sha(p) == sha(p.replace(R, FIXTURE)) for p in webps)
    flagged = bool(re.search(
        r"mipmap-(x*h|m)dpi|\.webp|density-specific|template|(existing|old|stale|legacy)[^.\n]{0,40}(bitmap|webp|launcher|icon)",
        report, re.I))
    # With --legacy and the default name, vdt itself removes the template
    # bitmaps; what matters is that the agent tells the user what happened to them.
    g["t1_old_bitmaps_reported"] = flagged
    g["t1_old_bitmaps_kept"] = kept
    manifest = read(f"{R}/app/src/main/AndroidManifest.xml")
    g["t1_manifest_round"] = f'android:roundIcon="@mipmap/{name}_round"' in manifest \
        and f'android:icon="@mipmap/{name}"' in manifest
    g["t1_logo_unchanged"] = sha(f"{R}/design/logo.svg") == sha(f"{FIXTURE}/design/logo.svg")

    # Task 2: notification icon
    notification = last(log, "notification")
    kt = read(f"{R}/app/src/main/java/com/example/pulse/NotificationHelper.kt")
    m = re.search(r"setSmallIcon\(\s*R\.drawable\.(\w+)", kt)
    name = m.group(1) if m else None
    g["t2_generated"] = notification is not None
    g["t2_wired"] = bool(name) and bool(glob.glob(f"{res}/drawable*/{name}.xml"))
    g["t2_fit_20"] = bool(notification) and opt(notification, "--fit") in ("20", "20.0")
    g["t2_ic_stat_name"] = bool(name) and name.startswith("ic_stat_")

    # Task 3: hero illustration that cannot convert
    g["t3_artwork_unchanged"] = sha(f"{R}/design/hero.svg") == sha(f"{FIXTURE}/design/hero.svg") \
        and not [p for p in glob.glob(f"{R}/design/*") if os.path.basename(p) not in
                 ("logo.svg", "bell.svg", "hero.svg")]
    g["t3_no_drawable_faked"] = not glob.glob(f"{res}/drawable*/onboarding_hero.*")
    # Pointing code at a resource that does not exist breaks the build.
    screen = read(f"{R}/app/src/main/java/com/example/pulse/OnboardingScreen.kt")
    g["t3_no_dangling_reference"] = "R.drawable.onboarding_hero" not in screen \
        or bool(glob.glob(f"{res}/drawable*/onboarding_hero.xml"))
    mentions = {
        "text": r"VDT006|\btext\b",
        "filter": r"VDT002|shadow|filter",
        "focal": r"VDT003|focal",
        "size": r"VDT016|200\s*(x|×|dp)|200dp|--size",
    }
    g["t3_report_covers"] = sum(bool(re.search(p, report, re.I)) for p in mentions.values()) / 4

    g["used_guide"] = sum(l.split()[0] == "guide" for l in log)
    g["used_explain"] = sum(l.split()[0] == "explain" for l in log)
    g["used_help"] = sum("--help" in l.split() or l.split()[0] == "help" for l in log)
    g["vdt_calls"] = len(log)
    g["report_written"] = bool(report)
    return g


CORE = ["t1_generated", "t1_themed", "t1_background", "t1_fit_safe", "t1_legacy",
        "t1_old_bitmaps_reported", "t1_manifest_round", "t1_logo_unchanged",
        "t2_generated", "t2_wired", "t2_fit_20", "t2_ic_stat_name",
        "t3_artwork_unchanged", "t3_no_drawable_faked", "t3_no_dangling_reference"]

if __name__ == "__main__":
    runs = sys.argv[1:] or sorted(
        name for name in os.listdir(f"{E}/runs") if os.path.isdir(f"{E}/runs/{name}"))
    results = {r: grade(r) for r in runs}
    json.dump(results, open(f"{E}/results.json", "w"), indent=2)
    for r, g in results.items():
        score = sum(bool(g[k]) for k in CORE) + g["t3_report_covers"]
        print(f"{r:12} {score:5.2f}/{len(CORE) + 1}  guide={g['used_guide']} explain={g['used_explain']} "
              f"help={g['used_help']} calls={g['vdt_calls']}  fails: "
              + ", ".join(k for k in CORE if not g[k]))

    by_condition = {}
    for r, g in results.items():
        score = sum(bool(g[k]) for k in CORE) + g["t3_report_covers"]
        by_condition.setdefault(r.rsplit("-", 1)[-1], []).append((score, g["vdt_calls"]))
    print()
    for condition, rows in sorted(by_condition.items()):
        scores = [score for score, _ in rows]
        print(f"{condition:5} mean {statistics.mean(scores):5.2f}  "
              f"perfect {sum(s == len(CORE) + 1 for s in scores)}/{len(scores)}  "
              f"mean vdt calls {statistics.mean(c for _, c in rows):.1f}")
