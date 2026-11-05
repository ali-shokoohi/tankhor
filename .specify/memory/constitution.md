# Tankhor (تن‌خور) Constitution

## Core Principles

### I. Test-Driven Development (Non-Negotiable)

- Production code MUST be driven by a test written and run first, observed to fail for the
  intended missing behaviour. The cycle MUST be red → minimum green implementation → refactor
  with tests remaining green. Writing tests after production code does not satisfy TDD.
- Every implementation task in `tasks.md` MUST have a preceding test task that drives it.
  Every acceptance criterion (`AC-*`) and guardrail (`GR*`) MUST map to at least one named test.
  Guardrail test names MUST contain the corresponding GR ID.
- Tests MUST cover the applicable layers: pure domain unit and `proptest` tests; `mockall`
  tests on ports; fake/behaviour tests with in-memory adapters; shared fake/real contract
  suites; integration tests with Testcontainers PostgreSQL and MinIO and `wiremock` vendors;
  HTTP/template tests with axum `oneshot`, `scraper` and `insta`; and browser E2E journeys J1–J5.
- Line coverage MUST be at least 90% in each of `kernel`, `wallet`, `generation` and `identity`.
  Mutation testing MUST run on `wallet` and `generation` domain rules; surviving mutations
  MUST be reviewed and unresolved test gaps MUST be recorded, not concealed by exclusions.
- Tests MUST NOT call vendor HTTP APIs; vendor behaviour MUST be simulated with `wiremock`.
  A test MUST NOT be weakened to obtain green. A correction to an erroneous test MUST be
  explained in the commit message with the governing requirement.

This makes financial, privacy and generation guarantees executable rather than aspirational.

### II. Rust Monorepo and Hexagonal Architecture

- Server-side production code MUST be Rust in one Cargo workspace. Each bounded context
  (`admin`, `identity`, `wallet`, `media`, `generation`, `catalog`, `tryon`, `studio`, `notify`,
  `reporting`) MUST have its own crate with `domain/`, `application/` and `ports/` modules.
  Shared value types belong in `kernel`; concrete adapters belong in `infra-*` crates.
  The workspace MUST also separate `web`, `worker`, `testkit` and `app` responsibilities.
- Domain code MUST be pure and free of I/O. Context crates MUST NOT depend on `sqlx`, `axum`,
  `aws-*`, `reqwest` or other I/O libraries. Dependency boundaries MUST be enforced in CI
  through `cargo-deny` and context-manifest checks.
- Context dependencies MUST be limited to `kernel`, pure libraries and other contexts'
  public ports/DTOs where unavoidable. Infrastructure adapters MUST implement ports and
  MUST NOT call other infrastructure adapters directly.
- `web` and `worker` MUST call application services only, never repositories directly.
  `app` MUST be the sole composition root wiring concrete adapters into services.

These boundaries keep business rules independently testable and vendors replaceable.

### III. Repository Pattern with Unit of Work

- Every dependency on storage, time, ID generation, randomness, crypto challenges or any
  external service MUST be a port: an async trait in `<context>/src/ports/`, annotated with
  `#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]` ABOVE
  `#[async_trait::async_trait]`.
- Application services MUST depend only on port traits for these dependencies. Domain and
  application code MUST NOT call the system clock or generate IDs directly; `Clock` and
  `IdGenerator` MUST be ports. Domain code MUST remain pure as required by Principle II.
- Every port, without exception, MUST have all four of the following:
  1. A `mockall` mock for interaction tests.
  2. An in-memory fake in `crates/testkit` behind feature `"fakes"`.
  3. A contract test suite in `crates/testkit` run against BOTH the fake and every real adapter.
  4. At least one real adapter in an `infra-*` crate.
- Every persistence need MUST remain a repository-port method, with its fake, shared contract
  suite and PostgreSQL implementation in `infra-postgres` satisfying the same four rules.
- Writes spanning repositories MUST go through the `UnitOfWork` port, which MUST satisfy
  the same four rules. Transaction handles exposed to contexts MUST remain infrastructure-
  independent; SQL transaction types MUST stay inside adapters. Commit and rollback
  behaviour MUST be tested, including failures.
- The `app` crate MUST be the only composition root. Adapters MUST be selected by
  configuration, and the whole application MUST be runnable locally on fakes with no
  database or external service. Startup MUST fail when the environment is production and
  any fake adapter is selected. Release builds MUST NOT include the `"mocks"` or `"fakes"`
  features.
