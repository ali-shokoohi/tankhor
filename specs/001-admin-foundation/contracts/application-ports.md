# Application and Port Contracts — Feature 001

Design contracts, not implementation. Trace: PRD FR-CON-01–11/32/37, FR-UX-01–09/14/17;
`spec.md` AC-ADM-001–045; [data-model.md](../data-model.md).

## 1. Boundary and type rules

All I/O-capable ports are dyn-compatible async traits with owned domain DTOs/errors.
`#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]` precedes
`#[async_trait::async_trait]`. Async kernel Clock/IdGenerator and UoW ports follow the same
mocking convention; domain transitions accept explicit time/IDs and remain synchronous/pure.

No context type mentions SQLx, HTTP requests/responses, reqwest, WebAuthn library structs,
provider errors, database rows or storage SDKs. Opaque protected crypto-state wrappers expose
no secret Debug/Display representation. Persistence methods participating in a use case receive
the same `&mut dyn TxHandle`; repositories never open a replacement connection or commit it.
Read-only services may use a read UoW for consistent snapshots.

`TxHandle` permits adapter-only checked downcasting and store/backend identity checks. A
wrong/finished handle yields a typed error rather than a panic. UoW commit/rollback consume
the transaction. Drop provides safety rollback; explicit rollback is awaited on known aborts.
Unknown commit outcome is distinct from rollback and resolved using a stable operation ID.

Web/worker call application services only. App constructs all real adapters, including clock,
ID/token sources, secret keys, provider order and readiness/impact composition. No infra
adapter calls another infra adapter. Crypto/storage/vendor cooperation is application logic.

## 2. Application service surface

| Services | Input and success result | Required rules and failures |
| --- | --- | --- |
| RequestAdminOtp / VerifyAdminOtp | Browser-bound phone input/challenge and code → restricted primary proof | Eligible provisioned/invited admins only; no customer account creation; normalization; local send/verify limits; expiry/supersession/single-use; generic non-enumerating request response |
| Begin/FinishPasskey / Begin/FinishTotp | Primary proof, ceremony/assertion or TOTP → audited full admin session | RP/origin/purpose/browser/epoch binding; current credential revision; TOTP step CAS; no full access from factor transport acceptance |
| InviteAdmin / BeginEnrollment / FinishEnrollment | Fully verified actor and target; restricted grant; recipient primary proof → confirmed factor | Another-admin authorisation, except initial operator provisioning precondition; no grant directly permits console access |
| RecoverAdmin | Verified other admin, target, reason → restricted re-enrollment grant | One UoW: epoch bump, all target session/factor/proof revocation and audit; target must verify primary and enrol afresh; other admins unchanged |
| VerifyConsoleAccess / SignOut | Opaque cookie → current actor context, or session revocation | Authoritative session/epoch/expiry check per request; no end-user cookie substitution; mutating use cases recheck actor validity before commit |
| ListSettings / ListFlags / GetChange / GetVersion | Verified actor and key/IDs → owned presentation DTOs | Current values/history and complete metadata; no secret-bearing fields |
| SaveDraft | Verified actor, subject/value/reason/time, expected head → new immutable revision | Exact bounds/formats; CAS conflicts; draft only; edited timing/content invalidates old approval |
| Preview | Verified actor, exact revision → persisted report | Current subject baseline, known/unknown impact, provenance, readiness and policy; no downstream side effects |
| Approve | Verified actor, exact revision and preview → approval ID | Match current head/baseline/review; missing required information cannot become approved; self-approval allowed as distinct steps |
| Publish / SchedulePublish | Verified actor, approval, stable request identity → publication result | Revalidate current prerequisites; final state/history/audit atomic; payload identity conflict; no duplicate success |
| Rollback | Verified actor, historic version/reason → new draft | Ordinary preview/approval/publication follows; never erase prior history or undo executed downstream transactions |
| ScheduledPublish | Clock instant, bounded claim batch → completed/blocked results | Due-work lease/fencing; fresh checks; stale live baseline blocks; overdue execution once; actual time, no backdating |
| SearchAudit | Verified actor, filters and cursor → page and next cursor | Actor/action/subject/time filtering, stable bounded ordering, append-only evidence |
| SeedFoundation | System operation identity → seeded/already applied | Exact defaults and six OFF flags; no overwrite of edited values; no admin credentials seeded |
| GetHealth | No admin grant needed; storage readiness → minimal ready/unavailable result | Application-level read of required seed/storage availability; web maps to sanitized JSON; no provider key or infrastructure detail |

