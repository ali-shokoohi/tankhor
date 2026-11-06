# Phase 0 Research — Admin Foundation

Date: 2026-11-03. Updated: 2026-11-04. Design research only;
no implementation or passing-test claims.
Sources: `spec.md`, constitution v1.0.0, PRD E14/E17/§10/§12, Technical Specification
§8/§12, final Q01–Q41, and Rust Implementation Pack §§2–7 and its common planning block.

## R01 — Scope and primary authentication

**Decision:** Implement primary phone/SMS verification only for provisioned or invited admin
identities in this feature, followed by passkey or TOTP verification. This is not public
registration, customer sign-in, customer sessions or account lifecycle. Provisioning is
eligibility, not authentication. The owner selected **Kavenegar + SMS.ir** during planning;
record that selection here rather than putting vendor implementation details into `spec.md`.

**Rationale:** Q09 and Technical Specification P30 require base authentication plus the
second factor; no identity implementation currently exists. A fake primary verifier or
passkey-only login would silently weaken that requirement. A small admin-only slice is
necessary to deliver FR-CON-01–03 and AC-ADM-001–006 without building Feature 002.

**Alternatives considered:** Wait for Feature 002, which reverses the specified dependency;
invent password/IdP authentication, which is not authorised; treat provisioning as login,
which is insecure. SMS contracts, credentials and approved templates remain deployment
prerequisites, not evidence that provider delivery already works.

**Evidence:** PRD §10.4; Q09/Q23; Technical Specification P30; `spec.md` initial-access
dependency and recovery clarifications. Provider protocol evidence is consolidated in R12.

## R02 — Dependency boundaries and composition

**Decision:** Initial workspace members are `kernel`, `admin`, `infra-postgres`, `infra-auth`,
`infra-sms`, `web`, `worker`, `testkit`, `app`. Add other contexts/adapters only with their
features, not as empty crates pretending to be delivered functionality. `admin` contains
only `domain/`, `application/`, `ports/`. Place WebAuthn/TOTP/secret-protection implementations
in `infra-auth`; HTTP presentation in `web`; vendor transports in `infra-sms`; all wiring in
`app`. Application services orchestrate persistence and crypto; adapters never call each other.

**Rationale:** Putting crypto verification directly in handlers bypasses application policy.
The additional adapter crates enforce, rather than waive, constitution Principles II/III.
`kernel` defines UUIDv7 ID wrappers, `Clock`, `IdGenerator`, locale/errors and UoW interfaces;
real time/random generation belongs in composition-root adapters. `FixedClock` and deterministic
IDs are testkit implementations. Money/ledger behaviour remains Feature 003 work.

**Alternatives considered:** A single generic domain crate; repositories in handlers;
infra-auth calling infra-postgres; context-local SQL types. All violate the chosen boundaries.

## R03 — Enforceable architecture checks

**Decision:** Combine cargo-deny supply-chain/direct-wrapper rules with a tested
`scripts/check-architecture.py` Cargo metadata/manifest audit. Inspect resolved package
identities, aliases, workspace inheritance, normal/build/dev dependencies, inactive optional
and target-specific declarations, forbidden transitive context paths and adapter-to-adapter
edges. Context dev dependencies are not an I/O escape hatch. Add service-boundary import tests
for `web`/`worker`; cargo-deny alone cannot prove which application API a handler calls.

**Rationale:** cargo-deny's wrappers constrain direct dependents, not arbitrary context-root
ancestry. A global ban would also reject legitimate axum/SQLx adapter dependencies.

**Alternatives considered:** Grep manifest keys (aliases bypass it); global bans (overbroad);
`skip-tree` (not an architecture policy).

**Evidence:**
- https://embarkstudios.github.io/cargo-deny/checks/bans/cfg.html
- https://raw.githubusercontent.com/EmbarkStudios/cargo-deny/main/src/bans.rs
- https://doc.rust-lang.org/cargo/commands/cargo-metadata.html

## R04 — Implementable Unit of Work and honest fakes

**Decision:** Async mockable repository methods take an explicit mutable infrastructure-neutral
`TxHandle`; consuming UoW commit/rollback controls the transaction. A handle offers opaque
`Any` access for adapter-only downcasting plus backend/store identity validation. Contexts
forward it without inspecting SQL types. A pool-begun SQLx transaction can be owned without
borrowing a local connection. SQLx execution stays inside infra-postgres.

One fake database shared by all fake repositories owns transactional working state. Commit
atomically replaces committed state; rollback/drop discards it. An owned async mutex guard
serializes fake transactions; this does not prove PostgreSQL isolation or lock behaviour.

