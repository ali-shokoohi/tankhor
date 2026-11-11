"""T019: FR-024/FR-CON-37 scope, live nextest owner and serialized lease.

Compile the private, std-only policy functions verbatim from fixture-runner into a
temporary process probe. This exercises real environment and /proc ancestry without
starting shared services or adding a test-only operation to the production runner.
"""

import os
import fcntl
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
RUNNER = ROOT / "target/debug/fixture-runner"


class FixtureLeaseTest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory(prefix="tankhor-scope-owner-")
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.base = Path(cls.temporary.name)
        source = (ROOT / "crates/testkit/src/bin/fixture-runner.rs").read_text()
        functions = []
        for name in ("shared_scope", "nextest_owner", "environment"):
            match = re.search(rf"(?ms)^fn {name}\(.*?^}}", source)
            if match is None:
                raise AssertionError(f"fixture-runner policy function missing: {name}")
            functions.append(match.group())
        probe_source = cls.base / "probe.rs"
        probe_source.write_text("use std::path::PathBuf;\n" + "\n".join(functions) + r'''
fn main() {
    match std::env::args().nth(1).as_deref() {
        Some("scope") => match shared_scope() {
            Ok((project, run, state)) => {
                println!("{project}\n{run}\n{}", state.display());
            }
            Err(()) => std::process::exit(1),
        },
        Some("owner") => match nextest_owner() {
            Ok(owner) => println!("{owner}"),
            Err(()) => std::process::exit(1),
        },
        _ => std::process::exit(2),
    }
}
''')
        cls.probe = cls.base / "probe"
        subprocess.run(["rustc", "--edition=2024", str(probe_source), "-o", str(cls.probe)],
                       env=dict(os.environ, CARGO_MANIFEST_DIR=str(ROOT / "crates/testkit")),
                       check=True, capture_output=True, text=True)
        launcher_source = cls.base / "launcher.rs"
        launcher_source.write_text(r'''
use std::io::Write;
fn main() {
    println!("{}", std::process::id());
    std::io::stdout().flush().unwrap();
    let status = std::process::Command::new(std::env::args_os().nth(1).unwrap())
        .arg("owner").status().unwrap();
    std::process::exit(status.code().unwrap_or(1));
}
''')
        cls.launcher = cls.base / "cargo-nextest"
        subprocess.run(["rustc", "--edition=2024", str(launcher_source),
                        "-o", str(cls.launcher)], check=True, capture_output=True, text=True)

    def scope(self, **changes):
        environment = {key: value for key, value in os.environ.items()
                       if key not in {"CI", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_JOB"}}
        return subprocess.run([str(self.probe), "scope"], cwd=self.base,
                              env=dict(environment, **changes),
                              capture_output=True, text=True, timeout=10)

    def test_shared_scope_derives_local_repository_scope(self):
        result = self.scope(GITHUB_RUN_ID="ignored", GITHUB_JOB="ignored")
        self.assertEqual(result.returncode, 0, result.stderr)
        project, run, state = result.stdout.splitlines()
        self.assertRegex(project, r"^tankhor-[0-9a-f]+$")
        self.assertEqual(run, "local", "non-CI scope must be local")
        self.assertEqual(Path(state), ROOT.resolve() / "target/fixtures/local")
        self.assertEqual(self.scope().stdout, result.stdout,
                         "local scope must be stable despite irrelevant CI identifiers")

    def test_shared_scope_derives_exact_ci_job_run_scope(self):
        local = self.scope().stdout.splitlines()
        for run_id, attempt, job in (("701", "1", "integration"), ("702", "1", "integration"),
                                     ("701", "2", "integration"), ("701", "1", "bootstrap")):
            with self.subTest(run_id=run_id, attempt=attempt, job=job):
                result = self.scope(CI="true", GITHUB_RUN_ID=run_id,
                                    GITHUB_RUN_ATTEMPT=attempt, GITHUB_JOB=job)
                self.assertEqual(result.returncode, 0, result.stderr)
                project, run, state = result.stdout.splitlines()
                self.assertEqual(project, local[0], "CI must retain repository identity")
                expected = f"ci-{run_id}-{attempt}-{job}"
                self.assertEqual(run, expected, "CI scope must include exact run, attempt and job")
                self.assertEqual(Path(state), ROOT.resolve() / "target/fixtures" / expected)
        required = dict(CI="true", GITHUB_RUN_ID="701", GITHUB_RUN_ATTEMPT="1",
                        GITHUB_JOB="integration")
        for key in ("GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT", "GITHUB_JOB"):
            for invalid in (None, "", "invalid\nvalue"):
                with self.subTest(missing_or_invalid=key, value=invalid):
                    environment = dict(required)
                    if invalid is None:
                        del environment[key]
                    else:
                        environment[key] = invalid
                    self.assertNotEqual(self.scope(**environment).returncode, 0,
                                        "CI scope must refuse missing/invalid exact identity")

    def test_nextest_owner_identifies_live_ancestor(self):
        result = subprocess.run([str(self.launcher), str(self.probe)],
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, "live nextest ancestor must be found")
        expected, actual = result.stdout.splitlines()
        self.assertEqual(actual, expected, "lease owner must be nextest, not its setup child")

    def test_nextest_owner_rejects_mismatched_ancestor(self):
        impostor = self.base / "cargo-nextest-impostor"
        impostor.symlink_to(self.launcher)
        result = subprocess.run([str(impostor), str(self.probe)],
                                capture_output=True, text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0, "a nextest-like ancestor is not the owner")
        self.assertEqual(len(result.stdout.splitlines()), 1, "must not emit an accepted owner")

    def test_nextest_owner_rejects_absent_ancestor(self):
        result = subprocess.run([str(self.probe), "owner"],
                                capture_output=True, text=True, timeout=10)
        self.assertNotEqual(result.returncode, 0, "setup without nextest must refuse an owner")
        self.assertEqual(result.stdout, "", "must not substitute the setup process as owner")

    def test_sigkill_owner_reclaims_stale_lease_and_releases_setup_lock(self):
        subprocess.run(["cargo", "build", "-q", "-p", "testkit", "--bin", "fixture-runner"],
                       cwd=ROOT, check=True)
        with tempfile.TemporaryDirectory(prefix="tankhor-crashed-lease-") as directory:
            base = Path(directory)
            base.chmod(0o700)
            first = subprocess.Popen(
                ["python3", "-c",
                 "import fcntl,sys; sys.stdin.readline(); "
                 "f=open(sys.argv[1], 'rb'); fcntl.flock(f, fcntl.LOCK_EX); "
                 "print('locked', flush=True); sys.stdin.read()",
                 str(base / "setup.lock")],
                stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True,
            )
            second = subprocess.Popen(["sleep", "60"])
            try:
                environment = dict(os.environ, FIXTURE_STATE_DIR=str(base),
                                   FIXTURE_PROJECT="repository-a", FIXTURE_RUN="local")

                def claim(owner):
                    return subprocess.run(
                        [str(RUNNER), "lease-claim"], cwd=ROOT,
                        env=dict(environment, FIXTURE_OWNER_PID=str(owner.pid)),
                        capture_output=True, text=True, timeout=10,
                    )

                self.assertEqual(claim(first).returncode, 0)
                lease = base / "live-run.lease"
                recorded = lease.read_bytes()
                self.assertNotEqual(claim(second).returncode, 0,
                                    "a live independent owner must still block a claim")
                self.assertEqual(lease.read_bytes(), recorded)
                # Make the soon-to-crash owner hold the actual setup file lock too.
                first.stdin.write("lock\n")
                first.stdin.flush()
                self.assertEqual(first.stdout.readline().strip(), "locked")
                self.assertNotEqual(claim(first).returncode, 0,
                                    "setup lock must remain exclusive before the crash")
                first.kill()
                first.wait(timeout=5)
                reclaimed = claim(second)
                self.assertEqual(reclaimed.returncode, 0,
                                 "SIGKILL must release the lock and permit dead-owner reclaim: "
                                 + reclaimed.stderr)
                self.assertNotEqual(lease.read_bytes(), recorded)
                self.assertEqual(lease.read_text().splitlines()[2], str(second.pid))
            finally:
                for process in (first, second):
                    if process.poll() is None:
                        process.kill()
                    process.communicate(timeout=5)

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
