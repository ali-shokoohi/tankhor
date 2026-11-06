"""Workspace skeleton checks (T002; FR-001, FR-CON-01; constitution I-II)."""

import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PINNED_RUST = "1.98.1"
REQUIRED_COMPONENTS = {"rustfmt", "clippy", "llvm-tools-preview"}


def load_toml(path: Path) -> dict:
    with path.open("rb") as handle:
        return tomllib.load(handle)


class WorkspaceSkeletonTest(unittest.TestCase):
    def test_workspace_skeleton_requires_inherited_lints(self) -> None:
        root_manifest = ROOT / "Cargo.toml"
        self.assertTrue(root_manifest.is_file(), "root Cargo.toml is missing")
        self.assertTrue((ROOT / "Cargo.lock").is_file(), "Cargo.lock is missing")

        workspace = load_toml(root_manifest).get("workspace", {})
        self.assertTrue(workspace, "root Cargo.toml must declare [workspace]")
        self.assertEqual(workspace.get("resolver"), "3")

        package = workspace.get("package", {})
        self.assertEqual(package.get("edition"), "2024")
        self.assertEqual(package.get("rust-version"), PINNED_RUST)

        lints = workspace.get("lints", {})
        self.assertEqual(lints.get("rust", {}).get("unsafe_code"), "forbid")
        pedantic = lints.get("clippy", {}).get("pedantic")
        if isinstance(pedantic, dict):
            pedantic = pedantic.get("level")
        self.assertEqual(pedantic, "warn")

        toolchain_file = ROOT / "rust-toolchain.toml"
        self.assertTrue(toolchain_file.is_file(), "rust-toolchain.toml is missing")
        toolchain = load_toml(toolchain_file).get("toolchain", {})
        self.assertEqual(toolchain.get("channel"), PINNED_RUST)
        self.assertTrue(
            REQUIRED_COMPONENTS.issubset(set(toolchain.get("components", []))),
            "toolchain must pin rustfmt, clippy and llvm-tools-preview",
        )

        members = workspace.get("members", [])
        self.assertIn("crates/kernel", members)
        member_manifests = [ROOT / member / "Cargo.toml" for member in members]
        self.assertTrue(member_manifests, "workspace has no members")
        for manifest in member_manifests:
            with self.subTest(manifest=str(manifest.relative_to(ROOT))):
                self.assertTrue(manifest.is_file(), "member manifest is missing")
                member = load_toml(manifest)
                self.assertEqual(
                    member.get("lints", {}).get("workspace"),
                    True,
                    "member must inherit workspace lints",
                )
                member_package = member.get("package", {})
                self.assertEqual(member_package.get("edition"), {"workspace": True})
                self.assertEqual(member_package.get("rust-version"), {"workspace": True})


if __name__ == "__main__":
    unittest.main()
