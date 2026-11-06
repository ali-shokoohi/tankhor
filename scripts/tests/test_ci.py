"""T028: FR-024/FR-CON-37 executable pinned fast/bootstrap/integration gates."""

import json
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class CiTest(unittest.TestCase):
    def test_ci_requires_fast_and_adapter_integration_gates(self):
        for name in ("ci", "nightly"):
            path = ROOT / f".github/workflows/{name}.yml"
            self.assertTrue(path.is_file(), f"{name} workflow is missing")
            # JSON is a YAML subset; using it keeps policy validation standard-library-only.
            workflow = json.loads(path.read_text())
            self.assertEqual(workflow["permissions"], {"contents": "read"})
            if name == "ci":
                self.assertIn("pull_request", workflow["on"])
            else:
                self.assertIn("schedule", workflow["on"])
                self.assertIn("workflow_dispatch", workflow["on"])
            jobs = workflow["jobs"]
            for job in jobs.values():
                self.assertEqual(job["env"]["SQLX_OFFLINE"], "true")
                self.assertGreater(job["timeout-minutes"], 0)
                steps = job["steps"]
                for step in steps:
                    if "uses" in step:
                        self.assertRegex(step["uses"], r"^[\w/-]+@[a-f0-9]{40}$")
                commands = "\n".join(step.get("run", "") for step in steps)
                self.assertIn("rustup toolchain install 1.98.1", commands)
                self.assertIn("cargo install cargo-nextest --version 0.9.151 --locked", commands)
                self.assertIn("cargo install cargo-deny --version 0.20.2 --locked", commands)
                for gate in ("cargo fmt --check", "cargo clippy --all-targets --all-features -- -D warnings",
                             "cargo deny check", "python3 scripts/check-architecture.py",
                             "python3 scripts/check-test-traceability.py --mode partial",
                             "python3 -m unittest discover -s scripts/tests", "cargo nextest run --profile ci"):
                    self.assertIn(gate, commands)
                bootstrap = next(i for i, step in enumerate(steps)
                                 if "cargo nextest run --profile fixture-bootstrap -p testkit" in step.get("run", ""))
                integration = next(i for i, step in enumerate(steps)
                                   if "cargo nextest run --profile integration" in step.get("run", ""))
                self.assertLess(bootstrap, integration)
                cleanup = [step for step in steps if "bash scripts/test-down.sh" in step.get("run", "")]
                self.assertEqual(len(cleanup), 1)
                self.assertEqual(cleanup[0]["if"], "always()")
                self.assertIn("cargo metadata", commands)
                self.assertIn("not present", commands, "future crates must be reported honestly")
                if name == "ci":
                    detection = next(step for step in steps if step.get("id") == "changes")
                    for prefix in ("crates/infra-", "crates/wallet/", "crates/generation/", "crates/testkit/"):
                        self.assertIn(prefix, detection["run"])
                    for index in (bootstrap, integration):
                        self.assertIn("steps.changes.outputs.integration", steps[index]["if"])


if __name__ == "__main__":
    unittest.main()
