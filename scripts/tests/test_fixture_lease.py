"""T019: FR-024/FR-CON-37 persistent live-owner lease and serialized setup."""

import os
import fcntl
from pathlib import Path
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "target/debug/fixture-runner"


class FixtureLeaseTest(unittest.TestCase):
    def test_local_shared_server_rejects_concurrent_independent_runs(self):
        subprocess.run(["cargo", "build", "-q", "-p", "testkit", "--bin", "fixture-runner"],
                       cwd=ROOT, check=True)
        with tempfile.TemporaryDirectory(prefix="tankhor-lease-") as directory:
            base = Path(directory)
            base.chmod(0o700)
            first = subprocess.Popen(["sleep", "60"])
            second = subprocess.Popen(["sleep", "60"])
            try:
                environment = dict(os.environ, FIXTURE_STATE_DIR=str(base),
                                   FIXTURE_PROJECT="repository-a", FIXTURE_RUN="local",
                                   FIXTURE_OWNER_PID=str(first.pid))
                def claim(changes=None):
                    return subprocess.run([str(RUNNER), "lease-claim"],
                                          env=dict(environment, **(changes or {})), cwd=ROOT,
                                          capture_output=True, text=True, timeout=10)

                result = claim()
                self.assertEqual(result.returncode, 0, "live-run lease is missing: " + result.stderr)
                self.assertEqual(claim().returncode, 0, "same owner must be idempotent")
                lease = base / "live-run.lease"
                recorded = lease.read_bytes()
                self.assertEqual(lease.stat().st_mode & 0o777, 0o600)
                rejected = claim({"FIXTURE_OWNER_PID": str(second.pid)})
                self.assertNotEqual(rejected.returncode, 0, "independent owner must not reuse SQLx names")
                self.assertEqual(lease.read_bytes(), recorded)
                self.assertNotEqual(claim({"FIXTURE_PROJECT": "repository-ab"}).returncode, 0)
                self.assertEqual(lease.read_bytes(), recorded)
                with (base / "setup.lock").open("rb") as lock:
                    fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
                    self.assertNotEqual(claim().returncode, 0, "setup lock must serialize claims")
                    self.assertEqual(lease.read_bytes(), recorded)
                    fcntl.flock(lock, fcntl.LOCK_UN)
                # A matching PID with a different start identity is not the same owner.
                fields = recorded.decode().splitlines()
                fields[3] = str(int(fields[3]) + 1)
                lease.write_text("\n".join(fields) + "\n")
                self.assertEqual(claim().returncode, 0, "PID reuse must permit verified stale reclaim")
                self.assertEqual(lease.read_bytes(), recorded)
                first.terminate()
                first.wait(timeout=5)
                self.assertEqual(claim({"FIXTURE_OWNER_PID": str(second.pid)}).returncode, 0,
                                 "verified exited owner must permit stale reclaim")
                self.assertNotEqual(lease.read_bytes(), recorded)
                # Corrupt metadata cannot be interpreted as permission to claim a resource.
                lease.write_text("foreign-or-incomplete\n")
                self.assertNotEqual(claim({"FIXTURE_OWNER_PID": str(second.pid)}).returncode, 0)
                self.assertEqual(lease.read_text(), "foreign-or-incomplete\n")
            finally:
                for process in (first, second):
                    if process.poll() is None:
                        process.terminate()
                    process.wait(timeout=5)


if __name__ == "__main__":
    unittest.main()