`testkit` depends on `kernel`/`admin`, never on infrastructure. Infra test runners depend on
testkit and supply their own adapters/migrations. Admin mock/fake application suites live in
testkit; pure domain tests remain in admin. This avoids context↔testkit cycles.

**Rationale:** Separate repository hash maps cannot roll back a cross-repository change.
SQLx `Transaction` is not a dyn `Executor`; queries use its underlying connection. Repositories
must not acquire another connection or commit caller-owned work. A failed commit may have an
unknown outcome: resolve it by operation identity rather than claiming rollback.

**Alternatives considered:** Dyn SQLx executors (not object-safe); associated backend types
through every service (unnecessary generic coupling); nontransactional fakes (incorrect).

**Evidence:**
- https://docs.rs/sqlx/0.9.0/sqlx/struct.Pool.html#method.begin
- https://docs.rs/sqlx/0.9.0/sqlx/struct.Transaction.html
- https://docs.rs/sqlx/0.9.0/sqlx/trait.Executor.html

## R05 — Revision identity, approval and publication

**Decision:** Each change targets one setting or flag. Keep immutable revision IDs plus a
SHA-256 `version_hash` of canonical typed content, and a mutable draft-head revision number
for compare-and-set saves. Approval binds revision, reviewed preview and the affected live
version ID. Hash inputs have fixed field ordering, sorted sets and UTC instants, not Rust's
default hash/map order. Reverting a value never resurrects an old approval.

Compare live version identity, not only value: 30→7→30 is still an intervening change.
Unrelated subjects do not invalidate approval. Rollback creates a fresh draft/review/version.
Lock draft head → publication request → target live row consistently during finalisation.

**Rationale:** Implements AC-ADM-014–020/044 without whole-site approval invalidation or
silent overwrites. Explicit single-subject changes avoid an unrequested batch-edit abstraction.

**Alternatives considered:** Value-only comparisons (ABA bug); mutable approved records
(approval loses meaning); global config version (unrelated edits break schedules).

**Evidence:** https://www.postgresql.org/docs/16/explicit-locking.html

## R06 — Durable scheduling and atomic effects

**Decision:** At-least-once worker attempts, exactly-once committed effects. Persist one
publication identity per approved execution, unique subject-version and success-audit
identities, payload identity checks, lease/fencing information and blocked reasons.
Claim due work briefly with `FOR UPDATE SKIP LOCKED`; gather impact/readiness evidence
outside locks; finalise under one UoW after rechecking revision, approval, baseline, due
time and prerequisites. Append history, advance live pointer, complete request and append
audit together. A stale lease cannot finalise. No adapter calls another adapter.

On service recovery, overdue requests publish once if current gates pass; otherwise block
visibly. Preserve authorised scheduled time and actual application time. Never backdate live
effect; transaction visibility occurs at commit. Application timestamps are not advertised
as exact measured PostgreSQL commit timestamps. An unknown commit outcome is looked up by
the same publication ID before retrying.

**Rationale:** Covers AC-ADM-016–018/025/044–045 and the three accepted clarification answers.

**Alternatives considered:** In-process timers (lost at restart); holding locks across HTTP
calls (contention); exactly-once execution claims (false); `SKIP LOCKED` for live-value checks
(could omit a conflict).

**Evidence:**
- https://www.postgresql.org/docs/16/sql-select.html#SQL-FOR-UPDATE-SHARE
- https://www.postgresql.org/docs/16/functions-datetime.html#FUNCTIONS-DATETIME-CURRENT

## R07 — Runtime append-only audit

**Decision:** Separate migration/owner and non-owner runtime roles. Runtime audit access is
SELECT/INSERT only; revoke UPDATE/DELETE/TRUNCATE/TRIGGER and unintended PUBLIC/inherited
rights. Protect UPDATE/DELETE and statement-level TRUNCATE with guards; runtime cannot
disable guards, alter/drop objects or assume ownership. Audit FKs cannot cascade mutations.
Successful access issuance/recovery/publication and their audit evidence share a UoW.

Use typed, allowlisted event fields and managed reason codes with bilingual explanations;
do not serialize arbitrary requests, credential snapshots, vendor errors or free-text secrets
into audit. Human reason selection supplies the required why without an unrestricted
secret-bearing log channel. Diagnostic logs contain IDs/action/result only.

