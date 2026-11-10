#!/usr/bin/env python3
"""Verify release/default feature closure excludes testkit, mocks, and fakes, and dev-fakes is debug-only."""

from __future__ import annotations

import argparse
from pathlib import Path
import subprocess
import sys

FORBIDDEN_FEATURES = ("mocks", "fakes", "dev-fakes")


def run_cargo(args: list[str], cwd: Path) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["cargo", "--offline", *args],
        cwd=cwd,
        capture_output=True,
        text=True,
        check=False,
    )


def check_release_features(manifest_path: Path) -> list[str]:
    manifest_path = manifest_path.resolve()
    root = manifest_path.parent
    violations: list[str] = []

    # 1. Inspect default/release normal+build feature closure of `app` via `cargo tree`.
    default_tree = run_cargo(
        [
            "tree",
            "--manifest-path",
            str(manifest_path),
            "-p",
            "app",
            "-e",
            "normal,build",
            "-f",
            "{p} [{f}]",
        ],
        cwd=root,
    )
    if default_tree.returncode != 0:
        return [f"cargo tree failed on default graph: {default_tree.stderr.strip()}"]

    for line in default_tree.stdout.splitlines():
        if "testkit v" in line:
            violations.append(f"default/release app closure includes testkit: {line.strip()}")
        for feat in FORBIDDEN_FEATURES:
            if feat in line:
                violations.append(
                    f"default/release app closure enables forbidden feature `{feat}`: {line.strip()}"
                )

    # 2. Inspect explicit `app/dev-fakes` debug feature closure.
    fake_tree = run_cargo(
        [
            "tree",
            "--manifest-path",
            str(manifest_path),
            "-p",
            "app",
            "--features",
            "app/dev-fakes",
            "-e",
            "normal,build",
            "-f",
            "{p} [{f}]",
        ],
        cwd=root,
    )
    if fake_tree.returncode != 0:
        violations.append(f"cargo tree failed on app/dev-fakes graph: {fake_tree.stderr.strip()}")
    else:
        text = fake_tree.stdout
        if "dev-fakes" not in text or "testkit v" not in text or "fakes" not in text or "mocks" not in text:
            violations.append(
                "app/dev-fakes closure missing required explicit dev-fakes/testkit/fakes/mocks wiring"
            )

    # 3. Negative release-build invocation: `--release -p app --features dev-fakes` must fail!
    release_check = run_cargo(
        [
            "check",
            "--manifest-path",
            str(manifest_path),
            "--release",
            "-p",
            "app",
            "--features",
            "dev-fakes",
        ],
        cwd=root,
    )
    if release_check.returncode == 0:
        violations.append(
            "cargo check --release -p app --features dev-fakes unexpectedly succeeded"
        )

    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description="Tankhor release feature closure checker")
    parser.add_argument(
        "--manifest-path",
        type=Path,
        default=Path(__file__).resolve().parents[1] / "Cargo.toml",
        help="workspace Cargo.toml path",
    )
    args = parser.parse_args()

    violations = check_release_features(args.manifest_path)
    if violations:
        for violation in violations:
            print(f"release feature violation: {violation}")
        return 1

    print("release feature check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
