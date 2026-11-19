"""Sample headless Windows process memory/GDI objects; separate from timing runs.

Usage: python tools/perf_memory.py BASELINE CANDIDATE OUTPUT_DIR [REFERENCE_DIR]
For an intentional UI change, supply reviewed PNGs named SIDE-WxH.png
for each side and size. The default requires identical output across sides.
Reports sampled peaks, not allocation counts or whole desktop UI memory.
Requires Pillow for decoding and validating the PNG output.
"""
import ctypes as c
from ctypes import wintypes as w
import json
import hashlib
import os
import re
import platform
from pathlib import Path
import subprocess
import sys
import time
import tempfile
from PIL import Image, __version__ as pillow_version

class Counters(c.Structure):
    _fields_ = [("cb", w.DWORD), ("PageFaultCount", w.DWORD)] + [
        (name, c.c_size_t) for name in ["PeakWorkingSetSize", "WorkingSetSize",
        "QuotaPeakPagedPoolUsage", "QuotaPagedPoolUsage", "QuotaPeakNonPagedPoolUsage",
        "QuotaNonPagedPoolUsage", "PagefileUsage", "PeakPagefileUsage", "PrivateUsage"]]

get_memory = c.WinDLL("psapi", use_last_error=True).GetProcessMemoryInfo
get_memory.argtypes = [w.HANDLE, c.POINTER(Counters), w.DWORD]
get_memory.restype = w.BOOL
get_gdi = c.WinDLL("user32", use_last_error=True).GetGuiResources
get_gdi.argtypes = [w.HANDLE, w.DWORD]
get_gdi.restype = w.DWORD

get_times = c.WinDLL("kernel32", use_last_error=True).GetProcessTimes
get_times.argtypes = [w.HANDLE] + [c.POINTER(w.FILETIME)] * 4
get_times.restype = w.BOOL

def image_identity(path, size):
    with Image.open(path) as image:
        if image.format != "PNG" or image.size != tuple(map(int, size.split("x"))):
            raise ValueError(f"Invalid PNG dimensions/format: {path}")
        image.verify()
    with Image.open(path) as image:
        pixels = image.convert("RGBA").tobytes()
    return {"image_sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
            "pixels_sha256": hashlib.sha256(pixels).hexdigest(), "pixel_bytes": len(pixels)}

def gdi_after_final_output(error, stderr_bytes, observed_peak):
    # The shipped CLI prints this line after render() returns and drops its GDI
    # surfaces. Require the entire line to predate the failing syscall.
    lines = stderr_bytes.splitlines()
    return (error["api"] == "GetGuiResources" and error["winerror"] == 87
            and observed_peak > 0 and error["stderr_bytes_before_sample"] == len(stderr_bytes)
            and stderr_bytes.endswith(b"\n") and bool(lines) and lines[-1].startswith(b"osk: wrote "))