**Rationale:** A missing UI edit button is not database enforcement. Test permission denial
using the real runtime role and trigger defence independently using a suitably privileged
non-owner fixture role. Operators/superusers remain outside the runtime immutability threat
boundary; a same-database hash chain does not change that.

**Alternatives considered:** Application-only immutability; runtime as owner; a hash chain
claimed to resist the database operator.

**Evidence:**
- https://www.postgresql.org/docs/16/sql-grant.html
- https://www.postgresql.org/docs/16/sql-createtrigger.html
- https://www.postgresql.org/docs/16/sql-altertable.html

## R08 — Impact/readiness without future-feature stubs

**Decision:** Return typed known/unknown impact and delivered/pass/fail/unknown readiness,
including checked revision, live baseline, policy identity, source and assessment time.
Pure foundation policies can explain new-upload-only and preservation rules now. Absent
population providers return unknown, never zero; unbuilt OFF capabilities report not
delivered, never healthy. Actual lifetime counts arrive with owning contexts. Unknown or
unverifiable prerequisites block publication. Recheck evidence on publication; materially
changed destructive impact requires fresh review, not an invented tolerance/health TTL.

**Rationale:** Positive-count scenarios use honest test fixtures; production does not import
testkit or manufacture counts. Registering six OFF flags does not deliver six capabilities.
Existing default values and owner-approved ranges/lowering rules are copied exactly.

**Alternatives considered:** Fake healthy checks in production; inferring an empty population
from a missing table/provider; implementing future media/jobs to make this foundation look complete.

## R09 — HTML-first authentication and OpenAPI bootstrap

**Decision:** Normal HTML/form routes for console actions, OTP/TOTP and passkey begin/finish.
A small local essential bridge calls `navigator.credentials`; escaped hidden inputs carry
public WebAuthn options/results, never unsafe executable interpolation. TOTP is the no-JS
path for admins with that fallback enrolled; no automatic enrollment or second-factor bypass.
Optional htmx is not required for any management task.

The initial `OpenApiRouter` builder is populated with `GET /api/v1/health` and
`GET /api/v1/openapi.json`. Both are annotated/documented; the schema-serving route is not
an exception. Register routes, split router/document once, then attach the frozen document
to state. A local `OpenApiDocument` schema bridge models document shape without embedding
the document into itself. Export uses that same offline builder without connecting to services.

Swagger lives at `/api/docs`, uses local vendored assets, points to the independently
registered schema route and disables external validation. Protect docs, assets, redirects
and schema with admin auth in production. Do not use a helper that mounts a second
undocumented `/api/openapi.json` route. All JSON errors use RFC 9457 problem+json.

**Rationale:** No JSON auth API consumer exists. HTML forms need fewer routes and keep
validation/navigation progressive. This resolves the implementation-pack sample schema URL
conflict in favour of constitution Principle VI.

**Alternatives considered:** A separate JSON auth API now; schema route outside `/api/v1`;
CDN Swagger assets; passkeys claimed to work without essential browser scripting.

**Evidence:**
- https://developer.mozilla.org/en-US/docs/Web/API/Web_Authentication_API
- https://docs.rs/utoipa-axum/0.2.0/utoipa_axum/router/struct.OpenApiRouter.html
- https://docs.rs/utoipa-swagger-ui/9.0.2/utoipa_swagger_ui/struct.Config.html

## R10 — Nextest fixture lifetime

**Decision — owner supplied on 2026-11-04:** Share one detached, labelled PostgreSQL 16 and
MinIO fixture pair per nextest integration run, reusing the pair locally. An idempotent setup
script starts/reuses services, waits for readiness and writes `DATABASE_URL`/`S3_ENDPOINT`
to `$NEXTEST_ENV`. Enable top-level `experimental = ["setup-scripts"]`; define the script
under `[scripts.setup.integration-services]` and select it with
`[[profile.integration.scripts]]`, filtered to integration tests. Pin cargo-nextest **0.9.151**
in CI and developer toolchain documentation; verified available, not yanked, Rust ≥1.91.

Use `#[sqlx::test]` with the canonical infra-postgres workspace migrator and a fresh database
per test, not schema-per-test isolation. SQLx attempts successful database cleanup and retains
failed databases until the same test path is rerun or fixture storage is removed. Extra
application pools must close before cleanup. MinIO fixtures use a unique bucket
per test and delete it on ordinary teardown. Test fixture roles/cluster-global setup are
created once idempotently; database-local migrations run for each test. Real repositories
still connect through restricted runtime roles, never the SQLx fixture superuser pool.

