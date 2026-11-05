# Quickstart Validation — Admin Foundation

This guide describes commands **after test-driven implementation**. The planning command
creates documentation only: there is currently no Cargo workspace, migrations, compose file,
runtime OpenAPI output or browser suite to execute. No command below is reported as run.

Contracts: [application-ports.md](contracts/application-ports.md),
[http-ui.md](contracts/http-ui.md), [test-plan.md](contracts/test-plan.md).
Persistence/roles/seeds: [data-model.md](data-model.md).

## 1. Prerequisites

- Rust 1.98.1 with rustfmt, clippy and llvm-tools-preview; Docker/Compose; Node/npm;
  Python 3 for repository-boundary/traceability checks.
- cargo-nextest, cargo-llvm-cov, cargo-mutants, cargo-deny and sqlx-cli 0.9.0. These cargo
  subcommands are not currently installed locally; install before implementation validation.
  Pin cargo-nextest **0.9.151** in CI and developer toolchain documentation because the
  integration profile uses experimental setup scripts. `just` is needed for local fixture
  cleanup and test-runner preparation.
- Untracked protected local configuration: separate migration/runtime database URLs, exact
  HTTPS origin/RP, trusted local TLS, finite non-disableable auth/session/rate-limit policy,
  OTP digest key, phone lookup/encryption keys and versioned TOTP secret-envelope key.
  Never put keys/phone numbers in command arguments, committed files or diagnostic output.
- Production additionally needs activated Kavenegar/SMS.ir accounts and approved templates.
  Their existence is not assumed. Automated validation always uses wiremock, never these
  live services. Initial admin provisioning is owner-authorised; tests use explicit local
  fixture identities/real crypto, not a production authentication bypass.

Possible tool installation, performed separately from feature implementation:

```sh
rustup component add rustfmt clippy llvm-tools-preview --toolchain 1.98.1
cargo install cargo-nextest --version 0.9.151 --locked
cargo install cargo-llvm-cov --locked
cargo install cargo-mutants --locked
cargo install cargo-deny --locked
cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features rustls,postgres
```

Pin selected tool/image/action versions in the implemented CI/lockfiles; research metadata
verification alone is not successful compilation or a working provider account.

## 2. Start storage and prepare the schema

From the repository root, with protected local configuration loaded without shell tracing:

```sh
docker compose -f deploy/compose.yml up -d
cargo run -p app -- migrate
cargo run -p app -- seed-foundation
cargo run -p app -- seed-foundation
```

Expected: PostgreSQL 16 and MinIO healthy; app migrate uses the migration role; serve/worker
use the restricted runtime role. The second seed is a no-op, with ten unchanged default
values and six OFF flags. It must not overwrite a value previously published by an admin.
No privileged admin credentials or fabricated population counts are seeded.

SQLx metadata generation/check uses the intended migrated schema and a protected DATABASE_URL:

```sh
cargo sqlx prepare --workspace -- --all-targets --all-features
cargo sqlx prepare --check --workspace -- --all-targets --all-features
```

Commit `.sqlx/` during implementation; ordinary compile/test CI explicitly forces offline
checking where required. Role/trigger enforcement is validated separately with runtime-role
integration tests, not inferred from query compilation.

## 3. Run tests in the required order

First write/run the driving test for the selected task and record its intended failure.
Only then implement the minimum code, rerun it, and run the affected-crate suite.
The implemented developer setup prepares the testkit fallback launcher once:

```sh
just test-tools
```

Nextest's integration setup script starts/reuses its own labelled detached Postgres 16 and
MinIO, waits for readiness and exports test endpoints through `$NEXTEST_ENV`; these are
separate from the developer application stack started in section 2. Each PostgreSQL test
uses `#[sqlx::test]` and the canonical workspace migrator to obtain a fresh database. MinIO
tests use unique buckets and clean them up. No schema-per-test or process-local singleton
is presented as whole-run sharing. See [integration-fixtures.md](contracts/integration-fixtures.md).

```sh
python3 -m unittest discover -s scripts/tests
cargo nextest run -p kernel
cargo nextest run -p admin
cargo nextest run -p testkit
cargo nextest run -p infra-auth
cargo nextest run -p infra-sms
cargo nextest run --profile integration -p infra-postgres
cargo nextest run -p web
cargo nextest run -p worker
cargo nextest run -p app
cargo nextest run --profile ci
```

