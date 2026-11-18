"""Harness guards: run with python -m unittest discover -s tools -p test_perf_memory.py."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from PIL import Image
import perf_memory as perf


class MemoryHarnessTests(unittest.TestCase):
    def test_png_is_decoded_and_dimensions_are_checked(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "frame.png"
            Image.new("RGBA", (2, 3), (10, 20, 30, 255)).save(path)
            identity = perf.image_identity(path, "2x3")
            self.assertEqual(identity["pixel_bytes"], 24)
            with self.assertRaises(ValueError):
                perf.image_identity(path, "3x2")
            path.write_bytes(path.read_bytes()[:30])
            with self.assertRaises(Exception):
                perf.image_identity(path, "2x3")

    def exercise(self, *, frames=1000, memory_ok=1, mismatch=False, references=False):
        with tempfile.TemporaryDirectory() as temp:
            directory = Path(temp)
            baseline, candidate = directory / "a.exe", directory / "b.exe"
            baseline.write_bytes(b"baseline identity")
            candidate.write_bytes(b"candidate identity")
            args = ["perf_memory.py", str(baseline), str(candidate), str(directory)]
            if references:
                args.append(str(directory))
                for side, color in [("baseline", "blue"), ("candidate", "red")]:
                    for size in ["800x300", "1440x540"]:
                        Image.new("RGB", tuple(map(int, size.split("x"))), color).save(directory / f"{side}-{size}.png")
            class Process:
                def __init__(self, command, stdout, stderr, env):
                    self._handle = 1
                    self.returncode = None
                    self.polls = 0
                    size = tuple(map(int, command[4].split("x")))
                    color = "red" if mismatch and Path(command[0]).name == "b.exe" else "blue"
                    Image.new("RGB", size, color).save(command[2])
                    stderr.write(f"bench: {frames} frames, frame median 1 ms".encode())
                def __enter__(self): return self
                def __exit__(self, *args): pass
                def poll(self):
                    self.polls += 1
                    if self.polls > 1: self.returncode = 0
                    return self.returncode
            error = None
            with patch.object(perf.sys, "argv", args), \
                 patch.object(perf.platform, "platform", return_value="test platform"), \
                 patch.object(perf.subprocess, "Popen", Process), \
                 patch.object(perf, "get_memory", return_value=memory_ok), \
                 patch.object(perf, "get_gdi", return_value=2), \
                 patch.object(perf, "get_times", return_value=1), \
                 patch.object(perf.time, "sleep"), contextlib.redirect_stdout(io.StringIO()):
                try:
                    perf.main()
                except (RuntimeError, AssertionError) as caught:
                    error = caught
            return json.loads((directory / "memory.json").read_text()), error

    def test_valid_runs_record_binary_identity_and_work(self):
        report, error = self.exercise()
        self.assertIsNone(error)
        self.assertTrue(report["valid"])
        self.assertEqual(len(report["runs"]), 20)
        self.assertNotEqual(report["executables"]["baseline"]["sha256"], report["executables"]["candidate"]["sha256"])
        self.assertTrue(all(run["frames"] == 1000 and run["samples"] > 0 for run in report["runs"]))

    def test_missing_work_invalidates_report(self):
        report, error = self.exercise(frames=999)
        self.assertIsNotNone(error)
        self.assertFalse(report["valid"])
        self.assertFalse(report["runs"][0]["valid"])

    def test_sampling_failure_is_preserved(self):
        report, error = self.exercise(memory_ok=0)
        self.assertIsNotNone(error)
        self.assertFalse(report["valid"])
        self.assertEqual(report["runs"][0]["sampling_errors"][0]["api"], "GetProcessMemoryInfo")

    def test_changed_pixels_invalidate_report(self):
        report, error = self.exercise(mismatch=True)
        self.assertIsNotNone(error)
        self.assertFalse(report["valid"])
        self.assertEqual(report["runs"][-1]["error"], "image mismatch")

    def test_intentional_visual_change_checks_each_reference(self):
        report, error = self.exercise(mismatch=True, references=True)
        self.assertIsNone(error)
        self.assertTrue(report["valid"])
        report, error = self.exercise(references=True)
        self.assertIsNotNone(error)
        self.assertFalse(report["valid"])


if __name__ == "__main__":
    unittest.main()
