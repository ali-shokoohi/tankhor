#!/usr/bin/env python3
"""Task/requirement traceability and source-backed port evidence (T031).

Partial mode checks planned links and completed artifacts only. Final mode additionally
requires declared port implementations, generic-suite invocations, positive direct mock
interactions and resolved application real/fake feature graphs. This structural gate does
not replace compiling/running the suites or establish observed red/green history.
"""

import argparse
from pathlib import Path
import re
import subprocess
import json

TASK = re.compile(r"^- \[([ Xx])\] T(\d{3}) (.+)$", re.MULTILINE)
TASK_ID = re.compile(r"T(\d{3})(?:[–-]T?(\d{3}))?")
REFERENCE = re.compile(r"\b(?:AC-ADM-\d{3}|FR-\d{3})\b")
PATH = re.compile(r"`((?:crates|scripts|docs|deploy|e2e|\.github|\.config|\.cargo)/[^`]+|Cargo\.(?:toml|lock)|rust-toolchain\.toml|deny\.toml|justfile)`")


def ids(text):
    result = []
    for match in TASK_ID.finditer(text):
        first = int(match[1])
        last = int(match[2]) if match[2] else first
        result.extend(range(first, last + 1))
    return result


def paths(text):
    return [match[1] for match in PATH.finditer(text)
            if not any(char in match[1] for char in (" ", "*", "{", ","))]


def table(text, heading):
    rows = []
    enabled = False
    for line in text.splitlines():
        if line.startswith(heading):
            enabled = True
            continue
        if enabled:
            if not line.startswith("|"):
                break
            cells = [cell.strip() for cell in line.strip("|").split("|")]
            if not all(re.fullmatch(r"[- :]+", cell) for cell in cells):
                rows.append(cells)
    return rows


def uncomment(source):
    # Preserve strings while removing Rust/Python comments; comment-only symbols cannot
    # satisfy evidence. Token scanning keeps braces in quoted literals out of body parsing.
    token = re.compile(r'"(?:\\.|[^"\\])*"|\'[^\'\n]*\'|//[^\n]*|/\*.*?\*/|(?m:^\s*#[^\[\n][^\n]*)', re.DOTALL)
    return token.sub(lambda match: "" if match[0].startswith(("//", "/*", "#")) else match[0], source)


def task_source(root, description):
    return "\n".join(uncomment((root / path).read_text()) for path in paths(description)
                     if (root / path).is_file() and path.endswith((".rs", ".py")))


def function_bodies(source):
    """Return balanced Rust function bodies, ignoring quoted strings and comments."""
    source = uncomment(source)
    masked = re.sub(r'"(?:\\.|[^"\\])*"', lambda match: " " * len(match[0]), source)
    result = []
    for match in re.finditer(r"\b(?:async\s+)?fn\s+(\w+)\b[^;{]*\{", masked):
        start = match.end()
        depth = 1
        for offset in range(start, len(masked)):
            depth += (masked[offset] == "{") - (masked[offset] == "}")
            if depth == 0:
                result.append((match[1], source[start:offset]))
                break
    return result