CI removes only containers carrying its exact run labels in an always-run post-step.
Local fixtures persist between runs until `just test-down`. Use loopback dynamic ports,
isolated ownership labels and a setup lock so concurrent invocations cannot start duplicates
or attach to production storage. A live-run lease additionally serializes independent local
runs sharing SQLx's deterministic database-name namespace. No child performs a broad cleanup
sweep. Endpoints exported through `$NEXTEST_ENV` must not expose
passwords/keys: nextest can include their values in JUnit properties. Keep fixture secrets
in protected configuration rather than those endpoint values or stdout.

Without `DATABASE_URL`, plain Cargo/IDE runs use a testkit-owned, process-scoped Testcontainers
fallback and the same test bodies. SQLx reads the URL before a pool-argument test body runs;
the fallback must therefore be established by a test-process launcher, not inside that body
and not by unsafe global environment mutation. Detailed execution/cleanup contracts are in
[contracts/integration-fixtures.md](contracts/integration-fixtures.md).

**Rationale:** This explicit owner decision supersedes both the initial per-compiled-binary
wording and the proposed per-nextest-process primary model. Sharing service processes is
compatible with nextest process isolation when database/bucket isolation belongs to each
test. Do not claim a process-local singleton spans the whole nextest run.

**Alternatives considered:** Per-process containers as the main path (owner chose shared
run services); literal per-binary supervisor; shared schema with reset races; setting
DATABASE_URL inside a SQLx test body; hiding cases inside a single mega-test.

**Evidence:**
- https://nexte.st/docs/design/how-it-works/
- https://nexte.st/docs/configuration/setup-scripts/
- https://crates.io/api/v1/crates/cargo-nextest/0.9.151
- https://docs.rs/sqlx/0.9.0/sqlx/attr.test.html
- https://github.com/launchbadge/sqlx/blob/v0.9.0/sqlx-postgres/src/testing/mod.rs
- https://doc.rust-lang.org/cargo/reference/config.html#targettriplerunner

## R11 — Toolchain, CI and release honesty

**Decision:** Rust stable 1.98.1/edition 2024 is available locally; pin that toolchain and
commit Cargo.lock during implementation. Root lints inherit unsafe_code=forbid and clippy
pedantic=warn; CI uses `-D warnings`. PostgreSQL 16 and MinIO are local compose services;
object-storage behaviour is not part of this feature. Add only members that have real tests
and responsibilities. Preserve ≥90% coverage for kernel and for wallet/generation/identity
as those crates are delivered. Future-crate absence is not a passing coverage result.

Build full-release gates now, with an explicit feature/launch readiness distinction:
unbuilt J1–J5/contexts must block a full-product release, not be replaced by empty tests.
Feature 001 browser tests prove the foundation slice of J5, not its pricing/incident steps.
Nightly mutation runs wallet/generation when present; no unapproved 85% threshold is imposed.
The pack's proposed ≥80% elsewhere, ≥85% mutation score and PRD proposed latency/availability
targets remain reported proposals until approved. No new binding performance constants.

Locally rustc/cargo 1.98.1, Docker 29.9.0 and Node 26.11.1 are installed. nextest, llvm-cov,
mutants, cargo-deny and sqlx-cli are not installed. No Cargo workspace exists yet, so
application tests/builds cannot truthfully have run in this planning command.

**Alternatives considered:** Empty context crates/green skipped journeys; lowering the
coverage floor for convenience; treating proposed numeric targets as owner-approved rules.

## R12 — Authentication protocols and release compatibility evidence

**Decision:** Use the owner-selected approved-template protocols, never live vendor calls
from tests. Kavenegar receives a POST form to
`https://api.kavenegar.com/v1/{API-KEY}/verify/lookup.json` with `receptor`, `token`,
`template`, `type=sms`. SMS.ir receives a POST JSON request to
`https://api.sms.ir/v1/send/verify` with sensitive `X-API-KEY` and `mobile`, `templateId`,
`parameters` containing the configured OTP variable name (without template delimiters).
Production documentation, not SMS.ir's inconsistent sandbox GET example, defines the method.

Provider acceptance is not delivery or primary authentication. Tankhor owns six-digit OTP
generation, keyed digest verification, expiry, abuse limits and atomic single-use consumption.
Failover reuses the same code/challenge/expiry: one bounded primary attempt then at most one
secondary attempt for eligible transport/provider failures. Invalid input does not trigger
blind retry; queued delivery alone does not trigger failover. No exactly-once vendor-send
guarantee is assumed. User-requested replacement invalidates the previous challenge.

