# Tasks: Tankhor Admin Foundation (001)

**Revision**: Owner-approved in-place consolidation (I1/U1 and architecture/duplicate decisions); 580 → 491 unchecked tasks, 19 → 16 ports. All 47 acceptance names and observable scope preserved; only MinIO fixture implementation is deferred to 004.

**Input**: `specs/001-admin-foundation/{spec.md,plan.md,research.md,data-model.md,contracts/,quickstart.md}`.
**Authority**: `.specify/memory/constitution.md` v1.2.0; only the authoritative documents in
`docs/README.md` decide requirements. `docs/fa/` is never a decision source.
**Tests**: MANDATORY. All boxes are initially unchecked; generation is not implementation evidence.
**Organization**: shared setup/foundation, then US1–US6 in specification priority order, then gates.

## Format and execution contract

- Format: `- [ ] Tnnn [P?] [USn?] Write failing test: [layer] ...` or
  `- [ ] Tnnn [P?] [USn?] Make pass: Tnnn ...`. `Setup:` prepares tooling/harness/metadata;
  `Regression test:` and `Acceptance test:` verify delivered behavior without requiring
  fabricated red or a production change. Checkpoint tasks record actual validation evidence.
- Each implementation is limited to its named behaviour and driving test IDs; target 1–2 hours.
  If that budget is exceeded, split it into additional red/green pairs before implementation,
  preserving traceability. Never combine independent behaviours into an untested implementation.
- Registration notation identifies these exact additional touched files: "domain index" =
  `crates/admin/src/domain/mod.rs`; "port index" = `crates/admin/src/ports/mod.rs`;
  "application index" = `crates/admin/src/application/mod.rs`; "adapter index" =
  `crates/infra-postgres/src/lib.rs`; "testkit indexes" =
  `crates/testkit/src/fakes/mod.rs` and `crates/testkit/src/contracts/mod.rs`;
  "shared store" = `crates/testkit/src/fakes/store.rs`. Other crate module indexes are that
  task's explicitly named `src/lib.rs`. "Both console Fluent files" always means
  `crates/web/locales/fa/console.ftl` and `crates/web/locales/en/console.ftl`.
  Workspace member/dependency additions also touch `Cargo.toml` and generated `Cargo.lock`;
  do not use that registration step to introduce unrelated files or behaviour.
- Run the named test and record the intended red result before its implementation. Missing
  intended types/traits may initially produce a compilation failure; missing tools, unrelated
  syntax errors, inaccessible vendors and broken fixtures are NOT valid red evidence.
- Before Cargo exists, run `python3 -m unittest discover -s scripts/tests`. Thereafter, run
  `cargo nextest run -p <affected-crate>` after every code change. Adapter/SQL changes also run
  their integration profile; persistence changes always run
  `cargo nextest run --profile integration -p infra-postgres`. Complete each red/green pair
  before starting another; do not mark red tests complete as delivered functionality.
  Fake/service runs additionally enable the explicit test features: use
  `cargo nextest run -p testkit --features fakes` with kernel/admin mocks enabled by testkit's
  dev-dependencies; workspace CI enables `testkit/fakes,admin/mocks,kernel/mocks` explicitly.
  A default-feature run that omits required feature-gated test targets is not sufficient.
  Keep these features off normal/release graphs; never enable them as production defaults.
- `[unit]` includes proptest; `[mock]`, `[fake]`, `[contract]`, `[integration]`, `[http]`, `[e2e]`
  identify the other layers. `[P]` marks only explicitly listed, disjoint test files ready
  after their common prerequisite. Shared manifests/module indexes are edited serially.
- Every new port uses an async trait in `crates/<context>/src/ports/<port>.rs`, with
  `#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]` ABOVE
  `#[async_trait::async_trait]`. Contexts never import SQLx, axum, aws-*, reqwest or vendor types.
- Required per port: trait/automock, shared generic contract, feature-gated fake, real adapter,
  service interaction tests. This is an inventory, not a temporal ordering. Contract-on-fake
  must be green before its real adapter is written; mock AND fake service tests precede services.
  Each contract covers success, typed errors, retry/idempotency and ordering where meaningful;
  clock reads/random outputs are not incorrectly required to be identical or monotonic.
- Application tests live in testkit to avoid admin→testkit→admin dependency cycles. Testkit
  imports kernel/admin, never infra-*; real-adapter runners live in their adapter crates.
  All fakes share a single transactional store. They are behind testkit feature `fakes`;
  context feature `mocks` enables automock for downstream tests. Neither enters a release build.
- Each PostgreSQL runner calls the SAME generic contract via
  `#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]`, on a fresh database with a restricted
  runtime pool. Fixture/migration credentials are not runtime credentials. HTTP vendors use
  wiremock only; local crypto/system adapters use ordinary async tests, not SQLx artificially.
- `UnitOfWork` includes begin and consuming commit/rollback; `TxHandle` is an opaque carrier,
  not a second independent external-dependency port. Never expose a SQL transaction to contexts.
- Naming reconciliations: `AdminCredentialRepository`, `SessionStore` and `UnitOfWork` are the canonical names
  in both plan and contracts. There are no duplicate legacy-port aliases to implement.
  Kernel ports are in `kernel/src/ports/`; real Clock/IdGenerator adapters are in
  `infra-system`, not app. ImpactAssessment/CapabilityReadiness are direct pure rules in
  admin/domain, not ports; the code-owned registry marks all six capabilities not delivered.
  `HealthProbe` makes the plan's storage/seed-readiness dependency explicit. It fails closed
  before the registry exists. App only wires these adapters; it implements none of them.
- Foundation creates only genuinely needed crate entry points; no empty future contexts,
  placeholder successful handlers, arbitrary-proof authenticators or production testkit imports.
  `dev-fakes` is an explicit debug-only app feature. Fake mode remains forbidden in production.
- Configuration uses TANKHOR_ENV=development|staging|production and one application-wide
  TANKHOR_ADAPTERS=real|fake (default real), never per-group overrides/mixing. Repositories,
  UoW, HealthProbe, time, IDs, crypto and both SMS slots use the selected coherent backend.
  Unknown values/legacy overrides fail with sanitized key-specific errors. Production
  rejects fake mode before acquisition; releases exclude mocks/fakes features.
- Exact named AC tests from `spec.md` are retained, including AC-ADM-046/047, now synchronized in
  the contract test plan. All final tables refer to concrete tasks below, never to future empty tests.
- No new numeric mutation threshold: ≥90% line coverage applies to present kernel/wallet/
  generation/identity. Other-crate 80% and mutation 85% remain unapproved proposals.

- Tool/harness installation, starting fixture services and SQLx metadata preparation are
  `Setup:` tasks, never red evidence. Run prepared harnesses before any behavioral assertion.
  Missing tools/fixtures block execution. Fixture-policy helper code itself is test-driven.
- Each retained checked-query adapter includes metadata preparation/offline validation
  in its own completion criterion before green, not a standalone later task. Online `SQLX_OFFLINE=false` preparation uses the current migrated isolated
  schema only; ordinary validation returns to `SQLX_OFFLINE=true`. Final workspace freshness
  is additional assurance, not deferred first preparation.
- Run behavior tests unchanged. If a proposed driving assertion is already green because an
  earlier increment implements the guarantee, record that evidence and make no production
  change; never break code or weaken tests to manufacture red. New production code still
  requires an observed intended behavioral failure. Acceptance/regression tasks need no
  fabricated implementation partner.

- Fixture lifecycle tests use `cargo nextest run --profile fixture-bootstrap -p testkit`;
  fixture_bootstrap_* binaries invoke their test-owned helpers directly, with no shared
  setup/fallback launcher. Normal shared integration is enabled only after helper,
  export and live-run lease tests pass. Phase 1 and CI validate bootstrap before the shared path; this changes
  harness ordering only, never what counts as an intended behavioral red.

### Owner-approved nextest sequencing correction — 2026-11-06

Nextest 0.9.151 validates package and binary predicates against Cargo's current workspace
target inventory, including profiles other than the selected one. A predicate matching no
target prevents configuration loading; it is a setup failure, never behavioral red evidence.
T009 therefore installs the profile/group/timeout scaffolding with storage profiles selecting
`none()` until their genuine targets exist. Default/ci select all currently present fast
tests until exclusions become applicable. T013 adds bootstrap selectors and exclusions in
the same task as the first real bootstrap driver, before its red run. T021 adds integration
selectors, exclusions and shared setup alongside the genuine shared-fixture smoke target.
These configuration steps are harness preparation, not fixture implementation. Never create
placeholder targets, use test-name filters as substitutes for binary isolation, or count an
empty storage profile as executed coverage. No task IDs, product requirements, tool pins or
Phase 2 responsibilities change; all final selectors and real-run evidence remain mandatory
at T032. The existing T008 red evidence is retained; its test must additionally verify staged
configuration and real-workspace loadability before T009 is marked complete.

## Phase 1: Setup — Workspace, fixtures and CI

**Purpose**: Make the test-first toolchain executable without implementing console behaviour.
**Independent test**: repository-check negatives reject missing lints and forbidden edges;
integration setup provisions only owned isolated services. No application acceptance is claimed.

- [X] T001 Setup: verify/install Python 3, Rust 1.98.1 plus rustfmt/clippy/llvm-tools, nextest 0.9.151, cargo-deny, llvm-cov, mutants, SQLx CLI 0.9.0, Docker/Compose and Node in `docs/development.md`; pin selected compatible tool versions and verify each executable before any task invokes it. Playwright/axe package setup remains separately scheduled before E2E. Missing tooling is a setup blocker, never red evidence; create no application behavior (FR-001, FR-024, FR-CON-01, FR-CON-37).
- [X] T002 Write failing test: [unit] `workspace_skeleton_requires_inherited_lints` in `scripts/tests/test_workspace.py`; expect absent workspace/toolchain/lint inheritance to fail (FR-001, FR-CON-01; constitution I–II).
- [X] T003 Make pass: T002 create `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates/kernel/Cargo.toml`, `crates/kernel/src/lib.rs`; pin Rust 1.98.1/edition 2024, inherit unsafe_code=forbid and pedantic=warn, expose no invented runtime behaviour (FR-001, FR-CON-01).
- [X] T004 Write failing test: [unit] `architecture_policy_rejects_forbidden_context_edges` in `scripts/tests/test_architecture.py`; inject SQLx under alias/workspace/optional/target/dev/transitive edges and a web→repository edge; expect missing checker acceptance (FR-001, FR-CON-01; constitution II).
- [X] T005 Make pass: T004 implement metadata/manifest/import checks in `scripts/check-architecture.py`; reject I/O in contexts/kernel, adapter→adapter calls, web/worker→repositories and direct domain/application clock/ID calls (FR-001, FR-CON-01).
- [X] T006 Write failing test: [integration] `cargo_deny_rejects_sqlx_context_dependency` in `scripts/tests/test_deny.py`; construct disposable manifests under the test harness, run cargo-deny and require a nonzero result for admin→sqlx but allow infra-postgres→sqlx; expect missing bans (FR-001, FR-CON-01).
- [X] T007 Make pass: T006 configure wrapper-aware bans/licenses/advisories in `deny.toml`; retain T005 metadata checking for indirect paths cargo-deny cannot express (FR-001, FR-CON-01).
- [X] T008 Write failing test: [unit] `nextest_profiles_select_storage_tests_only` in `scripts/tests/test_nextest.py`; expect missing profile/group/timeout scaffolding, absent applicable default/ci exclusions, overlapping fixture-bootstrap/integration selectors, shared setup on bootstrap, or unbounded concurrency to fail. Retain the observed missing-config red and all final selection assertions. Before T009 implementation, extend the driver with staged genuine-target inventories (fast-only, bootstrap-present, both families present), refusal of premature binary/package predicates, and `cargo nextest list` validation for all four profiles in the real workspace; observe the current real-workspace configuration failure. An inactive storage profile must select `none()`, not fast tests. A completed T013 or T021 must not be accepted with its required selectors still inactive; the complete probe inventory must prove the final disjoint binary selection at T032 (FR-024, FR-CON-37).
- [X] T009 Make pass: T008 create `.config/nextest.toml` profile/group/timeout scaffolding with top-level experimental setup scripts enabled, dedicated fixture-bootstrap/integration profiles initially selecting `none()`, default/ci selecting present fast tests, bounded profile concurrency and named groups ready for later assignment. Use only predicates matching present packages/targets; do not add binary-based overrides before their real targets exist. Re-run the strengthened T008 driver and list all four profiles in the real workspace; empty inactive storage selections prove configuration loadability only, not fixture coverage. T013 installs bootstrap selectors/exclusions/group assignment before its first red; T021 installs integration selectors/exclusions/group assignment and activates shared setup only after bootstrap/export/lease tests pass. Pin nextest 0.9.151 in `docs/development.md` (FR-024, FR-CON-37).
- [X] T010 Setup: prepare the test-only fixture crate/tooling in `crates/testkit/Cargo.toml`, `crates/testkit/src/lib.rs`, `crates/testkit/src/bin/fixture-runner.rs`, `Cargo.toml` and `Cargo.lock`; install/pin required tools and prepare isolated credentials. No fixture-policy logic or successful application stand-in is added; missing tools/containers are setup blockers (FR-024, FR-CON-37). T001 installs SQLx CLI 0.9.0; configure isolated fixture-owned migration/runtime and preparation credentials here, without repeating installation.
- [X] T011 Write failing test: [unit] `fixture_ownership_rejects_foreign_resources` in `crates/testkit/tests/fixture_ownership.rs`; expect foreign/production labels to be accepted rather than a typed ownership refusal (FR-024, FR-CON-37).
- [X] T012 Make pass: T011 implement only exact ownership-label and nonproduction-endpoint validation in `crates/testkit/src/fixtures/ownership.rs`; edit only this behavior and its module registration (FR-024, FR-CON-37).
- [X] T013 Write failing test: [integration] `postgres_fixture_startup_is_idempotent_and_waits_for_readiness` in `crates/testkit/tests/fixture_bootstrap_postgres.rs`; expect the ready owned PostgreSQL fixture to be duplicated on reuse or returned before readiness (FR-024, FR-CON-37). With this genuine driver file present, first prepare `.config/nextest.toml`: select only fixture_bootstrap_* binaries in fixture-bootstrap, exclude them from default/ci, and assign their bounded group; leave integration selecting none() with no hook. Re-run T008 and list all profiles to verify loadability and bootstrap isolation before the behavioral red. Run with the fixture-bootstrap profile on test-owned resources; never invoke shared setup, inherited endpoints or the fallback launcher. Configuration/load/list failures are setup blockers, not the intended red; no fixture-policy production code is introduced here.
- [X] T014 Make pass: T013 implement owned detached PostgreSQL 16 startup/reuse and readiness, using the prepared Testcontainers harness in `crates/testkit/src/fixtures/postgres.rs`; edit only this behavior and its module registration (FR-024, FR-CON-37).
- [X] T015 Write failing test: [integration] `fixture_roles_are_idempotent_and_runtime_is_not_owner` in `crates/testkit/tests/fixture_bootstrap_roles.rs`; expect repeated role setup to grant runtime ownership or race into duplicate cluster roles (FR-024, FR-CON-37). Run with the fixture-bootstrap profile on test-owned resources; never invoke shared setup, inherited endpoints or the fallback launcher.
- [X] T016 Make pass: T015 implement idempotent cluster migration/runtime role provisioning only in `crates/testkit/src/fixtures/roles.rs`; edit only this behavior and its module registration (FR-024, FR-CON-37).
- [X] T017 Write failing test: [unit] `setup_exports_only_matching_tests_ready_endpoints` in `scripts/tests/test_setup_integration.py`; expect passwords/unready or foreign endpoints to be exported into NEXTEST_ENV; also reject shared setup on fixture-bootstrap, selectors that overlap, or shared-hook activation before ownership/startup/roles/export tests pass (FR-024, FR-CON-37). Assert wrapper implementation follows green bootstrap tests plus its own observed intended red; activation/smoke follows its green result, with no recursive bootstrap setup. Activation must also wait for the live-run lease driver/implementation T019/T020 to pass; test premature activation refusal without using shared setup for bootstrap.
- [X] T018 Make pass: T017 after T011/T013/T015 bootstrap tests pass and T017's intended endpoint-export failure is observed, implement only the thin ready-endpoint wrapper in `scripts/setup-integration.sh` using tested helpers through `crates/testkit/src/bin/fixture-runner.rs`. Do not activate the shared hook yet and do not require T017 to be green before implementing it (FR-024, FR-CON-37).
- [X] T019 Write failing test: [integration] `local_shared_server_rejects_concurrent_independent_runs`, `shared_scope_derives_local_repository_scope`, `shared_scope_derives_exact_ci_job_run_scope`, `nextest_owner_identifies_live_ancestor`, `nextest_owner_rejects_mismatched_ancestor` and `nextest_owner_rejects_absent_ancestor` in `scripts/tests/test_fixture_lease.py`; cover repository-local versus exact CI run/attempt/job scope derivation and live nextest-owner identification/refusal (contract §1 steps 1–2), as well as a second run reusing SQLx deterministic database names unsafely or losing ownership metadata. Require per-behavior mutation → red → revert → green evidence for the existing scope/owner implementation (FR-024, FR-CON-37).
- [X] T020 Make pass: T019 implement repository/CI scope derivation and nextest-owner identification in `crates/testkit/src/bin/fixture-runner.rs`, setup lock/live owner lease/stale-owner checks in `crates/testkit/src/fixtures/lease.rs` and wire `crates/testkit/src/fixtures/services.rs`; retain the owned PostgreSQL service between runs; MinIO fixture startup is deferred to 004 (FR-024, FR-CON-37).
- [X] T021 Setup (wiring only): only after T017 and the scope/owner/live-run lease driver/implementation T019/T020 are green and bootstrap validation passes, assemble tested fixture modules in `crates/testkit/src/fixtures/services.rs` and `crates/testkit/src/lib.rs`. Add `shared_fixture_exports_owned_ready_postgres` in `crates/testkit/tests/integration_shared_fixture.rs` as a genuine smoke check of the already-tested ownership/readiness/export/role behavior through shared setup; require the inherited password-free endpoint and verify the owned PostgreSQL connection, without bootstrap startup or fallback in the test body. With this real target present, update `.config/nextest.toml` to select only integration_* binaries in integration, exclude both storage families from default/ci, assign the bounded integration group, and activate shared setup using present-package filters (testkit initially). Re-run T008, list all four profiles against the real workspace, then run the bootstrap suite followed by the shared smoke test before normal integration. Scope derivation, owner identification and lease policy are driven by T019/T020; no new policy logic or fabricated red belongs here. MinIO fixtures move to 004 (FR-024, FR-CON-37). Acquire the already-tested live-run lease before shared startup/readiness and retain the disjoint bootstrap selectors.
- [X] T022 Write failing test: [unit] `fallback_runner_sets_only_child_environment` in `scripts/tests/test_test_runner.py`; expect missing runner, discovery-triggered Docker, bootstrap binaries not passed through directly, lost child status or global environment mutation (FR-024, FR-CON-37).
- [X] T023 Make pass: T022 create `scripts/test-runner.sh`, `.cargo/config.toml`, `justfile`, `crates/testkit/src/fixtures/process.rs`; launch the same test binary with child-only endpoints, retain guards, pass through fixture_bootstrap_* binaries directly without setup and non-storage/list/run commands and validate supplied fixture ownership (FR-024, FR-CON-37).
- [X] T024 Write failing test: [unit] `ci_cleanup_matches_only_current_run_labels` in `scripts/tests/test_test_down.py`; expect missing cleanup or acceptance of an active/foreign run (FR-024, FR-CON-37).
- [X] T025 Make pass: T024 create `scripts/test-down.sh` and update `justfile` for explicit local cleanup and always-run exact-label CI cleanup; no prune or broad database sweep (FR-024, FR-CON-37).
- [X] T026 Write failing test: [unit] `compose_keeps_migration_and_runtime_roles_separate` in `scripts/tests/test_compose.py`; expect missing healthy PostgreSQL 16/MinIO services, protected credential references or separation from test fixture ownership (FR-024, FR-CON-37).
- [X] T027 Make pass: T026 create `deploy/compose.yml` with health checks and pinned service images; no privileged application role or baked-in secrets (FR-024, FR-CON-37).
- [X] T028 Write failing test: [unit] `ci_requires_fast_and_adapter_integration_gates` in `scripts/tests/test_ci.py`; expect missing fmt/clippy/deny/architecture/nextest gates, offline SQL settings, a separate fixture-bootstrap gate before shared integration, and always-run fixture teardown (FR-024, FR-CON-37).
- [X] T029 Make pass: T028 create `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`; pin tooling/actions, run fast tests on PRs and bootstrap then shared integration on infra-* changes/nightly, and report absent future crates rather than green skipped claims (FR-024, FR-CON-37).
- [X] T030 Write failing test: [unit] `traceability_checker_rejects_implementation_without_driving_test` in `scripts/tests/test_traceability.py` and `every_port_has_fake_real_contract_and_service_mock_evidence` in `scripts/tests/test_port_inventory.py`; test partial and final modes. Partial validates planned AC/FR/port references, preceding drivers and planned-artifact/completion consistency at the completed-work boundary only; no actual-symbol port inventory is required in partial mode; unfinished feature symbols may be absent. Reject completed work with missing required planned artifacts or preceding-driver links. Final rejects a missing actual trait/fake/real/shared runner/direct mock interaction or feature graph in any of sixteen rows, including either SmsSender adapter; accept a complete final fixture. Test missing/misordered IDs, missing ACs, unpaired ports, zero-call query links misused as ID/audit evidence and filename/mock-label-only evidence (FR-001, FR-038, FR-CON-37; constitution III).
- [X] T031 Make pass: T030 implement `scripts/check-test-traceability.py`, `.github/workflows/ci.yml`; provide --mode partial for planned-artifact/driver/completion links at the completed-work boundary only and --mode final for the full actual-symbol/runner/direct-interaction inventory and both real/fake feature graphs. Phase 1 must not require future ports to exist; final completeness is mandatory at T491. Use the tested completed-task boundary, never treat absent unfinished features as passing runtime tests (FR-001, FR-038, FR-CON-37).
- [X] T032 Verify checkpoint: run repository checks, `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo deny check`, `cargo nextest run --profile fixture-bootstrap -p testkit`, `cargo nextest run`, and the now-enabled shared `cargo nextest run --profile integration`; all green. Require T008's complete final selection assertions and real-workspace listings for all four profiles: bootstrap/integration selectors and bounded group assignments must be active, both families excluded from default/ci, and shared setup scoped only to integration. Run `shared_fixture_exports_owned_ready_postgres` through shared setup; empty selections or probe-only success do not satisfy bootstrap/shared coverage. Confirm only contract-permitted reusable owned services remain. Record only observed results in `specs/001-admin-foundation/tasks.md` (FR-CON-01, FR-CON-37). Run traceability in partial mode: validate planned-artifact/driver/completion links for work completed so far only; do not require later port symbols/runners. The full actual inventory belongs to T491. Stop and ask the owner before any commit; completion of this task does not authorize a commit or Phase 2 work.

**T032 observed checkpoint evidence (2026-11-06):** fmt and clippy passed; Python repository
suite 13 passed; fixture-bootstrap 2 passed, default 1 passed, shared integration 1 passed
with `integration-services` SETUP PASS; cargo-deny advisories ok with 1 owner-approved ignore (RUSTSEC-2026-0440); bans/licenses/sources ok
(duplicate/unused-allowance warnings remain non-failing). Architecture and partial
traceability passed. Real-workspace listings verified default/ci select fixture_ownership,
bootstrap selects fixture_bootstrap_postgres/fixture_bootstrap_roles, and integration
selects integration_shared_fixture. `docker ps -a --filter label=io.tankhor.fixture=true`
found only reusable local PostgreSQL bc7dea1fe78b (project tankhor-34921e0ba9d2704e, run local).
Live isolated Compose verification reached healthy PostgreSQL/MinIO and confirmed restricted
distinct database roles; its temporary stack/volumes were removed. Hosted GitHub workflows
were syntax/policy-checked locally, not executed remotely. No Phase 2 work or commit performed.

**T008/T019–T021 remediation evidence (2026-11-06):** T019/T020/T021 were reopened
before extending the driver and re-ticked only after the following observed evidence.
The Python driver compiles `shared_scope`, `nextest_owner` and `environment` verbatim
from fixture-runner into a temporary std-only probe, exercising child-only environment
and live `/proc` ancestry without shared startup. A compiled launcher named exactly
`cargo-nextest` supplies a known live ancestor PID; `cargo-nextest-impostor` and direct
launch exercise mismatch/absence refusal. Each row below was run separately; every
production mutation was reverted immediately, and fixture-runner has no remaining diff.

| Task · test | Deliberate mutation / wrong expectation | Observed red assertion | Revert → green |
| --- | --- | --- | --- |
| T008 · `test_real_workspace_profiles_select_exact_fixture_binaries` | Add integration_shared_fixture to default's expected set | Items in the second set but not the first: integration_shared_fixture; FAILED (failures=1) | Restore exact set; full test_nextest module: 2 tests OK |
| T019 · `test_shared_scope_derives_local_repository_scope` | local → local-wrong | 'local-wrong' != 'local'; FAILED (failures=1) | Reverted; 1 test OK |
| T019 · `test_shared_scope_derives_exact_ci_job_run_scope` | GITHUB_JOB lookup → wrong-job | 'ci-701-1-wrong-job' != 'ci-701-1-integration'; missing/invalid job accepted; FAILED (failures=7) | Reverted; 1 test OK |
| T019 · `test_nextest_owner_identifies_live_ancestor` | Return setup process PID instead of discovered nextest PID | '1014252' != '1014251'; lease owner must be nextest, not its setup child; FAILED (failures=1) | Reverted; 1 test OK |
| T019 · `test_nextest_owner_rejects_mismatched_ancestor` | Exact name equality → starts_with(cargo-nextest) | 0 == 0: a nextest-like ancestor is not the owner; FAILED (failures=1) | Reverted; 1 test OK |
| T019 · `test_nextest_owner_rejects_absent_ancestor` | At ancestry exhaustion return setup PID instead of Err | 0 == 0: setup without nextest must refuse an owner; FAILED (failures=1) | Reverted; full test_fixture_lease module: 6 tests OK |

T020: existing implementation passes the strengthened driver; `cargo nextest run -p testkit`
passed (1 test). T021: bootstrap passed (2 tests), then the shared smoke passed (1 test)
with integration-services SETUP PASS. Exact real-workspace listings include test binaries:
default/ci → fixture_ownership; fixture-bootstrap → fixture_bootstrap_postgres and
fixture_bootstrap_roles; integration → integration_shared_fixture.

**Remediation checkpoint (2026-11-06):** `cargo fmt --check` passed;
`cargo clippy --all-targets --all-features -- -D warnings` passed;
`cargo nextest run` passed (1/1); `cargo nextest run --profile integration` passed
(1/1, integration-services SETUP PASS); `cargo nextest run --profile fixture-bootstrap
-p testkit` passed (2/2); `cargo deny check` passed: advisories ok with 1 owner-approved
ignore (RUSTSEC-2026-0440), bans/licenses/sources ok, non-failing duplicate/unused-wrapper/
unused-licence warnings retained. `python3 -m unittest discover -s scripts/tests` passed
(19/19); `python3 scripts/check-test-traceability.py --mode partial` passed;
`python3 scripts/check-architecture.py` passed; `git diff --check` passed.
Dependency exceptions are recorded in plan CI gates and research R13; all eight requested
later checkpoint tasks and the execution-order note now require evidence and stopping,
with the owner committing. No outstanding remediation blocker or retained mutation;
Phase 2 was not started and no commit was made. Next scheduled task is T033, owner-gated.

## Phase 2: Foundational — Kernel ports, safe composition and web/API base

**Purpose**: Shared executable infrastructure blocks all story work; only shell/status routes
are delivered here. No placeholder protected management route or completed sign-in is claimed.
**Independent test**: kernel contracts pass on fakes and local/transactional real adapters;
configuration forbids production fakes; bilingual shell and minimal documented JSON are tested.
HealthProbe's real seed-ready branch completes with actual registry storage in US2, not a stub.
The foundational fake server smoke must serve shell/health with outbound networking denied;
unseeded readiness is unavailable and no placeholder management routes are installed.

### Kernel values and contract harness

- [X] T033 [P] Write failing test: [unit] `uuid_v7_ids_reject_wrong_version` in `crates/kernel/tests/ids.rs`; expect missing wrappers for AdminId, CredentialId, SessionId, ChallengeId, ChangeId, RevisionId, PreviewId, ApprovalId, PublicationId, VersionId and AuditId, and acceptance of non-v7 values (FR-004, FR-012, FR-CON-37).
- [X] T034 [P] Write failing test: [unit] `locale_direction_is_derived` in `crates/kernel/tests/presentation.rs`; expect missing fa/en locale, RTL/LTR derivation and light/dark/system values (FR-025, FR-027, FR-UX-01).
- [X] T035 Make pass: T033 implement checked UUIDv7 wrappers in `crates/kernel/src/ids.rs`, register `crates/kernel/src/lib.rs`; browser capability tokens are not UUID IDs (FR-004, FR-012, FR-CON-37).
- [X] T036 Make pass: T034 implement locale/direction/theme values in `crates/kernel/src/locale.rs`, register `crates/kernel/src/lib.rs`; no independently editable direction (FR-025, FR-027, FR-UX-01).
- [X] T037 Write failing test: [unit] `domain_errors_do_not_expose_infrastructure_secrets` in `crates/kernel/tests/errors.rs`; expect missing validation/auth/conflict/unavailable/invalid-handle/retryable/unknown-commit taxonomy and unsafe Debug payloads (FR-024, FR-CON-37).
- [X] T038 Make pass: T037 implement typed sanitized errors in `crates/kernel/src/error.rs`, register `crates/kernel/src/lib.rs`; web later localizes them (FR-024, FR-CON-37).
- [X] T039 Write failing test: [unit] `testkit_features_do_not_create_context_cycles` in `scripts/tests/test_testkit_features.py`; expect missing fakes/contracts feature gates or context→testkit dependency (FR-024, FR-CON-37; constitution III).
- [X] T040 Make pass: T039 extend `crates/testkit/Cargo.toml`, `crates/testkit/src/lib.rs`, `crates/testkit/src/contracts/mod.rs`, `crates/testkit/src/fakes/mod.rs`; enable fakes only explicitly, gate fake integration targets with required-features, keep generic suites infra-independent and enable kernel/admin mocks through testkit dev-dependencies as contexts arrive; CI must run those targets with fakes enabled (FR-024, FR-CON-37).

### Clock, IdGenerator and UnitOfWork

- [X] T041 Write failing test: [unit] `clock_port_is_async_and_automockable` in `crates/kernel/tests/clock_port.rs`; expect missing async Clock trait/MockClock or wrong macro order (FR-013, FR-CON-32, P41).
- [X] T042 Make pass: T041 define Clock::now in `crates/kernel/src/ports/clock.rs`, `crates/kernel/src/ports/mod.rs`, `crates/kernel/src/lib.rs`, `crates/kernel/Cargo.toml` with the required macro order and mocks feature (FR-013, FR-CON-32, P41).
- [X] T043 Write failing test: [contract] `clock_contract` in `crates/testkit/src/contracts/clock.rs` and its runner `crates/testkit/tests/clock_fake.rs`; expect absent FixedClock; assert UTC/error taxonomy and fixture-controlled boundaries without assuming wall-clock monotonicity (FR-013, FR-CON-32, P41).
- [X] T044 Make pass: T043 implement explicit advance/set FixedClock in `crates/testkit/src/fakes/clock.rs`, register `crates/testkit/src/fakes/mod.rs` and `crates/testkit/src/contracts/mod.rs`; contract green on fake (FR-013, FR-CON-32, P41).
- [X] T045 Write failing test: [unit] `id_generator_port_is_async_and_automockable` in `crates/kernel/tests/id_generator_port.rs`; expect missing typed async generator/MockIdGenerator (FR-012, FR-CON-37).
- [X] T046 Make pass: T045 define IdGenerator in `crates/kernel/src/ports/id_generator.rs`, register `crates/kernel/src/ports/mod.rs`; no uuid generation in contexts (FR-012, FR-CON-37).
- [X] T047 Write failing test: [contract] `id_generator_contract` in `crates/testkit/src/contracts/id_generator.rs` and `crates/testkit/tests/id_generator_fake.rs`; expect absent deterministic sequence/exhaustion errors; assert UUIDv7 and uniqueness, not identical real random values (FR-012, FR-CON-37).
- [X] T048 Make pass: T047 implement deterministic IdGenerator in `crates/testkit/src/fakes/id_generator.rs`, register testkit module indexes; pass the contract without system time/randomness (FR-012, FR-CON-37).
- [X] T049 Write failing test: [unit] `uow_port_has_consuming_transaction_lifecycle` in `crates/kernel/tests/unit_of_work_port.rs`; expect missing async begin/commit/rollback, opaque handle/store identity and mocks (FR-021, FR-024, FR-CON-37).
- [X] T050 Make pass: T049 define UnitOfWork and infrastructure-neutral TxHandle in `crates/kernel/src/ports/unit_of_work.rs`, register `crates/kernel/src/ports/mod.rs`; finished/wrong handles return typed errors, not panics (FR-021, FR-024, FR-CON-37).
- [X] T051 Write failing test: [contract] `unit_of_work_contract` in `crates/testkit/src/contracts/unit_of_work.rs` and `crates/testkit/tests/unit_of_work_fake.rs`; expect absent commit/rollback/drop isolation, wrong-store refusal and unknown-outcome handling (FR-021, FR-024, FR-CON-37).
- [X] T052 Make pass: T051 implement shared serialized transactional FakeStore/UnitOfWork in `crates/testkit/src/fakes/store.rs`, `crates/testkit/src/fakes/unit_of_work.rs` and testkit module indexes; rollback/drop discards working state across repositories (FR-021, FR-024, FR-CON-37).
- [X] T053 Write failing test: [mock] [fake] `health_transaction_reads_once_and_rolls_back_on_failure` in `crates/testkit/tests/application_health_uow.rs`; expect missing read-only application entry point, duplicate begin, write from a read-only service or leaked handle (FR-001, FR-CON-01).
- [X] T054 Make pass: T053 add minimal admin crate and read-UoW application helper in `crates/admin/Cargo.toml`, `crates/admin/src/lib.rs`, `crates/admin/src/application/mod.rs`, `crates/admin/src/application/read_transaction.rs`, plus workspace/testkit dependency wiring; no SQL/clock generation in the service (FR-001, FR-CON-01).
- [X] T055 Write failing test: [contract] [integration] `clock_contract_real_system` in `crates/infra-system/tests/clock.rs`; expect the real clock adapter to be absent after its fake suite is green; run the SAME generic suite from T043 (FR-012, FR-013, FR-CON-37).
- [X] T056 Make pass: T055 implement only this real local adapter; add infra-system manifest/workspace/module registration on first use in `crates/infra-system/src/clock.rs`; edit only this behavior and its module registration (FR-012, FR-013, FR-CON-37).
- [X] T057 Write failing test: [contract] [integration] `id_generator_contract_real_system` in `crates/infra-system/tests/id_generator.rs`; expect the real id_generator adapter to be absent after its fake suite is green; run the SAME generic suite from T047 (FR-012, FR-013, FR-CON-37).
- [X] T058 Make pass: T057 implement only this real local adapter; add infra-system manifest/workspace/module registration on first use in `crates/infra-system/src/id_generator.rs`; edit only this behavior and its module registration (FR-012, FR-013, FR-CON-37).
- [X] T059 Write failing test: [contract] [integration] `postgres_uow_passes_shared_contract` in `crates/infra-postgres/tests/integration_unit_of_work.rs`; expect absent real transaction owner; use a test-owned scratch fixture to prove commit/rollback/drop/independent connections without inventing production tables (FR-021, FR-024, FR-CON-37).
- [X] T060 Make pass: T059 add `crates/infra-postgres/Cargo.toml`, `crates/infra-postgres/src/lib.rs`, `crates/infra-postgres/src/unit_of_work.rs`, `crates/infra-postgres/migrations/.gitkeep`, workspace membership and canonical MIGRATOR; lifecycle and runtime pool checks stay inside adapter (FR-021, FR-024, FR-CON-37). Now that infra-postgres exists, expand the shared integration setup selector in `.config/nextest.toml` to its SQLx binaries, retaining bootstrap exclusion and the existing profile tests. Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.
- [X] T061 Write failing test: [integration] `sqlx_cases_get_distinct_migrated_databases` in `crates/infra-postgres/tests/integration_fixture_isolation.rs`; expect absent database/role ownership and pool cleanup verification, including retained failed database until same-case rerun (FR-024, FR-CON-37).
- [X] T062 Make pass: T061 implement caller-supplied pool/ownership helpers in `crates/testkit/src/fixtures/database.rs`, register `crates/testkit/src/lib.rs`; no infra-postgres imports or production migration copies (FR-024, FR-CON-37).

