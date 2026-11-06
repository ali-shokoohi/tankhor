"""Nextest profile selection checks (T008; FR-024, FR-CON-37).

Checks the current workspace and staged fast-only/bootstrap/complete target inventories.
The complete probe retains the final isolation assertions; inactive profiles must select
nothing. Listing probe tests never starts a service or runs a setup script.
"""

import json
import re
import shutil
import subprocess
import tempfile
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CONFIG = ROOT / ".config" / "nextest.toml"
PINNED_NEXTEST = "0.9.151"

PROBE_MANIFEST = """[package]
name = "testkit"
version = "0.1.0"
edition = "2024"
publish = false

[workspace]
"""
PROBE_FILES = {
    "src/lib.rs": "#[cfg(test)]\nmod tests {\n    #[test]\n    fn unit_probe() {}\n}\n",
    "tests/plain_probe.rs": "#[test]\nfn plain_probe() {}\n",
    "tests/integration_probe.rs": "#[test]\nfn integration_probe() {}\n",
    "tests/fixture_bootstrap_probe.rs": "#[test]\nfn fixture_bootstrap_probe() {}\n",
}
PACKAGE_PREDICATE = re.compile(r"\b(?:package|deps|rdeps)\(\s*=?([A-Za-z0-9_-]+)\s*\)")


PROFILES = ("default", "ci", "integration", "fixture-bootstrap")
FAMILIES = {"integration": "integration_", "fixture-bootstrap": "fixture_bootstrap_"}


def write_probe(base: Path, families: set[str]) -> Path:
    (base / "Cargo.toml").write_text(PROBE_MANIFEST)
    for name, content in PROBE_FILES.items():
        if name.startswith("tests/integration_") and "integration" not in families:
            continue
        if name.startswith("tests/fixture_bootstrap_") and "fixture-bootstrap" not in families:
            continue
        (base / name).parent.mkdir(parents=True, exist_ok=True)
        (base / name).write_text(content)
    return base / "Cargo.toml"


def list_tests(manifest: Path, profile: str, config: Path = CONFIG) -> subprocess.CompletedProcess:
    return subprocess.run(
        ["cargo", "nextest", "list", "--manifest-path", str(manifest),
         "--config-file", str(config), "--profile", profile,
         "--message-format", "json", "--color", "never"],
        capture_output=True,
        text=True,
        check=False,
    )


def selected_binaries(manifest: Path, profile: str, config: Path = CONFIG) -> set[str]:
    result = list_tests(manifest, profile, config)
    if result.returncode != 0:
        raise AssertionError(f"nextest list --profile {profile} failed:\n{result.stderr}")
    suites = json.loads(result.stdout)["rust-suites"]
    return {
        suite["binary-name"]
        for suite in suites.values()
        if any(case["filter-match"]["status"] == "matches" for case in suite["testcases"].values())
    }


