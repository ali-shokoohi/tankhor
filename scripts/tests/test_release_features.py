"""T065: FR-001/FR-CON-01 release feature closure and debug-only dev-fakes enforcement."""

import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT = Path(__file__).resolve().parents[2]


class ReleaseFeaturesTest(unittest.TestCase):
    def test_release_graph_excludes_mocks_and_fakes(self):
        checker = ROOT / "scripts/check-release-features.py"
        self.assertTrue(checker.is_file(), "scripts/check-release-features.py is missing")

        # 1. Checker script itself must pass on the real workspace.
        result = subprocess.run(
            [sys.executable, str(checker), "--manifest-path", str(ROOT / "Cargo.toml")],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(
            result.returncode,
            0,
            f"check-release-features.py failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )

        # 2. Verify cargo tree default/release closure of `app` excludes testkit, mocks, fakes, dev-fakes.
        tree_default = subprocess.run(
            [
                "cargo",
                "tree",
                "--offline",
                "-p",
                "app",
                "-e",
                "normal,build",
                "-f",
                "{p} [{f}]",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=True,
        )
        for line in tree_default.stdout.splitlines():
            self.assertNotIn("testkit v", line, "default/release app graph must not include testkit")
            self.assertNotIn("mocks", line, "default/release app graph must not enable mocks")
            self.assertNotIn("fakes", line, "default/release app graph must not enable fakes")
            self.assertNotIn("dev-fakes", line, "default/release app graph must not enable dev-fakes")

        # 3. Verify explicit `--features app/dev-fakes` enables app/dev-fakes, testkit/fakes, and context mocks in debug.
        tree_fakes = subprocess.run(
            [
                "cargo",
                "tree",
                "--offline",
                "-p",
                "app",
                "--features",
                "app/dev-fakes",
                "-e",
                "normal,build",
                "-f",
                "{p} [{f}]",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=True,
        )
        self.assertIn("dev-fakes", tree_fakes.stdout)
        self.assertIn("testkit v", tree_fakes.stdout)
        self.assertIn("fakes", tree_fakes.stdout)
        self.assertIn("mocks", tree_fakes.stdout)

        # 4. Negative release-build invocation: `cargo check --release -p app --features dev-fakes` MUST fail!
        release_fakes = subprocess.run(
            [
                "cargo",
                "check",
                "--offline",
                "--release",
                "-p",
                "app",
                "--features",
                "dev-fakes",
            ],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertNotEqual(
            release_fakes.returncode,
            0,
            "release build with --features dev-fakes must be rejected at compile time",
        )
        self.assertIn("dev-fakes", release_fakes.stderr)

        # 5. CI workflow must invoke check-release-features.py.
        ci_workflow = json.loads((ROOT / ".github/workflows/ci.yml").read_text())
        ci_commands = "\n".join(
            step.get("run", "")
            for job in ci_workflow["jobs"].values()
            for step in job["steps"]
        )
        self.assertIn("python3 scripts/check-release-features.py", ci_commands)


if __name__ == "__main__":
    unittest.main()