def main():
    baseline, candidate, directory, *references = map(Path, sys.argv[1:])
    if len(references) > 1:
        raise ValueError("Expected at most one reference image directory")
    directory.mkdir(parents=True, exist_ok=True)
    results = []
    executables = {"baseline": baseline.resolve(), "candidate": candidate.resolve()}
    env = dict(os.environ, OSK_CONFIG=str((directory / "isolated-config.json").resolve()))
    for key in ["OSK_THEME", "OSK_LAYOUT"]:
        env.pop(key, None)
    report = {"schema": 2, "platform": platform.platform(), "pillow": pillow_version, "sample_interval_ms": 10,
              "executables": {side: {"path": str(exe), "size": exe.stat().st_size,
                                     "sha256": hashlib.sha256(exe.read_bytes()).hexdigest()}
                              for side, exe in executables.items()},
              "runs": results, "excluded_pairs": [], "valid": False}
    report_path = directory / "memory.json"
    def save():
        report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    save()
    expected_images = {}
    for size in ["800x300", "1440x540"]:
        if references:
            for side in executables:
                expected_images[(side, size)] = image_identity(references[0] / f"{side}-{size}.png", size)["pixels_sha256"]
        trial = 0
        attempt = 1
        while trial < 5:
            pair_start = len(results)
            sides = [("baseline", baseline), ("candidate", candidate)]
            for side, exe in (sides if trial % 2 == 0 else sides[::-1]):
                output = directory / f"memory-{side}.png"
                output.unlink(missing_ok=True)
                command = [str(exe.resolve()), "--render", str(output.resolve()),
                           "--size", size, "--layout", "Latino", "--theme", "default", "--menu", "--bench-frames", "1000"]
                peak = {"working_set": 0, "private_bytes": 0, "gdi_objects": 0}
                samples = 0
                attempts = 0
                errors = []
                start = time.monotonic()
                record = dict(side=side, size=size, trial=trial, pair_attempt=attempt, command=command,
                              sampling_errors=errors, valid=False)
                results.append(record)
                with tempfile.TemporaryFile() as stderr_file, subprocess.Popen(
                        command, stdout=subprocess.DEVNULL, stderr=stderr_file, env=env) as p:
                    deadline = time.monotonic() + 120
                    while p.poll() is None:
                        if time.monotonic() > deadline:
                            p.kill()
                            record["error"] = "timeout"
                            save()
                            raise TimeoutError(command)
                        attempts += 1
                        counters = Counters(cb=c.sizeof(Counters))
                        # CPython's Windows process handle remains open for this scope.
                        if get_memory(int(p._handle), c.byref(counters), c.sizeof(counters)):
                            samples += 1
                            peak["working_set"] = max(peak["working_set"], counters.WorkingSetSize)
                            peak["private_bytes"] = max(peak["private_bytes"], counters.PrivateUsage)
                        else:
                            errors.append({"api": "GetProcessMemoryInfo", "winerror": c.get_last_error(),
                                           "elapsed_ms": (time.monotonic() - start) * 1000,
                                           "exited": p.poll() is not None})
                        log_bytes = os.fstat(stderr_file.fileno()).st_size
                        c.set_last_error(0)
                        gdi = get_gdi(int(p._handle), 0)
                        error = c.get_last_error()
                        if not gdi and error:
                            errors.append({"api": "GetGuiResources", "winerror": error,
                                           "stderr_bytes_before_sample": log_bytes,
                                           "elapsed_ms": (time.monotonic() - start) * 1000,
                                           "exited": p.poll() is not None})
                        else:
                            peak["gdi_objects"] = max(peak["gdi_objects"], gdi)
                        time.sleep(0.01)
                    stderr_file.seek(0)
                    stderr_bytes = stderr_file.read()
                    stderr = stderr_bytes.decode("utf-8", errors="replace")
                    record.update(samples=samples, attempts=attempts, returncode=p.returncode,
                                  stderr_bytes_total=len(stderr_bytes),
                                  stderr=stderr, wall_ms=(time.monotonic() - start) * 1000, **peak)
                    work = re.search(r"bench: (\d+) frames,", stderr)
                    record["frames"] = int(work[1]) if work else 0
                    creation, exit_time, kernel, user = (w.FILETIME() for _ in range(4))
                    if get_times(int(p._handle), c.byref(creation), c.byref(exit_time), c.byref(kernel), c.byref(user)):
                        ticks = lambda t: (t.dwHighDateTime << 32) | t.dwLowDateTime
                        record["cpu_ms"] = (ticks(kernel) + ticks(user)) / 10000
                    else:
                        record["cpu_error"] = c.get_last_error()
                    save()
                    if p.returncode or not samples or not output.exists() or record["frames"] != 1000:
                        raise RuntimeError(f"Invalid run: {command}\n{stderr}")
                    try:
                        record.update(image_identity(output, size))
                    except Exception as error:
                        record["error"] = str(error)
                        save()
                        raise
                expected = expected_images.setdefault((side, size) if references else size, record["pixels_sha256"])
                if expected != record["pixels_sha256"]:
                    record["error"] = "image mismatch"
                    save()
                    raise AssertionError(f"Image regression: {size}, {side}, trial {trial}")
                # A process may exit between poll and sampling. Preserve that error
                # rather than quietly treating an incomplete sample as a zero.
                for error in errors:
                    error["after_final_output"] = gdi_after_final_output(error, stderr_bytes, peak["gdi_objects"])
                record["valid"] = (peak["gdi_objects"] > 0 and "cpu_error" not in record
                                   and all(e["exited"] or e["after_final_output"] for e in errors))
                save()
                print({k: record[k] for k in ("side", "size", "trial", "samples", "private_bytes", "gdi_objects")}, flush=True)
            pair = results[pair_start:]
            if all(record["valid"] for record in pair):
                trial += 1
                attempt = 1
                continue
            # Preserve both observations and retry in the same order. Pixel,
            # work-count and process failures above still stop immediately.
            report["excluded_pairs"].append(pair)
            del results[pair_start:]
            save()
            if attempt == 3:
                raise RuntimeError(f"Three sampling attempts failed for {size}, pair {trial}")
            attempt += 1
    report["valid"] = True
    excluded = [r for pair in report["excluded_pairs"] for r in pair]
    report["sampling_error_count"] = sum(len(r["sampling_errors"]) for r in results + excluded)
    save()

if __name__ == "__main__":
    main()