def check(root, feature, mode):
    root, feature = Path(root), Path(feature)
    text = (feature / "tasks.md").read_text()
    records = [(int(number), state.lower() == "x", description)
               for state, number, description in TASK.findall(text)]
    errors = []
    if [number for number, _, _ in records] != list(range(1, len(records) + 1)):
        errors.append("task IDs must be unique, contiguous and ordered")
    tasks = {number: (done, description) for number, done, description in records}
    specification = (feature / "spec.md").read_text()
    requirements = set(REFERENCE.findall(specification))
    for number, done, description in records:
        for reference in REFERENCE.findall(description):
            if reference not in requirements:
                errors.append(f"T{number:03}: unknown requirement {reference}")
        implementation = re.search(r"Make pass:\s*((?:T\d{3}(?:[–-]T?\d{3})?\s*)+)", description)
        if "Make pass:" in description and not implementation:
            errors.append(f"T{number:03}: missing preceding driving test")
        if implementation:
            for driver in ids(implementation[1]):
                record = tasks.get(driver)
                if not record or driver >= number or "Write failing test:" not in record[1]:
                    errors.append(f"T{number:03}: invalid preceding driver T{driver:03}")
                elif done and not record[0]:
                    errors.append(f"T{number:03}: driver T{driver:03} is incomplete")
        if done or mode == "final":
            for path in paths(description):
                if not (root / path).exists():
                    errors.append(f"T{number:03}: missing required artifact {path}")
            if "Write failing test:" in description:
                source = task_source(root, description)
                named = [name for name in re.findall(r"`(\w+)`", description)
                         if not name.startswith(("FR", "AC"))]
                for name in named:
                    if not re.search(rf"\b(?:fn|def)\s+(?:test_)?{name}\s*[(<]", source):
                        errors.append(f"T{number:03}: driving test symbol missing: {name}")
        if mode == "final" and not done:
            errors.append(f"T{number:03}: final requires completed task")

    coverage = table(text, "| AC/FR ID |")
    covered = set()
    for cells in coverage:
        if len(cells) < 3:
            errors.append("invalid AC/FR coverage row")
            continue
        covered.update(REFERENCE.findall(cells[0]))
        for column in cells[1:3]:
            links = ids(column)
            if not links or any(link not in tasks for link in links):
                errors.append(f"invalid requirement task links: {cells[0]}")
    for reference in requirements - covered:
        # Contract/UX references are separate namespaces; local FRs and every local AC
        # must appear in the planned traceability table, not just task prose.
        errors.append(f"missing requirement coverage: {reference}")

    ports = table(text, "| Port | Trait task |")
    contract_text = (feature / "contracts/application-ports.md").read_text()
    expected_ports = set()
    for heading in ("| Port | Minimum operations |", "| Port | Production implementation |"):
        expected_ports.update(cells[0] for cells in table(contract_text, heading))
    actual_ports = [cells[0].split(" / ")[0] for cells in ports]
    if set(actual_ports) != expected_ports or len(actual_ports) != len(set(actual_ports)):
        errors.append("port inventory must match every contract port exactly once")
    for cells in ports:
        if len(cells) != 6:
            errors.append("invalid port completeness row")
            continue
        links = [ids(cell) for cell in cells[1:]]
        if any(not group or any(number not in tasks for number in group) for group in links):
            errors.append(f"{cells[0]}: missing planned port task")
            continue
        trait, contract, fake, real, mocks = links
        if not min(trait) < min(contract) < min(fake) < min(real):
            errors.append(f"{cells[0]}: fake contract must precede real adapter")
        if mode == "final":
            errors.extend(check_port(root, cells[0].split(" / ")[0], links, tasks))
    if mode == "final":
        errors.extend(check_graphs(root))
    return errors


