"""Architecture policy checks (T004; FR-001, FR-CON-01; constitution II).

Each case builds a disposable Cargo workspace that uses only path dependencies (a local
stub package is named `sqlx`), then runs `scripts/check-architecture.py` against it.
"""

import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CHECKER = ROOT / "scripts" / "check-architecture.py"

STUB = 'pub fn marker() {}\n'


def manifest(name: str, deps: str = "", extra: str = "") -> str:
    return (
        f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n'
        f"[dependencies]\n{deps}\n{extra}"
    )


class FixtureWorkspace:
    """Clean workspace: kernel, admin (context), infra-postgres, infra-sms, web, worker, app."""

    def __init__(self, base: Path) -> None:
        self.base = base
        self.workspace_deps = ""
        self.crates: dict[str, dict[str, str]] = {
            "vendor/sqlx": {"Cargo.toml": manifest("sqlx")},
            "vendor/helper": {"Cargo.toml": manifest("helper")},
            "crates/kernel": {"Cargo.toml": manifest("kernel")},
            "crates/admin": {
                "Cargo.toml": manifest("admin", 'kernel = { path = "../kernel" }\n'),
                "src/domain/mod.rs": "pub fn policy() -> u8 { 1 }\n",
                "src/application/mod.rs": (
                    "// Uses the injected Clock port, never Utc::now() directly.\n"
                    "pub fn service() {}\n"
                ),
            },
            "crates/infra-postgres": {
                "Cargo.toml": manifest(
                    "infra-postgres",
                    'admin = { path = "../admin" }\nsqlx = { path = "../../vendor/sqlx" }\n',
                ),
                "src/lib.rs": "pub struct PostgresAdminRepository;\n",
            },
            "crates/infra-sms": {
                "Cargo.toml": manifest("infra-sms", 'admin = { path = "../admin" }\n'),
            },
            "crates/web": {
                "Cargo.toml": manifest("web", 'admin = { path = "../admin" }\n'),
                "src/lib.rs": "use admin::service;\npub fn handler() { service(); }\n",
            },
            "crates/worker": {
                "Cargo.toml": manifest("worker", 'admin = { path = "../admin" }\n'),
            },
            "crates/app": {
                "Cargo.toml": manifest(
                    "app",
                    'admin = { path = "../admin" }\n'
                    'infra-postgres = { path = "../infra-postgres" }\n'
                    'infra-sms = { path = "../infra-sms" }\n'
                    'web = { path = "../web" }\nworker = { path = "../worker" }\n',
                ),
            },
        }

    def edit(self, crate: str, file: str, content: str) -> None:
        self.crates[crate][file] = content

    def write(self) -> Path:
        members = ", ".join(f'"{path}"' for path in self.crates)
        root = f'[workspace]\nresolver = "3"\nmembers = [{members}]\n'
        if self.workspace_deps:
            root += f"\n[workspace.dependencies]\n{self.workspace_deps}"
        (self.base / "Cargo.toml").write_text(root)
        for path, files in self.crates.items():
            crate_dir = self.base / path
            (crate_dir / "src").mkdir(parents=True, exist_ok=True)
            files = dict(files)
            if "src/lib.rs" not in files:
                modules = [
                    name.split("/")[1]
                    for name in files
                    if name.startswith("src/") and name.endswith("/mod.rs")
                ]
                files["src/lib.rs"] = STUB + "".join(f"pub mod {m};\n" for m in modules)
            for name, content in files.items():
                target = crate_dir / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(content)
        return self.base / "Cargo.toml"


def assert_valid_fixture(test: unittest.TestCase, manifest_path: Path) -> None:
    metadata = subprocess.run(
        ["cargo", "metadata", "--offline", "--format-version", "1", "--quiet",
         "--manifest-path", str(manifest_path)],
        capture_output=True,
        text=True,
        check=False,
    )
    test.assertEqual(metadata.returncode, 0, f"fixture workspace is broken:\n{metadata.stderr}")


def run_checker(manifest_path: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        [sys.executable, str(CHECKER), "--manifest-path", str(manifest_path)],
        capture_output=True,
        text=True,
        check=False,
    )


ADMIN_BASE_DEPS = 'kernel = { path = "../kernel" }\n'
SQLX_PATH = '{ path = "../../vendor/sqlx" }'


