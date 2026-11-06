"""T022: FR-024/FR-CON-37 same-child execution, discovery and fallback lifetime."""

import json
import os
from pathlib import Path
import subprocess
import tempfile
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "scripts/test-runner.sh"


class FallbackRunnerTest(unittest.TestCase):
    def test_fallback_runner_sets_only_child_environment(self):
        self.assertTrue(RUNNER.is_file(), "test-aware Cargo runner is missing")
        with tempfile.TemporaryDirectory(prefix="tankhor-child-") as directory:
            base = Path(directory)
            target = base / "target/debug/deps"
            target.mkdir(parents=True)
            output = base / "child.json"
            program = """#!/usr/bin/env python3
import json,os,subprocess,sys
data={'args':sys.argv[1:], 'url':os.environ.get('DATABASE_URL'),
      'id':os.environ.get('FIXTURE_CONTAINER'), 'passfile':os.environ.get('PGPASSFILE')}
if data['id']:
    p=subprocess.run(['docker','inspect','--format','{{.State.Running}}',data['id']],capture_output=True,text=True)
    assert p.returncode == 0 and p.stdout.strip() == 'true', 'guard dropped before child exit'
with open(os.environ['CHILD_OUTPUT'],'w') as f: json.dump(data,f)
if os.environ.get('CHILD_WAIT'): __import__('time').sleep(60)
sys.exit(int(os.environ.get('CHILD_EXIT','0')))
"""
            binaries = {}
            for name in ("integration_probe-abcdef", "fixture_bootstrap_probe-abcdef", "plain_probe-abcdef"):
                binary = target / name
                binary.write_text(program)
                binary.chmod(0o700)
                binaries[name.split("-", 1)[0]] = binary
            environment = dict(os.environ, CHILD_OUTPUT=str(output))
            for key in ("DATABASE_URL", "PGPASSFILE", "FIXTURE_CONTAINER", "FIXTURE_PROJECT", "FIXTURE_RUN"):
                environment.pop(key, None)

            def run(binary, arguments=(), changes=None):
                result = subprocess.run(["bash", str(RUNNER), str(binary), *arguments],
                                        env=dict(environment, **(changes or {})), cwd=ROOT,
                                        capture_output=True, text=True, timeout=60)
                data = json.loads(output.read_text()) if output.exists() else None
                output.unlink(missing_ok=True)
                return result, data

            for binary, arguments in ((binaries["plain_probe"], ["case", "--exact"]),
                                      (binaries["fixture_bootstrap_probe"], ["case"]),
                                      (binaries["integration_probe"], ["--list", "--format", "terse"]),
                                      (Path("/bin/true"), [])):
                result, data = run(binary, arguments)
                self.assertEqual(result.returncode, 0, result.stderr)
                if data:
                    self.assertIsNone(data["url"])
                    self.assertIsNone(data["id"])
                    self.assertEqual(data["args"], arguments)
            for status in (0, 19):
                result, data = run(binaries["integration_probe"], ["same_case", "--exact"],
                                   {"CHILD_EXIT": str(status)})
                self.assertEqual(result.returncode, status, result.stderr)
                assert data is not None, "storage child must execute"
                self.assertEqual(data["args"], ["same_case", "--exact"])
                self.assertTrue(data["url"].startswith("postgresql://fixture_admin@127.0.0.1:"))
                self.assertTrue(data["id"])
                self.assertTrue(data["passfile"])
                gone = subprocess.run(["docker", "inspect", data["id"]], capture_output=True)
                self.assertNotEqual(gone.returncode, 0, "fallback container must be removed after child")
                self.assertFalse(Path(data["passfile"]).exists())
            process = subprocess.Popen(
                ["bash", str(RUNNER), str(binaries["integration_probe"])], cwd=ROOT,
                env=dict(environment, CHILD_WAIT="1"), stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            )
            try:
                deadline = time.monotonic() + 40
                while not output.exists():
                    self.assertIsNone(process.poll(), "fallback child exited before signal test")
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(0.1)
                signal_child = json.loads(output.read_text())
                process.terminate()
                process.communicate(timeout=10)
                self.assertEqual(process.returncode, 143, "forward SIGTERM and preserve child status")
                self.assertNotEqual(subprocess.run(["docker", "inspect", signal_child["id"]],
                                                  capture_output=True).returncode, 0)
                output.unlink()
            finally:
                if process.poll() is None:
                    process.kill()
                    process.communicate()
            refused, data = run(binaries["integration_probe"], changes={
                "DATABASE_URL": "postgresql://production.example/database"})
            self.assertNotEqual(refused.returncode, 0)
            self.assertIsNone(data, "unvalidated supplied endpoint must never reach SQLx setup")
            self.assertNotIn("DATABASE_URL", os.environ)


if __name__ == "__main__":
    unittest.main()
