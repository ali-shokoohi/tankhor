# Application and Port Contracts — Feature 001

Design contracts, not implementation. Trace: PRD FR-CON-01–11/32/37, FR-UX-01–09/14/17;
`spec.md` AC-ADM-001–047, FR-001–038; [data-model.md](../data-model.md).

## 1. Boundary and type rules

All I/O-capable ports are dyn-compatible async traits with owned domain DTOs/errors.
`#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]` precedes
`#[async_trait::async_trait]`. Async kernel Clock/IdGenerator and UoW ports follow the same
mocking convention; domain transitions accept explicit time/IDs and remain synchronous/pure.

Kernel traits live in `crates/kernel/src/ports/`; admin traits live in
`crates/admin/src/ports/`. Every port has a mockall mock, generic contract suite in
`crates/testkit/src/contracts/`, fake in `crates/testkit/src/fakes/` behind `fakes`, and
real infra-* adapter. Fake contracts pass before real-adapter work; application mock/fake
tests precede the service implementation. App is configuration/composition only. Normal
release graphs contain neither `mocks` nor `fakes`; production rejects any fake selection.

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
ID/token sources, secret keys and provider order. Impact/readiness are pure domain calls. No infra
adapter calls another infra adapter. Crypto/storage/vendor cooperation is application logic.

One application-wide `TANKHOR_ADAPTERS=real|fake` selects a coherent backend; per-group
selection/mixing is unsupported. Default is real; production rejects fake mode before resource
acquisition and release dependency graphs exclude mocks/fakes features.

## 2. Application service surface

| Services | Input and success result | Required rules and failures |
| --- | --- | --- |
| RequestAdminOtp / VerifyAdminOtp | Browser-bound phone input/challenge and code → restricted primary proof | Eligible provisioned/invited admins only; no customer account creation; normalization; local send/verify limits; expiry/supersession/single-use; generic non-enumerating request response |
| Begin/FinishPasskey / Begin/FinishTotp | Primary proof, ceremony/assertion or TOTP → audited full admin session | RP/origin/purpose/browser/epoch binding; current credential revision; TOTP step CAS; no full access from factor transport acceptance |
| InviteAdmin / BeginEnrollment / FinishEnrollment | Fully verified authorizing actor and target identity; target's phone-OTP primary proof → confirmed factor | Another admin records protected enrollment authorization, except initial operator provisioning; target-matching authorization and bound one-use ceremony are rechecked at begin/finish; authorization alone is not console access |
| BootstrapAdmin | Explicit owner-authorised operator context and initial identity → protected eligibility/enrollment authorization | Owns the existing first-admin provisioning precondition, not recovery or public registration; reject repeated/unauthorised bootstrap; one UoW for eligibility/authorization/audit, no factor or full session created |
| GetEnrollment | Restricted primary-proof reference and browser context → safe enrollment-page DTO | Read-only query of SessionStore proof and AdminRepository target authorization; validate stage, purpose, target, current epoch, browser binding, finite expiry and revocation/consumption; no secret/ceremony payload or console access |
| RecoverAdmin | Verified other admin, target, reason → protected re-enrollment authorization | One UoW: epoch bump, all target session/factor/proof/OTP revocation, authorization replacement and audit; target must verify phone OTP and enrol afresh; other admins unchanged |
| VerifyConsoleAccess / SignOut | Opaque cookie → current actor context, or session revocation | Authoritative session/epoch/expiry check per request; no end-user cookie substitution; mutating use cases recheck actor validity before commit |
| PrepareAccessForm | Browser context → bound presentation/CSRF capability | Capability generation uses AdminAuthenticator; never grants primary or console access |
| ListAdmins | Verified actor, bounded cursor → safe actor/enrollment DTOs | Authorize before one bounded list read; no phone/credential material or writes |
| ListSettings / ListFlags / GetSetting / GetVersion | Verified actor and key/IDs → owned presentation DTOs | Each query has its own mock/fake driver; complete metadata/history, missing/unknown ID errors, authorization before reads and no writes |
| GetChange | Verified actor and exact ChangeId → current head/review/baseline DTO | Mock: one exact-ID read after authorization, zero writes; fake: correct draft, missing ID, isolated subjects and stale-head visibility |
| GetPublication | Verified actor and exact PublicationId → pending/blocked/cancelled/completed DTO | Exact-ID authorized read, zero writes; include scheduling admin/approver and visible block/cancellation reason, scheduled/actual time and rescheduling action; no secret details |
| SaveDraft | Verified actor, subject/value/reason/time, expected head → new immutable revision | Exact bounds/formats; CAS conflicts; draft only; edited timing/content invalidates old approval |
| Preview | Verified actor, exact revision → persisted report | Current subject baseline, known/unknown impact, provenance, readiness and policy; no downstream side effects |
| Approve | Verified actor, exact revision and preview → approval ID | Match current head/baseline/review; missing required information cannot become approved; self-approval allowed as distinct steps |
| Publish / SchedulePublish | Verified actor, approval, stable request identity → publication result | Revalidate current prerequisites; final state/history/audit atomic; payload identity conflict; no duplicate success |
| Rollback | Verified actor, historic version/reason → new draft | Ordinary preview/approval/publication follows; never erase prior history or undo executed downstream transactions |
| ScheduledPublish | Clock instant, bounded due-candidate batch → completed/blocked/cancelled results | Execute on behalf of stored scheduling admin; recheck active eligibility, captured security epoch and current approval validity; revoked/recovered/deactivated admin causes terminal audited cancellation requiring another admin to reschedule; otherwise existing baseline/impact/health gates; short row locking, one effect and no backdating |
| SearchAudit | Verified actor, filters and cursor → page and next cursor | Actor/action/subject/time filtering, stable bounded ordering, append-only evidence |
| SeedFoundation | System operation identity → seeded/already applied | Exact defaults and six OFF flags; no overwrite of edited values; no admin credentials seeded |
| GetHealth | No admin grant needed; storage readiness → minimal ready/unavailable result | Application-level read of required seed/storage availability; web maps to sanitized JSON; no provider key or infrastructure detail |

