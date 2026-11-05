# Integration Fixture Contract — Owner Decision 2026-11-04

Design only; no containers/scripts are created or started by planning. This owner-supplied
model supersedes the original per-compiled-binary wording and initial per-process primary
proposal. Trace: user planning clarification; constitution Principles I–III; FR-CON-37 and
NFR-SEC-09 database ownership/isolation validation. Tests: [test-plan.md](test-plan.md).

## 1. Shared-run services and pinned nextest

Pin **cargo-nextest 0.9.151** in CI installation and developer toolchain documentation. Rust
remains 1.98.1. Setup scripts are experimental; do not silently follow newer config syntax.
The declarative target in `.config/nextest.toml` is:

```toml
experimental = ["setup-scripts"]

[scripts.setup.integration-services]
command = ["bash", "scripts/setup-integration.sh"]

[[profile.integration.scripts]]
filter = 'binary(/^integration_/) & (rdeps(infra-postgres) | rdeps(testkit))'
setup = "integration-services"
```

This is a design fragment, not the complete config. The profile also selects integration
binaries, sets a bounded test group and defines timeouts. Default/ci exclude those binaries;
pure/mock/HTTP tests do not start services. Verify filters against actual test inventory:
no required integration test may miss setup. Enable `experimental` at the top level, not
inside a profile; use `scripts.setup`, not deprecated `script.*` definitions.

For each run, the idempotent setup script:

1. Identifies repository fixture scope and, in CI, exact job/run scope.
2. Acquires a setup lock, claims a live-run lease and starts/reuses one detached PostgreSQL
   16 and one detached MinIO container carrying exact ownership labels.
3. Uses dynamic loopback ports and refuses unrelated/developer/production containers.
4. Waits for PostgreSQL connectivity and MinIO readiness, not just container existence.
5. Provisions cluster-global fixture roles once idempotently; per-test migrations do not
   race to CREATE/ALTER the same global roles.
6. Appends DATABASE_URL and S3_ENDPOINT to `$NEXTEST_ENV` for matching tests. Failure ends
   setup/run with an error, never a skipped-green suite.

No nextest child owns a Testcontainers guard for these detached shared services. Local
services persist between runs; CI services belong to exact run labels. Mutable fixture
state and secrets stay outside committed files.

## 2. Fresh database per PostgreSQL test

Adapter-hosted tests use `#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]` with the
canonical workspace migrator from `crates/infra-postgres/migrations`. SQLx creates and
migrates a fresh database before entering a pool/options-argument test. Do not replace
database isolation with schema resets or copy production migrations into testkit.

The fixture URL can create/drop databases on the dedicated fixture server. Its pool is
not the runtime adapter pool. Migrations establish intended migration-owner objects and
restricted grants inside each database; adapter/application tests connect to that exact
database with runtime credentials. Ownership and privilege tests prove the distinction.

Pinned SQLx 0.9.0 constraints:

- Database names are deterministic hashes of test paths. Retry/rerun replaces that test's
  previous database. Failure inspection lasts until the same case is rerun or fixture
  storage is removed, not indefinitely.
- Independent local runs sharing a PostgreSQL server must be serialized. Separate root
  databases on the same server do not isolate cluster-wide generated names.
- The setup lock alone is insufficient: retain an external run lease bound to the live
  nextest owner process and its start identity. Reject a concurrent independent run;
  reclaim a stale lease only after verifying its owner exited. Direct nextest invocation
  must be safe even without a whole-run `just` wrapper lock.
- No child calls broad `cleanup_test_dbs()`: that sweep can race with another test.
  Normal SQLx setup uses per-test cleanup rather than the broad sweep.
- Successful database drop is best effort in SQLx. Close extra app/runtime pools and
  verify cleanup in fixture tests; a passing body alone is not proof the drop succeeded.
- SQLx connection limits are process-local, not nextest-wide. Bound test concurrency
  and budget administrator, migration, SQLx and application pool connections together.