Authentication issuance, recovery, draft/preview/approval/scheduling/publication/rollback and
seed mutations require audit in the same UoW. No successful unaudited mutation is returned.
SMS calls and expensive crypto happen outside DB locks; consume/update proof state under
locks/CAS before issuing a session. Audit events use internal actor IDs and managed reasons.

## 3. Persistence ports and shared suites

All real persistence adapters below live in infra-postgres; all fakes share one transactional
testkit store. Each named suite is executed unchanged against both implementations.

| Port | Minimum operations | Fake/real contract suite |
| --- | --- | --- |
| UnitOfWorkFactory / UnitOfWork | begin, handle, consuming commit/rollback | `uow_contract`: commit/rollback/drop, wrong handle/store, failure atomicity and retry identity |
| SettingsRepository | list definitions; get live/history; lock subject; append version; CAS live pointer | `settings_repository_contract`: typed bounds, matching version FK, immutable history, no lost update |
| FlagRepository | list/get live/history; lock; append version; CAS pointer | `flag_repository_contract`: six OFF seeds, boolean values/history, matching subject and no silent enablement |
| ChangeRepository | get head/revision; append revision with expected head; persist preview/approval; read exact review | `change_repository_contract`: stale save, immutable revisions, timing edits, approval/baseline identity |
| PublicationRepository | create idempotent request; claim due; lock/get result; fenced complete/block; resolve outcome | `publication_repository_contract`: payload conflicts, due timing, lease expiry/fencing, one committed result |
| AuditRepository | append typed event; search filtered cursor pages | `audit_repository_contract`: event uniqueness, ordering/filters, no edit/delete API, transactional append failure |
| AdminRepository | lookup by keyed normalized-phone digest; get/lock; invite; compare/update epoch | `admin_repository_contract`: uniqueness, another-admin invitation, target epoch and eligibility |
| CredentialRepository | list active; insert; get/lock; revision CAS; accepted-step CAS; revoke all | `credential_repository_contract`: credential identity uniqueness, stale snapshot/replay rejection and revocation |
| AdminSessionRepository | insert; digest lookup/current access check; revoke one/all | `admin_session_repository_contract`: expiry, epoch, target-only revocation and no plaintext token |
| AuthStateRepository | create/read/consume primary/ceremony/enrollment state; supersede/revoke | `auth_state_repository_contract`: browser/purpose/epoch/expiry binding, single-use races and no raw client state |
| OtpChallengeRepository | create; supersede predecessor; increment failed attempts; consume once; record safe submission category | `otp_repository_contract`: keyed digest only, original expiry across failover, replay and concurrent consumption |
| RateLimitRepository | atomic per-subject/origin bucket increment/read | `rate_limit_repository_contract`: limits persist across provider failover, resend and concurrent requests |

Repository fakes serialize transactions; real PostgreSQL tests additionally prove row locks,
CAS, SKIP LOCKED, restricted privileges, triggers and simultaneous worker/HTTP execution.
No mocked SQLx/reqwest types. Fake errors must use the same domain taxonomy as real adapters.

## 4. Non-persistence ports