Authentication issuance, recovery, draft/preview/approval/scheduling/publication/rollback and
seed mutations require audit in the same UoW. No successful unaudited mutation is returned.
SMS calls and expensive crypto happen outside DB locks; consume/update proof state under
locks/CAS before issuing a session. Audit events use internal actor IDs and managed reasons.

BootstrapAdmin is an admin application service driven by mock/fake tests before real storage
work. It uses the existing AdminRepository, AuditRepository, UnitOfWork,
Clock, IdGenerator and AdminAuthenticator ports; no new port or public bootstrap endpoint.
The CLI only parses protected input/configuration, constructs the authorised operator context
and delegates. It never performs eligibility, repository, authorization or audit orchestration itself.

There is no transferable enrollment token, delivery channel or grant lifecycle. InviteAdmin,
RecoverAdmin and BootstrapAdmin persist authorization on the target admin: purpose, authorizer,
current epoch, authorized/expiry times and pending/consumed/revoked status. Only the authenticated
target, identified by verified phone OTP, can use it. FinishEnrollment conditionally consumes
the authorization and SessionStore proofs with credential/session/audit writes in one UoW.
Concurrent attempts cannot enroll twice; expiry at or before now is refused.

GetEnrollment resolves purpose-separated primary-reference/browser digests through
AdminAuthenticator, reads proof via SessionStore and protected authorization/current target
eligibility/epoch via AdminRepository in one read-only UnitOfWork, using Clock for expiry.
Validate target, purpose, primary-verified stage, epoch, browser/session binding, finite expiry
and pending/unrevoked/unconsumed state. Authorization is identity-bound, not browser-bound;
the primary proof and ceremony are browser/session-bound. Return safe target reference and
allowed factor choices only, never phone/credential/crypto payloads. Mock/fake tests assert
the reads and every refusal with zero writes, consumption, extension or ID/token/session
issuance. Begin/finish independently recheck; CSRF remains mandatory on mutating forms.

Constitution v1.2.0 permits one UI-secret exception: a TOTP secret/QR shown exactly once to
the admin performing authenticated enrollment (matching phone-OTP proof, protected target
authorization and bound SessionStore ceremony), with `Cache-Control: no-store`. SessionStore
conditionally marks provisioning presented before the response; repeats/concurrent losers
and GetEnrollment cannot retrieve it again. No logging, redisplay, other delivery, client
bundle or other UI secret is permitted. A lost response requires a fresh superseding ceremony.
The presentation gate retains opaque protected verifier state for confirmation without any
redisplay path. Application drivers test one-use result/policy; HTTP drivers test the header.

