"""T024: FR-024/FR-CON-37 exact project/run cleanup, with active-run refusal."""

import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts/test-down.sh"


class FixtureCleanupTest(unittest.TestCase):
    def test_ci_cleanup_matches_only_current_run_labels(self):
        self.assertTrue(SCRIPT.is_file(), "exact-scope cleanup command is missing")
        with tempfile.TemporaryDirectory(prefix="tankhor-cleanup-") as directory:
            state = Path(directory)
            state.chmod(0o700)
            project = f"cleanup-{os.getpid()}-{time.time_ns()}"
            containers = []
            for selected_project, run in [(project, "ci-job-a"), (project, "ci-job-ab"),
                                          (project + "-foreign", "ci-job-a")]:
                result = subprocess.run([
                    "docker", "create", "--label", "io.tankhor.fixture=true",
                    "--label", f"io.tankhor.fixture.project={selected_project}",
                    "--label", f"io.tankhor.fixture.run={run}",
                    "--label", "io.tankhor.fixture.service=postgres", "postgres:16",
                ], capture_output=True, text=True, check=True)
                containers.append(result.stdout.strip())
            owner = subprocess.Popen(["sleep", "60"])
            try:
                environment = dict(os.environ, FIXTURE_STATE_DIR=str(state), FIXTURE_PROJECT=project,
                                   FIXTURE_RUN="ci-job-a", FIXTURE_OWNER_PID=str(owner.pid))
                subprocess.run([str(ROOT / "target/debug/fixture-runner"), "lease-claim"],
                               env=environment, check=True, capture_output=True)
                refused = subprocess.run(["bash", str(SCRIPT)], env=environment,
                                         capture_output=True, text=True)
                self.assertNotEqual(refused.returncode, 0, "active run cleanup must fail")
                self.assertEqual(subprocess.run(["docker", "inspect", containers[0]],
                                               capture_output=True).returncode, 0)
                owner.terminate()
                owner.wait(timeout=5)
                removed = subprocess.run(["bash", str(SCRIPT)], env=environment,
                                         capture_output=True, text=True)
                self.assertEqual(removed.returncode, 0, removed.stderr)
                for index, container in enumerate(containers):
                    exists = subprocess.run(["docker", "inspect", container], capture_output=True).returncode == 0
                    self.assertEqual(exists, index != 0, "cleanup must match exact project AND run")
                foreign = subprocess.run(["bash", str(SCRIPT)],
                                         env=dict(environment, FIXTURE_PROJECT=project + "-foreign"),
                                         capture_output=True, text=True)
                self.assertNotEqual(foreign.returncode, 0, "foreign state cannot authorize cleanup")
            finally:
                if owner.poll() is None:
                    owner.terminate()
                owner.wait(timeout=5)
                subprocess.run(["docker", "rm", "-f", "-v", *containers], capture_output=True)


if __name__ == "__main__":
    unittest.main()
