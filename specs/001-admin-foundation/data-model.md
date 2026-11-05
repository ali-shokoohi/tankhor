# Phase 1 Data Model — Admin Foundation

Design only. Sources: PRD FR-CON-01–11/32/37, FR-UX-01–09/14/17, Q09/Q23;
`spec.md` FR-001–038 and accepted clarification answers. PostgreSQL objects belong exclusively
to infra-postgres; context entities and TxHandle contain no SQLx types.

## 1. Shared values and policy

- UUIDv7 wrappers: AdminId, CredentialId, SessionId, ChallengeId, ChangeId, RevisionId,
  PreviewId, ApprovalId, PublicationId, VersionId, AuditId. Validate UUID version when
  constructing from external values. Opaque browser tokens are random capabilities,
  **not UUIDv7 IDs**; only their digests are persisted.
- Locale: `fa`/`en`; direction derived, never independently editable. Theme:
  light/dark/system. UTC instants plus original scheduled-zone information for display.
- `DomainError`: validation, unauthorised, forbidden, stale revision/baseline, unavailable
  prerequisite/store, invalid transaction handle, safely retryable transaction error and
  commit outcome unknown. Web translates errors; SQL/provider details never cross the port.
- `Setting<T>` has stable key, bilingual label key, unit, typed value, immutable allowed
  policy, live version and history references. Numeric values are exact, never floats.
  Finite bounds are inclusive; integral counts/pixels/day settings use checked integers.
  Upload size is kept in its declared MB unit; downstream byte conversion belongs to media.
- `FormatSet`: JPEG/PNG/WebP/HEIC only, canonical sorted representation. JPEG is mandatory.
  Persistence may use a four-bit mask with JPEG bit 1; valid masks are 1..15 with bit 1 set.
- Change subject: exactly one setting key **or** flag key. No initial batch editor.
  Reasons are managed, bilingual `ReasonCode` values, not unrestricted log-bearing text.

## 2. Exact initial registry

The keys below are stable implementation identifiers; the owner-approved values come from
`spec.md` Initial Settings. Registry policy is not editable through the value-change UI.

| Key | Default | Min | Max | Unit/reference | Trace |
| --- | --- | --- | --- | --- | --- |
| buyer_temp_photo_days | 14 | 1 | 30 | Days from upload | FR-CON-32; R15 |
| studio_temp_upload_days | 30 | 7 | 90 | Days from upload | FR-CON-32; Q06 |
| studio_auto_draft_days | 30 | 1 | 90 | Days from last activity | FR-CON-32; Q32 |
| account_recovery_days | 30 | 14 | 90 | Days from deletion request | FR-CON-32; R26 |
| studio_max_cards | 50 | 1 | 100 | Cards | FR-CON-32; Q15 |
| buyer_max_permanent_photos | 5 | 1 | 10 | Photos | FR-CON-32; Q40 |
| upload_max_mb | 15 | 1 | 25 | MB | FR-CON-32; Q05 |
| upload_min_shortest_edge_px | 512 | 256 | 1024 | Pixels | FR-CON-32; Q05 |
| generation_max_attempts | 3 | 1 | 3 | Total attempts | FR-CON-32; Q29 |
| upload_formats | JPEG, PNG, WebP, HEIC | JPEG required | Four approved formats only | Format subset | FR-CON-32; Q05 |

Flag keys, all initially false: `partner_widget_api`, `percentage_rollout`, `per_user_usage_limits`,
`ticketing`, `external_notifications`, `paid_share_billing`. Names/descriptions reference
managed FA/EN keys. Registry membership, policy constraints and initial values have unit,
proptest, fake/real seed and HTTP tests (AC-ADM-008/009/033/037/038).

## 3. Settings, flags and review storage