### Configuration and dev/release safety

- [X] T063 Write failing test: [unit] `production_rejects_fake_application_mode` in `crates/app/tests/adapter_config.rs`; expect invalid/unknown modes, per-group overrides or production fake selection to be accepted. One TANKHOR_ADAPTERS=real|fake mode must select a coherent application backend before acquisition. Also run `transport_policy_requires_finite_positive_limits`: reject missing/zero/negative/unbounded body-byte/in-flight limits for serve; offline openapi remains independent (FR-001, FR-CON-01; constitution III).
- [X] T064 Make pass: T063 add `crates/app/Cargo.toml`, `crates/app/src/lib.rs`, `crates/app/src/config.rs` and workspace entry; parse one application-wide TANKHOR_ADAPTERS=real|fake (default real), reject per-group overrides/unknown modes and production fakes before connecting. Validate TANKHOR_HTTP_MAX_BODY_BYTES and TANKHOR_HTTP_MAX_INFLIGHT in `crates/app/src/transport_policy.rs` for serve, with explicit positive fixtures and no invented defaults, and pass owned policy to web; offline openapi needs no runtime policy (FR-001, FR-CON-01).
- [X] T065 Write failing test: [unit] `release_graph_excludes_mocks_and_fakes` in `scripts/tests/test_release_features.py`; expect default/release feature graph to include forbidden testkit or context mocks, or permit release dev-fakes (FR-001, FR-CON-01).
- [X] T066 Make pass: T065 add `scripts/check-release-features.py`, guard debug-only dev-fakes in `crates/app/src/lib.rs`, and update `crates/app/Cargo.toml`, `.github/workflows/ci.yml`; inspect cargo tree feature closure and a negative release-build invocation (FR-001, FR-CON-01).

### Bilingual shell and design tokens

- [X] T067 Write failing test: [unit] `token_set_matches_authoritative_semantic_roles` in `scripts/tests/test_design_tokens.py`; expect absent/extra roles versus tech spec §12.2 and missing source values for type/spacing/radii/icons/motion (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-047).
- [X] T068 Make pass: T067 create `crates/web/design/tokens.toml`; encode exactly the sixteen semantic colour names in §12.2 for both themes, §12.3 scales with proposed typography values kept labeled proposed and declared pair/large-text/control roles; keep proposed motion values labeled proposed (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-047).
- [X] T069 Write failing test: [unit] `token_css_is_generated_and_no_stray_hex_is_accepted` in `scripts/tests/test_generated_tokens.py`; inject a stray literal in a disposable template/CSS fixture; expect absent generator/scanner or independently defined visual values (FR-038, FR-UX-20, AC-ADM-047).
- [X] T070 Make pass: T069 add `crates/web/Cargo.toml`, `crates/web/src/lib.rs`, `crates/web/build.rs`, `scripts/check-design-values.py`, workspace membership; generate CSS into OUT_DIR from tokens only, scan authored templates/CSS/brand SVG against stray literals, and preserve supplied artwork without inventing brand assets (FR-038, FR-UX-20, AC-ADM-047).
- [X] T071 Write failing test: [unit] `visual_identity_values_have_one_source_and_checked_contrast` and `foundation_brand_and_contrast_follow_approved_baseline` in `crates/web/tests/design_tokens.rs`; mutate a pair below 4.5/3.0 and expect failure; control/focus 3:1, unrounded ratios, border never sole boundary (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-029, AC-ADM-047).
- [X] T072 Make pass: T071 implement token parsing/sRGB contrast/pair validation in `crates/web/src/design.rs`; read values from TOML, not duplicated test constants; forbid white-small-text/light-sky roles (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-029, AC-ADM-047).
- [X] T073 Write failing test: [unit] `foundation_copy_has_fa_en_parity_and_context` in `crates/web/tests/fluent.rs`; expect absent surface/situation keys, mismatched variables and silent fallback (FR-030, FR-UX-01, FR-UX-14, AC-ADM-031).
- [X] T074 Make pass: T073 implement Fluent loading in `crates/web/src/i18n.rs`, `crates/web/locales/fa/console.ftl`, `crates/web/locales/en/console.ftl`; both locales include base shell, theme, validation and accessible-name keys (FR-030, FR-UX-01, FR-UX-14, AC-ADM-031).
- [X] T075 Write failing test: [http] `base_shell_has_locale_theme_and_accessible_structure` in `crates/web/tests/shell.rs`; instantiate Askama for six locale/theme choices; expect missing lang/dir, one h1, labels or CSS logical properties (FR-025, FR-027, FR-029, FR-UX-01, FR-UX-08).
- [X] T076 Make pass: T075 implement `crates/web/templates/console/layout.html`, `crates/web/templates/partials/field.html`, `crates/web/src/shell.rs`, `crates/web/static/console.css`; use generated tokens, self-hosted Vazirmatn and local htmx with licenses under `crates/web/static/vendor/` as exact asset filenames `vazirmatn.woff2`, `OFL.txt`, `htmx.min.js`, `htmx-LICENSE.txt`; no invented console logo (FR-025, FR-027, FR-029, FR-UX-01, FR-UX-06, Q03).
- [X] T077 Write failing test: [http] `csrf_is_required_for_every_mutating_console_form` in `crates/web/tests/csrf.rs`; expect missing origin/token checks and insecure cookie attributes; use a test-only mutation route, not a placeholder production action (FR-001, FR-002, FR-CON-02, Q23).
- [X] T078 Make pass: T077 implement pure CSRF/cookie transport validation in `crates/web/src/security.rs`; middleware receives browser-bound expected token material from application context (synthetic context only in this foundation test); it generates no randomness and calls no port directly; Secure/HttpOnly/SameSite, host-only cookies, no GET mutation (FR-001, FR-002, FR-CON-02, Q23).

### Health port, API contracts and offline export

