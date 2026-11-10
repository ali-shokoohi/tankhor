"""Testkit feature gates and context-cycle checks (T039; FR-024, FR-CON-37; constitution III)."""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
ARCH_CHECKER = ROOT / "scripts" / "check-architecture.py"
CONTEXTS = {
    "admin",
    "identity",
    "wallet",
    "media",
    "generation",
    "catalog",
    "tryon",
    "studio",
    "notify",
    "reporting",
}


class TestkitFeaturesTest(unittest.TestCase):
    def test_testkit_features_do_not_create_context_cycles(self) -> None:
        manifest_path = ROOT / "crates" / "testkit" / "Cargo.toml"
        lib_path = ROOT / "crates" / "testkit" / "src" / "lib.rs"
        contracts_mod = ROOT / "crates" / "testkit" / "src" / "contracts" / "mod.rs"
        fakes_mod = ROOT / "crates" / "testkit" / "src" / "fakes" / "mod.rs"

        self.assertTrue(manifest_path.is_file(), "testkit Cargo.toml missing")
        self.assertTrue(lib_path.is_file(), "testkit src/lib.rs missing")
        self.assertTrue(contracts_mod.is_file(), "testkit src/contracts/mod.rs missing")
        self.assertTrue(fakes_mod.is_file(), "testkit src/fakes/mod.rs missing")

        manifest = tomllib.loads(manifest_path.read_text(encoding="utf-8"))
        features = manifest.get("features", {})
        self.assertEqual(features.get("default"), [], "testkit default features must be empty")
        self.assertIn("fakes", features, "testkit must expose an explicit fakes feature")
        self.assertIn("contracts", features, "testkit must expose an explicit contracts feature")

        lib_source = lib_path.read_text(encoding="utf-8")
        self.assertRegex(
            lib_source,
            r'#\[cfg\(feature\s*=\s*"fakes"\)\]\s*pub mod fakes;',
            "fakes module must be gated behind feature = \"fakes\"",
        )
        self.assertIn("pub mod contracts;", lib_source, "contracts module must be registered")

        # Generic contract suites must stay independent of infra-* crates and SQLx/reqwest types.
        contracts_dir = contracts_mod.parent
        for rs_file in contracts_dir.rglob("*.rs"):
            text = rs_file.read_text(encoding="utf-8")
            self.assertNotRegex(
                text,
                r"\b(infra_postgres|infra_auth|infra_sms|infra_system|sqlx|reqwest)\b",
                f"{rs_file.relative_to(ROOT)} must remain infrastructure-independent",
            )

        # Fake integration test targets in testkit/tests/ must declare required-features = ["fakes"] in Cargo.toml.
        declared_tests = {
            entry["name"]: entry.get("required-features", [])
            for entry in manifest.get("test", [])
        }
        tests_dir = ROOT / "crates" / "testkit" / "tests"
        for test_file in sorted(tests_dir.glob("*.rs")):
            stem = test_file.stem
            if stem.startswith(("fixture_", "integration_")):
                continue
            self.assertIn(
                stem,
                declared_tests,
                f"fake/application test target {stem} must declare [[test]] with required-features",
            )
            self.assertIn(
                "fakes",
                declared_tests[stem],
                f"test target {stem} must require feature 'fakes'",
            )

        # Workspace metadata check: no kernel or context crate may depend on testkit.
        meta_proc = subprocess.run(
            ["cargo", "metadata", "--format-version", "1", "--all-features", "--quiet"],
            cwd=ROOT,
            capture_output=True,
            text=True,
            check=True,
        )
        metadata = json.loads(meta_proc.stdout)
        packages = {pkg["id"]: pkg["name"] for pkg in metadata["packages"]}
        for node in metadata["resolve"]["nodes"]:
            pkg_name = packages[node["id"]]
            if pkg_name == "kernel" or pkg_name in CONTEXTS:
                dep_names = {packages[dep["pkg"]] for dep in node["deps"]}
                self.assertNotIn(
                    "testkit",
                    dep_names,
                    f"{pkg_name} must never depend on testkit",
                )

        # Architecture checker must explicitly reject a context -> testkit dependency edge.
        with tempfile.TemporaryDirectory() as tmp:
            base = Path(tmp)
            (base / "Cargo.toml").write_text(
                '[workspace]\nresolver = "3"\nmembers = ["crates/kernel", "crates/admin", "crates/testkit"]\n',
                encoding="utf-8",
            )
            for crate, deps in (
                ("kernel", ""),
                ("testkit", 'kernel = { path = "../kernel" }\n'),
                ("admin", 'kernel = { path = "../kernel" }\ntestkit = { path = "../testkit" }\n'),
            ):
                cdir = base / "crates" / crate / "src"
                cdir.mkdir(parents=True, exist_ok=True)
                (base / "crates" / crate / "Cargo.toml").write_text(
                    f'[package]\nname = "{crate}"\nversion = "0.1.0"\nedition = "2024"\npublish = false\n\n'
                    f"[dependencies]\n{deps}",
                    encoding="utf-8",
                )
                (cdir / "lib.rs").write_text("pub fn marker() {}\n", encoding="utf-8")
            result = subprocess.run(
                [sys.executable, str(ARCH_CHECKER), "--manifest-path", str(base / "Cargo.toml")],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(
                result.returncode,
                1,
                f"check-architecture.py must reject admin -> testkit:\n{result.stdout}{result.stderr}",
            )
            self.assertIn("testkit", result.stdout + result.stderr)

        # CI workflows must run testkit fake/application targets with fakes enabled.
        for wf_name in ("ci", "nightly"):
            wf = json.loads((ROOT / f".github/workflows/{wf_name}.yml").read_text(encoding="utf-8"))
            for job in wf["jobs"].values():
                commands = "\n".join(step.get("run", "") for step in job["steps"])
                self.assertRegex(
                    commands,
                    r"cargo nextest run[^\n]*testkit/fakes|cargo nextest run[^\n]*-p testkit --features fakes",
                    f"{wf_name}.yml must run testkit targets with fakes enabled",
                )


if __name__ == "__main__":
    unittest.main()