Expected: shared fake/real port suites, real local signature/TOTP verification, SMS protocol
tests against wiremock, forbidden audit UPDATE/DELETE/TRUNCATE, rollback-on-audit-failure,
stale draft/live-baseline conflicts, fenced duplicate-worker execution and overdue retry.
When DATABASE_URL is absent, the configured Cargo/IDE test launcher starts a scoped testkit
Testcontainers fallback before the unchanged SQLx test wrapper runs, sets endpoints only on
the child process, and retains the guard through its exit. Direct IDE binary launches must
use that launcher or supply fixture endpoints. No vendor API is contacted and no testkit
fake is wired into production. Scope-local fallback storage ends with its container; persistent
failure inspection uses the shared detached path instead.

Do not supply a developer/production DATABASE_URL to the test launcher. Supplied endpoints
must match the managed test fixture configuration; reject unrelated databases before SQLx
can create or drop anything. Nextest setup exports its own isolated fixture endpoints.

SQLx retains failed shared-fixture databases until that test path is rerun or fixtures are
removed. It derives names from test paths, so independent local runs sharing the same server
are serialized. CI removes exact-run-labelled containers in an always-run post-step. Locally,
services are reused until explicit cleanup after the run:

```sh
just test-down
```

Cleanup refuses an active local run and never targets developer-stack or unrelated containers.

## 4. Validate architecture, coverage and OpenAPI

```sh
cargo fmt --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check
python3 scripts/check-architecture.py
python3 scripts/check-test-traceability.py
cargo llvm-cov nextest -p kernel --fail-under-lines 90
cargo run -p app -- openapi > target/openapi.actual.json
diff -u docs/openapi.json target/openapi.actual.json
```

Expected: contexts cannot import forbidden dependencies even through aliases/optional/target
edges; web/worker cannot bypass services; kernel coverage ≥90%; output is stable and includes
both `/api/v1/health` and `/api/v1/openapi.json`. Offline export succeeds without a running DB
or auth configuration. No `/api/openapi.json` JSON route or undocumented auth API is created.
The committed `docs/openapi.json` is first generated during implementation, not handwritten
by this plan. Mutation and corresponding ≥90% gates apply as wallet/generation/identity are
delivered; absent crates are not represented as tested.

## 5. Browser and end-to-end validation

```sh
npm ci --prefix e2e
npm exec --prefix e2e -- playwright install
npm test --prefix e2e
```

The browser harness starts real app composition against isolated PostgreSQL and wiremock,
uses real crypto/software or virtual authenticators, and supplies synthetic messages through
recorded wiremock requests without logging them. Production composition retains the real
selected provider adapters and cannot accept a test-only bypass.

Verify these observable scenarios:

1. Primary phone verification alone never opens the console; passkey and TOTP complete
   sign-in. Non-admin/direct paths and end-user cookies fail. No-JS TOTP is supported for
   an admin with the fallback enrolled. (AC-ADM-001–003/032)
2. Draft an upload-limit change within 1..25 MB; live default stays 15 until explicit
   preview/approval/publication. Inspect audit and roll back through a fresh reviewed
   version. This exercises real foundation composition without needing future media
   population counts. (AC-ADM-010/011/016/019/021/037/041)
3. A shortened lifetime with an unavailable population provider is visibly blocked, not
   presented as zero affected items. Fake/fixture tests independently prove exact positive
   count calculations and next-sweep policy. (AC-ADM-012)
4. Two admins edit one draft: second stale save conflicts. Another publication of the same
   live subject blocks an older scheduled change; an unrelated change does not. Stop/resume
   worker around due time and verify one current-prerequisite-checked effect, not backdating.
   (AC-ADM-015/018/044/045)
5. Another admin recovers a target: all old target sessions/factors fail, fresh primary
   verification and enrollment are required, and the other admin stays signed in. Audit
   failure cannot leave an unaudited recovery. (AC-ADM-005/025/043)
6. Check every screen in FA/EN and light/dark/system at 360/768/1024/1440 px, unsaved form
   preservation, RTL codes/digits, keyboard/focus/errors, zoom, reduced motion and axe.
   Verify unrounded contrast on real samples, not only stored token pairs. (AC-ADM-026–032)
7. Six flags remain OFF/not delivered; enablement cannot override absent/failed readiness.
   Production docs/assets/schema require an admin and Swagger makes no CDN/validator requests.
   (AC-ADM-033–036; API/HTTP contract)

Playwright WebKit is not actual iOS Safari. Record real-device Safari/Android Chrome checks
before release. This suite proves **J5 foundation**, not full J1–J5, media, money or AI recovery.
Full-product release-readiness must still fail until the remaining product gates are built.

## 6. Running services after validation

```sh
cargo run -p app -- serve
cargo run -p app -- worker
```

Run the two commands in separate terminals with protected runtime configuration and trusted
HTTPS termination. Open `/console` and `/api/docs` on the configured single origin. No live
provider sends are part of this validation guide. Production provider activation, credentials,
approved templates and owner provisioning are operational prerequisites handled separately.
