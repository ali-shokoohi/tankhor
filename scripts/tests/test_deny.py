"""cargo-deny ban checks (T006; FR-001, FR-CON-01).

Builds disposable workspaces whose `sqlx` is a local path package, then runs the
repository's cargo-deny configuration against
them with `--config`. No registry or network access is needed.
"""

import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def package(name: str, deps: str = "") -> str:
    return (
        f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n'
        f"[dependencies]\n{deps}"
    )


def write_workspace(base: Path, admin_deps: str) -> Path:
    crates = {
        "vendor/sqlx": package("sqlx"),
        "crates/kernel": package("kernel"),
        "crates/admin": package("admin", 'kernel = { path = "../kernel" }\n' + admin_deps),
        "crates/infra-postgres": package(
            "infra-postgres",
            'admin = { path = "../admin" }\nsqlx = { path = "../../vendor/sqlx" }\n',
        ),
    }
    members = ", ".join(f'"{path}"' for path in crates)
    (base / "Cargo.toml").write_text(f'[workspace]\nresolver = "3"\nmembers = [{members}]\n')
    for path, manifest in crates.items():
        (base / path / "src").mkdir(parents=True)
        (base / path / "Cargo.toml").write_text(manifest)
        (base / path / "src" / "lib.rs").write_text("")
    return base / "Cargo.toml"


def deny_bans(manifest: Path) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["cargo", "deny", "--color", "never", "--offline", "--manifest-path", str(manifest),
         "--config", str(ROOT / "deny.toml"), "check", "bans"],
        capture_output=True,
        text=True,
        check=False,
    )


@unittest.skipUnless(shutil.which("cargo-deny"), "setup blocker: cargo-deny is not installed")
class CargoDenyBansTest(unittest.TestCase):
    def test_fixture_tonic_axum_is_allowed_but_context_axum_is_rejected(self) -> None:
        # T010 owner-approved wrapper correction; FR-001/FR-CON-01. Exercise the
        # real cargo-deny policy and the complementary transitive architecture check.
        for context_edge in ("", 'axum = { path = "../../vendor/axum" }\n',
                             'tonic = { path = "../../vendor/tonic" }\n'):
            with self.subTest(context_edge=context_edge), tempfile.TemporaryDirectory() as tmp:
                base = Path(tmp)
                crates = {
                    "vendor/axum": package("axum"),
                    "vendor/tonic": package("tonic", 'axum = { path = "../axum" }\n'),
                    "crates/testkit": package("testkit", 'tonic = { path = "../../vendor/tonic" }\n'),
                    "crates/admin": package("admin", context_edge),
                }
                members = ", ".join(f'"{path}"' for path in crates)
                manifest = base / "Cargo.toml"
                manifest.write_text(f'[workspace]\nresolver = "3"\nmembers = [{members}]\n')
                for path, content in crates.items():
                    directory = base / path
                    (directory / "src").mkdir(parents=True)
                    (directory / "Cargo.toml").write_text(content)
                    (directory / "src/lib.rs").write_text("")

                result = deny_bans(manifest)
                if context_edge.startswith("axum"):
                    self.assertNotEqual(result.returncode, 0, result.stderr)
                    self.assertIn("error[banned]", result.stderr)
                    self.assertIn("admin", result.stderr)
                else:
                    self.assertEqual(result.returncode, 0, result.stderr)

                architecture = subprocess.run(
                    ["python3", str(ROOT / "scripts/check-architecture.py"),
                     "--manifest-path", str(manifest)],
                    capture_output=True, text=True, check=False,
                )
                self.assertEqual(architecture.returncode, 1 if context_edge else 0,
                                 architecture.stdout + architecture.stderr)
                if context_edge:
                    self.assertIn("admin", architecture.stdout)
                    self.assertIn("axum", architecture.stdout)

    def test_cargo_deny_rejects_sqlx_context_dependency(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            allowed = deny_bans(write_workspace(Path(tmp), ""))
            self.assertEqual(
                allowed.returncode, 0,
                f"infra-postgres -> sqlx must be allowed:\n{allowed.stderr}",
            )

        with tempfile.TemporaryDirectory() as tmp:
            rejected = deny_bans(
                write_workspace(Path(tmp), 'sqlx = { path = "../../vendor/sqlx" }\n')
            )
            self.assertNotEqual(
                rejected.returncode, 0,
                f"admin -> sqlx must be rejected by cargo-deny:\n{rejected.stderr}",
            )
            self.assertIn("sqlx", rejected.stderr)
            self.assertIn("admin", rejected.stderr)


if __name__ == "__main__":
    unittest.main()