def workspace_inventory() -> tuple[set[str], set[str]]:
    result = subprocess.run(
        ["cargo", "metadata", "--no-deps", "--format-version", "1", "--offline", "--quiet"],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    packages = json.loads(result.stdout)["packages"]
    return (
        {package["name"] for package in packages},
        {target["name"] for package in packages for target in package["targets"]},
    )


def filters_in(config: dict) -> list[str]:
    found = []
    for profile in config.get("profile", {}).values():
        if "default-filter" in profile:
            found.append(profile["default-filter"])
        found += [entry["filter"] for entry in profile.get("overrides", []) if "filter" in entry]
        found += [entry["filter"] for entry in profile.get("scripts", []) if "filter" in entry]
    return found


def staged_config(source: str, families: set[str]) -> str:
    """Exercise the approved later harness edits against genuine probe target files.

    These are test-only config fixtures, not generated production configuration. Strip
    storage overrides/hooks before constructing each stage, while retaining all other
    actual profile settings, groups and timeouts.
    """
    source = re.sub(
        r"(?ms)^\[\[profile\.(?:integration|fixture-bootstrap)\.(?:overrides|scripts)\]\]\n.*?(?=^\[|\Z)",
        "", source,
    )
    source = re.sub(r"(?ms)^\[scripts\.setup\.[^\n]+\]\n.*?(?=^\[|\Z)", "", source)
    exclusions = [f"binary(/^{FAMILIES[name]}/)" for name in sorted(families)]
    fast = "not (" + " | ".join(exclusions) + ")" if exclusions else "all()"
    for name in PROFILES:
        selection = fast if name in {"default", "ci"} else (
            f"binary(/^{FAMILIES[name]}/)" if name in families else "none()"
        )
        source, count = re.subn(
            rf"(?m)(^\[profile\.{re.escape(name)}\]\n(?:[ \t]*#[^\n]*\n)*)default-filter = [^\n]+",
            rf"\g<1>default-filter = '{selection}'", source,
        )
        if count != 1:
            raise AssertionError(f"profile.{name} must have an explicit default-filter")
    for name in sorted(families):
        group = "integration-storage" if name == "integration" else "fixture-bootstrap"
        source += (
            f"\n[[profile.{name}.overrides]]\n"
            f"filter = 'binary(/^{FAMILIES[name]}/)'\ntest-group = \"{group}\"\n"
        )
    return source


@unittest.skipUnless(shutil.which("cargo-nextest"), "setup blocker: cargo-nextest is not installed")
class NextestProfilesTest(unittest.TestCase):
    def test_real_workspace_profiles_select_exact_fixture_binaries(self) -> None:
        expectations = {
            "default": {"fixture_ownership"},
            "ci": {"fixture_ownership"},
            "fixture-bootstrap": {"fixture_bootstrap_postgres", "fixture_bootstrap_roles"},
            "integration": {"integration_shared_fixture"},
        }
        for profile, expected in expectations.items():
            with self.subTest(real_workspace=profile):
                self.assertEqual(selected_binaries(ROOT / "Cargo.toml", profile), expected)

    def test_nextest_profiles_select_storage_tests_only(self) -> None:
        self.assertTrue(CONFIG.is_file(), ".config/nextest.toml is missing")
        with CONFIG.open("rb") as handle:
            config = tomllib.load(handle)

        self.assertIn("setup-scripts", config.get("experimental", []),
                      "experimental setup-scripts must be enabled at the top level")
        profiles = config.get("profile", {})
        for name in ("default", "ci", "integration", "fixture-bootstrap"):
            self.assertIn(name, profiles, f"profile.{name} is missing")

        for profile in PROFILES:
            with self.subTest(real_workspace_load=profile):
                selected_binaries(ROOT / "Cargo.toml", profile)

        # Bootstrap inherits from default, so neither may register shared setup.
        for name in ("default", "ci", "fixture-bootstrap"):
            self.assertNotIn("scripts", profiles[name], f"profile.{name} must not run setup")

        groups = config.get("test-groups", {})
        for name in ("integration", "fixture-bootstrap"):
            with self.subTest(budget=name):
                profile = profiles[name]
                assigned = {
                    entry.get("test-group")
                    for entry in profile.get("overrides", [])
                    if entry.get("test-group")
                }
                threads = profile.get("test-threads")
                self.assertIsInstance(threads, int, f"profile.{name} must bound concurrency")
                self.assertGreaterEqual(threads, 1)
                required_group = "integration-storage" if name == "integration" else name
                self.assertIn(required_group, groups)
                self.assertIsInstance(groups[required_group].get("max-threads"), int)
                self.assertGreaterEqual(groups[required_group]["max-threads"], 1)
                for group in assigned:
                    threads = groups.get(group, {}).get("max-threads")
                    self.assertIsInstance(threads, int, f"test group {group} must be bounded")
                    self.assertGreaterEqual(threads, 1)
                slow = profile.get("slow-timeout", {})
                self.assertIsInstance(slow, dict, f"profile.{name} needs a structured slow-timeout")
                self.assertIn("period", slow)
                self.assertIsInstance(slow.get("terminate-after"), int,
                                      f"profile.{name} must terminate hung tests")

        present, targets = workspace_inventory()
        for expression in filters_in(config):
            for package in PACKAGE_PREDICATE.findall(expression):
                self.assertIn(package, present, f"filter names absent package: {expression}")

        development = (ROOT / "docs" / "development.md").read_text(encoding="utf-8")
        self.assertIn(f"cargo-nextest --version {PINNED_NEXTEST}", development)

        active = {name for name, prefix in FAMILIES.items()
                  if any(target.startswith(prefix) for target in targets)}
        tasks = (ROOT / "specs/001-admin-foundation/tasks.md").read_text(encoding="utf-8")
        for task, family in (("T013", "fixture-bootstrap"), ("T021", "integration")):
            if re.search(rf"^- \[[Xx]\] {task} ", tasks, re.MULTILINE):
                self.assertIn(family, active, f"completed {task} requires a genuine target")
        for name in FAMILIES:
            if name in active:
                self.assertNotEqual(profiles[name]["default-filter"], "none()")
                self.assertTrue(profiles[name].get("overrides"),
                                f"active {name} must assign a bounded group")
            else:
                self.assertEqual(profiles[name]["default-filter"], "none()",
                                 f"inactive {name} must select no tests")
                self.assertNotIn("overrides", profiles[name])
                self.assertNotIn("scripts", profiles[name])

        # This previously missing assertion catches unmatched predicates in every profile.
        for profile in PROFILES:
            with self.subTest(real_workspace=profile):
                selected = selected_binaries(ROOT / "Cargo.toml", profile)
                if profile in FAMILIES:
                    self.assertTrue(all(name.startswith(FAMILIES[profile]) for name in selected))
                else:
                    self.assertFalse(any(name.startswith(tuple(FAMILIES.values()))
                                         for name in selected))

        # The actual config must select each genuine probe target for its current stage.
        with tempfile.TemporaryDirectory() as tmp:
            manifest = write_probe(Path(tmp), active)
            expectations = {
                "default": {"testkit", "plain_probe"},
                "ci": {"testkit", "plain_probe"},
                "integration": {"integration_probe"} if "integration" in active else set(),
                "fixture-bootstrap": ({"fixture_bootstrap_probe"}
                                      if "fixture-bootstrap" in active else set()),
            }
            for profile, expected in expectations.items():
                with self.subTest(profile=profile):
                    self.assertEqual(selected_binaries(manifest, profile), expected)

        # Preserve final selection assertions now, while also covering intermediate stages.
        source = CONFIG.read_text(encoding="utf-8")
        for families in (set(), {"fixture-bootstrap"}, set(FAMILIES)):
            with self.subTest(stage=sorted(families)), tempfile.TemporaryDirectory() as tmp:
                base = Path(tmp)
                manifest = write_probe(base, families)
                config = base / "nextest.toml"
                config.write_text(staged_config(source, families), encoding="utf-8")
                for profile in PROFILES:
                    expected = {"testkit", "plain_probe"} if profile in {"default", "ci"} else (
                        {FAMILIES[profile] + "probe"} if profile in families else set()
                    )
                    self.assertEqual(selected_binaries(manifest, profile, config), expected)

        # A future-only binary/package predicate is rejected before test discovery, even
        # when it appears in an unselected profile. The probe itself is otherwise runnable.
        for predicate, diagnostic in (
            ("binary(/^integration_/)", "no binary names matched"),
            ("package(absent-future-package)", "no packages matched"),
        ):
            with self.subTest(premature=predicate), tempfile.TemporaryDirectory() as tmp:
                base = Path(tmp)
                manifest = write_probe(base, set())
                config = base / "nextest.toml"
                config.write_text(staged_config(source, set()), encoding="utf-8")
                self.assertEqual(selected_binaries(manifest, "default", config),
                                 {"testkit", "plain_probe"})
                config.write_text(re.sub(
                    r"(?m)(^\[profile.integration\]\n(?:[ \t]*#[^\n]*\n)*)default-filter = 'none\(\)'",
                    rf"\g<1>default-filter = '{predicate}'", config.read_text(),
                ))
                result = list_tests(manifest, "default", config)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(diagnostic, result.stderr)


if __name__ == "__main__":
    unittest.main()