- Required per port: mock, fake, contract suite, real adapter. This list specifies required
  artifacts, not a time order. The contract suite MUST pass on the fake before any real
  adapter is written. Mock and fake tests for an application service MUST be written and
  observed to fail before that service's implementation. The same contract suite MUST
  drive the real adapter through the TDD cycle in Principle I.

Ports make every external dependency replaceable; shared contracts keep fakes honest, and
a Unit of Work preserves cross-repository invariants without sacrificing local isolation.

### IV. Money Is Sacred

- `Tokens` and `Irr` MUST be distinct newtypes with checked arithmetic and no implicit
  conversion. Raw integers MUST NOT represent money at domain or application boundaries.
- The token ledger MUST be double-entry and append-only. Corrections MUST be compensating
  entries with actor and reason, never edits or deletion of history (L1–L3).
- The database MUST enforce unique idempotency keys with the prefixes `pay:`, `debit:`,
  `return:`, `adj:` and `forfeit:`, and `CHECK (balance >= 0)`. Cached wallet balances MUST
  equal ledger totals. Concurrency tests MUST prove no overspend, double debit, duplicate
  payment credit or duplicate return (L1–L5; GR1–GR4).
- A debit, its generation unit and its outbox row MUST commit in one transaction (L6).
  Internal attempts MUST NOT charge again; a unit MUST have at most three total attempts,
  not an initial attempt plus three retries (Q29; L7). Final failure and its exact token
  return MUST commit in the same transaction. UI MUST say "returned" only after recording it.
- Generation MUST require explicit confirmation of the garment and price. Each confirmed
  unit MUST store its price version, output-type base price, style premium and add-ons.
  Changed prices MUST require fresh confirmation; later prices MUST NOT alter a snapshot
  (Q36; GR9).
- Payment credit MUST follow server-side gateway verification. Blocked wallets MUST retain
  their balance and reject new spend; simulations MUST NOT write real ledger entries
  (L4, L8–L9). Token returns MUST NOT be represented as IRR refunds or cash-out.

### V. Server-Rendered UI

- HTML MUST use axum, Askama with `askama_web`, and locally vendored htmx. A SPA framework
  MUST NOT replace server-rendered flows. Every interaction MUST also work as a full-page
  request without htmx; partial updates MUST be progressive enhancement.
- Every surface MUST support Persian (`fa`, RTL) and English (`en`, LTR), with correct
  `lang` and `dir`, and light/dark/system themes from the first template. All UI strings
  MUST come from Fluent; locale-key parity MUST be tested. CSS MUST use logical properties.
- Templates and interactions MUST meet WCAG 2.2 AA, including keyboard operation, accessible
  names, visible focus, associated form errors, contrast and reduced-motion alternatives.
  HTTP/template tests and browser accessibility checks MUST verify these obligations.
  Every text/background pair MUST be checked automatically against WCAG 2.2 AA using
  unrounded contrast ratios: at least 4.5:1 for normal text and 3:1 for large text
  (technical specification §12.2 and §12.6). Automated flat-colour checks MUST NOT replace
  real-sample accessibility checks for textures, photos, opacity and rendered interactions.
- Fonts, scripts and other UI assets MUST be self-hosted, never CDN dependencies. Typography
  and all other visual identity values (colours, radii, icons and motion) MUST come only
  from `crates/web/design/tokens.toml`, the single design tokens source governed by the
  technical specification §12.2 and Q02–Q04. CSS MUST be generated from that source.
  Hex colours MUST NOT be authored elsewhere; generated CSS MUST derive its colour values
  from the token source. Independent visual identity values copied from brand references
  or hard-coded elsewhere MUST NOT be used. Language/theme changes MUST preserve user input.

### VI. API Contract

- Every JSON endpoint MUST live under `/api/v1`, be annotated with `#[utoipa::path]`, and
  be registered through `utoipa-axum`'s `OpenApiRouter`. Each endpoint MUST appear in the
  generated OpenAPI document and Swagger UI at `/api/docs`.
- JSON errors MUST use the shared RFC 9457 `application/problem+json` schema. Money-moving
  POSTs MUST require and document `Idempotency-Key`. HTTP tests MUST validate response
  status and body against the generated contract.
