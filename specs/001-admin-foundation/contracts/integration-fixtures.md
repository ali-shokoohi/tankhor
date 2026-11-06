# Integration Fixture Contract — Owner Decision 2026-11-04

Design only; no containers/scripts are created or started by planning. This owner-supplied
model supersedes the original per-compiled-binary wording and initial per-process primary
proposal. Trace: user planning clarification; constitution Principles I–III; FR-CON-37 and
NFR-SEC-09 database ownership/isolation validation. Tests: [test-plan.md](test-plan.md).

Owner amendment: feature 001 implements PostgreSQL fixtures only. MinIO startup/readiness
and bucket-lifecycle fixtures are deferred to feature 004; the Compose MinIO service remains.
No storage behavior or MinIO fixture coverage is claimed for 001. Automatic PostgreSQL
fallback, ownership checks and the live-run fixture lease remain required.

## 1. Shared-run services and pinned nextest

Pin **cargo-nextest 0.9.151** in CI installation and developer toolchain documentation. Rust
remains 1.98.1. Setup scripts are experimental; do not silently follow newer config syntax.
The declarative target in `.config/nextest.toml` is:

```toml
experimental = ["setup-scripts"]

[profile.fixture-bootstrap]
default-filter = 'binary(/^fixture_bootstrap_/)'

[scripts.setup.integration-services]
command = ["bash", "scripts/setup-integration.sh"]

[[profile.integration.scripts]]
filter = 'binary(/^integration_/) & (rdeps(infra-postgres) | rdeps(testkit))'
setup = "integration-services"
```

This is a design fragment, not the complete config. The profile also selects integration
binaries, sets a bounded test group and defines timeouts. Default/ci exclude both
`integration_*` and `fixture_bootstrap_*` binaries; normal integration excludes bootstrap
binaries. `fixture-bootstrap` inherits no shared setup registration; the shared hook remains
scoped only to `profile.integration`. Bootstrap lifecycle tests are named
`fixture_bootstrap_postgres` and `fixture_bootstrap_roles` and run
with `cargo nextest run --profile fixture-bootstrap -p testkit`. They start/use only their
own test-owned Testcontainers resources through the helper under test, never through
`scripts/setup-integration.sh`, global fixture endpoints or the Cargo fallback launcher.
The runner passes bootstrap binaries through directly, including after normal setup exists.

Owner-approved sequencing correction (2026-11-06; FR-024/FR-CON-37): nextest 0.9.151 validates
package and binary predicates in every profile against Cargo's current workspace target
inventory. Before storage targets exist, T009 installs profile/group/timeout scaffolding with
`fixture-bootstrap` and `integration` selecting `none()`, bounded profile concurrency, and
no binary overrides or shared hook. Default/ci select the currently present fast tests.
At T013, create the real `fixture_bootstrap_postgres.rs` driver, then install its family's
bootstrap selector, default/ci exclusion and bounded group assignment before running its red.
Integration stays inactive. Missing-target filter errors are setup blockers, not valid red;
never create placeholder targets to silence validation.

Install the applicable bootstrap selectors before testing helpers. After ownership,
PostgreSQL startup/readiness and role tests pass, observe the endpoint-export test's intended
failure, then implement the wrapper
without activating shared setup. Run the live-run lease driver and implementation next;
only after export and lease tests are green, T021 adds the genuine
`crates/testkit/tests/integration_shared_fixture.rs` target and its
`shared_fixture_exports_owned_ready_postgres` smoke check of already-tested behavior. With
that target present, install integration selectors, default/ci exclusions for both storage
families and bounded group assignment, register the shared hook and smoke-test the shared
path. The smoke test requires the inherited password-free DATABASE_URL, validates ownership
and PostgreSQL connectivity through tested helpers, and never starts services or invokes
fallback itself. This setup verification does not introduce new policy or require fabricated
red. Activation must acquire the already-tested lease before starting/reusing shared services.
The export/configuration driving test verifies disjoint
selectors and this activation order. No missing script
or recursive invocation of the helper under test is accepted as red evidence. The complete
Phase 1 checkpoint includes both bootstrap and shared-path validation, including the shared
smoke target. T008 must verify staged inventories and load/list all four profiles against
the real workspace, retaining the full final selection assertions. T032 requires active
selectors for both real target families and their bounded groups; empty inactive profiles or
probe-only selection tests are not evidence of executed fixture coverage. Thereafter
pure/mock/HTTP tests do not start services. Verify filters against actual test inventory:
no required integration test may miss setup. Enable `experimental` at the top level, not
inside a profile; use `scripts.setup`, not deprecated `script.*` definitions.
The TOML fragment describes the eventual workspace. At shared-hook activation use only
present package filters (initially testkit) and binary predicates matching genuine current
targets; expand to infra-postgres when that crate exists, never reference a missing package
or unmatched target family during helper bootstrap. Each predicate must match the current
inventory; it need not enumerate every future binary in an already-present family.

