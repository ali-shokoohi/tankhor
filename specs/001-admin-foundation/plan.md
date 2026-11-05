# Implementation Plan: Tankhor Admin Foundation

**Branch**: `001-admin-foundation` (Spec Kit identifier; actual Git branch remains `main`)

**Date**: 2026-11-05 | **Spec**: [spec.md](spec.md)

**Input**: `specs/001-admin-foundation/spec.md`; user planning prompt; common block from
`docs/tankhor-rust-implementation-pack.md` §10, with stack/testing rules from §§2–7.

**Status**: Phase 0 research and Phase 1 design complete; in-place remediation against
constitution v1.2.0 aligned at design level. Existing tasks are edited directly, not
regenerated. No application code, migrations, deployment or runtime test evidence is created.

**Owner consolidation**: 580 → **491 tasks**; 19 → **16 ports**. All 47 canonical acceptance
names, 38 FRs and nine outcomes remain. The full 580-baseline → current-ID map and assertion
destinations are in tasks.md's Owner Consolidation section. No task is marked complete.
These current owner decisions supersede incompatible older research/quickstart examples
about publication coordination and MinIO fixtures; those background files are outside this
requested eight-file edit set and are not rewritten here.

**Owner follow-up decisions**: 491 → **491 tasks** (net zero); existing drivers gain all
new assertions. TOTP provisioning follows the constitution's authenticated, exactly-once,
no-store UI exception, never logging, redisplaying or sending the secret/QR elsewhere.
Phase gates validate planned-artifact/driver/completion links for completed work only; the final gate
requires every actual port symbol/runner/interaction and both feature graphs. Test partial
and final modes. Shared setup is activated/smoke-tested only after the live-run lease pair
passes; the wrapper still follows its own observed intended red. T460–T462 are regression
validation of existing flag guards unless a genuinely new flag-specific case is named first.

## Summary

Design the first tested Cargo workspace and a real admin foundation: admin-only phone/SMS
primary verification, passkey/TOTP second factor, independent server-side admin sessions,
another-admin addition/recovery, ten bounded settings, six OFF flags, exact-version review,
durable publication/rollback, protected audit, and an accessible FA/EN theme shell.
The owner selected **Kavenegar + SMS.ir** for the admin-only primary-verification transports.
Public sign-in/account lifecycle remains Feature 002; later product capabilities are not stubs
hidden behind flags. Crypto and SMS implementations require their own adapters rather than
being performed by handlers. (FR-CON-01–11/32/37; FR-UX-01–09/14/17; Q01–Q03/Q09/Q23)

Publication commits live pointer, immutable history, completion and success audit atomically.
Recovery atomically revokes all target sessions/factors and outstanding proof state. Scheduled
changes recheck affected live versions and current prerequisites; overdue work runs once on
recovery without backdating. Owner-approved defaults, ranges and preservation rules are
unchanged. Design detail and rationale are in [research.md](research.md).

Scheduled requests store scheduling_admin_id, scheduling_security_epoch and approver_id.
The worker acts on behalf of the scheduling admin, rechecking active eligibility, epoch and
approval validity under the final transaction. Revocation, recovery or deactivation produces
terminal cancellation plus audit and a visible reason, no live effect; another admin must
reschedule through the reviewed workflow. Audit actor = scheduling admin, executed_by =
system worker, with approver recorded. This adds no admin-deactivation route or lifecycle.
Every committed admin revocation, deactivation or recovery atomically advances that admin's
durable, monotonic security epoch through the existing AdminRepository/UoW, under the same
admin-row lock used by finalization. Reactivation/restoration never resets or reuses an
epoch. Compare the captured scheduling epoch to the current value: any mismatch cancels
with audit and a visible reason even when eligibility has been restored before execution.
Existing domain, shared fake/real repository and ScheduledPublish drivers cover
deactivate→reactivate and revoke→restore before execution (cancelled), plus unchanged epoch
with all existing gates satisfied (executes). No tasks, ports or endpoints are added.
Console scheduling accepts local date+time in a display zone (default Asia/Tehran), rejects
ambiguous/nonexistent local times and past submissions, and stores UTC plus the display zone.
JSON time decoding requires ISO 8601 with explicit offset (Z or a numeric offset); no new JSON endpoint.
Use Asia/Tehran and Europe/Berlin DST boundary fixtures; previously authorized overdue work
is not rejected merely because its persisted schedule is now past.

