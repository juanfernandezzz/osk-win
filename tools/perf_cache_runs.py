"""Run the release test probe for each supplied binary (ORIGINAL FIRST ... OUT.json)."""
import hashlib
import json
import os
from pathlib import Path
import re
import statistics
import subprocess
import sys


def main():
    *paths, output = map(Path, sys.argv[1:])
    probes = [p.resolve() for p in paths]
    env = dict(os.environ, OSK_CONFIG=str(output.with_suffix(".config.json").resolve()))
    report = {"valid": False, "binaries": [
        {"id": p.parent.name, "sha256": hashlib.sha256(p.read_bytes()).hexdigest()}
        for p in probes], "runs": []}
    for trial in range(5):
        order = probes[trial % len(probes):] + probes[:trial % len(probes)]
        for p in order:
            record = {"id": p.parent.name, "trial": trial}
            report["runs"].append(record)
            try:
                proc = subprocess.run([p, "--ignored", "probe_cold_rebuild_hot", "--nocapture"],
                                      capture_output=True, text=True, timeout=180, env=env, check=True)
                cases = json.loads(re.search(r"^probe: (.+)$", proc.stdout, re.M)[1])
                expected = [(w, mode, n) for w in (800, 1440)
                            for mode, n in (("first", 1), ("rebuild", 80), ("hot", 500))]
                assert [(c["size"][0], c["mode"], c["frames"]) for c in cases] == expected
                assert all(len(c["us"]) == c["frames"] and min(c["us"]) > 0 for c in cases)
                assert "1 passed; 0 failed" in proc.stdout
                record["cases"] = [{**c, "median_us": statistics.median(c["us"])} for c in cases]
            except Exception as error:
                record["error"] = str(error)
                raise
            finally:
                output.write_text(json.dumps(report), encoding="utf-8")
    report["valid"] = True
    output.write_text(json.dumps(report), encoding="utf-8")


if __name__ == "__main__":
    main()
