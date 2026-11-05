# Tasks: Tankhor Admin Foundation (001)

**Input**: `specs/001-admin-foundation/{spec.md,plan.md,research.md,data-model.md,contracts/,quickstart.md}`.
**Authority**: `.specify/memory/constitution.md` v1.1.1; only the authoritative documents in
`docs/README.md` decide requirements. `docs/fa/` is never a decision source.
**Tests**: MANDATORY. All boxes are initially unchecked; generation is not implementation evidence.
**Organization**: shared setup/foundation, then US1–US6 in specification priority order, then gates.

## Format and execution contract

- Format: `- [ ] Tnnn [P?] [USn?] Write failing test: [layer] ...` or
  `- [ ] Tnnn [P?] [USn?] Make pass: Tnnn ...`. Checkpoint tasks change only this task list.
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
- Naming reconciliations: `AdminCredentialRepository` replaces the plan's `CredentialRepository`;
  `SessionStore` replaces `AdminSessionRepository`. They are single ports, not duplicate APIs.
  Kernel ports are in `kernel/src/ports/`; real time/UUID adapters are in `infra-system`, not app.
  `HealthProbe` makes the plan's storage/seed-readiness dependency explicit. It fails closed
  before the registry exists. App only wires these adapters; it implements none of them.
- Foundation creates only genuinely needed crate entry points; no empty future contexts,
  placeholder successful handlers, arbitrary-proof authenticators or production testkit imports.
  `dev-fakes` is an explicit debug-only app feature. Fake mode remains forbidden in production.
- Configuration keys for T054/T055 and later composition are `TANKHOR_ENV` with values
  `development|staging|production`, a default `TANKHOR_ADAPTERS=real|fake`, and optional
  `TANKHOR_ADAPTERS_PERSISTENCE`, `TANKHOR_ADAPTERS_TIME`, `TANKHOR_ADAPTERS_IDS`,
  `TANKHOR_ADAPTERS_AUTH`, `TANKHOR_ADAPTERS_SMS`, `TANKHOR_ADAPTERS_POLICY` overrides.
  Persistence includes every repository, UnitOfWork and HealthProbe as one coherent group;
  SMS selects both configured provider slots; policy selects impact/readiness. Omitted mode
  defaults to real, never silently fake. Unknown values fail startup with a sanitized key-
  specific error. Production rejects any fake group before resource acquisition.
- Exact named AC tests from `spec.md` are retained, including AC-ADM-046/047 absent from older
  contract prose. All final tables refer to concrete tasks below, never to future empty tests.
- No new numeric mutation threshold: ≥90% line coverage applies to present kernel/wallet/
  generation/identity. Other-crate 80% and mutation 85% remain unapproved proposals.

## Phase 1: Setup — Workspace, fixtures and CI

**Purpose**: Make the test-first toolchain executable without implementing console behaviour.
**Independent test**: repository-check negatives reject missing lints and forbidden edges;
integration setup provisions only owned isolated services. No application acceptance is claimed.

- [ ] T001 Write failing test: [unit] `workspace_skeleton_requires_inherited_lints` in `scripts/tests/test_workspace.py`; expect absent workspace/toolchain/lint inheritance to fail (FR-001, FR-CON-01; constitution I–II).
- [ ] T002 Make pass: T001 create `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `crates/kernel/Cargo.toml`, `crates/kernel/src/lib.rs`; pin Rust 1.98.1/edition 2024, inherit unsafe_code=forbid and pedantic=warn, expose no invented runtime behaviour (FR-001, FR-CON-01).
- [ ] T003 Write failing test: [unit] `architecture_policy_rejects_forbidden_context_edges` in `scripts/tests/test_architecture.py`; inject SQLx under alias/workspace/optional/target/dev/transitive edges and a web→repository edge; expect missing checker acceptance (FR-001, FR-CON-01; constitution II).
- [ ] T004 Make pass: T003 implement metadata/manifest/import checks in `scripts/check-architecture.py`; reject I/O in contexts/kernel, adapter→adapter calls, web/worker→repositories and direct domain/application clock/ID calls (FR-001, FR-CON-01).
- [ ] T005 Write failing test: [integration] `cargo_deny_rejects_sqlx_context_dependency` in `scripts/tests/test_deny.py`; construct disposable manifests under the test harness, run cargo-deny and require a nonzero result for admin→sqlx but allow infra-postgres→sqlx; expect missing bans (FR-001, FR-CON-01).
- [ ] T006 Make pass: T005 configure wrapper-aware bans/licenses/advisories in `deny.toml`; retain T004 metadata checking for indirect paths cargo-deny cannot express (FR-001, FR-CON-01).
- [ ] T007 Write failing test: [unit] `nextest_profiles_select_storage_tests_only` in `scripts/tests/test_nextest.py`; expect absent default/ci exclusions, integration selection/setup scope and concurrency budget to fail (FR-024, FR-CON-37).
- [ ] T008 Make pass: T007 create `.config/nextest.toml` with top-level setup-scripts, integration_* binaries, bounded groups and setup environment export per `contracts/integration-fixtures.md`; select existing integration binaries without referencing an absent future crate, expand package scoping as adapters arrive, and pin nextest 0.9.151 in `docs/development.md` (FR-024, FR-CON-37).
- [ ] T009 Write failing test: [unit] [integration] `integration_setup_is_idempotent_and_waits_for_both_services` in `scripts/tests/test_setup_integration.py` and `fixture_ownership_rejects_foreign_resources` in `crates/testkit/tests/fixture_ownership.rs`; expect absent labelled setup/readiness/password-free NEXTEST_ENV endpoints and rejection of foreign containers; the latter is a Docker-free Rust target included in default nextest (FR-024, FR-CON-37).
- [ ] T010 Make pass: T009 create `scripts/setup-integration.sh`, `crates/testkit/Cargo.toml`, `crates/testkit/src/lib.rs`, `crates/testkit/src/fixtures/services.rs`, `crates/testkit/src/bin/fixture-runner.rs`; use detached Testcontainers-managed PostgreSQL 16/MinIO, protected credentials and idempotent cluster roles, never live vendor services (FR-024, FR-CON-37).
- [ ] T011 Write failing test: [integration] `local_shared_server_rejects_concurrent_independent_runs` in `scripts/tests/test_fixture_lease.py`; expect a second run to reuse SQLx deterministic database names unsafely or lose ownership metadata (FR-024, FR-CON-37).
- [ ] T012 Make pass: T011 implement setup lock/live owner lease/stale-owner checks in `crates/testkit/src/fixtures/lease.rs` and wire `crates/testkit/src/fixtures/services.rs`; retain local service pair between runs (FR-024, FR-CON-37).
- [ ] T013 Write failing test: [unit] `fallback_runner_sets_only_child_environment` in `scripts/tests/test_test_runner.py`; expect missing runner, discovery-triggered Docker, lost child status or global environment mutation (FR-024, FR-CON-37).
- [ ] T014 Make pass: T013 create `scripts/test-runner.sh`, `.cargo/config.toml`, `justfile`, `crates/testkit/src/fixtures/process.rs`; launch the same test binary with child-only endpoints, retain guards, pass through non-storage/list/run commands and validate supplied fixture ownership (FR-024, FR-CON-37).
- [ ] T015 Write failing test: [integration] `minio_bucket_names_are_isolated_and_cleanup_covers_panics` in `crates/testkit/tests/integration_buckets.rs`; expect missing per-test bucket isolation/awaited cleanup; this proves fixtures, not GR5 media delivery (FR-024, FR-CON-37, GR5).
- [ ] T016 Make pass: T015 implement owned bucket creation/normal-error-panic teardown in `crates/testkit/src/fixtures/buckets.rs`; close fixture clients before returning (FR-024, FR-CON-37, GR5).
- [ ] T017 Write failing test: [unit] `ci_cleanup_matches_only_current_run_labels` in `scripts/tests/test_test_down.py`; expect missing cleanup or acceptance of an active/foreign run (FR-024, FR-CON-37).
- [ ] T018 Make pass: T017 create `scripts/test-down.sh` and update `justfile` for explicit local cleanup and always-run exact-label CI cleanup; no prune or broad database sweep (FR-024, FR-CON-37).
- [ ] T019 Write failing test: [unit] `compose_keeps_migration_and_runtime_roles_separate` in `scripts/tests/test_compose.py`; expect missing healthy PostgreSQL 16/MinIO services, protected credential references or separation from test fixture ownership (FR-024, FR-CON-37).
- [ ] T020 Make pass: T019 create `deploy/compose.yml` with health checks and pinned service images; no privileged application role or baked-in secrets (FR-024, FR-CON-37).
- [ ] T021 Write failing test: [unit] `ci_requires_fast_and_adapter_integration_gates` in `scripts/tests/test_ci.py`; expect missing fmt/clippy/deny/architecture/nextest gates, offline SQL settings and always-run fixture teardown (FR-024, FR-CON-37).
- [ ] T022 Make pass: T021 create `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`; pin tooling/actions, run fast tests on PRs and integration on infra-* changes/nightly, and report absent future crates rather than green skipped claims (FR-024, FR-CON-37).
- [ ] T023 Write failing test: [unit] `traceability_checker_rejects_implementation_without_driving_test` in `scripts/tests/test_traceability.py`; expect absent/misordered test IDs, missing ACs and unpaired ports to pass incorrectly (FR-001, FR-038, FR-CON-37).
- [ ] T024 Make pass: T023 implement `scripts/check-test-traceability.py` over `specs/001-admin-foundation/tasks.md` and named tests; fail incomplete references and premature completion, not merely missing optional future-feature tests (FR-001, FR-038, FR-CON-37).
- [ ] T025 Verify checkpoint: run repository checks and `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Record only observed results in `specs/001-admin-foundation/tasks.md` (FR-CON-01, FR-CON-37).

## Phase 2: Foundational — Kernel ports, safe composition and web/API base

**Purpose**: Shared executable infrastructure blocks all story work; only shell/status routes
are delivered here. No placeholder protected management route or completed sign-in is claimed.
**Independent test**: kernel contracts pass on fakes and local/transactional real adapters;
configuration forbids production fakes; bilingual shell and minimal documented JSON are tested.
HealthProbe's real seed-ready branch completes with actual registry storage in US2, not a stub.

### Kernel values and contract harness

- [ ] T026 [P] Write failing test: [unit] `uuid_v7_ids_reject_wrong_version` in `crates/kernel/tests/ids.rs`; expect missing wrappers for AdminId, CredentialId, SessionId, ChallengeId, ChangeId, RevisionId, PreviewId, ApprovalId, PublicationId, VersionId and AuditId, and acceptance of non-v7 values (FR-004, FR-012, FR-CON-37).
- [ ] T027 [P] Write failing test: [unit] `locale_direction_is_derived` in `crates/kernel/tests/presentation.rs`; expect missing fa/en locale, RTL/LTR derivation and light/dark/system values (FR-025, FR-027, FR-UX-01).
- [ ] T028 Make pass: T026 implement checked UUIDv7 wrappers in `crates/kernel/src/ids.rs`, register `crates/kernel/src/lib.rs`; browser capability tokens are not UUID IDs (FR-004, FR-012, FR-CON-37).
- [ ] T029 Make pass: T027 implement locale/direction/theme values in `crates/kernel/src/locale.rs`, register `crates/kernel/src/lib.rs`; no independently editable direction (FR-025, FR-027, FR-UX-01).
- [ ] T030 Write failing test: [unit] `domain_errors_do_not_expose_infrastructure_secrets` in `crates/kernel/tests/errors.rs`; expect missing validation/auth/conflict/unavailable/invalid-handle/retryable/unknown-commit taxonomy and unsafe Debug payloads (FR-024, FR-CON-37).
- [ ] T031 Make pass: T030 implement typed sanitized errors in `crates/kernel/src/error.rs`, register `crates/kernel/src/lib.rs`; web later localizes them (FR-024, FR-CON-37).
- [ ] T032 Write failing test: [unit] `testkit_features_do_not_create_context_cycles` in `scripts/tests/test_testkit_features.py`; expect missing fakes/contracts feature gates or context→testkit dependency (FR-024, FR-CON-37; constitution III).
- [ ] T033 Make pass: T032 extend `crates/testkit/Cargo.toml`, `crates/testkit/src/lib.rs`, `crates/testkit/src/contracts/mod.rs`, `crates/testkit/src/fakes/mod.rs`; enable fakes only explicitly, gate fake integration targets with required-features, keep generic suites infra-independent and enable kernel/admin mocks through testkit dev-dependencies as contexts arrive; CI must run those targets with fakes enabled (FR-024, FR-CON-37).

### Clock, IdGenerator and UnitOfWork

- [ ] T034 Write failing test: [unit] `clock_port_is_async_and_automockable` in `crates/kernel/tests/clock_port.rs`; expect missing async Clock trait/MockClock or wrong macro order (FR-013, FR-CON-32, P41).
- [ ] T035 Make pass: T034 define Clock::now in `crates/kernel/src/ports/clock.rs`, `crates/kernel/src/ports/mod.rs`, `crates/kernel/src/lib.rs`, `crates/kernel/Cargo.toml` with the required macro order and mocks feature (FR-013, FR-CON-32, P41).
- [ ] T036 Write failing test: [contract] `clock_contract` in `crates/testkit/src/contracts/clock.rs` and its runner `crates/testkit/tests/clock_fake.rs`; expect absent FixedClock; assert UTC/error taxonomy and fixture-controlled boundaries without assuming wall-clock monotonicity (FR-013, FR-CON-32, P41).
- [ ] T037 Make pass: T036 implement explicit advance/set FixedClock in `crates/testkit/src/fakes/clock.rs`, register `crates/testkit/src/fakes/mod.rs` and `crates/testkit/src/contracts/mod.rs`; contract green on fake (FR-013, FR-CON-32, P41).
- [ ] T038 Write failing test: [unit] `id_generator_port_is_async_and_automockable` in `crates/kernel/tests/id_generator_port.rs`; expect missing typed async generator/MockIdGenerator (FR-012, FR-CON-37).
- [ ] T039 Make pass: T038 define IdGenerator in `crates/kernel/src/ports/id_generator.rs`, register `crates/kernel/src/ports/mod.rs`; no uuid generation in contexts (FR-012, FR-CON-37).
- [ ] T040 Write failing test: [contract] `id_generator_contract` in `crates/testkit/src/contracts/id_generator.rs` and `crates/testkit/tests/id_generator_fake.rs`; expect absent deterministic sequence/exhaustion errors; assert UUIDv7 and uniqueness, not identical real random values (FR-012, FR-CON-37).
- [ ] T041 Make pass: T040 implement deterministic IdGenerator in `crates/testkit/src/fakes/id_generator.rs`, register testkit module indexes; pass the contract without system time/randomness (FR-012, FR-CON-37).
- [ ] T042 Write failing test: [unit] `uow_port_has_consuming_transaction_lifecycle` in `crates/kernel/tests/unit_of_work_port.rs`; expect missing async begin/commit/rollback, opaque handle/store identity and mocks (FR-021, FR-024, FR-CON-37).
- [ ] T043 Make pass: T042 define UnitOfWork and infrastructure-neutral TxHandle in `crates/kernel/src/ports/unit_of_work.rs`, register `crates/kernel/src/ports/mod.rs`; finished/wrong handles return typed errors, not panics (FR-021, FR-024, FR-CON-37).
- [ ] T044 Write failing test: [contract] `unit_of_work_contract` in `crates/testkit/src/contracts/unit_of_work.rs` and `crates/testkit/tests/unit_of_work_fake.rs`; expect absent commit/rollback/drop isolation, wrong-store refusal and unknown-outcome handling (FR-021, FR-024, FR-CON-37).
- [ ] T045 Make pass: T044 implement shared serialized transactional FakeStore/UnitOfWork in `crates/testkit/src/fakes/store.rs`, `crates/testkit/src/fakes/unit_of_work.rs` and testkit module indexes; rollback/drop discards working state across repositories (FR-021, FR-024, FR-CON-37).
- [ ] T046 Write failing test: [mock] [fake] `health_transaction_reads_once_and_rolls_back_on_failure` in `crates/testkit/tests/application_health_uow.rs`; expect missing read-only application entry point, duplicate begin, write from a read-only service or leaked handle (FR-001, FR-CON-01).
- [ ] T047 Make pass: T046 add minimal admin crate and read-UoW application helper in `crates/admin/Cargo.toml`, `crates/admin/src/lib.rs`, `crates/admin/src/application/mod.rs`, `crates/admin/src/application/read_transaction.rs`, plus workspace/testkit dependency wiring; no SQL/clock generation in the service (FR-001, FR-CON-01).
- [ ] T048 Write failing test: [contract] [integration] `system_clock_and_uuid_sources_pass_shared_contracts` in `crates/infra-system/tests/system_ports.rs`; expect absent real adapters after T037/T041 are green (FR-012, FR-013, FR-CON-37).
- [ ] T049 Make pass: T048 add `crates/infra-system/Cargo.toml`, `crates/infra-system/src/lib.rs`, `crates/infra-system/src/clock.rs`, `crates/infra-system/src/id_generator.rs` and workspace membership; real UTC/UUIDv7 generation exists only here (FR-012, FR-013, FR-CON-37).
- [ ] T050 Write failing test: [contract] [integration] `postgres_uow_passes_shared_contract` in `crates/infra-postgres/tests/integration_unit_of_work.rs`; expect absent real transaction owner; use a test-owned scratch fixture to prove commit/rollback/drop/independent connections without inventing production tables (FR-021, FR-024, FR-CON-37).
- [ ] T051 Make pass: T050 add `crates/infra-postgres/Cargo.toml`, `crates/infra-postgres/src/lib.rs`, `crates/infra-postgres/src/unit_of_work.rs`, `crates/infra-postgres/migrations/.gitkeep`, workspace membership and canonical MIGRATOR; lifecycle and runtime pool checks stay inside adapter (FR-021, FR-024, FR-CON-37).
- [ ] T052 Write failing test: [integration] `sqlx_cases_get_distinct_migrated_databases` in `crates/infra-postgres/tests/integration_fixture_isolation.rs`; expect absent database/role ownership and pool cleanup verification, including retained failed database until same-case rerun (FR-024, FR-CON-37).
- [ ] T053 Make pass: T052 implement caller-supplied pool/ownership helpers in `crates/testkit/src/fixtures/database.rs`, register `crates/testkit/src/lib.rs`; no infra-postgres imports or production migration copies (FR-024, FR-CON-37).