## Technical Context

**Language/Version**: Rust stable 1.98.1, edition 2024; pin toolchain, rustfmt, clippy and
llvm-tools-preview. Single Cargo.lock and shared workspace dependency/lint declarations.

**Primary Dependencies**: tokio 1.x; axum 0.8.9; askama/askama_web 0.16 with `axum-0.8`;
Fluent bundles; vendored htmx 2.x; SQLx 0.9.0; utoipa 5.5/utoipa-axum 0.2/Swagger UI 9.0.2
(`axum,vendored`); webauthn-rs 0.6.2; totp-rs 6.0.0; aes-gcm 0.11.1; reqwest in infra-sms
only; uuid, time, serde, thiserror, async-trait and sha2 as appropriate pure libraries;
pure chrono/chrono-tz with pinned embedded IANA rules for local scheduling resolution.
Research R12–R13 verifies metadata/protocols; the first TDD build resolves exact compatible
patches into Cargo.lock. No dependency compilation is claimed by planning.

**Storage**: PostgreSQL 16 via infra-postgres; server-side admin sessions, proof state,
credentials, revisions, publications and audit. Separate migration/runtime roles. MinIO
is a pinned local-stack service only; media/storage behaviour is not Feature 001 work.

**Testing**: nextest; unit/proptest; mockall on ports; transactional testkit fakes; shared
fake/real suites; Testcontainers modules 0.15/core 0.27; wiremock; axum oneshot/scraper/insta;
test-only Playwright/axe plus required real-device checks. Pin cargo-nextest 0.9.151 in CI
and developer toolchain docs. Owner-approved integration setup shares labelled PostgreSQL
across a run, with a fresh SQLx database per test; MinIO fixture implementation moves to 004;
plain Cargo/IDE execution has a testkit process-launcher fallback. All 47 named scenarios
are mapped in [contracts/test-plan.md](contracts/test-plan.md).

**Target Platform**: Linux services; one HTTPS origin with `/console`; browsers per PRD
§12.7, viewports 360/768/1024/1440 px; FA RTL/EN LTR; light/dark/system. WebKit emulation
does not replace actual iOS Safari acceptance.

**Project Type**: Rust hexagonal monorepo, SSR web application and scheduled-publication worker.

**Performance Goals**: paginated audit/registry reads, bounded request bodies/concurrency,
indexed bounded due-work reads, no vendor calls under database locks, no full-table population
scans in handlers. Measure PRD proposed latency/availability targets; do not promote them
or the pack's proposed mutation/other-crate coverage percentages to decided requirements.
The 1,000/day generation target and 2× load test remain later full-product checks, not a
foundation capacity claim. (Q28; NFR-PERF-01–05; NFR-CAP-01)

**Constraints**: no I/O libraries in contexts; app sole composition root; adapters do not
call adapters; web/worker use application services only; no secrets/PII in logs; secure
cookies/CSRF/CSP; genuine primary and second-factor checks; ≥90% kernel coverage and the
same floor for wallet/generation/identity as delivered; no vendor APIs from tests.

**Scale/Scope**: ten settings, nine finite numeric ranges plus JPEG-mandatory format subset;
six OFF flags; equal full admin permissions; one subject per draft; 47 acceptance scenarios,
38 feature requirements and nine measurable outcomes. No invented admin-count/throughput SLA.

## Constitution Check

Gates apply before Phase 0, after Phase 1 and after this task remediation. PASS means
conformant design only, not built software or successful runtime tests. The initial
repository contains documentation only.