| Table/entity | Fields and relationships | Constraints/indexes |
| --- | --- | --- |
| settings | key, label_key, unit, policy identity, min/max or format policy, current_version_id | key PK; fixed initial key/policy checks; non-null live pointer after seed transaction; runtime cannot alter registry policy |
| setting_versions | id, setting_key, numeric_value or format_mask, publication_id, actor_ref, reason_code, applied_at | FK setting; exactly one typed value; per-key CHECK for the approved finite ranges; JPEG mask CHECK; UNIQUE(publication_id, setting_key); immutable |
| flags | key, label_key, description_key, current_version_id | key PK; six fixed keys; live pointer references matching flag version |
| flag_versions | id, flag_key, enabled, publication_id, actor_ref, reason_code, applied_at | FK flag; UNIQUE(publication_id, flag_key); immutable |
| change_drafts | id, setting_key or flag_key, head_revision_id, edit_counter, creator_id, created_at | CHECK exactly one subject; FK admin/subject; CAS on edit_counter; index by subject/status |
| change_revisions | id, change_id, revision_number, typed proposed value, reason_code, scheduled_at, scheduled_zone, version_hash, created_by, created_at | FK draft/admin; UNIQUE(change_id, revision_number); immutable content; UTC timing, no floats; hash is not the primary identity |
| change_previews | id, revision_id, baseline_version_id, impact report/provenance, readiness report/provenance, policy identity, assessed_at, reviewed_by | FK exact revision and matching subject baseline; no secret-bearing data; immutable report |
| change_approvals | id, revision_id, preview_id, baseline_version_id, approver_id, approved_at | FK exact revision/preview/admin; UNIQUE(id, approver_id) supports publication's matching approval/approver FK; same revision/subject as preview; new edit never mutates approved revision |
| publication_requests | id, approval_id, payload_digest, scheduling_admin_id, scheduling_security_epoch, approver_id, scheduled_at (UTC), zone, state, blocked_reason, cancellation_reason, cancelled_at, reschedules_publication_id (optional), completed_version_id, applied_at | UNIQUE(approval_id); FK scheduling admin and approver; approver must match immutable approval; payload identity includes scheduling authority/epoch and reschedule reference; partial pending and retryable-blocked due indexes exclude cancelled/completed; pending/blocked/cancelled/completed checks; cancelled rows have reason/time but no completed version/applied time; optional reschedule FK preserves terminal predecessor; stable ID reused on retries |

Settings/flags use composite subject/version FKs so a live pointer cannot target another
subject's version. For initial cyclic registry/version references, migrations define matching
deferrable FKs and seed all rows inside one transaction. Every committed live pointer is
valid. Subject-specific preview/approval FKs use typed setting/flag baseline references with
an XOR CHECK rather than an unchecked arbitrary polymorphic ID.

Console schedule input is local date+time plus IANA display zone (default Asia/Tehran),
resolved with pinned embedded zone rules and explicit Clock input, never system-clock calls
or runtime zone-file I/O in a context. Reject ambiguous/nonexistent local times and new or
edited schedules earlier than now; store UTC plus display zone. JSON time decoding accepts
ISO 8601 only with explicit Z or numeric offset (for example ±HH:mm). This is a pure value/DTO contract, not a new
JSON endpoint. Existing durable overdue rows remain executable subject to execution gates.
Publication's (approval_id, approver_id) references the matching immutable approval pair;
the worker cannot substitute an approver independent of the reviewed record.

Runtime roles may change draft heads and live pointers through repository operations but
cannot mutate immutable revisions, approvals, previews or published history. SQL constraints
reinforce domain validation; metadata min/max alone is not enforcement against a direct write.

### Review state transitions

`draft revision → previewed revision → approved revision → pending publication → published`.
Blocked publication retains a stable reason and target revision. Editing appends a revision,
CAS-advances the head and invalidates eligibility of old approvals. Reverting the value still
requires a new approval. Rollback creates a new change referencing the selected historic value.
Self-approval of ordinary changes is allowed as distinct steps; another person is mandatory
only for admin addition/recovery. (AC-ADM-010–020/044–045)

An unavailable-evidence block is retryable under the same request identity. Due reads retain
that durable candidate; the existing idempotent request/retry operation conditionally reopens
the locked blocked state only after all current authorization/review/baseline/due/evidence
checks permit. Identical recovered evidence needs no fresh confirmation; semantic mismatch
still needs renewed review. Never reopen completed/terminal state or erase a blocked reason
before a checked retry.