### Configuration and dev/release safety

- [ ] T054 Write failing test: [unit] `production_rejects_any_fake_adapter_group` in `crates/app/tests/adapter_config.rs`; expect missing config validation for real/fake group selection, unknown modes and mixed transactional backends (FR-001, FR-CON-01; constitution III).
- [ ] T055 Make pass: T054 add `crates/app/Cargo.toml`, `crates/app/src/lib.rs`, `crates/app/src/config.rs` and workspace entry; parse TANKHOR_ADAPTERS plus per-group overrides, reject production fakes and incoherent shared-store/UoW groups before connecting (FR-001, FR-CON-01).
- [ ] T056 Write failing test: [unit] `release_graph_excludes_mocks_and_fakes` in `scripts/tests/test_release_features.py`; expect default/release feature graph to include forbidden testkit or context mocks, or permit release dev-fakes (FR-001, FR-CON-01).
- [ ] T057 Make pass: T056 add `scripts/check-release-features.py`, guard debug-only dev-fakes in `crates/app/src/lib.rs`, and update `crates/app/Cargo.toml`, `.github/workflows/ci.yml`; inspect cargo tree feature closure and a negative release-build invocation (FR-001, FR-CON-01).

### Bilingual shell and design tokens

- [ ] T058 Write failing test: [unit] `token_set_matches_authoritative_semantic_roles` in `scripts/tests/test_design_tokens.py`; expect absent/extra roles versus tech spec §12.2 and missing source values for type/spacing/radii/icons/motion (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-047).
- [ ] T059 Make pass: T058 create `crates/web/design/tokens.toml`; encode exactly the sixteen semantic colour names in §12.2 for both themes, approved §12.3 scales and declared pair/large-text/control roles; keep proposed motion values labeled proposed (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-047).
- [ ] T060 Write failing test: [unit] `token_css_is_generated_and_no_stray_hex_is_accepted` in `scripts/tests/test_generated_tokens.py`; inject a stray literal in a disposable template/CSS fixture; expect absent generator/scanner or independently defined visual values (FR-038, FR-UX-20, AC-ADM-047).
- [ ] T061 Make pass: T060 add `crates/web/Cargo.toml`, `crates/web/src/lib.rs`, `crates/web/build.rs`, `scripts/check-design-values.py`, workspace membership; generate CSS into OUT_DIR from tokens only, scan authored templates/CSS/brand SVG against stray literals, and preserve supplied artwork without inventing brand assets (FR-038, FR-UX-20, AC-ADM-047).
- [ ] T062 Write failing test: [unit] `visual_identity_values_have_one_source_and_checked_contrast` and `foundation_brand_and_contrast_follow_approved_baseline` in `crates/web/tests/design_tokens.rs`; mutate a pair below 4.5/3.0 and expect failure; control/focus 3:1, unrounded ratios, border never sole boundary (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-029, AC-ADM-047).
- [ ] T063 Make pass: T062 implement token parsing/sRGB contrast/pair validation in `crates/web/src/design.rs`; read values from TOML, not duplicated test constants; forbid white-small-text/light-sky roles (FR-028, FR-038, FR-UX-07, FR-UX-20, AC-ADM-029, AC-ADM-047).
- [ ] T064 Write failing test: [unit] `foundation_copy_has_fa_en_parity_and_context` in `crates/web/tests/fluent.rs`; expect absent surface/situation keys, mismatched variables and silent fallback (FR-030, FR-UX-01, FR-UX-14, AC-ADM-031).
- [ ] T065 Make pass: T064 implement Fluent loading in `crates/web/src/i18n.rs`, `crates/web/locales/fa/console.ftl`, `crates/web/locales/en/console.ftl`; both locales include base shell, theme, validation and accessible-name keys (FR-030, FR-UX-01, FR-UX-14, AC-ADM-031).
- [ ] T066 Write failing test: [http] `base_shell_has_locale_theme_and_accessible_structure` in `crates/web/tests/shell.rs`; instantiate Askama for six locale/theme choices; expect missing lang/dir, one h1, labels or CSS logical properties (FR-025, FR-027, FR-029, FR-UX-01, FR-UX-08).
- [ ] T067 Make pass: T066 implement `crates/web/templates/console/layout.html`, `crates/web/templates/partials/field.html`, `crates/web/src/shell.rs`, `crates/web/static/console.css`; use generated tokens, self-hosted Vazirmatn and local htmx with licenses under `crates/web/static/vendor/` as exact asset filenames `vazirmatn.woff2`, `OFL.txt`, `htmx.min.js`, `htmx-LICENSE.txt`; no invented console logo (FR-025, FR-027, FR-029, FR-UX-01, FR-UX-06, Q03).
- [ ] T068 Write failing test: [http] `csrf_is_required_for_every_mutating_console_form` in `crates/web/tests/csrf.rs`; expect missing origin/token checks and insecure cookie attributes; use a test-only mutation route, not a placeholder production action (FR-001, FR-002, FR-CON-02, Q23).
- [ ] T069 Make pass: T068 implement pure CSRF/cookie transport validation in `crates/web/src/security.rs`; middleware receives browser-bound expected token material from application context (synthetic context only in this foundation test); it generates no randomness and calls no port directly; Secure/HttpOnly/SameSite, host-only cookies, no GET mutation (FR-001, FR-002, FR-CON-02, Q23).

### Health port, API contracts and offline export

- [ ] T070 Write failing test: [unit] `health_probe_is_an_async_mockable_port` in `crates/admin/src/ports/health_probe.rs`; expect missing trait/automock for sanitized store-and-seed readiness (FR-001, FR-CON-01).
- [ ] T071 Make pass: T070 define HealthProbe in `crates/admin/src/ports/health_probe.rs`, `crates/admin/src/ports/mod.rs`, `crates/admin/src/lib.rs`; owned ready/unavailable DTO contains no server/provider details (FR-001, FR-CON-01).
- [ ] T072 Write failing test: [contract] `health_probe_contract` in `crates/testkit/src/contracts/health_probe.rs`, runner `crates/testkit/tests/health_probe_fake.rs`; expect absent seeded/unseeded/unavailable fixtures and typed errors (FR-001, FR-CON-01).
- [ ] T073 Make pass: T072 implement HealthProbe fake in `crates/testkit/src/fakes/health_probe.rs` and module indexes; explicitly configured fixture readiness, never infer success from an absent backend (FR-001, FR-CON-01).
- [ ] T074 Write failing test: [mock] [fake] `get_health_calls_probe_once_and_fails_closed` in `crates/testkit/tests/application_health.rs`; expect missing service, repeated calls or unavailable→ready conversion (FR-001, FR-CON-01).
- [ ] T075 Make pass: T074 implement `crates/admin/src/application/health.rs`, register `crates/admin/src/application/mod.rs`; service depends only on HealthProbe (FR-001, FR-CON-01).
- [ ] T076 Write failing test: [http] `health_and_problem_responses_match_schema` in `crates/web/tests/health.rs`; expect undocumented/non-v1 JSON or leaked infrastructure details; assert 200 ready and 503 problem+json unavailable (FR-001, FR-CON-01; constitution VI).
- [ ] T077 Make pass: T076 implement annotated health handler, shared RFC 9457 errors and OpenApiRouter in `crates/web/src/api/health.rs`, `crates/web/src/api/problem.rs`, `crates/web/src/api/mod.rs` and `crates/web/src/lib.rs`; no JSON auth/management API added (FR-001, FR-CON-01).
- [ ] T078 Write failing test: [http] `schema_endpoint_describes_itself_finitely` and `all_json_routes_are_documented_under_v1` in `crates/web/tests/openapi.rs`; expect missing schema route or recursive document embedding (FR-001, FR-CON-01; constitution VI).
- [ ] T079 Make pass: T078 implement frozen schema bridge/router builder in `crates/web/src/api/openapi.rs`, register `crates/web/src/api/mod.rs`; annotate `/api/v1/openapi.json`, split router/document once, no undocumented `/api/openapi.json` (FR-001, FR-CON-01).
- [ ] T080 Write failing test: [http] `swagger_makes_no_external_asset_or_validator_requests` in `crates/web/tests/swagger.rs`; expect missing local `/api/docs`, remote validator/assets or an undocumented extra schema URL (FR-001, FR-CON-01; constitution VI).
- [ ] T081 Make pass: T080 implement vendored Swagger UI with external validator disabled in `crates/web/src/api/docs.rs`, `crates/web/src/api/mod.rs`, `crates/web/Cargo.toml`; production auth wrapper is driven in US1 once real session services exist (FR-001, FR-CON-01).
- [ ] T082 Write failing test: [unit] `offline_openapi_export_needs_no_database` in `crates/app/tests/openapi_export.rs`; expect missing CLI/export or an attempted database/provider/config connection (FR-001, FR-CON-01; constitution VI).
- [ ] T083 Make pass: T082 add `crates/app/src/main.rs`, `crates/app/src/openapi.rs`, app/web dependency wiring in `crates/app/Cargo.toml`; `tankhor openapi` uses the same builder with no live state construction (FR-001, FR-CON-01).
- [ ] T084 Write failing test: [http] `openapi_snapshot_matches_router` in `crates/web/tests/openapi_snapshot.rs`; expect missing committed generated document or schema drift, with a pinned local meta-schema in `crates/web/tests/fixtures/openapi-meta-schema.json` (FR-001, FR-CON-01; constitution VI).
- [ ] T085 Make pass: T084 generate `docs/openapi.json` via `cargo run -p app -- openapi`, add offline regeneration/diff to `.github/workflows/ci.yml`; never hand-edit the generated schema (FR-001, FR-CON-01).
- [ ] T086 Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Record actual kernel/fake/real/HTTP results in `specs/001-admin-foundation/tasks.md`; story auth/management and real health seeding remain explicitly undelivered (FR-CON-01, FR-CON-37, FR-UX-20).

## Phase 3: User Story 1 — Enter and Recover the Console Securely (P1, MVP)

**Goal**: Real admin-only primary proof plus passkey/TOTP, separate sessions, another-admin
invitation/recovery, immediate target-only revocation and attributable access.
**Independent test**: AC-ADM-001–007/043 with controlled fakes, then real local crypto,
PostgreSQL and wiremock SMS; neither ordinary cookies nor missing/invalid factors grant access.

### Pure access domain