| Port | Production implementation | Fake and same contract evidence |
| --- | --- | --- |
| Clock | App SystemClock; UTC instants | Kernel FixedClock; `clock_contract` on both, with explicit time assumptions rather than monotonic-wall-clock claims |
| IdGenerator | App UUIDv7 source | Testkit deterministic UUIDv7 sequence; `id_generator_contract` validates version/type/uniqueness, not equal random outputs |
| AdminAuthenticator | infra-auth WebAuthn/TOTP/OTP-digest/secure-token/secret-envelope adapter | Fixture-bound fake; `authenticator_contract` on fake and real local crypto fixtures: valid/invalid proof categories, origin/challenge binding, redacted values and matched TOTP step |
| SmsSender | Separate Kavenegar and SMS.ir HTTP adapters | Capturing fake; `sms_sender_contract` on fake and each real adapter against wiremock: approved-template payload, acceptance/error taxonomy, no redirects/leaks |
| ImpactAssessment | App-wired foundation policy assessor plus registered population application-query providers | Fixture assessor; `impact_contract` checks revision/baseline/provenance and known/unknown semantics on fake and real policy composition |
| CapabilityReadiness | App capability registry with registered delivered-capability health providers | Fixture registry; `readiness_contract` checks delivered/pass/fail/unknown on fake and real registry; absent six capabilities are explicitly not delivered |

Clock and ID fixtures do not turn production time/random dependencies into context I/O.
Auth crypto fake accepts only matching controlled fixtures, never arbitrary assertions.
Software/virtual authenticators produce real signature vectors for the real implementation.
OTP delivery failover is an application orchestration over two SmsSender instances, not an
adapter calling another adapter. OTP sending is mandatory identity traffic, **not** the
external-notifications feature controlled by an OFF flag.

ImpactAssessment consumes exact revision/current baseline and returns counts only from a
registered authoritative population source. Foundation can report a pure preservation/new-
upload policy without inventing counts; lifetime counts remain unknown until supplied by the
owning feature. Delivery/readiness states are read-only results, never admin-entered overrides.
Fresh evidence is checked at finalisation; changed destructive impact requires renewed review.

## 5. Atomicity, retry and error semantics

- A command's stable operation ID is distinct from a random transport request ID. Reuse with
  the same payload returns the established result; reuse with another payload conflicts.
- Final publication transaction locks draft head, publication request and live subject in
  that order. Security mutations lock involved admin rows in sorted ID order before their
  credential/session/proof children; authenticated mutations validate actor state before
  configuration locks. Due-work claim transactions lock only publication work rows and end
  before finalisation; no reverse-order cycle is introduced.
- Concurrent recovery versus login: epoch/credential/proof/session checks and audit share
  the final UoW. Recovery cannot leave a newly issued old-epoch session usable.
- Errors distinguish validation, unauthorised, forbidden, conflict, missing/failed prerequisite,
  infrastructure unavailable and unknown commit outcome. Presentation cannot infer success
  from timeout, SMS transport acceptance or a disconnected DB response.
- Provider timeout may have delivered a message. Secondary sending reuses the same code and
  expiry; no exactly-once SMS claim. Rate counters are not reset by provider switches.
- Operator security-policy configuration is validated, finite and non-disableable through
  general settings. Missing keys/RP/origin/provider templates/required policy blocks startup
  or the relevant operation, never falls back to testkit or unconditional success.

## 6. Test placement and dependency direction

Pure unit/proptest tests live beside kernel/admin domain code. Async application mock/fake
tests and generic contract functions live in testkit, which depends only on contexts/kernel
and optional test infrastructure libraries. Adapter tests instantiate their own real adapter
and call testkit's generic suite. Testkit never imports infra-postgres/auth/sms; admin never
depends on testkit. Web/worker tests may depend on testkit and test-only real adapter fixtures.
App integration tests prove actual composition without production imports from testkit.

The owner-selected [integration fixture contract](integration-fixtures.md) shares detached
PostgreSQL/MinIO services per nextest run, not per-process primary containers. Adapter-hosted
`#[sqlx::test]` cases reference the canonical infra-postgres migrator and get fresh databases;
testkit accepts caller-supplied pools/options without importing infra-postgres. App/service
pools are restricted-runtime pools, not SQLx's database-creation pool. A test-only process
launcher supplies fallback Testcontainers endpoints before SQLx setup for Cargo/configured
IDE runs; no global environment mutation or production fixture wiring is permitted.