- [X] T079 Write failing test: [unit] `health_probe_is_an_async_mockable_port` in `crates/admin/src/ports/health_probe.rs`; expect missing trait/automock for sanitized store-and-seed readiness (FR-001, FR-CON-01).
- [X] T080 Make pass: T079 define HealthProbe in `crates/admin/src/ports/health_probe.rs`, `crates/admin/src/ports/mod.rs`, `crates/admin/src/lib.rs`; owned ready/unavailable DTO contains no server/provider details (FR-001, FR-CON-01).
- [X] T081 Write failing test: [contract] `health_probe_contract` in `crates/testkit/src/contracts/health_probe.rs`, runner `crates/testkit/tests/health_probe_fake.rs`; expect absent seeded/unseeded/unavailable fixtures and typed errors (FR-001, FR-CON-01).
- [X] T082 Make pass: T081 implement HealthProbe fake in `crates/testkit/src/fakes/health_probe.rs` and module indexes; explicitly configured fixture readiness, never infer success from an absent backend (FR-001, FR-CON-01).
- [X] T083 Write failing test: [mock] [fake] `get_health_calls_probe_once_and_fails_closed` in `crates/testkit/tests/application_health.rs`; expect missing service, repeated calls or unavailable→ready conversion (FR-001, FR-CON-01).
- [X] T084 Make pass: T083 implement `crates/admin/src/application/health.rs`, register `crates/admin/src/application/mod.rs`; service depends only on HealthProbe (FR-001, FR-CON-01).
- [X] T085 Write failing test: [http] `health_and_problem_responses_match_schema` in `crates/web/tests/health.rs`; expect undocumented/non-v1 JSON or leaked infrastructure details; assert 200 ready and 503 problem+json unavailable (FR-001, FR-CON-01; constitution VI).
- [X] T086 Make pass: T085 implement annotated health handler, shared RFC 9457 errors and OpenApiRouter in `crates/web/src/api/health.rs`, `crates/web/src/api/problem.rs`, `crates/web/src/api/mod.rs` and `crates/web/src/lib.rs`; no JSON auth/management API added (FR-001, FR-CON-01).
- [X] T087 Write failing test: [http] `schema_endpoint_describes_itself_finitely` and `all_json_routes_are_documented_under_v1` in `crates/web/tests/openapi.rs`; expect missing schema route or recursive document embedding (FR-001, FR-CON-01; constitution VI).
- [X] T088 Make pass: T087 implement frozen schema bridge/router builder in `crates/web/src/api/openapi.rs`, register `crates/web/src/api/mod.rs`; annotate `/api/v1/openapi.json`, split router/document once, no undocumented `/api/openapi.json` (FR-001, FR-CON-01).
- [X] T089 Write failing test: [http] `swagger_makes_no_external_asset_or_validator_requests` in `crates/web/tests/swagger.rs`; expect missing local `/api/docs`, remote validator/assets or an undocumented extra schema URL (FR-001, FR-CON-01; constitution VI).
- [X] T090 Make pass: T089 implement vendored Swagger UI with external validator disabled in `crates/web/src/api/docs.rs`, `crates/web/src/api/mod.rs`, `crates/web/Cargo.toml`; production auth wrapper is driven in US1 once real session services exist (FR-001, FR-CON-01).
- [X] T091 Write failing test: [http] `csp_allows_only_local_console_scripts` in `crates/web/tests/csp.rs`; expect missing/unsafe CSP, inline handlers or an external script/validator to be allowed; assert the local passkey/dialog/Swagger scripts remain permitted (FR-001, FR-002, FR-CON-01, FR-CON-02).
- [X] T092 Make pass: T091 implement the strict header policy in contracts/http-ui.md, with local-file script initialization and no unsafe-inline/unsafe-eval in `crates/web/src/middleware/csp.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-CON-01, FR-CON-02).
- [X] T093 Write failing test: [http] `oversized_body_never_reaches_application` in `crates/web/tests/body_limit.rs`; expect a body beyond a configured finite positive byte limit to reach the application; require localized HTML 413 or JSON problem+json 413 (FR-001, FR-002, FR-CON-01, FR-CON-02).
- [X] T094 Make pass: T093 implement only pre-handler bounded-body rejection, preserving CSRF and admin checks in `crates/web/src/middleware/body_limit.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-CON-01, FR-CON-02).
- [X] T095 Write failing test: [http] `concurrency_limit_rejects_excess_and_releases_permits` in `crates/web/tests/concurrency_limit.rs`; expect excess in-flight requests to enter the handler or permits to leak after completion/error/cancel; require bounded HTML/JSON 503 responses (FR-001, FR-002, FR-CON-01, FR-CON-02).
- [X] T096 Make pass: T095 implement only configured in-flight permits and immediate sanitized overload rejection; no unbounded queue in `crates/web/src/middleware/concurrency_limit.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-CON-01, FR-CON-02).
- [X] T097 Write failing test: [unit] `offline_openapi_export_needs_no_database` in `crates/app/tests/openapi_export.rs`; expect missing CLI/export or an attempted database/provider/config connection (FR-001, FR-CON-01; constitution VI).
- [X] T098 Make pass: T097 add `crates/app/src/main.rs`, `crates/app/src/openapi.rs`, app/web dependency wiring in `crates/app/Cargo.toml`; `tankhor openapi` uses the same builder with no live state construction (FR-001, FR-CON-01).
- [X] T099 Write failing test: [http] `openapi_snapshot_matches_router` in `crates/web/tests/openapi_snapshot.rs`; expect missing committed generated document or schema drift, with a pinned local meta-schema in `crates/web/tests/fixtures/openapi-meta-schema.json` (FR-001, FR-CON-01; constitution VI).
- [X] T100 Make pass: T099 generate `docs/openapi.json` via `cargo run -p app -- openapi`, add offline regeneration/diff to `.github/workflows/ci.yml`; never hand-edit the generated schema (FR-001, FR-CON-01).
- [X] T101 Setup: prepare a child-process outbound-network-denial harness in `crates/app/tests/support/network_sandbox.rs`; retain loopback listener access, refuse external sockets and verify the sandbox itself before the app smoke assertion. Missing isolation capability blocks setup, not a red business result (FR-001, FR-CON-01).
- [X] T102 Write failing test: [integration] `fake_foundation_shell_and_health_start_without_network` in `crates/app/tests/foundation_fake_serve.rs`; expect the prepared debug fake-mode process to fail to serve the localized shell and sanitized health endpoint without DB/S3/provider configuration, to contact an external service, or to register a placeholder management route; unseeded health must be 503, never fabricated ready (FR-001, FR-025, FR-CON-01, FR-UX-01).
- [X] T103 Make pass: T102 wire only delivered shell/health services to feature-gated fakes; extend crates/app/src/serve.rs, crates/app/src/main.rs and crates/app/src/composition.rs within the app crate; reject production fakes before acquisition and keep absent management routes unregistered in `crates/app/src/foundation.rs`; edit only this behavior and its module registration (FR-001, FR-025, FR-CON-01, FR-UX-01).
- [X] T104 Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Record actual kernel/fake/real/HTTP results in `specs/001-admin-foundation/tasks.md`; story auth/management and real health seeding remain explicitly undelivered (FR-CON-01, FR-CON-37, FR-UX-20).

**T104 observed checkpoint evidence (2026-11-10):** `cargo fmt --check` passed;
`cargo clippy --all-targets --all-features -- -D warnings` reported no issues. Fast suite with
CI features (`app/dev-fakes,testkit/fakes,admin/mocks,kernel/mocks`): 34 passed (kernel 6,
testkit fakes/contracts/services 7, admin 1, infra-system real Clock/IdGenerator contracts 2,
app 4, web HTTP/shell/design/i18n/security 14). Plain `cargo nextest run`: 33 passed; the only
difference is `app::foundation_fake_serve` (`required-features = ["dev-fakes"]`), which CI runs
through the featured fast-test step. `cargo nextest run --profile integration`: 4 passed with
`integration-services` SETUP PASS (real Postgres UnitOfWork contract, SQLx case isolation,
shared fixture). `cargo deny check`: advisories, bans, licenses and sources ok. Architecture,
release-feature, design-value checks and partial traceability passed; Python repository suite
23 passed. Real health seeding (T356), admin authentication and settings/flags/audit management
remain undelivered; `TANKHOR_ADAPTERS=real serve` refuses to start, and management routes are
unregistered. No commit performed.

## Phase 3: User Story 1 — Enter and Recover the Console Securely (P1, MVP)

**Goal**: Real admin-only primary proof plus passkey/TOTP, separate sessions, another-admin
invitation/recovery, immediate target-only revocation and attributable access.
**Independent test**: AC-ADM-001–007/043 with controlled fakes, then real local crypto,
PostgreSQL and wiremock SMS; neither ordinary cookies nor missing/invalid factors grant access.

### Pure access domain

**U3 — existing T109/T110 driver and implementation criteria (FR-004, AC-ADM-043):**
Extend `admin_eligibility_epoch_and_equal_permissions_are_pure` before implementation:
each committed admin revocation, deactivation or recovery strictly advances that admin's
durable, monotonic security epoch. Reactivation/restoration never resets or reuses an epoch.
Drive deactivate→reactivate and revoke→restore sequences retaining an epoch greater than
the starting value, recovery advancement and unchanged-epoch control; property sequences
reject regression/reuse and overflow must fail closed, never wrap. T110 implements only
these driven pure transitions; persistence and execution remain their existing later slices.

- [ ] T105 [US1] Write failing test: [unit] `security_policy_is_finite_and_nondisableable` in `crates/admin/src/domain/security_policy.rs`; expect missing/zero expiry, attempt, session or abuse limits and invalid RP/origin configuration to pass (FR-002, FR-011, FR-CON-02, Q09, Q23).
- [ ] T106 [US1] Make pass: T105 implement validated operator policy in `crates/admin/src/domain/security_policy.rs`, `crates/admin/src/domain/mod.rs`, `crates/admin/src/lib.rs`; no invented business defaults or editable second-factor switch (FR-002, FR-011, FR-CON-02, Q23).
- [ ] T107 [US1] Write failing test: [unit] `audit_excludes_authentication_secrets_and_private_data` in `crates/admin/src/domain/audit.rs`; expect arbitrary snapshots/Debug secrets, missing actor/action/subject/time/reason and unrestricted reason text (FR-021, FR-024, FR-CON-37, AC-ADM-024). Include safe scheduled execution/cancellation event metadata (actor=scheduler, executed_by=system_worker, approver and visible reason) and reject provisioning secret/QR canaries in audit payloads; no private identifiers or client-chosen executor identity.
- [ ] T108 [US1] Make pass: T107 implement AuditEntry, allowlisted metadata, redacted secret wrappers and bilingual ReasonCode identifiers in `crates/admin/src/domain/audit.rs`, `crates/admin/src/domain/secrets.rs`, domain module index; never store raw phone/code/key/photo/payment content (FR-021, FR-024, FR-CON-37, AC-ADM-024). Support only the preceding allowlisted scheduled-cancellation/executor/approver metadata in the pure typed audit value; no logging or UI provisioning leakage.
- [ ] T109 [US1] Write failing test: [unit] `admin_eligibility_epoch_and_equal_permissions_are_pure` in `crates/admin/src/domain/admin_state.rs`; expect ineligible/current-epoch transitions or unequal launch permissions to be accepted; proptest transition sequences (FR-004, FR-005, FR-CON-03, AC-ADM-007, AC-ADM-043). Drive identity-bound protected enrollment authorization (purpose/authorizer/epoch/time/expiry/status), one-use consume/revoke transitions and the distinction from browser-bound proof; no transferable enrollment token.
- [ ] T110 [US1] Make pass: T109 implement only Admin eligibility, security epoch and equal-permission rules in `crates/admin/src/domain/admin_state.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-007, AC-ADM-043). Keep enrollment authorization on protected Admin state; the target proves their identity with phone OTP, never a delivered bearer grant.
- [ ] T111 [US1] Write failing test: [unit] `credential_revision_and_revocation_are_epoch_bound` in `crates/admin/src/domain/credential_state.rs`; expect a stale/revoked credential or nonincreasing accepted TOTP step to remain usable; proptest transition sequences (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T112 [US1] Make pass: T111 implement only Credential revision/epoch/revocation and accepted-step transitions in `crates/admin/src/domain/credential_state.rs`; edit only this behavior and its module registration (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T113 [US1] Write failing test: [unit] `access_requires_primary_factor_and_current_epoch` in `crates/admin/src/domain/session_state.rs`; expect a primary-only, expired, revoked or old-epoch session to grant access; proptest transition sequences (FR-002, FR-003, FR-CON-02, AC-ADM-002, AC-ADM-003, AC-ADM-043).
- [ ] T114 [US1] Make pass: T113 implement only Session expiry/factor/epoch access rules with explicit input time in `crates/admin/src/domain/session_state.rs`; edit only this behavior and its module registration (FR-002, FR-003, FR-CON-02, AC-ADM-002, AC-ADM-003, AC-ADM-043).
- [ ] T115 [US1] Write failing test: [unit] `auth_state_requires_browser_purpose_epoch_and_one_use` in `crates/admin/src/domain/auth_state.rs`; expect replayed/wrong-browser/wrong-purpose or stale-epoch enrollment/ceremony proof to advance; proptest transition sequences (FR-002, FR-004, FR-CON-03, AC-ADM-002, AC-ADM-005, AC-ADM-006, AC-ADM-043). Stage, target and session binding also apply. Authorization is identity-bound Admin state; this pure AuthState value represents SessionStore pending proofs/ceremonies, not a repository or transferable grant.
- [ ] T116 [US1] Make pass: T115 implement only AuthState restricted proof and enrollment transitions; no last-admin recovery bypass in `crates/admin/src/domain/auth_state.rs`; edit only this behavior and its module registration (FR-002, FR-004, FR-CON-03, AC-ADM-002, AC-ADM-005, AC-ADM-006, AC-ADM-043). SessionStore persists these pending proof/ceremony values; protected target authorization remains on Admin state.
- [ ] T117 [US1] Write failing test: [unit] `otp_state_expires_supersedes_and_consumes_once` in `crates/admin/src/domain/otp_state.rs`; expect expired/superseded/consumed digest-backed OTP state to verify or failed attempts to reset; proptest transition sequences (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T118 [US1] Make pass: T117 implement only pure OTP state transitions using explicit time/IDs and opaque digests in `crates/admin/src/domain/otp_state.rs`; edit only this behavior and its module registration (FR-002, FR-CON-02, AC-ADM-002).

### Port traits, contracts and fakes — no PostgreSQL code

**U3 — existing T123–T126 repository surface/contract criteria (FR-004, AC-ADM-043):**
T123/T124 cover the existing admin-state/epoch CAS operations committing invalidating state
and a strictly greater epoch atomically; no new port, service or endpoint. Extend T125's
`admin_repository_contract` before T126's fake implementation: commit deactivation then
reactivation in separate UoWs, and revocation then restoration in separate UoWs; fresh reads
must retain the advanced epoch after restoration, never reset or reuse it. Also cover every
committed recovery advancing epoch, unchanged-epoch reads, stale CAS/non-increasing invalidation write refusal,
overflow refusal, concurrent invalidations serialized by target lock/expected-epoch CAS and
rollback preserving both prior state and epoch. These are repository persistence assertions,
not a dependency on an unbuilt worker. T126 implements the expanded suite in the existing
transactional fake; T205 runs it unchanged on PostgreSQL and T206 implements the same atomic
durable behavior through the caller UoW, using no adapter-to-adapter orchestration.

- [ ] T119 [US1] Write failing test: [unit] `audit_repository_port_is_mockable_and_append_only` in `crates/admin/src/ports/audit_repository.rs`; expect missing async append/search/MockAuditRepository or an edit/delete API (FR-021, FR-022, FR-023, FR-CON-37).
- [ ] T120 [US1] Make pass: T119 define AuditRepository in `crates/admin/src/ports/audit_repository.rs`, register `crates/admin/src/ports/mod.rs`; bounded cursor/filter DTOs and shared TxHandle only (FR-021, FR-022, FR-023, FR-CON-37).
- [ ] T121 [US1] Write failing test: [contract] `audit_repository_contract` in `crates/testkit/src/contracts/audit_repository.rs`, runner `crates/testkit/tests/audit_repository_fake.rs`; expect missing event-key uniqueness, transactional failure rollback, actor/action/subject/time filters and `(occurred_at,id)` ordering (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025). Round-trip scheduled actor/system-worker executor/approver/reason safely, with a stable cancellation event key and once-only append/failure rollback; no success-publication event on cancellation.
- [ ] T122 [US1] Make pass: T121 implement `crates/testkit/src/fakes/audit_repository.rs`, extend `crates/testkit/src/fakes/store.rs` and testkit indexes; append-only fake and failure injection share the UoW store (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025). Preserve the expanded allowlisted cancellation metadata and stable event-key semantics in the shared audit fake; no application cancellation policy inside this repository.
- [ ] T123 [US1] Write failing test: [unit] `admin_repository_port_is_async_and_mockable` in `crates/admin/src/ports/admin_repository.rs`; expect missing digest lookup/safe bounded list/get/lock/invite/epoch-CAS surface (FR-004, FR-005, FR-CON-03). Cover protected target authorization create/read/conditional consume/revoke/supersede, purpose/authorizer/current epoch/finite expiry/status, another-admin authorization and one-use concurrency without a transferable token; caller UoW and target lock preserve atomic audit/recovery.
- [ ] T124 [US1] Make pass: T123 define AdminRepository in `crates/admin/src/ports/admin_repository.rs`, register port index; no plaintext identity in returned audit DTOs (FR-004, FR-005, FR-CON-03). Implement protected target-identity enrollment authorization operations and atomic conditional consume/revoke under the caller transaction; keep browser ceremony state in SessionStore.
- [ ] T125 [US1] Write failing test: [contract] `admin_repository_contract` in `crates/testkit/src/contracts/admin_repository.rs`, runner `crates/testkit/tests/admin_repository_fake.rs`; expect absent unique keyed identity, safe bounded listing, eligibility, epoch CAS and stable sorted locking (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043). Cover protected target authorization create/read/conditional consume/revoke/supersede, purpose/authorizer/current epoch/finite expiry/status, another-admin authorization and one-use concurrency without a transferable token; caller UoW and target lock preserve atomic audit/recovery. Include current scheduling-admin active/eligible vs revoked/deactivated/epoch-changed reads and row locking under recovery; no new admin lifecycle service or route.
- [ ] T126 [US1] Make pass: T125 implement `crates/testkit/src/fakes/admin_repository.rs`, extend shared store and indexes; reject wrong-store/finished handles and duplicate identities (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043). Implement protected target-identity enrollment authorization operations and atomic conditional consume/revoke under the caller transaction; keep browser ceremony state in SessionStore.
- [ ] T127 [US1] Write failing test: [unit] `admin_credential_repository_port_is_mockable` in `crates/admin/src/ports/admin_credential_repository.rs`; expect missing active-list/insert/lock/revision-CAS/accepted-step-CAS/revoke-all (FR-002, FR-004, FR-CON-02).
- [ ] T128 [US1] Make pass: T127 define AdminCredentialRepository in `crates/admin/src/ports/admin_credential_repository.rs`, register port index; library credential/secret material remains opaque and redacted (FR-002, FR-004, FR-CON-02).
- [ ] T129 [US1] Write failing test: [contract] `admin_credential_repository_contract` in `crates/testkit/src/contracts/admin_credential_repository.rs`, runner `crates/testkit/tests/admin_credential_repository_fake.rs`; expect absent global credential uniqueness, stale revision/step rejection and target-only revocation (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T130 [US1] Make pass: T129 implement `crates/testkit/src/fakes/admin_credential_repository.rs`, shared store/index wiring; consume enrollment TOTP step as well as sign-in steps (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T131 [US1] Write failing test: [unit] `session_store_port_is_async_and_mockable` in `crates/admin/src/ports/session_store.rs`; expect missing insert/digest-lookup/current-check/revoke-one/revoke-all surface (FR-002, FR-003, FR-CON-02). Also drive Pending primary proofs and complete encrypted WebAuthn/TOTP ceremony state use create/read/conditional consume/supersede/revoke methods, with stage/target/browser/session/purpose/epoch/expiry/credential-revision checks, one-use replay protection, typed errors and target-only recovery revocation; no raw capability or serialized server state in client DTOs. Drive conditional TOTP provisioning-presented marking, one atomic winner and refusal to retrieve/redisplay the same secret/QR, including concurrent attempts and recovery/supersession; preserve encrypted state and digest-only references.
- [ ] T132 [US1] Make pass: T131 define SessionStore in `crates/admin/src/ports/session_store.rs`, register port index; sessions carry factor, epoch and finite absolute/idle expiry (FR-002, FR-003, FR-CON-02). Also implement Pending primary proofs and complete encrypted WebAuthn/TOTP ceremony state use create/read/conditional consume/supersede/revoke methods, with stage/target/browser/session/purpose/epoch/expiry/credential-revision checks, one-use replay protection, typed errors and target-only recovery revocation; no raw capability or serialized server state in client DTOs. Include conditional one-time TOTP provisioning presentation in SessionStore; never expose marked provisioning material through read/query DTOs or permit replay.
- [ ] T133 [US1] Write failing test: [contract] `session_store_contract` in `crates/testkit/src/contracts/session_store.rs`, runner `crates/testkit/tests/session_store_fake.rs`; expect absent expiry/epoch/target-only revocation and accidental raw capability persistence (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043). Also drive Pending primary proofs and complete encrypted WebAuthn/TOTP ceremony state use create/read/conditional consume/supersede/revoke methods, with stage/target/browser/session/purpose/epoch/expiry/credential-revision checks, one-use replay protection, typed errors and target-only recovery revocation; no raw capability or serialized server state in client DTOs. Drive conditional TOTP provisioning-presented marking, one atomic winner and refusal to retrieve/redisplay the same secret/QR, including concurrent attempts and recovery/supersession; preserve encrypted state and digest-only references.
- [ ] T134 [US1] Make pass: T133 implement `crates/testkit/src/fakes/session_store.rs`, shared store/index wiring; repeat revocation is harmless and access checks observe committed recovery immediately (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043). Also implement Pending primary proofs and complete encrypted WebAuthn/TOTP ceremony state use create/read/conditional consume/supersede/revoke methods, with stage/target/browser/session/purpose/epoch/expiry/credential-revision checks, one-use replay protection, typed errors and target-only recovery revocation; no raw capability or serialized server state in client DTOs. Include conditional one-time TOTP provisioning presentation in SessionStore; never expose marked provisioning material through read/query DTOs or permit replay.
- [ ] T135 [US1] Write failing test: [unit] `otp_challenge_repository_port_is_mockable` in `crates/admin/src/ports/otp_challenge_repository.rs`; expect missing superseding issue/failed-attempt-CAS/one-use consume/safe submission status (FR-002, FR-CON-02, Q09).
- [ ] T136 [US1] Make pass: T135 define OtpChallengeRepository in `crates/admin/src/ports/otp_challenge_repository.rs`, register port index; keyed digest only, original expiry retained across transport retries (FR-002, FR-CON-02, Q09).
- [ ] T137 [US1] Write failing test: [contract] `otp_challenge_repository_contract` in `crates/testkit/src/contracts/otp_challenge_repository.rs`, runner `crates/testkit/tests/otp_challenge_repository_fake.rs`; expect expiry/supersession/replay and concurrent consume to fail open (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T138 [US1] Make pass: T137 implement `crates/testkit/src/fakes/otp_challenge_repository.rs`, shared store/index wiring; failed-attempt budgets cannot reset on resend/failover (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T139 [US1] Write failing test: [unit] `rate_limit_repository_port_is_mockable` in `crates/admin/src/ports/rate_limit_repository.rs`; expect missing atomic keyed subject/origin bucket increment/read (FR-002, FR-CON-02, Q09).
- [ ] T140 [US1] Make pass: T139 define RateLimitRepository in `crates/admin/src/ports/rate_limit_repository.rs`, register port index; no raw phone/IP in errors or diagnostics (FR-002, FR-CON-02, Q09).
- [ ] T141 [US1] Write failing test: [contract] `rate_limit_repository_contract` in `crates/testkit/src/contracts/rate_limit_repository.rs`, runner `crates/testkit/tests/rate_limit_repository_fake.rs`; expect concurrency, finite-window boundaries or resend/failover to bypass budgets (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T142 [US1] Make pass: T141 implement `crates/testkit/src/fakes/rate_limit_repository.rs`, shared store/index wiring; deterministic Clock controls windows (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T143 [US1] Write failing test: [unit] `admin_authenticator_port_is_mockable_and_secret_safe` in `crates/admin/src/ports/admin_authenticator.rs`; expect missing passkey/TOTP/OTP-digest/token/envelope operations or exposed vendor structs (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T144 [US1] Make pass: T143 define AdminAuthenticator in `crates/admin/src/ports/admin_authenticator.rs`, register port index; all randomness/challenges/crypto cross this port, IDs/time through their own ports (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T145 [US1] Write failing test: [contract] `admin_authenticator_contract` in `crates/testkit/src/contracts/admin_authenticator.rs`, runner `crates/testkit/tests/admin_authenticator_fake.rs`; expect missing valid/invalid fixture proof, RP/origin/binding checks, matched TOTP steps and redacted errors (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T146 [US1] Make pass: T145 implement fixture-bound `crates/testkit/src/fakes/admin_authenticator.rs` and testkit indexes; exact controlled proofs only, never unconditional success; pass common fake contract before crypto adapter work (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T147 [US1] Write failing test: [unit] `sms_sender_port_is_mockable` in `crates/admin/src/ports/sms_sender.rs`; expect missing approved-template send with safe accepted/rejected/unavailable/unknown-result categories (FR-002, FR-CON-02, Q09).
- [ ] T148 [US1] Make pass: T147 define SmsSender in `crates/admin/src/ports/sms_sender.rs`, register port index; send acceptance is not verification or delivery (FR-002, FR-CON-02, Q09).
- [ ] T149 [US1] Write failing test: [contract] `sms_sender_contract` in `crates/testkit/src/contracts/sms_sender.rs`, runner `crates/testkit/tests/sms_sender_fake.rs`; expect missing controlled accepted/rejected/timeout sequences and capture of the same challenge across failover (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T150 [US1] Make pass: T149 implement capturing `crates/testkit/src/fakes/sms_sender.rs` and indexes; redact captured payload Debug, expose synthetic test inspection without logging (FR-002, FR-CON-02, AC-ADM-002, Q09).

### Application services — mock and fake red tests before code

- [ ] T151 [US1] Write failing test: [mock] `sms_failover_reuses_challenge_and_expiry` in `crates/testkit/tests/mock_request_admin_otp.rs`; sequence rate-limit→eligible-admin→crypto→persist, finish UoW before vendor calls, at most primary+one eligible fallback, no calls for invalid identity and no budget reset (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T152 [US1] Write failing test: [fake] `sms_acceptance_is_not_primary_proof` in `crates/testkit/tests/fake_request_admin_otp.rs`; expect absent non-enumerating request flow and transport acceptance granting access; include resend supersession and unknown delivery outcome (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T153 [US1] Make pass: T151 T152 implement RequestAdminOtp in `crates/admin/src/application/request_admin_otp.rs`, register application index; application orchestrates two sender ports, never adapter→adapter or network while locked (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T154 [US1] Write failing test: [mock] `invalid_primary_proof_never_issues_session` in `crates/testkit/tests/mock_verify_admin_otp.rs`; expect missing once-only consume/attempt update and zero full-session insert on invalid/expired code (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T155 [US1] Write failing test: [fake] `primary_proof_is_browser_bound_and_one_use` in `crates/testkit/tests/fake_verify_admin_otp.rs`; expect absent restricted proof, expiry/rate/epoch enforcement and replay protection (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T156 [US1] Make pass: T154 T155 implement VerifyAdminOtp in `crates/admin/src/application/verify_admin_otp.rs`, application index; no customer account creation or completed admin session yet (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T157 [US1] Write failing test: [mock] `failed_second_factor_never_issues_session` in `crates/testkit/tests/mock_passkey.rs`; expect missing verify→credential/state-CAS→session→audit-once ordering and rollback when audit fails (FR-002, FR-024, FR-CON-02, AC-ADM-002, AC-ADM-025). Drive `passkey_consumption_is_single_use_under_race_state_contract` and `audit_failure_aborts_access_state_contract` here: final epoch/revision/state CAS, persisted counters/backup state, port order/count, unchanged committed state on failed CAS or audit, no unaudited session. Real independent-connection acceptance remains T230/T233. Also drive `audit_passkey_signin_has_complete_safe_fields_once` in this original service driver: expect the passkey sign-in and factor event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T158 [US1] Write failing test: [fake] `console_requires_completed_second_factor` in `crates/testkit/tests/fake_passkey.rs`; expect missing bound begin/finish passkey flow, stale credential/epoch refusal and persisted counter/backup updates (FR-002, FR-CON-02, AC-ADM-002). Drive `passkey_consumption_is_single_use_under_race_state_contract` and `audit_failure_aborts_access_state_contract` here: final epoch/revision/state CAS, persisted counters/backup state, port order/count, unchanged committed state on failed CAS or audit, no unaudited session. Real independent-connection acceptance remains T230/T233. Also drive `audit_passkey_signin_has_complete_safe_fields_once` in this original service driver: expect the passkey sign-in and factor event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T159 [US1] Make pass: T157 T158 implement BeginPasskey/FinishPasskey in `crates/admin/src/application/passkey.rs`, application index; one-use server-state check and final session/audit UoW, crypto outside locks (FR-002, FR-024, FR-CON-02, AC-ADM-002, AC-ADM-025). Complete the typed allowlisted `audit_passkey_signin_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T160 [US1] Write failing test: [mock] `totp_step_and_session_audit_commit_once` in `crates/testkit/tests/mock_totp.rs`; expect missing accepted-step CAS before session and exactly one audit; no writes after invalid proof (FR-002, FR-021, FR-CON-02, AC-ADM-002). Drive `otp_and_totp_consumption_are_single_use_under_race_state_contract` here: final accepted-step CAS before session/audit commit, exact port order/count and unchanged committed state on failure; real race acceptance remains T231. Also drive `audit_totp_signin_has_complete_safe_fields_once` in this original service driver: expect the TOTP sign-in and factor event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T161 [US1] Write failing test: [fake] `totp_fallback_rejects_replayed_or_expired_proof` in `crates/testkit/tests/fake_totp.rs`; expect absent fallback with primary/epoch checks and replayed enrollment/sign-in step rejection (FR-002, FR-CON-02, AC-ADM-002). Drive `otp_and_totp_consumption_are_single_use_under_race_state_contract` here: final accepted-step CAS before session/audit commit, exact port order/count and unchanged committed state on failure; real race acceptance remains T231. Also drive `audit_totp_signin_has_complete_safe_fields_once` in this original service driver: expect the TOTP sign-in and factor event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T162 [US1] Make pass: T160 T161 implement FinishTotp in `crates/admin/src/application/totp.rs`, application index; fallback still requires primary proof and audited final UoW (FR-002, FR-021, FR-CON-02, AC-ADM-002). Include the original-driver final accepted-step CAS before session/audit commit, with no later repair pass. Complete the typed allowlisted `audit_totp_signin_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T163 [US1] Write failing test: [mock] `passkey_enrollment_authorization_is_not_console_access_port_interactions` in `crates/testkit/tests/mock_begin_passkey_enrollment.rs`; expect authorization/actor validation not to precede credential/state writes and exactly one applicable audit (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token.
- [ ] T164 [US1] Write failing test: [fake] `passkey_enrollment_authorization_is_not_console_access` in `crates/testkit/tests/fake_begin_passkey_enrollment.rs`; expect wrong-stage/browser/session/purpose/target/epoch primary proof or absent/expired authorization to start a ceremony, or authorization alone to grant console access (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token.
- [ ] T165 [US1] Make pass: T163 T164 implement only BeginPasskeyEnrollment, preserving the other operations and epoch binding in `crates/admin/src/application/enrollment.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token.
- [ ] T166 [US1] Write failing test: [mock] `passkey_enrollment_confirms_bound_factor_once_port_interactions` in `crates/testkit/tests/mock_finish_passkey_enrollment.rs`; expect authorization/actor validation not to precede credential/state writes and exactly one applicable audit (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Also drive `audit_enrollment_has_complete_safe_fields_once` in this original service driver: expect the confirmed factor enrollment event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T167 [US1] Write failing test: [fake] `passkey_enrollment_confirms_bound_factor_once` in `crates/testkit/tests/fake_finish_passkey_enrollment.rs`; expect stale/replayed ceremony state to create a credential without a confirmed factor and one audit (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Also drive `audit_enrollment_has_complete_safe_fields_once` in this original service driver: expect the confirmed factor enrollment event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T168 [US1] Make pass: T166 T167 implement only FinishPasskeyEnrollment, preserving the other operations and epoch binding in `crates/admin/src/application/enrollment.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Atomically consume authorization and one-use ceremony with credential/matched-step, session and audit; failure rolls back all effects. Complete the typed allowlisted `audit_enrollment_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T169 [US1] Write failing test: [mock] `totp_enrollment_secret_is_bound_and_presented_once_port_interactions` in `crates/testkit/tests/mock_begin_totp_enrollment.rs`; expect authorization/actor validation not to precede credential/state writes and exactly one applicable audit (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Also drive `totp_provisioning_is_once_authenticated_and_no_store`: Constitution v1.2.0: only the admin performing the authenticated enrollment ceremony may receive exactly one TOTP secret/QR display with Cache-Control: no-store. Require target phone-OTP proof and bound stage/browser/session/purpose/epoch/authorization; atomically mark provisioning presented before returning it. Repeated/concurrent/refresh/query/error paths never redisplay the same material; never log, bundle, put it in a URL or send it elsewhere, and no other secret may appear in UI. A lost response needs a fresh superseding ceremony, not redisplay; first enrollment does not require an already-completed second factor. Application tests cover the one-use presentation result/policy, not HTTP types; the actual no-store response header is driven by T260. Retain opaque protected verifier state for FinishTotpEnrollment without making it re-displayable.
- [ ] T170 [US1] Write failing test: [fake] `totp_enrollment_secret_is_bound_and_presented_once` in `crates/testkit/tests/fake_begin_totp_enrollment.rs`; expect absent authorization or unbound primary proof to create/present a secret, or secret data to enter logs/audit (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Also drive `totp_provisioning_is_once_authenticated_and_no_store`: Constitution v1.2.0: only the admin performing the authenticated enrollment ceremony may receive exactly one TOTP secret/QR display with Cache-Control: no-store. Require target phone-OTP proof and bound stage/browser/session/purpose/epoch/authorization; atomically mark provisioning presented before returning it. Repeated/concurrent/refresh/query/error paths never redisplay the same material; never log, bundle, put it in a URL or send it elsewhere, and no other secret may appear in UI. A lost response needs a fresh superseding ceremony, not redisplay; first enrollment does not require an already-completed second factor. Application tests cover the one-use presentation result/policy, not HTTP types; the actual no-store response header is driven by T260. Retain opaque protected verifier state for FinishTotpEnrollment without making it re-displayable.
- [ ] T171 [US1] Make pass: T169 T170 implement only BeginTotpEnrollment, preserving the other operations and epoch binding in `crates/admin/src/application/enrollment.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Implement only the preceding-driver one-time provisioning behavior: Constitution v1.2.0: only the admin performing the authenticated enrollment ceremony may receive exactly one TOTP secret/QR display with Cache-Control: no-store. Require target phone-OTP proof and bound stage/browser/session/purpose/epoch/authorization; atomically mark provisioning presented before returning it. Repeated/concurrent/refresh/query/error paths never redisplay the same material; never log, bundle, put it in a URL or send it elsewhere, and no other secret may appear in UI. A lost response needs a fresh superseding ceremony, not redisplay; first enrollment does not require an already-completed second factor. Application tests cover the one-use presentation result/policy, not HTTP types; the actual no-store response header is driven by T260. Retain opaque protected verifier state for FinishTotpEnrollment without making it re-displayable.
- [ ] T172 [US1] Write failing test: [mock] `enrollment_authorization_is_not_console_access_port_interactions` in `crates/testkit/tests/mock_finish_totp_enrollment.rs`; expect authorization/actor validation not to precede credential/state writes and exactly one applicable audit (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Also drive `audit_enrollment_has_complete_safe_fields_once` in this original service driver: expect the confirmed factor enrollment event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T173 [US1] Write failing test: [fake] `enrollment_authorization_is_not_console_access` in `crates/testkit/tests/fake_finish_totp_enrollment.rs`; expect a replayed enrollment step or missing primary proof to complete enrollment/access (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Also drive `audit_enrollment_has_complete_safe_fields_once` in this original service driver: expect the confirmed factor enrollment event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T174 [US1] Make pass: T172 T173 implement only FinishTotpEnrollment, preserving the other operations and epoch binding in `crates/admin/src/application/enrollment.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005). Require target phone-OTP proof plus protected AdminRepository authorization, independently recheck stage/target/browser/session/purpose/epoch/expiry and replay; SessionStore owns ceremonies, not a delivered enrollment token. Atomically consume authorization and one-use ceremony with credential/matched-step, session and audit; failure rolls back all effects. Complete the typed allowlisted `audit_enrollment_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T175 [US1] Write failing test: [mock] `admin_addition_requires_another_admin_and_audit_port_interactions` in `crates/testkit/tests/mock_invite_admin.rs`; expect another-admin authorization/reason validation not to precede one create/authorization/audit UoW, or self-registration to write anything (FR-004, FR-021, FR-CON-03, AC-ADM-004). Record authorization against the target identity in protected Admin state and audit it; no bearer token/delivery path. Also drive `audit_invitation_has_complete_safe_fields_once` in this original service driver: expect the another-admin invitation event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T176 [US1] Write failing test: [fake] `admin_addition_requires_another_admin_and_audit` in `crates/testkit/tests/fake_invite_admin.rs`; expect duplicate identity/self-invitation to succeed or new eligibility to grant a full session (FR-004, FR-021, FR-CON-03, AC-ADM-004). Also drive `audit_invitation_has_complete_safe_fields_once` in this original service driver: expect the another-admin invitation event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T177 [US1] Make pass: T175 T176 implement InviteAdmin only in `crates/admin/src/application/invite_admin.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-004, FR-021, FR-CON-03, AC-ADM-004). Complete the typed allowlisted `audit_invitation_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T178 [US1] Write failing test: [mock] `admins_have_equal_permissions_and_distinct_attribution_port_interactions` in `crates/testkit/tests/mock_list_admins.rs`; expect one bounded safe list read to precede current-admin authorization or any write to occur (FR-001, FR-005, FR-CON-04, AC-ADM-007).
- [ ] T179 [US1] Write failing test: [fake] `admins_have_equal_permissions_and_distinct_attribution` in `crates/testkit/tests/fake_list_admins.rs`; expect listing to lose distinct internal actors/equal permissions, expose phone/credential material or accept an invalid cursor (FR-001, FR-005, FR-CON-04, AC-ADM-007).
- [ ] T180 [US1] Make pass: T178 T179 implement ListAdmins only with safe owned DTOs in `crates/admin/src/application/list_admins.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-001, FR-005, FR-CON-04, AC-ADM-007).
- [ ] T181 [US1] Write failing test: [mock] `recovery_only_revokes_target_admin` in `crates/testkit/tests/mock_recover_admin.rs`; expect missing sorted-admin locks→epoch bump→all target session/credential/proof/OTP revocations→restricted target authorization→one audit and rollback on failure (FR-004, FR-024, FR-CON-03, AC-ADM-025, AC-ADM-043). Revoke pending SessionStore states and protected authorization before recording fresh identity authorization at the new epoch; no transferable credential. Drive `recovery_races_with_login_safely_state_contract` here: atomic target epoch/child-state revocation against in-flight login, port order/count and unchanged committed state on failure; real concurrency remains T232. Also drive `audit_recovery_has_complete_safe_fields_once` in this original service driver: expect the another-admin recovery and target identity event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T182 [US1] Write failing test: [fake] `admin_recovery_preserves_second_factor_requirement`, `last_admin_lockout_has_no_bypass`, `recovery_revokes_all_admin_sessions_and_old_factors` in `crates/testkit/tests/fake_recover_admin.rs`; expect absent target-only atomic revocation, fresh primary/enrollment requirement or last-admin bypass (FR-004, FR-CON-03, AC-ADM-005, AC-ADM-006, AC-ADM-043). Drive `recovery_races_with_login_safely_state_contract` here: atomic target epoch/child-state revocation against in-flight login, port order/count and unchanged committed state on failure; real concurrency remains T232. Also drive `audit_recovery_has_complete_safe_fields_once` in this original service driver: expect the another-admin recovery and target identity event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T183 [US1] Make pass: T181 T182 implement RecoverAdmin in `crates/admin/src/application/recover_admin.rs`, application index; commit audit/revocation/authorization together and return no unaudited success (FR-004, FR-024, FR-CON-03, AC-ADM-005, AC-ADM-006, AC-ADM-025, AC-ADM-043). Include atomic target epoch and all child-state revocation against concurrent login, with no later repair pass. Complete the typed allowlisted `audit_recovery_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T184 [US1] Write failing test: [mock] `console_rejects_non_admin_access_port_interactions` in `crates/testkit/tests/mock_console_access.rs`; expect access to skip current SessionStore/AdminRepository/Clock validation or perform writes (FR-001, FR-002, FR-003, FR-024, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-025).
- [ ] T185 [US1] Write failing test: [fake] `console_rejects_non_admin_access` in `crates/testkit/tests/fake_console_access.rs`; expect end_user_session_does_not_grant_admin_access or expiry/epoch separation to fail; unauditable_admin_action_is_not_reported_successful remains a required related assertion (FR-001, FR-002, FR-003, FR-024, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-025).
- [ ] T186 [US1] Make pass: T184 T185 implement VerifyConsoleAccess only in `crates/admin/src/application/console_access.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-001, FR-002, FR-003, FR-024, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-025).
- [ ] T187 [US1] Write failing test: [mock] `signout_revokes_current_admin_session_only_port_interactions` in `crates/testkit/tests/mock_sign_out.rs`; expect more than one current-session revocation or unrelated session writes to occur (FR-003, FR-CON-02, AC-ADM-003, AC-ADM-043).
- [ ] T188 [US1] Write failing test: [fake] `signout_revokes_current_admin_session_only` in `crates/testkit/tests/fake_sign_out.rs`; expect repeated signout to revive a session or another admin to be signed out (FR-003, FR-CON-02, AC-ADM-003, AC-ADM-043).
- [ ] T189 [US1] Make pass: T187 T188 implement SignOut only in `crates/admin/src/application/sign_out.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-003, FR-CON-02, AC-ADM-003, AC-ADM-043).
- [ ] T190 [US1] Write failing test: [mock] `access_form_capability_is_browser_bound_not_an_auth_grant_port_interactions` in `crates/testkit/tests/mock_prepare_access_form.rs`; expect capability creation to bypass AdminAuthenticator or handlers to receive a raw port; require one bounded capability/state operation (FR-001, FR-002, FR-CON-01, AC-ADM-001, AC-ADM-002).
- [ ] T191 [US1] Write failing test: [fake] `access_form_capability_is_browser_bound_not_an_auth_grant` in `crates/testkit/tests/fake_prepare_access_form.rs`; expect CSRF/browser context to grant primary/full access or secret form values to be reflected (FR-001, FR-002, FR-CON-01, AC-ADM-001, AC-ADM-002).
- [ ] T192 [US1] Make pass: T190 T191 implement PrepareAccessForm only in `crates/admin/src/application/prepare_access_form.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-001, FR-002, FR-CON-01, AC-ADM-001, AC-ADM-002).
- [ ] T193 [US1] Write failing test: [mock] `bootstrap_admin_uses_ports_and_one_audited_uow` in `crates/testkit/tests/mock_bootstrap_admin.rs`; expect missing operator/bootstrap-precondition checks before AdminRepository eligibility creation, AdminAuthenticator protection/digests, AdminRepository protected target authorization and AuditRepository append in one UnitOfWork; Clock/IdGenerator supply time/IDs, audit failure rolls back and no factor/session is issued (FR-004, FR-024, FR-CON-03, AC-ADM-004, AC-ADM-006).
- [ ] T194 [US1] Write failing test: [fake] `bootstrap_admin_creates_restricted_eligibility_not_recovery_or_access` in `crates/testkit/tests/fake_bootstrap_admin.rs`; expect missing owner-authorised first-admin eligibility/authorization/audit, or a public/unauthorised/repeated bootstrap or last-admin self-recovery to succeed; confirm atomic audit failure and primary plus fresh factor still required (FR-004, FR-024, FR-CON-03, AC-ADM-004, AC-ADM-006).
- [ ] T195 [US1] Make pass: T193 T194 implement only BootstrapAdmin in `crates/admin/src/application/bootstrap_admin.rs`, register application index; own the existing operator-authorised eligibility/enrollment/audit transition through existing ports/UoW, never a public endpoint or recovery exception. App only delegates (FR-004, FR-024, FR-CON-03, AC-ADM-004, AC-ADM-006).
- [ ] T196 [US1] Write failing test: [mock] `get_enrollment_validates_authorization_and_primary_proof_without_writes` in `crates/testkit/tests/mock_get_enrollment.rs`; expect missing AdminAuthenticator purpose-separated reference/browser digest resolution, SessionStore primary-proof/ceremony reads and AdminRepository protected target authorization reads, AdminRepository current target epoch/eligibility and Clock expiry checks in a read-only UnitOfWork; assert zero state/audit writes, authorization consumption, ID/token issuance or session creation (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-004, AC-ADM-005, AC-ADM-043). Authorization is not browser-bound; the matching phone-OTP proof is. Assert missing authorization, wrong stage/session and zero authorization consumption too.
- [ ] T197 [US1] Write failing test: [fake] `get_enrollment_refuses_invalid_state_and_returns_safe_choices` in `crates/testkit/tests/fake_get_enrollment.rs`; expect missing safe choices for a valid protected target authorization plus verified primary proof, or missing/wrong-purpose/target/browser, unverified proof, stale epoch, revoked/consumed or expired-at-now state to be accepted; prove unchanged state and no secret/ceremony material or full access (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-004, AC-ADM-005, AC-ADM-043). A transferable token cannot substitute for target phone verification or protected authorization.
- [ ] T198 [US1] Make pass: T196 T197 implement only GetEnrollment in `crates/admin/src/application/get_enrollment.rs`, register application index; enforce contracts/application-ports.md authorization/primary/browser/epoch/expiry rules through existing ports, return safe factor choices and perform no mutations. Begin/finish retain independent checks (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-004, AC-ADM-005, AC-ADM-043).

### Real storage — only after all corresponding fake suites pass

- [ ] T199 [US1] Write failing test: [integration] `auth_schema_enforces_identity_proof_and_credential_constraints` in `crates/infra-postgres/tests/integration_auth_schema.rs`; expect absent tables and model constraints: admins "unique lookup digest"; credentials "globally unique WebAuthn credential ID; kind/material XOR; optimistic revision checks"; sessions "unique digest; FK admin; finite expiry"; SessionStore pending states "stage/target/browser/session/purpose/epoch/expiry/one-use checks"; admins "protected enrollment authorization and conditional consume/revoke"; OTP "finite expiry/attempt policy; compare-and-set consumption"; limits "unique bucket/window; atomic increment" (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043). Assert SessionStore provisioning_presented_at/phase constraints and one-time marking, with no plaintext provisioning material in storage or serializers.
- [ ] T200 [US1] Make pass: T199 add `crates/infra-postgres/migrations/0001_admin_access.sql`; implement data-model §5 columns/FKs/indexes, encrypted phone/secret envelopes, no raw capability/code, no audit cascade; existing cluster roles are fixture/operator-owned (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043). Store pending proofs/ceremonies in admin_session_states owned by SessionStore, and identity authorization on admins; no separate enrollment-grant table. Include the protected provisioning-presented marker demanded by T199; the caller transaction performs conditional one-use presentation.
- [ ] T201 [US1] Write failing test: [integration] `runtime_cannot_update_delete_or_truncate_audit`, `audit_guards_reject_privileged_dml`, `runtime_cannot_disable_guards_or_assume_owner` in `crates/infra-postgres/tests/integration_audit_guards.rs`; expect missing audit table/guards; assert "UNIQUE event key" and search indexes `(occurred_at,id)`, `(actor_id,occurred_at,id)`, `(action,occurred_at,id)`, `(subject_id,occurred_at,id)` (FR-021, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-023).
- [ ] T202 [US1] Make pass: T201 add `crates/infra-postgres/migrations/0002_audit.sql`; SELECT/INSERT-only runtime grants, UPDATE/DELETE guards plus statement-level TRUNCATE guard, no trigger/owner escalation and no cascading history mutation (FR-021, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-023).
- [ ] T203 [US1] Write failing test: [contract] [integration] `audit_repository_contract_postgres` in `crates/infra-postgres/tests/integration_audit_repository.rs`; run T121 unchanged, expect missing real append/search and audit-failure rollback (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025). Run expanded T121 unchanged for scheduled executor/approver/reason metadata and unique cancellation event keys.
- [ ] T204 [US1] Make pass: T203 implement `crates/infra-postgres/src/audit_repository.rs`, register adapter index; keyed retry result and bounded stable filtered paging, no mutation methods (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red. Persist the preceding typed safe cancellation metadata in the caller transaction; preserve audit-failure rollback and no secret-bearing snapshots.
- [ ] T205 [US1] Write failing test: [contract] [integration] `admin_repository_contract_postgres` in `crates/infra-postgres/tests/integration_admin_repository.rs`; run T125 unchanged, expect missing keyed identity uniqueness/row locks/epoch CAS (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043). The unchanged shared suite includes target authorization, current epoch/expiry/status and conditional consumption/revocation races. Include current scheduling-admin active/eligible vs revoked/deactivated/epoch-changed reads and row locking under recovery; no new admin lifecycle service or route.
- [ ] T206 [US1] Make pass: T205 implement `crates/infra-postgres/src/admin_repository.rs`, adapter index; use caller transaction only and sorted admin locks (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043). Implement protected target-identity enrollment authorization operations and atomic conditional consume/revoke under the caller transaction; keep browser ceremony state in SessionStore. Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red. Expose current scheduling eligibility/epoch through existing admin get/lock operations used by ScheduledPublish; preserve sorted lock rules.
- [ ] T207 [US1] Write failing test: [contract] [integration] `admin_credential_repository_contract_postgres` in `crates/infra-postgres/tests/integration_admin_credential_repository.rs`; run T129 unchanged, expect missing uniqueness/revision/accepted-step/revocation enforcement (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T208 [US1] Make pass: T207 implement `crates/infra-postgres/src/admin_credential_repository.rs`, adapter index; current epoch/revision/step checks remain transactional (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.
- [ ] T209 [US1] Write failing test: [contract] [integration] `session_store_contract_postgres` in `crates/infra-postgres/tests/integration_session_store.rs`; run T133 unchanged, expect absent expiry/epoch/current-access checks and target-only revocation (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043). Run the same expanded T133 suite, including all pending proof/ceremony cases, conditional-consume races and encrypted server-state handling formerly assigned to separate auth-state storage. Run expanded T133 unchanged, including the presented marker and concurrent one-winner/no-redisplay cases.
- [ ] T210 [US1] Make pass: T209 implement `crates/infra-postgres/src/session_store.rs`, adapter index; only capability digests persisted and current admin state joined on access (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043). Also implement Pending primary proofs and complete encrypted WebAuthn/TOTP ceremony state use create/read/conditional consume/supersede/revoke methods, with stage/target/browser/session/purpose/epoch/expiry/credential-revision checks, one-use replay protection, typed errors and target-only recovery revocation; no raw capability or serialized server state in client DTOs. Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red. Include conditional one-time TOTP provisioning presentation in SessionStore; never expose marked provisioning material through read/query DTOs or permit replay.
- [ ] T211 [US1] Write failing test: [contract] [integration] `otp_challenge_repository_contract_postgres` in `crates/infra-postgres/tests/integration_otp_challenge_repository.rs`; run T137 unchanged and race two consumes; expect more than one accepted proof (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T212 [US1] Make pass: T211 implement `crates/infra-postgres/src/otp_challenge_repository.rs`, adapter index; CAS consume/supersede/attempt operations share caller UoW (FR-002, FR-CON-02, AC-ADM-002, Q09). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.
- [ ] T213 [US1] Write failing test: [contract] [integration] `rate_limit_repository_contract_postgres` in `crates/infra-postgres/tests/integration_rate_limit_repository.rs`; run T141 unchanged with concurrent increments and finite window boundaries; expect lost increments (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T214 [US1] Make pass: T213 implement `crates/infra-postgres/src/rate_limit_repository.rs`, adapter index; no reset on provider switch/resend and no raw identity errors (FR-002, FR-CON-02, AC-ADM-002, Q09). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.

### Real crypto and SMS — no live vendor requests

- [ ] T215 [US1] Write failing test: [unit] `secret_envelope_binds_admin_and_epoch` and `totp_wrong_envelope_key_fails_closed` in `crates/infra-auth/tests/envelopes.rs`; expect missing AES-256-GCM key-id/nonce/ciphertext-tag handling and AAD checks (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T216 [US1] Make pass: T215 add `crates/infra-auth/Cargo.toml`, `crates/infra-auth/src/lib.rs`, `crates/infra-auth/src/envelopes.rs`, workspace membership; external protected key configuration and unique nonces, no Debug key output (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T217 [US1] Write failing test: [unit] `otp_and_capabilities_are_secure_and_digest_only` in `crates/infra-auth/tests/capabilities.rs`; expect missing six-digit OTP generation/keyed digest checks, independent opaque session/CSRF capabilities and encrypted canonical-phone lookup support (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T218 [US1] Make pass: T217 implement cryptographic randomness, constant-time digest verification and purpose-separated protection in `crates/infra-auth/src/capabilities.rs`, infra-auth module index; UUID IDs are not session capabilities (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T219 [US1] Write failing test: [unit] `totp_reports_matched_step_and_rejects_invalid_code` in `crates/infra-auth/tests/totp.rs`; expect missing RFC6238 SHA1/six digits/30s/one-step skew/160-bit-secret verification using fixed vectors, not loggable codes (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T220 [US1] Make pass: T219 implement totp-rs verification/enrollment primitives in `crates/infra-auth/src/totp.rs`, infra-auth module index; return matched step for application CAS, never claim the library alone prevents replay (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T221 [US1] Write failing test: [unit] `webauthn_rejects_wrong_origin_and_replayed_state`, `valid_zero_counter_passkey_is_supported` in `crates/infra-auth/tests/passkey.rs`; expect missing real signed-fixture start/finish verification, UV/RP/origin/challenge checks and credential update result (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T222 [US1] Make pass: T221 implement webauthn-rs safe API primitives in `crates/infra-auth/src/passkey.rs`, infra-auth module index; protected full server state, no danger-credential-internals or client-serialized ceremony state (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T223 [US1] Write failing test: [contract] [integration] `admin_authenticator_contract_real_crypto` in `crates/infra-auth/tests/authenticator_contract.rs`; run T145 unchanged with actual local vectors plus invalid/rebound/expired proof cases; expect missing complete port implementation (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T224 [US1] Make pass: T223 implement complete AdminAuthenticator delegation in `crates/infra-auth/src/admin_authenticator.rs`, infra-auth module index; no method stubs and no calls to other infra adapters (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T225 [US1] Write failing test: [contract] [integration] `sms_sender_contract_kavenegar` in `crates/infra-sms/tests/kavenegar.rs`; run T149 unchanged via wiremock, assert approved POST form lookup, timeout/malformed/error categories, redirects refused and key-path/body redaction (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T226 [US1] Make pass: T225 add `crates/infra-sms/Cargo.toml`, `crates/infra-sms/src/lib.rs`, `crates/infra-sms/src/kavenegar.rs`, workspace membership; sensitive approved-template payload only, no raw reqwest error formatting (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T227 [US1] Write failing test: [contract] [integration] `sms_sender_contract_sms_ir` in `crates/infra-sms/tests/sms_ir.rs`; run T149 unchanged via wiremock, assert POST `/v1/send/verify`, sensitive X-API-KEY, configured template/parameter name, redacted rejection/timeout and no redirects (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T228 [US1] Make pass: T227 implement `crates/infra-sms/src/sms_ir.rs`, infra-sms module index; failover remains application orchestration, no sender calls another adapter (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T229 [US1] Setup: prepare the real SQL/crypto/wiremock service harness in `crates/infra-postgres/tests/support/access.rs` and dev-dependencies in `crates/infra-postgres/Cargo.toml`; verify isolated runtime pools and usable synthetic signed proofs before any race assertion. Missing fixtures/tools block setup (FR-002, FR-024, FR-CON-02).
- [ ] T230 [US1] Acceptance test: [integration] `passkey_consumption_is_single_use_under_race` in `crates/infra-postgres/tests/integration_passkey_race.rs`; use the prepared access harness and real runtime pools to require the already fake-driven enforce final credential/state epoch and revision CAS before session/audit commit guarantee under concurrent SQL; a duplicate/old-epoch/unaudited success fails acceptance, not fixture setup (FR-002, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T231 [US1] Acceptance test: [integration] `otp_and_totp_consumption_are_single_use_under_race` in `crates/infra-postgres/tests/integration_totp_race.rs`; use the prepared access harness and real runtime pools to require the already fake-driven enforce final accepted-step CAS before session/audit commit guarantee under concurrent SQL; a duplicate/old-epoch/unaudited success fails acceptance, not fixture setup (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T232 [US1] Acceptance test: [integration] `recovery_races_with_login_safely` in `crates/infra-postgres/tests/integration_recovery_race.rs`; use the prepared access harness and real runtime pools to require the already fake-driven enforce target epoch/child-state revocation atomically against concurrent login guarantee under concurrent SQL; a duplicate/old-epoch/unaudited success fails acceptance, not fixture setup (FR-004, FR-CON-03, AC-ADM-043).
- [ ] T233 [US1] Acceptance test: [integration] `audit_failure_aborts_access` in `crates/infra-postgres/tests/integration_access_audit_failure.rs`; use the prepared access harness and real runtime pools to require the already fake-driven refuse full-session issuance if its required audit cannot commit guarantee under concurrent SQL; a duplicate/old-epoch/unaudited success fails acceptance, not fixture setup (FR-024, FR-CON-37, AC-ADM-025).

### HTML access and story composition

- [ ] T234 [US1] Write failing test: [http] `sign_in_page_is_localized_no_store_and_uses_safe_return_target` in `crates/web/tests/sign_in_page.rs`; expect GET /console/sign-in to omit PrepareAccessForm, CSRF/origin enforcement where mutating, localized validation/303 behavior, no-store or safe return handling (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T235 [US1] Make pass: T234 implement only GET /console/sign-in through PrepareAccessForm; add its form in crates/web/templates/console/sign_in.html, route registration in crates/web/src/console/mod.rs and matching FA/EN console Fluent keys; use no JSON auth endpoint in `crates/web/src/console/auth.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T236 [US1] Write failing test: [http] `access_forms_never_grant_protected_access_from_primary_only` in `crates/web/tests/request_code_form.rs`; expect POST /console/sign-in/request-code to omit RequestAdminOtp, CSRF/origin enforcement where mutating, localized validation/303 behavior, no-store or safe return handling (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T237 [US1] Make pass: T236 implement only POST /console/sign-in/request-code through RequestAdminOtp; add its form in crates/web/templates/console/sign_in.html, route registration in crates/web/src/console/mod.rs and matching FA/EN console Fluent keys; use no JSON auth endpoint in `crates/web/src/console/auth.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T238 [US1] Write failing test: [http] `verify_code_form_sets_restricted_proof_only` in `crates/web/tests/verify_code_form.rs`; expect POST /console/sign-in/verify-code to omit VerifyAdminOtp, CSRF/origin enforcement where mutating, localized validation/303 behavior, no-store or safe return handling (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T239 [US1] Make pass: T238 implement only POST /console/sign-in/verify-code through VerifyAdminOtp; add its form in crates/web/templates/console/second_factor.html, route registration in crates/web/src/console/mod.rs and matching FA/EN console Fluent keys; use no JSON auth endpoint in `crates/web/src/console/auth.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T240 [US1] Write failing test: [http] `passkey_begin_form_escapes_public_options_and_never_exposes_server_state` in `crates/web/tests/passkey_begin_form.rs`; expect POST /console/sign-in/passkey/begin to omit BeginPasskey, CSRF/origin enforcement where mutating, localized validation/303 behavior, no-store or safe return handling (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T241 [US1] Make pass: T240 implement only POST /console/sign-in/passkey/begin through BeginPasskey; add its form in crates/web/templates/console/second_factor.html, route registration in crates/web/src/console/mod.rs and matching FA/EN console Fluent keys; use no JSON auth endpoint in `crates/web/src/console/auth.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T242 [US1] Write failing test: [http] `passkey_finish_form_requires_current_bound_proof` in `crates/web/tests/passkey_finish_form.rs`; expect POST /console/sign-in/passkey/finish to omit FinishPasskey, CSRF/origin enforcement where mutating, localized validation/303 behavior, no-store or safe return handling (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T243 [US1] Make pass: T242 implement only POST /console/sign-in/passkey/finish through FinishPasskey; add its form in crates/web/templates/console/second_factor.html, route registration in crates/web/src/console/mod.rs and matching FA/EN console Fluent keys; use no JSON auth endpoint in `crates/web/src/console/auth.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T244 [US1] Write failing test: [http] `totp_form_requires_primary_and_second_factor` in `crates/web/tests/totp_form.rs`; expect POST /console/sign-in/totp to omit FinishTotp, CSRF/origin enforcement where mutating, localized validation/303 behavior, no-store or safe return handling (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T245 [US1] Make pass: T244 implement only POST /console/sign-in/totp through FinishTotp; add its form in crates/web/templates/console/second_factor.html, route registration in crates/web/src/console/mod.rs and matching FA/EN console Fluent keys; use no JSON auth endpoint in `crates/web/src/console/auth.rs`; edit only this behavior and its module registration (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T246 [US1] Write failing test: [http] `local_passkey_bridge_serializes_binary_fields_without_inline_execution` in `crates/web/tests/passkey_bridge.rs`; expect the locally served bridge/options markup to be missing, unsafe to interpolate or blocked by the strict CSP; valid public options only, no server ceremony serialization (FR-002, FR-029, FR-CON-02, FR-UX-08, AC-ADM-002, AC-ADM-030).
- [ ] T247 [US1] Make pass: T246 implement only the essential navigator.credentials bridge and safe form submission; browser acceptance later exercises real signatures under the same CSP in `crates/web/static/passkey.js`; edit only this behavior and its module registration (FR-002, FR-029, FR-CON-02, FR-UX-08, AC-ADM-002, AC-ADM-030).
- [ ] T248 [US1] Write failing test: [http] `admin_list_page_has_required_authorization_and_bound_state` in `crates/web/tests/admin_list_page.rs`; expect GET /console/admins to omit its ListAdmins authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T249 [US1] Make pass: T248 implement only GET /console/admins through ListAdmins; add its template fragment in crates/web/templates/console/admins.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/admins.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T250 [US1] Write failing test: [http] `admin_invite_form_has_required_authorization_and_bound_state` in `crates/web/tests/admin_invite_form.rs`; expect POST /console/admins/invite to omit its InviteAdmin authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T251 [US1] Make pass: T250 implement only POST /console/admins/invite through InviteAdmin; add its template fragment in crates/web/templates/console/admins.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/admins.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T252 [US1] Write failing test: [http] `admin_recover_form_has_required_authorization_and_bound_state` in `crates/web/tests/admin_recover_form.rs`; expect POST /console/admins/{id}/recover to omit its RecoverAdmin authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T253 [US1] Make pass: T252 implement only POST /console/admins/{id}/recover through RecoverAdmin; add its template fragment in crates/web/templates/console/admins.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/admins.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T254 [US1] Write failing test: [http] `enrollment_page_has_required_authorization_and_bound_state` in `crates/web/tests/enrollment_page.rs`; expect GET /console/access/enrol to omit its GetEnrollment authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T255 [US1] Make pass: T254 implement only GET /console/access/enrol through GetEnrollment; add its template fragment in crates/web/templates/console/enrollment.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/enrollment.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043). Delegate to the completed GetEnrollment service only; no port or repository is exposed to web.
- [ ] T256 [US1] Write failing test: [http] `enroll_passkey_begin_has_required_authorization_and_bound_state` in `crates/web/tests/enroll_passkey_begin.rs`; expect POST /console/access/enrol/passkey/begin to omit its BeginPasskeyEnrollment authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T257 [US1] Make pass: T256 implement only POST /console/access/enrol/passkey/begin through BeginPasskeyEnrollment; add its template fragment in crates/web/templates/console/enrollment.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/enrollment.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T258 [US1] Write failing test: [http] `enroll_passkey_finish_has_required_authorization_and_bound_state` in `crates/web/tests/enroll_passkey_finish.rs`; expect POST /console/access/enrol/passkey/finish to omit its FinishPasskeyEnrollment authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T259 [US1] Make pass: T258 implement only POST /console/access/enrol/passkey/finish through FinishPasskeyEnrollment; add its template fragment in crates/web/templates/console/enrollment.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/enrollment.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T260 [US1] Write failing test: [http] `enroll_totp_begin_has_required_authorization_and_bound_state` in `crates/web/tests/enroll_totp_begin.rs`; expect POST /console/access/enrol/totp/begin to omit its BeginTotpEnrollment authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043). Also drive `totp_provisioning_is_once_authenticated_and_no_store` over the existing POST: authorized enrolling target only, direct no-store HTML response, one winner and no redisplay via refresh/repeat/GET/error/redirect; no external QR/network delivery or logs and no other UI secret.
- [ ] T261 [US1] Make pass: T260 implement only POST /console/access/enrol/totp/begin through BeginTotpEnrollment; add its template fragment in crates/web/templates/console/enrollment.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/enrollment.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043). Render the original BeginTotpEnrollment result once in a direct Cache-Control: no-store response, not a reusable GET/redirect; preserve all C1 refusal/redaction assertions from T260.
- [ ] T262 [US1] Write failing test: [http] `enroll_totp_finish_has_required_authorization_and_bound_state` in `crates/web/tests/enroll_totp_finish.rs`; expect POST /console/access/enrol/totp/finish to omit its FinishTotpEnrollment authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T263 [US1] Make pass: T262 implement only POST /console/access/enrol/totp/finish through FinishTotpEnrollment; add its template fragment in crates/web/templates/console/enrollment.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/enrollment.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T264 [US1] Write failing test: [http] `sign_out_form_has_required_authorization_and_bound_state` in `crates/web/tests/sign_out_form.rs`; expect POST /console/sign-out to omit its SignOut authorization/binding, CSRF on POST, localized safe errors or no-store; secrets must not be reflected in errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T265 [US1] Make pass: T264 implement only POST /console/sign-out through SignOut; add its template fragment in crates/web/templates/console/sign_in.html, registration in crates/web/src/console/mod.rs and FA/EN console Fluent keys; no repository calls or automatic enrollment in `crates/web/src/console/auth.rs`; edit only this behavior and its module registration (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T266 [US1] Write failing test: [http] `production_docs_and_assets_require_admin` in `crates/web/tests/admin_guard.rs`; expect unprotected console, Swagger redirects/assets or schema, end-user-cookie substitution and stale recovered-session access (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-043).
- [ ] T267 [US1] Make pass: T266 implement application-service auth middleware in `crates/web/src/console/guard.rs`, `crates/web/src/api/docs.rs`, `crates/web/src/api/openapi.rs`, console index; document JSON auth errors and regenerate `docs/openapi.json` (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-043).
- [ ] T268 [US1] Write failing test: [http] `no_js_totp_and_no_htmx_access_have_equivalent_security` in `crates/web/tests/auth_progressive.rs`; expect missing full-page fallback, hidden primary/factor bypass or inaccessible passkey-disabled explanation (FR-002, FR-029, FR-031, FR-UX-08, AC-ADM-002, AC-ADM-030, AC-ADM-032).
- [ ] T269 [US1] Make pass: T268 complete full-page no-JS TOTP/enrollment navigation and accessible fallback messages in `crates/web/src/console/auth.rs`, `crates/web/src/console/enrollment.rs`, access templates and both console Fluent files; no automatic fallback enrollment (FR-002, FR-029, FR-031, FR-UX-08, AC-ADM-002, AC-ADM-030, AC-ADM-032).
- [ ] T270 [US1] Write failing test: [integration] `access_composition_selects_all_ports_from_one_backend_group` in `crates/app/tests/integration_access_composition.rs`; expect missing real/fake configuration wiring, incomplete debug fake catalog or production startup accepting a fake (FR-001, FR-002, FR-004, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-043).
- [ ] T271 [US1] Make pass: T270 implement `crates/app/src/composition.rs`, `crates/app/src/serve.rs`, `crates/app/src/main.rs`, app dependencies; real adapters only in normal production path, debug-only testkit selection, same shared UoW/store, no database connection in fake mode (FR-001, FR-002, FR-004, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-043).
- [ ] T272 [US1] Write failing test: [integration] `initial_provisioning_is_operator_authorized_not_public_registration` in `crates/app/tests/integration_provision_admin.rs`; expect missing protected CLI parsing/config-based operator context or delegation to BootstrapAdmin, unsafe secrets in CLI/logs or an exposed alternate write path; use the completed service in real/fake composition, not CLI-owned repository orchestration (FR-004, FR-CON-03, AC-ADM-004, AC-ADM-006).
- [ ] T273 [US1] Make pass: T272 implement only CLI parsing, protected configuration and BootstrapAdmin delegation for the operator provisioning command in `crates/app/src/provision_admin.rs`, `crates/app/src/main.rs`; wire the completed BootstrapAdmin service; no eligibility, authorization, repository, UoW or audit business logic in app, no direct storage access or recovery bypass (FR-004, FR-CON-03, AC-ADM-004, AC-ADM-006).
- [ ] T274 [US1] Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Confirm AC-ADM-001–007/043 on fakes AND real adapters, record only observed results in `specs/001-admin-foundation/tasks.md` (FR-CON-01, FR-CON-02, FR-CON-03, FR-CON-37).

## Phase 4: User Story 2 — Inspect and Safely Prepare Settings (P1)

**Goal**: Ten bounded settings, exact draft/preview, honest impact/preservation rules, no
live mutation before review. Flags share subject storage here; enablement remains US6.
**Independent test**: AC-ADM-008–015/037–042/046, including OI8, against fake and real storage.

### Pure setting, flag and review domain

- [ ] T275 [US2] Write failing test: [unit] `initial_settings_have_decided_values_and_complete_metadata`, `settings_accept_inclusive_bounds_without_changing_defaults`, `setting_rejects_values_outside_declared_policy` in `crates/admin/src/domain/settings.rs`; proptest all nine default/min/max triples: buyer_temp_photo_days 14/1/30, studio_temp_upload_days 30/7/90, studio_auto_draft_days 30/1/90, account_recovery_days 30/14/90, studio_max_cards 50/1/100, buyer_max_permanent_photos 5/1/10, upload_max_mb 15/1/25, upload_min_shortest_edge_px 512/256/1024, generation_max_attempts 3/1/3; expect missing registry, wrong-kind/outside/overflow acceptance or absent metadata (FR-006, FR-007, FR-CON-32, P41, AC-ADM-008, AC-ADM-009, AC-ADM-037).
- [ ] T276 [US2] Make pass: T275 implement Setting<T>, stable registry, units/FA-EN label keys/live-version references and checked inclusive finite numeric policy in `crates/admin/src/domain/settings.rs`, domain index; no float values or default rewrites (FR-006, FR-007, FR-CON-32, P41, AC-ADM-008, AC-ADM-009, AC-ADM-037).
- [ ] T277 [US2] Write failing test: [unit] `upload_format_policy_requires_jpeg_and_known_formats` in `crates/admin/src/domain/formats.rs`; enumerate every subset/unknown format and expect missing JPEG-mandatory sorted set; quote policy "valid masks are 1..15 with bit 1 set" (FR-032, FR-CON-32, AC-ADM-038, Q05).
- [ ] T278 [US2] Make pass: T277 implement FormatSet and tenth setting default JPEG/PNG/WebP/HEIC in `crates/admin/src/domain/formats.rs`, `crates/admin/src/domain/settings.rs`, domain index; only nonempty subsets containing JPEG (FR-032, FR-CON-32, AC-ADM-038, Q05).
- [ ] T279 [US2] Write failing test: [unit] `six_launch_flags_start_off` in `crates/admin/src/domain/flag.rs`; expect the canonical FlagKey/FlagDefinition registry to be absent or accept missing/extra/enabled initial keys: partner_widget_api, percentage_rollout, per_user_usage_limits, ticketing, external_notifications, paid_share_billing (FR-017, FR-018, FR-CON-32, AC-ADM-033).
- [ ] T280 [US2] Make pass: T279 define FlagKey/FlagDefinition and the typed canonical six-key OFF registry with managed label/description keys once; its boolean Flag value DTO preserves subject/version identity, and every later contract/SeedFoundation reads this registry in `crates/admin/src/domain/flag.rs`; edit only this behavior and its module registration (FR-017, FR-018, FR-CON-32, AC-ADM-033).
- [ ] T281 [US2] Write failing test: [unit] `single_subject_revision_is_immutable_and_head_cas_is_explicit` in `crates/admin/src/domain/change_revision.rs`; expect a draft with both/neither setting/flag subject or stale head to overwrite state (FR-008, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-015). In the same pure driver, drive `schedule_time_rejects_ambiguous_nonexistent_or_past` and `json_schedule_time_requires_explicit_offset` in `crates/admin/src/domain/scheduled_time.rs` before SaveDraft consumes the value: Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input.
- [ ] T282 [US2] Make pass: T281 implement only ChangeSubject/ChangeDraft/immutable ChangeRevision and head-CAS transitions in `crates/admin/src/domain/change_revision.rs`; edit only this behavior and its module registration (FR-008, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-015). Implement the preceding pure scheduled-time value/parser in `crates/admin/src/domain/scheduled_time.rs`, register the domain index and pure embedded-zone dependencies in `crates/admin/Cargo.toml`; no filesystem, external service or direct system-clock access. Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input.
- [ ] T283 [US2] Write failing test: [unit] `draft_hash_is_canonical` in `crates/admin/src/domain/change_hash.rs`; expect equivalent sorted format sets/UTC instants to hash differently or changed content/time to share identity (FR-012, FR-CON-07, AC-ADM-014). Assert equivalent explicit-offset instants canonicalize to the same UTC value while the reviewed display-zone/timing identity stays bound; local-time rejection precedes hashing.
- [ ] T284 [US2] Make pass: T283 implement canonical typed revision digest only; hash is not the primary revision identity in `crates/admin/src/domain/change_hash.rs`; edit only this behavior and its module registration (FR-012, FR-CON-07, AC-ADM-014).
- [ ] T285 [US2] Write failing test: [unit] `preview_identity_binds_revision_subject_and_baseline` in `crates/admin/src/domain/change_preview.rs`; expect a preview to accept mismatched revision/subject/baseline or mutate live state (FR-009, FR-012, FR-CON-06, AC-ADM-011, AC-ADM-014). Also drive typed impact report/provenance values: exact revision/subject/baseline/policy/source/time and known-positive versus known-empty versus unavailable; unavailable is never zero. No I/O, population query or lifecycle effects. Comparison and individual policy calculations are driven in their own following pairs, not prerequisites for this report type.
- [ ] T286 [US2] Make pass: T285 implement immutable exact-version PreviewReport/provenance values only in `crates/admin/src/domain/change_preview.rs`; edit only this behavior and its module registration (FR-009, FR-012, FR-CON-06, AC-ADM-011, AC-ADM-014). Use direct pure domain calculation on typed evidence, retain the report/provenance distinctions from T285; comparison and policy calculations follow in their own pairs, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T287 [US2] Write failing test: [unit] `editing_approved_draft_invalidates_approval` in `crates/admin/src/domain/change_approval.rs`; expect an edited value/time or reverted value to reuse old approval (FR-012, FR-CON-07, AC-ADM-014, AC-ADM-044). In the same domain test also run `impact_comparison_ignores_assessment_time`, `changed_destructive_impact_requires_reapproval` and `unavailable_impact_blocks_review`; compare the canonical fields in application-ports §4, including unchanged counts at later assessed_at, changed counts/effect/source, unknown versus known zero and recovery to identical evidence without a TTL. Use direct pure report values for known-positive/known-empty/unavailable and all canonical provenance comparisons; no policy-port fake or real wrapper.
- [ ] T288 [US2] Make pass: T287 implement exact preview/revision/baseline Approval eligibility and the pure comparable-impact decision from application-ports §4; timestamps/report IDs alone preserve eligibility, changed comparable evidence requires new preview/approval, unavailable required evidence blocks; old records stay immutable in `crates/admin/src/domain/change_approval.rs`; edit only this behavior and its module registration (FR-012, FR-CON-07, AC-ADM-014, AC-ADM-044). Use direct pure domain calculation on typed evidence, retain all provenance/comparison/preservation cases from the preceding driver, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T289 [US2] Write failing test: [unit] `shorter_lifetime_previews_existing_item_impact` in `crates/admin/src/domain/impact_lifetime.rs`; expect missing original upload/last-activity reference, changed/already-due counts, equality-boundary and unknown≠zero semantics; no deletion while previewing (FR-010, FR-CON-32, P41, AC-ADM-012). For this policy, include typed provenance and known-positive/known-empty/unavailable cases where population counts apply, original-reference/preservation/OI8 assertions as applicable, and T287 canonical comparison without a TTL. Use test-only typed snapshots, never fabricated production population evidence or owning-lifecycle effects.
- [ ] T290 [US2] Make pass: T289 implement pure lifetime-impact calculation in `crates/admin/src/domain/impact_lifetime.rs`, domain index; typed snapshots/provenance, next-sweep explanation, no downstream expiry executor (FR-010, FR-CON-32, P41, AC-ADM-012). Use direct pure domain calculation on typed evidence, retain all provenance/comparison/preservation cases from the preceding driver, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T291 [US2] Write failing test: [unit] `card_limit_preview_preserves_existing_projects` in `crates/admin/src/domain/impact_cards.rs`; expect missing keep-existing/confirmed-unaffected rule, creation permitted at limit but no addition beyond it (FR-033, FR-CON-32, P41, AC-ADM-039, Q15). For this policy, include typed provenance and known-positive/known-empty/unavailable cases where population counts apply, original-reference/preservation/OI8 assertions as applicable, and T287 canonical comparison without a TTL. Use test-only typed snapshots, never fabricated production population evidence or owning-lifecycle effects.
- [ ] T292 [US2] Make pass: T291 implement card-preservation impact DTO/policy in `crates/admin/src/domain/impact_cards.rs`, domain index; no Studio project mutation (FR-033, FR-CON-32, P41, AC-ADM-039, Q15). Use direct pure domain calculation on typed evidence, retain all provenance/comparison/preservation cases from the preceding driver, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T293 [US2] Write failing test: [unit] `photo_limit_preview_preserves_existing_photos` in `crates/admin/src/domain/impact_photos.rs`; expect deletion or add-permitted at/above limit instead of keep-all/add-only-below (FR-034, FR-CON-32, P41, AC-ADM-040, Q40). For this policy, include typed provenance and known-positive/known-empty/unavailable cases where population counts apply, original-reference/preservation/OI8 assertions as applicable, and T287 canonical comparison without a TTL. Use test-only typed snapshots, never fabricated production population evidence or owning-lifecycle effects.
- [ ] T294 [US2] Make pass: T293 implement photo-preservation policy in `crates/admin/src/domain/impact_photos.rs`, domain index; no media or account-lifecycle implementation (FR-034, FR-CON-32, P41, AC-ADM-040, Q40). Use direct pure domain calculation on typed evidence, retain all provenance/comparison/preservation cases from the preceding driver, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T295 [US2] Write failing test: [unit] `upload_policy_preview_applies_only_to_new_uploads` in `crates/admin/src/domain/impact_uploads.rs`; expect stored-file revalidation/removal after size/format/shortest-edge changes (FR-035, FR-CON-32, P41, AC-ADM-041, Q05). For this policy, include typed provenance and known-positive/known-empty/unavailable cases where population counts apply, original-reference/preservation/OI8 assertions as applicable, and T287 canonical comparison without a TTL. Use test-only typed snapshots, never fabricated production population evidence or owning-lifecycle effects.
- [ ] T296 [US2] Make pass: T295 implement new-upload-only policy in `crates/admin/src/domain/impact_uploads.rs`, domain index; preserve stored files (FR-035, FR-CON-32, P41, AC-ADM-041, Q05). Use direct pure domain calculation on typed evidence, retain all provenance/comparison/preservation cases from the preceding driver, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T297 [US2] Write failing test: [unit] `attempt_limit_preview_preserves_existing_units` in `crates/admin/src/domain/impact_attempts.rs`; expect queued/processing allowance rewrite or more than three total attempts (FR-036, FR-CON-32, P41, AC-ADM-042, Q29). For this policy, include typed provenance and known-positive/known-empty/unavailable cases where population counts apply, original-reference/preservation/OI8 assertions as applicable, and T287 canonical comparison without a TTL. Use test-only typed snapshots, never fabricated production population evidence or owning-lifecycle effects.
- [ ] T298 [US2] Make pass: T297 implement future-queued-only attempt policy in `crates/admin/src/domain/impact_attempts.rs`, domain index; no generation/job executor (FR-036, FR-CON-32, P41, AC-ADM-042, Q29). Use direct pure domain calculation on typed evidence, retain all provenance/comparison/preservation cases from the preceding driver, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T299 [US2] Write failing test: [unit] `recovery_window_decrease_keeps_pending_end_dates` in `crates/admin/src/domain/impact_recovery.rs`; expect a pending account's shown end date D to move after 30→14 while new requests use 14 (FR-010, FR-CON-32, P41, AC-ADM-046, OI8). For this policy, include typed provenance and known-positive/known-empty/unavailable cases where population counts apply, original-reference/preservation/OI8 assertions as applicable, and T287 canonical comparison without a TTL. Use test-only typed snapshots, never fabricated production population evidence or owning-lifecycle effects.
- [ ] T300 [US2] Make pass: T299 implement new-deletion-request-only recovery policy in `crates/admin/src/domain/impact_recovery.rs`, domain index; existing D unchanged, no deletion scheduling code (FR-010, FR-CON-32, P41, AC-ADM-046, OI8). Use direct pure domain calculation on typed evidence, retain all provenance/comparison/preservation cases from the preceding driver, and report absent authoritative population evidence as unavailable; no policy port, adapter or production fixture counts.
- [ ] T301 [US2] Write failing test: [unit] `flag_enablement_requires_delivered_passing_evidence` in `crates/admin/src/domain/flag_readiness.rs`; expect fail/unknown/not-delivered/stale evidence accepted, urgent bypass or readiness override in draft input (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036). Include delivered+pass, failed, unknown, not-delivered and stale revision/baseline/source/policy/time provenance cases; code-owned registry has all six capabilities not delivered. Registration alone is not delivery, no mutable admin override or implicit healthy default. Typed passing evidence fixtures exercise policy only, not production delivery. This domain prerequisite moves before Preview and US3 consumers.
- [ ] T302 [US2] Make pass: T301 implement readiness eligibility in `crates/admin/src/domain/flag_readiness.rs`, `crates/admin/src/domain/mod.rs`; readiness is a read-only fact, disabling still follows reviewed workflow (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036). Use direct pure CapabilityReadiness calculation and code-owned delivered-capability registry, all six not delivered; expose no port or real/fake adapter. Unknown evidence remains explicit.
- [ ] T303 [US2] Write failing test: [unit] `settings_cannot_weaken_security_or_financial_integrity` in `crates/admin/src/domain/protected_policy.rs`; expect arbitrary keys/security-off/audit-off/financial-bypass values to be accepted; name invariants GR1–GR9 without claiming their owning features are delivered (FR-011, FR-CON-32, AC-ADM-013, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).
- [ ] T304 [US2] Make pass: T303 implement fixed editable registry/security boundary in `crates/admin/src/domain/protected_policy.rs`, domain index; reject bypass keys regardless of admin/urgency (FR-011, FR-CON-32, AC-ADM-013, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).

### Shared management ports and fakes

- [ ] T305 [US2] Write failing test: [unit] `settings_repository_port_is_mockable` in `crates/admin/src/ports/settings_repository.rs`; expect missing list/live/history/lock/append-version/live-pointer-CAS/seed methods (FR-006, FR-007, FR-CON-32).
- [ ] T306 [US2] Make pass: T305 define SettingsRepository in `crates/admin/src/ports/settings_repository.rs`, port index; explicit TxHandle and immutable definition policy (FR-006, FR-007, FR-CON-32).
- [ ] T307 [US2] Write failing test: [contract] `settings_repository_contract` in `crates/testkit/src/contracts/settings_repository.rs`, runner `crates/testkit/tests/settings_repository_fake.rs`; expect missing ten-key metadata, numeric/format bounds, matching version identity, immutable history and CAS/no-op seed retry (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038).
- [ ] T308 [US2] Make pass: T307 implement `crates/testkit/src/fakes/settings_repository.rs`, shared store/index wiring; reject wrong-store transactions and invalid values without effects (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038).
- [ ] T309 [US2] Write failing test: [unit] `flag_repository_port_is_mockable` in `crates/admin/src/ports/flag_repository.rs`; expect missing six-key list/live/history/lock/append/CAS/seed operations needed by shared subject workflow (FR-017, FR-018, FR-CON-32).
- [ ] T310 [US2] Make pass: T309 define FlagRepository in `crates/admin/src/ports/flag_repository.rs`, port index; no capability-health override operation (FR-017, FR-018, FR-CON-32).
- [ ] T311 [US2] Write failing test: [contract] `flag_repository_contract` in `crates/testkit/src/contracts/flag_repository.rs`, runner `crates/testkit/tests/flag_repository_fake.rs`; expect absent six OFF seed identities, immutable history, matching subject live pointer and stale-CAS refusal (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T312 [US2] Make pass: T311 implement `crates/testkit/src/fakes/flag_repository.rs`, shared store/index wiring; registration does not assert delivery/readiness (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T313 [US2] Write failing test: [unit] `change_repository_port_is_mockable` in `crates/admin/src/ports/change_repository.rs`; expect missing head/revision CAS, immutable append, preview/approval persist/read (FR-008, FR-012, FR-016, FR-CON-05).
- [ ] T314 [US2] Make pass: T313 define ChangeRepository in `crates/admin/src/ports/change_repository.rs`, port index; exact revision/subject/baseline typed identity (FR-008, FR-012, FR-016, FR-CON-05).
- [ ] T315 [US2] Write failing test: [contract] `change_repository_contract` in `crates/testkit/src/contracts/change_repository.rs`, runner `crates/testkit/tests/change_repository_fake.rs`; expect missing stale-head conflict, immutable revision/review, content/time invalidation and same-subject baseline checks (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T316 [US2] Make pass: T315 implement `crates/testkit/src/fakes/change_repository.rs`, shared store/index wiring; old approval records remain immutable but lose eligibility (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).

### Fake-first management services

- [ ] T317 [US2] Write failing test: [mock] `seed_records_defaults_flags_and_audit_once` in `crates/testkit/tests/mock_seed_foundation.rs`; expect missing one-UoW/operation-key/id/time/one-audit ordering and duplicate writes on rerun (FR-006, FR-017, FR-021, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T318 [US2] Write failing test: [fake] `seed_rerun_preserves_published_values` in `crates/testkit/tests/fake_seed_foundation.rs`; expect missing ten exact defaults, six exact OFF flags or reseeding to overwrite a changed value; no privileged admin seed (FR-006, FR-017, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T319 [US2] Make pass: T317 T318 implement SeedFoundation in `crates/admin/src/application/seed_foundation.rs`, application index; keys from data-model §2, baseline versions+system audit atomically, idempotent stable seed operation (FR-006, FR-017, FR-021, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T320 [US2] Write failing test: [mock] `initial_settings_have_decided_values_and_complete_metadata_port_interactions` in `crates/testkit/tests/mock_list_settings.rs`; expect current-admin authorization not to precede SettingsRepository bounded definition/live read, repeated/unbounded reads, or any write (FR-006, FR-CON-32, AC-ADM-008).
- [ ] T321 [US2] Write failing test: [fake] `initial_settings_have_decided_values_and_complete_metadata` in `crates/testkit/tests/fake_list_settings.rs`; expect missing ten-key unit/label/range/effective-time/history DTOs; confirm valid actor isolation and zero state changes (FR-006, FR-CON-32, AC-ADM-008).
- [ ] T322 [US2] Make pass: T320 T321 implement only list_settings with owned presentation DTOs and typed missing-ID/key errors in `crates/admin/src/application/list_settings.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-006, FR-CON-32, AC-ADM-008).
- [ ] T323 [US2] Write failing test: [mock] `flag_registry_queries_return_safe_bounded_metadata_port_interactions` in `crates/testkit/tests/mock_list_flags.rs`; expect current-admin authorization not to precede FlagRepository bounded registry read, repeated/unbounded reads, or any write (FR-018, FR-CON-32, AC-ADM-034).
- [ ] T324 [US2] Write failing test: [fake] `flag_registry_queries_return_safe_bounded_metadata` in `crates/testkit/tests/fake_list_flags.rs`; expect missing canonical six-key state/descriptions/history without invented readiness; confirm valid actor isolation and zero state changes (FR-018, FR-CON-32, AC-ADM-034).
- [ ] T325 [US2] Make pass: T323 T324 implement only list_flags with owned presentation DTOs and typed missing-ID/key errors in `crates/admin/src/application/list_flags.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-018, FR-CON-32, AC-ADM-034).
- [ ] T326 [US2] Write failing test: [mock] `get_setting_requires_exact_known_key_port_interactions` in `crates/testkit/tests/mock_get_setting.rs`; expect current-admin authorization not to precede SettingsRepository exact-key live/history reads, repeated/unbounded reads, or any write (FR-006, FR-007, FR-CON-32, AC-ADM-008, AC-ADM-009).
- [ ] T327 [US2] Write failing test: [fake] `get_setting_requires_exact_known_key` in `crates/testkit/tests/fake_get_setting.rs`; expect unknown key accepted or a different subject/history returned; confirm valid actor isolation and zero state changes (FR-006, FR-007, FR-CON-32, AC-ADM-008, AC-ADM-009).
- [ ] T328 [US2] Make pass: T326 T327 implement only get_setting with owned presentation DTOs and typed missing-ID/key errors in `crates/admin/src/application/get_setting.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-006, FR-007, FR-CON-32, AC-ADM-008, AC-ADM-009).
- [ ] T329 [US2] Write failing test: [mock] `get_version_requires_exact_subject_version_identity_port_interactions` in `crates/testkit/tests/mock_get_version.rs`; expect current-admin authorization not to precede SettingsRepository/FlagRepository exact typed-version read, repeated/unbounded reads, or any write (FR-006, FR-014, FR-CON-08, AC-ADM-008, AC-ADM-019).
- [ ] T330 [US2] Write failing test: [fake] `get_version_requires_exact_subject_version_identity` in `crates/testkit/tests/fake_get_version.rs`; expect missing/wrong-subject version accepted or historical actor/value lost; confirm valid actor isolation and zero state changes (FR-006, FR-014, FR-CON-08, AC-ADM-008, AC-ADM-019).
- [ ] T331 [US2] Make pass: T329 T330 implement only get_version with owned presentation DTOs and typed missing-ID/key errors in `crates/admin/src/application/get_version.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-006, FR-014, FR-CON-08, AC-ADM-008, AC-ADM-019).
- [ ] T332 [US2] Write failing test: [mock] `get_change_returns_exact_head_review_and_baseline_port_interactions` in `crates/testkit/tests/mock_get_change.rs`; expect current-admin authorization not to precede ChangeRepository one exact-ID read, repeated/unbounded reads, or any write (FR-009, FR-012, FR-CON-06, AC-ADM-011, AC-ADM-014).
- [ ] T333 [US2] Write failing test: [fake] `get_change_returns_exact_head_review_and_baseline` in `crates/testkit/tests/fake_get_change.rs`; expect unknown ID accepted or wrong draft/current head/review/baseline returned; confirm valid actor isolation and zero state changes (FR-009, FR-012, FR-CON-06, AC-ADM-011, AC-ADM-014).
- [ ] T334 [US2] Make pass: T332 T333 implement only get_change with owned presentation DTOs and typed missing-ID/key errors in `crates/admin/src/application/get_change.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-009, FR-012, FR-CON-06, AC-ADM-011, AC-ADM-014).
- [ ] T335 [US2] Write failing test: [mock] `validation_does_not_invoke_publish_or_vendor` in `crates/testkit/tests/mock_save_draft.rs`; expect missing invalid-policy/stale-head checks before revision/audit writes, or ID/time generation bypassing ports (FR-007, FR-008, FR-011, FR-012, FR-016, FR-CON-05, AC-ADM-009, AC-ADM-013, AC-ADM-014, AC-ADM-015). Also drive `audit_draft_has_complete_safe_fields_once` in this original service driver: expect the draft revision save event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025). Include the existing scheduled-time input rules from T281: Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input.
- [ ] T336 [US2] Write failing test: [fake] `saving_setting_creates_draft_only`, `editing_approved_draft_invalidates_approval`, `concurrent_setting_edits_do_not_silently_overwrite` in `crates/testkit/tests/fake_save_draft.rs`; expect missing draft-only state, CAS conflict or retained approval after content/time edit (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015). Also drive `audit_draft_has_complete_safe_fields_once` in this original service driver: expect the draft revision save event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025). Include the existing scheduled-time input rules from T281: Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input.
- [ ] T337 [US2] Make pass: T335 T336 implement SaveDraft in `crates/admin/src/application/save_draft.rs`, application index; single subject, one revision+audit, no live-pointer mutation, preserve original input at presentation boundary (FR-007, FR-008, FR-011, FR-012, FR-016, FR-CON-05, AC-ADM-009, AC-ADM-010, AC-ADM-013, AC-ADM-014, AC-ADM-015). Complete the typed allowlisted `audit_draft_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework. Reuse the pure T282 scheduled-time value/parser and explicit Clock boundary checks; reject new/revised past or ambiguous/nonexistent input while preserving durable overdue semantics, UTC/display-zone storage and existing exact-review invalidation.
- [ ] T338 [US2] Write failing test: [mock] `preview_reads_exact_revision_assesses_once_and_never_executes_effects` in `crates/testkit/tests/mock_preview.rs`; expect missing revision/baseline assessment, fresh Clock provenance, one preview/audit write, or lifecycle/vendor/financial mutation while locked (FR-009, FR-010, FR-CON-06, FR-CON-09, P41, AC-ADM-011, AC-ADM-012). Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Also drive `audit_preview_has_complete_safe_fields_once` in this original service driver: expect the exact-version preview event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T339 [US2] Write failing test: [fake] `preview_uses_exact_draft_without_live_effects` in `crates/testkit/tests/fake_preview.rs`; expect missing immutable report, known/unknown impact, original reference/next-sweep and OI8/preservation explanations, or live mutation (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046). Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Also drive `audit_preview_has_complete_safe_fields_once` in this original service driver: expect the exact-version preview event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T340 [US2] Make pass: T338 T339 implement Preview in `crates/admin/src/application/preview.rs`, application index; gather evidence outside transaction, verify exact baseline/head on persistence, unknown stays unknown and blocks later approval/publication (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046). Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Complete the typed allowlisted `audit_preview_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.

### Actual registry storage and health adapter

- [ ] T341 [US2] Write failing test: [integration] `registry_schema_rejects_invalid_values_and_cross_subject_pointers` in `crates/infra-postgres/tests/integration_settings_schema.rs`; expect missing enforcement of settings: "key PK; fixed initial key/policy checks; non-null live pointer after seed transaction"; setting_versions: "exactly one typed value", per-key finite CHECK/JPEG mask, "UNIQUE(publication_id, setting_key)" and matching deferrable subject/version FKs (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-037, AC-ADM-038).
- [ ] T342 [US2] Make pass: T341 create settings and setting_versions with only these typed-value and pointer constraints in `crates/infra-postgres/migrations/0003_settings.sql`; edit only this behavior and its module registration (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-037, AC-ADM-038).
- [ ] T343 [US2] Write failing test: [integration] `flag_schema_rejects_unknown_keys_and_cross_subject_versions` in `crates/infra-postgres/tests/integration_flags_schema.rs`; expect missing enforcement of flags: "six fixed keys"; flag_versions: "UNIQUE(publication_id, flag_key)"; matching boolean typed live-pointer FKs (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T344 [US2] Make pass: T343 create flags and flag_versions with canonical membership and matching deferrable live-pointer constraints in `crates/infra-postgres/migrations/0004_flags.sql`; edit only this behavior and its module registration (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T345 [US2] Write failing test: [integration] `runtime_cannot_rewrite_registry_policy_or_version_history` in `crates/infra-postgres/tests/integration_registry_immutability.rs`; expect missing enforcement of runtime cannot alter registry policy or UPDATE/DELETE immutable published histories; no cascading history mutation (FR-006, FR-023, FR-CON-32, AC-ADM-008, AC-ADM-023).
- [ ] T346 [US2] Make pass: T345 grant runtime only intended pointer/draft operations and protect registry policy/history immutability using existing guard primitives in `crates/infra-postgres/migrations/0005_registry_immutability.sql`; edit only this behavior and its module registration (FR-006, FR-023, FR-CON-32, AC-ADM-008, AC-ADM-023).
- [ ] T347 [US2] Write failing test: [integration] `review_schema_enforces_immutable_exact_version_links` in `crates/infra-postgres/tests/integration_review_schema.rs`; expect absent drafts "CHECK exactly one subject"/"CAS on edit_counter"; revisions "UNIQUE(change_id, revision_number); immutable content"; previews "FK exact revision and matching subject baseline"; approvals "FK exact revision/preview/admin" and same-revision/subject checks (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015). Provide the exact approval/approver matching key for the later publication composite FK; retain immutable review records.
- [ ] T348 [US2] Make pass: T347 add `crates/infra-postgres/migrations/0006_change_review.sql`; typed baseline XOR checks, immutable revision/preview/approval guards, subject/status indexes and CAS head, no unchecked polymorphic identity (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015). Provide the exact approval/approver matching key for the later publication composite FK; retain immutable review records.
- [ ] T349 [US2] Write failing test: [contract] [integration] `settings_repository_contract_postgres` in `crates/infra-postgres/tests/integration_settings_repository.rs`; run T307 unchanged, expect missing SQL methods and runtime-policy enforcement (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038).
- [ ] T350 [US2] Make pass: T349 implement `crates/infra-postgres/src/settings_repository.rs`, adapter index; matching live pointers, bounded history, caller UoW, immutable policy and insert-if-unseeded semantics (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.
- [ ] T351 [US2] Write failing test: [contract] [integration] `flag_repository_contract_postgres` in `crates/infra-postgres/tests/integration_flag_repository.rs`; run T311 unchanged, expect missing six-key OFF registry/CAS/history (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T352 [US2] Make pass: T351 implement `crates/infra-postgres/src/flag_repository.rs`, adapter index; boolean state and history only, not fabricated delivery (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.
- [ ] T353 [US2] Write failing test: [contract] [integration] `change_repository_contract_postgres` in `crates/infra-postgres/tests/integration_change_repository.rs`; run T315 unchanged, race stale saves and expect missing exact-baseline/CAS protection (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T354 [US2] Make pass: T353 implement `crates/infra-postgres/src/change_repository.rs`, adapter index; immutable append and conditional head update in caller transaction (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.
- [ ] T355 [US2] Write failing test: [contract] [integration] `health_probe_contract_postgres` in `crates/infra-postgres/tests/integration_health_probe.rs`; run T081 unchanged against actual missing/unseeded/seeded registries and unavailable connection; expect absent real probe (FR-001, FR-006, FR-017, FR-CON-01).
- [ ] T356 [US2] Make pass: T355 implement `crates/infra-postgres/src/health_probe.rs`, adapter index; ready only when the genuine migrated seed is available, otherwise typed unavailable without leaking infrastructure (FR-001, FR-006, FR-017, FR-CON-01). Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red.
- [ ] T357 [US2] Setup: prepare the real seed/draft service fixture in `crates/infra-postgres/tests/support/settings.rs`, reusing migrated runtime-role pools and already completed services. Missing harness/credentials block setup; do not modify admin services to prepare a test (FR-006, FR-008, FR-016, FR-024, FR-CON-32, AC-ADM-008, AC-ADM-010, AC-ADM-015, AC-ADM-025).
- [ ] T358 [US2] Acceptance test: [integration] `registry_seed_and_draft_commit_with_audit_or_not_at_all` in `crates/infra-postgres/tests/integration_settings_services.rs`; require the fake-driven seed rerun protection, one-winner stale save and audit-failure rollback using the prepared real fixture. A discovered behavior failure needs a focused port/service driver before correction, not an unscoped multi-crate repair (FR-006, FR-008, FR-016, FR-024, FR-CON-32, AC-ADM-008, AC-ADM-010, AC-ADM-015, AC-ADM-025).

### Settings/preview HTML and composition

- [ ] T359 [US2] Write failing test: [http] `settings_forms_preserve_input_and_report_bounds_and_conflicts` in `crates/web/tests/settings.rs`; expect absent ten-key list/detail/draft form routes, localized 422/error labels, 409 stale conflict, metadata/history and draft-only 303 flow (FR-006, FR-007, FR-008, FR-016, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-009, AC-ADM-010, AC-ADM-015, AC-ADM-037, AC-ADM-038).
- [ ] T360 [US2] Make pass: T359 implement `crates/web/src/console/settings.rs`, console index, `crates/web/templates/console/settings.html`, `crates/web/templates/console/setting.html`, `crates/web/templates/partials/setting_row.html` and setting keys in both FA/EN console Fluent files; no arbitrary registry policy editing (FR-006, FR-007, FR-008, FR-016, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-009, AC-ADM-010, AC-ADM-015, AC-ADM-037, AC-ADM-038).
- [ ] T361 [US2] Write failing test: [http] `preview_views_explain_every_preservation_policy_and_unknown_count` in `crates/web/tests/preview.rs`; expect absent exact draft/live display, viewport preview selector, positive/unknown/next-sweep and OI8 explanations in FA/EN (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).
- [ ] T362 [US2] Make pass: T361 implement `crates/web/src/console/changes.rs`, console index, `crates/web/templates/console/change.html`, `crates/web/templates/partials/impact.html` and both console Fluent files; no deletion/financial side effects, preserve revision and safe valid input (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).
- [ ] T363 [US2] Write failing test: [integration] `foundation_seed_command_updates_health_and_is_idempotent` in `crates/app/tests/integration_seed_foundation.rs`; expect absent migrate/seed commands, wrong migration/runtime role, seed rerun overwrite or fake mode contacting storage (FR-006, FR-017, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T364 [US2] Make pass: T363 implement `crates/app/src/migrate.rs`, `crates/app/src/seed_foundation.rs`, `crates/app/src/main.rs`, `crates/app/src/composition.rs`; wire registry/impact/health adapters by config, run seed through application UoW, keep deployment secrets out of arguments/logs (FR-006, FR-017, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T365 [US2] Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Confirm AC-ADM-008–015/037–042 on fakes AND real adapters; AC-ADM-046 is domain/preview-only here, with publication-policy acceptance in US3 in `specs/001-admin-foundation/tasks.md`; lifetime real counts may honestly remain unavailable until owning contexts exist (FR-CON-32, FR-CON-37, P41).

## Phase 5: User Story 3 — Publish, Schedule and Roll Back Changes (P1)

**Goal**: Exact reviewed revision becomes live once at its authorised time; durable blocked
state, ABA-safe baselines, overdue rechecks and history-preserving rollback.
**Independent test**: AC-ADM-016–020/044–045, fake Clock first, then real concurrent SQL/worker.

### Publication domain and new ports

**U3 — existing T366/T367 pure driver criteria (FR-013, FR-037, AC-ADM-045):**
Extend the original publication driver with typed captured/current epoch inputs. After
deactivate→reactivate or revoke→restore before execution, restored active eligibility cannot
hide the advanced epoch: any mismatch selects terminal cancellation with a visible reason.
Unchanged epoch with valid eligibility, due time, approval and all other gates selects
execution. T367 implements this pure decision; no I/O or locks enter the domain.

- [ ] T366 [US3] Write failing test: [unit] `publication_requires_exact_review_and_authorised_time` in `crates/admin/src/domain/publication.rs`; proptest invalidation, same-subject ABA versus unrelated change, urgency, due-time and overdue-actual-time transitions; expect missing pure rules (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045). Include the existing scheduled-time input rules from T281: Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input. Drive pure scheduled authority/cancellation transitions using explicit typed scheduling-admin eligibility/epoch, captured epoch, approval status and time inputs: revoked/recovered/deactivated selects terminal cancellation before prerequisite blocks, current authority with invalid approval blocks, and valid authority preserves one-effect execution. Keep scheduler/worker/approver identities distinct; no I/O or runtime locking in this pure driver.
- [ ] T367 [US3] Make pass: T366 implement PublicationRequest/Result, immutable version identity and explicit blocked reasons in `crates/admin/src/domain/publication.rs`, `crates/admin/src/domain/mod.rs`; urgent requests never bypass approval, no in-process schedule timer (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045). Reuse the pure T282 scheduled-time value/parser and explicit Clock boundary checks; reject new/revised past or ambiguous/nonexistent input while preserving durable overdue semantics, UTC/display-zone storage and existing exact-review invalidation. Implement the preceding pure cancellation/approval/authority transitions and typed scheduler/epoch/approver/cancellation values; receive state/time as explicit inputs, never perform I/O or locks in domain. Runtime orchestration is driven later by T384/T385.
- [ ] T368 [US3] Write failing test: [unit] `publication_repository_port_is_mockable` in `crates/admin/src/ports/publication_repository.rs`; expect missing idempotent create/payload-conflict/bounded due-candidate read/lock/conditional complete/block/resolve-outcome methods (FR-013, FR-037, FR-CON-05). Also cover retry after an explicit unavailable-evidence block: retain the original request ID/payload and blocked reason until rechecked; bounded due selection may include retryable blocked rows. The idempotent request/retry operation conditionally reopens the locked blocked state only after current actor/head/approval/baseline/due/evidence checks permit. Identical recovered evidence needs no fresh confirmation; semantic mismatch still needs renewed review, and terminal/completed state cannot reopen. Drive only the PublicationRepository surface and owned DTOs here: persist scheduling_admin_id/scheduling_security_epoch/approver_id/reschedules_publication_id, terminal cancelled result, cancellation reason/time and conditional cancel; worker authority policy remains application code, not a prerequisite for this trait test.
- [ ] T369 [US3] Make pass: T368 define PublicationRepository in `crates/admin/src/ports/publication_repository.rs`, `crates/admin/src/ports/mod.rs`; owned expected-pending-state/result DTOs and explicit TxHandle (FR-013, FR-037, FR-CON-05). The existing request/retry operation supports conditional reopening of retryable blocked state under the same identity and current review/baseline/prerequisite checks; due work includes those durable candidates. No automatic approval bypass or duplicate completion. Define the preceding owned fields and conditional cancel/result operations; do not implement worker policy or invoke another adapter from a repository.
- [ ] T370 [US3] Write failing test: [contract] `publication_repository_contract` in `crates/testkit/src/contracts/publication_repository.rs`, runner `crates/testkit/tests/publication_repository_fake.rs`; expect missing payload-sensitive retry identity, due ordering, durable pending work after crash/precommit failure, stale conditional-finalization refusal and one committed result (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045). Include `worker_and_http_publish_commit_one_effect`: independent HTTP/worker retries commit one version/result and uniquely keyed success audit. Include `aba_live_version_invalidates_approval`: 30→7→30 version identity is not equal to the old baseline. Include `unrelated_subject_change_preserves_approval`: unrelated subject updates do not invalidate or lock the target incorrectly. Include `stale_publication_state_rejects_stale_worker`: a stale finalizer cannot overwrite a completed/blocked or changed expected state; precommit failure leaves pending work durable. Include `unknown_prerequisite_persists_blocked_reason`: unavailable prerequisite persists an explicit blocked reason without live-pointer or success-audit writes. Include `overdue_effect_is_not_backdated`: authorized schedule and actual application timestamps are distinct, never backdated. Lock draft head → request → subject in a short final transaction; condition on pending state, current revision/approval/actor, baseline version identity, due time and current evidence; no vendor/evidence I/O under write locks. Unique approval/publication identity rejects conflicting payload; a concurrent loser returns the stored result or typed conflict. Also cover retry after an explicit unavailable-evidence block: retain the original request ID/payload and blocked reason until rechecked; bounded due selection may include retryable blocked rows. The idempotent request/retry operation conditionally reopens the locked blocked state only after current actor/head/approval/baseline/due/evidence checks permit. Identical recovered evidence needs no fresh confirmation; semantic mismatch still needs renewed review, and terminal/completed state cannot reopen. Before services exist, shared repository cases test scheduling-admin/captured-epoch/approver round-trip, matching immutable approval, terminal conditional cancellation/retry identity, cancelled due-work exclusion and linked reschedule identity, with UoW/audit rollback using already delivered fake repositories. Runtime eligibility/approval orchestration belongs to T384/T385, not this contract prerequisite. The fake shared contract includes cancellation terminality, scheduling authority/approver round-trip, different-admin linked rescheduling, cancelled due-work exclusion, once-only audit/result and failure rollback. Admin read-unavailability is not proof of revocation; fail closed without executing or inventing a confirmed cancellation.
- [ ] T371 [US3] Make pass: T370 implement `crates/testkit/src/fakes/publication_repository.rs`, extend `crates/testkit/src/fakes/store.rs`, register `crates/testkit/src/fakes/mod.rs` and `crates/testkit/src/contracts/mod.rs`; only the locked expected pending state can finalize; repeated/concurrent attempts return one committed result (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045). Preserve durable pending due work without exclusive claims, short draft→request→subject locks, payload-sensitive unique identity and expected-state conditional complete/block; stale attempts return stored result/conflict. Atomic version/live/result/audit; no publication ownership lease or fencing generation. The existing request/retry operation supports conditional reopening of retryable blocked state under the same identity and current review/baseline/prerequisite checks; due work includes those durable candidates. No automatic approval bypass or duplicate completion. Implement only expanded repository DTO/storage/expected-state cancellation and identity/linkage behavior demanded by T370 in the shared FakeStore/UoW; application services own current-authority policy, no repository-to-repository orchestration.

### Review/publication services, entirely fake-first

**U3 — existing T378/T379 and T384/T385 driver criteria (FR-013, FR-037, AC-ADM-045):**
T378/T379 require SchedulePublish to capture the scheduling admin's current epoch under
the existing UoW/lock; T380 persists it without subsequent refresh/replacement on retries.
Extend both original ScheduledPublish drivers T384/T385 with
`deactivated_then_reactivated_scheduler_cancels_publication`,
`revoked_then_restored_scheduler_cancels_publication` and
`unchanged_scheduler_epoch_executes_publication` before T386 implementation. Capture E,
commit invalidation plus epoch advance and then restoration before the worker's first
execution; use due work, active restored eligibility, valid approval and otherwise passing
prerequisites to isolate epoch comparison. Any mismatch cancels, with one visible-reason
audit (actor=scheduling admin, executed_by=system_worker, approver recorded), no live/version
or success-publication effect and unchanged terminal result under retries. Unchanged E with
all gates passing executes once. Audit failure rolls back cancellation, as already required.
T386 implements this comparison under the scheduling-admin lock, never refreshing captured E
to authorize old work; no new endpoint or task is introduced.

- [ ] T372 [US3] Write failing test: [mock] `invalid_approval_writes_nothing` in `crates/testkit/tests/mock_approve.rs`; expect missing head/preview/baseline/provenance/known-impact checks before exactly one approval+audit, no writes on failure (FR-012, FR-015, FR-CON-07, AC-ADM-014, AC-ADM-020, AC-ADM-044). Require fresh pure impact calculation outside write locks and the pure comparison to reviewed evidence before approval writes; changed or unavailable required evidence yields zero approval writes. Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Also drive `audit_approval_has_complete_safe_fields_once` in this original service driver: expect the exact-version approval event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T373 [US3] Write failing test: [fake] `approval_binds_current_preview_revision_and_baseline` in `crates/testkit/tests/fake_approve.rs`; expect missing self-approval-as-distinct-step support, stale/edited/unknown-impact refusal and preserved current live state (FR-012, FR-015, FR-CON-07, AC-ADM-014, AC-ADM-020, AC-ADM-044). Include later assessed_at with identical semantic fields (eligible), changed applicable counts/effect/source (new preview/approval required), and unavailable required evidence (blocked). Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Also drive `audit_approval_has_complete_safe_fields_once` in this original service driver: expect the exact-version approval event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T374 [US3] Make pass: T372 T373 implement Approve in `crates/admin/src/application/approve.rs`, `crates/admin/src/application/mod.rs`; no new two-person rule for ordinary changes (FR-012, FR-015, FR-CON-07, AC-ADM-014, AC-ADM-020, AC-ADM-044). Reuse the pure comparison from T288 and current typed impact evidence; no new time-based validity rule. Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Complete the typed allowlisted `audit_approval_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T375 [US3] Write failing test: [mock] `publish_rechecks_then_commits_version_pointer_result_and_one_audit` in `crates/testkit/tests/mock_publish.rs`; expect missing short-transaction boundaries, draft→request→subject lock order, fresh impact/readiness, exactly-once writes and rollback on failed audit (FR-013, FR-015, FR-024, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-020, AC-ADM-025). Assert canonical impact comparison with reviewed evidence: unchanged later assessment preserves approval; changed semantic fields require renewed review; unavailable required evidence blocks without live writes/success audit. Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Drive `unknown_publication_commit_is_resolved_without_duplicate_effect` and `unknown_publication_commit_is_resolved_without_duplicate_effect_port_interactions` here: bounded stable-ID lookup before retry, never report unknown as confirmed rollback, lost-ack injection creates one effect/audit, precommit/audit failure commits nothing. Keep real lost-ack acceptance T399. Also cover retry after an explicit unavailable-evidence block: retain the original request ID/payload and blocked reason until rechecked; bounded due selection may include retryable blocked rows. The idempotent request/retry operation conditionally reopens the locked blocked state only after current actor/head/approval/baseline/due/evidence checks permit. Identical recovered evidence needs no fresh confirmation; semantic mismatch still needs renewed review, and terminal/completed state cannot reopen. Also drive `audit_publication_has_complete_safe_fields_once` in this original service driver: expect the successful publication including scheduled/actual times event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T376 [US3] Write failing test: [fake] `approved_change_publishes_only_at_authorised_time`, `repeated_publication_has_one_effect`, `urgent_foundation_change_still_requires_review` in `crates/testkit/tests/fake_publish.rs`; expect missing one-effect identity, preserved baseline gates and no premature/unaudited live change (FR-013, FR-015, FR-024, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-020, AC-ADM-025). Exercise all three T287 impact-comparison cases through immediate Publish, including retry after transient unavailable evidence returns unchanged; never reinterpret unknown as zero. Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Drive `unknown_publication_commit_is_resolved_without_duplicate_effect` and `unknown_publication_commit_is_resolved_without_duplicate_effect_port_interactions` here: bounded stable-ID lookup before retry, never report unknown as confirmed rollback, lost-ack injection creates one effect/audit, precommit/audit failure commits nothing. Keep real lost-ack acceptance T399. Also cover retry after an explicit unavailable-evidence block: retain the original request ID/payload and blocked reason until rechecked; bounded due selection may include retryable blocked rows. The idempotent request/retry operation conditionally reopens the locked blocked state only after current actor/head/approval/baseline/due/evidence checks permit. Identical recovered evidence needs no fresh confirmation; semantic mismatch still needs renewed review, and terminal/completed state cannot reopen. Also drive `audit_publication_has_complete_safe_fields_once` in this original service driver: expect the successful publication including scheduled/actual times event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T377 [US3] Make pass: T375 T376 implement Publish finalisation in `crates/admin/src/application/publish.rs`, `crates/admin/src/application/mod.rs`; reads/assessments outside locks, final expected-state/head/baseline/actor checks and atomic version/pointer/completion/audit (FR-013, FR-015, FR-024, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-020, AC-ADM-025). Reuse T288 canonical comparison; persist the existing blocked reason on changed/unavailable evidence, require renewed review only on semantic mismatch and never use assessed_at as a TTL. Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Preserve durable pending due work without exclusive claims, short draft→request→subject locks, payload-sensitive unique identity and expected-state conditional complete/block; stale attempts return stored result/conflict. Atomic version/live/result/audit; no publication ownership lease or fencing generation. Resolve unknown commit by bounded lookup of the original publication ID before retry; return stored result without duplicate effect/success audit. The existing request/retry operation supports conditional reopening of retryable blocked state under the same identity and current review/baseline/prerequisite checks; due work includes those durable candidates. No automatic approval bypass or duplicate completion. Complete the typed allowlisted `audit_publication_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T378 [US3] Write failing test: [mock] `scheduled_change_blocks_when_live_baseline_changes_port_interactions` in `crates/testkit/tests/mock_schedule_publish.rs`; expect exact-version/Clock/head/baseline validation not to precede one request/audit and zero live-pointer writes (FR-012, FR-013, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-044). Also drive `audit_schedule_has_complete_safe_fields_once` in this original service driver: expect the scheduled request creation event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025). Include the existing scheduled-time input rules from T281: Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input. Drive scheduling creation with stored scheduling_admin_id/captured scheduling_security_epoch and approver_id from the exact approval, including distinct scheduler/approver, actor eligibility at creation and payload-sensitive idempotency; execution/cancellation itself is driven in T384/T385. When rescheduling a cancelled request, verify caller differs from its immutable scheduling admin, retain the predecessor and use a new reviewed revision/approval for the changed future time, not the cancelled request ID.
- [ ] T379 [US3] Write failing test: [fake] `scheduled_change_blocks_when_live_baseline_changes` in `crates/testkit/tests/fake_schedule_publish.rs`; expect scheduled_publish_rejects_invalidated_approval or ABA identity checks to fail; unrelated subjects must remain independent (FR-012, FR-013, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-044). Also drive `audit_schedule_has_complete_safe_fields_once` in this original service driver: expect the scheduled request creation event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025). Include the existing scheduled-time input rules from T281: Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input. Drive scheduling creation with stored scheduling_admin_id/captured scheduling_security_epoch and approver_id from the exact approval, including distinct scheduler/approver, actor eligibility at creation and payload-sensitive idempotency; execution/cancellation itself is driven in T384/T385. When rescheduling a cancelled request, verify caller differs from its immutable scheduling admin, retain the predecessor and use a new reviewed revision/approval for the changed future time, not the cancelled request ID.
- [ ] T380 [US3] Make pass: T378 T379 implement SchedulePublish only with durable UTC/zone and blocked reasons in `crates/admin/src/application/schedule_publish.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-012, FR-013, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-044). Complete the typed allowlisted `audit_schedule_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework. Reuse the pure T282 scheduled-time value/parser and explicit Clock boundary checks; reject new/revised past or ambiguous/nonexistent input while preserving durable overdue semantics, UTC/display-zone storage and existing exact-review invalidation. Persist the driven scheduling authority/approver and optional cancelled predecessor under the existing UoW; validate active caller, exact approval, time and different-admin reschedule identity. Do not execute scheduled work here.
- [ ] T381 [US3] Write failing test: [mock] `get_publication_returns_exact_state_without_writes_port_interactions` in `crates/testkit/tests/mock_get_publication.rs`; expect authorization not to precede exactly one PublicationRepository exact-ID read or any write/due-work mutation to occur (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-017, AC-ADM-045). Cover cancelled state, safe scheduling-admin/approver attribution, cancellation reason/time and another-admin rescheduling instruction with zero writes; never present cancellation as success or retryable blocked work.
- [ ] T382 [US3] Write failing test: [fake] `get_publication_returns_exact_state_without_writes` in `crates/testkit/tests/fake_get_publication.rs`; expect missing/other-subject ID accepted, pending/blocked/cancelled/completed state confused, or blocked reason/scheduled/actual time omitted (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-017, AC-ADM-045). Cover cancelled state, safe scheduling-admin/approver attribution, cancellation reason/time and another-admin rescheduling instruction with zero writes; never present cancellation as success or retryable blocked work.
- [ ] T383 [US3] Make pass: T381 T382 implement GetPublication only with typed not-found and sanitized state DTOs in `crates/admin/src/application/get_publication.rs`, register `crates/admin/src/application/mod.rs`; use port traits only (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-017, AC-ADM-045). Return the driven cancelled/authority/approver/reason fields safely without writes or terminal-state revival.
- [ ] T384 [US3] Write failing test: [mock] `overdue_runner_reads_bounded_due_work_rechecks_and_audits_once` in `crates/testkit/tests/mock_scheduled_publish.rs`; expect missing Clock/bounded due-candidates/reassess/conditional finalise order, external evidence gathered under locks or duplicate success audit (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045). Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Also drive scheduled authority/cancellation cases: Persist scheduling_admin_id, captured scheduling_security_epoch and approver_id from the exact approval. Worker acts on behalf of scheduling admin; re-read/lock that admin before draft→request→subject locks and recheck active eligibility, captured epoch and current approval validity. Revoked/recovered/deactivated scheduler causes terminal audited cancellation with visible reason and no version/live-pointer/success-publication event; invalid admin takes precedence over unavailable prerequisites. Audit actor = scheduling admin, executed_by = system_worker, approver recorded. Only another active verified admin may reschedule via the existing reviewed workflow, new request identity and reschedules_publication_id linkage; reject original scheduler, never reopen/overwrite cancellation. Time edits still require fresh review/approval; unique approval identity remains. Named cases: `revoked_scheduler_cancels_publication`, `recovered_scheduler_cancels_publication`, `deactivated_scheduler_cancels_publication`, `cancelled_schedule_requires_different_admin`, `scheduled_cancellation_is_audited_once_or_rolled_back`. Use distinct scheduler/approver, active happy path, valid admin/invalid approval (blocked), recovery vs finalization serialized on the admin row, audit failure rollback and repeated cancellation with one audit/no success; reenrollment cannot revive the original schedule.
- [ ] T385 [US3] Write failing test: [fake] `overdue_publication_rechecks_prerequisites_and_runs_once` in `crates/testkit/tests/fake_scheduled_publish.rs`; advance FixedClock across outage, expect absent exact-version/baseline/impact/readiness rechecks, persisted block reason, actual-time effect or stale expected-state finalization refusal (FR-013, FR-037, FR-CON-05, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045). Exercise the same unchanged-later, changed-destructive and unavailable-evidence cases in overdue processing; recovered identical evidence may reuse an otherwise valid approval. Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Assert no removal of pending due work during candidate selection, and safe retry after precommit crash; overlapping candidates must still finalize one effect. Also cover retry after an explicit unavailable-evidence block: retain the original request ID/payload and blocked reason until rechecked; bounded due selection may include retryable blocked rows. The idempotent request/retry operation conditionally reopens the locked blocked state only after current actor/head/approval/baseline/due/evidence checks permit. Identical recovered evidence needs no fresh confirmation; semantic mismatch still needs renewed review, and terminal/completed state cannot reopen. Also drive scheduled authority/cancellation cases: Persist scheduling_admin_id, captured scheduling_security_epoch and approver_id from the exact approval. Worker acts on behalf of scheduling admin; re-read/lock that admin before draft→request→subject locks and recheck active eligibility, captured epoch and current approval validity. Revoked/recovered/deactivated scheduler causes terminal audited cancellation with visible reason and no version/live-pointer/success-publication event; invalid admin takes precedence over unavailable prerequisites. Audit actor = scheduling admin, executed_by = system_worker, approver recorded. Only another active verified admin may reschedule via the existing reviewed workflow, new request identity and reschedules_publication_id linkage; reject original scheduler, never reopen/overwrite cancellation. Time edits still require fresh review/approval; unique approval identity remains. Named cases: `revoked_scheduler_cancels_publication`, `recovered_scheduler_cancels_publication`, `deactivated_scheduler_cancels_publication`, `cancelled_schedule_requires_different_admin`, `scheduled_cancellation_is_audited_once_or_rolled_back`. Use distinct scheduler/approver, active happy path, valid admin/invalid approval (blocked), recovery vs finalization serialized on the admin row, audit failure rollback and repeated cancellation with one audit/no success; reenrollment cannot revive the original schedule.
- [ ] T386 [US3] Make pass: T384 T385 implement ScheduledPublish in `crates/admin/src/application/scheduled_publish.rs`, `crates/admin/src/application/mod.rs`; one recovery effect, no fresh confirmation if unchanged valid approval, never backdate (FR-013, FR-037, FR-CON-05, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045). Share the T288 comparison with immediate Publish; no additional freshness threshold. Call pure impact/readiness rules directly on typed evidence with the code-owned delivered registry; no policy-port mock/fake. Unavailable required evidence blocks destructive publication; all six undelivered capabilities stay OFF. Preserve durable pending due work without exclusive claims, short draft→request→subject locks, payload-sensitive unique identity and expected-state conditional complete/block; stale attempts return stored result/conflict. Atomic version/live/result/audit; no publication ownership lease or fencing generation. The existing request/retry operation supports conditional reopening of retryable blocked state under the same identity and current review/baseline/prerequisite checks; due work includes those durable candidates. No automatic approval bypass or duplicate completion. Implement the preceding-driver scheduled authority/cancellation behavior: Persist scheduling_admin_id, captured scheduling_security_epoch and approver_id from the exact approval. Worker acts on behalf of scheduling admin; re-read/lock that admin before draft→request→subject locks and recheck active eligibility, captured epoch and current approval validity. Revoked/recovered/deactivated scheduler causes terminal audited cancellation with visible reason and no version/live-pointer/success-publication event; invalid admin takes precedence over unavailable prerequisites. Audit actor = scheduling admin, executed_by = system_worker, approver recorded. Only another active verified admin may reschedule via the existing reviewed workflow, new request identity and reschedules_publication_id linkage; reject original scheduler, never reopen/overwrite cancellation. Time edits still require fresh review/approval; unique approval identity remains.
- [ ] T387 [US3] Write failing test: [mock] `rollback_appends_a_draft_and_audit_without_rewriting_history` in `crates/testkit/tests/mock_rollback.rs`; expect missing historic read→new revision→one audit and direct live/history overwrite (FR-014, FR-CON-08, AC-ADM-019). Also drive `audit_rollback_has_complete_safe_fields_once` in this original service driver: expect the new rollback draft and retained prior version event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T388 [US3] Write failing test: [fake] `rollback_creates_new_version_and_preserves_history` in `crates/testkit/tests/fake_rollback.rs`; expect missing new rollback draft followed by ordinary preview/approval/publish, original actor/history retention and refusal to undo external executed effects (FR-014, FR-CON-08, AC-ADM-019). Also drive `audit_rollback_has_complete_safe_fields_once` in this original service driver: expect the new rollback draft and retained prior version event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success. Assert complete safe actor/action/subject/time/reason/allowlisted before-after/factor fields as applicable, exactly one event and atomic rollback/no unaudited success (FR-021, FR-024, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T389 [US3] Make pass: T387 T388 implement Rollback in `crates/admin/src/application/rollback.rs`, `crates/admin/src/application/mod.rs`; reuse reviewed workflow, no history deletion (FR-014, FR-CON-08, AC-ADM-019). Complete the typed allowlisted `audit_rollback_has_complete_safe_fields_once` event demanded by these preceding drivers in this original slice; preserve atomic UoW and all prior tests, with no later audit-completion pass or speculative shared event framework.
- [ ] T390 [US3] Acceptance test: [fake] `published_recovery_policy_is_new_requests_only` in `crates/testkit/tests/published_recovery_policy.rs`; publish an approved 30→14 recovery-window draft through the implemented service, inspect the exact live version/policy, keep existing end-date D fixture unchanged and expose the new window for future requests only. Require existing behavior green; do not add account deletion execution or invent a failure (FR-010, FR-013, FR-CON-32, P41, AC-ADM-046, OI8).
- [ ] T391 [US3] Write failing test: [fake] `worker_uses_services_and_resumes_due_work_once` in `crates/worker/tests/scheduled_loop.rs`; expect the service-only bounded loop to miss controlled due work, duplicate a recovered effect or ignore cancellation under fake Clock/UoW (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045).
- [ ] T392 [US3] Make pass: T391 implement only the fake-driven service loop and graceful shutdown; add crates/worker/Cargo.toml, crates/worker/src/lib.rs and workspace registration, no real adapter imports in `crates/worker/src/scheduled.rs`; edit only this behavior and its module registration (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045).

### Real publication storage, scheduling authority and concurrency

**U3 — existing T395/T396/T397 integration criteria (FR-013, FR-037, AC-ADM-045):**
Extend the prepared harness and original real runner with the same three named T384/T385
cases: separately commit invalidation/epoch advance and restoration before worker execution,
observe durability through a fresh runtime connection, and require cancellation for any
captured/current epoch mismatch. Keep the unchanged-epoch/all-gates-valid execution control,
one attributable visible-reason cancellation audit, no live/success effect, audit rollback
and terminal retry. Use completed AdminRepository operations, not direct fixture updates
that bypass the epoch invariant. T397 implements only repository behavior driven by this
existing suite; the T386 application service owns the scheduling decision.

- [ ] T393 [US3] Write failing test: [integration] `publication_schema_enforces_unique_approval_and_conditional_state` in `crates/infra-postgres/tests/integration_publication_schema.rs`; expect missing "UNIQUE(approval_id)", payload identity conflict detection, "partial due index on scheduled_at for pending work" and "pending/blocked/cancelled/completed state/result checks" plus completed subject/version links (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045). Drive SQL schema constraints for scheduling-admin/epoch and approver storage/FKs, approval/approver matching, optional reschedules_publication_id FK, cancelled reason/time with no completed version/applied time, and due indexes excluding terminal cancelled/completed rows. This test drives migration constraints, not unbuilt real worker behavior.
- [ ] T394 [US3] Make pass: T393 add `crates/infra-postgres/migrations/0007_publications.sql`; durable requests, scheduled/actual times, completion references and pending due indexes, no destructive history rewrites (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045). Add only those scheduling authority, approval/approver matching, reschedule-link and terminal cancellation columns/FKs/checks/indexes to the publication migration; current-eligibility policy remains in the completed application service.
- [ ] T395 [US3] Setup: prepare `crates/infra-postgres/tests/support/publication.rs` with completed publication services, two isolated runtime connections and deterministic evidence/time controls. Verify harness readiness first; do not mix setup with admin/adapter behavior changes (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-044, AC-ADM-045). Prepare distinct scheduler/approver and active/revoked/recovered/deactivated epoch fixtures, cancellation audit failure and independent-connection recovery controls; no new admin lifecycle endpoint.
- [ ] T396 [US3] Write failing test: [contract] [integration] `publication_repository_contract_postgres` in `crates/infra-postgres/tests/integration_publication_repository.rs`; run T370 unchanged with concurrent HTTP/worker attempts, payload conflict, durable pending work, stale conditional-finalization rejection and committed-result lookup; expect absent real methods (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-044, AC-ADM-045). Include `worker_and_http_publish_commit_one_effect`: independent HTTP/worker retries commit one version/result and uniquely keyed success audit. Include `aba_live_version_invalidates_approval`: 30→7→30 version identity is not equal to the old baseline. Include `unrelated_subject_change_preserves_approval`: unrelated subject updates do not invalidate or lock the target incorrectly. Include `stale_publication_state_rejects_stale_worker`: a stale finalizer cannot overwrite a completed/blocked or changed expected state; precommit failure leaves pending work durable. Include `unknown_prerequisite_persists_blocked_reason`: unavailable prerequisite persists an explicit blocked reason without live-pointer or success-audit writes. Include `overdue_effect_is_not_backdated`: authorized schedule and actual application timestamps are distinct, never backdated. Lock draft head → request → subject in a short final transaction; condition on pending state, current revision/approval/actor, baseline version identity, due time and current evidence; no vendor/evidence I/O under write locks. Unique approval/publication identity rejects conflicting payload; a concurrent loser returns the stored result or typed conflict. Use T395 prepared harness, two independent restricted-runtime connections and deterministic time/evidence controls; these real SQL cases drive the original adapter, not later repair passes. Also cover retry after an explicit unavailable-evidence block: retain the original request ID/payload and blocked reason until rechecked; bounded due selection may include retryable blocked rows. The idempotent request/retry operation conditionally reopens the locked blocked state only after current actor/head/approval/baseline/due/evidence checks permit. Identical recovered evidence needs no fresh confirmation; semantic mismatch still needs renewed review, and terminal/completed state cannot reopen. Also drive scheduled authority/cancellation cases: Persist scheduling_admin_id, captured scheduling_security_epoch and approver_id from the exact approval. Worker acts on behalf of scheduling admin; re-read/lock that admin before draft→request→subject locks and recheck active eligibility, captured epoch and current approval validity. Revoked/recovered/deactivated scheduler causes terminal audited cancellation with visible reason and no version/live-pointer/success-publication event; invalid admin takes precedence over unavailable prerequisites. Audit actor = scheduling admin, executed_by = system_worker, approver recorded. Only another active verified admin may reschedule via the existing reviewed workflow, new request identity and reschedules_publication_id linkage; reject original scheduler, never reopen/overwrite cancellation. Time edits still require fresh review/approval; unique approval identity remains. Run expanded T370 unchanged and real independent-connection recovery/finalization races; cancellation commits atomically with its audit, retries return one terminal result and cancelled rows never re-enter due work.
- [ ] T397 [US3] Make pass: T396 implement `crates/infra-postgres/src/publication_repository.rs`, `crates/infra-postgres/src/lib.rs`; short transactional row locks plus conditional finalisation under unique publication identity, one result across independent connections (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-044, AC-ADM-045). Preserve durable pending due work without exclusive claims, short draft→request→subject locks, payload-sensitive unique identity and expected-state conditional complete/block; stale attempts return stored result/conflict. Atomic version/live/result/audit; no publication ownership lease or fencing generation. The existing request/retry operation supports conditional reopening of retryable blocked state under the same identity and current review/baseline/prerequisite checks; due work includes those durable candidates. No automatic approval bypass or duplicate completion. Completion criterion: sqlx metadata prepared and offline check passes before green; migrate the fixture-owned preparation database, run `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, refresh `.sqlx/query-*.json`, restore SQLX_OFFLINE=true and rerun driving/affected/integration suites. Setup/tool failures are blockers, not intended red. Implement the expanded repository persistence/cancel/result operations and guarded SQL state transitions required by T396; preserve caller UoW, cancellation audit atomicity and immutable identity/history. Do not call other adapters or move scheduling authority policy out of the existing application service.
- [ ] T398 [US3] Acceptance test: [integration] `crash_before_commit_leaves_no_live_change` in `crates/infra-postgres/tests/integration_precommit_failure.rs`; using prepared test-owned fault controls require no visible version/pointer/success audit before commit, already driven by fake contracts/services. Missing fault tooling is a setup blocker; never introduce a production failure switch (FR-013, FR-024, FR-037, FR-CON-37, AC-ADM-018, AC-ADM-025, AC-ADM-045).
- [ ] T399 [US3] Acceptance test: [integration] `lost_commit_ack_resolves_by_publication_id` in `crates/infra-postgres/tests/integration_lost_ack.rs`; using prepared test-owned fault controls require one committed result resolved by the stable ID after a lost acknowledgement, already driven by fake contracts/services. Missing fault tooling is a setup blocker; never introduce a production failure switch (FR-013, FR-024, FR-037, FR-CON-37, AC-ADM-018, AC-ADM-025, AC-ADM-045).
- [ ] T400 [US3] Acceptance test: [integration] `publication_and_audit_commit_together` in `crates/infra-postgres/tests/integration_publication_audit_atomicity.rs`; using prepared test-owned fault controls require both the live change and required audit or neither, already driven by fake contracts/services. Missing fault tooling is a setup blocker; never introduce a production failure switch (FR-013, FR-024, FR-037, FR-CON-37, AC-ADM-018, AC-ADM-025, AC-ADM-045).
- [ ] T401 [US3] Acceptance test: [integration] `published_recovery_policy_is_new_requests_only_postgres` in `crates/infra-postgres/tests/integration_published_recovery_policy.rs`; run the same publication-policy assertion with real runtime-role storage and the prepared publication harness; verify live version/history and preserved D fixture, not a new account-deletion subsystem (FR-010, FR-013, FR-CON-32, P41, AC-ADM-046, OI8).

### Review/publication HTML and durable worker

- [ ] T402 [US3] Write failing test: [http] `review_publish_forms_bind_exact_revision_and_show_zone` in `crates/web/tests/publication_forms.rs`; expect missing approve/publish/schedule routes, fresh preview after edits, timezone ambiguity, missing urgent confirmation or omitted block reason (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-014, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045). Include the existing scheduled-time input rules from T281: Console local date+time plus IANA display zone (default Asia/Tehran) resolves to UTC with pinned embedded chrono/chrono-tz rules and explicit Clock input. Reject nonexistent/ambiguous local times and new/revised past schedules, never guess offsets. JSON time values accept ISO 8601 with explicit Z or numeric offset (e.g. ±HH:mm) only, rejecting naive/malformed values; this adds no JSON endpoint. Test Tehran conversion/default, valid neighboring Europe/Berlin times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap rejection with now=2026-01-01T00:00:00Z so past-time checks cannot mask them, separate future/past boundaries and positive/negative/Z offsets. Already-authorized overdue execution is not rejected as newly submitted past input. Assert visible localized cancellation reason and another-admin reschedule action, retaining original request history and a new exact-review/timing identity.
- [ ] T403 [US3] Make pass: T402 extend `crates/web/src/console/changes.rs`, `crates/web/templates/console/change.html`, add `crates/web/templates/partials/publication_status.html` and both console Fluent files; secure POST/303, no full-page/htmx approval bypass (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-014, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045). Reuse the pure T282 scheduled-time value/parser and explicit Clock boundary checks; reject new/revised past or ambiguous/nonexistent input while preserving durable overdue semantics, UTC/display-zone storage and existing exact-review invalidation. Expose the driven cancelled result/rescheduling instructions through existing forms only, with no new route or admin-deactivation behavior.
- [ ] T404 [US3] Write failing test: [http] `rollback_and_publication_detail_keep_history_attributed` in `crates/web/tests/rollback.rs`; expect missing publication/version detail and rollback form, missing scheduled/actual times or old-version overwrite (FR-013, FR-014, FR-021, FR-CON-08, AC-ADM-019, AC-ADM-021, AC-ADM-045). Include cancelled detail/history and scheduler/worker/approver attribution; cancelled is not completed or silently retryable.
- [ ] T405 [US3] Make pass: T404 implement `crates/web/src/console/publications.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/publication.html`, `crates/web/templates/console/version.html`, both console Fluent files; rollback enters a new draft, not direct publish (FR-013, FR-014, FR-021, FR-CON-08, AC-ADM-019, AC-ADM-021, AC-ADM-045). Render the driven terminal cancellation/history and another-admin rescheduling action using existing services and Fluent FA/EN keys.
- [ ] T406 [US3] Write failing test: [integration] `app_worker_selects_real_or_fake_dependencies_and_recovers_due_work` in `crates/app/tests/integration_worker.rs`; expect the prepared app worker command to miss configuration-based readiness/publication/UoW wiring, resume incorrectly or bypass fake-production guards (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045). Exercise worker cancellation for each invalid scheduling-admin state/epoch, active execution and separate approver, correct audit attribution and no retry of cancelled work.
- [ ] T407 [US3] Make pass: T406 wire only the completed worker service loop in crates/app/src/main.rs and crates/app/src/composition.rs; the app crate selects adapters, the worker crate calls services only in `crates/app/src/worker.rs`; edit only this behavior and its module registration (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045). Wire the existing AdminRepository, PublicationRepository, AuditRepository and UoW into ScheduledPublish for current authority checks/cancellation; system_worker identity is trusted composition, not client input.
- [ ] T408 [US3] Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Validate AC-ADM-016–020/044–046 (AC-ADM-046 proves published policy only, not downstream deletion execution) on fakes AND real adapters in `specs/001-admin-foundation/tasks.md` (FR-CON-05, FR-CON-07, FR-CON-08, FR-CON-37).

## Phase 6: User Story 4 — Investigate an Attributable, Protected History (P1)

**Goal**: Complete searchable actor-specific evidence, immutable even for full-permission admins.
Audit storage/domain came first in US1 because access/publication must already be audited.
**Independent test**: AC-ADM-021–025 on fakes and runtime-role SQL; rejected UPDATE/DELETE/TRUNCATE
are successful security-test assertions, never intentionally red tests left unfinished.

- [ ] T409 [US4] Write failing test: [unit] `audit_filters_reject_unbounded_or_invalid_cursors` in `crates/admin/src/domain/audit_query.rs`; expect missing typed actor/action/subject/time filters, bounded limit and stable `(occurred_at,id)` cursor validation (FR-022, FR-CON-37, AC-ADM-022).
- [ ] T410 [US4] Make pass: T409 implement AuditQuery/KeysetCursor in `crates/admin/src/domain/audit_query.rs`, `crates/admin/src/domain/mod.rs`; no new arbitrary throughput SLA or editable audit record (FR-022, FR-CON-37, AC-ADM-022).
- [ ] T411 [US4] Write failing test: [mock] `audit_search_authorizes_before_read_and_never_writes` in `crates/testkit/tests/mock_search_audit.rs`; expect query before current-admin verification, unbounded reads or any append/update/delete during search (FR-001, FR-022, FR-CON-37, AC-ADM-022).
- [ ] T412 [US4] Write failing test: [fake] `admin_can_search_foundation_audit` in `crates/testkit/tests/fake_search_audit.rs`; expect missing actor/action/subject/period filters and deterministic no-gap/no-duplicate pagination across same-time events (FR-022, FR-CON-37, AC-ADM-022).
- [ ] T413 [US4] Make pass: T411 T412 implement SearchAudit in `crates/admin/src/application/search_audit.rs`, `crates/admin/src/application/mod.rs`; read-UoW and owned sanitized DTOs, no repository exposed to web (FR-001, FR-022, FR-CON-37, AC-ADM-022).
- [ ] T414 [US4] Write failing test: [mock] [fake] `audit_otp_request_has_complete_safe_fields_once` in `crates/testkit/tests/audit_otp_request.rs`; expect the primary challenge issuance event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T415 [US4] Make pass: T414 complete only the typed allowlisted event for primary challenge issuance in `crates/admin/src/application/request_admin_otp.rs`; retain its atomic UoW and all prior port/service tests. No other action or shared speculative event framework changes (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T416 [US4] Write failing test: [mock] [fake] `audit_otp_verify_has_complete_safe_fields_once` in `crates/testkit/tests/audit_otp_verify.rs`; expect the restricted primary proof verification event to omit the applicable actor/action/subject/time/reason/before-after/factor or be written more than once; audit failure must prevent success (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T417 [US4] Make pass: T416 complete only the typed allowlisted event for restricted primary proof verification in `crates/admin/src/application/verify_admin_otp.rs`; retain its atomic UoW and all prior port/service tests. No other action or shared speculative event framework changes (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T418 [US4] Regression test: [integration] `foundation_actions_have_complete_attributed_audit` in `crates/infra-postgres/tests/integration_audit_actions.rs`; rerun the now-established action inventory and require all event shapes/redaction/once-only semantics green, with no new production changes justified by a missing harness (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025). Include scheduled cancellation and its one safe actor=scheduler/executed_by=system_worker/approver/reason audit in the established action inventory.
- [ ] T419 [US4] Acceptance test: [integration] `audit_otp_request_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_otp_request.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe primary challenge issuance evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T420 [US4] Acceptance test: [integration] `audit_otp_verify_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_otp_verify.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe restricted primary proof verification evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T421 [US4] Acceptance test: [integration] `audit_passkey_signin_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_passkey_signin.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe passkey sign-in and factor evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T422 [US4] Acceptance test: [integration] `audit_totp_signin_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_totp_signin.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe TOTP sign-in and factor evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T423 [US4] Acceptance test: [integration] `audit_enrollment_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_enrollment.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe confirmed factor enrollment evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T424 [US4] Acceptance test: [integration] `audit_invitation_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_invitation.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe another-admin invitation evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T425 [US4] Acceptance test: [integration] `audit_recovery_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_recovery.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe another-admin recovery and target identity evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T426 [US4] Acceptance test: [integration] `audit_draft_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_draft.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe draft revision save evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T427 [US4] Acceptance test: [integration] `audit_preview_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_preview.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe exact-version preview evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T428 [US4] Acceptance test: [integration] `audit_approval_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_approval.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe exact-version approval evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T429 [US4] Acceptance test: [integration] `audit_schedule_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_schedule.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe scheduled request creation evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T430 [US4] Acceptance test: [integration] `audit_publication_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_publication.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe successful publication including scheduled/actual times evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025). Also accept the completed scheduled-cancellation event: scheduler actor, system worker executor, approver and visible reason, one event under retries and no live/success-publication effect; audit failure is atomic.
- [ ] T431 [US4] Acceptance test: [integration] `audit_rollback_matches_committed_action` in `crates/infra-postgres/tests/integration_audit_rollback.rs`; after all story fake-driven event pairs pass, use existing prepared fixtures to require exact safe new rollback draft and retained prior version evidence and failure atomicity on real SQL. Do not count harness failure as behavioral red (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T432 [US4] Write failing test: [http] `audit_entries_cannot_be_edited_or_deleted` and `audit_search_requires_current_admin` in `crates/web/tests/audit.rs`; expect absent searchable view, missing safe filters/next cursor or exposed mutation/non-admin route (FR-001, FR-022, FR-023, FR-CON-37, AC-ADM-022, AC-ADM-023).
- [ ] T433 [US4] Make pass: T432 implement `crates/web/src/console/audit.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/audit.html`, `crates/web/templates/partials/audit_table.html`, both console Fluent files; no edit/delete endpoint, no private-data details (FR-001, FR-022, FR-023, FR-CON-37, AC-ADM-022, AC-ADM-023).
- [ ] T434 [US4] Write failing test: [integration] `diagnostics_redact_synthetic_auth_and_payment_canaries` in `crates/app/tests/integration_redaction.rs`; expect synthetic phone/code/key/photo/payment canaries to leak through prepared captured tracing on provider/SQL/validation failures (FR-024, FR-CON-37, AC-ADM-024, AC-ADM-025). Include the one-time TOTP provisioning secret/QR as synthetic diagnostic canaries; its authorized response is never permission to log response bodies, request replay state or secret material.
- [ ] T435 [US4] Make pass: T434 implement only allowlisted app tracing and wire crates/app/src/main.rs without request/body/cookie/vendor URL Debug output in `crates/app/src/telemetry.rs`; edit only this behavior and its module registration (FR-024, FR-CON-37, AC-ADM-024, AC-ADM-025).
- [ ] T436 [US4] Write failing test: [http] `problem_responses_never_echo_secret_canaries` in `crates/web/tests/problem_redaction.rs`; expect a JSON/HTML error to expose injected secret canaries or claim successful change after audit/store failure (FR-024, FR-CON-37, AC-ADM-024, AC-ADM-025).
- [ ] T437 [US4] Make pass: T436 implement only sanitized web error mapping with RFC9457 JSON and localized HTML errors in `crates/web/src/api/problem.rs`; edit only this behavior and its module registration (FR-024, FR-CON-37, AC-ADM-024, AC-ADM-025).
- [ ] T438 [US4] Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Confirm AC-ADM-021–025 on fakes AND real adapters and record restricted-role/trigger results in `specs/001-admin-foundation/tasks.md` (FR-CON-01, FR-CON-37).

## Phase 7: User Story 5 — Work in Either Language and Any Theme (P1)

**Goal**: Complete every delivered foundation flow in FA/EN, light/dark/system, assistive input
and full-page navigation without losing safe unsaved work. Extend the existing foundation shell.
**Independent test**: AC-ADM-026–032/047 across instantiated templates and browser samples;
real rendered contrast supplements token calculations, never replaces them.

- [ ] T439 [US5] Write failing test: [unit] `preference_changes_preserve_safe_form_state_only` in `crates/web/tests/preferences_state.rs`; expect absent valid form/revision/selection preservation or reflected OTP/shared-secret/assertion (FR-026, FR-030, FR-UX-03, AC-ADM-027).
- [ ] T440 [US5] Make pass: T439 implement presentation-only carried form-state DTO in `crates/web/src/preferences.rs`, `crates/web/src/lib.rs`; state must not edit/save/publish a business draft (FR-026, FR-030, FR-UX-03, AC-ADM-027).
- [ ] T441 [US5] Write failing test: [http] `language_and_theme_switch_preserve_unsaved_input` in `crates/web/tests/preferences.rs`; expect missing CSRF-protected POST preference flow, discarded invalid-form safe fields/draft selection or changed live data (FR-026, FR-UX-03, AC-ADM-027).
- [ ] T442 [US5] Make pass: T441 implement preference route and shell controls in `crates/web/src/console/preferences.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/partials/preferences.html`, `crates/web/templates/console/layout.html`, both console Fluent files; system follows device preference and manual choice survives navigation (FR-025, FR-026, FR-UX-02, FR-UX-03, AC-ADM-026, AC-ADM-027).
- [ ] T443 [US5] Write failing test: [http] `foundation_screens_support_both_languages_and_all_themes`, `persian_direction_and_digits_preserve_identifiers` in `crates/web/tests/locale_theme_matrix.rs`; instantiate every delivered sign-in/enrollment/admin/settings/change/publication/version/audit view at six locale/theme combinations; expect missing keys/RTL/focus order, mirrored logos or localized IDs/URLs; US6 later extends this same inventory with its flag view (FR-025, FR-027, FR-030, FR-UX-01, FR-UX-04, FR-UX-05, AC-ADM-026, AC-ADM-028, AC-ADM-031).
- [ ] T444 [US5] Make pass: T443 implement digit/bidi/display helpers in `crates/web/src/presentation.rs`, `crates/web/src/lib.rs`, `crates/web/static/console.css`, relevant templates listed in T235, T237, T239, T241, T243, T245, T247/T249, T251, T253, T255, T257, T259, T261, T263, T265/T360/T362/T405/T433 and both console Fluent files; feature-flag view joins this same inventory in US6 (FR-025, FR-027, FR-030, FR-UX-01, FR-UX-04, FR-UX-05, AC-ADM-026, AC-ADM-028, AC-ADM-031).
- [ ] T445 [US5] Write failing test: [http] `field_errors_are_labeled_linked_and_non_colour_only` in `crates/web/tests/field_semantics.rs`; expect a control to lack its label/field-linked error and status text (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T446 [US5] Make pass: T445 implement only reusable accessible field/error semantics in `crates/web/templates/partials/field.html`; edit only this behavior and its module registration (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T447 [US5] Write failing test: [http] `dialog_has_named_controls_and_focus_return_metadata` in `crates/web/tests/dialog_semantics.rs`; expect a dialog to lack its name, close action or explicit return-focus target (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T448 [US5] Make pass: T447 implement only dialog structure and focus metadata; actual browser focus behavior has its own later driver in `crates/web/templates/partials/dialog.html`; edit only this behavior and its module registration (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T449 [US5] Write failing test: [http] `console_navigation_has_one_h1_and_logical_reading_order` in `crates/web/tests/navigation_semantics.rs`; expect navigation/headings or directional semantics to be absent or duplicated (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T450 [US5] Make pass: T449 implement only semantic navigation/headings and meaningful image alternatives in `crates/web/templates/console/layout.html`; edit only this behavior and its module registration (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T451 [US5] Write failing test: [http] `controls_have_44px_targets_and_visible_token_focus` in `crates/web/tests/target_sizes.rs`; expect controls to lack token-driven 44x44 targets or visible boundary/focus styles (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T452 [US5] Make pass: T451 implement only target sizing and focus/boundary styles from generated tokens in `crates/web/static/console.css`; edit only this behavior and its module registration (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T453 [US5] Write failing test: [http] `nonessential_motion_has_reduced_and_pause_fallbacks` in `crates/web/tests/motion_fallback.rs`; expect reduced motion/opaque fallback to hide meaning or nonessential auto-motion over five seconds to lack a pause/stop control (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T454 [US5] Make pass: T453 implement only reduced-motion/opaque behavior; add pause control only if such motion is actually present in `crates/web/static/console.css`; edit only this behavior and its module registration (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T455 [US5] Regression test: [http] `foundation_tasks_are_keyboard_and_assistive_accessible` in `crates/web/tests/accessibility.rs`; run the delivered instantiated-view semantic inventory after its focused primitives pass; browser/device acceptance is additional, not inferred from markup (FR-029, FR-UX-08, AC-ADM-030).
- [ ] T456 [US5] Write failing test: [http] `foundation_tasks_work_without_optional_enhancements` in `crates/web/tests/progressive_management.rs`; run draft/preview/approve/publish/rollback/recovery without HX-Request or optional scripts; expect missing full-page forms, validation preservation or security parity (FR-031, FR-UX-08, AC-ADM-032).
- [ ] T457 [US5] Make pass: T456 complete full-page/partial negotiation in `crates/web/src/console/changes.rs`, `crates/web/src/console/publications.rs`, `crates/web/src/console/settings.rs`, `crates/web/templates/console/change.html`, `crates/web/templates/console/setting.html`; Vary HX-Request where appropriate, no different approval rules (FR-031, FR-UX-08, AC-ADM-032).
- [ ] T458 [US5] Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Confirm AC-ADM-026–032/047 with fake and real composition in `specs/001-admin-foundation/tasks.md`; all delivered screens use the same matrix (FR-CON-11, FR-UX-01, FR-UX-07, FR-UX-08, FR-UX-20).

## Phase 8: User Story 6 — Inspect Disabled Capabilities Before Enablement (P2)

**Goal**: Inspect exactly six OFF flags, honest readiness, and shared reviewed history without
delivering the optional capabilities. Storage and readiness ports were introduced by dependencies.
**Independent test**: AC-ADM-033–036, including failed/unknown/not-delivered enablement and
fixture-delivered/pass through the same workflow; production flags remain OFF/not delivered.

- [ ] T459 [US6] Regression test: [unit] rerun `six_launch_flags_start_off` from T279 in `crates/admin/src/domain/flag.rs` and the seeded fake/real contract runners; require the already implemented six OFF members unchanged. Do not add a duplicate definition, manufacture red or claim optional capability delivery (FR-017, FR-CON-32, AC-ADM-033).
- [ ] T460 [US6] Regression test: [mock] `failed_readiness_writes_no_live_flag_or_success_audit` in `crates/testkit/tests/mock_flag_workflow.rs`; rerun existing exact-revision readiness rechecks: failed/unknown/not-delivered evidence requires zero live-flag writes and zero success-publication audits; permitted typed passing fixtures require exact review and one safe audit. No production registry override, duplicate implementation or manufactured red. Further code requires a genuinely new named flag-specific case and its preceding red/green driver (FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T461 [US6] Regression test: [fake] `flag_changes_use_versioned_review_and_history`, `failed_or_unknown_health_blocks_enablement`, `unbuilt_capability_cannot_be_enabled_by_flag` in `crates/testkit/tests/fake_flag_workflow.rs`; rerun existing flag draft/preview/approval/publish/rollback/history, typed delivered+pass test-fixture success and preserved OFF on failed/unknown/not-delivered checks. Production code-owned registry keeps all six not delivered, no override. No manufactured red or duplicate code; a genuinely new flag-specific case must be named and test-driven first (FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T462 [US6] Regression test: [mock] [fake] rerun T460 T461 against the existing `crates/admin/src/application/preview.rs`, `crates/admin/src/application/approve.rs`, `crates/admin/src/application/publish.rs`; require the previously driven flag-specific evidence policy green, failed/unknown/not-delivered leaves OFF, passing typed test fixtures follow exact review/history, one safe audit and no readiness override. No production repair or manufactured red. A genuinely new flag-specific case must be explicitly named and test-driven before further code; none is introduced here (FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T463 [US6] Setup: prepare real flag service fixtures in `crates/infra-postgres/tests/support/flags.rs` using the existing app/readiness composition from the US3 worker increment; verify actual runtime pools and immutable registered evidence before tests. No production app wiring or fake passing evidence is added (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T464 [US6] Acceptance test: [integration] `real_flag_workflow_preserves_off_and_history_when_readiness_blocks` in `crates/infra-postgres/tests/integration_flag_workflow.rs`; with the prepared real fixture, require the already driven workflow to preserve OFF/history/blocked reasons for unknown/not-delivered readiness; fixture setup failures are blockers, not behavioral red (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T465 [US6] Write failing test: [http] `flag_register_shows_six_off_states_and_truthful_readiness` in `crates/web/tests/flags.rs`; expect absent list/draft/history forms, managed descriptions, impact/health/result/status and blocked-enable explanation across FA/EN/themes (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T466 [US6] Make pass: T465 implement `crates/web/src/console/flags.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/flags.html`, `crates/web/templates/partials/flag_row.html`, both console Fluent files; add flags to US5 matrix inventory, no optional-capability routes or controls (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T467 [US6] Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Confirm AC-ADM-033–036 on fakes AND real adapters in `specs/001-admin-foundation/tasks.md`; flag registration never counts as P0-OFF capability completion (FR-CON-32, FR-CON-37).

## Phase 9: Polish & Cross-Cutting Gates

**Purpose**: Finish full-console composition and dual-backend E2E/axe, feature evidence and
honest full-product release blockers. Never fabricate wallet/media/generation completion.

- [ ] T468 Write failing test: [integration] `fake_console_serve_needs_no_database_minio_or_vendor` in `crates/app/tests/fake_serve.rs`; run `tankhor serve` with debug dev-fakes, no DATABASE_URL/S3/provider credentials and denied external networking; expect the extension of the already runnable foundation to miss a delivered management service or contact an external service; the sandbox/tooling is already prepared (FR-001, FR-006, FR-017, FR-024, FR-CON-01, FR-CON-32).
- [ ] T469 Make pass: T468 extend the existing debug-only foundation composition with complete delivered console/seed/fixture enrollment in `crates/app/src/composition.rs`, `crates/app/src/serve.rs`, `crates/app/Cargo.toml`; normal real startup keeps all required policy/secrets, fake mode requires controlled local fixtures and still exercises primary+second-factor flow (FR-001, FR-006, FR-017, FR-024, FR-CON-01, FR-CON-32).
- [ ] T470 Setup: on the fully migrated isolated database run the final complete-workspace metadata preparation with SQLX_OFFLINE=false in `justfile`, refreshing generated `.sqlx/query-*.json` files; this supplements per-adapter preparation already completed, and never counts as a failing behavioral test (FR-024, FR-CON-37).
- [ ] T471 Setup: run `cargo sqlx prepare --check --workspace -- --all-targets --all-features` on the intended isolated schema/role, then restore SQLX_OFFLINE=true and run offline workspace validation; confirm `.github/workflows/ci.yml` and `justfile` enforce the same final freshness gate and committed `.sqlx/query-*.json` files (FR-024, FR-CON-37).
- [ ] T472 Setup: create/pin `e2e/package.json`, `e2e/package-lock.json` and `e2e/playwright.config.ts`; install Playwright/browsers and verify runner discovery before behavioral tests. A missing package/browser is a setup blocker (FR-001, FR-CON-01).
- [ ] T473 Setup: install/pin axe in `e2e/package.json` and `e2e/package-lock.json`; validate local axe loading in a harmless harness page before application accessibility tests. Tool load failures are not red application evidence (FR-029, FR-UX-08, AC-ADM-030).
- [ ] T474 Setup: prepare isolated fake/real projects and synthetic crypto/SMS fixtures in `e2e/fixtures.ts` and `e2e/playwright.config.ts`; verify loopback services, outbound denial, virtual authenticators and cleanup separately before acceptance. No app source or authentication bypass changes (FR-002, FR-024, FR-CON-02, AC-ADM-002, AC-ADM-024).
- [ ] T475 Acceptance test: [e2e] `J5_foundation_safe_change_and_recovery` in `e2e/admin-foundation.spec.ts`; run the same suite for fake and real adapter projects: OTP+passkey/TOTP, safe upload-size draft/live separation, preview/approve/publish/audit/rollback, another-admin recovery/old-cookie denial, unknown lifetime impact and six OFF flags; require the already implemented business flow green with the prepared executable harness; if a behavioral defect appears, stop and add a focused layer-appropriate failing test before correcting app code; never manufacture a failure (FR-001, FR-002, FR-004, FR-008, FR-009, FR-013, FR-014, FR-017, FR-024, FR-CON-05, AC-ADM-001, AC-ADM-002, AC-ADM-005, AC-ADM-010, AC-ADM-011, AC-ADM-012, AC-ADM-016, AC-ADM-019, AC-ADM-021, AC-ADM-025, AC-ADM-033, AC-ADM-036, AC-ADM-043).
- [ ] T476 Write failing test: [e2e] `browser_dialog_returns_focus_without_traps` in `e2e/browser_focus.spec.ts`; expect opening/closing/canceling the real dialog to lose or trap keyboard focus under strict CSP (FR-029, FR-UX-08, AC-ADM-030).
- [ ] T477 Make pass: T476 implement only local dialog keyboard/focus-return behavior and its asset hook in crates/web/templates/partials/dialog.html in `crates/web/static/dialog.js`; edit only this behavior and its module registration (FR-029, FR-UX-08, AC-ADM-030).
- [ ] T478 Write failing test: [e2e] `rendered_console_text_and_focus_meet_unrounded_contrast` in `e2e/browser_contrast.spec.ts`; expect rendered text/background/control/focus combinations to violate 4.5/3/3 ratios despite declared token checks (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-029, AC-ADM-047).
- [ ] T479 Make pass: T478 correct only rendered token-role usage; do not invent colour literals or relax thresholds in `crates/web/static/console.css`; edit only this behavior and its module registration (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-029, AC-ADM-047).
- [ ] T480 Write failing test: [e2e] `console_reflows_without_clipping_at_supported_viewports` in `e2e/browser_reflow.spec.ts`; expect delivered forms/tables/dialogs to clip actionable content at 360/768/1024/1440 and zoom/reflow (FR-025, FR-027, FR-029, FR-UX-08, AC-ADM-026, AC-ADM-028, AC-ADM-030).
- [ ] T481 Make pass: T480 correct only responsive reflow/layout behavior using generated tokens and logical properties in `crates/web/static/console.css`; edit only this behavior and its module registration (FR-025, FR-027, FR-029, FR-UX-08, AC-ADM-026, AC-ADM-028, AC-ADM-030).
- [ ] T482 Acceptance test: [e2e] `console_locale_theme_keyboard_and_axe_matrix` in `e2e/accessibility.spec.ts`; run FA/EN x light/dark/system and 360/768/1024/1440 on both backend projects, with safe unsaved preferences, bidi/digits, no-JS TOTP, no-htmx management, reduced motion, keyboard/assistive semantics and axe. Require zero critical findings/no known AA failures; never use a missing harness as red (FR-025, FR-026, FR-027, FR-028, FR-029, FR-030, FR-031, FR-038, FR-UX-01, FR-UX-07, FR-UX-08, FR-UX-20, AC-ADM-026, AC-ADM-027, AC-ADM-028, AC-ADM-029, AC-ADM-030, AC-ADM-031, AC-ADM-032, AC-ADM-047). In the same `e2e/accessibility.spec.ts` task add `swagger_initializes_under_strict_csp_without_external_requests` to both backend projects: sign in through the delivered flow; install securitypolicyviolation listeners and attempted-request capture before navigating to /api/docs; follow local redirects, prove actual Swagger initialization and same-origin /api/v1/openapi.json retrieval matching the generated schema, and a rendered health operation. Require zero CSP violations/off-origin attempted requests (including blocked attempts), no auth bypass and no CSP relaxation (FR-001, FR-CON-01).
- [ ] T483 Write failing test: [unit] `quality_gates_enforce_decided_coverage_without_invented_thresholds` in `scripts/tests/test_quality_gates.py`; expect missing ≥90% present-kernel/wallet/generation/identity gate, missing nightly wallet/generation mutations, unreviewed survivors or proposed 80/85 promoted to requirements (FR-001, FR-CON-37; constitution I).
- [ ] T484 Make pass: T483 implement `scripts/check-quality-gates.py`, `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`; scoped 001 evidence includes kernel, missing future crates remain explicitly undelivered, mutation survivors recorded/reviewed (FR-001, FR-CON-37).
- [ ] T485 Write failing test: [unit] `full_product_release_blocks_missing_GR1_to_GR9_and_J1_to_J5` in `scripts/tests/test_release_readiness.py`; expect absent owning-feature tests/contexts/legal-accounting/name/asset signoffs to permit full release; guardrail inventory uses exact GR-prefixed names from contracts/test-plan §3 (FR-011, FR-CON-32, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).
- [ ] T486 Make pass: T485 implement `scripts/check-release-readiness.py`, `.github/workflows/release-readiness.yml`; distinguish foundation acceptance from full-product release, block missing evidence rather than add empty or ignored tests, run delivered E2E nightly/pre-release (FR-011, FR-CON-32, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).
- [ ] T487 Write failing test: [unit] `operator_docs_cover_real_fake_seed_security_and_device_acceptance` in `scripts/tests/test_development_docs.py`; expect missing exact commands, protected inputs, fake-production/release exclusions, ten defaults/six OFF review or physical iOS/Android/assistive evidence form (FR-006, FR-017, FR-029, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-030, AC-ADM-033).
- [ ] T488 Make pass: T487 update `docs/development.md`, `specs/001-admin-foundation/quickstart.md`, create `specs/001-admin-foundation/checklists/acceptance.md`; document tested fake/real console commands, seed/security-policy setup, actual-device/assistive/token-freeze evidence requirements; leave evidence unclaimed until observed (FR-006, FR-017, FR-029, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-030, AC-ADM-033).
- [ ] T489 Write failing test: [integration] `deployment_uses_release_binary_and_restricted_runtime_identity` in `scripts/tests/test_deployment.py`; expect missing reproducible image, migration/runtime command distinction or test features/keys in release layers (FR-001, FR-024, FR-CON-01, FR-CON-37).
- [ ] T490 Make pass: T489 create `deploy/Dockerfile`, update `deploy/compose.yml` for separately invoked migration/seed/serve/worker commands and protected runtime configuration; no automatic fake fallback or vendor calls in image tests (FR-001, FR-024, FR-CON-01, FR-CON-37).
- [ ] T491 Verify checkpoint: Run the checkpoint commands; all green; report evidence and stop. The owner commits. Commands include `cargo nextest run` (and `--profile integration` where relevant). Run dual-backend E2E/axe, fmt/clippy/deny, architecture/traceability, kernel coverage, SQLx/OpenAPI diffs and record actual evidence in `specs/001-admin-foundation/tasks.md`, `specs/001-admin-foundation/checklists/acceptance.md`; full-product release remains blocked by undelivered owning features (FR-CON-01, FR-CON-32, FR-CON-37, FR-UX-20, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9). Run `python3 scripts/check-test-traceability.py --mode final`: require actual trait/fake/real adapter/shared fake-and-real runners/direct mock interactions for all sixteen ports, both SmsSender adapters and real/fake feature graphs; fail missing evidence, never waive it because partial mode passed.

## Dependencies & Execution Order

### Phase and story graph

```text
Setup (1) → Foundation (2) → US1 access/audit base (3)
  → US2 settings/shared review+flags storage (4)
  → US3 publication/readiness (5)
  → US4 audit investigation (6) → US5 presentation completion (7)
  → US6 flag enablement controls (8) → Polish/gates (9)
```

- US1 has no business-story dependency after foundation. US2 needs US1 attribution/access.
- US3 requires US2 registry/review/impact and already-audited UoW; US4 reuses US1 audit storage
  and validates US3's complete event set. US5 extends all delivered routes; US6 joins its matrix.
- Audit, flag storage, base messages and shell are legitimately shared foundations; their
  early introduction does not claim later user-facing stories are complete.
- Before any real adapter in a story, finish that port's fake contract AND all service red/green
  pairs in that story. Kernel transaction/system adapters are foundational, not permission to
  write later admin PostgreSQL code before that story's fake-first tests.
- A checkpoint runs only after every preceding pair is green. Run the checkpoint commands;
  all green; report evidence and stop. The owner commits.
- Physical device/assistive acceptance and production credentials/assets are external evidence
  gates: blocked evidence is recorded as blocked, never represented by passing mocks.

### Original ten-workstream mapping

| Requested workstream | Story-first location |
| --- | --- |
| Workspace & CI | Phase 1, completed quality/release wiring in Phase 9 |
| Kernel/testkit | Phase 2; store extends with each story's ports |
| Admin domain | Pure rules first inside US1/US2/US3/US4/US6 |
| Ports + fake/mock application services | Each introducing story before its real adapters |
| PostgreSQL | Kernel UoW in Phase 2; domain migrations/adapters inside introducing story |
| Admin auth | US1, with primary phone/SMS as required by the plan/Q09 |
| Web shell/tokens | Base in Phase 2, every story's templates, complete matrix in US5 |
| JSON + OpenAPI | Phase 2, auth wrapping in US1, final regenerated snapshot/diff gate |
| Composition/seed/scheduled publish/impact | Runnable fake shell/health plus config/guards in Phase 2; access US1; seed/impact US2; worker US3; full fake console regression Phase 9 |
| E2E and gates | Phase 9; per-story fake/real checkpoints remain mandatory |

## Parallel Examples by User Story

**Marked task parallelism**: T033 and T034 can run together after Phase 1; different test
files, no mutual dependency. T035/T036 and all manifest/module registration remain serial.

One implementation task is owned at a time under AGENTS.md. The examples below describe
independent test invocations after the cited implementation dependencies are green, not
permission to work on overlapping production files or run a later red test prematurely.

| Story | Parallel validation example | Required completed work |
| --- | --- | --- |
| US1 | `cargo nextest run -p infra-auth` alongside `cargo nextest run -p infra-sms` | T224, T228; separate local-crypto and wiremock resources |
| US2 | `cargo nextest run -p web -E 'test(settings)'` alongside `cargo nextest run -p admin` | T300, T360; pure domain validation, no common mutable fixture |
| US3 | `cargo nextest run -p worker` alongside `cargo nextest run -p web -E 'test(publication)'` | T403, T392, T407; no shared database run |
| US4 | `cargo nextest run -p web -E 'test(audit)'` alongside repository-check Python tests | T433, T435, T437; disjoint read-only validation |
| US5 | `cargo nextest run -p web -E 'test(fluent)'` alongside a token-only unit test selection | T444, T446, T448, T450, T452, T454; no browser/storage fixtures |
| US6 | fake-workflow test binary alongside HTTP flag test binary | T462, T466; separate FakeStore instances |

Never run independent SQLx integration invocations concurrently against the same shared local
server; the live-run lease must reject that. E2E fake/real projects get isolated backends.

## Implementation Strategy

1. Prepare executable tooling/harnesses as setup, never as failing business tests. Drive
   independently testable fixture-policy behavior through focused red/green pairs. Work only on the next unchecked red/green pair.
2. Complete shared foundation, then US1 as the smallest admin-access MVP. Demonstrate secure
   console access/invitation/recovery in both backend modes, not settings that do not yet exist.
3. Deliver US2, then US3, so management becomes useful without weakening approval/audit.
4. Complete investigation, all-surface accessibility and safe flag controls in US4–US6.
5. Validate the complete foundation on fakes and real SQL/crypto/wiremock; report scoped J5,
   not full J5 or J1–J5. No vendor service, source code, deployment or commit is executed by
   this task-generation operation.

## Remediation Record — Historical IDs → Current IDs

The left column refers only to the pre-remediation 368-task revision. Current references outside historical-map left columns use the right-column IDs,
composed through the second remediation and the owner consolidation below. Setup/acceptance replacements
are intentional: they do not claim a tooling failure is a behavioral red.

| Old task ID | Current task ID(s) | Treatment |
| --- | --- | --- |
| old T001 | T002 | renumbered/retained |
| old T002 | T003 | renumbered/retained |
| old T003 | T004 | renumbered/retained |
| old T004 | T005 | renumbered/retained |
| old T005 | T006 | renumbered/retained |
| old T006 | T007 | renumbered/retained |
| old T007 | T008 | renumbered/retained |
| old T008 | T009 | renumbered/retained |
| old T009 | T011, T013, feature 004 (deferred), T015, T017 | Deferred MinIO assertions to feature 004; no 001 coverage |
| old T010 | T012, T014, feature 004 (deferred), T016, T018 | Deferred MinIO assertions to feature 004; no 001 coverage |
| old T011 | T019 | renumbered/retained |
| old T012 | T020 | renumbered/retained |
| old T013 | T022 | renumbered/retained |
| old T014 | T023 | renumbered/retained |
| old T015 | feature 004 (deferred) | Deferred MinIO assertions to feature 004; no 001 coverage |
| old T016 | feature 004 (deferred) | Deferred MinIO assertions to feature 004; no 001 coverage |
| old T017 | T024 | renumbered/retained |
| old T018 | T025 | renumbered/retained |
| old T019 | T026 | renumbered/retained |
| old T020 | T027 | renumbered/retained |
| old T021 | T028 | renumbered/retained |
| old T022 | T029 | renumbered/retained |
| old T023 | T030 | renumbered/retained |
| old T024 | T031 | renumbered/retained |
| old T025 | T032 | renumbered/retained |
| old T026 | T033 | renumbered/retained |
| old T027 | T034 | renumbered/retained |
| old T028 | T035 | renumbered/retained |
| old T029 | T036 | renumbered/retained |
| old T030 | T037 | renumbered/retained |
| old T031 | T038 | renumbered/retained |
| old T032 | T039 | renumbered/retained |
| old T033 | T040 | renumbered/retained |
| old T034 | T041 | renumbered/retained |
| old T035 | T042 | renumbered/retained |
| old T036 | T043 | renumbered/retained |
| old T037 | T044 | renumbered/retained |
| old T038 | T045 | renumbered/retained |
| old T039 | T046 | renumbered/retained |
| old T040 | T047 | renumbered/retained |
| old T041 | T048 | renumbered/retained |
| old T042 | T049 | renumbered/retained |
| old T043 | T050 | renumbered/retained |
| old T044 | T051 | renumbered/retained |
| old T045 | T052 | renumbered/retained |
| old T046 | T053 | renumbered/retained |
| old T047 | T054 | renumbered/retained |
| old T048 | T055, T057 | split |
| old T049 | T056, T058 | split |
| old T050 | T059 | renumbered/retained |
| old T051 | T060 | renumbered/retained |
| old T052 | T061 | renumbered/retained |
| old T053 | T062 | renumbered/retained |
| old T054 | T063 | renumbered/retained |
| old T055 | T064 | renumbered/retained |
| old T056 | T065 | renumbered/retained |
| old T057 | T066 | renumbered/retained |
| old T058 | T067 | renumbered/retained |
| old T059 | T068 | renumbered/retained |
| old T060 | T069 | renumbered/retained |
| old T061 | T070 | renumbered/retained |
| old T062 | T071 | renumbered/retained |
| old T063 | T072 | renumbered/retained |
| old T064 | T073 | renumbered/retained |
| old T065 | T074 | renumbered/retained |
| old T066 | T075 | renumbered/retained |
| old T067 | T076 | renumbered/retained |
| old T068 | T077 | renumbered/retained |
| old T069 | T078 | renumbered/retained |
| old T070 | T079 | renumbered/retained |
| old T071 | T080 | renumbered/retained |
| old T072 | T081 | renumbered/retained |
| old T073 | T082 | renumbered/retained |
| old T074 | T083 | renumbered/retained |
| old T075 | T084 | renumbered/retained |
| old T076 | T085 | renumbered/retained |
| old T077 | T086 | renumbered/retained |
| old T078 | T087 | renumbered/retained |
| old T079 | T088 | renumbered/retained |
| old T080 | T089 | renumbered/retained |
| old T081 | T090 | renumbered/retained |
| old T082 | T097 | renumbered/retained |
| old T083 | T098 | renumbered/retained |
| old T084 | T099 | renumbered/retained |
| old T085 | T100 | renumbered/retained |
| old T086 | T104 | renumbered/retained |
| old T087 | T105 | renumbered/retained |
| old T088 | T106 | renumbered/retained |
| old T089 | T107 | renumbered/retained |
| old T090 | T108 | renumbered/retained |
| old T091 | T109, T111, T113, T115, T117 | split |
| old T092 | T110, T112, T114, T116, T118 | split |
| old T093 | T119 | renumbered/retained |
| old T094 | T120 | renumbered/retained |
| old T095 | T121 | renumbered/retained |
| old T096 | T122 | renumbered/retained |
| old T097 | T123 | renumbered/retained |
| old T098 | T124 | renumbered/retained |
| old T099 | T125 | renumbered/retained |
| old T100 | T126 | renumbered/retained |
| old T101 | T127 | renumbered/retained |
| old T102 | T128 | renumbered/retained |
| old T103 | T129 | renumbered/retained |
| old T104 | T130 | renumbered/retained |
| old T105 | T131 | renumbered/retained |
| old T106 | T132 | renumbered/retained |
| old T107 | T133 | renumbered/retained |
| old T108 | T134 | renumbered/retained |
| old T109 | T131 | renumbered/retained |
| old T110 | T132 | renumbered/retained |
| old T111 | T133 | renumbered/retained |
| old T112 | T134 | renumbered/retained |
| old T113 | T135 | renumbered/retained |
| old T114 | T136 | renumbered/retained |
| old T115 | T137 | renumbered/retained |
| old T116 | T138 | renumbered/retained |
| old T117 | T139 | renumbered/retained |
| old T118 | T140 | renumbered/retained |
| old T119 | T141 | renumbered/retained |
| old T120 | T142 | renumbered/retained |
| old T121 | T143 | renumbered/retained |
| old T122 | T144 | renumbered/retained |
| old T123 | T145 | renumbered/retained |
| old T124 | T146 | renumbered/retained |
| old T125 | T147 | renumbered/retained |
| old T126 | T148 | renumbered/retained |
| old T127 | T149 | renumbered/retained |
| old T128 | T150 | renumbered/retained |
| old T129 | T151 | renumbered/retained |
| old T130 | T152 | renumbered/retained |
| old T131 | T153 | renumbered/retained |
| old T132 | T154 | renumbered/retained |
| old T133 | T155 | renumbered/retained |
| old T134 | T156 | renumbered/retained |
| old T135 | T157 | renumbered/retained |
| old T136 | T158 | renumbered/retained |
| old T137 | T159 | renumbered/retained |
| old T138 | T160 | renumbered/retained |
| old T139 | T161 | renumbered/retained |
| old T140 | T162 | renumbered/retained |
| old T141 | T163, T166, T169, T172 | split |
| old T142 | T164, T167, T170, T173 | split |
| old T143 | T165, T168, T171, T174 | split |
| old T144 | T175, T178 | split |
| old T145 | T176, T179 | split |
| old T146 | T177, T180 | split |
| old T147 | T181 | renumbered/retained |
| old T148 | T182 | renumbered/retained |
| old T149 | T183 | renumbered/retained |
| old T150 | T184, T187, T190 | split |
| old T151 | T185, T188, T191 | split |
| old T152 | T186, T189, T192 | split |
| old T153 | T199 | renumbered/retained |
| old T154 | T200 | renumbered/retained |
| old T155 | T201 | renumbered/retained |
| old T156 | T202 | renumbered/retained |
| old T157 | T203 | renumbered/retained |
| old T158 | T204 | renumbered/retained |
| old T159 | T205 | renumbered/retained |
| old T160 | T206 | renumbered/retained |
| old T161 | T207 | renumbered/retained |
| old T162 | T208 | renumbered/retained |
| old T163 | T209 | renumbered/retained |
| old T164 | T210 | renumbered/retained |
| old T165 | T209 | renumbered/retained |
| old T166 | T210 | renumbered/retained |
| old T167 | T211 | renumbered/retained |
| old T168 | T212 | renumbered/retained |
| old T169 | T213 | renumbered/retained |
| old T170 | T214 | renumbered/retained |
| old T171 | T215 | renumbered/retained |
| old T172 | T216 | renumbered/retained |
| old T173 | T217 | renumbered/retained |
| old T174 | T218 | renumbered/retained |
| old T175 | T219 | renumbered/retained |
| old T176 | T220 | renumbered/retained |
| old T177 | T221 | renumbered/retained |
| old T178 | T222 | renumbered/retained |
| old T179 | T223 | renumbered/retained |
| old T180 | T224 | renumbered/retained |
| old T181 | T225 | renumbered/retained |
| old T182 | T226 | renumbered/retained |
| old T183 | T227 | renumbered/retained |
| old T184 | T228 | renumbered/retained |
| old T185 | T157–T158, T160–T161, T181–T182, T157–T158, T230, T231, T232, T233 | split |
| old T186 | T157–T158, T162, T183, T157–T158 | split; service correctness moved before story SQL |
| old T187 | T234, T236, T238, T240, T242, T244, T246 | split |
| old T188 | T235, T237, T239, T241, T243, T245, T247 | split |
| old T189 | T248, T250, T252, T254, T256, T258, T260, T262, T264 | split |
| old T190 | T249, T251, T253, T255, T257, T259, T261, T263, T265 | split |
| old T191 | T266 | renumbered/retained |
| old T192 | T267 | renumbered/retained |
| old T193 | T268 | renumbered/retained |
| old T194 | T269 | renumbered/retained |
| old T195 | T270 | renumbered/retained |
| old T196 | T271 | renumbered/retained |
| old T197 | T272 | renumbered/retained |
| old T198 | T273 | renumbered/retained |
| old T199 | T274 | renumbered/retained |
| old T200 | T275 | renumbered/retained |
| old T201 | T276 | renumbered/retained |
| old T202 | T277 | renumbered/retained |
| old T203 | T278 | renumbered/retained |
| old T204 | T281, T283, T285, T287 | split |
| old T205 | T282, T284, T286, T288 | split |
| old T206 | T289 | renumbered/retained |
| old T207 | T290 | renumbered/retained |
| old T208 | T291 | renumbered/retained |
| old T209 | T292 | renumbered/retained |
| old T210 | T293 | renumbered/retained |
| old T211 | T294 | renumbered/retained |
| old T212 | T295 | renumbered/retained |
| old T213 | T296 | renumbered/retained |
| old T214 | T297 | renumbered/retained |
| old T215 | T298 | renumbered/retained |
| old T216 | T299 | renumbered/retained |
| old T217 | T300 | renumbered/retained |
| old T218 | T303 | renumbered/retained |
| old T219 | T304 | renumbered/retained |
| old T220 | T305 | renumbered/retained |
| old T221 | T306 | renumbered/retained |
| old T222 | T307 | renumbered/retained |
| old T223 | T308 | renumbered/retained |
| old T224 | T309 | renumbered/retained |
| old T225 | T310 | renumbered/retained |
| old T226 | T311 | renumbered/retained |
| old T227 | T312 | renumbered/retained |
| old T228 | T313 | renumbered/retained |
| old T229 | T314 | renumbered/retained |
| old T230 | T315 | renumbered/retained |
| old T231 | T316 | renumbered/retained |
| old T232 | T285 | renumbered/retained |
| old T233 | T286 | renumbered/retained |
| old T234 | T285, T287, T289, T291, T293, T295, T297, T299 | renumbered/retained |
| old T235 | T286, T288, T290, T292, T294, T296, T298, T300 | renumbered/retained |
| old T236 | T317 | renumbered/retained |
| old T237 | T318 | renumbered/retained |
| old T238 | T319 | renumbered/retained |
| old T239 | T320, T323, T326, T329, T332 | split |
| old T240 | T321, T324, T327, T330, T333 | split |
| old T241 | T322, T325, T328, T331, T334 | split |
| old T242 | T335 | renumbered/retained |
| old T243 | T336 | renumbered/retained |
| old T244 | T337 | renumbered/retained |
| old T245 | T338 | renumbered/retained |
| old T246 | T339 | renumbered/retained |
| old T247 | T340 | renumbered/retained |
| old T248 | T341, T343, T345 | split |
| old T249 | T342, T344, T346 | split |
| old T250 | T347 | renumbered/retained |
| old T251 | T348 | renumbered/retained |
| old T252 | T349 | renumbered/retained |
| old T253 | T350 | renumbered/retained |
| old T254 | T351 | renumbered/retained |
| old T255 | T352 | renumbered/retained |
| old T256 | T353 | renumbered/retained |
| old T257 | T354 | renumbered/retained |
| old T258 | T285, T287, T289, T291, T293, T295, T297, T299 | renumbered/retained |
| old T259 | T286, T288, T290, T292, T294, T296, T298, T300 | renumbered/retained |
| old T260 | T355 | renumbered/retained |
| old T261 | T356 | renumbered/retained |
| old T262 | T358 | renumbered/retained |
| old T263 | T357 | fixture setup before real acceptance; unscoped production repair removed |
| old T264 | T359 | renumbered/retained |
| old T265 | T360 | renumbered/retained |
| old T266 | T361 | renumbered/retained |
| old T267 | T362 | renumbered/retained |
| old T268 | T363 | renumbered/retained |
| old T269 | T364 | renumbered/retained |
| old T270 | T365 | renumbered/retained |
| old T271 | T366 | renumbered/retained |
| old T272 | T367 | renumbered/retained |
| old T273 | T368 | renumbered/retained |
| old T274 | T369 | renumbered/retained |
| old T275 | T370 | renumbered/retained |
| old T276 | T371 | renumbered/retained |
| old T277 | T301 | renumbered/retained |
| old T278 | T302 | renumbered/retained |
| old T279 | T301 | renumbered/retained |
| old T280 | T302 | renumbered/retained |
| old T281 | T372 | renumbered/retained |
| old T282 | T373 | renumbered/retained |
| old T283 | T374 | renumbered/retained |
| old T284 | T375 | renumbered/retained |
| old T285 | T376 | renumbered/retained |
| old T286 | T377 | renumbered/retained |
| old T287 | T378, T381 | split |
| old T288 | T379, T382 | split |
| old T289 | T380, T383 | split |
| old T290 | T384 | renumbered/retained |
| old T291 | T385 | renumbered/retained |
| old T292 | T386 | renumbered/retained |
| old T293 | T387 | renumbered/retained |
| old T294 | T388 | renumbered/retained |
| old T295 | T389 | renumbered/retained |
| old T296 | T393 | renumbered/retained |
| old T297 | T394 | renumbered/retained |
| old T298 | T396 | renumbered/retained |
| old T299 | T397 | renumbered/retained |
| old T300 | T301 | renumbered/retained |
| old T301 | T302 | renumbered/retained |
| old T302 | T370, T375–T376, T396, T366, T370, T396, T366, T370, T396 | split |
| old T303 | T371, T377, T397, T367, T371, T397, T367, T371, T397 | split |
| old T304 | T370, T385, T396, T370, T375–T376, T385, T396, T370, T385, T396 | split |
| old T305 | T371, T386, T397, T371, T377, T386, T397, T371, T386, T397 | split |
| old T306 | T375, T376, T398, T399, T400 | split |
| old T307 | T377 | unknown-outcome service behavior moved before story SQL; integration checks separated |
| old T308 | T402 | renumbered/retained |
| old T309 | T403 | renumbered/retained |
| old T310 | T404 | renumbered/retained |
| old T311 | T405 | renumbered/retained |
| old T312 | T391, T406 | split |
| old T313 | T392, T407 | split |
| old T314 | T408 | renumbered/retained |
| old T315 | T409 | renumbered/retained |
| old T316 | T410 | renumbered/retained |
| old T317 | T411 | renumbered/retained |
| old T318 | T412 | renumbered/retained |
| old T319 | T413 | renumbered/retained |
| old T320 | T414, T416, T157–T158, T160–T161, T166–T167, T172–T173, T175–T176, T181–T182, T335–T336, T338–T339, T372–T373, T378–T379, T375–T376, T387–T388, T419, T420, T421, T422, T423, T424, T425, T426, T427, T428, T429, T430, T431 | split |
| old T321 | T415, T417, T159, T162, T168, T174, T177, T183, T337, T340, T374, T380, T377, T389 | split |
| old T322 | T432 | renumbered/retained |
| old T323 | T433 | renumbered/retained |
| old T324 | T434, T436 | split |
| old T325 | T435, T437 | split |
| old T326 | T438 | renumbered/retained |
| old T327 | T439 | renumbered/retained |
| old T328 | T440 | renumbered/retained |
| old T329 | T441 | renumbered/retained |
| old T330 | T442 | renumbered/retained |
| old T331 | T443 | renumbered/retained |
| old T332 | T444 | renumbered/retained |
| old T333 | T445, T447, T449, T451, T453 | split |
| old T334 | T446, T448, T450, T452, T454 | split |
| old T335 | T456 | renumbered/retained |
| old T336 | T457 | renumbered/retained |
| old T337 | T458 | renumbered/retained |
| old T338 | T279 | moved to US2 before first flag consumer; US6 regression added separately |
| old T339 | T280 | moved to US2 before first flag consumer; US6 regression added separately |
| old T340 | T301 | renumbered/retained |
| old T341 | T302 | renumbered/retained |
| old T342 | T460 | renumbered/retained |
| old T343 | T461 | renumbered/retained |
| old T344 | T462 | renumbered/retained; T462 is now regression-only; implementation remains in original workflow slices |
| old T345 | T464 | renumbered/retained |
| old T346 | T463 | fixture setup moved before real acceptance; redundant production wiring removed |
| old T347 | T465 | renumbered/retained |
| old T348 | T466 | renumbered/retained |
| old T349 | T467 | renumbered/retained |
| old T350 | T468 | renumbered/retained |
| old T351 | T469 | renumbered/retained |
| old T352 | T030 | renumbered/retained |
| old T353 | T031 | renumbered/retained |
| old T354 | T470 | final metadata setup/freshness; not behavioral red/implementation |
| old T355 | T471 | final metadata setup/freshness; not behavioral red/implementation |
| old T356 | T475 | renumbered/retained |
| old T357 | T472, T474 | browser setup moved before E2E behavior |
| old T358 | T476, T478, T480, T482 | split |
| old T359 | T473, T477, T479, T481 | split |
| old T360 | T483 | renumbered/retained |
| old T361 | T484 | renumbered/retained |
| old T362 | T485 | renumbered/retained |
| old T363 | T486 | renumbered/retained |
| old T364 | T487 | renumbered/retained |
| old T365 | T488 | renumbered/retained |
| old T366 | T489 | renumbered/retained |
| old T367 | T490 | renumbered/retained |
| old T368 | T491 | renumbered/retained |

## Second Remediation ID Map — 576-task Baseline

Left-column IDs below are historical IDs from the analysis, not current references. Six
service tasks were added; two redundant test/repair pairs were merged into regressions.
Total: **576 → 580** (+4; cap +15). Historical tests **297 → 301**, implementation **237 → 237**,
setup **33 → 33**, checkpoints **9 → 9**. The older 368-task map above is composed forward.

| Previous ID(s) | Current ID(s) | Treatment |
| --- | --- | --- |
| previous T001–T204 | T001–T192 | Retained IDs; focused descriptions updated where needed |
| New C1 slice | T193–T195 | BootstrapAdmin mock, fake, application implementation |
| New U1 slice | T196–T198 | GetEnrollment mock, fake, application implementation |
| previous T205–T206 | T157–T158 | Merged passkey-state regression; no production repair |
| previous T207–T210 | T160–T162, T181–T183 | Renumbered, unchanged behavior |
| previous T211–T212 | T157–T158 | Merged audit-failure regression; no production repair |
| previous T213–T576 | T030–T031, T157–T162, T166–T168, T172–T177, T181–T183, T199–T491 | Renumbered by +4; focused edits retain scope |
| previous T277–T278 | T254–T255 | Web delegates to GetEnrollment |
| previous T295–T296 | T272–T273 | CLI composition test and delegation only |
| previous T310–T311 | T287–T288 | Existing pure impact-comparison driver/implementation |
| previous T567 | T482 | Existing E2E task includes strict-CSP Swagger assertion |

## Traceability — AC/FR → Driving Tests → Implementation

The tables are completeness checks. **Missing coverage flags: none** for the 47 local ACs,
38 local FRs or sixteen dependency ports. These links specify future work, not passing tests.
Prerequisite implementations are included where an AC spans domain/service/presentation.

U3 traceability extends these existing rows without adding requirements or task IDs:

| Existing requirement / assertion | Existing driving tests | Existing implementation |
| --- | --- | --- |
| FR-004 / AC-ADM-043 / SC-001 — durable monotonic epoch on every admin revocation/deactivation/recovery; no reset/reuse on restoration | T109, T123, T125, T205 | T110, T124, T126, T206 |
| FR-013 / FR-037 / AC-ADM-016–018/045 / SC-002 — capture current epoch; deactivate→reactivate or revoke→restore before execution cancels; unchanged epoch plus all gates passes | T366, T378–T379, T384–T385, T396 | T367, T380, T386, T397 |
| FR-021 / FR-024 / SC-005 — epoch-mismatch cancellation is terminal, visibly explained and audited once, or rolled back with no live/success effect | T384–T385, T396 | T386, T397 |

| AC/FR ID | Test task ID(s) | Implementation task ID(s) |
| --- | --- | --- |
| AC-ADM-001 / FR-001 | T184–T185, T190–T191, T234, T236, T238, T240, T242, T244, T266, T270, T475 | T186, T192, T235, T237, T239, T241, T243, T245, T267, T271 |
| AC-ADM-002 / FR-002 | T111, T113, T115, T117, T129, T133, T137, T141, T145, T149, T151–T152, T154–T155, T157–T158, T160–T161, T190–T191, T196–T197, T199, T207, T209, T211, T213, T219, T221, T223, T230–T231, T234, T236, T238, T240, T242, T244, T246, T268, T270, T475 | T112, T114, T116, T118, T130, T134, T138, T142, T146, T150, T153, T156, T159, T162, T192, T198, T200, T208, T210, T212, T214, T220, T222, T224, T235, T237, T239, T241, T243, T245, T247, T269, T271 |
| AC-ADM-003 / FR-003 | T113, T133, T184–T185, T187–T188, T209, T234, T236, T238, T240, T242, T244, T266 | T114, T134, T186, T189, T210, T235, T237, T239, T241, T243, T245, T267 |
| AC-ADM-004 / FR-004 | T125, T163–T164, T166–T167, T169–T170, T172–T173, T175–T176, T193–T194, T196–T197, T205, T248, T250, T252, T254, T256, T258, T260, T262, T264, T272 | T126, T165, T168, T171, T174, T177, T195, T198, T206, T249, T251, T253, T255, T257, T259, T261, T263, T265, T273 |
| AC-ADM-005 / FR-004 | T115, T163–T164, T166–T167, T169–T170, T172–T173, T182, T196–T197, T248, T250, T252, T254, T256, T258, T260, T262, T264, T475 | T116, T165, T168, T171, T174, T183, T198, T249, T251, T253, T255, T257, T259, T261, T263, T265 |
| AC-ADM-006 / FR-004 | T115, T182, T193–T194, T248, T250, T252, T254, T256, T258, T260, T262, T264, T272 | T116, T183, T195, T249, T251, T253, T255, T257, T259, T261, T263, T265, T273 |
| AC-ADM-007 / FR-005 | T109, T125, T178–T179, T205, T248, T250, T252, T254, T256, T258, T260, T262, T264 | T110, T126, T180, T206, T249, T251, T253, T255, T257, T259, T261, T263, T265 |
| AC-ADM-008 / FR-006 | T275, T307, T317–T318, T320–T321, T326–T327, T329–T330, T341, T345, T349, T358–T359, T363, T487 | T276, T308, T319, T322, T328, T331, T342, T346, T350, T360, T364, T488 |
| AC-ADM-009 / FR-007 | T275, T307, T326–T327, T335, T349, T359 | T276, T308, T328, T337, T350, T360 |
| AC-ADM-010 / FR-008 | T281, T315, T336, T347, T353, T358–T359, T475 | T282, T316, T337, T348, T354, T360 |
| AC-ADM-011 / FR-009 | T285, T287, T289, T291, T293, T295, T297, T299, T332–T333, T338–T339, T361, T475 | T286, T288, T290, T292, T294, T296, T298, T300, T334, T340, T362 |
| AC-ADM-012 / FR-010 | T285, T287, T289, T291, T293, T295, T297, T299, T338–T339, T361, T475 | T286, T288, T290, T292, T294, T296, T298, T300, T340, T362 |
| AC-ADM-013 / FR-011 | T303, T335 | T304, T337 |
| AC-ADM-014 / FR-012 | T281, T283, T285, T287, T315, T332–T333, T335–T336, T347, T353, T366, T372–T373, T378–T379, T384–T385, T393, T396, T402 | T282, T284, T286, T288, T316, T334, T337, T348, T354, T367, T374, T380, T386, T394, T397, T403 |
| AC-ADM-015 / FR-016 | T281, T315, T335–T336, T347, T353, T358–T359 | T282, T316, T337, T348, T354, T360 |
| AC-ADM-016 / FR-013 | T281, T366, T370, T375–T376, T378–T379, T381–T382, T384–T385, T393, T396, T402, T475 | T282, T367, T371, T377, T380, T383, T386, T394, T397, T403 |
| AC-ADM-017 / FR-013 | T281, T366, T370, T375–T376, T378–T379, T381–T382, T384–T385, T393, T396, T402 | T282, T367, T371, T377, T380, T383, T386, T394, T397, T403 |
| AC-ADM-018 / FR-013 / FR-037 | T370, T375–T376, T384–T385, T391, T393, T396, T398–T400, T406 | T371, T377, T386, T392, T394, T397, T407 |
| AC-ADM-019 / FR-014 | T329–T330, T387–T388, T404, T475 | T331, T389, T405 |
| AC-ADM-020 / FR-015 | T366, T372–T373, T375–T376, T402 | T367, T374, T377, T403 |
| AC-ADM-021 / FR-021 | T121, T157–T158, T160–T161, T166–T167, T172–T173, T175–T176, T181–T182, T201, T203, T335–T336, T338–T339, T372–T373, T375–T376, T378–T379, T384–T385, T387–T388, T396, T404, T414, T416, T418–T431, T475 | T122, T159, T162, T168, T174, T177, T183, T202, T204, T337, T340, T374, T377, T380, T386, T389, T397, T405, T415, T417 |
| AC-ADM-022 / FR-022 | T121, T203, T409, T411–T412, T432 | T122, T204, T410, T413, T433 |
| AC-ADM-023 / FR-023 | T121, T201, T203, T345, T432 | T122, T202, T204, T346, T433 |
| AC-ADM-024 / FR-024 | T107, T157–T158, T160–T161, T166–T167, T169–T170, T172–T173, T175–T176, T181–T182, T260, T335–T336, T338–T339, T372–T373, T375–T376, T378–T379, T387–T388, T414, T416, T418–T431, T434, T436 | T108, T159, T162, T168, T171, T174, T177, T183, T261, T337, T340, T374, T377, T380, T389, T415, T417, T435, T437 |
| AC-ADM-025 / FR-024 | T121, T157–T158, T160–T161, T166–T167, T172–T173, T175–T176, T181–T182, T184–T185, T203, T233, T335–T336, T338–T339, T358, T372–T373, T375–T376, T378–T379, T384–T385, T387–T388, T396, T398–T400, T414, T416, T418–T431, T434, T436, T475 | T122, T159, T162, T168, T174, T177, T183, T186, T204, T337, T340, T374, T377, T380, T386, T389, T397, T415, T417, T435, T437 |
| AC-ADM-026 / FR-025 | T443, T480, T482 | T442, T444, T481 |
| AC-ADM-027 / FR-026 | T439, T441, T482 | T440, T442 |
| AC-ADM-028 / FR-027 | T443, T480, T482 | T444, T481 |
| AC-ADM-029 / FR-028 | T071, T445, T447, T449, T451, T453, T478, T482 | T072, T446, T448, T450, T452, T454, T479 |
| AC-ADM-030 / FR-029 | T246, T268, T445, T447, T449, T451, T453, T455, T476, T480, T482, T487 | T247, T269, T446, T448, T450, T452, T454, T477, T481, T488 |
| AC-ADM-031 / FR-030 | T073, T443, T482 | T074, T444 |
| AC-ADM-032 / FR-031 | T268, T456, T482 | T269, T457 |
| AC-ADM-033 / FR-017 | T279, T311, T317–T318, T343, T351, T363, T459, T464–T465, T475, T487 | T280, T312, T319, T344, T352, T364, T466, T488 |
| AC-ADM-034 / FR-018 | T311, T323–T324, T335–T336, T338–T339, T343, T351, T372–T373, T375–T376, T378–T379, T387–T388, T460–T462, T464–T465 | T312, T325, T337, T340, T344, T352, T374, T377, T380, T389, T466 |
| AC-ADM-035 / FR-019 | T301, T338–T339, T372–T373, T375–T376, T384–T385, T460–T462, T464–T465 | T302, T340, T374, T377, T386, T466 |
| AC-ADM-036 / FR-020 | T301, T338–T339, T372–T373, T375–T376, T384–T385, T460–T462, T464–T465, T475 | T302, T340, T374, T377, T386, T466 |
| AC-ADM-037 / FR-007 | T275, T307, T341, T349, T359 | T276, T308, T342, T350, T360 |
| AC-ADM-038 / FR-032 | T277, T307, T341, T349, T359 | T278, T308, T342, T350, T360 |
| AC-ADM-039 / FR-033 | T285, T287, T289, T291, T293, T295, T297, T299, T339, T361 | T286, T288, T290, T292, T294, T296, T298, T300, T340, T362 |
| AC-ADM-040 / FR-034 | T285, T287, T289, T291, T293, T295, T297, T299, T339, T361 | T286, T288, T290, T292, T294, T296, T298, T300, T340, T362 |
| AC-ADM-041 / FR-035 | T285, T287, T289, T291, T293, T295, T297, T299, T339, T361 | T286, T288, T290, T292, T294, T296, T298, T300, T340, T362 |
| AC-ADM-042 / FR-036 | T285, T287, T289, T291, T293, T295, T297, T299, T339, T361 | T286, T288, T290, T292, T294, T296, T298, T300, T340, T362 |
| AC-ADM-043 / FR-004 | T109, T111, T113, T115, T125, T129, T133, T157–T158, T181–T182, T187–T188, T196–T197, T199, T205, T207, T209, T230, T232, T248, T250, T252, T254, T256, T258, T260, T262, T264, T266, T270, T475 | T110, T112, T114, T116, T126, T130, T134, T183, T189, T198, T200, T206, T208, T210, T249, T251, T253, T255, T257, T259, T261, T263, T265, T267, T271 |
| AC-ADM-044 / FR-012 / FR-013 | T287, T366, T370, T372–T373, T378–T379, T385, T396, T402 | T288, T367, T371, T374, T380, T386, T397, T403 |
| AC-ADM-045 / FR-037 | T281, T366, T370, T375–T376, T378–T379, T381–T382, T384–T385, T391, T393, T396, T398–T400, T402, T404, T406 | T282, T367, T371, T377, T380, T383, T386, T392, T394, T397, T403, T405, T407 |
| AC-ADM-046 / FR-010 / P41 / OI8 | T285, T287, T289, T291, T293, T295, T297, T299, T339, T361, T390, T401 | T286, T288, T290, T292, T294, T296, T298, T300, T340, T362 |
| AC-ADM-047 / FR-038 / FR-UX-20 | T067, T069, T071, T478, T482 | T068, T070, T072, T479 |
| SC-001 | T109, T111, T113, T115, T117, T125, T129, T133, T137, T141, T145, T149, T151–T152, T154–T155, T157–T158, T160–T161, T163–T164, T166–T167, T169–T170, T172–T173, T175–T176, T181–T182, T184–T185, T187–T188, T190–T191, T193–T194, T196–T197, T199, T205, T207, T209, T211, T213, T219, T221, T223, T230–T232, T234, T236, T238, T240, T242, T244, T246, T248, T250, T252, T254, T256, T258, T260, T262, T264, T266, T268, T270, T272, T475 | T110, T112, T114, T116, T118, T126, T130, T134, T138, T142, T146, T150, T153, T156, T159, T162, T165, T168, T171, T174, T177, T183, T186, T189, T192, T195, T198, T200, T206, T208, T210, T212, T214, T220, T222, T224, T235, T237, T239, T241, T243, T245, T247, T249, T251, T253, T255, T257, T259, T261, T263, T265, T267, T269, T271, T273 |
| SC-002 | T281, T283, T285, T287, T289, T291, T293, T295, T297, T299, T311, T315, T323–T324, T329–T330, T332–T333, T335–T336, T338–T339, T343, T347, T351, T353, T358–T359, T361, T366, T370, T372–T373, T375–T376, T378–T379, T381–T382, T384–T385, T387–T388, T391, T393, T396, T398–T400, T402, T404, T406, T460–T462, T464–T465, T475 | T282, T284, T286, T288, T290, T292, T294, T296, T298, T300, T312, T316, T325, T331, T334, T337, T340, T344, T348, T352, T354, T360, T362, T367, T371, T374, T377, T380, T383, T386, T389, T392, T394, T397, T403, T405, T407, T466 |
| SC-003 | T275, T277, T285, T287, T289, T291, T293, T295, T297, T299, T303, T307, T317–T318, T320–T321, T326–T327, T329–T330, T335, T338–T339, T341, T345, T349, T358–T359, T361, T363, T475, T487 | T276, T278, T286, T288, T290, T292, T294, T296, T298, T300, T304, T308, T319, T322, T328, T331, T337, T340, T342, T346, T350, T360, T362, T364, T488 |
| SC-004 | T279, T301, T311, T317–T318, T323–T324, T338–T339, T343, T351, T363, T372–T373, T375–T376, T384–T385, T459–T462, T464–T465, T475, T487 | T280, T302, T312, T319, T325, T340, T344, T352, T364, T374, T377, T386, T466, T488 |
| SC-005 | T107, T121, T157–T158, T160–T161, T166–T167, T172–T173, T175–T176, T181–T182, T184–T185, T201, T203, T233, T335–T336, T338–T339, T345, T358, T372–T373, T375–T376, T378–T379, T384–T385, T387–T388, T396, T398–T400, T404, T409, T411–T412, T414, T416, T418–T432, T434, T436, T475 | T108, T122, T159, T162, T168, T174, T177, T183, T186, T202, T204, T337, T340, T346, T374, T377, T380, T386, T389, T397, T405, T410, T413, T415, T417, T433, T435, T437 |
| SC-006 | T071, T073, T439, T441, T443, T445, T447, T449, T451, T453, T478, T480, T482 | T072, T074, T440, T442, T444, T446, T448, T450, T452, T454, T479, T481 |
| SC-007 | T246, T268, T445, T447, T449, T451, T453, T455–T456, T476, T480, T482, T487 | T247, T269, T446, T448, T450, T452, T454, T457, T477, T481, T488 |
| SC-008 | T121, T157–T158, T160–T161, T166–T167, T172–T173, T175–T176, T181–T182, T201, T203, T281, T285, T287, T289, T291, T293, T295, T297, T299, T315, T329–T330, T332–T333, T335–T336, T338–T339, T347, T353, T358–T359, T361, T366, T370, T372–T373, T375–T376, T378–T379, T381–T382, T387–T388, T396, T402, T404, T409, T411–T412, T414, T416, T418–T432, T475 | T122, T159, T162, T168, T174, T177, T183, T202, T204, T282, T286, T288, T290, T292, T294, T296, T298, T300, T316, T331, T334, T337, T340, T348, T354, T360, T362, T367, T371, T374, T377, T380, T383, T389, T397, T403, T405, T410, T413, T415, T417, T433 |
| SC-009 | T285, T287, T289, T291, T293, T295, T297, T299, T338–T339, T361, T390, T401, T475 | T286, T288, T290, T292, T294, T296, T298, T300, T340, T362 |
| GR1–GR9 boundary only | T303, T485 | T304, T486; actual GR-prefixed invariant tests remain owning-feature release blockers |

## Port Completeness — Not Temporal Ordering

Partial gates validate planned-artifact/driver/completion links at the completed-work boundary;
actual-symbol port inventory is mandatory at T491, not a Phase 1 prerequisite. Trait cells
name implementation tasks with preceding drivers.
AdminAuthenticator real adapter = T224; T216/T218/T220/T222 are its internal component dependencies.
Contract cells name generic-suite red tasks; fake/real cells name implementations, with runner
tasks immediately preceding them. All real cells require the fake cell's contract to be green.
Mock-test cells list representative direct positive port interactions only, not adapter mocks,
transitive authorization, zero-call assertions or empty mocks. Read-only queries do not count
as ID generation or audit append evidence. Bootstrap/enrollment queries use existing ports; SessionStore includes pending authentication proofs/ceremonies. There are sixteen ports; pure impact/readiness policies have no inventory rows.

| Port | Trait task | Contract suite task | Fake task | Real adapter task | Mock-test task(s) |
| --- | --- | --- | --- | --- | --- |
| Clock | T042 | T043 | T044 | T056 | T184, T193, T196, T338, T378, T384 |
| IdGenerator | T046 | T047 | T048 | T058 | T193, T317, T335 |
| UnitOfWork / TxHandle lifecycle | T050 | T051 | T052 | T060 | T053, T151, T157, T181, T193, T196, T375, T384 |
| HealthProbe | T080 | T081 | T082 | T356 | T083 |
| AuditRepository | T120 | T121 | T122 | T204 | T157, T160, T175, T181, T193, T375, T384 |
| AdminRepository | T124 | T125 | T126 | T206 | T151, T175, T178, T181, T184, T193, T196, T378, T384 |
| AdminCredentialRepository | T128 | T129 | T130 | T208 | T157, T160, T181 |
| SessionStore | T132 | T133 | T134 | T210 | T154, T157, T160, T169, T181, T184, T187, T196 |
| OtpChallengeRepository | T136 | T137 | T138 | T212 | T151, T154, T181 |
| RateLimitRepository | T140 | T141 | T142 | T214 | T151 |
| AdminAuthenticator | T144 | T145 | T146 | T224 | T151, T157, T160, T193, T196 |
| SmsSender | T148 | T149 | T150 | T226 (Kavenegar), T228 (SMS.ir) | T151 |
| SettingsRepository | T306 | T307 | T308 | T350 | T320, T326, T329 |
| FlagRepository | T310 | T311 | T312 | T352 | T323, T329 |
| ChangeRepository | T314 | T315 | T316 | T354 | T332, T335, T338, T372, T375, T378, T387 |
| PublicationRepository | T369 | T370 | T371 | T397 | T375, T378, T381, T384 |

## Owner Consolidation — 580-task Baseline → Current IDs

The left column below is historical (the owner request used the 580-task revision).
Every old ID is listed exactly once. Right columns are current IDs; merged tests now
live in the earlier driving tasks, implementations in their original slices. MinIO
startup/readiness and isolated-bucket success/error/panic cleanup assertions move to
feature 004; Compose stays and no storage coverage is claimed here.

Current counts: **491 tasks**: 266 tests, 203 implementation, 13 setup, 9 checkpoints. **16 ports**.
No application code, migration, container, build/test result or completion claim is produced.

| Old ID (580 baseline) | Current ID(s) | Treatment / assertion destination |
| --- | --- | --- |
| baseline T001 | T001 | Retained; renumbered |
| baseline T002 | T002 | Retained; renumbered |
| baseline T003 | T003 | Retained; renumbered |
| baseline T004 | T004 | Retained; renumbered |
| baseline T005 | T005 | Retained; renumbered |
| baseline T006 | T006 | Retained; renumbered |
| baseline T007 | T007 | Retained; renumbered |
| baseline T008 | T008 | Retained; renumbered |
| baseline T009 | T009 | Retained; renumbered |
| baseline T010 | T010 | Retained; renumbered |
| baseline T011 | T011 | Retained; renumbered |
| baseline T012 | T012 | Retained; renumbered |
| baseline T013 | T013 | Retained; renumbered |
| baseline T014 | T014 | Retained; renumbered |
| baseline T015 | feature 004 (deferred) | Deferred to feature 004: MinIO fixture only |
| baseline T016 | feature 004 (deferred) | Deferred to feature 004: MinIO fixture only |
| baseline T017 | T015 | Retained; renumbered |
| baseline T018 | T016 | Retained; renumbered |
| baseline T019 | T017 | Retained; renumbered |
| baseline T020 | T018 | Retained; renumbered |
| baseline T021 | T021 | Retained; renumbered |
| baseline T022 | T019 | Retained; renumbered |
| baseline T023 | T020 | Retained; renumbered |
| baseline T024 | T022 | Retained; renumbered |
| baseline T025 | T023 | Retained; renumbered |
| baseline T026 | feature 004 (deferred) | Deferred to feature 004: MinIO fixture only |
| baseline T027 | feature 004 (deferred) | Deferred to feature 004: MinIO fixture only |
| baseline T028 | T024 | Retained; renumbered |
| baseline T029 | T025 | Retained; renumbered |
| baseline T030 | T026 | Retained; renumbered |
| baseline T031 | T027 | Retained; renumbered |
| baseline T032 | T028 | Retained; renumbered |
| baseline T033 | T029 | Retained; renumbered |
| baseline T034 | T030 | Retained; renumbered |
| baseline T035 | T031 | Retained; renumbered |
| baseline T036 | T032 | Retained; renumbered |
| baseline T037 | T033 | Retained; renumbered |
| baseline T038 | T034 | Retained; renumbered |
| baseline T039 | T035 | Retained; renumbered |
| baseline T040 | T036 | Retained; renumbered |
| baseline T041 | T037 | Retained; renumbered |
| baseline T042 | T038 | Retained; renumbered |
| baseline T043 | T039 | Retained; renumbered |
| baseline T044 | T040 | Retained; renumbered |
| baseline T045 | T041 | Retained; renumbered |
| baseline T046 | T042 | Retained; renumbered |
| baseline T047 | T043 | Retained; renumbered |
| baseline T048 | T044 | Retained; renumbered |
| baseline T049 | T045 | Retained; renumbered |
| baseline T050 | T046 | Retained; renumbered |
| baseline T051 | T047 | Retained; renumbered |
| baseline T052 | T048 | Retained; renumbered |
| baseline T053 | T049 | Retained; renumbered |
| baseline T054 | T050 | Retained; renumbered |
| baseline T055 | T051 | Retained; renumbered |
| baseline T056 | T052 | Retained; renumbered |
| baseline T057 | T053 | Retained; renumbered |
| baseline T058 | T054 | Retained; renumbered |
| baseline T059 | T055 | Retained; renumbered |
| baseline T060 | T056 | Retained; renumbered |
| baseline T061 | T057 | Retained; renumbered |
| baseline T062 | T058 | Retained; renumbered |
| baseline T063 | T001, T010 | Consolidated into earlier setup/config/checker driver |
| baseline T064 | T059 | Retained; renumbered |
| baseline T065 | T060 | Retained; renumbered |
| baseline T066 | T060 | Metadata completion criterion; final freshness checks retained |
| baseline T067 | T061 | Retained; renumbered |
| baseline T068 | T062 | Retained; renumbered |
| baseline T069 | T063 | Retained; renumbered |
| baseline T070 | T064 | Retained; renumbered |
| baseline T071 | T065 | Retained; renumbered |
| baseline T072 | T066 | Retained; renumbered |
| baseline T073 | T063 | Consolidated into earlier setup/config/checker driver |
| baseline T074 | T064 | Consolidated into earlier setup/config/checker driver |
| baseline T075 | T067 | Retained; renumbered |
| baseline T076 | T068 | Retained; renumbered |
| baseline T077 | T069 | Retained; renumbered |
| baseline T078 | T070 | Retained; renumbered |
| baseline T079 | T071 | Retained; renumbered |
| baseline T080 | T072 | Retained; renumbered |
| baseline T081 | T073 | Retained; renumbered |
| baseline T082 | T074 | Retained; renumbered |
| baseline T083 | T075 | Retained; renumbered |
| baseline T084 | T076 | Retained; renumbered |
| baseline T085 | T077 | Retained; renumbered |
| baseline T086 | T078 | Retained; renumbered |
| baseline T087 | T079 | Retained; renumbered |
| baseline T088 | T080 | Retained; renumbered |
| baseline T089 | T081 | Retained; renumbered |
| baseline T090 | T082 | Retained; renumbered |
| baseline T091 | T083 | Retained; renumbered |
| baseline T092 | T084 | Retained; renumbered |
| baseline T093 | T085 | Retained; renumbered |
| baseline T094 | T086 | Retained; renumbered |
| baseline T095 | T087 | Retained; renumbered |
| baseline T096 | T088 | Retained; renumbered |
| baseline T097 | T089 | Retained; renumbered |
| baseline T098 | T090 | Retained; renumbered |
| baseline T099 | T091 | Retained; renumbered |
| baseline T100 | T092 | Retained; renumbered |
| baseline T101 | T093 | Retained; renumbered |
| baseline T102 | T094 | Retained; renumbered |
| baseline T103 | T095 | Retained; renumbered |
| baseline T104 | T096 | Retained; renumbered |
| baseline T105 | T097 | Retained; renumbered |
| baseline T106 | T098 | Retained; renumbered |
| baseline T107 | T099 | Retained; renumbered |
| baseline T108 | T100 | Retained; renumbered |
| baseline T109 | T101 | Retained; renumbered |
| baseline T110 | T102 | Retained; renumbered |
| baseline T111 | T103 | Retained; renumbered |
| baseline T112 | T104 | Retained; renumbered |
| baseline T113 | T105 | Retained; renumbered |
| baseline T114 | T106 | Retained; renumbered |
| baseline T115 | T107 | Retained; renumbered |
| baseline T116 | T108 | Retained; renumbered |
| baseline T117 | T109 | Retained; renumbered |
| baseline T118 | T110 | Retained; renumbered |
| baseline T119 | T111 | Retained; renumbered |
| baseline T120 | T112 | Retained; renumbered |
| baseline T121 | T113 | Retained; renumbered |
| baseline T122 | T114 | Retained; renumbered |
| baseline T123 | T115 | Retained; renumbered |
| baseline T124 | T116 | Retained; renumbered |
| baseline T125 | T117 | Retained; renumbered |
| baseline T126 | T118 | Retained; renumbered |
| baseline T127 | T119 | Retained; renumbered |
| baseline T128 | T120 | Retained; renumbered |
| baseline T129 | T121 | Retained; renumbered |
| baseline T130 | T122 | Retained; renumbered |
| baseline T131 | T123 | Retained; renumbered |
| baseline T132 | T124 | Retained; renumbered |
| baseline T133 | T125 | Retained; renumbered |
| baseline T134 | T126 | Retained; renumbered |
| baseline T135 | T127 | Retained; renumbered |
| baseline T136 | T128 | Retained; renumbered |
| baseline T137 | T129 | Retained; renumbered |
| baseline T138 | T130 | Retained; renumbered |
| baseline T139 | T131 | Retained; renumbered |
| baseline T140 | T132 | Retained; renumbered |
| baseline T141 | T133 | Retained; renumbered |
| baseline T142 | T134 | Retained; renumbered |
| baseline T143 | T131 | Pending proof/ceremony assertions → SessionStore; authorization → AdminRepository |
| baseline T144 | T132 | Pending proof/ceremony assertions → SessionStore; authorization → AdminRepository |
| baseline T145 | T133 | Pending proof/ceremony assertions → SessionStore; authorization → AdminRepository |
| baseline T146 | T134 | Pending proof/ceremony assertions → SessionStore; authorization → AdminRepository |
| baseline T147 | T135 | Retained; renumbered |
| baseline T148 | T136 | Retained; renumbered |
| baseline T149 | T137 | Retained; renumbered |
| baseline T150 | T138 | Retained; renumbered |
| baseline T151 | T139 | Retained; renumbered |
| baseline T152 | T140 | Retained; renumbered |
| baseline T153 | T141 | Retained; renumbered |
| baseline T154 | T142 | Retained; renumbered |
| baseline T155 | T143 | Retained; renumbered |
| baseline T156 | T144 | Retained; renumbered |
| baseline T157 | T145 | Retained; renumbered |
| baseline T158 | T146 | Retained; renumbered |
| baseline T159 | T147 | Retained; renumbered |
| baseline T160 | T148 | Retained; renumbered |
| baseline T161 | T149 | Retained; renumbered |
| baseline T162 | T150 | Retained; renumbered |
| baseline T163 | T151 | Retained; renumbered |
| baseline T164 | T152 | Retained; renumbered |
| baseline T165 | T153 | Retained; renumbered |
| baseline T166 | T154 | Retained; renumbered |
| baseline T167 | T155 | Retained; renumbered |
| baseline T168 | T156 | Retained; renumbered |
| baseline T169 | T157 | Retained; renumbered |
| baseline T170 | T158 | Retained; renumbered |
| baseline T171 | T159 | Retained; renumbered |
| baseline T172 | T160 | Retained; renumbered |
| baseline T173 | T161 | Retained; renumbered |
| baseline T174 | T162 | Retained; renumbered |
| baseline T175 | T163 | Retained; renumbered |
| baseline T176 | T164 | Retained; renumbered |
| baseline T177 | T165 | Retained; renumbered |
| baseline T178 | T166 | Retained; renumbered |
| baseline T179 | T167 | Retained; renumbered |
| baseline T180 | T168 | Retained; renumbered |
| baseline T181 | T169 | Retained; renumbered |
| baseline T182 | T170 | Retained; renumbered |
| baseline T183 | T171 | Retained; renumbered |
| baseline T184 | T172 | Retained; renumbered |
| baseline T185 | T173 | Retained; renumbered |
| baseline T186 | T174 | Retained; renumbered |
| baseline T187 | T175 | Retained; renumbered |
| baseline T188 | T176 | Retained; renumbered |
| baseline T189 | T177 | Retained; renumbered |
| baseline T190 | T178 | Retained; renumbered |
| baseline T191 | T179 | Retained; renumbered |
| baseline T192 | T180 | Retained; renumbered |
| baseline T193 | T181 | Retained; renumbered |
| baseline T194 | T182 | Retained; renumbered |
| baseline T195 | T183 | Retained; renumbered |
| baseline T196 | T184 | Retained; renumbered |
| baseline T197 | T185 | Retained; renumbered |
| baseline T198 | T186 | Retained; renumbered |
| baseline T199 | T187 | Retained; renumbered |
| baseline T200 | T188 | Retained; renumbered |
| baseline T201 | T189 | Retained; renumbered |
| baseline T202 | T190 | Retained; renumbered |
| baseline T203 | T191 | Retained; renumbered |
| baseline T204 | T192 | Retained; renumbered |
| baseline T205 | T193 | Retained; renumbered |
| baseline T206 | T194 | Retained; renumbered |
| baseline T207 | T195 | Retained; renumbered |
| baseline T208 | T196 | Retained; renumbered |
| baseline T209 | T197 | Retained; renumbered |
| baseline T210 | T198 | Retained; renumbered |
| baseline T211 | T157–T158 | Assertions folded into original driving tests / completion criteria |
| baseline T212 | T160–T161 | Assertions folded into original driving tests / completion criteria |
| baseline T213 | T162 | Assertions folded into original driving tests / completion criteria |
| baseline T214 | T181–T182 | Assertions folded into original driving tests / completion criteria |
| baseline T215 | T183 | Assertions folded into original driving tests / completion criteria |
| baseline T216 | T157–T158 | Assertions folded into original driving tests / completion criteria |
| baseline T217 | T199 | Retained; renumbered |
| baseline T218 | T200 | Retained; renumbered |
| baseline T219 | T201 | Retained; renumbered |
| baseline T220 | T202 | Retained; renumbered |
| baseline T221 | T203 | Retained; renumbered |
| baseline T222 | T204 | Retained; renumbered |
| baseline T223 | T204 | Metadata completion criterion; final freshness checks retained |
| baseline T224 | T205 | Retained; renumbered |
| baseline T225 | T206 | Retained; renumbered |
| baseline T226 | T206 | Metadata completion criterion; final freshness checks retained |
| baseline T227 | T207 | Retained; renumbered |
| baseline T228 | T208 | Retained; renumbered |
| baseline T229 | T208 | Metadata completion criterion; final freshness checks retained |
| baseline T230 | T209 | Retained; renumbered |
| baseline T231 | T210 | Retained; renumbered |
| baseline T232 | T210 | Metadata completion criterion; final freshness checks retained |
| baseline T233 | T209 | Pending proof/ceremony assertions → SessionStore; authorization → AdminRepository |
| baseline T234 | T210 | Pending proof/ceremony assertions → SessionStore; authorization → AdminRepository |
| baseline T235 | T210 | Metadata completion criterion; final freshness checks retained |
| baseline T236 | T211 | Retained; renumbered |
| baseline T237 | T212 | Retained; renumbered |
| baseline T238 | T212 | Metadata completion criterion; final freshness checks retained |
| baseline T239 | T213 | Retained; renumbered |
| baseline T240 | T214 | Retained; renumbered |
| baseline T241 | T214 | Metadata completion criterion; final freshness checks retained |
| baseline T242 | T215 | Retained; renumbered |
| baseline T243 | T216 | Retained; renumbered |
| baseline T244 | T217 | Retained; renumbered |
| baseline T245 | T218 | Retained; renumbered |
| baseline T246 | T219 | Retained; renumbered |
| baseline T247 | T220 | Retained; renumbered |
| baseline T248 | T221 | Retained; renumbered |
| baseline T249 | T222 | Retained; renumbered |
| baseline T250 | T223 | Retained; renumbered |
| baseline T251 | T224 | Retained; renumbered |
| baseline T252 | T225 | Retained; renumbered |
| baseline T253 | T226 | Retained; renumbered |
| baseline T254 | T227 | Retained; renumbered |
| baseline T255 | T228 | Retained; renumbered |
| baseline T256 | T229 | Retained; renumbered |
| baseline T257 | T230 | Retained; renumbered |
| baseline T258 | T231 | Retained; renumbered |
| baseline T259 | T232 | Retained; renumbered |
| baseline T260 | T233 | Retained; renumbered |
| baseline T261 | T234 | Retained; renumbered |
| baseline T262 | T235 | Retained; renumbered |
| baseline T263 | T236 | Retained; renumbered |
| baseline T264 | T237 | Retained; renumbered |
| baseline T265 | T238 | Retained; renumbered |
| baseline T266 | T239 | Retained; renumbered |
| baseline T267 | T240 | Retained; renumbered |
| baseline T268 | T241 | Retained; renumbered |
| baseline T269 | T242 | Retained; renumbered |
| baseline T270 | T243 | Retained; renumbered |
| baseline T271 | T244 | Retained; renumbered |
| baseline T272 | T245 | Retained; renumbered |
| baseline T273 | T246 | Retained; renumbered |
| baseline T274 | T247 | Retained; renumbered |
| baseline T275 | T248 | Retained; renumbered |
| baseline T276 | T249 | Retained; renumbered |
| baseline T277 | T250 | Retained; renumbered |
| baseline T278 | T251 | Retained; renumbered |
| baseline T279 | T252 | Retained; renumbered |
| baseline T280 | T253 | Retained; renumbered |
| baseline T281 | T254 | Retained; renumbered |
| baseline T282 | T255 | Retained; renumbered |
| baseline T283 | T256 | Retained; renumbered |
| baseline T284 | T257 | Retained; renumbered |
| baseline T285 | T258 | Retained; renumbered |
| baseline T286 | T259 | Retained; renumbered |
| baseline T287 | T260 | Retained; renumbered |
| baseline T288 | T261 | Retained; renumbered |
| baseline T289 | T262 | Retained; renumbered |
| baseline T290 | T263 | Retained; renumbered |
| baseline T291 | T264 | Retained; renumbered |
| baseline T292 | T265 | Retained; renumbered |
| baseline T293 | T266 | Retained; renumbered |
| baseline T294 | T267 | Retained; renumbered |
| baseline T295 | T268 | Retained; renumbered |
| baseline T296 | T269 | Retained; renumbered |
| baseline T297 | T270 | Retained; renumbered |
| baseline T298 | T271 | Retained; renumbered |
| baseline T299 | T272 | Retained; renumbered |
| baseline T300 | T273 | Retained; renumbered |
| baseline T301 | T274 | Retained; renumbered |
| baseline T302 | T275 | Retained; renumbered |
| baseline T303 | T276 | Retained; renumbered |
| baseline T304 | T277 | Retained; renumbered |
| baseline T305 | T278 | Retained; renumbered |
| baseline T306 | T279 | Retained; renumbered |
| baseline T307 | T280 | Retained; renumbered |
| baseline T308 | T281 | Retained; renumbered |
| baseline T309 | T282 | Retained; renumbered |
| baseline T310 | T283 | Retained; renumbered |
| baseline T311 | T284 | Retained; renumbered |
| baseline T312 | T285 | Retained; renumbered |
| baseline T313 | T286 | Retained; renumbered |
| baseline T314 | T287 | Retained; renumbered |
| baseline T315 | T288 | Retained; renumbered |
| baseline T316 | T289 | Retained; renumbered |
| baseline T317 | T290 | Retained; renumbered |
| baseline T318 | T291 | Retained; renumbered |
| baseline T319 | T292 | Retained; renumbered |
| baseline T320 | T293 | Retained; renumbered |
| baseline T321 | T294 | Retained; renumbered |
| baseline T322 | T295 | Retained; renumbered |
| baseline T323 | T296 | Retained; renumbered |
| baseline T324 | T297 | Retained; renumbered |
| baseline T325 | T298 | Retained; renumbered |
| baseline T326 | T299 | Retained; renumbered |
| baseline T327 | T300 | Retained; renumbered |
| baseline T328 | T303 | Retained; renumbered |
| baseline T329 | T304 | Retained; renumbered |
| baseline T330 | T305 | Retained; renumbered |
| baseline T331 | T306 | Retained; renumbered |
| baseline T332 | T307 | Retained; renumbered |
| baseline T333 | T308 | Retained; renumbered |
| baseline T334 | T309 | Retained; renumbered |
| baseline T335 | T310 | Retained; renumbered |
| baseline T336 | T311 | Retained; renumbered |
| baseline T337 | T312 | Retained; renumbered |
| baseline T338 | T313 | Retained; renumbered |
| baseline T339 | T314 | Retained; renumbered |
| baseline T340 | T315 | Retained; renumbered |
| baseline T341 | T316 | Retained; renumbered |
| baseline T342 | T285 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T343 | T286 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T344 | T285, T287, T289, T291, T293, T295, T297, T299 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T345 | T286, T288, T290, T292, T294, T296, T298, T300 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T346 | T317 | Retained; renumbered |
| baseline T347 | T318 | Retained; renumbered |
| baseline T348 | T319 | Retained; renumbered |
| baseline T349 | T320 | Retained; renumbered |
| baseline T350 | T321 | Retained; renumbered |
| baseline T351 | T322 | Retained; renumbered |
| baseline T352 | T323 | Retained; renumbered |
| baseline T353 | T324 | Retained; renumbered |
| baseline T354 | T325 | Retained; renumbered |
| baseline T355 | T326 | Retained; renumbered |
| baseline T356 | T327 | Retained; renumbered |
| baseline T357 | T328 | Retained; renumbered |
| baseline T358 | T329 | Retained; renumbered |
| baseline T359 | T330 | Retained; renumbered |
| baseline T360 | T331 | Retained; renumbered |
| baseline T361 | T332 | Retained; renumbered |
| baseline T362 | T333 | Retained; renumbered |
| baseline T363 | T334 | Retained; renumbered |
| baseline T364 | T335 | Retained; renumbered |
| baseline T365 | T336 | Retained; renumbered |
| baseline T366 | T337 | Retained; renumbered |
| baseline T367 | T338 | Retained; renumbered |
| baseline T368 | T339 | Retained; renumbered |
| baseline T369 | T340 | Retained; renumbered |
| baseline T370 | T341 | Retained; renumbered |
| baseline T371 | T342 | Retained; renumbered |
| baseline T372 | T343 | Retained; renumbered |
| baseline T373 | T344 | Retained; renumbered |
| baseline T374 | T345 | Retained; renumbered |
| baseline T375 | T346 | Retained; renumbered |
| baseline T376 | T347 | Retained; renumbered |
| baseline T377 | T348 | Retained; renumbered |
| baseline T378 | T349 | Retained; renumbered |
| baseline T379 | T350 | Retained; renumbered |
| baseline T380 | T350 | Metadata completion criterion; final freshness checks retained |
| baseline T381 | T351 | Retained; renumbered |
| baseline T382 | T352 | Retained; renumbered |
| baseline T383 | T352 | Metadata completion criterion; final freshness checks retained |
| baseline T384 | T353 | Retained; renumbered |
| baseline T385 | T354 | Retained; renumbered |
| baseline T386 | T354 | Metadata completion criterion; final freshness checks retained |
| baseline T387 | T285, T287, T289, T291, T293, T295, T297, T299 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T388 | T286, T288, T290, T292, T294, T296, T298, T300 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T389 | T355 | Retained; renumbered |
| baseline T390 | T356 | Retained; renumbered |
| baseline T391 | T356 | Metadata completion criterion; final freshness checks retained |
| baseline T392 | T357 | Retained; renumbered |
| baseline T393 | T358 | Retained; renumbered |
| baseline T394 | T359 | Retained; renumbered |
| baseline T395 | T360 | Retained; renumbered |
| baseline T396 | T361 | Retained; renumbered |
| baseline T397 | T362 | Retained; renumbered |
| baseline T398 | T363 | Retained; renumbered |
| baseline T399 | T364 | Retained; renumbered |
| baseline T400 | T365 | Retained; renumbered |
| baseline T401 | T366 | Retained; renumbered |
| baseline T402 | T367 | Retained; renumbered |
| baseline T403 | T368 | Retained; renumbered |
| baseline T404 | T369 | Retained; renumbered |
| baseline T405 | T370 | Retained; renumbered |
| baseline T406 | T371 | Retained; renumbered |
| baseline T407 | T301 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T408 | T302 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T409 | T301 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T410 | T302 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T411 | T372 | Retained; renumbered |
| baseline T412 | T373 | Retained; renumbered |
| baseline T413 | T374 | Retained; renumbered |
| baseline T414 | T375 | Retained; renumbered |
| baseline T415 | T376 | Retained; renumbered |
| baseline T416 | T377 | Retained; renumbered |
| baseline T417 | T378 | Retained; renumbered |
| baseline T418 | T379 | Retained; renumbered |
| baseline T419 | T380 | Retained; renumbered |
| baseline T420 | T381 | Retained; renumbered |
| baseline T421 | T382 | Retained; renumbered |
| baseline T422 | T383 | Retained; renumbered |
| baseline T423 | T384 | Retained; renumbered |
| baseline T424 | T385 | Retained; renumbered |
| baseline T425 | T386 | Retained; renumbered |
| baseline T426 | T387 | Retained; renumbered |
| baseline T427 | T388 | Retained; renumbered |
| baseline T428 | T389 | Retained; renumbered |
| baseline T429 | T375 | Assertions folded into original driving tests / completion criteria |
| baseline T430 | T376 | Assertions folded into original driving tests / completion criteria |
| baseline T431 | T377 | Assertions folded into original driving tests / completion criteria |
| baseline T432 | T390 | Retained; renumbered |
| baseline T433 | T391 | Retained; renumbered |
| baseline T434 | T392 | Retained; renumbered |
| baseline T435 | T393 | Retained; renumbered |
| baseline T436 | T394 | Retained; renumbered |
| baseline T437 | T396 | Retained; renumbered |
| baseline T438 | T397 | Retained; renumbered |
| baseline T439 | T397 | Metadata completion criterion; final freshness checks retained |
| baseline T440 | T301 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T441 | T302 | Pure domain/provenance/policy cases; port/adapter removed |
| baseline T442 | T395 | Harness moved before integrated original SQL driver |
| baseline T443 | T370, T375–T376, T396 | Assertions folded into original driving tests / completion criteria |
| baseline T444 | T371, T377, T397 | Assertions folded into original driving tests / completion criteria |
| baseline T445 | T397 | Metadata completion criterion; final freshness checks retained |
| baseline T446 | T366, T370, T396 | Assertions folded into original driving tests / completion criteria |
| baseline T447 | T367, T371, T397 | Assertions folded into original driving tests / completion criteria |
| baseline T448 | T397 | Metadata completion criterion; final freshness checks retained |
| baseline T449 | T366, T370, T396 | Assertions folded into original driving tests / completion criteria |
| baseline T450 | T367, T371, T397 | Assertions folded into original driving tests / completion criteria |
| baseline T451 | T397 | Metadata completion criterion; final freshness checks retained |
| baseline T452 | T370, T385, T396 | Assertions folded into original driving tests / completion criteria |
| baseline T453 | T371, T386, T397 | Assertions folded into original driving tests / completion criteria |
| baseline T454 | T397 | Metadata completion criterion; final freshness checks retained |
| baseline T455 | T370, T375–T376, T385, T396 | Assertions folded into original driving tests / completion criteria |
| baseline T456 | T371, T377, T386, T397 | Assertions folded into original driving tests / completion criteria |
| baseline T457 | T397 | Metadata completion criterion; final freshness checks retained |
| baseline T458 | T370, T385, T396 | Assertions folded into original driving tests / completion criteria |
| baseline T459 | T371, T386, T397 | Assertions folded into original driving tests / completion criteria |
| baseline T460 | T397 | Metadata completion criterion; final freshness checks retained |
| baseline T461 | T398 | Retained; renumbered |
| baseline T462 | T399 | Retained; renumbered |
| baseline T463 | T400 | Retained; renumbered |
| baseline T464 | T401 | Retained; renumbered |
| baseline T465 | T402 | Retained; renumbered |
| baseline T466 | T403 | Retained; renumbered |
| baseline T467 | T404 | Retained; renumbered |
| baseline T468 | T405 | Retained; renumbered |
| baseline T469 | T406 | Retained; renumbered |
| baseline T470 | T407 | Retained; renumbered |
| baseline T471 | T408 | Retained; renumbered |
| baseline T472 | T409 | Retained; renumbered |
| baseline T473 | T410 | Retained; renumbered |
| baseline T474 | T411 | Retained; renumbered |
| baseline T475 | T412 | Retained; renumbered |
| baseline T476 | T413 | Retained; renumbered |
| baseline T477 | T414 | Retained; renumbered |
| baseline T478 | T415 | Retained; renumbered |
| baseline T479 | T416 | Retained; renumbered |
| baseline T480 | T417 | Retained; renumbered |
| baseline T481 | T157–T158 | Assertions folded into original driving tests / completion criteria |
| baseline T482 | T159 | Assertions folded into original driving tests / completion criteria |
| baseline T483 | T160–T161 | Assertions folded into original driving tests / completion criteria |
| baseline T484 | T162 | Assertions folded into original driving tests / completion criteria |
| baseline T485 | T166–T167, T172–T173 | Assertions folded into original driving tests / completion criteria |
| baseline T486 | T168, T174 | Assertions folded into original driving tests / completion criteria |
| baseline T487 | T175–T176 | Assertions folded into original driving tests / completion criteria |
| baseline T488 | T177 | Assertions folded into original driving tests / completion criteria |
| baseline T489 | T181–T182 | Assertions folded into original driving tests / completion criteria |
| baseline T490 | T183 | Assertions folded into original driving tests / completion criteria |
| baseline T491 | T335–T336 | Assertions folded into original driving tests / completion criteria |
| baseline T492 | T337 | Assertions folded into original driving tests / completion criteria |
| baseline T493 | T338–T339 | Assertions folded into original driving tests / completion criteria |
| baseline T494 | T340 | Assertions folded into original driving tests / completion criteria |
| baseline T495 | T372–T373 | Assertions folded into original driving tests / completion criteria |
| baseline T496 | T374 | Assertions folded into original driving tests / completion criteria |
| baseline T497 | T378–T379 | Assertions folded into original driving tests / completion criteria |
| baseline T498 | T380 | Assertions folded into original driving tests / completion criteria |
| baseline T499 | T375–T376 | Assertions folded into original driving tests / completion criteria |
| baseline T500 | T377 | Assertions folded into original driving tests / completion criteria |
| baseline T501 | T387–T388 | Assertions folded into original driving tests / completion criteria |
| baseline T502 | T389 | Assertions folded into original driving tests / completion criteria |
| baseline T503 | T418 | Retained; renumbered |
| baseline T504 | T419 | Retained; renumbered |
| baseline T505 | T420 | Retained; renumbered |
| baseline T506 | T421 | Retained; renumbered |
| baseline T507 | T422 | Retained; renumbered |
| baseline T508 | T423 | Retained; renumbered |
| baseline T509 | T424 | Retained; renumbered |
| baseline T510 | T425 | Retained; renumbered |
| baseline T511 | T426 | Retained; renumbered |
| baseline T512 | T427 | Retained; renumbered |
| baseline T513 | T428 | Retained; renumbered |
| baseline T514 | T429 | Retained; renumbered |
| baseline T515 | T430 | Retained; renumbered |
| baseline T516 | T431 | Retained; renumbered |
| baseline T517 | T432 | Retained; renumbered |
| baseline T518 | T433 | Retained; renumbered |
| baseline T519 | T434 | Retained; renumbered |
| baseline T520 | T435 | Retained; renumbered |
| baseline T521 | T436 | Retained; renumbered |
| baseline T522 | T437 | Retained; renumbered |
| baseline T523 | T438 | Retained; renumbered |
| baseline T524 | T439 | Retained; renumbered |
| baseline T525 | T440 | Retained; renumbered |
| baseline T526 | T441 | Retained; renumbered |
| baseline T527 | T442 | Retained; renumbered |
| baseline T528 | T443 | Retained; renumbered |
| baseline T529 | T444 | Retained; renumbered |
| baseline T530 | T445 | Retained; renumbered |
| baseline T531 | T446 | Retained; renumbered |
| baseline T532 | T447 | Retained; renumbered |
| baseline T533 | T448 | Retained; renumbered |
| baseline T534 | T449 | Retained; renumbered |
| baseline T535 | T450 | Retained; renumbered |
| baseline T536 | T451 | Retained; renumbered |
| baseline T537 | T452 | Retained; renumbered |
| baseline T538 | T453 | Retained; renumbered |
| baseline T539 | T454 | Retained; renumbered |
| baseline T540 | T455 | Retained; renumbered |
| baseline T541 | T456 | Retained; renumbered |
| baseline T542 | T457 | Retained; renumbered |
| baseline T543 | T458 | Retained; renumbered |
| baseline T544 | T459 | Retained; renumbered |
| baseline T545 | T301 | Pure readiness policy moved before first consumers |
| baseline T546 | T302 | Pure readiness policy moved before first consumers |
| baseline T547 | T460 | Retained; renumbered |
| baseline T548 | T461 | Retained; renumbered |
| baseline T549 | T462 | Retained; renumbered; T462 is now regression-only; implementation remains in original workflow slices |
| baseline T550 | T463 | Retained; renumbered |
| baseline T551 | T464 | Retained; renumbered |
| baseline T552 | T465 | Retained; renumbered |
| baseline T553 | T466 | Retained; renumbered |
| baseline T554 | T467 | Retained; renumbered |
| baseline T555 | T468 | Retained; renumbered |
| baseline T556 | T469 | Retained; renumbered |
| baseline T557 | T030 | Consolidated into earlier setup/config/checker driver |
| baseline T558 | T031 | Consolidated into earlier setup/config/checker driver |
| baseline T559 | T470 | Retained; renumbered |
| baseline T560 | T471 | Retained; renumbered |
| baseline T561 | T472 | Retained; renumbered |
| baseline T562 | T473 | Retained; renumbered |
| baseline T563 | T474 | Retained; renumbered |
| baseline T564 | T475 | Retained; renumbered |
| baseline T565 | T476 | Retained; renumbered |
| baseline T566 | T477 | Retained; renumbered |
| baseline T567 | T478 | Retained; renumbered |
| baseline T568 | T479 | Retained; renumbered |
| baseline T569 | T480 | Retained; renumbered |
| baseline T570 | T481 | Retained; renumbered |
| baseline T571 | T482 | Retained; renumbered |
| baseline T572 | T483 | Retained; renumbered |
| baseline T573 | T484 | Retained; renumbered |
| baseline T574 | T485 | Retained; renumbered |
| baseline T575 | T486 | Retained; renumbered |
| baseline T576 | T487 | Retained; renumbered |
| baseline T577 | T488 | Retained; renumbered |
| baseline T578 | T489 | Retained; renumbered |
| baseline T579 | T490 | Retained; renumbered |
| baseline T580 | T491 | Retained; renumbered |

## Owner Follow-up — C1 / I1 / I2 / U1 / U2 / I3 / D1

491 → **491 tasks** (net zero); **16 ports**; 47 canonical AC names, 38 FRs and nine SCs retained.
No application code, build, migration, container, runtime result or task completion is claimed.
All new behavior is driven in existing earlier tests; no generation command ran.

| Previous 491-baseline ID | Current ID | Change |
| --- | --- | --- |
| prior T019 | T021 | Activate/smoke only after bootstrap, export and lease are green |
| prior T020 | T019 | Live-run lease driver precedes activation |
| prior T021 | T020 | Lease implementation follows its intended red and precedes activation |
| All other IDs | Unchanged | Existing drivers/criteria extended; no tasks added or removed |

- C1: T169–T171, T260–T261 and SessionStore's surface/shared fake/real drivers cover the
  authenticated exactly-once/no-store TOTP secret/QR exception, no redisplay/logging/other
  delivery, and the continued ban on all other UI secrets, per constitution v1.2.0.
- I1: T030–T032 test/check valid partial plans and completed evidence, reject premature
  completion, and test full final mode. T491 requires every actual port symbol/runner/direct
  interaction plus real/fake feature graphs; missing future symbols do not block Phase 1.
- I2: the current order is T017 red → T018 wrapper green → T019 lease red → T020 lease
  green → T021 shared activation/smoke. Bootstrap selectors remain disjoint.
- U1: SchedulePublish/ScheduledPublish and the original shared fake/real contracts persist
  scheduling admin/captured epoch/approver; invalid scheduling authority causes one terminal
  audited cancellation, no live effect, actor=scheduler/executed_by=system_worker/approver,
  visible reason and another-admin linked rescheduling through the existing review flow.
- U2: T281–T282 drive pure time values before SaveDraft; SchedulePublish and HTTP drivers
  reuse them: local date+time, Asia/Tehran default, embedded IANA → UTC, gap/overlap/past
  rejection and explicit-offset ISO 8601 JSON decoding. No JSON endpoint or clock I/O added;
  durable overdue work keeps its existing execution rules. Berlin DST fixtures are explicit.
- I3: T224 is AdminAuthenticator's real adapter; T216/T218/T220/T222 remain component work.
- D1: T460–T462 retain all flag assertions as regression validation, not a production repair
  promise. Further code requires a genuinely new, named, independently driven case.
