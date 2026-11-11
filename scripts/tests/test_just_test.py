"""Owner-approved fast-test entry point stays aligned with CI."""

import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[2]


class JustTest(unittest.TestCase):
    def test_just_test_runs_ci_fast_features(self):
        result = subprocess.run(
            ["just", "--show", "test"], cwd=ROOT, capture_output=True, text=True
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        command = "cargo nextest run --features app/dev-fakes,testkit/fakes,admin/mocks,kernel/mocks"
        self.assertIn(command, result.stdout)
        for workflow in ("ci", "nightly"):
            data = json.loads((ROOT / f".github/workflows/{workflow}.yml").read_text())
            fast = [step["run"] for job in data["jobs"].values()
                    for step in job["steps"] if step.get("name") == "Fast tests"]
            self.assertTrue(fast, workflow)
            for run in fast:
                self.assertIn("--features app/dev-fakes,testkit/fakes,admin/mocks,kernel/mocks", run)
        self.assertIn("| Unit / mock / fake / HTTP | `just test` |", (ROOT / "AGENTS.md").read_text())


if __name__ == "__main__":
    unittest.main()