| Principle | Pre-research gate | Post-design evidence | Result |
| --- | --- | --- | --- |
| I — TDD | Tests must precede code; tooling failures are not red evidence | All 47 AC names preserved; focused red/green pairs; setup precedes behavioral tests; regression checks do not manufacture failures | PASS (design only) |
| II — Hexagonal Rust | One workspace and pure context boundaries | infra-auth/infra-sms added where needed; cargo-deny plus metadata/import checks; no empty future context crates | PASS |
| III — Ports/UoW | Every external dependency has an async automock port, fake, shared contract and infra-* adapter | Sixteen ports; SessionStore absorbs pending authentication state; pure impact/readiness are not ports; fake contracts pass before real adapters; mock/fake tests precede services; app-wide mode, fake smoke and production/release guards retained | PASS (design only; implementation evidence pending) |
| IV — Money | No raw-money/ledger shortcuts | No money behaviour in 001; no simulation writes balances; future debit/unit/outbox and return atomicity retained as later gates | PASS |
| V — SSR | Askama/Fluent/progressive, bilingual accessible shell | HTML/form contracts, local assets, approved Vazirmatn/palette; no new brand or SPA | PASS |
| VI — JSON contract | All JSON under `/api/v1`, generated schema/errors | Annotated health and schema-serving routes; local Swagger `/api/docs`; documented finite bootstrap; offline export/diff | PASS |
| VII — Security/privacy | Real authorisation, independent factors/sessions, no leaks | Concrete SMS protocols, crypto/replay/epoch contracts, append-only runtime roles, failure-atomic audit; no plaintext secret persistence | PASS |
| VIII — Traceability/simplicity | Closed Q decisions, no speculative capabilities | Exact owner policies; single-subject drafts; explicit unknown impact/not-delivered readiness; no false P0-OFF completion | PASS |

**Post-design caveats, not waivers:** production SMS account/template access, encryption keys,
trusted TLS, operator-authorised initial provisioning and developer-tool installation are
documented prerequisites. Constitution v1.2.0 retains original ratification 2026-11-03; no governance
metadata is changed here. Missing prerequisites block deployment/acceptance, never
allow fake primary authentication, healthy stubs or silent gate bypasses.

## Project Structure

### Documentation (this feature)

```text
specs/001-admin-foundation/
├── spec.md
├── plan.md
├── research.md
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── contracts/
    ├── application-ports.md
    ├── integration-fixtures.md
    ├── http-ui.md
    └── test-plan.md
```

`tasks.md` is a later `/speckit.tasks` output, not an artifact of this command.

### Source Code (repository root)

The following is the implementation target, not a claim these files already exist:

```text
Cargo.toml / Cargo.lock / rust-toolchain.toml / deny.toml
.config/nextest.toml
.cargo/config.toml                  # Test-aware fallback runner; pass through other commands
.github/workflows/{ci,nightly,release-readiness}.yml
scripts/{check-architecture.py,check-test-traceability.py,check-release-readiness.py}
scripts/{setup-integration.sh,test-runner.sh,test-down.sh,tests/}
justfile
deploy/{compose.yml,Dockerfile}
crates/
├── kernel/src/
│   ├── {ids,locale,error}.rs
│   └── ports/{clock,id_generator,unit_of_work}.rs
├── admin/src/{domain/,application/,ports/}
├── infra-postgres/{src/,migrations/,tests/}
├── infra-auth/{src/,tests/}
├── infra-sms/{src/,tests/}
├── infra-system/src/{clock,id_generator}.rs
├── web/{src/,templates/console/,templates/partials/,locales/{fa,en}/,static/,tests/}
│   ├── design/tokens.toml           # ONLY source of visual identity values (FR-UX-20)
│   ├── build.rs                     # Generates CSS custom properties from design/tokens.toml
│   └── static/brand/                # Replaceable brand assets: logo/wordmark SVG, Lottie, reduced-motion SVG stills
├── worker/{src/,tests/}
├── testkit/{src/{fakes,contracts,fixtures,builders}/,src/bin/fixture-runner.rs,tests/}
└── app/{src/,tests/}
e2e/{package.json,package-lock.json,playwright.config.ts,admin-foundation.spec.ts}
docs/openapi.json
docs/development.md                 # Pin Rust and nextest/tool installation versions
```

**Structure Decision:** Context crates are added only when their feature is built. Kernel
contains pure ID/value/error types and async Clock/IdGenerator/UnitOfWork traits in
`crates/kernel/src/ports/`. FixedClock and deterministic IDs live only in
`crates/testkit/src/fakes/` behind feature `fakes`; real clock/UUID implementations live in
`infra-system`. App selects and wires adapters but implements none. Admin modules:
typed settings/flags/revisions/approval/audit/access; SaveDraft, Preview, Approve, Publish,
Rollback, ScheduledPublish, access/enrollment/recovery, registry/audit queries and health
application services; repositories and crypto/SMS ports. Infra-postgres owns
SQLx/migrations. Infra-auth owns WebAuthn/TOTP/token/secret protection. Infra-sms owns only
provider HTTP protocols. Web never verifies factors or queries repositories directly.

