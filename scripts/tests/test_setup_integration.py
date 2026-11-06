"""T017: FR-024/FR-CON-37 ready, owned, password-free nextest exports.

The driver prepares its own PostgreSQL independently of shared setup. The first red
exercises the existing runner's absent export operation, never an absent shell script.
"""

import os
import copy
from pathlib import Path
import secrets
import subprocess
import tempfile
import time
import tomllib
import unittest

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "target/debug/fixture-runner"
WRAPPER = ROOT / "scripts/setup-integration.sh"


def docker(*arguments):
    return subprocess.run(["docker", *arguments], capture_output=True, text=True, check=True)


def validate_activation(config, tasks):
    for profile in ("default", "ci", "fixture-bootstrap"):
        if config["profile"][profile].get("scripts"):
            raise AssertionError("shared setup must run only in integration")
    if config["profile"]["integration"].get("scripts"):
        for task in ("T011", "T013", "T015", "T017", "T018", "T019", "T020"):
            if f"- [X] {task} " not in tasks:
                raise AssertionError("premature shared-hook activation: " + task)


class SetupIntegrationTest(unittest.TestCase):
    def test_setup_exports_only_matching_tests_ready_endpoints(self):
        subprocess.run(["cargo", "build", "--quiet", "-p", "testkit", "--bin", "fixture-runner"],
                       cwd=ROOT, check=True)
        with tempfile.TemporaryDirectory(prefix="tankhor-export-") as directory:
            base = Path(directory)
            base.chmod(0o700)
            passwords = {role: secrets.token_hex(32)
                         for role in ("admin", "migration", "runtime", "preparation")}
            for role, password in passwords.items():
                path = base / role
                path.write_text(password)
                path.chmod(0o600)
            scope = f"export-{os.getpid()}-{time.time_ns()}"
            container = docker(
                "run", "-d", "--label", "io.tankhor.fixture=true",
                "--label", "io.tankhor.fixture.project=tankhor-export",
                "--label", f"io.tankhor.fixture.run={scope}",
                "--label", "io.tankhor.fixture.service=postgres",
                "-p", "127.0.0.1::5432", "-e", "POSTGRES_USER=fixture_admin",
                "-e", "POSTGRES_PASSWORD_FILE=/run/secrets/admin",
                "--mount", f"type=bind,src={base / 'admin'},dst=/run/secrets/admin,readonly",
                "postgres:16",
            ).stdout.strip()
            try:
                deadline = time.monotonic() + 30
                while True:
                    ready = subprocess.run(
                        ["docker", "exec", container, "pg_isready", "-h", "127.0.0.1", "-U", "fixture_admin"],
                        capture_output=True, check=False,
                    )
                    if ready.returncode == 0:
                        break
                    self.assertLess(time.monotonic(), deadline, "setup blocker: PostgreSQL not ready")
                    time.sleep(0.1)
                port = docker("inspect", "--format",
                              '{{(index (index .NetworkSettings.Ports "5432/tcp") 0).HostPort}}',
                              container).stdout.strip()
                environment = dict(os.environ, FIXTURE_PROJECT="tankhor-export", FIXTURE_RUN=scope,
                                   FIXTURE_CONTAINER=container, FIXTURE_HOST="127.0.0.1",
                                   FIXTURE_PORT=port, FIXTURE_CREDENTIAL_DIR=str(base),
                                   NEXTEST_ENV=str(base / "nextest.env"), FIXTURE_RUNNER=str(RUNNER))
                environment.pop("DATABASE_URL", None)
                command = [str(RUNNER), "export-ready"]
                if WRAPPER.exists():
                    command = ["bash", str(WRAPPER)]
                result = subprocess.run(command, env=environment, cwd=ROOT,
                                        capture_output=True, text=True, timeout=40)
                self.assertEqual(result.returncode, 0,
                                 "ready owned endpoint export is missing: " + result.stderr)
                exported = (base / "nextest.env").read_text()
                self.assertIn(f"DATABASE_URL=postgresql://fixture_admin@127.0.0.1:{port}/postgres", exported)
                self.assertIn("PGPASSFILE=", exported)
                self.assertEqual((base / "pgpass").stat().st_mode & 0o777, 0o600)
                for password in passwords.values():
                    self.assertNotIn(password, exported + result.stdout + result.stderr)
                self.assertNotIn("DATABASE_URL", os.environ)
                for changes in (
                    {"FIXTURE_RUN": scope + "-foreign"},
                    {"FIXTURE_PROJECT": "developer-compose"},
                    {"FIXTURE_HOST": "production.example"},
                    {"FIXTURE_PORT": "1"},
                ):
                    with self.subTest(refusal=changes):
                        before = (base / "nextest.env").read_bytes()
                        rejected = subprocess.run(command, env=dict(environment, **changes), cwd=ROOT,
                                                  capture_output=True, text=True, timeout=40)
                        self.assertNotEqual(rejected.returncode, 0)
                        self.assertEqual((base / "nextest.env").read_bytes(), before)
                        for password in passwords.values():
                            self.assertNotIn(password, rejected.stdout + rejected.stderr)
                docker("pause", container)
                try:
                    before = (base / "nextest.env").read_bytes()
                    unready = subprocess.run(command, env=environment, cwd=ROOT,
                                             capture_output=True, text=True, timeout=10)
                    self.assertNotEqual(unready.returncode, 0, "paused server must not be exported")
                    self.assertEqual((base / "nextest.env").read_bytes(), before)
                finally:
                    docker("unpause", container)
                self.assertTrue(WRAPPER.is_file(), "thin setup wrapper must be installed at green")
            finally:
                docker("rm", "-f", "-v", container)

        with (ROOT / ".config/nextest.toml").open("rb") as handle:
            config = tomllib.load(handle)
        tasks = (ROOT / "specs/001-admin-foundation/tasks.md").read_text()
        validate_activation(config, tasks)
        required = ("T011", "T013", "T015", "T017", "T018", "T019", "T020")
        # Exercise premature activation independently of the current completion stage.
        activated = copy.deepcopy(config)
        activated["profile"]["integration"]["scripts"] = [{"setup": "integration-services"}]
        completed = "\n".join(f"- [X] {task} " for task in required)
        validate_activation(activated, completed)
        for missing in required:
            with self.assertRaisesRegex(AssertionError, missing):
                validate_activation(activated, completed.replace(f"- [X] {missing} ", ""))
        for profile in ("default", "ci", "fixture-bootstrap"):
            misplaced = copy.deepcopy(activated)
            misplaced["profile"][profile]["scripts"] = [{"setup": "integration-services"}]
            with self.assertRaisesRegex(AssertionError, "only in integration"):
                validate_activation(misplaced, completed)


if __name__ == "__main__":
    unittest.main()
