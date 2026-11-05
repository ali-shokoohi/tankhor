# HTTP, Template and API Contracts — Feature 001

Design only. Trace: FR-CON-01–11/32/37; FR-UX-01–09/14/17; Q01–Q03/Q09/Q23;
AC-ADM-001–045. All handlers call application services, not repositories or vendor clients.

## 1. Common transport/security behaviour

- One configured HTTPS origin, paths on that host; no console subdomain. Exact WebAuthn
  RP/origin configuration, no wildcard origins. Browser challenge state is stored server-side.
- Separate opaque primary/enrollment and full-admin cookies. Production uses Secure,
  HttpOnly, SameSite=Lax and no Domain attribute. If using `__Host-` names, Path MUST be `/`,
  not `/console`. Cookie path is not an authorisation boundary. Full admin access always
  rechecks session, finite expiry and current security epoch; no end-user-cookie substitution.
- Every state-changing form—including access begin/finish, preferences and sign-out—requires
  CSRF and origin checks. GETs do not perform those mutations. No sensitive values in URLs,
  request tracing, redirects, analytics or body dumps. Auth responses use `Cache-Control: no-store`.
- Unauthenticated HTML access redirects to `/console/sign-in` with a validated same-origin
  return destination. A known non-admin/invalid grant cannot see protected content. JSON
  errors return problem+json, never an HTML redirect disguised as API success.
- Valid form mutation: 303 to a full-page GET. Invalid field: 422 rendered form with preserved
  safe input and associated errors; stale revision: 409 conflict view; absent/failed prerequisite:
  visible blocked status without live changes. Missing audit/DB: sanitized retryable failure,
  not success. Invalid primary/factor material is not echoed back into HTML.
- htmx may request partial content, but the same action/validation/full-page destination
  works without it; apply `Vary: HX-Request` where representation changes. No GET side effects.
- Strict CSP and local assets. Minimal essential passkey bridge calls browser WebAuthn and
  submits a form; no SPA, CDN, unsafe JSON-to-script interpolation or logging credentials.

## 2. Access HTML/form routes

All POSTs are form submissions returning HTML/303, not JSON API endpoints. Route names are
implementation choices; their semantics and service boundaries are the contract.

| Method/path | Service and allowed state | Outcome |
| --- | --- | --- |
| GET `/console/sign-in` | Access presentation, no full session required | Localized phone form and safe return target; no credential values |
| POST `/console/sign-in/request-code` | RequestAdminOtp | Generic request response; sends only for eligible admin; no public registration |
| POST `/console/sign-in/verify-code` | VerifyAdminOtp | One-use primary proof and second-factor choice; never full access yet |
| POST `/console/sign-in/passkey/begin` | BeginPasskey, primary proof required | HTML containing escaped public options and opaque ceremony reference |
| POST `/console/sign-in/passkey/finish` | FinishPasskey | Verify server state + assertion; audited session or localized error |
| POST `/console/sign-in/totp` | FinishTotp, primary proof required | Atomically verify accepted timestep and issue audited session |
| GET `/console/access/enrol` | Restricted invited/recovery grant + primary proof | Enrollment screen only; no protected management content |
| POST `/console/access/enrol/passkey/begin` | BeginEnrollment | Bound registration state/options |
| POST `/console/access/enrol/passkey/finish` | FinishEnrollment | New confirmed factor, one-use state, no old-epoch access |
| POST `/console/access/enrol/totp/begin` | BeginEnrollment | Locally rendered shared-secret/QR setup once; no-store, not logged |
| POST `/console/access/enrol/totp/finish` | FinishEnrollment | Confirm and consume timestep before session eligibility |
| POST `/console/sign-out` | SignOut | Revoke current full session, clear cookie, redirect |

Only public credential **options**, not serialized library ceremony state, enter an HTML-
escaped hidden field. The bridge reads/serializes required WebAuthn binary fields safely and
POSTs them with CSRF. Bind completion to browser/admin/purpose/epoch/expiry and current
credential revision. Test malicious-string escaping, replay, origin/RP and session mismatch.

No-JS sign-in uses phone OTP plus already-enrolled TOTP. Passkeys intrinsically need essential
browser scripting; when unavailable, explain fallback. Do not automatically enrol TOTP,
invent password login or bypass a second factor for an admin lacking the fallback. Test both
real verification paths and no-htmx management. (AC-ADM-002/005/030/032/043)

## 3. Protected management HTML/form routes

All rows require a current fully verified admin session. CSRF applies to every POST.