For each run, the idempotent setup script:

1. Identifies repository fixture scope from the canonical repository root (stable
   `tankhor-<root-hash>` project ID). Local scope is `local`; when CI is present, require
   nonempty, single-line GITHUB_RUN_ID, GITHUB_RUN_ATTEMPT and GITHUB_JOB and derive exact
   `ci-<run-id>-<attempt>-<job>` scope. State is under `target/fixtures/<scope>` in that root.
2. Acquires a setup lock, claims a live-run lease and starts/reuses one detached PostgreSQL
   16 container carrying exact ownership labels. Identify the live nextest owner by walking
   the setup process's `/proc` ancestry for executable basename exactly `cargo-nextest`;
   never substitute the setup child's PID or accept a prefix/lookalike name. Refuse an
   absent or unverifiable owner. T019 drives this scope/owner policy and lease claims;
   T020 implements it. T021 only wires the tested behavior into shared setup.
3. Uses dynamic loopback ports and refuses unrelated/developer/production containers.
4. Waits for PostgreSQL connectivity, not just container existence.
5. Provisions cluster-global fixture roles once idempotently; per-test migrations do not
   race to CREATE/ALTER the same global roles.
6. Appends password-free DATABASE_URL to `$NEXTEST_ENV` for matching tests. Failure ends
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

Existing admin/publication integration drivers use those isolated databases and restricted
runtime connections to commit admin revocation/deactivation/recovery with a durable monotonic
epoch advance, then commit restoration separately before worker execution. Fresh reads must
retain the advanced epoch, never reset/reuse it; rollbacks commit neither invalidation nor
epoch change. Cover deactivate→reactivate and revoke→restore cancellation, and an unchanged
epoch/all-other-gates-valid execution control in the existing prepared publication harness.
No new fixture service, endpoint or task is required.

## 3. MinIO bucket ownership — deferred to feature 004

Deferred startup case `minio_fixture_startup_is_idempotent_and_waits_for_readiness` uses
`fixture_bootstrap_minio` on test-owned resources, without shared setup, inherited endpoints
or fallback recursion. Implement owned detached startup/reuse in `fixtures/minio.rs`; neither
duplicate an existing owned container nor report ready before its readiness endpoint succeeds.
Deferred `minio_bucket_names_are_isolated_and_cleanup_covers_panics` drives `fixtures/buckets.rs`:
per-test isolation, awaited success/error/panic teardown and closing clients before returning.
These are the four removed 001 checklist entries, not deleted assertions or media delivery.

These assertions move to feature 004, not to 001's green checklist. Each test needing
MinIO gets a random unique bucket namespaced to the run. Helpers remove
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
   starting another service. Validation requires the managed test fixture identity;
   reject an unrelated developer/production DATABASE_URL before SQLx setup.
3. For PostgreSQL tests without DATABASE_URL, start scoped Testcontainers PostgreSQL,
   retain guards, then spawn the exact same binary/arguments
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
stdout. Export password-free DATABASE_URL. Use a protected Postgres passfile (SQLx supports
PGPASSFILE). Feature 004 additionally exports credential-free S3_ENDPOINT with protected
MinIO credentials outside endpoint values. Never print keys, userinfo or passfile contents.
Only safe ownership IDs/readiness categories enter logs. No production endpoint is reused.

CI always removes containers using both project and exact run labels, including failure
handling. Local fixtures persist until `just test-down`; it refuses active runs and touches
only this repository's fixture labels, never developer compose/unrelated containers. A stale
label is not permission for broad Docker prune or external-volume deletion.

Future commands: `just test-tools`, `cargo nextest run --profile integration`, plain
`cargo test` after runner setup, IDE through that runner, and `just test-down`. Their actual
implementations require the named failing fixture tests first; none is created here.

Toolchain installation, container startup and process/harness preparation are setup steps,
not behavioral red evidence. Fixture-policy helpers (ownership, lease, readiness protocol,
role isolation and cleanup) each have their own focused tests before implementation. Prepare
the executable test harness first; a missing Docker/runner/credential file is a setup blocker.

For each new or changed checked query, migrate the isolated preparation database, prepare
metadata online with `SQLX_OFFLINE=false`, then rerun the driving/affected/integration suites
offline with `SQLX_OFFLINE=true` before marking the implementation pair green. Refresh and
commit the generated `.sqlx/query-*.json` files in that implementation increment. Never use
online preparation as a substitute for runtime-role privilege tests; final full-workspace
freshness validation remains mandatory.

This is each retained adapter's completion criterion: **sqlx metadata prepared and offline
check passes before green**, not a separate later metadata task.

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