def violation_cases() -> dict:
    def alias(ws: FixtureWorkspace) -> None:
        ws.edit("crates/admin", "Cargo.toml", manifest(
            "admin", ADMIN_BASE_DEPS + 'db = { package = "sqlx", path = "../../vendor/sqlx" }\n'))

    def workspace_inherited(ws: FixtureWorkspace) -> None:
        ws.workspace_deps = 'sqlx = { path = "vendor/sqlx" }\n'
        ws.edit("crates/admin", "Cargo.toml", manifest(
            "admin", ADMIN_BASE_DEPS + "sqlx = { workspace = true }\n"))

    def optional(ws: FixtureWorkspace) -> None:
        ws.edit("crates/admin", "Cargo.toml", manifest(
            "admin", ADMIN_BASE_DEPS + 'sqlx = { path = "../../vendor/sqlx", optional = true }\n'))

    def target_specific(ws: FixtureWorkspace) -> None:
        ws.edit("crates/admin", "Cargo.toml", manifest(
            "admin", ADMIN_BASE_DEPS,
            f"[target.'cfg(windows)'.dependencies]\nsqlx = {SQLX_PATH}\n"))

    def dev_only(ws: FixtureWorkspace) -> None:
        ws.edit("crates/admin", "Cargo.toml", manifest(
            "admin", ADMIN_BASE_DEPS, f"[dev-dependencies]\nsqlx = {SQLX_PATH}\n"))

    def transitive(ws: FixtureWorkspace) -> None:
        ws.edit("vendor/helper", "Cargo.toml", manifest("helper", 'sqlx = { path = "../sqlx" }\n'))
        ws.edit("crates/admin", "Cargo.toml", manifest(
            "admin", ADMIN_BASE_DEPS + 'helper = { path = "../../vendor/helper" }\n'))

    def kernel_io(ws: FixtureWorkspace) -> None:
        ws.edit("crates/kernel", "Cargo.toml", manifest("kernel", f"sqlx = {SQLX_PATH}\n"))

    def web_depends_on_adapter(ws: FixtureWorkspace) -> None:
        ws.edit("crates/web", "Cargo.toml", manifest(
            "web", 'admin = { path = "../admin" }\ninfra-postgres = { path = "../infra-postgres" }\n'))

    def web_imports_repository(ws: FixtureWorkspace) -> None:
        ws.edit("crates/web", "src/lib.rs",
                "use admin::ports::AdminRepository;\npub fn handler() {}\n")

    def worker_imports_unit_of_work(ws: FixtureWorkspace) -> None:
        ws.edit("crates/worker", "src/lib.rs",
                "pub fn run(uow: &dyn kernel::ports::UnitOfWork) { let _ = uow; }\n")

    def adapter_calls_adapter(ws: FixtureWorkspace) -> None:
        ws.edit("crates/infra-sms", "Cargo.toml", manifest(
            "infra-sms",
            'admin = { path = "../admin" }\ninfra-postgres = { path = "../infra-postgres" }\n'))

    def domain_reads_clock(ws: FixtureWorkspace) -> None:
        ws.edit("crates/admin", "src/domain/mod.rs",
                "pub fn policy() -> i64 { chrono::Utc::now().timestamp() }\n")

    def application_mints_id(ws: FixtureWorkspace) -> None:
        ws.edit("crates/admin", "src/application/mod.rs",
                "pub fn service() { let _id = uuid::Uuid::now_v7(); }\n")

    return {
        "alias": (alias, ["admin", "sqlx"]),
        "workspace": (workspace_inherited, ["admin", "sqlx"]),
        "optional": (optional, ["admin", "sqlx"]),
        "target": (target_specific, ["admin", "sqlx"]),
        "dev": (dev_only, ["admin", "sqlx"]),
        "transitive": (transitive, ["admin", "sqlx", "helper"]),
        "kernel": (kernel_io, ["kernel", "sqlx"]),
        "web_adapter_edge": (web_depends_on_adapter, ["web", "infra-postgres"]),
        "web_repository_import": (web_imports_repository, ["web", "AdminRepository"]),
        "worker_unit_of_work_import": (worker_imports_unit_of_work, ["worker", "UnitOfWork"]),
        "adapter_to_adapter": (adapter_calls_adapter, ["infra-sms", "infra-postgres"]),
        "domain_clock": (domain_reads_clock, ["admin", "Utc::now"]),
        "application_id": (application_mints_id, ["admin", "Uuid::now_v7"]),
    }


class ArchitecturePolicyTest(unittest.TestCase):
    def test_architecture_policy_rejects_forbidden_context_edges(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            clean_manifest = FixtureWorkspace(Path(tmp)).write()
            assert_valid_fixture(self, clean_manifest)
            clean = run_checker(clean_manifest)
            self.assertEqual(
                clean.returncode, 0,
                f"checker must accept the clean workspace:\n{clean.stdout}{clean.stderr}",
            )

        for case, (mutate, expected) in violation_cases().items():
            with self.subTest(case=case), tempfile.TemporaryDirectory() as tmp:
                workspace = FixtureWorkspace(Path(tmp))
                mutate(workspace)
                manifest_path = workspace.write()
                assert_valid_fixture(self, manifest_path)
                result = run_checker(manifest_path)
                output = result.stdout + result.stderr
                self.assertEqual(result.returncode, 1, f"{case} must be rejected:\n{output}")
                for fragment in expected:
                    self.assertIn(fragment, output)


if __name__ == "__main__":
    unittest.main()
