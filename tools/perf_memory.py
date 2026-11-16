"""Sample headless Windows process memory/GDI objects; separate from timing runs.

Usage: python tools/perf_memory.py BASELINE CANDIDATE OUTPUT_DIR
Reports sampled peaks, not allocation counts or whole desktop UI memory.
"""
import ctypes as c
from ctypes import wintypes as w
import json
from pathlib import Path
import subprocess
import sys
import time

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

def main():
    baseline, candidate, directory = map(Path, sys.argv[1:])
    directory.mkdir(parents=True, exist_ok=True)
    results = []
    for size in ["800x300", "1440x540"]:
        for trial in range(5):
            sides = [("baseline", baseline), ("candidate", candidate)]
            for side, exe in (sides if trial % 2 == 0 else sides[::-1]):
                output = directory / f"memory-{side}.png"
                output.unlink(missing_ok=True)
                command = [str(exe.resolve()), "--render", str(output.resolve()),
                           "--size", size, "--menu", "--bench-frames", "1000"]
                peak = {"working_set": 0, "private_bytes": 0, "gdi_objects": 0}
                samples = 0
                with subprocess.Popen(command, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE) as p:
                    deadline = time.monotonic() + 120
                    while p.poll() is None:
                        if time.monotonic() > deadline:
                            p.kill()
                            raise TimeoutError(command)
                        counters = Counters(cb=c.sizeof(Counters))
                        # CPython's Windows process handle remains open for this scope.
                        if get_memory(int(p._handle), c.byref(counters), c.sizeof(counters)):
                            samples += 1
                            peak["working_set"] = max(peak["working_set"], counters.WorkingSetSize)
                            peak["private_bytes"] = max(peak["private_bytes"], counters.PrivateUsage)
                            peak["gdi_objects"] = max(peak["gdi_objects"], get_gdi(int(p._handle), 0))
                        time.sleep(0.01)
                    stderr = p.stderr.read().decode("utf-8", errors="replace")
                    if p.returncode or not samples or not output.exists() or "bench: 1000 frames" not in stderr:
                        raise RuntimeError(f"Invalid run: {command}\n{stderr}")
                results.append(dict(side=side, size=size, trial=trial, samples=samples, **peak))
                print(results[-1], flush=True)
    (directory / "memory.json").write_text(json.dumps(results, indent=2), encoding="utf-8")

if __name__ == "__main__":
    main()