SchedulePublish persists scheduling_admin_id, captured scheduling_security_epoch and the
approver_id from the exact approval. ScheduledPublish uses the trusted system-worker context,
not a browser session for the scheduling admin. Lock/re-read that admin before configuration
locks and compare active eligibility and epoch; any mismatch with the captured scheduling
epoch cancels even after reactivation/restoration or reenrollment before execution.
Invalid authority cancels before other prerequisite decisions.
With valid authority, invalid approval still blocks through existing review rules. Completion
and cancellation audit actor = scheduling admin, executed_by = system_worker, approver recorded.
Cancellation reason is visible, has no live effect/success-publication event and commits with
one uniquely keyed audit or not at all. Retries return the terminal cancellation unchanged.
Another verified active admin reschedules via the existing workflow and a new request
referencing the cancelled ID; reject the original scheduler's reschedule, preserve history,
and retain exact-review/time-edit rules. No new endpoint or admin-deactivation service.

Every committed revocation, deactivation or recovery of an admin atomically advances that
admin's durable, monotonic security epoch through existing AdminRepository/UoW operations.
Reactivation/restoration retains the current epoch or advances it, never resets it or reuses
an older value. Target-row lock/expected-epoch CAS serializes invalidation with ScheduledPublish;
reject stale CAS and non-increasing invalidation writes, roll back
transition and epoch together, and fail closed on overflow rather than wrapping. The shared
fake/real admin repository suite checks committed deactivate→reactivate and revoke→restore
with fresh reads retaining the advanced epoch, recovery advancement and unchanged-epoch
reads, before any worker is required. Existing ScheduledPublish drivers then require both
restoration sequences to cancel with one visible-reason audit/no live effect; unchanged epoch
executes only with active eligibility and all existing approval/publication gates satisfied.

Time values are pure: console local date+time in a display zone (default Asia/Tehran),
unambiguous resolution to UTC plus retained zone, rejection of nonexistent/ambiguous local
times and newly submitted past schedules. JSON decoding requires ISO 8601 with explicit
Z or numeric offset (for example ±HH:mm). Pinned embedded IANA rules and explicit Clock values avoid context I/O.
Test Tehran and Europe/Berlin gap/overlap boundaries; immediate intent and already-authorized
overdue execution retain their existing semantics. No management JSON endpoint is introduced.

## 3. Persistence ports and shared suites

All real persistence adapters below live in infra-postgres; all fakes share one transactional
testkit store. Each named suite is executed unchanged against both implementations.

| Port | Minimum operations | Fake/real contract suite |
| --- | --- | --- |
| UnitOfWork | begin, opaque TxHandle, consuming commit/rollback | `unit_of_work_contract`: commit/rollback/drop, wrong handle/store, failure atomicity and retry identity; TxHandle is a carrier, not another dependency port |
| SettingsRepository | list definitions; get live/history; lock subject; append version; CAS live pointer | `settings_repository_contract`: typed bounds, matching version FK, immutable history, no lost update |
| FlagRepository | list/get live/history; lock; append version; CAS pointer | `flag_repository_contract`: six OFF seeds, boolean values/history, matching subject and no silent enablement |
| ChangeRepository | get head/revision; append revision with expected head; persist preview/approval; read exact review | `change_repository_contract`: stale save, immutable revisions, timing edits, approval/baseline identity |
| PublicationRepository | create/retry idempotent request with scheduling admin/epoch/approver and optional cancelled predecessor; list bounded pending/retryable-blocked due candidates; lock/get result; conditionally complete/block/cancel expected state; resolve outcome | `publication_repository_contract`: identity/payload conflicts, authority and approval attribution round-trip, cancellation terminality/audit rollback/once-only result, another-admin reschedule linkage, due timing, guarded blocked retry, stale-state rejection and concurrent one effect |
| AuditRepository | append typed event; search filtered cursor pages | `audit_repository_contract`: event uniqueness, ordering/filters, no edit/delete API, transactional append failure |
| AdminRepository | keyed identity lookup; bounded safe list; get/lock; invite; epoch CAS atomically with invalidating admin state; record/read/conditionally consume/revoke target enrollment authorization | `admin_repository_contract`: identity uniqueness, safe listing, another-admin authorization, target/purpose/expiry/epoch/stage matching, single-use races and recovery replacement; every committed admin revocation/deactivation/recovery durably advances epoch, restoration never resets/reuses it; deactivate→reactivate/revoke→restore round-trips, unchanged-epoch control, stale CAS and rollback |
| AdminCredentialRepository | list active; insert; get/lock; revision CAS; accepted-step CAS; revoke all | `admin_credential_repository_contract`: credential identity uniqueness, stale snapshot/replay rejection and revocation |
| SessionStore | insert/lookup/check/revoke sessions; create/read/conditionally consume/supersede/revoke primary proofs and ceremonies; conditionally mark TOTP provisioning presented once | `session_store_contract`: stage/target/browser/session/purpose/epoch/expiry, credential revision, one-use/presentation races, no redisplay, revocation, encrypted state and digest-only references |
| OtpChallengeRepository | create; supersede predecessor; increment failed attempts; consume once; record safe submission category | `otp_challenge_repository_contract`: keyed digest only, original expiry across failover, replay and concurrent consumption |
| RateLimitRepository | atomic per-subject/origin bucket increment/read | `rate_limit_repository_contract`: limits persist across provider failover, resend and concurrent requests |
| HealthProbe | sanitized migrated-store and actual-seed readiness | `health_probe_contract`: missing schema/unseeded/unavailable fail closed; genuine seeded store is ready |