Kavenegar puts its API key in the URL path and echoes sensitive data in responses. Map HTTP
errors to allowlisted categories without formatting raw errors/URLs/bodies; suppress automatic
request tracing, mark SMS.ir's key header sensitive, and disable redirects. Neither provider's
raw response, phone number nor supplied code enters tracing/audit. Templates and credentials
must be approved/installed before live acceptance; send acceptance cannot be labelled delivery.

Use `webauthn-rs 0.6.2` safe start/finish passkey APIs with user verification required. Persist
full ceremony state server-side with `danger-allow-state-serialisation` only for that storage;
never send serialized state to clients. Bind it to admin, browser/pre-auth state, purpose,
primary proof, expiry, epoch and credential revision. No `danger-credential-internals` feature.
Apply library counter/backup-property updates; valid zero counters are not rejected. Recheck
epoch/revision and atomically consume ceremony, update credential, issue session and audit.

Use `totp-rs 6.0.0`; its matched timestep is required to exceed the credential's previously
accepted step, including enrollment confirmation. RFC 6238 baseline: SHA1, six digits, 30-second
steps, one-step skew, independent 160-bit secret. Encrypt shared secrets with AES-256-GCM
(`aes-gcm 0.11.1`), unique nonces and associated admin/credential/purpose/epoch data; keys are
supplied by app outside the database. Hash-only TOTP storage cannot verify future codes.
Recovery increments the target's security epoch and atomically revokes all sessions, factors,
outstanding proofs and ceremonies, then authorises restricted fresh enrollment—not access.

**Rationale:** Concrete protocols resolve the required production adapter design; secrets,
account access and approved templates are deployment prerequisites, not missing design choices.
OTP/TOTP/WebAuthn replay defence is application persistence behaviour, not a library promise.

**Alternatives considered:** Password login; a fake primary verifier; trusting SMS acceptance;
two independent failover codes; OTP plaintext outbox; browser-stored ceremony state; TOTP
without accepted-step tracking; logging generic reqwest errors containing provider secrets.

**Evidence:**
- https://kavenegar.com/rest.html#basic-intro
- https://kavenegar.com/rest.html#sms-Lookup
- https://kavenegar.com/sdk.html#curl
- https://sms.ir/rest-api/
- https://docs.rs/webauthn-rs/0.6.2/webauthn_rs/struct.Webauthn.html
- https://docs.rs/webauthn-rs/0.6.2/webauthn_rs/#allow-serialising-registration-and-authentication-state
- https://docs.rs/totp-rs/6.0.0/totp_rs/struct.Totp.html#method.check
- https://datatracker.ietf.org/doc/html/rfc6238#section-5.2
- https://docs.rs/aes-gcm/0.11.1/aes_gcm/

## R13 — Verified dependency family and browser evidence

**Decision:** Retain the implementation pack's compatible release family. Metadata verified
available/not-yanked releases are axum 0.8.9, askama 0.16.0 (0.16.1 compatible patch available),
askama_web 0.16.0 with `axum-0.8`, SQLx 0.9.0, utoipa 5.5.0, utoipa-axum 0.2.0,
utoipa-swagger-ui 9.0.2 with `axum,vendored`, mockall 0.15.0 and testcontainers-modules 0.15.0
with `postgres,minio` and compatible core 0.27.x (0.27.3 verified). SQLx requires Rust ≥1.94;
the selected 1.98.1 satisfies it. Enable SQLx checked-query macros explicitly if defaults
are disabled. Use tokio 1.x, Fluent bundles, proptest, scraper, insta, wiremock and tracing;
resolve compatible patches into Cargo.lock during the first test-driven workspace build.

### Dependency policy exceptions

Owner approval: **2026-11-06**; implemented policy is in `deny.toml`. Re-check all entries
at **every `cargo update`** using cargo-deny and the metadata architecture checker.

