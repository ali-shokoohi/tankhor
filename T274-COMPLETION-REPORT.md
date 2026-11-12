# T274 Completion Report — Admin Foundation / Phase 3 / US1

Date: 2026-11-12

## Status

- **T203–T274 completed and checked.**
- The mandatory T274 checkpoint is GREEN, including the separately owner-approved, narrowly scoped RSA advisory exception.
- First remaining task: **T275** (not started; outside the authorized scope).
- No Phase 4 or T275+ work started.
- No commits, tags, pushes, or Git history changes were made.

## TDD evidence

Each RED driver failed for its intended missing implementation before GREEN:

| Group | RED evidence | GREEN evidence |
| --- | --- | --- |
| T203–T204 | Missing `infra_postgres::audit_repository` | Shared audit contract on PostgreSQL |
| T205–T206 | Missing `admin_repository` | Shared admin contract |
| T207–T208 | Missing `admin_credential_repository` | Shared credential contract |
| T209–T210 | Missing `session_store` | Shared session contract |
| T211–T212 | Missing `otp_challenge_repository` | Shared OTP contract, including concurrent consumption |
| T213–T214 | Missing `rate_limit_repository` | Shared rate-limit contract |
| T215–T224 | Missing infra-auth package/modules | Envelope, capability, TOTP, WebAuthn and real authenticator tests |
| T225–T228 | Missing infra-sms package/modules | Both unchanged SMS contracts over wiremock |
| T229–T233 | Missing real harness/race helpers | Four real concurrency acceptance tests |
| T234–T269 | Missing named HTTP routers/behavior | All 18 HTTP RED/GREEN pairs completed |
| T270–T271 | Missing composition API | Real/fake composition integration test |
| T272–T273 | Missing `app::provision_admin` | Protected provisioning integration test |

T233 passed on its first execution because the required audit-failure behavior already existed.

## Observed test results

- `cargo nextest run`: **148/148 passed**.
- Final `cargo nextest run --all-features`: **149/149 passed**.
- Final `cargo nextest run --profile integration --all-features`: **21/21 passed**.
- `cargo nextest run -p admin -p testkit --features testkit/fakes`: **93/93 passed**.
- `cargo nextest run --profile fixture-bootstrap -p testkit`: **2/2 passed**.
- Python repository tests: **25/25 passed**.
- Web console suite reached **33/33 passed**.
- infra-auth suite: **7/7 passed**.
- infra-sms suite: **4/4 passed**.

These results include AC-ADM-001–007/043 fake application tests, real repository contracts, HTTP security tests, and real PostgreSQL race acceptance.

## PostgreSQL, SQLx and concurrency verification

Verified against restricted runtime PostgreSQL roles:

- All six unchanged shared repository contracts.
- Append-only audit privileges and mutation guards.
- Caller-owned UnitOfWork commit/rollback behavior.
- Passkey single-use race: one session, one audit, final credential revision.
- OTP/TOTP single-use races and accepted-step CAS.
- Recovery/login race with stale-login refusal and target-only revocation.
- Audit failure aborting session creation with complete rollback and safe retry.

Online SQLx preparation was run against migrated fixture-owned databases after repository changes. Final checks:

- `SQLX_OFFLINE=true cargo check --workspace --all-targets --all-features`: **PASS**.
- `cargo check -p app --release`: **PASS**.

## Real cryptographic and SMS verification

- AES-256-GCM envelopes bind protected values to their intended context and reject wrong keys, AAD and tampering.
- OTP and opaque capabilities use secure generation, purpose-separated keyed digests and constant-time comparisons.
- TOTP verifies the matched step and remains usable with a later independent primary proof.
- WebAuthn uses safe `webauthn-rs` APIs, local signed fixtures, origin/challenge validation, replay rejection, counter updates and zero-counter support.
- The unchanged real `AdminAuthenticator` contract passed.
- Kavenegar and SMS.ir contracts passed over wiremock, including malformed responses, timeouts, redirect refusal, safe result categories and redaction.
- No live SMS providers or external passkey services were contacted.

## HTTP and security verification

Observed passing coverage for:

- Localized FA/EN forms and safe return targets.
- CSRF and Origin enforcement on mutations.
- No-store authentication responses.
- Restricted primary proofs and completed-second-factor session requirements.
- Passkey option escaping and protected server state.
- Exactly-once, authenticated, direct `Cache-Control: no-store` TOTP provisioning.
- Replay, GET, refresh and concurrent redisclosure refusal.
- Admin invitation, recovery, sign-out and stale-session guards.
- Protected Swagger UI, assets and schema.
- Full-page no-JS/no-htmx equivalents and accessible passkey-disabled guidance.
- Production rejection of fake adapters and coherent real/fake composition.
- Operator-only CLI provisioning through `BootstrapAdmin`, without phone arguments or secret output.

## Repository checks

Passed:

- Workspace Clippy with `-D warnings`
- `cargo fmt --check`
- `git diff --check`
- `python3 scripts/check-architecture.py`
- `python3 scripts/check-test-traceability.py --mode partial`
- OpenAPI snapshot and offline generation tests

Dependency policy:

- `cargo deny check`: **PASS** for advisories, bans, licenses and sources.
- The original finding was `RUSTSEC-2023-0071` through `webauthn-rs 0.6.2` → `crypto-glue 0.2.2` → `rsa 0.10.0-rc.19`.
- The official advisory concerns timing leakage in RSA private-key operations. Source and call-path inspection found WebAuthn uses RSA only for public-key signature verification of RS256 assertions and RSA attestation/X.509 certificates. Tankhor holds no server RSA private key and exposes no RSA private-key decryption or signing operation. Untrusted passkey input can reach public verification, not the advisory's private-key operation.
- No compatible released dependency fix exists: the latest webauthn-rs/core and crypto-glue releases retain this graph. Selecting patched `rsa 0.10.0` fails because crypto-glue requests RSA's removed `sha2` feature; current SHA2 support is mandatory rather than feature-selected in that release.
- The owner separately approved the precise exception on 2026-11-12. `deny.toml` scopes it only to RUSTSEC-2023-0071, records the public-verification-only rationale, and requires removal when a compatible crypto-glue release selects `rsa >=0.10.0`. No dependency override, fork, patch or blanket ignore was introduced.
- A subsequent direct-SQLx dependency-ban finding in `app` was removed architecturally by adding `PostgresUnitOfWork::connect_lazy` in infra-postgres and removing app's direct SQLx dependency.

## Files changed by crate

- **Workspace:** `Cargo.toml`, `Cargo.lock`, generated `.sqlx/`, and the owner-approved scoped advisory entry in `deny.toml`.
- **admin:** narrow domain getters, application module registration, and application-owned `console_services.rs`.
- **infra-postgres:** six real repository adapters, shared SQL error mapping, migration corrections, integration contracts, real race harness and acceptance tests.
- **infra-auth:** new crate with envelopes, capabilities, TOTP, WebAuthn, `RealAdminAuthenticator`, and signed local fixtures.
- **infra-sms:** new crate with Kavenegar and SMS.ir adapters and wiremock tests.
- **web:** console handlers, application guard, templates, FA/EN Fluent messages, local passkey bridge, security cookies and HTTP tests.
- **app:** real/fake composition, serving integration, protected provisioning CLI and integration tests.
- **Generated documentation:** `docs/openapi.json` regenerated; resulting content matched the existing snapshot.

## Deviations, exclusions and blockers

- The RSA vulnerability remains in the resolved transitive crate, but its private-key operation is not reachable from Tankhor's WebAuthn paths; the owner-approved exception is explicit and reversible. RS256 exposure was established by source inspection; the dynamic signed fixture uses ES256.
- No governing product requirement was changed and no test was weakened, deleted, renamed or ignored.
- Browser JavaScript was source/HTTP tested, not executed in a real browser.
- Final-mode whole-feature traceability was not run; the required US1 partial mode passed.
- Real production HTTP serving remains intentionally blocked by the future real HealthProbe task, T356.
- All prior work and Git history were preserved.

## Next task

Stop at the completed US1 boundary. The owner reviews and commits. **T275 was not started.**