BootstrapAdmin owns the existing operator-authorised eligibility/enrollment/audit transition
in `admin/application`; the provisioning CLI only parses protected input/config and delegates.
GetEnrollment owns read-only protected identity authorization and SessionStore primary-proof/
stage/browser/session/purpose/target/epoch/expiry checks before the
enrollment page. Both service slices have failing mock/fake drivers before implementation
and before US1 real storage; no new port, public endpoint or product requirement is added.

**Pure policies:** ImpactAssessment and CapabilityReadiness are direct pure calculations in
`admin/domain`, not ports or adapters. Typed input/provenance cases live in original domain
and workflow drivers. A code-owned delivered-capability registry marks all six capabilities
not delivered; no writable health override. Missing population evidence stays explicitly
unavailable and blocks destructive publication, never an invented zero.

**Owner simplifications:** enrollment authorization is protected, audited target-admin state,
not a transferable token. Target verifies phone OTP then enrolls passkey/TOTP; SessionStore
owns pending proofs and ceremonies. AdminRepository owns authorization; no AuthStateRepository.
Epochs, one-use/replay, CSRF, challenge/session binding and recovery revocation remain.
Publication reads bounded durable due candidates, gathers evidence outside write locks, then
uses short transactional row locking, unique publication identity and conditional finalization.
Baseline, one-effect HTTP/worker retry, audit atomicity, unknown-outcome lookup and overdue
rechecks remain; no publication lease/fencing protocol. The PostgreSQL fixture live-run lease
is a separate safety mechanism and remains unchanged.

**Incremental foundation:** before any user story, debug fake mode starts only the delivered
shell and health routes with outbound networking denied. Health is unavailable until its
genuine seed prerequisite exists; no synthetic readiness or placeholder management route is
installed. Each story extends this same composition. One application-wide `real|fake` mode
selects a coherent backend, without per-group overrides or mixing. Production rejects fake mode
before resource acquisition; release builds exclude mocks/fakes features.

AdminAuthenticator's real port implementation is T224. T216/T218/T220/T222 implement its
internal envelope/capability/TOTP/passkey dependencies, not additional real port adapters;
the unchanged shared contract drives the aggregate adapter through T223–T224.

**Design tokens:** `crates/web/design/tokens.toml` is the only source of visual identity
values: light and dark colours including `borderControl`, typography, spacing, radii and
motion (180/500 ms), with values taken from @docs/tankhor-technical-spec.md §12.
`crates/web/build.rs` generates CSS custom properties from it. Brand assets (logo and
wordmark SVG, Lottie, reduced-motion SVG stills) are replaceable files under
`crates/web/static/brand/`. Rationale in [research.md](research.md) R14. (FR-UX-20, FR-UX-07; Q02–Q04)

### Phase 0 — Research outcome

[research.md](research.md) resolves transaction representation, fake semantics, dependency
enforcement, schedule idempotency, audit roles, no-stub impact/readiness, HTML-only factor
ceremonies, OpenAPI bootstrap, fixture lifetime, concrete SMS protocols and compatible crate
families. No design clarification remains; source values marked proposed stay proposed.

### Phase 1 — Design outcome

- [data-model.md](data-model.md): entities, SQL ownership/constraints/indexes, security state,
  immutable approvals/versions, seeds, recovery/publication transitions and migration order.
- [contracts/application-ports.md](contracts/application-ports.md): all application, repository,
  transaction, authentication, transport and assessment contracts with fake/real suites.
- [contracts/http-ui.md](contracts/http-ui.md): separate HTML/form and JSON surfaces,
  templates, Fluent keys, authorisation/CSRF, errors, progressive behaviour and Swagger.
- [contracts/test-plan.md](contracts/test-plan.md): named AC coverage and additional domain,
  contract, integration, HTTP/template, browser, architecture and workspace tests.