- `docs/openapi.json` MUST be committed. CI MUST fail when generation differs from the
  committed document or an API route lacks documentation. HTML routes MUST have HTTP tests
  but MUST NOT be represented as JSON API endpoints.
- API documentation MUST be admin-protected in production, as required by the implementation
  pack. Documentation MUST NOT bypass request authorisation.

### VII. Security and Privacy by Default

- Every request and file access MUST enforce server-side authorisation, allowing anonymous
  access only where the product explicitly permits it. Admin access MUST require a passkey
  second factor with TOTP fallback and a session separate from end users (Q23).
- Private media MUST be served only through short-lived signed URLs, never permanent public
  URLs (GR5). Admin viewing of private media MUST require a linked report/support ticket ID,
  be scoped to that case and produce an attributable audit entry (Q12; GR6).
- Logs MUST NOT contain phone numbers, OTPs, keys, photos or payment data. Secrets MUST NOT
  appear in UI or client bundles. Audit records MUST be append-only and access-controlled.
- Uploads MUST be validated by actual content, size and dimensions, re-encoded and stripped
  of metadata including GPS before use. Q05 limits MUST be checked before payment. Permanent
  personal-photo storage MUST require explicit consent and an AI-processing notice.
- Sessions MUST use secure, HttpOnly cookies; state-changing forms and htmx requests MUST
  be CSRF-protected. OTPs MUST be hashed, single-use, expiring and rate-limited.
- Garment-link ingestion MUST enforce the onboarded-store allowlist on the final host after
  redirects, block private network targets and cap redirects and size (Q31). Security and
  financial integrity MUST NOT be bypassable through settings or feature flags.

### VIII. Traceability and Simplicity

- Specs, plans, tasks, tests and commits MUST cite applicable PRD IDs (`FR-*`, `AC-*`, `GR*`)
  and decision IDs (`Q*`); ledger rules MUST also retain L1–L9 references where relevant.
  Feature traceability MUST connect each in-scope acceptance criterion and guardrail to
  named tests and their driving implementation tasks.
- Decisions Q01–Q41 are final. Contributors MUST NOT reopen or change a decided value
  without explicit owner approval recorded with its Q ID. Genuine source conflicts or
  unanswered requirements MUST be escalated, not silently resolved by invention.
- The design MUST be the simplest that passes the required tests. New abstraction or
  dependency MUST solve a current, cited requirement; speculative infrastructure and
  out-of-scope capabilities MUST NOT be added.
- A feature MUST NOT be marked complete before it is implemented, wired and tested.
  P0-OFF means fully built and tested but disabled, never a stub behind a flag. P2 and OUT
  items MUST NOT be built merely because the source documents mention them.

## Sources of Truth and Product Constraints

The project is Tankhor / تن‌خور (ZWNJ U+200C), formerly Taneh (Q38). Current project and
product references MUST use the approved name; the former name is historical only.

`docs/README.md` MUST be read first as the source-of-truth registry. Only files marked
**Authoritative** there MAY decide product or implementation requirements. The currently
registered governing sources MUST be read before feature specification or planning:

- `docs/tankhor-prd.md`: product behaviour, priorities, FR/AC IDs, GR1–GR9 and journeys J1–J5.
- `docs/tankhor-technical-spec.md`: normative implementation behaviour, L1–L9, page/flow
  details, security and design requirements.
- `docs/tankhor-decisions-qa.md`: approved Q01–Q41 answers; these override superseded options
  and earlier open questions, not merely their summary labels.
- `docs/tankhor-rust-implementation-pack.md`: Rust stack, crate boundaries, ports, adapters
  and testing strategy. It replaces earlier Go recommendations, not normative product rules.

Brand inputs, change records and derived translations MUST NOT independently decide
requirements; accepted requirements MUST be taken from the authoritative files.
`docs/fa/` MUST NOT be used for decisions. Older document versions MUST exist only as git
tags; versioned document copies MUST NOT be created in the tree.

The PRD governs product behaviour and the technical specification governs implementation
detail, subject to final owner decisions. This constitution makes the engineering rules
mandatory. If these authorities conflict, work on the affected requirement MUST stop for
owner resolution. Values marked proposed MUST NOT be promoted to final rules without approval.

