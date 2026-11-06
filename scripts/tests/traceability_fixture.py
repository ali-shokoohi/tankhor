"""Synthetic source-backed inventory for the T030 checker drivers; never production code."""

from pathlib import Path

PORTS = ("Clock", "IdGenerator", "UnitOfWork", "HealthProbe", "AuditRepository",
         "AdminRepository", "AdminCredentialRepository", "SessionStore", "OtpChallengeRepository",
         "RateLimitRepository", "AdminAuthenticator", "SmsSender", "SettingsRepository",
         "FlagRepository", "ChangeRepository", "PublicationRepository")


def put(root: Path, path: str, source: str):
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(source)


def fixture(root: Path, complete=False):
    feature = root / "specs/probe"
    feature.mkdir(parents=True)
    put(root, "specs/probe/spec.md", "AC-ADM-001 FR-001 FR-CON-01\n")
    put(root, "specs/probe/contracts/application-ports.md", "\n".join(
        ["| Port | Minimum operations | Fake/real contract suite |", "| --- | --- | --- |"] +
        [f"| {port} | act | `{port.lower()}_contract` |" for port in PORTS]))
    tasks = [
        "- [X] T001 Write failing test: [unit] `driving_test` in `scripts/tests/driver.py` (FR-001, AC-ADM-001).",
        "- [X] T002 Make pass: T001 implement `scripts/behavior.py` (FR-001, AC-ADM-001).",
    ]
    put(root, "scripts/tests/driver.py", "def test_driving_test():\n    assert behavior() == 1\n")
    put(root, "scripts/behavior.py", "def behavior():\n    return 1\n")
    mapping = []
    for index, port in enumerate(PORTS):
        number = 3 + index * 8
        snake = port.lower()
        # Both SMS real adapters are independently represented and independently checked.
        actuals = [f"Real{port}"] if port != "SmsSender" else ["KavenegarSmsSender", "SmsIrSmsSender"]
        checked = "X" if complete else " "
        records = [
            ("Write failing test: [unit] `port_shape`", f"crates/context/tests/{snake}_shape.rs"),
            (f"Make pass: T{number:03} define {port}", f"crates/context/src/ports/{snake}.rs"),
            (f"Write failing test: [contract] `{snake}_contract`", f"crates/testkit/src/contracts/{snake}.rs"),
            (f"Make pass: T{number+2:03} implement Fake{port}", f"crates/testkit/src/fakes/{snake}.rs"),
            (f"Write failing test: [integration] `{snake}_real`", f"crates/infra-system/tests/{snake}.rs"),
            (f"Make pass: T{number+4:03} implement {' '.join(actuals)}", f"crates/infra-system/src/{snake}.rs"),
            ("Write failing test: [mock] `service_calls_port`", f"crates/testkit/tests/mock_{snake}.rs"),
            (f"Make pass: T{number+6:03} implement service", f"crates/context/src/application/{snake}.rs"),
        ]
        for offset, (description, path) in enumerate(records):
            tasks.append(f"- [{checked}] T{number+offset:03} {description} in `{path}` (FR-001).")
        mapping.append(f"| {port} | T{number+1:03} | T{number+2:03} | T{number+3:03} | T{number+5:03} | T{number+6:03} |")
        if not complete:
            continue
        put(root, records[0][1], "#[test] fn port_shape() { assert!(true); }\n")
        put(root, records[1][1], f'#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]\n'
            f'#[async_trait::async_trait]\npub trait {port}: Send + Sync {{ async fn act(&self); }}\n')
        put(root, records[2][1], f'pub async fn {snake}_contract<P: {port}>(port: P) {{ port.act().await; }}\n')
        put(root, records[3][1], f'pub struct Fake{port};\nimpl {port} for Fake{port} {{ async fn act(&self) {{}} }}\n')
        put(root, f"crates/testkit/tests/{snake}_fake.rs",
            f'#[tokio::test] async fn fake_contract() {{ {snake}_contract(Fake{port}).await; }}\n')
        put(root, records[4][1], "\n".join(
            f'#[tokio::test] async fn {snake}_real{"" if i == 0 else str(i)}() {{ {snake}_contract({actual}).await; }}'
            for i, actual in enumerate(actuals)))
        put(root, records[5][1], "\n".join(
            f'pub struct {actual};\nimpl {port} for {actual} {{ async fn act(&self) {{}} }}'
            for actual in actuals))
        put(root, records[6][1], f'#[tokio::test] async fn service_calls_port() {{\n'
            f'let mut port = Mock{port}::new();\nport.expect_act().times(1).returning(|| ());\n'
            'service(&port).await;\n}\n')
        put(root, records[7][1], f'pub async fn service(port: &impl {port}) {{ port.act().await; }}\n')
    put(root, "specs/probe/tasks.md", "\n".join(tasks) + "\n\n## Traceability\n"
        "| AC/FR ID | Test task ID(s) | Implementation task ID(s) |\n| --- | --- | --- |\n"
        "| AC-ADM-001 / FR-001 | T001 | T002 |\n\n## Port Completeness\n"
        "| Port | Trait task | Contract suite task | Fake task | Real adapter task | Mock-test task(s) |\n"
        "| --- | --- | --- | --- | --- | --- |\n" + "\n".join(mapping))
    if complete:
        put(root, "Cargo.toml", '[workspace]\nresolver="3"\nmembers=["crates/app","crates/testkit","crates/context"]\n')
        put(root, "crates/app/Cargo.toml", '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n'
            '[features]\ndefault=[]\ndev-fakes=["dep:testkit","testkit/fakes","context/mocks"]\n'
            '[dependencies]\ncontext={path="../context"}\ntestkit={path="../testkit",optional=true}\n')
        put(root, "crates/testkit/Cargo.toml", '[package]\nname="testkit"\nversion="0.1.0"\nedition="2024"\n'
            '[features]\ndefault=[]\nfakes=[]\n[dependencies]\ncontext={path="../context"}\n')
        put(root, "crates/context/Cargo.toml", '[package]\nname="context"\nversion="0.1.0"\nedition="2024"\n'
            '[features]\ndefault=[]\nmocks=[]\n')
        for crate in ("app", "testkit", "context"):
            put(root, f"crates/{crate}/src/lib.rs", "//! Synthetic feature-graph fixture.\n")
    return feature