| Exception / affected crate or policy | Why accepted / scope | Approval | Review trigger and evidence |
| --- | --- | --- | --- |
| RUSTSEC-2026-0440 · `astral-tokio-tar` 0.6.4 | Unmaintained transitive fixture tooling, not a vulnerability waiver; no patched version. Testcontainers core 0.27.3 and 0.28.0 require it; modules 0.15.0 requires the approved 0.27.x core family. | Owner, 2026-11-06 | Upstream fix or compatible maintained replacement available; re-check at every `cargo update`. `cargo tree -i astral-tokio-tar` confirms only Testcontainers → testkit paths. |
| `tonic` → `axum` wrapper | Allow tonic's internal HTTP dependency for Testcontainers' Bollard Docker client, without relaxing kernel/context transitive I/O bans. | Owner, 2026-11-06 | Re-check at every `cargo update` or Docker transport change. `cargo tree -i axum` observed axum 0.8.9 ← tonic 0.14.9 ← bollard/bollard-buildkit-proto/tonic-prost ← testcontainers 0.27.3 ← testkit, directly or through modules 0.15.0. No kernel/context root reaches axum: `cargo tree -p kernel --all-features` lists kernel only; context crates are not present yet. Repeat inverse-tree and full architecture validation as crates arrive. |
| Licence allowlist | MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-2-Clause, BSD-3-Clause, ISC, Unicode-3.0, Zlib, CC0-1.0, 0BSD, MPL-2.0, CDLA-Permissive-2.0 cover the approved graph. No per-crate licence exceptions (`exceptions = []`); confidence threshold 0.8; unpublished private workspace crates ignored. | Owner, 2026-11-06 | Re-check at every `cargo update`, dependency addition or licence change; all other identifiers remain denied. |

Newer utoipa 6/utoipa-axum 0.3/Swagger 10 and Testcontainers core 0.28 exist but are not mixed
into this family without an intentional compatibility change. Metadata verification is not
a successful compile/run claim.

Use Playwright with axe as test-only tooling. WebKit emulation is not actual iOS Safari;
real iOS Safari and Android Chrome release checks remain necessary. Nextest `default` is
built-in; configure `[profile.default]`, `[profile.integration]`, `[profile.ci]` and explicit
test filters. Merely naming a profile does not classify integration tests. The owner's
shared-run setup uses pinned cargo-nextest 0.9.151 and the current `scripts.setup` syntax.
SQLx CLI 0.9 documents explicit `SQLX_OFFLINE=true` for ordinary offline builds and
`cargo sqlx prepare --check --workspace`; do not rely on the pack's "always on" shorthand.

**Rationale:** Preserve the approved stack while recording verified separate crate versions
and features. Avoid dependency-major drift and false browser/fixture claims.

**Alternatives considered:** Unverified newest-everything upgrades; treating WebKit emulation
as physical iOS coverage; interpreting execution profiles as Cargo compilation profiles.

**Evidence:**
- https://crates.io/api/v1/crates/axum/0.8.9
- https://docs.rs/askama_web/0.16.0/askama_web/
- https://crates.io/api/v1/crates/sqlx/0.9.0
- https://crates.io/api/v1/crates/utoipa/5.5.0
- https://crates.io/api/v1/crates/utoipa-axum/0.2.0/dependencies
- https://docs.rs/utoipa-swagger-ui/9.0.2/utoipa_swagger_ui/
- https://docs.rs/mockall/0.15.0/mockall/#async-traits
- https://crates.io/api/v1/crates/testcontainers-modules/0.15.0/dependencies
- https://nexte.st/docs/configuration/
- https://docs.rs/crate/sqlx-cli/0.9.0
- https://playwright.dev/docs/browsers#webkit

## R14 — Design tokens: single TOML source + build.rs generation

**Decision:** Keep every visual identity value (light and dark colours including
`borderControl`, typography, spacing, radii, motion 180/500 ms) in
`crates/web/design/tokens.toml`, with values from Technical Specification §12.
`crates/web/build.rs` generates the CSS custom properties; templates and CSS use only those
properties. Brand assets are replaceable files under `crates/web/static/brand/`.

**Rationale:** A brand change touches one file (PRD FR-UX-20). Declared text/background pairs
are read from the same file, so WCAG 2.2 AA contrast is checked automatically in unit tests
rather than by manual review (FR-UX-20, FR-UX-07).

**Alternatives considered:** Hand-written CSS variables, which let values drift between the
spec, CSS and contrast checks; runtime JSON, which adds a load-time dependency and moves
missing-token and contrast failures from build/test time to runtime.

**Evidence:** PRD FR-UX-07, FR-UX-20, §10.5; Technical Specification §12.2–§12.4.

## Phase 0 conclusion

All design unknowns are resolved. No Q01–Q41 decision or approved setting default is changed.
The owner-selected SMS provider pair is explicit. Required secrets, templates, initial-admin
authorisation, trusted local TLS and installed developer tools remain setup/deployment
prerequisites, not fabricated available integrations. Phase 1 may proceed. Actual compatibility,
contracts and acceptance are proven only by the subsequent failing-test-first implementation.