Schema privileges still matter inside each database; isolation does not waive runtime-role,
append-only trigger, constraint or UoW atomicity tests.

## 3. MinIO bucket ownership

Each test needing MinIO gets a random unique bucket namespaced to the run. Helpers remove
its objects/bucket on normal success/error/panic teardown, awaiting async cleanup before
propagating failure. A killed process cannot promise async cleanup; exact-run cleanup handles
orphans. Never remove resources outside managed fixture scope. These are infrastructure
checks, not proof of Feature 004 private-media URLs, consent or retention.

## 4. Safe fallback for Cargo and configured IDE execution

The same SQLx tests must work without pre-set DATABASE_URL. SQLx sets up its database
**before the user body**; starting Testcontainers there is too late. Its public test-context
hook does not accept alternate root options. Rust 2024 global environment mutation is unsafe;
do not evade `unsafe_code=forbid` through dotenv/older-edition helpers or constructors.

Use a testkit process launcher via the repository's test-aware Cargo target runner and
configured IDE launch. `just test-tools` prepares it before developer test runs:

1. Pass non-test executables, discovery/list/help and non-storage tests through unchanged
   without Docker. Cargo runners also see `cargo run`, bench and test discovery.
2. With validated fixture endpoints supplied, execute the same binary/arguments without
   starting another service pair. Validation requires the managed test fixture identity;
   reject an unrelated developer/production DATABASE_URL before SQLx setup.
3. For storage tests without DATABASE_URL, start scoped Testcontainers PostgreSQL (and
   MinIO only when needed), retain guards, then spawn the exact same binary/arguments
   with safe `Command::env` child-only endpoints and protected credentials.
4. Await the child, forward failures/signals/exit status, then release guards. Never call
   `std::env::set_var`; the child's endpoints prevent fallback recursion.

Do not configure a nextest wrapper overriding this Cargo runner. No extra experimental
wrapper feature is needed. A direct IDE bare-binary launch bypasses Cargo's runner and must
explicitly use the launcher or supply fixture endpoints. Do not promise fallback for an
unconfigured direct launch. The tested launcher, not a production flag, owns this behaviour.

Fallback containers disappear on teardown even after failure: a failed database may remain
inside temporarily but is not inspectable after removal. Persistent failure inspection uses
the shared detached path. No production code imports testkit.

## 5. Secrets and cleanup

Nextest may include exported setup environment values in JUnit properties even with quiet
stdout. Export password-free DATABASE_URL and credential-free S3_ENDPOINT. Use a protected
Postgres passfile (SQLx supports PGPASSFILE) and protected MinIO credential configuration
outside exported endpoint values. Never print keys, endpoint userinfo or passfile contents.
Only safe ownership IDs/readiness categories enter logs. No production endpoint is reused.

CI always removes containers using both project and exact run labels, including failure
handling. Local fixtures persist until `just test-down`; it refuses active runs and touches
only this repository's fixture labels, never developer compose/unrelated containers. A stale
label is not permission for broad Docker prune or external-volume deletion.

Future commands: `just test-tools`, `cargo nextest run --profile integration`, plain
`cargo test` after runner setup, IDE through that runner, and `just test-down`. Their actual
implementations require the named failing fixture tests first; none is created here.

## Evidence

- https://nexte.st/docs/configuration/setup-scripts/
- https://crates.io/api/v1/crates/cargo-nextest/0.9.151
- https://docs.rs/sqlx/0.9.0/sqlx/attr.test.html
- https://github.com/launchbadge/sqlx/blob/v0.9.0/sqlx-macros-core/src/test_attr.rs
- https://github.com/launchbadge/sqlx/blob/v0.9.0/sqlx-postgres/src/testing/mod.rs
- https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner
- https://nexte.st/docs/features/target-runners/
- https://doc.rust-lang.org/std/process/struct.Command.html#method.env
- https://doc.rust-lang.org/edition-guide/rust-2024/newly-unsafe-functions.html
- https://docs.rs/sqlx/0.9.0/sqlx/postgres/struct.PgConnectOptions.html
