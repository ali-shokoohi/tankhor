"""T030: planned task links and completed-work artifact boundary (partial gate)."""

import importlib.util
from pathlib import Path
import tempfile
import subprocess
import sys
import unittest

from traceability_fixture import fixture, put

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts/check-test-traceability.py"


def load_checker():
    spec = importlib.util.spec_from_file_location("traceability_checker", CHECKER)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class TraceabilityTest(unittest.TestCase):
    def test_traceability_checker_rejects_implementation_without_driving_test(self):
        self.assertTrue(CHECKER.is_file(), "traceability checker is missing")
        checker = load_checker()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            feature = fixture(root)
            self.assertEqual(checker.check(root, feature, "partial"), [])
            invocation = subprocess.run(
                [sys.executable, str(CHECKER), "--root", str(root), "--feature-dir", str(feature),
                 "--mode", "partial"], capture_output=True, text=True,
            )
            self.assertEqual(invocation.returncode, 0, invocation.stderr + invocation.stdout)
            path = feature / "tasks.md"
            original = path.read_text()
            cases = {
                "missing driver": original.replace("Make pass: T001", "Make pass: T999", 1),
                "unchecked driver": original.replace("[X] T001", "[ ] T001", 1),
                "misordered IDs": original.replace("T002 Make pass", "T004 Make pass", 1),
                "missing AC": original.replace("| AC-ADM-001 / FR-001 |", "| FR-001 |"),
                "missing port row": original.replace("| Clock | T004 | T005 | T006 | T008 | T009 |", ""),
                "unknown FR": original.replace("(FR-001, AC-ADM-001)", "(FR-999, AC-ADM-001)", 1),
                "premature artifact": original.replace("[ ] T003", "[X] T003", 1),
                "unpaired port": original.replace("| Clock | T004 | T005 | T006 | T008 | T009 |",
                                                  "| Clock | T004 | T005 | | T008 | T009 |"),
            }
            for name, tasks in cases.items():
                with self.subTest(case=name):
                    path.write_text(tasks)
                    self.assertTrue(checker.check(root, feature, "partial"), name)
            path.write_text(original)
            (root / "scripts/behavior.py").unlink()
            self.assertTrue(checker.check(root, feature, "partial"), "missing completed artifact")
            put(root, "scripts/behavior.py", "def behavior(): return 1\n")
            self.assertEqual(checker.check(root, feature, "partial"), [])
            driver = root / "scripts/tests/driver.py"
            original_driver = driver.read_text()
            driver.write_text("# def test_driving_test(): pass\n")
            self.assertTrue(checker.check(root, feature, "partial"), "comments are not driving tests")
            driver.write_text(original_driver)
            self.assertTrue(checker.check(root, feature, "final"), "unfinished symbols cannot pass final")


if __name__ == "__main__":
    unittest.main()