Repository fakes serialize transactions; real PostgreSQL tests additionally prove row locks,
CAS, restricted privileges, triggers and simultaneous worker/HTTP execution.
No mocked SQLx/reqwest types. Fake errors must use the same domain taxonomy as real adapters.

Retry after unavailable evidence retains the original request ID/payload. The existing
request/retry operation may conditionally reopen a retryable blocked row under lock only
after current actor/head/approval/baseline/due/evidence checks permit; identical recovered
evidence does not require fresh confirmation. Semantic mismatch still needs renewed review.
Completed/terminal state never reopens; blocked reasons stay visible until a checked retry.
Bounded due reads include eligible retryable-blocked work without taking ownership.

## 4. Non-persistence ports

| Port | Production implementation | Fake and same contract evidence |
| --- | --- | --- |
| Clock | `crates/infra-system/src/clock.rs`; UTC instants | `crates/testkit/src/fakes/clock.rs`, behind `fakes`; `clock_contract` on both, without monotonic-wall-clock claims |
| IdGenerator | `crates/infra-system/src/id_generator.rs`; UUIDv7 source | `crates/testkit/src/fakes/id_generator.rs`, behind `fakes`; `id_generator_contract` validates version/type/uniqueness, not equal random outputs |
| AdminAuthenticator | infra-auth WebAuthn/TOTP/OTP-digest/secure-token/secret-envelope adapter | Fixture-bound fake behind `fakes`; `admin_authenticator_contract` on fake and real local crypto fixtures: valid/invalid proof categories, origin/challenge binding, redacted values and matched TOTP step |
| SmsSender | Separate Kavenegar and SMS.ir HTTP adapters | Capturing fake; `sms_sender_contract` on fake and each real adapter against wiremock: approved-template payload, acceptance/error taxonomy, no redirects/leaks |

Clock and ID fixtures do not turn production time/random dependencies into context I/O.
The inventory is **16 ports** (12 persistence including UoW/HealthProbe, four non-persistence).
ImpactAssessment and CapabilityReadiness are pure calculations in admin/domain, not traits,
ports, mocks, fakes or infra adapters. Application services call them directly with typed
evidence and explicit time. The code-owned delivered-capability registry marks all six
capabilities not delivered; synthetic delivered/pass/fail/unknown inputs exercise pure policy
and service tests without adding a production override. No new population I/O is introduced.
Auth crypto fake accepts only matching controlled fixtures, never arbitrary assertions.
Software/virtual authenticators produce real signature vectors for the real implementation.
OTP delivery failover is an application orchestration over two SmsSender instances, not an
adapter calling another adapter. OTP sending is mandatory identity traffic, **not** the
external-notifications feature controlled by an OFF flag.