- [ ] T087 [US1] Write failing test: [unit] `security_policy_is_finite_and_nondisableable` in `crates/admin/src/domain/security_policy.rs`; expect missing/zero expiry, attempt, session or abuse limits and invalid RP/origin configuration to pass (FR-002, FR-011, FR-CON-02, Q09, Q23).
- [ ] T088 [US1] Make pass: T087 implement validated operator policy in `crates/admin/src/domain/security_policy.rs`, `crates/admin/src/domain/mod.rs`, `crates/admin/src/lib.rs`; no invented business defaults or editable second-factor switch (FR-002, FR-011, FR-CON-02, Q23).
- [ ] T089 [US1] Write failing test: [unit] `audit_excludes_authentication_secrets_and_private_data` in `crates/admin/src/domain/audit.rs`; expect arbitrary snapshots/Debug secrets, missing actor/action/subject/time/reason and unrestricted reason text (FR-021, FR-024, FR-CON-37, AC-ADM-024).
- [ ] T090 [US1] Make pass: T089 implement AuditEntry, allowlisted metadata, redacted secret wrappers and bilingual ReasonCode identifiers in `crates/admin/src/domain/audit.rs`, `crates/admin/src/domain/secrets.rs`, domain module index; never store raw phone/code/key/photo/payment content (FR-021, FR-024, FR-CON-37, AC-ADM-024).
- [ ] T091 [US1] Write failing test: [unit] `access_requires_primary_factor_and_current_epoch` in `crates/admin/src/domain/access.rs`; proptest proof/expiry/revocation transitions, equal permissions, fresh enrollment after recovery and last-admin lockout; expect missing access policy (FR-002, FR-004, FR-005, FR-CON-02, AC-ADM-002, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T092 [US1] Make pass: T091 implement pure Admin/Credential/Session/AuthState/OTP state transitions in `crates/admin/src/domain/access.rs`, register domain index; explicit clock/IDs, opaque token digests, no crypto library/IO (FR-002, FR-004, FR-005, FR-CON-02, AC-ADM-002, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).

### Port traits, contracts and fakes — no PostgreSQL code

- [ ] T093 [US1] Write failing test: [unit] `audit_repository_port_is_mockable_and_append_only` in `crates/admin/src/ports/audit_repository.rs`; expect missing async append/search/MockAuditRepository or an edit/delete API (FR-021, FR-022, FR-023, FR-CON-37).
- [ ] T094 [US1] Make pass: T093 define AuditRepository in `crates/admin/src/ports/audit_repository.rs`, register `crates/admin/src/ports/mod.rs`; bounded cursor/filter DTOs and shared TxHandle only (FR-021, FR-022, FR-023, FR-CON-37).
- [ ] T095 [US1] Write failing test: [contract] `audit_repository_contract` in `crates/testkit/src/contracts/audit_repository.rs`, runner `crates/testkit/tests/audit_repository_fake.rs`; expect missing event-key uniqueness, transactional failure rollback, actor/action/subject/time filters and `(occurred_at,id)` ordering (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025).
- [ ] T096 [US1] Make pass: T095 implement `crates/testkit/src/fakes/audit_repository.rs`, extend `crates/testkit/src/fakes/store.rs` and testkit indexes; append-only fake and failure injection share the UoW store (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025).
- [ ] T097 [US1] Write failing test: [unit] `admin_repository_port_is_async_and_mockable` in `crates/admin/src/ports/admin_repository.rs`; expect missing digest lookup/safe bounded list/get/lock/invite/epoch-CAS surface (FR-004, FR-005, FR-CON-03).
- [ ] T098 [US1] Make pass: T097 define AdminRepository in `crates/admin/src/ports/admin_repository.rs`, register port index; no plaintext identity in returned audit DTOs (FR-004, FR-005, FR-CON-03).
- [ ] T099 [US1] Write failing test: [contract] `admin_repository_contract` in `crates/testkit/src/contracts/admin_repository.rs`, runner `crates/testkit/tests/admin_repository_fake.rs`; expect absent unique keyed identity, safe bounded listing, eligibility, epoch CAS and stable sorted locking (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043).
- [ ] T100 [US1] Make pass: T099 implement `crates/testkit/src/fakes/admin_repository.rs`, extend shared store and indexes; reject wrong-store/finished handles and duplicate identities (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043).
- [ ] T101 [US1] Write failing test: [unit] `admin_credential_repository_port_is_mockable` in `crates/admin/src/ports/admin_credential_repository.rs`; expect missing active-list/insert/lock/revision-CAS/accepted-step-CAS/revoke-all (FR-002, FR-004, FR-CON-02).
- [ ] T102 [US1] Make pass: T101 define AdminCredentialRepository in `crates/admin/src/ports/admin_credential_repository.rs`, register port index; library credential/secret material remains opaque and redacted (FR-002, FR-004, FR-CON-02).
- [ ] T103 [US1] Write failing test: [contract] `admin_credential_repository_contract` in `crates/testkit/src/contracts/admin_credential_repository.rs`, runner `crates/testkit/tests/admin_credential_repository_fake.rs`; expect absent global credential uniqueness, stale revision/step rejection and target-only revocation (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T104 [US1] Make pass: T103 implement `crates/testkit/src/fakes/admin_credential_repository.rs`, shared store/index wiring; consume enrollment TOTP step as well as sign-in steps (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T105 [US1] Write failing test: [unit] `session_store_port_is_async_and_mockable` in `crates/admin/src/ports/session_store.rs`; expect missing insert/digest-lookup/current-check/revoke-one/revoke-all surface (FR-002, FR-003, FR-CON-02).
- [ ] T106 [US1] Make pass: T105 define SessionStore in `crates/admin/src/ports/session_store.rs`, register port index; sessions carry factor, epoch and finite absolute/idle expiry (FR-002, FR-003, FR-CON-02).
- [ ] T107 [US1] Write failing test: [contract] `session_store_contract` in `crates/testkit/src/contracts/session_store.rs`, runner `crates/testkit/tests/session_store_fake.rs`; expect absent expiry/epoch/target-only revocation and accidental raw capability persistence (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043).
- [ ] T108 [US1] Make pass: T107 implement `crates/testkit/src/fakes/session_store.rs`, shared store/index wiring; repeat revocation is harmless and access checks observe committed recovery immediately (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043).
- [ ] T109 [US1] Write failing test: [unit] `auth_state_repository_port_is_mockable` in `crates/admin/src/ports/auth_state_repository.rs`; expect missing create/read/consume/supersede/revoke with browser/purpose/epoch binding (FR-002, FR-004, FR-CON-02).
- [ ] T110 [US1] Make pass: T109 define AuthStateRepository in `crates/admin/src/ports/auth_state_repository.rs`, register port index; opaque encrypted server state never becomes a browser DTO (FR-002, FR-004, FR-CON-02).
- [ ] T111 [US1] Write failing test: [contract] `auth_state_repository_contract` in `crates/testkit/src/contracts/auth_state_repository.rs`, runner `crates/testkit/tests/auth_state_repository_fake.rs`; expect replay, wrong browser/purpose/epoch/expiry or stale credential snapshot acceptance (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T112 [US1] Make pass: T111 implement `crates/testkit/src/fakes/auth_state_repository.rs`, shared store/index wiring; one-use proofs/grants/ceremonies and failure categories match real contract (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T113 [US1] Write failing test: [unit] `otp_challenge_repository_port_is_mockable` in `crates/admin/src/ports/otp_challenge_repository.rs`; expect missing superseding issue/failed-attempt-CAS/one-use consume/safe submission status (FR-002, FR-CON-02, Q09).
- [ ] T114 [US1] Make pass: T113 define OtpChallengeRepository in `crates/admin/src/ports/otp_challenge_repository.rs`, register port index; keyed digest only, original expiry retained across transport retries (FR-002, FR-CON-02, Q09).
- [ ] T115 [US1] Write failing test: [contract] `otp_challenge_repository_contract` in `crates/testkit/src/contracts/otp_challenge_repository.rs`, runner `crates/testkit/tests/otp_challenge_repository_fake.rs`; expect expiry/supersession/replay and concurrent consume to fail open (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T116 [US1] Make pass: T115 implement `crates/testkit/src/fakes/otp_challenge_repository.rs`, shared store/index wiring; failed-attempt budgets cannot reset on resend/failover (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T117 [US1] Write failing test: [unit] `rate_limit_repository_port_is_mockable` in `crates/admin/src/ports/rate_limit_repository.rs`; expect missing atomic keyed subject/origin bucket increment/read (FR-002, FR-CON-02, Q09).
- [ ] T118 [US1] Make pass: T117 define RateLimitRepository in `crates/admin/src/ports/rate_limit_repository.rs`, register port index; no raw phone/IP in errors or diagnostics (FR-002, FR-CON-02, Q09).
- [ ] T119 [US1] Write failing test: [contract] `rate_limit_repository_contract` in `crates/testkit/src/contracts/rate_limit_repository.rs`, runner `crates/testkit/tests/rate_limit_repository_fake.rs`; expect concurrency, finite-window boundaries or resend/failover to bypass budgets (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T120 [US1] Make pass: T119 implement `crates/testkit/src/fakes/rate_limit_repository.rs`, shared store/index wiring; deterministic Clock controls windows (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T121 [US1] Write failing test: [unit] `admin_authenticator_port_is_mockable_and_secret_safe` in `crates/admin/src/ports/admin_authenticator.rs`; expect missing passkey/TOTP/OTP-digest/token/envelope operations or exposed vendor structs (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T122 [US1] Make pass: T121 define AdminAuthenticator in `crates/admin/src/ports/admin_authenticator.rs`, register port index; all randomness/challenges/crypto cross this port, IDs/time through their own ports (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T123 [US1] Write failing test: [contract] `admin_authenticator_contract` in `crates/testkit/src/contracts/admin_authenticator.rs`, runner `crates/testkit/tests/admin_authenticator_fake.rs`; expect missing valid/invalid fixture proof, RP/origin/binding checks, matched TOTP steps and redacted errors (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T124 [US1] Make pass: T123 implement fixture-bound `crates/testkit/src/fakes/admin_authenticator.rs` and testkit indexes; exact controlled proofs only, never unconditional success; pass common fake contract before crypto adapter work (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T125 [US1] Write failing test: [unit] `sms_sender_port_is_mockable` in `crates/admin/src/ports/sms_sender.rs`; expect missing approved-template send with safe accepted/rejected/unavailable/unknown-result categories (FR-002, FR-CON-02, Q09).
- [ ] T126 [US1] Make pass: T125 define SmsSender in `crates/admin/src/ports/sms_sender.rs`, register port index; send acceptance is not verification or delivery (FR-002, FR-CON-02, Q09).
- [ ] T127 [US1] Write failing test: [contract] `sms_sender_contract` in `crates/testkit/src/contracts/sms_sender.rs`, runner `crates/testkit/tests/sms_sender_fake.rs`; expect missing controlled accepted/rejected/timeout sequences and capture of the same challenge across failover (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T128 [US1] Make pass: T127 implement capturing `crates/testkit/src/fakes/sms_sender.rs` and indexes; redact captured payload Debug, expose synthetic test inspection without logging (FR-002, FR-CON-02, AC-ADM-002, Q09).

### Application services — mock and fake red tests before code

- [ ] T129 [US1] Write failing test: [mock] `sms_failover_reuses_challenge_and_expiry` in `crates/testkit/tests/mock_request_admin_otp.rs`; sequence rate-limit→eligible-admin→crypto→persist, finish UoW before vendor calls, at most primary+one eligible fallback, no calls for invalid identity and no budget reset (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T130 [US1] Write failing test: [fake] `sms_acceptance_is_not_primary_proof` in `crates/testkit/tests/fake_request_admin_otp.rs`; expect absent non-enumerating request flow and transport acceptance granting access; include resend supersession and unknown delivery outcome (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T131 [US1] Make pass: T129 T130 implement RequestAdminOtp in `crates/admin/src/application/request_admin_otp.rs`, register application index; application orchestrates two sender ports, never adapter→adapter or network while locked (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T132 [US1] Write failing test: [mock] `invalid_primary_proof_never_issues_session` in `crates/testkit/tests/mock_verify_admin_otp.rs`; expect missing once-only consume/attempt update and zero full-session insert on invalid/expired code (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T133 [US1] Write failing test: [fake] `primary_proof_is_browser_bound_and_one_use` in `crates/testkit/tests/fake_verify_admin_otp.rs`; expect absent restricted proof, expiry/rate/epoch enforcement and replay protection (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T134 [US1] Make pass: T132 T133 implement VerifyAdminOtp in `crates/admin/src/application/verify_admin_otp.rs`, application index; no customer account creation or completed admin session yet (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T135 [US1] Write failing test: [mock] `failed_second_factor_never_issues_session` in `crates/testkit/tests/mock_passkey.rs`; expect missing verify→credential/state-CAS→session→audit-once ordering and rollback when audit fails (FR-002, FR-024, FR-CON-02, AC-ADM-002, AC-ADM-025).
- [ ] T136 [US1] Write failing test: [fake] `console_requires_completed_second_factor` in `crates/testkit/tests/fake_passkey.rs`; expect missing bound begin/finish passkey flow, stale credential/epoch refusal and persisted counter/backup updates (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T137 [US1] Make pass: T135 T136 implement BeginPasskey/FinishPasskey in `crates/admin/src/application/passkey.rs`, application index; one-use server-state check and final session/audit UoW, crypto outside locks (FR-002, FR-024, FR-CON-02, AC-ADM-002, AC-ADM-025).
- [ ] T138 [US1] Write failing test: [mock] `totp_step_and_session_audit_commit_once` in `crates/testkit/tests/mock_totp.rs`; expect missing accepted-step CAS before session and exactly one audit; no writes after invalid proof (FR-002, FR-021, FR-CON-02, AC-ADM-002).
- [ ] T139 [US1] Write failing test: [fake] `totp_fallback_rejects_replayed_or_expired_proof` in `crates/testkit/tests/fake_totp.rs`; expect absent fallback with primary/epoch checks and replayed enrollment/sign-in step rejection (FR-002, FR-CON-02, AC-ADM-002).
- [ ] T140 [US1] Make pass: T138 T139 implement FinishTotp in `crates/admin/src/application/totp.rs`, application index; fallback still requires primary proof and audited final UoW (FR-002, FR-021, FR-CON-02, AC-ADM-002).
- [ ] T141 [US1] Write failing test: [mock] `enrollment_consumes_bound_grant_and_audits_once` in `crates/testkit/tests/mock_enrollment.rs`; expect missing credential/state writes ordering and rejection before writes for wrong actor/browser/epoch (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005).
- [ ] T142 [US1] Write failing test: [fake] `enrollment_grant_is_not_console_access` in `crates/testkit/tests/fake_enrollment.rs`; expect missing passkey/TOTP begin/confirm, primary-proof requirement and one-use enrollment step (FR-004, FR-CON-03, AC-ADM-004, AC-ADM-005).
- [ ] T143 [US1] Make pass: T141 T142 implement BeginEnrollment/FinishEnrollment in `crates/admin/src/application/enrollment.rs`, application index; protected pages remain unavailable until fresh factor verification (FR-004, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-005).
- [ ] T144 [US1] Write failing test: [mock] `invitation_requires_other_admin_and_one_audit` in `crates/testkit/tests/mock_invite_admin.rs`; expect identity/authorization validation before create/grant/audit and zero writes for self-registration; the accompanying ListAdmins query must authorize before one bounded list call and perform no writes (FR-004, FR-005, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-007).
- [ ] T145 [US1] Write failing test: [fake] `admin_addition_requires_another_admin_and_audit` and `admins_have_equal_permissions_and_distinct_attribution` in `crates/testkit/tests/fake_invite_admin.rs`; expect missing another-admin invitation/reason, duplicate identity handling, equal permissions or a safe ListAdmins presentation query showing distinct internal actors without phone/credential material (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007).
- [ ] T146 [US1] Make pass: T144 T145 implement InviteAdmin and its bounded read-only ListAdmins query in `crates/admin/src/application/invite_admin.rs`, application index; invited eligibility never directly grants a full session and the listing discloses only safe actor/enrollment status DTOs (FR-004, FR-005, FR-021, FR-CON-03, AC-ADM-004, AC-ADM-007).
- [ ] T147 [US1] Write failing test: [mock] `recovery_only_revokes_target_admin` in `crates/testkit/tests/mock_recover_admin.rs`; expect missing sorted-admin locks→epoch bump→all target session/credential/proof/OTP revocations→restricted grant→one audit and rollback on failure (FR-004, FR-024, FR-CON-03, AC-ADM-025, AC-ADM-043).
- [ ] T148 [US1] Write failing test: [fake] `admin_recovery_preserves_second_factor_requirement`, `last_admin_lockout_has_no_bypass`, `recovery_revokes_all_admin_sessions_and_old_factors` in `crates/testkit/tests/fake_recover_admin.rs`; expect absent target-only atomic revocation, fresh primary/enrollment requirement or last-admin bypass (FR-004, FR-CON-03, AC-ADM-005, AC-ADM-006, AC-ADM-043).
- [ ] T149 [US1] Make pass: T147 T148 implement RecoverAdmin in `crates/admin/src/application/recover_admin.rs`, application index; commit audit/revocation/grant together and return no unaudited success (FR-004, FR-024, FR-CON-03, AC-ADM-005, AC-ADM-006, AC-ADM-025, AC-ADM-043).
- [ ] T150 [US1] Write failing test: [mock] `session_access_rechecks_epoch_and_signout_revokes_once` in `crates/testkit/tests/mock_console_access.rs`; expect missing current actor/Clock checks on each request, missing authenticator-supplied browser/CSRF capability for access forms and repeated unintended writes; no authenticator port is exposed to handlers (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-003).
- [ ] T151 [US1] Write failing test: [fake] `console_rejects_non_admin_access`, `end_user_session_does_not_grant_admin_access`, `unauditable_admin_action_is_not_reported_successful` in `crates/testkit/tests/fake_console_access.rs`; expect missing expiry/epoch separation, browser-bound access-form capability context or audit-failure denial (FR-001, FR-002, FR-003, FR-024, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-025).
- [ ] T152 [US1] Make pass: T150 T151 implement console session lifecycle and access-form presentation context in `crates/admin/src/application/console_access.rs`, application index; VerifyConsoleAccess/SignOut and PrepareAccessForm keep capability creation behind AdminAuthenticator, every protected operation rechecks actor validity, and no end-user cookie substitutes for an admin session (FR-001, FR-002, FR-003, FR-024, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-025).

### Real storage — only after all corresponding fake suites pass

- [ ] T153 [US1] Write failing test: [integration] `auth_schema_enforces_identity_proof_and_credential_constraints` in `crates/infra-postgres/tests/integration_auth_schema.rs`; expect absent tables and model constraints: admins "unique lookup digest"; credentials "globally unique WebAuthn credential ID; kind/material XOR; optimistic revision checks"; sessions "unique digest; FK admin; finite expiry"; auth states "one-use/expiry/purpose checks"; OTP "finite expiry/attempt policy; compare-and-set consumption"; limits "unique bucket/window; atomic increment" (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T154 [US1] Make pass: T153 add `crates/infra-postgres/migrations/0001_admin_access.sql`; implement data-model §5 columns/FKs/indexes, encrypted phone/secret envelopes, no raw capability/code, no audit cascade; existing cluster roles are fixture/operator-owned (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T155 [US1] Write failing test: [integration] `runtime_cannot_update_delete_or_truncate_audit`, `audit_guards_reject_privileged_dml`, `runtime_cannot_disable_guards_or_assume_owner` in `crates/infra-postgres/tests/integration_audit_guards.rs`; expect missing audit table/guards; assert "UNIQUE event key" and search indexes `(occurred_at,id)`, `(actor_id,occurred_at,id)`, `(action,occurred_at,id)`, `(subject_id,occurred_at,id)` (FR-021, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-023).
- [ ] T156 [US1] Make pass: T155 add `crates/infra-postgres/migrations/0002_audit.sql`; SELECT/INSERT-only runtime grants, UPDATE/DELETE guards plus statement-level TRUNCATE guard, no trigger/owner escalation and no cascading history mutation (FR-021, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-023).
- [ ] T157 [US1] Write failing test: [contract] [integration] `audit_repository_contract_postgres` in `crates/infra-postgres/tests/integration_audit_repository.rs`; run T095 unchanged, expect missing real append/search and audit-failure rollback (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025).
- [ ] T158 [US1] Make pass: T157 implement `crates/infra-postgres/src/audit_repository.rs`, register adapter index; keyed retry result and bounded stable filtered paging, no mutation methods (FR-021, FR-022, FR-023, FR-CON-37, AC-ADM-021, AC-ADM-022, AC-ADM-023, AC-ADM-025).
- [ ] T159 [US1] Write failing test: [contract] [integration] `admin_repository_contract_postgres` in `crates/infra-postgres/tests/integration_admin_repository.rs`; run T099 unchanged, expect missing keyed identity uniqueness/row locks/epoch CAS (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043).
- [ ] T160 [US1] Make pass: T159 implement `crates/infra-postgres/src/admin_repository.rs`, adapter index; use caller transaction only and sorted admin locks (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-007, AC-ADM-043).
- [ ] T161 [US1] Write failing test: [contract] [integration] `admin_credential_repository_contract_postgres` in `crates/infra-postgres/tests/integration_admin_credential_repository.rs`; run T103 unchanged, expect missing uniqueness/revision/accepted-step/revocation enforcement (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T162 [US1] Make pass: T161 implement `crates/infra-postgres/src/admin_credential_repository.rs`, adapter index; current epoch/revision/step checks remain transactional (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T163 [US1] Write failing test: [contract] [integration] `session_store_contract_postgres` in `crates/infra-postgres/tests/integration_session_store.rs`; run T107 unchanged, expect absent expiry/epoch/current-access checks and target-only revocation (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043).
- [ ] T164 [US1] Make pass: T163 implement `crates/infra-postgres/src/session_store.rs`, adapter index; only capability digests persisted and current admin state joined on access (FR-002, FR-003, FR-004, FR-CON-02, AC-ADM-003, AC-ADM-043).
- [ ] T165 [US1] Write failing test: [contract] [integration] `auth_state_repository_contract_postgres` in `crates/infra-postgres/tests/integration_auth_state_repository.rs`; run T111 unchanged, expect missing conditional consume/browser/purpose/epoch/expiry enforcement (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T166 [US1] Make pass: T165 implement `crates/infra-postgres/src/auth_state_repository.rs`, adapter index; ceremony/enrollment state encrypted and never returned to client serializers (FR-002, FR-004, FR-CON-02, AC-ADM-002, AC-ADM-043).
- [ ] T167 [US1] Write failing test: [contract] [integration] `otp_challenge_repository_contract_postgres` in `crates/infra-postgres/tests/integration_otp_challenge_repository.rs`; run T115 unchanged and race two consumes; expect more than one accepted proof (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T168 [US1] Make pass: T167 implement `crates/infra-postgres/src/otp_challenge_repository.rs`, adapter index; CAS consume/supersede/attempt operations share caller UoW (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T169 [US1] Write failing test: [contract] [integration] `rate_limit_repository_contract_postgres` in `crates/infra-postgres/tests/integration_rate_limit_repository.rs`; run T119 unchanged with concurrent increments and finite window boundaries; expect lost increments (FR-002, FR-CON-02, AC-ADM-002, Q09).
- [ ] T170 [US1] Make pass: T169 implement `crates/infra-postgres/src/rate_limit_repository.rs`, adapter index; no reset on provider switch/resend and no raw identity errors (FR-002, FR-CON-02, AC-ADM-002, Q09).

### Real crypto and SMS — no live vendor requests

- [ ] T171 [US1] Write failing test: [unit] `secret_envelope_binds_admin_and_epoch` and `totp_wrong_envelope_key_fails_closed` in `crates/infra-auth/tests/envelopes.rs`; expect missing AES-256-GCM key-id/nonce/ciphertext-tag handling and AAD checks (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T172 [US1] Make pass: T171 add `crates/infra-auth/Cargo.toml`, `crates/infra-auth/src/lib.rs`, `crates/infra-auth/src/envelopes.rs`, workspace membership; external protected key configuration and unique nonces, no Debug key output (FR-002, FR-024, FR-CON-02, Q23).
- [ ] T173 [US1] Write failing test: [unit] `otp_and_capabilities_are_secure_and_digest_only` in `crates/infra-auth/tests/capabilities.rs`; expect missing six-digit OTP generation/keyed digest checks, independent opaque session/CSRF capabilities and encrypted canonical-phone lookup support (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T174 [US1] Make pass: T173 implement cryptographic randomness, constant-time digest verification and purpose-separated protection in `crates/infra-auth/src/capabilities.rs`, infra-auth module index; UUID IDs are not session capabilities (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T175 [US1] Write failing test: [unit] `totp_reports_matched_step_and_rejects_invalid_code` in `crates/infra-auth/tests/totp.rs`; expect missing RFC6238 SHA1/six digits/30s/one-step skew/160-bit-secret verification using fixed vectors, not loggable codes (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T176 [US1] Make pass: T175 implement totp-rs verification/enrollment primitives in `crates/infra-auth/src/totp.rs`, infra-auth module index; return matched step for application CAS, never claim the library alone prevents replay (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T177 [US1] Write failing test: [unit] `webauthn_rejects_wrong_origin_and_replayed_state`, `valid_zero_counter_passkey_is_supported` in `crates/infra-auth/tests/passkey.rs`; expect missing real signed-fixture start/finish verification, UV/RP/origin/challenge checks and credential update result (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T178 [US1] Make pass: T177 implement webauthn-rs safe API primitives in `crates/infra-auth/src/passkey.rs`, infra-auth module index; protected full server state, no danger-credential-internals or client-serialized ceremony state (FR-002, FR-CON-02, AC-ADM-002, Q23).
- [ ] T179 [US1] Write failing test: [contract] [integration] `admin_authenticator_contract_real_crypto` in `crates/infra-auth/tests/authenticator_contract.rs`; run T123 unchanged with actual local vectors plus invalid/rebound/expired proof cases; expect missing complete port implementation (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T180 [US1] Make pass: T179 implement complete AdminAuthenticator delegation in `crates/infra-auth/src/admin_authenticator.rs`, infra-auth module index; no method stubs and no calls to other infra adapters (FR-002, FR-024, FR-CON-02, AC-ADM-002, Q23).
- [ ] T181 [US1] Write failing test: [contract] [integration] `sms_sender_contract_kavenegar` in `crates/infra-sms/tests/kavenegar.rs`; run T127 unchanged via wiremock, assert approved POST form lookup, timeout/malformed/error categories, redirects refused and key-path/body redaction (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T182 [US1] Make pass: T181 add `crates/infra-sms/Cargo.toml`, `crates/infra-sms/src/lib.rs`, `crates/infra-sms/src/kavenegar.rs`, workspace membership; sensitive approved-template payload only, no raw reqwest error formatting (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T183 [US1] Write failing test: [contract] [integration] `sms_sender_contract_sms_ir` in `crates/infra-sms/tests/sms_ir.rs`; run T127 unchanged via wiremock, assert POST `/v1/send/verify`, sensitive X-API-KEY, configured template/parameter name, redacted rejection/timeout and no redirects (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T184 [US1] Make pass: T183 implement `crates/infra-sms/src/sms_ir.rs`, infra-sms module index; failover remains application orchestration, no sender calls another adapter (FR-002, FR-024, FR-CON-02, Q09).
- [ ] T185 [US1] Write failing test: [integration] `otp_and_totp_consumption_are_single_use_under_race`, `recovery_races_with_login_safely`, `audit_failure_aborts_access_and_publication` in `crates/infra-postgres/tests/integration_access_services.rs`; drive US1 services with real crypto/SQL/wiremock; expect absent cross-adapter harness or concurrency violation (FR-002, FR-004, FR-024, FR-CON-02, AC-ADM-002, AC-ADM-025, AC-ADM-043).
- [ ] T186 [US1] Make pass: T185 add test-only composition/fixtures in `crates/infra-postgres/tests/support/access.rs`, `crates/infra-postgres/Cargo.toml`; exercise existing services without production adapter→adapter dependencies; fix only race behaviour driven by T185 in `crates/admin/src/application/passkey.rs`, `crates/admin/src/application/totp.rs`, `crates/admin/src/application/recover_admin.rs` (FR-002, FR-004, FR-024, FR-CON-02, AC-ADM-002, AC-ADM-025, AC-ADM-043).

### HTML access and story composition

- [ ] T187 [US1] Write failing test: [http] `access_forms_never_grant_protected_access_from_primary_only` in `crates/web/tests/auth_forms.rs`; expect missing localized request/verify-code, passkey begin/finish and TOTP form routes, escaped options, safe redirects and no-store responses (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T188 [US1] Make pass: T187 implement `crates/web/src/console/auth.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/sign_in.html`, `crates/web/templates/console/second_factor.html`, `crates/web/static/passkey.js` and auth keys in both `crates/web/locales/fa/console.ftl` and `crates/web/locales/en/console.ftl`; full-page form paths plus essential passkey bridge, never JSON auth endpoints (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-003).
- [ ] T189 [US1] Write failing test: [http] `enrollment_and_recovery_forms_require_bound_other_admin_grants` in `crates/web/tests/admin_access.rs`; expect missing admin list/invite/recover/enroll/signout routes, own-factor checks, CSRF, hidden-state escaping and one-use enrollment (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T190 [US1] Make pass: T189 implement `crates/web/src/console/admins.rs`, `crates/web/src/console/enrollment.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/admins.html`, `crates/web/templates/console/enrollment.html`, localized keys in FA/EN console resources; administration list uses the read-only application query from T146, never a repository; do not reflect code/shared-secret/assertion during errors/preferences (FR-004, FR-005, FR-CON-03, AC-ADM-004, AC-ADM-005, AC-ADM-006, AC-ADM-007, AC-ADM-043).
- [ ] T191 [US1] Write failing test: [http] `production_docs_and_assets_require_admin` in `crates/web/tests/admin_guard.rs`; expect unprotected console, Swagger redirects/assets or schema, end-user-cookie substitution and stale recovered-session access (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-043).
- [ ] T192 [US1] Make pass: T191 implement application-service auth middleware in `crates/web/src/console/guard.rs`, `crates/web/src/api/docs.rs`, `crates/web/src/api/openapi.rs`, console index; document JSON auth errors and regenerate `docs/openapi.json` (FR-001, FR-002, FR-003, FR-CON-01, AC-ADM-001, AC-ADM-003, AC-ADM-043).
- [ ] T193 [US1] Write failing test: [http] `no_js_totp_and_no_htmx_access_have_equivalent_security` in `crates/web/tests/auth_progressive.rs`; expect missing full-page fallback, hidden primary/factor bypass or inaccessible passkey-disabled explanation (FR-002, FR-029, FR-031, FR-UX-08, AC-ADM-002, AC-ADM-030, AC-ADM-032).
- [ ] T194 [US1] Make pass: T193 complete full-page no-JS TOTP/enrollment navigation and accessible fallback messages in `crates/web/src/console/auth.rs`, `crates/web/src/console/enrollment.rs`, access templates and both console Fluent files; no automatic fallback enrollment (FR-002, FR-029, FR-031, FR-UX-08, AC-ADM-002, AC-ADM-030, AC-ADM-032).
- [ ] T195 [US1] Write failing test: [integration] `access_composition_selects_all_ports_from_one_backend_group` in `crates/app/tests/integration_access_composition.rs`; expect missing real/fake configuration wiring, incomplete debug fake catalog or production startup accepting a fake (FR-001, FR-002, FR-004, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-043).
- [ ] T196 [US1] Make pass: T195 implement `crates/app/src/composition.rs`, `crates/app/src/serve.rs`, `crates/app/src/main.rs`, app dependencies; real adapters only in normal production path, debug-only testkit selection, same shared UoW/store, no database connection in fake mode (FR-001, FR-002, FR-004, FR-CON-01, AC-ADM-001, AC-ADM-002, AC-ADM-043).
- [ ] T197 [US1] Write failing test: [integration] `initial_provisioning_is_operator_authorized_not_public_registration` in `crates/app/tests/integration_provision_admin.rs`; expect missing controlled first-admin grant, unsafe secrets in CLI/logs or second-factor bypass (FR-004, FR-CON-03, AC-ADM-004, AC-ADM-006).
- [ ] T198 [US1] Make pass: T197 implement operator provisioning command in `crates/app/src/provision_admin.rs`, `crates/app/src/main.rs`; protected input/config and audited UoW create restricted eligibility/enrollment only, refuse last-admin self-recovery (FR-004, FR-CON-03, AC-ADM-004, AC-ADM-006).
- [ ] T199 [US1] Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Confirm AC-ADM-001–007/043 on fakes AND real adapters, record only observed results in `specs/001-admin-foundation/tasks.md` (FR-CON-01, FR-CON-02, FR-CON-03, FR-CON-37).

## Phase 4: User Story 2 — Inspect and Safely Prepare Settings (P1)

**Goal**: Ten bounded settings, exact draft/preview, honest impact/preservation rules, no
live mutation before review. Flags share subject storage here; enablement remains US6.
**Independent test**: AC-ADM-008–015/037–042/046, including OI8, against fake and real storage.

### Pure setting, flag and review domain

- [ ] T200 [US2] Write failing test: [unit] `initial_settings_have_decided_values_and_complete_metadata`, `settings_accept_inclusive_bounds_without_changing_defaults`, `setting_rejects_values_outside_declared_policy` in `crates/admin/src/domain/settings.rs`; proptest all nine default/min/max triples: buyer_temp_photo_days 14/1/30, studio_temp_upload_days 30/7/90, studio_auto_draft_days 30/1/90, account_recovery_days 30/14/90, studio_max_cards 50/1/100, buyer_max_permanent_photos 5/1/10, upload_max_mb 15/1/25, upload_min_shortest_edge_px 512/256/1024, generation_max_attempts 3/1/3; expect missing registry, wrong-kind/outside/overflow acceptance or absent metadata (FR-006, FR-007, FR-CON-32, P41, AC-ADM-008, AC-ADM-009, AC-ADM-037).
- [ ] T201 [US2] Make pass: T200 implement Setting<T>, stable registry, units/FA-EN label keys/live-version references and checked inclusive finite numeric policy in `crates/admin/src/domain/settings.rs`, domain index; no float values or default rewrites (FR-006, FR-007, FR-CON-32, P41, AC-ADM-008, AC-ADM-009, AC-ADM-037).
- [ ] T202 [US2] Write failing test: [unit] `upload_format_policy_requires_jpeg_and_known_formats` in `crates/admin/src/domain/formats.rs`; enumerate every subset/unknown format and expect missing JPEG-mandatory sorted set; quote policy "valid masks are 1..15 with bit 1 set" (FR-032, FR-CON-32, AC-ADM-038, Q05).
- [ ] T203 [US2] Make pass: T202 implement FormatSet and tenth setting default JPEG/PNG/WebP/HEIC in `crates/admin/src/domain/formats.rs`, `crates/admin/src/domain/settings.rs`, domain index; only nonempty subsets containing JPEG (FR-032, FR-CON-32, AC-ADM-038, Q05).
- [ ] T204 [US2] Write failing test: [unit] `editing_approved_draft_invalidates_approval`, `draft_hash_is_canonical` in `crates/admin/src/domain/change.rs`; expect absent "exactly one setting key or flag key", immutable revision/head-CAS, sorted-format/UTC hashing and content/time edit invalidation (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T205 [US2] Make pass: T204 implement Flag, ChangeDraft/Revision/Preview/Approval values and canonical digest in `crates/admin/src/domain/flag.rs`, `crates/admin/src/domain/change.rs`, domain index; exact revision/baseline IDs, no batch editor, no approval resurrection on value reversion (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T206 [US2] Write failing test: [unit] `shorter_lifetime_previews_existing_item_impact` in `crates/admin/src/domain/impact_lifetime.rs`; expect missing original upload/last-activity reference, changed/already-due counts, equality-boundary and unknown≠zero semantics; no deletion while previewing (FR-010, FR-CON-32, P41, AC-ADM-012).
- [ ] T207 [US2] Make pass: T206 implement pure lifetime-impact calculation in `crates/admin/src/domain/impact_lifetime.rs`, domain index; typed snapshots/provenance, next-sweep explanation, no downstream expiry executor (FR-010, FR-CON-32, P41, AC-ADM-012).
- [ ] T208 [US2] Write failing test: [unit] `card_limit_preview_preserves_existing_projects` in `crates/admin/src/domain/impact_cards.rs`; expect missing keep-existing/confirmed-unaffected rule, creation permitted at limit but no addition beyond it (FR-033, FR-CON-32, P41, AC-ADM-039, Q15).
- [ ] T209 [US2] Make pass: T208 implement card-preservation impact DTO/policy in `crates/admin/src/domain/impact_cards.rs`, domain index; no Studio project mutation (FR-033, FR-CON-32, P41, AC-ADM-039, Q15).
- [ ] T210 [US2] Write failing test: [unit] `photo_limit_preview_preserves_existing_photos` in `crates/admin/src/domain/impact_photos.rs`; expect deletion or add-permitted at/above limit instead of keep-all/add-only-below (FR-034, FR-CON-32, P41, AC-ADM-040, Q40).
- [ ] T211 [US2] Make pass: T210 implement photo-preservation policy in `crates/admin/src/domain/impact_photos.rs`, domain index; no media or account-lifecycle implementation (FR-034, FR-CON-32, P41, AC-ADM-040, Q40).
- [ ] T212 [US2] Write failing test: [unit] `upload_policy_preview_applies_only_to_new_uploads` in `crates/admin/src/domain/impact_uploads.rs`; expect stored-file revalidation/removal after size/format/shortest-edge changes (FR-035, FR-CON-32, P41, AC-ADM-041, Q05).
- [ ] T213 [US2] Make pass: T212 implement new-upload-only policy in `crates/admin/src/domain/impact_uploads.rs`, domain index; preserve stored files (FR-035, FR-CON-32, P41, AC-ADM-041, Q05).
- [ ] T214 [US2] Write failing test: [unit] `attempt_limit_preview_preserves_existing_units` in `crates/admin/src/domain/impact_attempts.rs`; expect queued/processing allowance rewrite or more than three total attempts (FR-036, FR-CON-32, P41, AC-ADM-042, Q29).
- [ ] T215 [US2] Make pass: T214 implement future-queued-only attempt policy in `crates/admin/src/domain/impact_attempts.rs`, domain index; no generation/job executor (FR-036, FR-CON-32, P41, AC-ADM-042, Q29).
- [ ] T216 [US2] Write failing test: [unit] `recovery_window_decrease_keeps_pending_end_dates` in `crates/admin/src/domain/impact_recovery.rs`; expect a pending account's shown end date D to move after 30→14 while new requests use 14 (FR-010, FR-CON-32, P41, AC-ADM-046, OI8).
- [ ] T217 [US2] Make pass: T216 implement new-deletion-request-only recovery policy in `crates/admin/src/domain/impact_recovery.rs`, domain index; existing D unchanged, no deletion scheduling code (FR-010, FR-CON-32, P41, AC-ADM-046, OI8).
- [ ] T218 [US2] Write failing test: [unit] `settings_cannot_weaken_security_or_financial_integrity` in `crates/admin/src/domain/protected_policy.rs`; expect arbitrary keys/security-off/audit-off/financial-bypass values to be accepted; name invariants GR1–GR9 without claiming their owning features are delivered (FR-011, FR-CON-32, AC-ADM-013, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).
- [ ] T219 [US2] Make pass: T218 implement fixed editable registry/security boundary in `crates/admin/src/domain/protected_policy.rs`, domain index; reject bypass keys regardless of admin/urgency (FR-011, FR-CON-32, AC-ADM-013, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).

### Shared management ports and fakes

- [ ] T220 [US2] Write failing test: [unit] `settings_repository_port_is_mockable` in `crates/admin/src/ports/settings_repository.rs`; expect missing list/live/history/lock/append-version/live-pointer-CAS/seed methods (FR-006, FR-007, FR-CON-32).
- [ ] T221 [US2] Make pass: T220 define SettingsRepository in `crates/admin/src/ports/settings_repository.rs`, port index; explicit TxHandle and immutable definition policy (FR-006, FR-007, FR-CON-32).
- [ ] T222 [US2] Write failing test: [contract] `settings_repository_contract` in `crates/testkit/src/contracts/settings_repository.rs`, runner `crates/testkit/tests/settings_repository_fake.rs`; expect missing ten-key metadata, numeric/format bounds, matching version identity, immutable history and CAS/no-op seed retry (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038).
- [ ] T223 [US2] Make pass: T222 implement `crates/testkit/src/fakes/settings_repository.rs`, shared store/index wiring; reject wrong-store transactions and invalid values without effects (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038).
- [ ] T224 [US2] Write failing test: [unit] `flag_repository_port_is_mockable` in `crates/admin/src/ports/flag_repository.rs`; expect missing six-key list/live/history/lock/append/CAS/seed operations needed by shared subject workflow (FR-017, FR-018, FR-CON-32).
- [ ] T225 [US2] Make pass: T224 define FlagRepository in `crates/admin/src/ports/flag_repository.rs`, port index; no capability-health override operation (FR-017, FR-018, FR-CON-32).
- [ ] T226 [US2] Write failing test: [contract] `flag_repository_contract` in `crates/testkit/src/contracts/flag_repository.rs`, runner `crates/testkit/tests/flag_repository_fake.rs`; expect absent six OFF seed identities, immutable history, matching subject live pointer and stale-CAS refusal (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T227 [US2] Make pass: T226 implement `crates/testkit/src/fakes/flag_repository.rs`, shared store/index wiring; registration does not assert delivery/readiness (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T228 [US2] Write failing test: [unit] `change_repository_port_is_mockable` in `crates/admin/src/ports/change_repository.rs`; expect missing head/revision CAS, immutable append, preview/approval persist/read (FR-008, FR-012, FR-016, FR-CON-05).
- [ ] T229 [US2] Make pass: T228 define ChangeRepository in `crates/admin/src/ports/change_repository.rs`, port index; exact revision/subject/baseline typed identity (FR-008, FR-012, FR-016, FR-CON-05).
- [ ] T230 [US2] Write failing test: [contract] `change_repository_contract` in `crates/testkit/src/contracts/change_repository.rs`, runner `crates/testkit/tests/change_repository_fake.rs`; expect missing stale-head conflict, immutable revision/review, content/time invalidation and same-subject baseline checks (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T231 [US2] Make pass: T230 implement `crates/testkit/src/fakes/change_repository.rs`, shared store/index wiring; old approval records remain immutable but lose eligibility (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T232 [US2] Write failing test: [unit] `impact_assessment_port_is_mockable` in `crates/admin/src/ports/impact_assessment.rs`; expect missing exact revision/baseline/policy/time/source and known/unknown impact output (FR-009, FR-010, FR-CON-32, P41).
- [ ] T233 [US2] Make pass: T232 define ImpactAssessment in `crates/admin/src/ports/impact_assessment.rs`, port index; population snapshot/evidence input is typed, no external query or fake counts in domain (FR-009, FR-010, FR-CON-32, P41).
- [ ] T234 [US2] Write failing test: [contract] `impact_assessment_contract` in `crates/testkit/src/contracts/impact_assessment.rs`, runner `crates/testkit/tests/impact_assessment_fake.rs`; expect missing provenance, known-positive/known-empty/unavailable distinction, original-reference math, preservation and OI8 cases (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-32, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).
- [ ] T235 [US2] Make pass: T234 implement `crates/testkit/src/fakes/impact_assessment.rs`, indexes; fixture-controlled snapshots and failures, never side effects on owning lifecycle (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-32, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).

### Fake-first management services

- [ ] T236 [US2] Write failing test: [mock] `seed_records_defaults_flags_and_audit_once` in `crates/testkit/tests/mock_seed_foundation.rs`; expect missing one-UoW/operation-key/id/time/one-audit ordering and duplicate writes on rerun (FR-006, FR-017, FR-021, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T237 [US2] Write failing test: [fake] `seed_rerun_preserves_published_values` in `crates/testkit/tests/fake_seed_foundation.rs`; expect missing ten exact defaults, six exact OFF flags or reseeding to overwrite a changed value; no privileged admin seed (FR-006, FR-017, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T238 [US2] Make pass: T236 T237 implement SeedFoundation in `crates/admin/src/application/seed_foundation.rs`, application index; keys from data-model §2, baseline versions+system audit atomically, idempotent stable seed operation (FR-006, FR-017, FR-021, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T239 [US2] Write failing test: [mock] `registry_reads_use_ports_without_writes` in `crates/testkit/tests/mock_registry_queries.rs`; expect missing verified-actor→read-UoW→settings/flags/history reads and any mutation on inspection (FR-006, FR-018, FR-CON-32, AC-ADM-008).
- [ ] T240 [US2] Write failing test: [fake] `registry_includes_bilingual_metadata_and_live_history` in `crates/testkit/tests/fake_registry_queries.rs`; expect absent ten-key labels/unit/permitted-policy/effective-time/actor history and six-key flag metadata (FR-006, FR-018, FR-CON-32, AC-ADM-008, AC-ADM-034).
- [ ] T241 [US2] Make pass: T239 T240 implement ListSettings/ListFlags/GetSetting/GetVersion/GetChange queries in `crates/admin/src/application/registry_queries.rs`, application index; typed owned DTOs and bounded reads only (FR-006, FR-018, FR-CON-32, AC-ADM-008, AC-ADM-034).
- [ ] T242 [US2] Write failing test: [mock] `validation_does_not_invoke_publish_or_vendor` in `crates/testkit/tests/mock_save_draft.rs`; expect missing invalid-policy/stale-head checks before revision/audit writes, or ID/time generation bypassing ports (FR-007, FR-008, FR-011, FR-012, FR-016, FR-CON-05, AC-ADM-009, AC-ADM-013, AC-ADM-014, AC-ADM-015).
- [ ] T243 [US2] Write failing test: [fake] `saving_setting_creates_draft_only`, `editing_approved_draft_invalidates_approval`, `concurrent_setting_edits_do_not_silently_overwrite` in `crates/testkit/tests/fake_save_draft.rs`; expect missing draft-only state, CAS conflict or retained approval after content/time edit (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T244 [US2] Make pass: T242 T243 implement SaveDraft in `crates/admin/src/application/save_draft.rs`, application index; single subject, one revision+audit, no live-pointer mutation, preserve original input at presentation boundary (FR-007, FR-008, FR-011, FR-012, FR-016, FR-CON-05, AC-ADM-009, AC-ADM-010, AC-ADM-013, AC-ADM-014, AC-ADM-015).
- [ ] T245 [US2] Write failing test: [mock] `preview_reads_exact_revision_assesses_once_and_never_executes_effects` in `crates/testkit/tests/mock_preview.rs`; expect missing revision/baseline assessment, fresh Clock provenance, one preview/audit write, or lifecycle/vendor/financial mutation while locked (FR-009, FR-010, FR-CON-06, FR-CON-09, P41, AC-ADM-011, AC-ADM-012).
- [ ] T246 [US2] Write failing test: [fake] `preview_uses_exact_draft_without_live_effects` in `crates/testkit/tests/fake_preview.rs`; expect missing immutable report, known/unknown impact, original reference/next-sweep and OI8/preservation explanations, or live mutation (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).
- [ ] T247 [US2] Make pass: T245 T246 implement Preview in `crates/admin/src/application/preview.rs`, application index; gather evidence outside transaction, verify exact baseline/head on persistence, unknown stays unknown and blocks later approval/publication (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).

### Actual registry storage and policy adapter

- [ ] T248 [US2] Write failing test: [integration] `registry_schema_rejects_invalid_values_and_cross_subject_pointers` in `crates/infra-postgres/tests/integration_registry_schema.rs`; expect absent tables/constraints: settings "key PK; fixed initial key/policy checks; non-null live pointer after seed transaction"; versions "exactly one typed value" and "UNIQUE(publication_id, setting_key)"; flags "six fixed keys"; flag versions "UNIQUE(publication_id, flag_key)"; per-key finite CHECK/JPEG mask, deferrable composite FKs and immutable histories (FR-006, FR-007, FR-017, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-033, AC-ADM-037, AC-ADM-038).
- [ ] T249 [US2] Make pass: T248 add `crates/infra-postgres/migrations/0003_registry.sql`; enforce all data-model §3 setting/flag columns, matching-version FKs, constraints/indexes and protected history/runtime policy, no seeded placeholder publication FK (FR-006, FR-007, FR-017, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-033, AC-ADM-037, AC-ADM-038).
- [ ] T250 [US2] Write failing test: [integration] `review_schema_enforces_immutable_exact_version_links` in `crates/infra-postgres/tests/integration_review_schema.rs`; expect absent drafts "CHECK exactly one subject"/"CAS on edit_counter"; revisions "UNIQUE(change_id, revision_number); immutable content"; previews "FK exact revision and matching subject baseline"; approvals "FK exact revision/preview/admin" and same-revision/subject checks (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T251 [US2] Make pass: T250 add `crates/infra-postgres/migrations/0004_change_review.sql`; typed baseline XOR checks, immutable revision/preview/approval guards, subject/status indexes and CAS head, no unchecked polymorphic identity (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T252 [US2] Write failing test: [contract] [integration] `settings_repository_contract_postgres` in `crates/infra-postgres/tests/integration_settings_repository.rs`; run T222 unchanged, expect missing SQL methods and runtime-policy enforcement (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038).
- [ ] T253 [US2] Make pass: T252 implement `crates/infra-postgres/src/settings_repository.rs`, adapter index; matching live pointers, bounded history, caller UoW, immutable policy and insert-if-unseeded semantics (FR-006, FR-007, FR-032, FR-CON-32, AC-ADM-008, AC-ADM-009, AC-ADM-037, AC-ADM-038).
- [ ] T254 [US2] Write failing test: [contract] [integration] `flag_repository_contract_postgres` in `crates/infra-postgres/tests/integration_flag_repository.rs`; run T226 unchanged, expect missing six-key OFF registry/CAS/history (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T255 [US2] Make pass: T254 implement `crates/infra-postgres/src/flag_repository.rs`, adapter index; boolean state and history only, not fabricated delivery (FR-017, FR-018, FR-CON-32, AC-ADM-033, AC-ADM-034).
- [ ] T256 [US2] Write failing test: [contract] [integration] `change_repository_contract_postgres` in `crates/infra-postgres/tests/integration_change_repository.rs`; run T230 unchanged, race stale saves and expect missing exact-baseline/CAS protection (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T257 [US2] Make pass: T256 implement `crates/infra-postgres/src/change_repository.rs`, adapter index; immutable append and conditional head update in caller transaction (FR-008, FR-012, FR-016, FR-CON-05, AC-ADM-010, AC-ADM-014, AC-ADM-015).
- [ ] T258 [US2] Write failing test: [contract] [integration] `impact_assessment_contract_real_policy` in `crates/infra-policy/tests/impact_assessment.rs`; run T234 unchanged with typed snapshot inputs; expect missing genuine preservation calculations and honest unknown when no authoritative population provider exists (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-32, P41, AC-ADM-012, AC-ADM-046).
- [ ] T259 [US2] Make pass: T258 add `crates/infra-policy/Cargo.toml`, `crates/infra-policy/src/lib.rs`, `crates/infra-policy/src/impact_assessment.rs`, workspace entry; use pure domain calculations on supplied evidence, never production fixture counts or adapter→adapter population calls (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-32, P41, AC-ADM-012, AC-ADM-046).
- [ ] T260 [US2] Write failing test: [contract] [integration] `health_probe_contract_postgres` in `crates/infra-postgres/tests/integration_health_probe.rs`; run T072 unchanged against actual missing/unseeded/seeded registries and unavailable connection; expect absent real probe (FR-001, FR-006, FR-017, FR-CON-01).
- [ ] T261 [US2] Make pass: T260 implement `crates/infra-postgres/src/health_probe.rs`, adapter index; ready only when the genuine migrated seed is available, otherwise typed unavailable without leaking infrastructure (FR-001, FR-006, FR-017, FR-CON-01).
- [ ] T262 [US2] Write failing test: [integration] `registry_seed_and_draft_commit_with_audit_or_not_at_all` in `crates/infra-postgres/tests/integration_settings_services.rs`; expect missing seed rerun protection, one-winner stale save and audit-failure rollback using real services/SQL (FR-006, FR-008, FR-016, FR-024, FR-CON-32, AC-ADM-008, AC-ADM-010, AC-ADM-015, AC-ADM-025).
- [ ] T263 [US2] Make pass: T262 add test harness `crates/infra-postgres/tests/support/settings.rs`; correct atomicity only where driven in `crates/admin/src/application/seed_foundation.rs`, `crates/admin/src/application/save_draft.rs`, no test-owned production schema or bypass pools (FR-006, FR-008, FR-016, FR-024, FR-CON-32, AC-ADM-008, AC-ADM-010, AC-ADM-015, AC-ADM-025).

### Settings/preview HTML and composition

- [ ] T264 [US2] Write failing test: [http] `settings_forms_preserve_input_and_report_bounds_and_conflicts` in `crates/web/tests/settings.rs`; expect absent ten-key list/detail/draft form routes, localized 422/error labels, 409 stale conflict, metadata/history and draft-only 303 flow (FR-006, FR-007, FR-008, FR-016, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-009, AC-ADM-010, AC-ADM-015, AC-ADM-037, AC-ADM-038).
- [ ] T265 [US2] Make pass: T264 implement `crates/web/src/console/settings.rs`, console index, `crates/web/templates/console/settings.html`, `crates/web/templates/console/setting.html`, `crates/web/templates/partials/setting_row.html` and setting keys in both FA/EN console Fluent files; no arbitrary registry policy editing (FR-006, FR-007, FR-008, FR-016, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-009, AC-ADM-010, AC-ADM-015, AC-ADM-037, AC-ADM-038).
- [ ] T266 [US2] Write failing test: [http] `preview_views_explain_every_preservation_policy_and_unknown_count` in `crates/web/tests/preview.rs`; expect absent exact draft/live display, viewport preview selector, positive/unknown/next-sweep and OI8 explanations in FA/EN (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).
- [ ] T267 [US2] Make pass: T266 implement `crates/web/src/console/changes.rs`, console index, `crates/web/templates/console/change.html`, `crates/web/templates/partials/impact.html` and both console Fluent files; no deletion/financial side effects, preserve revision and safe valid input (FR-009, FR-010, FR-033, FR-034, FR-035, FR-036, FR-CON-06, P41, AC-ADM-011, AC-ADM-012, AC-ADM-039, AC-ADM-040, AC-ADM-041, AC-ADM-042, AC-ADM-046).
- [ ] T268 [US2] Write failing test: [integration] `foundation_seed_command_updates_health_and_is_idempotent` in `crates/app/tests/integration_seed_foundation.rs`; expect absent migrate/seed commands, wrong migration/runtime role, seed rerun overwrite or fake mode contacting storage (FR-006, FR-017, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T269 [US2] Make pass: T268 implement `crates/app/src/migrate.rs`, `crates/app/src/seed_foundation.rs`, `crates/app/src/main.rs`, `crates/app/src/composition.rs`; wire registry/impact/health adapters by config, run seed through application UoW, keep deployment secrets out of arguments/logs (FR-006, FR-017, FR-CON-32, AC-ADM-008, AC-ADM-033).
- [ ] T270 [US2] Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Confirm AC-ADM-008–015/037–042/046 on fakes AND real adapters in `specs/001-admin-foundation/tasks.md`; lifetime real counts may honestly remain unavailable until owning contexts exist (FR-CON-32, FR-CON-37, P41).

## Phase 5: User Story 3 — Publish, Schedule and Roll Back Changes (P1)

**Goal**: Exact reviewed revision becomes live once at its authorised time; durable blocked
state, ABA-safe baselines, overdue rechecks and history-preserving rollback.
**Independent test**: AC-ADM-016–020/044–045, fake Clock first, then real concurrent SQL/worker.

### Publication domain and new ports

- [ ] T271 [US3] Write failing test: [unit] `publication_requires_exact_review_and_authorised_time` in `crates/admin/src/domain/publication.rs`; proptest invalidation, same-subject ABA versus unrelated change, urgency, due-time and overdue-actual-time transitions; expect missing pure rules (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045).
- [ ] T272 [US3] Make pass: T271 implement PublicationRequest/Result, immutable version identity and explicit blocked reasons in `crates/admin/src/domain/publication.rs`, `crates/admin/src/domain/mod.rs`; urgent requests never bypass approval, no in-process schedule timer (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045).
- [ ] T273 [US3] Write failing test: [unit] `publication_repository_port_is_mockable` in `crates/admin/src/ports/publication_repository.rs`; expect missing idempotent create/payload-conflict/claim/lock/fenced complete/block/resolve-outcome methods (FR-013, FR-037, FR-CON-05).
- [ ] T274 [US3] Make pass: T273 define PublicationRepository in `crates/admin/src/ports/publication_repository.rs`, `crates/admin/src/ports/mod.rs`; owned lease/fence/result DTOs and explicit TxHandle (FR-013, FR-037, FR-CON-05).
- [ ] T275 [US3] Write failing test: [contract] `publication_repository_contract` in `crates/testkit/src/contracts/publication_repository.rs`, runner `crates/testkit/tests/publication_repository_fake.rs`; expect missing payload-sensitive retry identity, due ordering, lease expiration, stale-fence refusal and one committed result (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045).
- [ ] T276 [US3] Make pass: T275 implement `crates/testkit/src/fakes/publication_repository.rs`, extend `crates/testkit/src/fakes/store.rs`, register `crates/testkit/src/fakes/mod.rs` and `crates/testkit/src/contracts/mod.rs`; no successful effect from an expired lease (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045).
- [ ] T277 [US3] Write failing test: [unit] `capability_readiness_port_is_mockable` in `crates/admin/src/ports/capability_readiness.rs`; expect missing delivered/pass/fail/unknown evidence with revision/baseline/source/policy/time (FR-019, FR-020, FR-CON-32).
- [ ] T278 [US3] Make pass: T277 define CapabilityReadiness in `crates/admin/src/ports/capability_readiness.rs`, `crates/admin/src/ports/mod.rs`; no writable health override or inference that registration equals delivery (FR-019, FR-020, FR-CON-32).
- [ ] T279 [US3] Write failing test: [contract] `capability_readiness_contract` in `crates/testkit/src/contracts/capability_readiness.rs`, runner `crates/testkit/tests/capability_readiness_fake.rs`; expect absent delivered+pass/failed/unknown/not-delivered and provenance semantics (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036).
- [ ] T280 [US3] Make pass: T279 implement fixture-controlled `crates/testkit/src/fakes/capability_readiness.rs`, `crates/testkit/src/fakes/mod.rs`, `crates/testkit/src/contracts/mod.rs`; no implicit healthy default (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036).

### Review/publication services, entirely fake-first

- [ ] T281 [US3] Write failing test: [mock] `invalid_approval_writes_nothing` in `crates/testkit/tests/mock_approve.rs`; expect missing head/preview/baseline/provenance/known-impact checks before exactly one approval+audit, no writes on failure (FR-012, FR-015, FR-CON-07, AC-ADM-014, AC-ADM-020, AC-ADM-044).
- [ ] T282 [US3] Write failing test: [fake] `approval_binds_current_preview_revision_and_baseline` in `crates/testkit/tests/fake_approve.rs`; expect missing self-approval-as-distinct-step support, stale/edited/unknown-impact refusal and preserved current live state (FR-012, FR-015, FR-CON-07, AC-ADM-014, AC-ADM-020, AC-ADM-044).
- [ ] T283 [US3] Make pass: T281 T282 implement Approve in `crates/admin/src/application/approve.rs`, `crates/admin/src/application/mod.rs`; no new two-person rule for ordinary changes (FR-012, FR-015, FR-CON-07, AC-ADM-014, AC-ADM-020, AC-ADM-044).
- [ ] T284 [US3] Write failing test: [mock] `publish_rechecks_then_commits_version_pointer_result_and_one_audit` in `crates/testkit/tests/mock_publish.rs`; expect missing short-transaction boundaries, draft→request→subject lock order, fresh impact/readiness, exactly-once writes and rollback on failed audit (FR-013, FR-015, FR-024, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-020, AC-ADM-025).
- [ ] T285 [US3] Write failing test: [fake] `approved_change_publishes_only_at_authorised_time`, `repeated_publication_has_one_effect`, `urgent_foundation_change_still_requires_review` in `crates/testkit/tests/fake_publish.rs`; expect missing one-effect identity, preserved baseline gates and no premature/unaudited live change (FR-013, FR-015, FR-024, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-020, AC-ADM-025).
- [ ] T286 [US3] Make pass: T284 T285 implement Publish finalisation in `crates/admin/src/application/publish.rs`, `crates/admin/src/application/mod.rs`; reads/assessments outside locks, final CAS/epoch/fence checks and atomic version/pointer/completion/audit (FR-013, FR-015, FR-024, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-020, AC-ADM-025).
- [ ] T287 [US3] Write failing test: [mock] `schedule_persists_approved_request_once_without_live_writes` in `crates/testkit/tests/mock_schedule_publish.rs`; expect missing exact-review/due-time/Clock checks and one request/audit, or an in-memory timer/live update (FR-013, FR-CON-05, AC-ADM-016, AC-ADM-017).
- [ ] T288 [US3] Write failing test: [fake] `scheduled_publish_rejects_invalidated_approval`, `scheduled_change_blocks_when_live_baseline_changes` in `crates/testkit/tests/fake_schedule_publish.rs`; expect missing durable scheduled state, ABA-version conflict and unrelated-subject independence (FR-012, FR-013, FR-CON-07, AC-ADM-017, AC-ADM-044).
- [ ] T289 [US3] Make pass: T287 T288 implement SchedulePublish/GetPublication in `crates/admin/src/application/schedule_publish.rs`, `crates/admin/src/application/mod.rs`; persist original zone/authorised time and visible blocking reasons (FR-012, FR-013, FR-CON-07, AC-ADM-016, AC-ADM-017, AC-ADM-044).
- [ ] T290 [US3] Write failing test: [mock] `overdue_runner_claims_bounded_work_rechecks_and_audits_once` in `crates/testkit/tests/mock_scheduled_publish.rs`; expect missing Clock/claim/reassess/finalise order, external evidence gathered under locks or duplicate success audit (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045).
- [ ] T291 [US3] Write failing test: [fake] `overdue_publication_rechecks_prerequisites_and_runs_once` in `crates/testkit/tests/fake_scheduled_publish.rs`; advance FixedClock across outage, expect absent exact-version/baseline/impact/readiness rechecks, persisted block reason, actual-time effect or stale-fence refusal (FR-013, FR-037, FR-CON-05, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045).
- [ ] T292 [US3] Make pass: T290 T291 implement ScheduledPublish in `crates/admin/src/application/scheduled_publish.rs`, `crates/admin/src/application/mod.rs`; one recovery effect, no fresh confirmation if unchanged valid approval, never backdate (FR-013, FR-037, FR-CON-05, AC-ADM-017, AC-ADM-018, AC-ADM-044, AC-ADM-045).
- [ ] T293 [US3] Write failing test: [mock] `rollback_appends_a_draft_and_audit_without_rewriting_history` in `crates/testkit/tests/mock_rollback.rs`; expect missing historic read→new revision→one audit and direct live/history overwrite (FR-014, FR-CON-08, AC-ADM-019).
- [ ] T294 [US3] Write failing test: [fake] `rollback_creates_new_version_and_preserves_history` in `crates/testkit/tests/fake_rollback.rs`; expect missing new rollback draft followed by ordinary preview/approval/publish, original actor/history retention and refusal to undo external executed effects (FR-014, FR-CON-08, AC-ADM-019).
- [ ] T295 [US3] Make pass: T293 T294 implement Rollback in `crates/admin/src/application/rollback.rs`, `crates/admin/src/application/mod.rs`; reuse reviewed workflow, no history deletion (FR-014, FR-CON-08, AC-ADM-019).

### Real publication storage, registry readiness and concurrency

- [ ] T296 [US3] Write failing test: [integration] `publication_schema_enforces_unique_approval_and_fenced_state` in `crates/infra-postgres/tests/integration_publication_schema.rs`; expect missing "UNIQUE(approval_id)", payload identity conflict detection, "partial due index on scheduled_at for pending work" and "state/lease checks" plus completed subject/version links (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045).
- [ ] T297 [US3] Make pass: T296 add `crates/infra-postgres/migrations/0005_publications.sql`; durable requests, scheduled/actual times, completion references and due/lease indexes, no destructive history rewrites (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045).
- [ ] T298 [US3] Write failing test: [contract] [integration] `publication_repository_contract_postgres` in `crates/infra-postgres/tests/integration_publication_repository.rs`; run T275 unchanged with concurrent claims, payload conflict, lease expiry and committed-result lookup; expect absent real methods (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-044, AC-ADM-045).
- [ ] T299 [US3] Make pass: T298 implement `crates/infra-postgres/src/publication_repository.rs`, `crates/infra-postgres/src/lib.rs`; SKIP LOCKED claims plus fenced finalisation, one result across independent connections (FR-013, FR-037, FR-CON-05, AC-ADM-016, AC-ADM-018, AC-ADM-044, AC-ADM-045).
- [ ] T300 [US3] Write failing test: [contract] [integration] `capability_readiness_contract_real_registry` in `crates/infra-policy/tests/capability_readiness.rs`; run T279 unchanged, expect absent genuine delivered-capability registry and all six absent capabilities mislabeled healthy (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036).
- [ ] T301 [US3] Make pass: T300 implement `crates/infra-policy/src/capability_readiness.rs`, `crates/infra-policy/src/lib.rs`; production absent registrations are not-delivered, future registered typed evidence may pass/fail/unknown, never a fake adapter or mutable admin override (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036).
- [ ] T302 [US3] Write failing test: [integration] `worker_and_http_publish_commit_one_effect`, `aba_live_version_invalidates_approval`, `unrelated_subject_change_preserves_approval` in `crates/infra-postgres/tests/integration_publication_races.rs`; expect duplicate success, value-only baseline comparison or lock-order failure (FR-012, FR-013, FR-CON-07, AC-ADM-018, AC-ADM-044).
- [ ] T303 [US3] Make pass: T302 implement conflict/fence corrections in `crates/admin/src/application/publish.rs`, `crates/infra-postgres/src/publication_repository.rs`; extend real-service harness in `crates/infra-postgres/tests/support/publication.rs` (FR-012, FR-013, FR-CON-07, AC-ADM-018, AC-ADM-044).
- [ ] T304 [US3] Write failing test: [integration] `expired_lease_rejects_stale_worker`, `unknown_prerequisite_persists_blocked_reason`, `overdue_effect_is_not_backdated` in `crates/infra-postgres/tests/integration_scheduled_publish.rs`; restart runner around due time and expect stale-worker commit or backdated/missing blocking state (FR-013, FR-037, FR-CON-05, AC-ADM-017, AC-ADM-044, AC-ADM-045).
- [ ] T305 [US3] Make pass: T304 implement recovery/fence/block-reason corrections in `crates/admin/src/application/scheduled_publish.rs`, `crates/infra-postgres/src/publication_repository.rs`; reuse real clock-controlled harness (FR-013, FR-037, FR-CON-05, AC-ADM-017, AC-ADM-044, AC-ADM-045).
- [ ] T306 [US3] Write failing test: [integration] `crash_before_commit_leaves_no_live_change`, `lost_commit_ack_resolves_by_publication_id`, `publication_and_audit_commit_together` in `crates/infra-postgres/tests/integration_publication_failures.rs`; expect unaudited live effect, claimed rollback after unknown commit or duplicate retry (FR-013, FR-024, FR-037, FR-CON-37, AC-ADM-018, AC-ADM-025, AC-ADM-045).
- [ ] T307 [US3] Make pass: T306 implement unknown-outcome lookup/retry and failure-atomic completion in `crates/admin/src/application/publish.rs`, `crates/infra-postgres/src/unit_of_work.rs`; fault injection remains test-owned, never a production failure switch (FR-013, FR-024, FR-037, FR-CON-37, AC-ADM-018, AC-ADM-025, AC-ADM-045).

### Review/publication HTML and durable worker

- [ ] T308 [US3] Write failing test: [http] `review_publish_forms_bind_exact_revision_and_show_zone` in `crates/web/tests/publication_forms.rs`; expect missing approve/publish/schedule routes, fresh preview after edits, timezone ambiguity, missing urgent confirmation or omitted block reason (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-014, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045).
- [ ] T309 [US3] Make pass: T308 extend `crates/web/src/console/changes.rs`, `crates/web/templates/console/change.html`, add `crates/web/templates/partials/publication_status.html` and both console Fluent files; secure POST/303, no full-page/htmx approval bypass (FR-012, FR-013, FR-015, FR-037, FR-CON-07, AC-ADM-014, AC-ADM-016, AC-ADM-017, AC-ADM-020, AC-ADM-044, AC-ADM-045).
- [ ] T310 [US3] Write failing test: [http] `rollback_and_publication_detail_keep_history_attributed` in `crates/web/tests/rollback.rs`; expect missing publication/version detail and rollback form, missing scheduled/actual times or old-version overwrite (FR-013, FR-014, FR-021, FR-CON-08, AC-ADM-019, AC-ADM-021, AC-ADM-045).
- [ ] T311 [US3] Make pass: T310 implement `crates/web/src/console/publications.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/publication.html`, `crates/web/templates/console/version.html`, both console Fluent files; rollback enters a new draft, not direct publish (FR-013, FR-014, FR-021, FR-CON-08, AC-ADM-019, AC-ADM-021, AC-ADM-045).
- [ ] T312 [US3] Write failing test: [fake] [integration] `worker_uses_services_and_resumes_due_work_once` in `crates/worker/tests/scheduled_loop.rs`, `crates/app/tests/integration_worker.rs`; expect missing bounded service loop, shutdown/restart recovery and configuration wiring; no repository access from worker (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045).
- [ ] T313 [US3] Make pass: T312 add `crates/worker/Cargo.toml`, `crates/worker/src/lib.rs`, `crates/worker/src/scheduled.rs`, workspace entry, `crates/app/src/worker.rs`, `crates/app/src/main.rs`, `crates/app/src/composition.rs`; app wires clock/readiness/publication/UoW, worker invokes application service only (FR-013, FR-037, FR-CON-05, AC-ADM-018, AC-ADM-045).
- [ ] T314 [US3] Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Validate AC-ADM-016–020/044–045 on fakes AND real adapters in `specs/001-admin-foundation/tasks.md` (FR-CON-05, FR-CON-07, FR-CON-08, FR-CON-37).

## Phase 6: User Story 4 — Investigate an Attributable, Protected History (P1)

**Goal**: Complete searchable actor-specific evidence, immutable even for full-permission admins.
Audit storage/domain came first in US1 because access/publication must already be audited.
**Independent test**: AC-ADM-021–025 on fakes and runtime-role SQL; rejected UPDATE/DELETE/TRUNCATE
are successful security-test assertions, never intentionally red tests left unfinished.

- [ ] T315 [US4] Write failing test: [unit] `audit_filters_reject_unbounded_or_invalid_cursors` in `crates/admin/src/domain/audit_query.rs`; expect missing typed actor/action/subject/time filters, bounded limit and stable `(occurred_at,id)` cursor validation (FR-022, FR-CON-37, AC-ADM-022).
- [ ] T316 [US4] Make pass: T315 implement AuditQuery/KeysetCursor in `crates/admin/src/domain/audit_query.rs`, `crates/admin/src/domain/mod.rs`; no new arbitrary throughput SLA or editable audit record (FR-022, FR-CON-37, AC-ADM-022).
- [ ] T317 [US4] Write failing test: [mock] `audit_search_authorizes_before_read_and_never_writes` in `crates/testkit/tests/mock_search_audit.rs`; expect query before current-admin verification, unbounded reads or any append/update/delete during search (FR-001, FR-022, FR-CON-37, AC-ADM-022).
- [ ] T318 [US4] Write failing test: [fake] `admin_can_search_foundation_audit` in `crates/testkit/tests/fake_search_audit.rs`; expect missing actor/action/subject/period filters and deterministic no-gap/no-duplicate pagination across same-time events (FR-022, FR-CON-37, AC-ADM-022).
- [ ] T319 [US4] Make pass: T317 T318 implement SearchAudit in `crates/admin/src/application/search_audit.rs`, `crates/admin/src/application/mod.rs`; read-UoW and owned sanitized DTOs, no repository exposed to web (FR-001, FR-022, FR-CON-37, AC-ADM-022).
- [ ] T320 [US4] Write failing test: [integration] `foundation_actions_have_complete_attributed_audit` in `crates/infra-postgres/tests/integration_audit_actions.rs`; inspect sign-in/factor, invitation, recovery, draft, preview, approval, schedule, publication, rollback and failed-audit rollback; expect missing actor/time/reason/before-after or duplicate events (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T321 [US4] Make pass: T320 complete typed event construction in `crates/admin/src/application/audit_event.rs`, `crates/admin/src/application/mod.rs` and call sites `request_admin_otp.rs`, `verify_admin_otp.rs`, `passkey.rs`, `totp.rs`, `enrollment.rs`, `invite_admin.rs`, `recover_admin.rs`, `save_draft.rs`, `preview.rs`, `approve.rs`, `schedule_publish.rs`, `publish.rs`, `rollback.rs` under `crates/admin/src/application/`; change only the event field omissions proven by T320, no new business actions (FR-021, FR-024, FR-CON-37, AC-ADM-021, AC-ADM-024, AC-ADM-025).
- [ ] T322 [US4] Write failing test: [http] `audit_entries_cannot_be_edited_or_deleted` and `audit_search_requires_current_admin` in `crates/web/tests/audit.rs`; expect absent searchable view, missing safe filters/next cursor or exposed mutation/non-admin route (FR-001, FR-022, FR-023, FR-CON-37, AC-ADM-022, AC-ADM-023).
- [ ] T323 [US4] Make pass: T322 implement `crates/web/src/console/audit.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/audit.html`, `crates/web/templates/partials/audit_table.html`, both console Fluent files; no edit/delete endpoint, no private-data details (FR-001, FR-022, FR-023, FR-CON-37, AC-ADM-022, AC-ADM-023).
- [ ] T324 [US4] Write failing test: [integration] `diagnostics_redact_synthetic_auth_and_payment_canaries` in `crates/app/tests/integration_redaction.rs`; force provider/SQL/validation failures and expect raw phone/code/key/photo/payment canaries in captured logs/HTML/audit or missing recoverable failure (FR-024, FR-CON-37, AC-ADM-024, AC-ADM-025).
- [ ] T325 [US4] Make pass: T324 implement allowlisted tracing/error mapping in `crates/app/src/telemetry.rs`, `crates/app/src/main.rs`, `crates/web/src/api/problem.rs`; never log request bodies, cookies, vendor URLs or secret-bearing Debug values (FR-024, FR-CON-37, AC-ADM-024, AC-ADM-025).
- [ ] T326 [US4] Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Confirm AC-ADM-021–025 on fakes AND real adapters and record restricted-role/trigger results in `specs/001-admin-foundation/tasks.md` (FR-CON-01, FR-CON-37).

## Phase 7: User Story 5 — Work in Either Language and Any Theme (P1)

**Goal**: Complete every delivered foundation flow in FA/EN, light/dark/system, assistive input
and full-page navigation without losing safe unsaved work. Extend the existing foundation shell.
**Independent test**: AC-ADM-026–032/047 across instantiated templates and browser samples;
real rendered contrast supplements token calculations, never replaces them.

- [ ] T327 [US5] Write failing test: [unit] `preference_changes_preserve_safe_form_state_only` in `crates/web/tests/preferences_state.rs`; expect absent valid form/revision/selection preservation or reflected OTP/shared-secret/assertion (FR-026, FR-030, FR-UX-03, AC-ADM-027).
- [ ] T328 [US5] Make pass: T327 implement presentation-only carried form-state DTO in `crates/web/src/preferences.rs`, `crates/web/src/lib.rs`; state must not edit/save/publish a business draft (FR-026, FR-030, FR-UX-03, AC-ADM-027).
- [ ] T329 [US5] Write failing test: [http] `language_and_theme_switch_preserve_unsaved_input` in `crates/web/tests/preferences.rs`; expect missing CSRF-protected POST preference flow, discarded invalid-form safe fields/draft selection or changed live data (FR-026, FR-UX-03, AC-ADM-027).
- [ ] T330 [US5] Make pass: T329 implement preference route and shell controls in `crates/web/src/console/preferences.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/partials/preferences.html`, `crates/web/templates/console/layout.html`, both console Fluent files; system follows device preference and manual choice survives navigation (FR-025, FR-026, FR-UX-02, FR-UX-03, AC-ADM-026, AC-ADM-027).
- [ ] T331 [US5] Write failing test: [http] `foundation_screens_support_both_languages_and_all_themes`, `persian_direction_and_digits_preserve_identifiers` in `crates/web/tests/locale_theme_matrix.rs`; instantiate every delivered sign-in/enrollment/admin/settings/change/publication/version/audit view at six locale/theme combinations; expect missing keys/RTL/focus order, mirrored logos or localized IDs/URLs; US6 later extends this same inventory with its flag view (FR-025, FR-027, FR-030, FR-UX-01, FR-UX-04, FR-UX-05, AC-ADM-026, AC-ADM-028, AC-ADM-031).
- [ ] T332 [US5] Make pass: T331 implement digit/bidi/display helpers in `crates/web/src/presentation.rs`, `crates/web/src/lib.rs`, `crates/web/static/console.css`, relevant templates listed in T188/T190/T265/T267/T311/T323 and both console Fluent files; feature-flag view joins this same inventory in US6 (FR-025, FR-027, FR-030, FR-UX-01, FR-UX-04, FR-UX-05, AC-ADM-026, AC-ADM-028, AC-ADM-031).
- [ ] T333 [US5] Write failing test: [http] `foundation_tasks_are_keyboard_and_assistive_accessible` in `crates/web/tests/accessibility.rs`; instantiated views must have one h1, labels/alt, field-linked errors, named controls, focus/dialog return, no traps and 44×44 targets; expect absent semantic support (FR-029, FR-UX-08, FR-UX-09, AC-ADM-030).
- [ ] T334 [US5] Make pass: T333 implement reusable semantic error/dialog/navigation primitives in `crates/web/templates/partials/field.html`, `crates/web/templates/partials/dialog.html`, `crates/web/templates/console/layout.html`, `crates/web/static/console.css`; generated-token focus/boundary contrast, reduced-motion/opaque fallback and pause/stop any nonessential auto-motion >5s (FR-028, FR-029, FR-UX-07, FR-UX-08, FR-UX-09, AC-ADM-029, AC-ADM-030).
- [ ] T335 [US5] Write failing test: [http] `foundation_tasks_work_without_optional_enhancements` in `crates/web/tests/progressive_management.rs`; run draft/preview/approve/publish/rollback/recovery without HX-Request or optional scripts; expect missing full-page forms, validation preservation or security parity (FR-031, FR-UX-08, AC-ADM-032).
- [ ] T336 [US5] Make pass: T335 complete full-page/partial negotiation in `crates/web/src/console/changes.rs`, `crates/web/src/console/publications.rs`, `crates/web/src/console/settings.rs`, `crates/web/templates/console/change.html`, `crates/web/templates/console/setting.html`; Vary HX-Request where appropriate, no different approval rules (FR-031, FR-UX-08, AC-ADM-032).
- [ ] T337 [US5] Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Confirm AC-ADM-026–032/047 with fake and real composition in `specs/001-admin-foundation/tasks.md`; all delivered screens use the same matrix (FR-CON-11, FR-UX-01, FR-UX-07, FR-UX-08, FR-UX-20).

## Phase 8: User Story 6 — Inspect Disabled Capabilities Before Enablement (P2)

**Goal**: Inspect exactly six OFF flags, honest readiness, and shared reviewed history without
delivering the optional capabilities. Storage and readiness ports were introduced by dependencies.
**Independent test**: AC-ADM-033–036, including failed/unknown/not-delivered enablement and
fixture-delivered/pass through the same workflow; production flags remain OFF/not delivered.

- [ ] T338 [US6] Write failing test: [unit] `six_launch_flags_start_off` in `crates/admin/src/domain/flag.rs`; expect wrong/missing/extra keys or an enabled seed among partner_widget_api, percentage_rollout, per_user_usage_limits, ticketing, external_notifications, paid_share_billing (FR-017, FR-CON-32, AC-ADM-033).
- [ ] T339 [US6] Make pass: T338 implement canonical six-key definitions/descriptions/initial OFF membership in `crates/admin/src/domain/flag.rs`; tie SeedFoundation's preexisting definitions to this single registry, not a second set of state constants (FR-017, FR-CON-32, AC-ADM-033).
- [ ] T340 [US6] Write failing test: [unit] `flag_enablement_requires_delivered_passing_evidence` in `crates/admin/src/domain/flag_readiness.rs`; expect fail/unknown/not-delivered/stale evidence accepted, urgent bypass or readiness override in draft input (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036).
- [ ] T341 [US6] Make pass: T340 implement readiness eligibility in `crates/admin/src/domain/flag_readiness.rs`, `crates/admin/src/domain/mod.rs`; readiness is a read-only fact, disabling still follows reviewed workflow (FR-019, FR-020, FR-CON-32, AC-ADM-035, AC-ADM-036).
- [ ] T342 [US6] Write failing test: [mock] `failed_readiness_writes_no_live_flag_or_success_audit` in `crates/testkit/tests/mock_flag_workflow.rs`; expect missing exact-revision readiness recheck, more than one successful audit or any live write on failed/unknown/undelivered checks (FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T343 [US6] Write failing test: [fake] `flag_changes_use_versioned_review_and_history`, `failed_or_unknown_health_blocks_enablement`, `unbuilt_capability_cannot_be_enabled_by_flag` in `crates/testkit/tests/fake_flag_workflow.rs`; expect missing flag-specific draft/preview/approval/publish/rollback, fake delivered+pass success or preserved OFF on blocked enablement (FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T344 [US6] Make pass: T342 T343 implement flag-specific evidence policy in `crates/admin/src/application/preview.rs`, `crates/admin/src/application/approve.rs`, `crates/admin/src/application/publish.rs`; shared workflow remains generic over typed subjects, no unbuilt optional behaviour (FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T345 [US6] Write failing test: [integration] `real_flag_workflow_preserves_off_and_history_when_readiness_blocks` in `crates/infra-postgres/tests/integration_flag_workflow.rs`; use real registry/SQL/services, expect unknown/not-delivered to become ON or history/blocked reason to disappear (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T346 [US6] Make pass: T345 add real-service flag fixtures in `crates/infra-postgres/tests/support/flags.rs`, wire readiness at `crates/app/src/composition.rs`; never inject fake passing evidence into the production registry (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T347 [US6] Write failing test: [http] `flag_register_shows_six_off_states_and_truthful_readiness` in `crates/web/tests/flags.rs`; expect absent list/draft/history forms, managed descriptions, impact/health/result/status and blocked-enable explanation across FA/EN/themes (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T348 [US6] Make pass: T347 implement `crates/web/src/console/flags.rs`, `crates/web/src/console/mod.rs`, `crates/web/templates/console/flags.html`, `crates/web/templates/partials/flag_row.html`, both console Fluent files; add flags to US5 matrix inventory, no optional-capability routes or controls (FR-017, FR-018, FR-019, FR-020, FR-CON-32, AC-ADM-033, AC-ADM-034, AC-ADM-035, AC-ADM-036).
- [ ] T349 [US6] Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Confirm AC-ADM-033–036 on fakes AND real adapters in `specs/001-admin-foundation/tasks.md`; flag registration never counts as P0-OFF capability completion (FR-CON-32, FR-CON-37).

## Phase 9: Polish & Cross-Cutting Gates

**Purpose**: Finish full-console composition and dual-backend E2E/axe, feature evidence and
honest full-product release blockers. Never fabricate wallet/media/generation completion.

- [ ] T350 Write failing test: [integration] `fake_console_serve_needs_no_database_minio_or_vendor` in `crates/app/tests/fake_serve.rs`; run `tankhor serve` with debug dev-fakes, no DATABASE_URL/S3/provider credentials and denied external networking; expect incomplete wiring, a connection attempt or an unusable management flow (FR-001, FR-006, FR-017, FR-024, FR-CON-01, FR-CON-32).
- [ ] T351 Make pass: T350 complete debug-only fake composition/seed/fixture enrollment in `crates/app/src/composition.rs`, `crates/app/src/serve.rs`, `crates/app/Cargo.toml`; normal real startup keeps all required policy/secrets, fake mode requires controlled local fixtures and still exercises primary+second-factor flow (FR-001, FR-006, FR-017, FR-024, FR-CON-01, FR-CON-32).
- [ ] T352 Write failing test: [unit] `every_port_has_fake_real_contract_and_service_mock_evidence` in `scripts/tests/test_port_inventory.py`; remove one runner/coverage column in a disposable inventory and expect the checker to reject it, including both SmsSender real adapters (FR-001, FR-038, FR-CON-37; constitution III).
- [ ] T353 Make pass: T352 extend `scripts/check-test-traceability.py`, `.github/workflows/ci.yml`; verify all nineteen port rows below against actual symbols/tests and both feature graphs, not merely presence of filenames (FR-001, FR-038, FR-CON-37).
- [ ] T354 Write failing test: [integration] `sqlx_offline_metadata_is_current_for_runtime_queries` in `scripts/tests/test_sqlx_metadata.py`; expect absent/stale metadata or prepare using wrong migrated schema/role to pass (FR-024, FR-CON-37).
- [ ] T355 Make pass: T354 generate committed per-query JSON files in `.sqlx/` using SQLx prepare, record their exact generated names in the implementation commit, and wire offline build/prepare-check in `.github/workflows/ci.yml`, `justfile`; never hand-author query hashes (FR-024, FR-CON-37).
- [ ] T356 Write failing test: [e2e] `J5_foundation_safe_change_and_recovery` in `e2e/admin-foundation.spec.ts`; run the same suite for fake and real adapter projects: OTP+passkey/TOTP, safe upload-size draft/live separation, preview/approve/publish/audit/rollback, another-admin recovery/old-cookie denial, unknown lifetime impact and six OFF flags; expect missing executable/browser flow (FR-001, FR-002, FR-004, FR-008, FR-009, FR-013, FR-014, FR-017, FR-024, FR-CON-05, AC-ADM-001, AC-ADM-002, AC-ADM-005, AC-ADM-010, AC-ADM-011, AC-ADM-012, AC-ADM-016, AC-ADM-019, AC-ADM-021, AC-ADM-025, AC-ADM-033, AC-ADM-036, AC-ADM-043).
- [ ] T357 Make pass: T356 create `e2e/package.json`, `e2e/package-lock.json`, `e2e/playwright.config.ts`, `e2e/fixtures.ts`; isolate real PostgreSQL/wiremock/crypto and fake projects, use virtual authenticators/test fixtures without production bypass or vendor traffic, log no synthetic secrets (FR-001, FR-002, FR-004, FR-008, FR-009, FR-013, FR-014, FR-017, FR-024, FR-CON-05, AC-ADM-001, AC-ADM-002, AC-ADM-005, AC-ADM-010, AC-ADM-011, AC-ADM-012, AC-ADM-016, AC-ADM-019, AC-ADM-021, AC-ADM-025, AC-ADM-033, AC-ADM-036, AC-ADM-043).
- [ ] T358 Write failing test: [e2e] `console_locale_theme_keyboard_and_axe_matrix` in `e2e/accessibility.spec.ts`; FA/EN × light/dark/system at 360/768/1024/1440 px, keyboard/focus/errors, zoom/reflow, reduced motion, unsaved-input preferences, no-JS TOTP and no-htmx management; expect known AA failure or critical axe finding (FR-025, FR-026, FR-027, FR-028, FR-029, FR-030, FR-031, FR-038, FR-UX-01, FR-UX-07, FR-UX-08, FR-UX-20, AC-ADM-026, AC-ADM-027, AC-ADM-028, AC-ADM-029, AC-ADM-030, AC-ADM-031, AC-ADM-032, AC-ADM-047).
- [ ] T359 Make pass: T358 add axe and cross-browser/project matrix in `e2e/package.json`, `e2e/package-lock.json`, `e2e/playwright.config.ts`, `e2e/fixtures.ts`; fix only browser-proven focus/contrast/reflow defects in `crates/web/static/console.css`, `crates/web/templates/console/layout.html`, `crates/web/templates/partials/dialog.html`, never weaken AA assertions (FR-025, FR-026, FR-027, FR-028, FR-029, FR-030, FR-031, FR-038, FR-UX-01, FR-UX-07, FR-UX-08, FR-UX-20, AC-ADM-026, AC-ADM-027, AC-ADM-028, AC-ADM-029, AC-ADM-030, AC-ADM-031, AC-ADM-032, AC-ADM-047).
- [ ] T360 Write failing test: [unit] `quality_gates_enforce_decided_coverage_without_invented_thresholds` in `scripts/tests/test_quality_gates.py`; expect missing ≥90% present-kernel/wallet/generation/identity gate, missing nightly wallet/generation mutations, unreviewed survivors or proposed 80/85 promoted to requirements (FR-001, FR-CON-37; constitution I).
- [ ] T361 Make pass: T360 implement `scripts/check-quality-gates.py`, `.github/workflows/ci.yml`, `.github/workflows/nightly.yml`; scoped 001 evidence includes kernel, missing future crates remain explicitly undelivered, mutation survivors recorded/reviewed (FR-001, FR-CON-37).
- [ ] T362 Write failing test: [unit] `full_product_release_blocks_missing_GR1_to_GR9_and_J1_to_J5` in `scripts/tests/test_release_readiness.py`; expect absent owning-feature tests/contexts/legal-accounting/name/asset signoffs to permit full release; guardrail inventory uses exact GR-prefixed names from contracts/test-plan §3 (FR-011, FR-CON-32, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).
- [ ] T363 Make pass: T362 implement `scripts/check-release-readiness.py`, `.github/workflows/release-readiness.yml`; distinguish foundation acceptance from full-product release, block missing evidence rather than add empty or ignored tests, run delivered E2E nightly/pre-release (FR-011, FR-CON-32, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).
- [ ] T364 Write failing test: [unit] `operator_docs_cover_real_fake_seed_security_and_device_acceptance` in `scripts/tests/test_development_docs.py`; expect missing exact commands, protected inputs, fake-production/release exclusions, ten defaults/six OFF review or physical iOS/Android/assistive evidence form (FR-006, FR-017, FR-029, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-030, AC-ADM-033).
- [ ] T365 Make pass: T364 update `docs/development.md`, `specs/001-admin-foundation/quickstart.md`, create `specs/001-admin-foundation/checklists/acceptance.md`; document tested fake/real console commands, seed/security-policy setup, actual-device/assistive/token-freeze evidence requirements; leave evidence unclaimed until observed (FR-006, FR-017, FR-029, FR-CON-32, FR-UX-08, AC-ADM-008, AC-ADM-030, AC-ADM-033).
- [ ] T366 Write failing test: [integration] `deployment_uses_release_binary_and_restricted_runtime_identity` in `scripts/tests/test_deployment.py`; expect missing reproducible image, migration/runtime command distinction or test features/keys in release layers (FR-001, FR-024, FR-CON-01, FR-CON-37).
- [ ] T367 Make pass: T366 create `deploy/Dockerfile`, update `deploy/compose.yml` for separately invoked migration/seed/serve/worker commands and protected runtime configuration; no automatic fake fallback or vendor calls in image tests (FR-001, FR-024, FR-CON-01, FR-CON-37).
- [ ] T368 Verify checkpoint: run `cargo nextest run` (and `--profile integration` where relevant); all green; commit. Run dual-backend E2E/axe, fmt/clippy/deny, architecture/traceability, kernel coverage, SQLx/OpenAPI diffs and record actual evidence in `specs/001-admin-foundation/tasks.md`, `specs/001-admin-foundation/checklists/acceptance.md`; full-product release remains blocked by undelivered owning features (FR-CON-01, FR-CON-32, FR-CON-37, FR-UX-20, GR1, GR2, GR3, GR4, GR5, GR6, GR7, GR8, GR9).

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
- A checkpoint runs only after every preceding pair is green. Checkpoint commits are execution
  instructions, not authorization for this generation command to run builds or commit code.
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
| Composition/seed/scheduled publish/impact | Config in Phase 2; access US1; seed/impact US2; worker US3; full fake serve Phase 9 |
| E2E and gates | Phase 9; per-story fake/real checkpoints remain mandatory |

## Parallel Examples by User Story

**Marked task parallelism**: T026 and T027 can run together after Phase 1; different test
files, no mutual dependency. T028/T029 and all manifest/module registration remain serial.

One implementation task is owned at a time under AGENTS.md. The examples below describe
independent test invocations after the cited implementation dependencies are green, not
permission to work on overlapping production files or run a later red test prematurely.

| Story | Parallel validation example | Required completed work |
| --- | --- | --- |
| US1 | `cargo nextest run -p infra-auth` alongside `cargo nextest run -p infra-sms` | T180, T184; separate local-crypto and wiremock resources |
| US2 | `cargo nextest run -p web -E 'test(settings)'` alongside `cargo nextest run -p infra-policy` | T259, T265; no common mutable fixture |
| US3 | `cargo nextest run -p worker` alongside `cargo nextest run -p web -E 'test(publication)'` | T309, T313; no shared database run |
| US4 | `cargo nextest run -p web -E 'test(audit)'` alongside repository-check Python tests | T323, T325; disjoint read-only validation |
| US5 | `cargo nextest run -p web -E 'test(fluent)'` alongside a token-only unit test selection | T332, T334; no browser/storage fixtures |
| US6 | fake-workflow test binary alongside HTTP flag test binary | T344, T348; separate FakeStore instances |

Never run independent SQLx integration invocations concurrently against the same shared local
server; the live-run lease must reject that. E2E fake/real projects get isolated backends.

## Implementation Strategy

1. Bootstrap tests without pretending the workspace exists; prepare fixtures/tooling only
   after their named failures. Work only on the next unchecked red/green pair.
2. Complete shared foundation, then US1 as the smallest admin-access MVP. Demonstrate secure
   console access/invitation/recovery in both backend modes, not settings that do not yet exist.
3. Deliver US2, then US3, so management becomes useful without weakening approval/audit.
4. Complete investigation, all-surface accessibility and safe flag controls in US4–US6.
5. Validate the complete foundation on fakes and real SQL/crypto/wiremock; report scoped J5,
   not full J5 or J1–J5. No vendor service, source code, deployment or commit is executed by
   this task-generation operation.

## Traceability — AC/FR → Driving Tests → Implementation

The tables are completeness checks. **Missing coverage flags: none** for the 47 local ACs,
38 local FRs or nineteen dependency ports. These links specify future work, not passing tests.
Prerequisite implementations are included where an AC spans domain/service/presentation.

| AC/FR ID | Test task ID(s) | Implementation task ID(s) |
| --- | --- | --- |
| AC-ADM-001 / FR-001 | T150, T151, T187, T191 | T152, T188, T192 |
| AC-ADM-002 / FR-002 | T091, T135, T136, T138, T139, T179, T187 | T092, T137, T140, T180, T188 |
| AC-ADM-003 / FR-003 | T107, T150, T151, T191 | T108, T152, T192 |
| AC-ADM-004 / FR-004 | T141, T142, T144, T145, T189 | T143, T146, T190 |
| AC-ADM-005 / FR-004 | T091, T148, T189 | T092, T149, T190 |
| AC-ADM-006 / FR-004 | T148, T197 | T149, T198 |
| AC-ADM-007 / FR-005 | T091, T145, T189 | T092, T146, T190 |
| AC-ADM-008 / FR-006 | T200, T222, T237, T240, T252, T264 | T201, T223, T238, T241, T253, T265 |
| AC-ADM-009 / FR-007 | T200, T242, T264 | T201, T244, T265 |
| AC-ADM-010 / FR-008 | T204, T243, T264 | T205, T244, T265 |
| AC-ADM-011 / FR-009 | T234, T245, T246, T266 | T235, T247, T267 |
| AC-ADM-012 / FR-010 | T206, T234, T246, T258, T266 | T207, T235, T247, T259, T267 |
| AC-ADM-013 / FR-011 | T218, T242 | T219, T244 |
| AC-ADM-014 / FR-012 | T204, T230, T243, T281, T282 | T205, T231, T244, T283 |
| AC-ADM-015 / FR-016 | T230, T243, T256, T264 | T231, T244, T257, T265 |
| AC-ADM-016 / FR-013 | T271, T285, T287, T298, T308 | T272, T286, T289, T299, T309 |
| AC-ADM-017 / FR-013 | T271, T288, T291, T304 | T272, T289, T292, T305 |
| AC-ADM-018 / FR-013 / FR-037 | T275, T285, T291, T302, T306 | T276, T286, T292, T303, T307 |
| AC-ADM-019 / FR-014 | T293, T294, T310, T356 | T295, T311, T357 |
| AC-ADM-020 / FR-015 | T271, T281, T285, T308 | T272, T283, T286, T309 |
| AC-ADM-021 / FR-021 | T095, T135, T284, T320 | T096, T137, T286, T321 |
| AC-ADM-022 / FR-022 | T095, T315, T317, T318, T322 | T096, T316, T319, T323 |
| AC-ADM-023 / FR-023 | T095, T155, T157, T322 | T096, T156, T158, T323 |
| AC-ADM-024 / FR-024 | T089, T320, T324 | T090, T321, T325 |
| AC-ADM-025 / FR-024 | T135, T147, T151, T185, T306, T320 | T137, T149, T152, T186, T307, T321 |
| AC-ADM-026 / FR-025 | T066, T331, T358 | T067, T332, T359 |
| AC-ADM-027 / FR-026 | T327, T329, T358 | T328, T330, T359 |
| AC-ADM-028 / FR-027 | T027, T331, T358 | T029, T332, T359 |
| AC-ADM-029 / FR-028 | T058, T062, T358 | T059, T063, T359 |
| AC-ADM-030 / FR-029 | T193, T333, T358 | T194, T334, T359 |
| AC-ADM-031 / FR-030 | T064, T331 | T065, T332 |
| AC-ADM-032 / FR-031 | T193, T335, T358 | T194, T336, T359 |
| AC-ADM-033 / FR-017 | T226, T237, T254, T338, T347 | T227, T238, T255, T339, T348 |
| AC-ADM-034 / FR-018 | T226, T240, T343, T347 | T227, T241, T344, T348 |
| AC-ADM-035 / FR-019 | T279, T300, T342, T343, T345, T347 | T280, T301, T344, T346, T348 |
| AC-ADM-036 / FR-020 | T279, T300, T343, T345, T347 | T280, T301, T344, T346, T348 |
| AC-ADM-037 / FR-007 | T200, T222, T248, T252 | T201, T223, T249, T253 |
| AC-ADM-038 / FR-032 | T202, T222, T248, T252, T264 | T203, T223, T249, T253, T265 |
| AC-ADM-039 / FR-033 | T208, T234, T246, T266 | T209, T235, T247, T267 |
| AC-ADM-040 / FR-034 | T210, T234, T246, T266 | T211, T235, T247, T267 |
| AC-ADM-041 / FR-035 | T212, T234, T246, T266 | T213, T235, T247, T267 |
| AC-ADM-042 / FR-036 | T214, T234, T246, T266 | T215, T235, T247, T267 |
| AC-ADM-043 / FR-004 | T103, T107, T111, T147, T148, T185, T191 | T104, T108, T112, T149, T186, T192 |
| AC-ADM-044 / FR-012 / FR-013 | T271, T282, T288, T291, T302 | T272, T283, T289, T292, T303 |
| AC-ADM-045 / FR-037 | T271, T290, T291, T304, T306, T312 | T272, T292, T305, T307, T313 |
| AC-ADM-046 / FR-010 / P41 / OI8 | T216, T234, T246, T266 | T217, T235, T247, T267 |
| AC-ADM-047 / FR-038 / FR-UX-20 | T058, T060, T062, T358 | T059, T061, T063, T359 |
| SC-001 | T185, T187, T189, T191, T356 | T186, T188, T190, T192, T357 |
| SC-002 | T285, T288, T291, T302, T304, T306 | T286, T289, T292, T303, T305, T307 |
| SC-003 | T200, T202, T222, T234, T264 | T201, T203, T223, T235, T265 |
| SC-004 | T338, T343, T345, T347 | T339, T344, T346, T348 |
| SC-005 | T155, T157, T320, T324 | T156, T158, T321, T325 |
| SC-006 | T064, T329, T331, T358 | T065, T330, T332, T359 |
| SC-007 | T333, T335, T358 | T334, T336, T359 |
| SC-008 | T264, T266, T308, T310, T322, T356 | T265, T267, T309, T311, T323, T357 |
| SC-009 | T206, T208, T210, T212, T214, T216, T266 | T207, T209, T211, T213, T215, T217, T267 |
| GR1–GR9 boundary only | T218, T362 | T219, T363; actual GR-prefixed invariant tests remain owning-feature release blockers |

## Port Completeness — Not Temporal Ordering

Trait cells name the implementation task; each has its own preceding surface-test task.
Contract cells name generic-suite red tasks; fake/real cells name implementations, with runner
tasks immediately preceding them. All real cells require the fake cell's contract to be green.
Mock-test cells refer to actual service interaction tests, not adapter-mocking or empty mocks.

| Port | Trait task | Contract suite task | Fake task | Real adapter task | Mock-test task(s) |
| --- | --- | --- | --- | --- | --- |
| Clock | T035 | T036 | T037 | T049 | T150, T245, T287, T290 |
| IdGenerator | T039 | T040 | T041 | T049 | T144, T236, T242, T284 |
| UnitOfWork / TxHandle lifecycle | T043 | T044 | T045 | T051 | T046, T129, T135, T147, T284 |
| HealthProbe | T071 | T072 | T073 | T261 | T074 |
| AuditRepository | T094 | T095 | T096 | T158 | T135, T138, T144, T147, T284, T317 |
| AdminRepository | T098 | T099 | T100 | T160 | T129, T144, T147, T150 |
| AdminCredentialRepository | T102 | T103 | T104 | T162 | T135, T138, T141, T147 |
| SessionStore | T106 | T107 | T108 | T164 | T135, T138, T147, T150 |
| AuthStateRepository | T110 | T111 | T112 | T166 | T132, T135, T141, T147 |
| OtpChallengeRepository | T114 | T115 | T116 | T168 | T129, T132, T147 |
| RateLimitRepository | T118 | T119 | T120 | T170 | T129, T132 |
| AdminAuthenticator | T122 | T123 | T124 | T172, T174, T176, T178, T180 | T129, T132, T135, T138, T141 |
| SmsSender | T126 | T127 | T128 | T182 (Kavenegar), T184 (SMS.ir) | T129 |
| SettingsRepository | T221 | T222 | T223 | T253 | T236, T239, T242, T284 |
| FlagRepository | T225 | T226 | T227 | T255 | T236, T239, T284, T342 |
| ChangeRepository | T229 | T230 | T231 | T257 | T242, T245, T281, T284, T287, T293 |
| ImpactAssessment | T233 | T234 | T235 | T259 | T245, T281, T284, T290 |
| PublicationRepository | T274 | T275 | T276 | T299 | T284, T287, T290 |
| CapabilityReadiness | T278 | T279 | T280 | T301 | T284, T290, T342 |