The product MUST retain one domain with path-based surfaces, a shared end-user identity and
wallet, and the approved separation of Try It On, Studio and Owner Console (Q01). Legal sign-off
for Q11 and accounting sign-off for Q24 remain launch gates; this constitution does not grant
either sign-off. Approved emergency-stop behaviour MUST block new generation while allowing
running work to finish (Q20). Automated tests MUST cover all guardrails, including AI cost-cap
auto-stop (GR7), no success notification on failure (GR8), and confirmed garment/price (GR9).

## Development Workflow and Quality Gates

1. Select one task at a time from the active feature's `tasks.md`, located through
   `.specify/feature.json` when present. Before implementation, the spec and plan MUST provide
   scope, requirement IDs, port contracts and a layered test plan. Every implementation task
   MUST have its driving test task first; missing traceability blocks implementation.
2. Run the driving test and observe the intended failure before production changes. Implement
   the minimum, run the tests and refactor only on green. A task MUST be ticked complete only
   when its tests pass; commits MUST include tests and implementation with requirement IDs.
3. For code changes, run `cargo nextest run -p <crate>` for affected crates after each change.
   Adapter/SQL changes MUST also run the relevant integration suites, including
   `cargo nextest run --profile integration -p infra-postgres` for persistence changes.
   PostgreSQL and MinIO integration tests MUST use Testcontainers; vendor tests MUST use
   `wiremock`. A blocked or unrun test MUST be reported, never described as passing.
4. Each code review MUST verify TDD evidence, AC/GR-to-test mapping, crate boundaries,
   fake/real contract results, transaction invariants, authorisation, localisation,
   accessibility and API/schema consistency for the affected scope.
5. CI MUST pass `cargo fmt --check`, `cargo clippy -- -D warnings`,
   `cargo nextest run --profile ci` and `cargo deny check`. SQLx offline metadata MUST remain
   current. Integration tests MUST run on every PR touching `infra-*`, `wallet` or
   `generation`, and the full integration suite MUST run nightly.
6. CI MUST enforce the coverage floor in Principle I and the generated OpenAPI comparison.
   Mutation testing of `wallet` and `generation` MUST run nightly. E2E journeys J1–J5 MUST
   run nightly and before releases; release checks MUST cover Android Chrome, iOS Safari
   and desktop Chromium, FA/EN, and light/dark modes as required by the PRD.
7. Plans, task lists and review checklists MUST account for every port, including `Clock`,
   `IdGenerator` and `UnitOfWork`. Each port's task coverage MUST include its trait,
   contract suite, fake, real adapter and application-service mock tests; these are
   completeness requirements, not a temporal sequence. The contract suite MUST be written
   and observed to fail before implementing the fake, then pass on the fake before any
   real adapter is written. The same suite MUST be observed to fail against missing real
   behaviour before that behaviour is implemented. Mock and fake application-service tests
   MUST be written and observed to fail before the service implementation they drive.
   Review MUST verify all four port deliverables, configuration-based
   composition, database-free local execution, production rejection of fakes and release
   exclusion of the `"mocks"` and `"fakes"` features.

## Governance

This constitution governs engineering work and overrides conflicting local habits, templates
and convenience shortcuts. `AGENTS.md` supplies operational guidance, not permission to
weaken these principles. Plans and reviews MUST include an explicit constitution compliance
check; a known violation MUST block completion until corrected or an amendment is approved.

An amendment MUST identify affected principles, rationale, source requirement/decision IDs,
test and delivery impacts, and any migration or compatibility work. The project owner MUST
approve it before adoption. Amendments changing a Q01–Q41 value MUST separately record explicit
owner approval of that decision and update its authoritative record; a constitution edit
alone MUST NOT silently redefine a product decision.

Versions MUST follow semantic versioning: MAJOR for incompatible principle removals or
redefinitions; MINOR for added principles/sections or materially expanded obligations; PATCH
for non-semantic clarifications and corrections. Initial adoption is version 1.0.0. Amendments
MUST preserve the original ratification date and set the last-amended date to the amendment
date in ISO 8601 format. An unchanged review MUST NOT alter the version or dates.

Each amendment MUST include a temporary HTML-comment Sync Impact Report for human review,
recording version change, modified principles, added/removed sections and deferred TODOs.
That report MUST be removed before committing the amendment. Constitution updates MUST NOT
rewrite dependent templates or commands; they consume this document at runtime. Any separate
implementation or delivery work MUST remain outside the constitution-update scope.

**Version**: 1.1.1 | **Ratified**: 2026-11-03 | **Last Amended**: 2026-11-05