The pure impact calculation consumes exact revision/current baseline and returns counts only from a
registered authoritative population source. Foundation can report a pure preservation/new-
upload policy without inventing counts; lifetime counts remain unknown until supplied by the
owning feature. Delivery/readiness states are read-only results, never admin-entered overrides.
Fresh evidence is checked at finalisation; changed destructive impact requires renewed review.

### Comparable impact evidence (definition of the existing review gate)

Compare canonical semantic fields, not report IDs or serialized reports:

| Comparable field | Meaning |
| --- | --- |
| Exact revision and typed subject | Revision identity binds proposed value and effective time; setting/flag subjects never alias |
| Live baseline version | Exact version identity, not merely an equal current value (including ABA) |
| Policy and source identities | The governing policy identity and authoritative evidence source identity; neither may be missing when required |
| Existing/new-item effect rules | Original reference kind, preservation/new-item treatment and next-sweep consequence applicable to that setting |
| Applicable impact counts | Exact known `changed_count` and `already_due_count` for lifetime policies requiring population evidence; known zero remains distinct from unavailable |

These are the fields already carried by the review/provenance and policy DTOs, not a new
population query, per-item inventory, capability or requirement. Policies that only report
preservation/new-item rules omit inapplicable counts; unavailable lifetime counts may never
be omitted or converted to zero. `assessed_at`, report IDs and incidental observation IDs
are not comparable semantic fields. A later assessment time alone does not change impact;
there is no time-to-live, tolerance or age-based approval invalidation.

Approval/publication uses this exact decision order:
1. Missing/unavailable required evidence, unknown counts or unverifiable required provenance
   blocks the operation without live writes or a successful publication audit. No new approval
   may be created from that evidence.
2. A different revision, subject, baseline, policy/source identity, effect rule or applicable
   count invalidates eligibility of the reviewed approval: persist/show the appropriate reason
   and require a new preview and approval. Immutable old review records are not rewritten.
3. Otherwise the impact evidence is unchanged, even with a later `assessed_at`; the existing
   exact-version approval remains eligible subject to the other existing checks. A transient
   unavailable result never counts as success; on retry, recovered evidence is compared again
   to the original review under these same rules, not a new TTL or unconditional reapproval.

The same pure comparison drives Approve, immediate Publish and ScheduledPublish. Existing
domain and mock/fake workflow drivers test unchanged impact at a later assessment time,
changed counts/effect/provenance requiring renewed review, and unavailable evidence refusing
success. All former policy-port cases move to those domain drivers, not adapter suites.

## 5. Atomicity, retry and error semantics

- A command's stable operation ID is distinct from a random transport request ID. Reuse with
  the same payload returns the established result; reuse with another payload conflicts.
- Final publication transaction locks draft head, publication request and live subject in
  that order. Security mutations lock involved admin rows in sorted ID order before their
  credential/session/proof children; authenticated mutations validate actor state before
   configuration locks. Due-candidate reads are bounded and non-exclusive: they never remove
   pending work or mark it in progress. Evidence is gathered outside write locks. Finalization
   locks and rereads current state, checks expected pending status/revision/approval/baseline/
   due time and current prerequisites, then conditionally writes one result. Concurrent or
   stale candidates return the committed result or conflict, never overwrite it. Crashes
    before commit leave durable pending work; no persistent lease, claim owner or fence exists.
- Scheduled publication first locks/rechecks scheduling-admin eligibility and captured epoch,
  serializing with recovery/revocation before those configuration locks. If invalid, atomically
  cancel and audit once; never retry or reopen a cancelled request. A prior committed execution
  is not undone by later recovery. Cancellation is not a success-publication event.
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
PostgreSQL services per nextest run, not per-process primary containers. MinIO fixture startup
and bucket lifecycle are deferred to feature 004; Compose retains MinIO, with no 001 storage
behavior coverage. Adapter-hosted
`#[sqlx::test]` cases reference the canonical infra-postgres migrator and get fresh databases;
testkit accepts caller-supplied pools/options without importing infra-postgres. App/service
pools are restricted-runtime pools, not SQLx's database-creation pool. A test-only process
launcher supplies fallback Testcontainers endpoints before SQLx setup for Cargo/configured
IDE runs; no global environment mutation or production fixture wiring is permitted.

Checked-query adapter completion includes metadata preparation against the current migrated
isolated schema: sqlx metadata prepared and offline check passes before green. Final
workspace freshness checking is additional assurance, never deferred initial preparation.