- [contracts/integration-fixtures.md](contracts/integration-fixtures.md): owner-selected
  shared-run PostgreSQL, per-test database isolation, safe process fallback and cleanup;
  MinIO startup/bucket fixture implementation is deferred to feature 004. Keep Compose
  MinIO; no storage behavior is claimed tested in feature 001.
- [quickstart.md](quickstart.md): prerequisites, future executable validation commands and
  expected results, clearly distinguished from current planning-only validation.

### Test-first implementation sequencing for later tasks

Tool installation, fixture startup, Playwright/axe configuration and SQLx metadata preparation
are explicit **setup** tasks. Missing tools or harnesses never count as a behavioral red.
Executable harnesses precede their test tasks. Every new/split production behavior has its
own preceding failing test; application behavior is driven by both mock and fake tests.
Already implemented membership/acceptance behavior is rerun as regression coverage, not
reimplemented or deliberately broken to obtain red. Canonical six-OFF flag definitions precede
the first flag contract/seed. Each introducing story completes fake-based services before SQL.
US2 validates OI8 domain/preview policy only; its publication-policy assertion runs in US3
after Publish exists. Account deletion execution remains outside feature 001.

The existing impact gate compares canonical revision/subject, baseline version, policy/source,
effect rules and applicable known counts, as specified in application-ports §4; assessment
time/report IDs alone do not require reapproval. Changed semantic evidence requires new
preview/approval; unavailable required evidence blocks; no TTL or population inventory is
introduced. Existing domain/contract/workflow tasks exercise all three cases. Existing
passkey/TOTP/recovery CAS/replay/audit-rollback assertions belong to their original drivers.
Later audit completion, unknown-commit and publication SQL repair assertions likewise move
into the original service/contract drivers; aggregate and real-storage acceptance remain.

| Driving failing tests first | Minimum implementation slice | Required green validation |
| --- | --- | --- |
| Architecture/lint inheritance, UUIDv7/locale/error/Clock/UoW contracts | Workspace and kernel | Kernel nextest, architecture negatives, clippy/fmt |
| Bounds/proptest/format subset; draft-edit approval invalidation | Pure admin domain | Admin nextest; no I/O dependencies |
| Shared fake commit/rollback and repository suites | Transactional testkit fakes and mock application suites | Testkit nextest, same-backend contracts |
| Runtime audit UPDATE/DELETE/TRUNCATE denial, atomicity/CAS/schedule races | Migrations and PostgreSQL UoW/repositories | Full infra-postgres integration |
| Provider protocol failures/redaction and factor replay/epoch tests | Real infra-auth and owner-selected infra-sms adapters | Auth/SMS unit+wiremock and PostgreSQL storage suites |
| Admin without second factor denied; recovery revocation; CSRF | Access services and HTML flows | HTTP + real-crypto tests, not mock-only acceptance |
| Draft-only save, exact preview, known/unknown gates, overdue retry | Management services and worker | Fake/real contracts + restart/concurrency integration |
| RTL/template/Fluent/contrast/no-htmx/OpenAPI snapshots. Contrast unit: WCAG 2.2 contrast for every declared text/background pair in `tokens.toml`, ≥ 4.5:1 normal, ≥ 3:1 large text (FR-UX-20, FR-UX-07). Single-source unit: build or test fails if any hex colour appears in templates/CSS outside the generated tokens file (FR-UX-20). Token-set unit: `tokens.toml` contains every colour token named in tech spec §12.2, including `borderControl`, and no colour token that §12.2 doesn't list | Web shell, `tokens.toml` + `build.rs` CSS generation, JSON health/schema and local Swagger | HTTP/templates/schema parity + token unit tests + browser/axe |
| Real seed idempotency and J5-foundation workflow | Composition, seed command, local stack and CI | Scoped E2E, coverage and release-readiness checks |

Every production slice still needs its own observed failing test. This table is sequencing
guidance, not permission to create workspace/migrations before their tests or mark tasks done.

### CI and release gates