Scheduling authority is immutable request data: scheduling admin identity, captured security
epoch and the approver from the exact approval. ScheduledPublish acts on behalf of that admin.
Under the final transaction, re-read/lock the scheduling admin before configuration locks;
revoked/deactivated/ineligible state or any mismatch with the captured scheduling epoch causes terminal
`cancelled`, before impact/approval failure can turn it into a retryable block. Atomically
persist reason, cancelled_at and one cancellation audit; never write a live version/pointer
or successful-publication event. Audit failure rolls back cancellation and reports no success.
Reactivated/restored/re-enrolled admins cannot revive the old request, even when restoration
committed before the worker first examined it. Another active verified admin
reschedules through the existing SchedulePublish/review flow, referencing the cancelled ID
in reschedules_publication_id with a new request identity; enforce a different scheduling
admin and unchanged predecessor. Changing authorized time still requires a new revision,
preview and approval. UNIQUE(approval_id) remains; cancellation history is never overwritten.

## 4. Publication transactions and evidence

1. Read a bounded, ordered set of durable pending/retryable-blocked due candidates without claiming ownership
   or removing them from due work. Concurrent HTTP/worker requests may select the same ID.
2. Application calls pure impact/readiness calculations with current typed evidence outside
   DB locks; absent population evidence is explicitly unavailable, never zero.
3. Short final UoW locks scheduling admin first, then draft head → request → live subject.
   Recheck active eligibility and captured epoch; invalid authority cancels as above. With
   valid authority, verify expected pending state, head/revision, current approval validity,
   baseline **version identity**, due time and current report provenance. Finalization is conditional.
4. If blocked, persist blocked reason and audit without moving live value.
5. If permitted, append immutable version, move live pointer, mark completed and append
   the uniquely keyed successful audit event in the same UoW.
6. Retry returns the committed result. Unknown commit outcome is resolved by bounded lookup
   of the stable publication ID before retrying; never falsely report confirmed rollback.

ABA changes (30→7→30) invalidate an old baseline despite equal final values. Unrelated subject
publications do not invalidate it. A stale finalizer cannot apply a completed/blocked or
otherwise changed request; it returns the existing result or a typed conflict. Precommit
failure/crash leaves pending work durable for retry, with no ownership timeout. Unique
publication identity, row locking and conditional finalization give one effect. Failed
audit rolls back the whole successful transition. Effective visibility is
commit, never the missed schedule; `applied_at` is an application timestamp, not an invented
exact commit measurement. Persist authorised and actual times separately.

