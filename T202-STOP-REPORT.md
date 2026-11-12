# Stop report — T199–T202

Stopped at a clean task group under **AGENTS.md §7’s long-session rule**.

## Completed

- **T199–T200:** PostgreSQL admin/authentication schema and integration constraints.
- **T201–T202:** Append-only audit schema, indexes, privileges, and mutation guards.

All four tasks are checked in `specs/001-admin-foundation/tasks.md`.

**Next unchecked task: T203 — `audit_repository_contract_postgres`.**

## RED evidence

- **T199:** PostgreSQL setup passed; the test compiled and executed, then failed with `missing table admins`.
- **T201:** All three required tests executed and failed specifically with `missing audit table`.

Earlier compile/fixture failures were corrected and were not counted as RED evidence.

## GREEN and integration evidence

- T199 focused schema test — **PASS**
- T201 final audit-guard suite — **3/3 PASS**
- Full `infra-postgres` integration profile — **7/7 PASS**
- Testkit shared-fixture integration — **1/1 PASS**
- Fixture-bootstrap profile — **2/2 PASS**
- `cargo fmt --check` — **PASS**
- Clippy for `infra-postgres` and `testkit`, all targets/features with warnings denied — **PASS**
- `git diff --check` — **PASS**

No fake tests were used as evidence of PostgreSQL correctness.

## Files changed

### `crates/infra-postgres`

- Added `crates/infra-postgres/migrations/0001_admin_access.sql`
- Added `crates/infra-postgres/migrations/0002_audit.sql`
- Added `crates/infra-postgres/tests/integration_auth_schema.rs`
- Added `crates/infra-postgres/tests/integration_audit_guards.rs`

### `crates/testkit`

- Modified `crates/testkit/src/fixtures/roles.rs` so cluster-global role provisioning takes its advisory lock through the shared `postgres` database rather than separate per-test database lock namespaces.
- Modified `crates/testkit/Cargo.toml` to add missing `required-features = ["fakes"]` declarations for previously added mock/fake targets.

### Task tracking

- Updated only T199–T202 checkboxes in `specs/001-admin-foundation/tasks.md`.

## SQLx status

Real migrations ran successfully in isolated fixture databases. These tasks added no checked Rust SQLx queries, so `cargo sqlx prepare` was not applicable and `.sqlx` metadata was unchanged.

## Security and architecture review

- Phone, OTP, session, ceremony, and provisioning values use digests or encrypted-envelope columns.
- Credential material XOR, finite expiry, revision, epoch, one-use, authorization-consumption, and rate-limit constraints are enforced.
- Audit runtime privileges are SELECT/INSERT-only.
- UPDATE, DELETE, and statement-level TRUNCATE guards also reject table-owner mutations.
- No audit cascade or runtime owner/trigger escalation was introduced.

## Corrections and exclusions

- Corrected test UUID casting and owner assumptions without weakening assertions.
- Fixed a demonstrated concurrent fixture role-provisioning defect.
- The full T274 workspace checkpoint, real repository contracts, concurrency acceptance, and SQLx online preparation for future adapters remain pending.
- Only generated Cargo artifacts were cleaned; **18 GB** was available after cleanup.

No governing documents were changed, no tests were ignored or weakened, and no commits or history modifications were made.