- `cargo fmt --check`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`,
  `cargo deny check`, tested metadata/import checks and traceability checks are required.
  Every crate inherits `unsafe_code = "forbid"`, clippy `pedantic = "warn"` from the workspace.
- Nextest default/ci filters exclude explicitly named integration binaries; integration
  profile selects them with a bounded test group. Pin cargo-nextest 0.9.151, enable top-level
  experimental setup scripts, and scope the idempotent service setup to integration tests.
  Setup exports endpoints through `$NEXTEST_ENV`; shared services do not imply shared test
  databases. Serialize independent local runs reusing the same PostgreSQL server.
- Fixture lifecycle helpers instead use `fixture_bootstrap_*` binaries and the dedicated
  `fixture-bootstrap` profile with no shared setup/fallback launcher. Install this profile
  before testing helpers. Implement the export wrapper after ownership/startup/role tests
  pass and its own intended failure is observed, without activating the shared hook. Only
  after export and the live-run lease driver/implementation are green activate and smoke-test
  shared setup. Default/ci and normal
  integration exclude bootstrap binaries;
  the Phase 1 checkpoint/CI run bootstrap and shared validation separately without recursion.
- Unit/mock/fake/HTTP run on every PR. Integration runs on infra-*/wallet/generation changes
  and nightly for all present adapters. Real restricted-role and trigger tests are mandatory.
- SQLx checked queries have committed `.sqlx/` metadata, generated with the intended migrated
  role/schema **with each adapter/query change, before that pair's offline green checkpoint**.
  Apply the current migrations to the isolated preparation database; run
  `SQLX_OFFLINE=false cargo sqlx prepare --workspace -- --all-targets --all-features`, then
  restore `SQLX_OFFLINE=true` and run the driving/affected/integration tests. This online
  preparation is a setup operation, not red evidence or a production offline-mode waiver.
  It is folded into every retained adapter's completion criterion: **sqlx metadata prepared
  and offline check passes before green**, not a standalone follow-up task.
  The final metadata tasks repeat prepare/check across the complete workspace; they are
  not the first metadata generation. CI checks freshness. Ordinary builds use `SQLX_OFFLINE=true`;
  prepare/check has access to the intended migrated test database. Compile-time SQL checking
  is not the audit privilege test. Each `#[sqlx::test]` uses the canonical workspace migrator,
  while adapter pools connect with restricted runtime credentials.
- CI always runs exact-run-label fixture teardown, including failure/cancellation cleanup
  where supported. Local fixtures persist until `just test-down`. Failed SQLx databases
  survive until the same test is rerun or infrastructure is removed; successful cleanup is
  verified separately because SQLx treats some drop failures as best effort.
- Generate `docs/openapi.json` offline, snapshot it and fail any diff from the committed
  artifact. Validate every JSON response, including the document endpoint, against its schema.
  The existing dual-backend browser matrix also loads protected `/api/docs`, proves actual
  Swagger initialization/schema fetch under strict CSP, and rejects CSP/off-origin attempts.
- Enforce ≥90% line coverage on each present kernel/wallet/generation/identity crate. Report
  other-crate coverage without silently adopting the proposed 80% threshold. Missing future
  crates are reported as not delivered, not as successful coverage results.
- Nightly mutation runs wallet/generation domain modules when delivered; survivors are
  reviewed/tracked. The proposed 85% score is not an approved numeric gate.
- Nightly/pre-release E2E includes all delivered journeys. A separate full-product release
  check fails until all required contexts, J1–J5, guardrails and PRD launch gates exist and
  pass. Feature 001's scoped J5 test is not labelled the complete J5 or a product release.
- Partial checkpoints check planned-artifact/driver/completion links for completed work only;
  unfinished feature symbols are not prerequisites for Phase 1. The final T491 gate runs
  the full actual-symbol/runner/direct-interaction inventory for all sixteen ports, including
  both SmsSender adapters and real/fake feature graphs. The original checker driver tests
  valid partial work, premature completion, final missing evidence and the complete state.

## Complexity Tracking

No constitution exceptions are requested. Infra-auth/infra-sms are necessary adapters for
required authentication, not additional product contexts. Infra-system groups the two local
port adapters without creating a new business context or a policy-only crate. The Cargo
metadata audit closes a
real cargo-deny limitation. Shared integration services and a test-only process launcher
implement the owner's 2026-11-04 fixture decision; no unsafe process-environment mutation or
false in-process cross-nextest singleton is used. No event bus, generic workflow engine,
speculative plugin system, public auth API or unbuilt flag behaviour.