| Method/path | Service | Contract |
| --- | --- | --- |
| GET `/console` | Registry/query services | Parent Tankhor shell; no placeholder reports/media controls |
| GET `/console/settings` | ListSettings | Ten settings, unit/bounds/live value/effective time/history |
| GET `/console/settings/{key}` | GetSetting/history | Version detail and draft entry; no arbitrary key registration UI |
| POST `/console/settings/{key}/drafts` | SaveDraft | Typed value, reason, expected head/timing; draft only |
| GET `/console/flags` | ListFlags | Six OFF flags; honest not-delivered/unknown health |
| POST `/console/flags/{key}/drafts` | SaveDraft | Boolean proposal through same review path; no health override |
| GET `/console/changes/{id}` | GetChange | Current revision, live versus proposed, approval/blocked status |
| POST `/console/changes/{id}/preview` | Preview | Persist exact revision/baseline report; no downstream side effects |
| POST `/console/changes/{id}/approve` | Approve | Explicit revision and reviewed preview; stale mismatch conflicts |
| POST `/console/changes/{id}/publish` | Publish/SchedulePublish | Approval + stable operation identity; immediate or scheduled UTC/zone |
| GET `/console/publications/{id}` | GetPublication | Pending/blocked/completed, reason, scheduled/actual time; no false success |
| POST `/console/versions/{id}/rollback` | Rollback | New draft from earlier values; normal review still required |
| GET `/console/audit` | SearchAudit | Actor/action/subject/time filters, bounded cursor pagination |
| GET `/console/admins` | Admin query | Equal permissions and identity-safe administration list |
| POST `/console/admins/invite` | InviteAdmin | Another-admin grant, reason and audit; not instant console access |
| POST `/console/admins/{id}/recover` | RecoverAdmin | Revoke all target sessions/factors/proofs, issue restricted enrollment grant, audit |
| POST `/console/preferences` | Presentation preference handling | Locale/theme only; carry safe current form state/revision without silently saving or editing business data |

Do not add audit-edit/delete endpoints, generic security toggles, user/media inspection,
financial operations, pricing, emergency stop, content editor, reports or optional-capability
behaviour. Those remain later features. OTP sends do not depend on the external-notifications flag.

## 4. Template, Fluent and interaction contract

Askama console layout and reusable partials: header/nav, field/error, setting row, flag row,
draft/live summary, impact panel, approval/publication status, audit table, pagination,
confirmation dialog and access forms. Every full page has one h1, explicit language/direction,
named controls, meaningful image alternatives, focus management/return and non-colour-only
statuses. Fields keep labels and errors associated; no trap at zoom/reflow or 360 px width.

Representative managed keys (each in FA and EN): `console-title`, `console-sign-in`,
`auth-primary-required`, `auth-code-invalid`, `auth-factor-required`, `auth-recovery-enrol`,
`setting-{key}-label`, `setting-range`, `setting-out-of-range`, `format-jpeg-required`,
`console-draft-saved`, `console-preview`, `console-approve`, `console-publish`,
`publication-baseline-changed`, `publication-prerequisite-unknown`, `publication-overdue`,
`publication-published`, `impact-existing-count`, `impact-next-sweep`, `impact-preserve-existing`,
`flag-not-delivered`, `audit-actor`, `audit-reason`, `theme-system`, `language-fa`, `language-en`,
and the managed reason-code explanations. Missing keys fail review/tests; no silent English
fallback establishes FA completeness. Key names are implementation identifiers, not final copy.

Use the source palette and Vazirmatn with its Latin; no generic SF Pro replacement or new
console logo. Meaningful controls use `borderControl`, curtain text uses `onCurtain`, and
decorative `border` is never the sole control boundary. Verify unrounded ratios on rendered
samples: normal text ≥4.5:1, large ≥3:1, meaningful boundary/focus ≥3:1. Touch targets ≥44×44 px,
reduced motion and pause/stop on nonessential auto-motion >5s. Persian prose digits/RTL focus
order; URLs/IDs/hex stay directional/Latin; photos/logos never mirror.

Language/theme changes POST safe form state or retain a server-side draft/form-state reference;
they do not discard unsaved fields, alter revision/hash or publish. Never preserve an OTP,
TOTP shared secret or credential assertion by reflecting it into a preference response.
System theme follows device preference; manual preference survives navigation. HTTP/scraper/
insta and browser/axe tests prove the full matrix, not screenshots alone. (AC-ADM-026–032)

## 5. JSON API and document bootstrap

| Method/path | Auth | Response and schema |
| --- | --- | --- |
| GET `/api/v1/health` | Explicitly permitted minimal health request | 200 `HealthResponse {status: "ok"}`; unavailable required store/seed returns 503 Problem; no internal diagnostics |
| GET `/api/v1/openapi.json` | Admin in production; explicit dev/staging policy | 200 `OpenApiDocument` schema bridge, served frozen document; auth/error responses documented |

Both handlers use `#[utoipa::path]` and `OpenApiRouter`. The builder starts empty, registers
both paths, splits router/document, freezes the resulting document and attaches state.
Document schema describes required OpenAPI top-level shape without embedding a literal copy
of the document. Validate it against a pinned local OpenAPI meta-schema in tests. API errors
share `application/problem+json`: type, title, status, safe detail and an opaque instance or
correlation reference; never echo sensitive paths/body/provider errors. State-changing API
routes are not invented for HTML management. Future money-moving POSTs require documented
Idempotency-Key; no money endpoint is introduced here.

Swagger `/api/docs` uses bundled local assets and an independently configured schema URL;
disable external validation. Production authorisation covers the UI, redirects, assets and
schema. Offline `tankhor openapi` uses the exact router/document builder without a database
connection, provider account or running web server. Implementation commits its output at
`docs/openapi.json`; snapshot/schema-conformance and CI regeneration/diff prevent drift.
