#!/usr/bin/env python3
"""Architecture boundary checker (T005; FR-001, FR-CON-01; constitution II).

Uses the fully resolved `cargo metadata` graph (all features, all targets, every dependency
kind) and a source scan to enforce boundaries that cargo-deny cannot express on its own:

* kernel and context crates have no I/O or vendor crates, directly or transitively, even
  through renamed, workspace-inherited, optional, target-specific or dev edges;
* context crates do not depend on adapters, web, worker or app;
* adapters (infra-*) do not depend on other adapters;
* web and worker do not depend on adapters/SQL and do not name repositories or UnitOfWork;
* kernel and context domain/application code never read the system clock or mint IDs.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
from collections import deque
from pathlib import Path

CONTEXTS = frozenset(
    {
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
)
KERNEL = "kernel"
ENTRY_POINTS = frozenset({"web", "worker"})
COMPOSITION_ROOT = "app"

FORBIDDEN_IO = re.compile(
    r"^(sqlx(-.*)?|axum(-.*)?|aws-.*|reqwest(-.*)?|hyper(-.*)?|tower-http|tokio-postgres|"
    r"postgres|askama(_.*)?|utoipa-axum|utoipa-swagger-ui|webauthn-rs(-.*)?|totp-rs|"
    r"aes-gcm|testcontainers(-.*)?|wiremock)$"
)
SQL_CRATE = re.compile(r"^(sqlx(-.*)?|tokio-postgres|postgres)$")

DIRECT_TIME_OR_ID = re.compile(
    r"\b(Utc::now|Local::now|SystemTime::now|Instant::now|OffsetDateTime::now_utc|"
    r"OffsetDateTime::now_local|Uuid::now_v7|Uuid::new_v4|Uuid::new_v7|thread_rng|rand::random)\b"
)
STORAGE_PORT = re.compile(r"\b([A-Z]\w*Repository|UnitOfWork|TxHandle)\b")

LINE_COMMENT = re.compile(r"//[^\n]*")
BLOCK_COMMENT = re.compile(r"/\*.*?\*/", re.DOTALL)


def is_adapter(name: str) -> bool:
    return name.startswith("infra-")


def load_metadata(manifest_path: str | None) -> dict:
    command = ["cargo", "metadata", "--format-version", "1", "--all-features", "--quiet"]
    if manifest_path:
        command += ["--manifest-path", manifest_path]
    result = subprocess.run(command, capture_output=True, text=True, check=False)
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        raise SystemExit(2)
    return json.loads(result.stdout)


class Graph:
    def __init__(self, metadata: dict) -> None:
        self.packages = {pkg["id"]: pkg for pkg in metadata["packages"]}
        self.members = set(metadata["workspace_members"])
        self.edges: dict[str, list[tuple[str, set[str | None]]]] = {}
        for node in metadata["resolve"]["nodes"]:
            self.edges[node["id"]] = [
                (dep["pkg"], {kind["kind"] for kind in dep["dep_kinds"]}) for dep in node["deps"]
            ]

    def name(self, package_id: str) -> str:
        return self.packages[package_id]["name"]

    def walk(self, root: str, root_kinds: set[str | None] | None):
        """Yield (package_id, path) for every package reachable from root.

        root_kinds limits which edge kinds may leave the root (None = every kind). Beyond
        the root only normal/build edges exist in the resolved graph that matter at runtime.
        """
        seen = {root}
        queue = deque([(root, [root])])
        while queue:
            current, path = queue.popleft()
            for dep, kinds in self.edges.get(current, []):
                if current == root and root_kinds is not None and not kinds & root_kinds:
                    continue
                if current != root and not kinds & {None, "build"}:
                    continue
                if dep in seen:
                    continue
                seen.add(dep)
                dep_path = path + [dep]
                yield dep, dep_path
                queue.append((dep, dep_path))

    def direct(self, root: str, kinds: set[str | None] | None):
        for dep, dep_kinds in self.edges.get(root, []):
            if kinds is None or dep_kinds & kinds:
                yield dep


def render(graph: Graph, path: list[str]) -> str:
    return " -> ".join(graph.name(package_id) for package_id in path)


def strip_comments(source: str) -> str:
    return LINE_COMMENT.sub("", BLOCK_COMMENT.sub("", source))


def scan(directory: Path, pattern: re.Pattern[str]):
    if not directory.is_dir():
        return
    for path in sorted(directory.rglob("*.rs")):
        text = strip_comments(path.read_text(encoding="utf-8"))
        for number, line in enumerate(text.splitlines(), start=1):
            for match in pattern.finditer(line):
                yield path, number, match.group(0)


def check(metadata: dict) -> list[str]:
    graph = Graph(metadata)
    root = Path(metadata["workspace_root"])
    violations: list[str] = []
    normal = {None, "build"}

    for member in sorted(graph.members, key=graph.name):
        name = graph.name(member)
        crate_dir = Path(graph.packages[member]["manifest_path"]).parent

        if name == KERNEL or name in CONTEXTS:
            for dep, path in graph.walk(member, None):
                dep_name = graph.name(dep)
                if FORBIDDEN_IO.match(dep_name):
                    violations.append(
                        f"{name}: forbidden I/O dependency {dep_name} via {render(graph, path)}"
                    )
                elif dep in graph.members and (
                    is_adapter(dep_name)
                    or dep_name in ENTRY_POINTS
                    or dep_name == COMPOSITION_ROOT
                    or dep_name == "testkit"
                ):
                    violations.append(
                        f"{name}: must not depend on {dep_name} via {render(graph, path)}"
                    )
            source_dirs = (
                [crate_dir / "src"]
                if name == KERNEL
                else [crate_dir / "src" / "domain", crate_dir / "src" / "application"]
            )
            for directory in source_dirs:
                for path, number, call in scan(directory, DIRECT_TIME_OR_ID):
                    location = path.relative_to(root)
                    violations.append(
                        f"{name}: direct {call} in {location}:{number}; use Clock/IdGenerator ports"
                    )

        if is_adapter(name):
            for dep in graph.direct(member, normal):
                dep_name = graph.name(dep)
                if dep in graph.members and is_adapter(dep_name):
                    violations.append(f"{name}: adapter must not depend on adapter {dep_name}")

        if name in ENTRY_POINTS:
            for dep, path in graph.walk(member, normal):
                dep_name = graph.name(dep)
                if (dep in graph.members and is_adapter(dep_name)) or SQL_CRATE.match(dep_name):
                    violations.append(
                        f"{name}: must call application services, not {dep_name} "
                        f"(via {render(graph, path)})"
                    )
            for path, number, symbol in scan(crate_dir / "src", STORAGE_PORT):
                location = path.relative_to(root)
                violations.append(
                    f"{name}: references storage port {symbol} in {location}:{number}; "
                    "use application services"
                )

    return violations


def main() -> int:
    parser = argparse.ArgumentParser(description="Tankhor architecture boundary checker")
    parser.add_argument("--manifest-path", help="workspace Cargo.toml (default: current)")
    args = parser.parse_args()

    violations = check(load_metadata(args.manifest_path))
    if violations:
        for violation in violations:
            print(f"architecture violation: {violation}")
        return 1
    print("architecture check passed")
    return 0


if __name__ == "__main__":
    sys.exit(main())
