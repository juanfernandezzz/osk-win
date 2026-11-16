"""Compare existing release executables; never opens a window or injects input.

Uses the application's --render/--bench-frames path (measurement, drawing and
RGBA-to-BGRA flush; NOT BitBlt, window input, startup-to-first-paint or dictation).
Every subprocess must succeed, report the requested work and produce identical
PNG bytes. Benchmark data includes process wall time as a separate coarse metric.
Run with --baseline PATH --candidate PATH --output DIRECTORY.
"""
import argparse
import hashlib
import json
import re
import statistics
import subprocess
import time
from pathlib import Path

PATTERN = re.compile(r"bench: (\d+) frames, frame median ([\d.]+) ms \(min ([\d.]+), max ([\d.]+)\)")
THEMES = ["Windows", "Default", "Tokyo", "Catppuccin", "Nord", "Gruvbox",
          "Rose Pine", "Dracula", "Everforest", "Kanagawa", "Solarized", "Cyberpunk", "Abyss"]
LAYOUTS = ["Latino", "65%", "65%+Num"]

def run(exe, output, args, frames=0):
    output.unlink(missing_ok=True)
    command = [str(exe), "--render", str(output), *args]
    if frames:
        command += ["--bench-frames", str(frames)]
    start = time.perf_counter()
    p = subprocess.run(command, capture_output=True, text=True, encoding="utf-8",
                       errors="replace", timeout=120)
    wall_ms = (time.perf_counter() - start) * 1000
    if p.returncode or not output.is_file():
        raise RuntimeError(f"Failed ({p.returncode}): {command}\n{p.stderr}")
    result = {"wall_ms": wall_ms, "sha256": hashlib.sha256(output.read_bytes()).hexdigest()}
    if frames:
        match = PATTERN.search(p.stderr)
        if not match or int(match[1]) != frames:
            raise RuntimeError(f"Missing work count: {p.stderr}")
        result.update(median_ms=float(match[2]), min_ms=float(match[3]), max_ms=float(match[4]))
    return result

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--baseline", type=Path, required=True)
    parser.add_argument("--candidate", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--frames", type=int, default=500)
    parser.add_argument("--matrix", action="store_true")
    opts = parser.parse_args()
    if opts.runs < 1 or opts.frames < 1:
        parser.error("runs and frames must be positive")
    opts.output.mkdir(parents=True, exist_ok=True)
    executables = {"baseline": opts.baseline.resolve(), "candidate": opts.candidate.resolve()}
    report = {"executables": {k: {"sha256": hashlib.sha256(v.read_bytes()).hexdigest(),
                                "size": v.stat().st_size} for k, v in executables.items()},
              "runs": opts.runs, "frames": opts.frames, "scenarios": {}, "matrix": []}
    cases = {"hover_800": ["--layout", "Latino", "--size", "800x300", "--hover", "A"],
             "menu_800": ["--layout", "Latino", "--size", "800x300", "--menu"],
             "hover_1440": ["--layout", "Latino", "--size", "1440x540", "--hover", "A"],
             "pressed_800": ["--layout", "Latino", "--size", "800x300", "--pressed", "A"]}
    for case, args in cases.items():
        samples = {"baseline": [], "candidate": []}
        for trial in range(opts.runs):
            for side in (["baseline", "candidate"] if trial % 2 == 0 else ["candidate", "baseline"]):
                samples[side].append(run(executables[side], opts.output / f"{case}-{side}.png", args, opts.frames))
        hashes = {s["sha256"] for values in samples.values() for s in values}
        if len(hashes) != 1:
            raise AssertionError(f"Image regression in {case}")
        summary = {side: {"median_ms": statistics.median(x["median_ms"] for x in values),
                          "range_ms": [min(x["median_ms"] for x in values), max(x["median_ms"] for x in values)]}
                   for side, values in samples.items()}
        report["scenarios"][case] = {"args": args, "samples": samples, "summary": summary}
        print(case, json.dumps(summary), flush=True)
    if opts.matrix:
        states = [[], ["--hover", "A"], ["--pressed", "A"], ["--menu"],
                  ["--mods", "shift=latched,ctrl=locked,caps"], ["--hover", "handle", "--anim", "0.5"]]
        for layout in LAYOUTS:
            for theme in THEMES:
                for index, state in enumerate(states):
                    # Include very small and asymmetric canvases, not just normal sizes.
                    size = ["800x300", "200x180", "1440x540", "400x700", "721x341", "801x301"][index]
                    args = ["--layout", layout, "--theme", theme, "--size", size, *state]
                    results = [run(exe, opts.output / f"matrix-{side}.png", args)
                               for side, exe in executables.items()]
                    if results[0]["sha256"] != results[1]["sha256"]:
                        raise AssertionError(f"Image regression: {args}")
                    report["matrix"].append({"args": args, "sha256": results[0]["sha256"]})
        print(f"Equivalent images: {len(report['matrix'])}", flush=True)
    (opts.output / "results.json").write_text(json.dumps(report, indent=2), encoding="utf-8")

if __name__ == "__main__":
    main()