def check_port(root, port, links, tasks):
    trait_ids, contract_ids, fake_ids, real_ids, mock_ids = links
    sources = ["\n".join(task_source(root, tasks[number][1]) for number in group) for group in links]
    trait, contract, fake, real, mock = sources
    errors = []
    if not re.search(rf"pub\s+trait\s+{port}\b", trait):
        errors.append(f"{port}: actual trait missing")
    if not re.search(r'cfg_attr\(any\(test,\s*feature\s*=\s*"mocks"\),\s*mockall::automock\)', trait) or (
        trait.find("mockall::automock") > trait.find("async_trait::async_trait")
    ) or "async_trait::async_trait" not in trait:
        errors.append(f"{port}: missing/misordered automock and async_trait")
    suites = [name for name, _ in function_bodies(contract) if name.endswith("_contract")]
    if not suites or not re.search(rf"\b(?:\w+\s*:\s*|impl\s+){port}\b", contract):
        errors.append(f"{port}: generic shared contract missing")

    def implementations(source):
        return re.findall(rf"\bimpl(?:\s*<[^{{]+?>)?\s+{port}\s+for\s+(\w+)", source)

    fakes = implementations(fake)
    adapters = implementations(real)
    if not fakes:
        errors.append(f"{port}: actual fake implementation missing")
    if len(adapters) < (2 if port == "SmsSender" else 1):
        errors.append(f"{port}: actual real adapter(s) missing")
    # Only real Rust test-function bodies count, not filenames, prose or comments.
    runner_sources = {"fake": [], "real": []}
    for base, kind in ((root / "crates/testkit/tests", "fake"), (root / "crates", "real")):
        for path in base.glob("**/*.rs"):
            if kind == "real" and not any(part.startswith("infra-") for part in path.parts):
                continue
            runner_sources[kind].extend(body for _, body in function_bodies(path.read_text()))
    for kind, types in (("fake", fakes), ("real", adapters)):
        for implementation in types:
            if not any(re.search(rf"\b{suite}(?:::<[^;]+>)?\s*\([^;]*\b{implementation}\b", body)
                       for body in runner_sources[kind] for suite in suites):
                errors.append(f"{port}: {implementation} lacks shared {kind} runner")
    positive = False
    methods = set(re.findall(r"\b(?:async\s+)?fn\s+(\w+)\s*\(", trait))
    applications = [uncomment(path.read_text()) for path in (root / "crates").glob("*/src/application/**/*.rs")]
    for _, body in function_bodies(mock):
        variables = re.findall(rf"\blet\s+(?:mut\s+)?(\w+)\s*=\s*Mock{port}::(?:new|default)\(", body)
        for variable in variables:
            expectation = re.search(rf"\b{variable}\.expect_(\w+)\(\)[^;]*\.times\(([1-9]\d*)\)", body)
            invocation = re.search(rf"\b(\w+)\([^;]*&(?:mut\s+)?{variable}\b[^;]*\)\.await", body)
            if expectation and expectation[1] in methods and invocation:
                for application in applications:
                    if not re.search(rf"\bfn\s+{invocation[1]}\s*\([^)]*\b{port}\b", application):
                        continue
                    for name, service_body in function_bodies(application):
                        if name == invocation[1] and re.search(rf"\.\s*{expectation[1]}\s*\(", service_body):
                            positive = True
    if not positive:
        errors.append(f"{port}: direct positive service mock interaction missing")
    return errors


def check_graphs(root):
    errors = []
    for fake in (False, True):
        command = ["cargo", "metadata", "--offline", "--format-version", "1", "--manifest-path", str(root / "Cargo.toml")]
        if fake:
            command += ["--features", "app/dev-fakes"]
        result = subprocess.run(command, capture_output=True, text=True, check=False)
        if result.returncode:
            errors.append(f"{'fake' if fake else 'real'} feature graph does not resolve")
            continue
        metadata = json.loads(result.stdout)
        packages = {package["id"]: package for package in metadata["packages"]}
        nodes = {node["id"]: node for node in metadata["resolve"]["nodes"]}
        roots = [key for key, package in packages.items() if package["name"] == "app"]
        if len(roots) != 1:
            errors.append("feature graph requires the actual app composition root")
            continue
        reachable, pending = set(), list(roots)
        while pending:
            current = pending.pop()
            if current in reachable:
                continue
            reachable.add(current)
            pending.extend(dep["pkg"] for dep in nodes[current]["deps"]
                           if any(kind["kind"] != "dev" for kind in dep["dep_kinds"]))
        test_features = {(packages[key]["name"], feature) for key in reachable
                         for feature in nodes[key]["features"] if feature in {"mocks", "fakes", "dev-fakes"}}
        if not fake and (test_features or any(packages[key]["name"] == "testkit" for key in reachable)):
            errors.append("real feature graph includes testkit/mocks/fakes")
        if fake and not ({("app", "dev-fakes"), ("testkit", "fakes")} <= test_features
                         and any(feature == "mocks" for _, feature in test_features)):
            errors.append("fake feature graph lacks explicit app/testkit/context test features")
    return errors


def main():
    parser = argparse.ArgumentParser(description="Task and source-backed port traceability")
    parser.add_argument("--mode", choices=("partial", "final"), default="partial")
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--feature-dir", type=Path)
    arguments = parser.parse_args()
    root = arguments.root.resolve()
    feature = arguments.feature_dir
    if feature is None:
        active = json.loads((root / ".specify/feature.json").read_text())
        feature = root / active["feature_directory"]
    errors = check(root, feature, arguments.mode)
    for error in errors:
        print("traceability violation: " + error)
    if errors:
        return 1
    print(f"traceability {arguments.mode}: passed (structural evidence; test execution remains required)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