Reports include revision, subject baseline, source, assessment time, policy identity and
known/unknown status. Recompute at publication. No invented numeric freshness tolerance:
compare revision/typed subject, baseline version, policy/source identities, existing/new-item
effect rules and applicable known changed/already-due counts as defined in
[application-ports.md §4](contracts/application-ports.md#comparable-impact-evidence-definition-of-the-existing-review-gate).
Assessment time/report IDs alone are not semantic changes. Unavailable required evidence
blocks without live writes; a changed comparable field requires new preview/approval;
identical comparable evidence preserves approval eligibility subject to the other checks.
This defines the existing gate, not a TTL, per-item population inventory or new executor. Pending
population providers remain explicitly unavailable, never inferred as empty. (AC-ADM-011/012/
018/025/035/036/044/045)

## 5. Admin, credential and authentication state

| Table/entity | Fields and relationships | Constraints/indexes |
| --- | --- | --- |
| admins | id, encrypted canonical phone, keyed phone lookup digest, eligibility state, security_epoch, created_by, created_at; protected enrollment authorization (purpose, authorizer, authorized_at, expires_at, epoch, status) | unique lookup digest; authorization belongs to this target identity, not a bearer token; another current admin authorizes addition/recovery, operator only for first bootstrap; atomic authorization consumption/revocation; no plaintext phone in audit |
| admin_credentials | id, admin_id, kind, public credential material or encrypted TOTP envelope, credential_revision, security_epoch, last_totp_step, revoked_at | FK admin; globally unique WebAuthn credential ID; kind/material XOR; optimistic revision checks; active credential index |
| admin_sessions | id, token_digest, admin_id, security_epoch, authenticated_factor, issued_at, expires_at, last_activity_at, revoked_at | unique digest; FK admin; finite expiry; active-admin/epoch/revocation checked on every protected request; no raw cookie value |
| admin_session_states (SessionStore-owned pending state) | id, admin_id where known, browser/session binding digest, purpose, phase, security_epoch, primary_verified_at, expires_at, consumed_at, superseded_at, revoked_at, credential_revision, provisioning_presented_at, protected ceremony payload | one-use/stage/target/expiry/purpose checks; indexed active states; encrypted complete server-side WebAuthn/TOTP state; atomically mark provisioning presentation once; replay/refresh cannot return the same secret/QR; opaque browser references only; no separate AuthStateRepository |
| admin_otp_challenges | id, admin_id, browser/purpose binding, keyed code digest, expires_at, failed_attempts, consumed_at, superseded_at, provider submission category | finite expiry/attempt policy; compare-and-set consumption; new challenge supersedes old; no plaintext code |
| admin_rate_limits | keyed subject/IP-derived bucket, purpose, window identity, count, blocked_until | unique bucket/window; atomic increment; no raw phone/IP in logs; finite non-disableable policy |

Tables use FK deletion restrictions, not cascading mutations into audit/history. Secret
envelopes record key ID, nonce and ciphertext/tag; associated data binds admin/credential/
purpose/epoch. Decryption/tag failures fail closed. Key material lives in operator secret
configuration, not in DB rows or generic settings. Public passkey IDs/material are still
excluded from diagnostic/audit dumps. Normal admin authentication cannot create customer
accounts or use an end-user session as primary proof.

### Access transitions

- Eligible admin → OTP issued → code atomically consumed → restricted primary-verified
  state → passkey/TOTP verified → audited full admin session. SMS acceptance/delivery is
  never verification. OTP/factor/ceremony proofs expire and cannot be replayed.
- Invited/recovering admin needs protected identity-bound enrollment authorization recorded
  and audited by another fully verified admin (or initial operator provisioning). No
  transferable enrollment credential is issued or delivered. Target verifies their phone
  OTP, then confirms a fresh passkey/TOTP before full access. Authorization alone never
  grants access; browser/session binding belongs to the SessionStore proof/ceremony.
- BootstrapAdmin owns the one-time operator eligibility/authorization/audit transition;
  app's provisioning command only delegates. GetEnrollment reads SessionStore primary
  proof and AdminRepository authorization/current epoch in a read-only UoW, refuses wrong
  stage/browser/purpose/target, consumed/revoked or expired state, and returns safe choices.
  It neither writes state nor issues tokens/sessions; begin/finish independently recheck.
- Confirmed enrollment atomically consumes the target authorization and one-use ceremony,
  inserts the credential, consumes the matched TOTP step if applicable, issues the full
  session and appends audit. Failure rolls all of these back.
- WebAuthn completion verifies a server-stored, one-use ceremony against current credential
  revision/epoch; stale snapshots restart. Persist library counter/backup updates. Both-zero
  counters can be valid. Wrong origin/RP, expired challenge and consumed state are rejected.
- TOTP matched step must be greater than `last_totp_step`; enrollment confirmation consumes
  its step too. Concurrent replays cannot issue two sessions.
- The only UI-secret exception is the authenticated enrolling admin's TOTP secret/QR, shown
  exactly once in a no-store response after a conditional SessionStore presentation mark.
  It is never logged, re-displayable, bundled or sent elsewhere; no other secret enters UI.
  If the response is lost, restarting uses a fresh ceremony/secret and supersedes the old
  pending state, never redisplays it. Full-session enrollment is not a bootstrap prerequisite.
  Presentation marking does not remove the opaque protected verifier state needed for
  confirmation; trusted verification use does not permit another UI presentation.
- Every committed admin revocation, deactivation or recovery advances security_epoch to a
  strictly greater durable value (monotonic) in the same transaction as the invalidating admin-state
  transition, using the existing target lock/expected-epoch CAS. Reactivation/restoration
  retains the current epoch or advances it, never resets it or reuses an older value.
  Reject stale CAS and non-increasing invalidation writes; failure/rollback
  persists neither the transition nor an epoch change, and overflow fails closed, never wraps.
  Fake and PostgreSQL repositories enforce the same invariant; the final worker lock
  serializes with these transitions. No new endpoint or admin lifecycle service is introduced.
  A schedule captured at E cancels after deactivate→reactivate or revoke→restore because
  the current epoch remains greater than E; an unchanged E executes only if all other gates pass.
- Recovery UoW locks target admin, advances epoch by the invariant above, revokes all target sessions/credentials/
  proofs/ceremonies, creates restricted re-enrollment authorisation and appends audit.
  Existing target cookies fail immediately on next use; other admins remain unaffected.
  The last admin has no bypass path. (AC-ADM-001–007/025/043)

OTP, ceremony, invitation, idle/absolute-session and abuse-limit parameters belong to a
validated operator security policy, not the ten editable business settings. Required values
are finite and cannot disable checks; startup rejects missing/insecure configuration. Tests
use explicit fixed policy fixtures and clock-controlled boundary cases. This design does
not silently assign new Q-number business constants.

## 6. Audit and managed copy

`audit_log`: id, stable operation/event key, actor type/id, action code, subject type/id,
occurred_at, reason_code, allowlisted before/after values and safe metadata (factor name,
revision/publication IDs, scheduled/actual times where relevant). Scheduled execution and
cancellation record actor_id = scheduling_admin_id, executed_by = system_worker and approver_id;
the system identity is application-owned, never client supplied. Cancellation retains its
visible reason and has its own stable event key, so retries produce one cancellation audit.
UNIQUE event key protects
the successful-publication event against retries. Search indexes: `(occurred_at,id)`,
`(actor_id,occurred_at,id)`, `(action,occurred_at,id)`, `(subject_id,occurred_at,id)`.
Use deterministic keyset pagination. No UPDATE/DELETE port, no PII/secret-bearing snapshots.

Migration owner creates UPDATE/DELETE guards plus statement-level TRUNCATE guard; runtime
cannot update/delete/truncate/disable triggers, own objects or assume owner privileges.
Protect immutable history similarly. Test runtime privileges and trigger defence separately.
This protects runtime/admin principals, not a hostile PostgreSQL superuser. (AC-ADM-021–025)

Managed foundation messages live in FA/EN Fluent resources, keyed by surface and situation,
with a parity test and explicit missing-key errors. They include labels, validation/status,
reason-code explanations and accessible names. This is not the future content editor.
Language/theme preferences are non-authoritative presentation state; toggling them carries
form fields/revision/CSRF safely without changing draft/live data. (AC-ADM-026–032)

## 7. Seed and migration ordering

1. Provision cluster-global migration/runtime roles once (operator setup, or idempotent
   shared-fixture setup in tests). Database-local schema migrations create ID/typed value
   support, registry/history tables and deferrable FKs under the intended migration owner.
2. Draft/review/publication tables, constraints and due/search indexes.
3. Admin/credential/session/proof/rate-limit tables; secret envelopes contain no seeded keys.
4. Audit/immutable-history guards and restricted runtime grants.
5. Idempotent seed through an application service/UoW: exact ten initial values and six OFF
   flags, baseline history and system-seed audit. A stable seed operation key makes reruns
   no-ops; never reset an already published admin value. No invented privileged admin/factor
   is included in general data seeds.

Each step is driven by failing integration/contract tests before its migration is written.
SQLx offline metadata follows the migrated schema. Deployment uses the migration role only
for migrations; serve/worker use restricted runtime credentials. No actual migration is
included in this planning artifact.

## 8. Test database ownership

The owner-approved shared-run fixture model is defined in
[contracts/integration-fixtures.md](contracts/integration-fixtures.md). Tests reference the
single exported infra-postgres `MIGRATOR` through `#[sqlx::test(migrator = "infra_postgres::MIGRATOR")]`.
Each test gets a fresh database, not an isolated schema on a common database. SQLx's fixture
connection may create/migrate databases, but real adapter/service pools use the corresponding
database with the restricted runtime role. Cluster-global roles are not recreated or mutated
concurrently by each database migration. Ownership and privilege tests verify this distinction.

Failed databases are retained only until the same test path is rerun or fixture storage is
removed. SQLx names test databases from test paths, so independent runs sharing a local
fixture server are serialized; no broad cleanup sweep runs from individual nextest children.
MinIO startup/bucket fixture implementation is deferred to feature 004; its Compose service
stays declared. Feature 001 asserts no storage-test coverage. No production schema, seed or
storage implementation moves into testkit to provide this test infrastructure.
