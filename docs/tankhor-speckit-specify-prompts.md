# Tankhor — `/speckit.specify` Prompt Pack

**15 features · 3 November 2026 · for Spec Kit + OpenCode**

## How to use

1. Put the source documents in the repo:
   - `docs/tankhor-prd.md`
   - `docs/tankhor-technical-spec.md`
   - `docs/tankhor-decisions-qa.md`
2. Save each prompt below as its own file in `docs/prompts/` (e.g. `docs/prompts/003-wallet.md`) so you can version and tweak it.
3. In OpenCode, run either form:
   - paste the prompt block after `/speckit.specify`, or
   - `/speckit.specify @docs/prompts/003-wallet.md` (OpenCode inlines `@path` files).
4. Run features **in order**. Each prompt lists the features it depends on; don't start one until its dependencies have converged.
5. After each specify: `/speckit.clarify` → read and fix `spec.md` by hand → `/speckit.plan` → `/speckit.checklist` → `/speckit.tasks` → `/speckit.analyze` → `/speckit.implement` → `/speckit.converge` until converged.

## Rules baked into every prompt

- **WHAT and WHY only.** Spec Kit's template wants no tech stack, APIs or code structure in `spec.md`; that belongs in `/speckit.plan`. The prompts describe behaviour, rules and outcomes, never Go, Postgres or endpoints.
- **Decisions are final.** All 41 product decisions (Q01–Q41) are closed. Each prompt tells the model not to reopen them and to mark `[NEEDS CLARIFICATION: …]` only for gaps the documents truly don't answer.
- **Traceability.** Every prompt names the PRD requirement IDs (`FR-*`), acceptance criteria (`AC-*`), guardrails (`GR*`) and tech-spec IDs (`P*`, `R*`, `U*`, `F*`) it covers, and asks the spec to cite them.
- **Hard boundaries.** Each prompt has an explicit out-of-scope list so features don't bleed into each other.
- **Testable success.** Each prompt ends with measurable success criteria the spec must carry forward.

## Feature order

| # | Feature | PRD coverage | Depends on |
| --- | --- | --- | --- |
| 001 | Admin foundation: console access, settings, flags, audit, i18n/theme shell | E14 cross-cutting, FR-CON-01–11, 32, 37; E17 base | — |
| 002 | Identity, session and account lifecycle | E2 | 001 |
| 003 | Wallet, payments and token ledger | E12; FR-CON-22, 27 | 001, 002 |
| 004 | Media intake, consent, retention and private access | E4/E11 media parts; FR-CON-23, 25 | 001, 002 |
| 005 | Generation engine: units, retries, providers, refunds, cost control | E5/E10 engine parts; FR-CON-28, 36 | 003, 004 |
| 006 | Content, catalogue and publishing workflow | FR-CON-05–09, 24, 29–31; E3 data | 001, 004 |
| 007 | Try It On: discovery, try-on creation, fitting room, result | E3, E4, E5 | 002–006 |
| 008 | Buyer library: Try-ons, compare, Set aside, photos, profile | E6 | 007 |
| 009 | Studio lobby and step 1 selection | E7, E8 | 002, 006 |
| 010 | Studio step 2 assembly and Create | E9 | 003, 004, 009 |
| 011 | Studio generation page and Projects | E10 | 005, 010 |
| 012 | Studio library, Saved, store profile, Share and publication review | E11; FR-CON-26 | 011 |
| 013 | Notifications, help, maintenance, reports and operations | E13; FR-CON-20, 21, 35, 38 | 005, 007, 011 |
| 014 | Landing and PWA | E1, E16 | 007, 009 |
| 015 | Built-but-OFF: partner widget/API, percentage rollout, usage caps, ticketing, external notifications | E15; FR-CON-33, 34; FR-NTF-02, 04 | all |

---

## 001 — Admin foundation

```text
/speckit.specify
Feature 001 — Tankhor admin foundation: secure Owner Console access, versioned settings, feature flags, audit log, and the bilingual/theme shell every later feature builds on.

SOURCES (read before writing):
@docs/tankhor-prd.md — §0.4, §3.3, §7, E14 cross-cutting rules FR-CON-01..11, FR-CON-32, FR-CON-37, E17 FR-UX-01..09, FR-UX-14, §10, §12.4
@docs/tankhor-technical-spec.md — §8 (P30, P41, P42, P44), §12.1–12.6
@docs/tankhor-decisions-qa.md — Q01, Q02, Q03, Q23

RULES FOR THIS SPEC:
- Describe WHAT and WHY only. No technology, frameworks, APIs, schemas or code structure.
- Decisions Q01–Q41 are final. Do not reopen them. Use [NEEDS CLARIFICATION: …] only for gaps the sources do not answer.
- Cite PRD IDs (FR-*, AC-*, GR*) next to every requirement and scenario.
- Stay inside scope. Anything listed under OUT OF SCOPE must not appear as a requirement.

WHY:
The owner must run the platform without engineers and without risk of breaking the live product. Every later feature (pricing, AI config, content, retention) relies on three shared capabilities: an admin can sign in securely, every tunable is a versioned setting rather than a hard-coded value, and every admin action is attributable. The FA/EN and light/dark/system shell must exist from the first screen so no later feature is built LTR-only or light-only.

ACTORS: Admin (all admins have equal full permissions at launch), System (scheduled effects of settings).

IN SCOPE:
1. Console access (FR-CON-01..04, Q23)
   - Console is a separate app at its own path on the single Tankhor domain (Q01), reachable only by admin accounts.
   - Admin sign-in requires a second factor: passkey preferred, authenticator-app code (TOTP) as fallback. Without a completed second factor the console never opens.
   - Adding an admin and recovering a locked-out admin are done by another admin; both are audited.
   - Admin sessions are separate from end-user sessions.
2. Settings registry (FR-CON-32, R72)
   - Every tunable value has: key, human label (FA/EN), unit, current value, allowed range, effective-from time, and full history of who changed it, when, from what, to what, and why.
   - Settings to register now (values from PRD §10): buyer temporary photo lifetime 14 days; Studio temporary upload lifetime 30 days; Studio auto-draft lifetime 30 days; account deletion recovery window 30 days; max cards per Studio project 50; buyer permanent photos 5; upload max size 15 MB; allowed formats JPEG/PNG/WebP/HEIC; minimum shortest edge 512 px; max generation attempts 3.
   - Shortening any lifetime shows how many existing items would be affected before it can be published.
   - Security and financial-integrity rules are not settings and cannot be weakened from here.
3. Feature flags (FR-CON-32)
   - Each flag: name, description, state (on/off), impact preview, health check result, history.
   - Register the six launch flags, all OFF: partner widget/API, percentage rollout, per-user usage limits, ticketing, external notifications, paid Share billing.
4. Change workflow for settings and flags (FR-CON-05..10)
   - Saving creates a draft; going live requires preview → approval → publish now or at a scheduled time → new version.
   - Approval binds to the exact version; editing after approval invalidates it.
   - Rollback publishes a previous version as a new event.
   - Urgent changes still show impact and require confirmation.
5. Audit log (FR-CON-37)
   - Append-only, searchable record of admin sign-ins, factor used, setting/flag drafts, approvals, publishes, rollbacks, admin additions and recoveries.
   - Each entry: actor, action, subject, time, reason, before/after where relevant.
   - No admin can edit or delete audit entries.
6. Bilingual and theme shell (FR-UX-01..09, FR-UX-14, Q02, Q03)
   - Every screen (console and, by extension, all future surfaces) supports Persian (RTL) and English (LTR), and light, dark and system themes.
   - Switching language or theme never alters data or unsaved input.
   - RTL rules: reading/focus order mirrors; IDs, URLs and hex codes keep their own direction; logos and photos never mirror; only directional icons flip. Persian digits in Persian prose; Latin digits in codes and IDs.
   - Typeface Vazirmatn with its Latin; palette per approved tokens (dark velvet brown + sky blue), including `borderStrong` for meaningful control boundaries and `onCurtain` for text on the curtain; the base `border` token is never a control's only boundary (Technical Spec §12.2).
   - One parent brand, Tankhor (تنخور), on every surface including the console; no separate logos per surface (FR-UX-17).
   - Accessibility baseline WCAG 2.2 AA: contrast, 44×44 touch targets, keyboard operation, visible focus, named controls, errors tied to fields and not colour-only, reduced motion.
   - All UI strings come from a managed string table keyed by surface and situation, never hard-coded.

KEY ACCEPTANCE SCENARIOS:
- Given an admin with correct password but no second factor, when they sign in, then the console stays closed.
- Given an admin changes "auto-draft lifetime" from 30 to 7 days and presses save, then nothing changes live until preview, approval and publish; the preview states how many drafts would be purged.
- Given a published setting change, when another admin rolls it back, then the old value is live again and both events appear in the audit log.
- Given the console in Persian, then layout, focus order and tables are RTL, setting keys and IDs render LTR inside RTL text.
- Given an admin tries to edit an audit entry, then it is impossible.

EDGE CASES TO COVER:
- Two admins edit the same setting draft at once.
- A scheduled publish whose approval was invalidated by a later edit.
- Last remaining admin loses their passkey and TOTP device.
- A setting value outside its allowed range.
- A flag's health check fails when someone tries to enable it.

OUT OF SCOPE (later features): end-user sign-in (002), money and pricing (003), media (004), AI providers and emergency stop (005), content pages and catalogue (006), reports (013), the behaviour behind each OFF flag (015).

SUCCESS CRITERIA:
- 100% of console access attempts without a second factor are refused.
- 100% of setting/flag changes are reachable live only via preview → approval → publish, and each produces an audit entry.
- Every registered setting has unit, range and history.
- Every console screen passes FA/EN and light/dark checks and a WCAG 2.2 AA automated scan with no critical violations.
```

---

## 002 — Identity, session and account lifecycle

```text
/speckit.specify
Feature 002 — Tankhor identity: mobile number + SMS one-time code sign-in, one shared session across Try It On and Studio, origin-preserving sign-in at the moment of a personal action, account settings, and the 30-day deletion/restore lifecycle with token forfeiture.

SOURCES (read before writing):
@docs/tankhor-prd.md — E2 (FR-IDN-01..15, AC-IDN-1..6), §10.4, §11.5, §12.4 NFR-SEC-04, §13 auth/account events, RK3, RK5
@docs/tankhor-technical-spec.md — §3 (P02, P14), F13, F14, U09, U10, U29
@docs/tankhor-decisions-qa.md — Q08, Q09, Q10, Q11

RULES FOR THIS SPEC:
- WHAT and WHY only; no technology, APIs or schemas.
- Q01–Q41 are final. Mark [NEEDS CLARIFICATION: …] only for genuine gaps.
- Cite FR-IDN-* / AC-IDN-* next to each requirement and scenario.
- Respect OUT OF SCOPE strictly.

WHY:
Phone number is the most universal identity in Iran (Q09). Asking for sign-in only when the user does something personal — and returning them to exactly what they chose — removes the biggest drop-off point. One session across both surfaces makes the shared wallet feel like one product.

ACTORS: Guest, Signed-in user, Support/admin (phone change with lost number), System (deletion window expiry).

IN SCOPE:
1. Sign-in method (FR-IDN-01..04)
   - Only mobile number + SMS one-time code. No email, no password.
   - First successful verification of a new number creates the account.
   - Number input accepts Persian or Latin digits and +98 / 0 prefixes; normalised to one canonical form.
   - Code entry: resend control with visible countdown; distinct errors for invalid number, wrong code, expired code, too many attempts.
   - Codes expire, are single-use, and sends/verifies are rate-limited per number and per network origin.
   - Code delivery uses two SMS providers; if the first fails, the second is used without the user doing anything.
2. When sign-in is asked (FR-IDN-05, Q08)
   - Never for browsing either surface.
   - Required for: starting any try-on (including free campaigns), any personal save (Set aside, photos, Saved), entering the Studio build flow from any of its three entries.
3. Origin preservation (FR-IDN-06)
   - The sign-in step carries the origin: selected garment and target item, ring model, sample project, or other personal action, plus the return location.
   - Success returns to that exact context with the selection intact. Cancel returns to the previous screen with the selection intact.
4. Session (FR-IDN-07, FR-IDN-08)
   - One session valid on Try It On, Studio, account and wallet pages.
   - Sign-out ends the session for all surfaces in that browser.
   - Signing in on another device never signs out an existing device.
5. Account settings (FR-IDN-11)
   - Language (FA/EN), theme (light/dark/system), account info, privacy.
   - Buyer profile, store profile and identity settings are separate places; none is a mandatory long form.
6. Phone number change (FR-IDN-10)
   - Requires verifying both old and new numbers; if the old number is lost, a support-assisted, audited process.
7. Deletion lifecycle (FR-IDN-12..15, Q10, Q11)
   - Requesting deletion shows: recovery end date (30 days), current token balance, explicit statement that the balance is forfeited after the window. User must confirm.
   - During the window: running generation jobs finish and results are kept; the account is restorable by signing in with the same number, which brings everything back exactly.
   - After the window: personal data deleted or anonymised; financial history kept anonymised; remaining tokens forfeited. No cash refund.
   - If a payment is still unresolved when the window ends, it is resolved and credited before forfeiture.

KEY ACCEPTANCE SCENARIOS:
- AC-IDN-1: guest presses «یه تن بزن» on a style → signs in → lands on the same style with the same target item.
- AC-IDN-2: guest picks a ring model → cancels sign-in → back in lobby with that model still highlighted.
- AC-IDN-3: signed in on Try It On → goes to landing → opens Studio → already signed in.
- AC-IDN-4: user with 120 tokens requests deletion → sees end date and "120 tokens will be lost after that".
- AC-IDN-5: deletion on day 1, sign-in on day 29 → everything restored.
- AC-IDN-6: too many code requests → refused with a wait time.
- Signing out on Studio signs the user out of Try It On in the same browser.

EDGE CASES:
- SMS never arrives from either provider.
- User changes number while a try-on is running.
- Carrier recycles a number to a new person (support path).
- Deletion requested while a payment is "unknown".
- Deletion requested while Studio cards are still processing.
- User signs in on two devices and requests deletion on one.

OUT OF SCOPE: wallet mechanics and forfeiture accounting (003); notifications about deletion beyond in-page messages (013); admin console access (001); buyer/store profile content (008, 012).

SUCCESS CRITERIA:
- 0 personal actions possible without sign-in; 0 browsing screens requiring it.
- 100% of sign-ins started from a personal action return to the origin with selection intact (automated test per origin type).
- OTP delivery failover proven in a drill where provider 1 is down.
- Deletion → restore within 30 days returns identical account state in tests; post-window deletion leaves no personal data and an anonymised ledger.
```

---

## 003 — Wallet, payments and token ledger

```text
/speckit.specify
Feature 003 — Tankhor wallet: one token wallet per account, IRR top-up through a domestic gateway, an immutable token ledger that makes double charges and double refunds impossible, price snapshots for confirmed units, history for users, and finance tools for admins.

SOURCES (read before writing):
@docs/tankhor-prd.md — E12 (FR-WAL-01..14, AC-WAL-1..3), FR-CON-22, FR-CON-27, §5.3 guardrails GR1–GR4, §5.5, §10.3, §11.2, §12.6, RK4, RK5, RK6
@docs/tankhor-technical-spec.md — §6 (P25, P26, ledger invariants L1–L9, quote snapshot), P36, U20, U24
@docs/tankhor-decisions-qa.md — Q11, Q13, Q24, Q29, Q36

RULES FOR THIS SPEC:
- WHAT and WHY only; no technology, tables or APIs. Express the ledger as business rules and invariants, not data structures.
- Q01–Q41 are final; [NEEDS CLARIFICATION: …] only for genuine gaps.
- Cite FR-WAL-* / AC-WAL-* / GR* / L* IDs.
- Respect OUT OF SCOPE strictly.

WHY:
Trust is the product's foundation: "no surprise charges" is a core principle. Every later feature spends tokens, so money must be provably correct before anything spends it. Finance must be able to report cash in vs recognised revenue vs unspent liability (Q24).

ACTORS: User, Payment gateway, Reconciliation process, Admin (finance), Generation features (as consumers of debit/return — specified here as rules, used later).

IN SCOPE:
1. Wallet and top-up (FR-WAL-01..07)
   - One wallet per account, spendable on both surfaces.
   - Top-up by preset packs or custom amount; IRR amount and token count always shown separately with units.
   - Discounts only as bulk-purchase tiers; generating more never earns a discount (R68).
   - Payment states: created, succeeded, failed, cancelled, unknown. Tokens are credited only after server-side verification with the gateway.
   - "Unknown" payments are resolved automatically by repeated verification; while one is unknown, the user is told so and is not nudged into paying again.
   - Repeated callbacks or verifications for one payment credit exactly once.
   - After top-up the user returns to the exact request or project they came from, inputs intact; any pending confirmation is still required.
2. Ledger rules (FR-WAL-08..14, L1–L9)
   - Balance always equals the sum of recorded events; corrections are new events, history is never edited.
   - Balance can never go negative.
   - Each confirmed unit (one try-on request or one Studio card) has at most one debit and at most one return.
   - Debit happens only on explicit user confirmation and together with creation of the unit to be generated — never one without the other.
   - Internal retries never charge again.
   - Token return for a failed unit is posted immediately after its final attempt (Q29).
   - Each confirmed unit stores the exact price it was confirmed at: price version, base price for its output type, style premium, add-ons (Q36). Later price changes never alter it.
   - A blocked account keeps its balance and cannot spend.
   - Admin simulations never write real events.
   - Forfeiture at final account deletion is its own event type (Q11).
3. Price confirmation rules (R66)
   - The price shown at confirmation must be the price charged; if it changed since display, fresh confirmation is required.
4. User history (FR-WAL-08)
   - Lists purchases, credits, debits, returns, admin adjustments and forfeiture with time, amount, status, and a reference to the related request, card or payment.
   - IRR and tokens never mixed in one figure. Users never see provider costs or keys.
5. Admin finance (FR-CON-22, FR-CON-27)
   - Manage packs, custom-amount bounds, bulk discount tiers, base price per output type (try-on, Studio image, Studio video), recolour add-on, per-style premiums — all versioned through the draft → approve → publish workflow from feature 001.
   - Balance adjustment: separate event with mandatory reason and confirmation (used e.g. for support goodwill returns, Q13).
   - Views: payment ledger, token ledger, returns, adjustments, forfeitures, unknown-payment queue, daily reconciliation status vs gateway settlements and vs cached balances.
6. Reporting basis (Q24 — accounting sign-off pending)
   - Cash in = verified payments. Recognised revenue = tokens spent on charged units minus returns. Unspent tokens = liability. Forfeited tokens reported separately.

KEY ACCEPTANCE SCENARIOS:
- AC-WAL-1: gateway calls back twice for one payment → exactly one credit.
- AC-WAL-2: browser closes after paying, before redirect → reconciler verifies and credits; history shows it.
- AC-WAL-3: blocked account with 50 tokens tries to confirm → refused; balance stays 50.
- Double-tap on a confirm → one debit (GR1).
- Unit fails after 3 attempts → one return equal to its debit, posted once (GR3).
- Price version changes between display and confirm → user must confirm the new price.
- Nightly check finds cached balance ≠ sum of events → alert and affected wallet frozen (GR4).

EDGE CASES:
- Gateway reports success after the app already showed "failed".
- Reconciler and callback arrive simultaneously.
- Return posted for a unit whose debit failed to record (must be impossible).
- Admin adjustment pushing balance below zero (refused).
- Forfeiture while an unknown payment is pending.
- Price version deleted while units confirmed at it still exist.

OUT OF SCOPE: what triggers debits and returns in UI flows (007, 010, 005 consume these rules); sign-in (002); reports dashboards beyond finance views (013); paid Share billing (012 OFF flag).

SUCCESS CRITERIA:
- GR1, GR2, GR3, GR4 hold at 0 under automated tests including random interleavings of duplicate callbacks, retries, returns and forfeitures.
- 14 consecutive days of zero reconciliation drift in beta.
- 100% of confirmed units carry a full price snapshot.
- Every top-up resolves to a terminal state; unknown payments resolve without user action.
```

---

## 004 — Media intake, consent, retention and private access

```text
/speckit.specify
Feature 004 — Tankhor media: validating uploads for both surfaces, consent for permanent personal photos, buyer photo gallery limits, Studio "save all uploads" retention, automatic expiry, private-by-default access, ticket-gated admin viewing, and the rule that outputs never depend on inputs.

SOURCES (read before writing):
@docs/tankhor-prd.md — FR-TRY-09..12, FR-LIB-09..11, FR-STO-01..04, FR-CON-23, FR-CON-25, §10.1, §10.4, §11.6, §12.4 NFR-SEC-02, 06, §12.5, GR5, GR6, RK9
@docs/tankhor-technical-spec.md — §4.4 (P06), §4.11 (P13), §5.6 (P18), §5.11 (P22), §10, U07, U14, U25, U34, U46
@docs/tankhor-decisions-qa.md — Q05, Q06, Q12, Q40

RULES FOR THIS SPEC:
- WHAT and WHY only; no storage technology, URLs or APIs.
- Q01–Q41 are final; [NEEDS CLARIFICATION: …] only for genuine gaps.
- Cite FR-* / AC-* / GR* IDs.
- Respect OUT OF SCOPE strictly.

WHY:
Personal photos are the most sensitive data Tankhor holds. Users must control them, admins must not browse them, and nothing must leak publicly. Clear upload limits rejected before payment avoid paying for unusable input. Sellers need a reusable library; buyers need instant photo reuse. Outputs must survive input expiry so results are never lost.

ACTORS: Buyer, Seller, Admin, System (expiry sweeps).

IN SCOPE:
1. Upload validation (Q05)
   - Max 15 MB; JPEG, PNG, WebP, HEIC; shortest edge ≥ 512 px. Values come from settings (001).
   - Validation by real file content, not file name. Violations produce a specific message (size / format / dimensions / corrupt) before any cost step.
   - Quality advice (low light, pose) is separate from technical rejection and never blocks.
   - Embedded metadata (including location) is removed from stored images.
2. Buyer personal photos (Q40, Q12)
   - Temporary by default, expiring after 14 days.
   - Permanent storage only after explicit consent with an AI-processing notice at first permanent save.
   - Up to 5 permanent photos; one may be default. A sixth requires choosing one to delete.
   - Deleting the default clears it or asks for an explicit replacement; never a random substitute.
   - A photo deleted while a job uses it is kept only for that job, then removed.
3. Studio uploads and library (Q06)
   - Per-seller setting "Save all uploaded photos", default ON.
   - ON: uploads join the permanent product library.
   - OFF: uploads made while OFF are temporary and expire 30 days after upload. Turning OFF never deletes existing library files.
   - The upload step tells the seller which retention applies to the file.
   - Library items carry apparel categories and tags (women, men, shoes, clothing, style), product type, in/out of stock.
   - Unassigning from a card or deleting a card never deletes a library file.
4. Independence of outputs (R16, R172)
   - Generated outputs are separate items. Expiry or deletion of any input never removes or breaks an output.
   - When an input referenced by a saved Studio assembly expires, the assembly shows that the slot needs a new image.
5. Privacy and access (FR-CON-23, GR5, GR6, Q12)
   - All personal photos, uploads and outputs are private by default; access is granted only to the owner, briefly, per request; nothing private is ever reachable via a permanent public address.
   - Admin viewing of a user's personal photos or try-on outputs requires linking a user report or support ticket; access is limited to that case and every view is logged with admin, time and ticket. Without a ticket, access is refused.
   - Viewing in the console never makes anything public.
6. Admin media oversight (FR-CON-25)
   - Admins can list files by owner, type, state, expiry and private/public, and see dependencies before deleting.

KEY ACCEPTANCE SCENARIOS:
- 16 MB photo → rejected "over 15 MB" before review; balance unchanged.
- HEIC 3024×4032 → accepted. 400×600 PNG → rejected for dimensions.
- Buyer declines consent → photo used once, expires in 14 days; any result made with it remains.
- Buyer with 5 permanent photos adds another permanently → must pick one to remove.
- Seller with setting OFF uploads on day 0 → file gone on day 31; outputs remain.
- Seller turns setting OFF → existing library unchanged.
- Admin opens a user's photo without a ticket → refused; with a ticket → shown and logged.

EDGE CASES:
- Upload interrupted mid-transfer.
- File with misleading extension.
- Photo containing no person (advice, not rejection, unless truly unprocessable).
- Retention setting lowered by admin while files exist (impact preview from 001).
- Same file uploaded twice.
- Account in deletion window: photos kept until final deletion.

OUT OF SCOPE: generation itself (005); try-on and Studio screens that use these files (007, 010); public publication of outputs (012); notification of expiry (013).

SUCCESS CRITERIA:
- 0 private files reachable without an owner/admin-authorised, time-limited grant (GR5).
- 0 admin views without a linked ticket (GR6); 100% of allowed views audited.
- 100% of limit violations rejected before any token cost.
- 0 outputs lost after input expiry in automated retention tests.
```

---

## 005 — Generation engine

```text
/speckit.specify
Feature 005 — Tankhor generation engine: the shared way every try-on request and Studio card is queued, processed by an external AI provider, retried, failed over, cost-capped, finalised, refunded on failure, and stopped in emergencies — plus the admin AI management that controls it.

SOURCES (read before writing):
@docs/tankhor-prd.md — FR-RES-02, 04, 07, 08, FR-GEN-02, 04..07, 12, FR-CON-28, FR-CON-36, FR-NTF-06, §10.1, §10.2, §11.1, §12.1 NFR-PERF-04, 06, §12.2, §12.3 NFR-AV-05, GR3, GR7, GR8, RK1, RK2, RK10, RK14
@docs/tankhor-technical-spec.md — §7 (lifecycle, state diagram), P37, P43, R45, R79–R82, R89
@docs/tankhor-decisions-qa.md — Q17, Q18, Q20, Q25, Q28, Q29, Q39

RULES FOR THIS SPEC:
- WHAT and WHY only; describe behaviour and states, not queues, workers or vendors' APIs.
- Q01–Q41 are final; [NEEDS CLARIFICATION: …] only for genuine gaps.
- Cite FR-* / GR* / Q* IDs.
- Respect OUT OF SCOPE strictly.

WHY:
Generation is where money meets an unreliable external dependency. Users must never pay for nothing, never get a fake result, and never lose work by leaving the page. The owner must survive provider outages and cost spikes without engineers.

ACTORS: Requesting feature (Try It On or Studio, on behalf of the user), External AI provider (primary and backup per capability), Admin, System.

IN SCOPE:
1. Generation unit
   - Unit = one try-on request (exactly one garment, Q39) or one Studio card (image or video, optional recolour, Q18).
   - A unit is created only at user confirmation, together with its debit (rules from 003). Pre-confirmation validation rejections never create a unit or a charge.
2. Lifecycle and states (§11.1): validating, rejected, queued, processing, retryable_error, succeeded, failed, returned.
   - Success is accepted only once the output is stored and linked to the unit.
   - Transient errors retry internally, up to 3 attempts total, at no extra cost (Q29).
   - After the last failed attempt the unit is failed and its token return is posted immediately (003 rule).
   - A unit has only one live attempt at a time; a late success from a superseded attempt is discarded and recorded.
   - Processing continues regardless of whether the user is on the page.
   - Users see only real states — no fake progress or ETA.
3. Capabilities and outputs (Q17, Q18)
   - Four capabilities: try-on, Studio image, Studio video, recolour.
   - Studio image output 2048 px long edge (WebP or JPEG); Studio video MP4 1080p, max 10 seconds; no watermark.
4. Providers and failover (Q25, FR-CON-28)
   - Each capability has a primary and one backup provider/model, configured by admins only. End users never choose.
   - Auto-fallback is ON at launch: when the primary is failing or unavailable, attempts route to the backup.
   - Connection tests for providers never charge users or create duplicate generation.
   - Per-attempt record of provider, model, duration, outcome class and cost.
   - Provider credentials are never displayed in full.
5. Cost control (R82, GR7)
   - Admin-set spend cap per period (overall and per capability).
   - Spend is checked before dispatch; at the cap, new dispatch stops automatically and admins are alerted; running attempts finish.
6. Emergency stop and maintenance (Q20, FR-CON-36, FR-NTF-06)
   - Admin can stop new generation globally or per surface (Try It On / Studio).
   - Running units finish and are delivered or refunded normally; new confirmations are refused with a clear message.
   - Stopping new generation never blocks access to existing results.
7. Outcome events for consumers
   - On each terminal state the engine emits an outcome the surfaces use for notifications and UI (success, failure + returned amount), never a success signal for a failed unit (GR8).
8. Capacity (Q28)
   - Must sustain the launch target of 1,000 units/day and pass a test at 2× with realistic image/video mix and peak-hour concentration.

KEY ACCEPTANCE SCENARIOS:
- Provider returns transient error twice then success → one debit, no return, success.
- Provider fails 3 times → failed, exactly one return, user sees "returned" only after it is recorded.
- Primary down → new attempts use backup; dashboard shows failover.
- Spend cap reached → new units refused, admins alerted, running units finish.
- Emergency stop on Studio only → Studio confirmations refused, try-on unaffected, running Studio cards finish.
- User closes the browser after confirm → unit still completes.
- Primary answers after the attempt was abandoned and retried on backup → first answer discarded; one output, one charge.

EDGE CASES:
- Output stored but marking success fails.
- Provider returns an output that violates the output spec (wrong size/length).
- Both primary and backup down.
- Cost cap changed while units are queued.
- Emergency stop lifted while queue has refused confirmations (users must reconfirm).

OUT OF SCOPE: what screens show during waiting and reveal (007, 011); notification delivery UI (013); provider-specific prompting quality tuning; price setting (003).

SUCCESS CRITERIA:
- GR3 and GR8 at 0 in fault-injection tests (timeouts, 5xx, slow success after timeout, malformed output, primary down).
- 100% of failed units refunded once; 0 refunded successful units.
- Failover and cost-cap drills pass per capability.
- Load test at 2× launch target passes with no lost units.
```

---

## 006 — Content, catalogue and publishing workflow

```text
/speckit.specify
Feature 006 — Tankhor content and catalogue: how the owner creates, imports, licenses, previews and publishes everything users see — garments and outfits for Try It On, models/styles/sample projects for Studio, page media and bilingual copy — plus the store allowlist and brand licences.

SOURCES (read before writing):
@docs/tankhor-prd.md — FR-CON-05..10, FR-CON-24, FR-CON-29..31, FR-DSC-13, FR-DSC-14, FR-LOB-05, FR-UX-14, §10.4, RK7, RK12
@docs/tankhor-technical-spec.md — P33, P38, P39, P40, P42, §9 data model (Style, Model, Campaign, AllowedHost, ContentVersion), U21, U22
@docs/tankhor-decisions-qa.md — Q07, Q19, Q26, Q31, Q34

RULES FOR THIS SPEC:
- WHAT and WHY only; no CMS product, storage or schema choices.
- Q01–Q41 are final; [NEEDS CLARIFICATION: …] only for genuine gaps.
- Cite FR-* / AC-* IDs.
- Respect OUT OF SCOPE strictly.

WHY:
Both surfaces are empty without curated, licensed content. The owner must manage it without code, at scale (hundreds or thousands of items), and without ever breaking the live site. Licensing must be enforced, not remembered.

ACTORS: Admin / content operator, Brand (future self-service), System.

IN SCOPE:
1. Content types and their fields (describe as business data, not tables)
   - Garment: bilingual name, brand, garment type, colour, store link (optional), licence reference, availability.
   - Outfit/style for Try It On: full-outfit image or short video, list of outfit parts, exactly one target item per try-on entry (Q39), tags (type, category, model, women/men, mood), related-style tags.
   - Studio model: images, attributes for filtering (age group, gender, skin tone, appearance), licence.
   - Studio style: media, model, attributes (product type, mood), optional premium surcharge (Q36), compatibility with models.
   - Sample project: styles + sample product images; sample products stay sample assets.
   - Page content: landing hero/slogan/intro/footer, Studio poster media, campaigns, menus, section order.
   - UI strings: every user-facing string in FA and EN, keyed by surface and situation.
2. Publishing workflow (FR-CON-05..10)
   - Every change starts as a draft; preview on mobile/tablet/desktop in FA and EN, light and dark; approval binds to the exact version; publish now or scheduled; version history; rollback as a new event.
   - Hiding or removing a section warns which user paths depend on it.
   - Missing translations are visible and block publish per policy.
3. Licensing (Q07, FR-DSC-13, FR-LOB-05)
   - Every public garment, model, style, sample and media asset references a recorded licence (holder, scope, expiry).
   - Items without a valid licence cannot be published; items whose licence expires are withdrawn from public view automatically. Existing user results that used them remain.
   - Brand names used as quality references in Studio never imply partnership.
4. Catalogue sourcing (Q07)
   - At launch: curated set entered by the Tankhor team.
   - Later: brand self-service via console and import (enable path designed, details can be minimal now).
5. Bulk import (Q26, FR-CON-30)
   - CSV files, one per entity (styles, models, garments, relations) plus a media archive.
   - UTF-8 with BOM so Persian survives editing in spreadsheet tools; stable IDs; FA/EN names; file references; relations as ID lists separated by "|".
   - Import validates into a staging area, shows per-row errors (missing file, duplicate ID, invalid relation, missing licence, missing translation), and publishes as one content version only when clean. Downloadable templates with examples. Import report kept.
6. Stores and allowlist (FR-CON-24, Q19, Q31)
   - Store records: name, link, status, licence for catalogue images.
   - Allowlist of onboarded store hosts used by buyer link ingestion (feature 007). Add/remove via the same draft → publish flow.
7. Ordering
   - Admin controls display order of gallery and Studio content; any data-driven ordering rules require explicit approval (no automatic ranking by default).

KEY ACCEPTANCE SCENARIOS:
- AC-CON-1: admin changes hero video and saves → live unchanged until preview, approval, publish.
- AC-CON-5: CSV with duplicate ID on row 12 → row error shown, nothing published.
- Garment without licence → cannot be published.
- Licence expires → garment disappears from gallery; a user's existing result with it still opens.
- English string missing for a new Studio button → publish blocked with a list of missing keys.
- Admin removes a store from the allowlist → buyer links from that host are refused from then on.

EDGE CASES:
- CSV saved without BOM (Persian corrupted) — detection and clear error.
- Media archive missing files referenced in CSV.
- Two drafts of the same page by different admins.
- Rolling back to a version that references a now-unlicensed asset.
- Style whose model is deleted.

OUT OF SCOPE: the gallery and Studio user screens (007, 009); generation (005); Share publication review of seller outputs (012); reports (013).

SUCCESS CRITERIA:
- 0 unlicensed items publicly visible.
- 100% of content changes reach live only via the approval workflow, each versioned and reversible.
- A 1,000-row CSV import with errors reports every error per row and publishes nothing until fixed.
- All UI strings exist in FA and EN before publish.
```

---

## 007 — Try It On: discovery, creation, fitting room, result

```text
/speckit.specify
Feature 007 — Tankhor Try It On core journey: public gallery and style detail, choosing exactly one garment, adding one personal photo, reviewing and confirming the price, waiting in the fitting room, and using the result.

SOURCES (read before writing):
@docs/tankhor-prd.md — §3.3 principles, §4 personas PS1–PS3, E3 (FR-DSC-01..14, AC-DSC-1..4), E4 (FR-TRY-01..20, AC-TRY-1..6), E5 (FR-RES-01..16, AC-RES-1..4), §9 J1, J2, §13 events for E3–E5, FR-UX-10, FR-UX-13
@docs/tankhor-technical-spec.md — §4.1–4.7 (P03–P09), §12.7 copy keys, F02, F03, F05, F18, F20, F21, U01–U07, U32, U35–U38
@docs/tankhor-decisions-qa.md — Q05, Q12, Q13, Q14, Q22, Q29, Q31, Q39

RULES FOR THIS SPEC:
- WHAT and WHY only; no frameworks, endpoints or component libraries.
- Q01–Q41 are final; [NEEDS CLARIFICATION: …] only for genuine gaps.
- Write user stories with priorities (P1 = must, P2 = should) and Given/When/Then scenarios; cite FR-* / AC-* IDs.
- Respect OUT OF SCOPE strictly.

WHY:
This is the product's first value moment: from "I like this" to "this is me wearing it" with one photo, no forms, and no surprise charges. If this journey is slow, confusing or untrustworthy, nothing else matters.

ACTORS: Guest, Signed-in buyer.

DEPENDS ON: 002 (sign-in with origin), 003 (prices, debit, return), 004 (uploads, consent, photo gallery), 005 (generation), 006 (catalogue, allowlist, strings).

IN SCOPE:
1. Gallery (FR-DSC-01..08)
   - Public, image-first, calm; Iranian models and licensed Iranian brand garments.
   - Card: full outfit as main image; the try-on-able target item as a thumbnail; independent actions: open detail, set aside, try on, go to store (only if a valid link exists).
   - Text search; filters for garment type, colour, brand, model, mood; compact panel.
   - No price, fabric, rating or commercial comparison anywhere.
   - Primary «یه تن بزن» action always within reach without covering content; it also opens standalone create.
   - Back from detail restores query, filters, category and scroll.
   - Empty state offers clearing filters; media errors offer retry; videos have posters.
2. Style detail (FR-DSC-09..12)
   - Large image, thumbnails of each outfit part, the target item marked by text and image, tags, category, model, brand, store link if valid, related styles by shared tags/categories.
3. Try-on creation (FR-TRY-01..12, FR-TRY-19, FR-TRY-20)
   - Exactly one garment and one photo per request.
   - From a detail: garment preselected; user adds or picks a photo.
   - Standalone create: garment from upload, from a link of an allowlisted store, from gallery, or from Set aside; already-valid choices are never asked again.
   - Links: only allowlisted hosts; other hosts refused with an upload option; fetched image shown for the user to confirm as the target; several candidate images → user picks one.
   - Never start on a guessed garment.
   - Photo: new upload (validation and consent from 004), one of the saved photos, or the default. Swappable before confirm.
   - One short flow, not several mandatory pages.
4. Review and confirm (FR-TRY-13..18)
   - Shows target garment, photo, token cost and balance beside confirm.
   - Nothing generated or charged before confirm; free campaigns show 0 and still need sign-in.
   - Shortfall shown exactly with a top-up path; return to the same review after top-up; confirm still required.
   - Price change since display → reconfirm. Repeated taps → one request, one charge.
5. Fitting room (FR-RES-01..08)
   - Request behind the closed curtain; pulling early triggers the hand-closes-curtain interaction.
   - Real states only; no fake percentage or ETA; screen-reader status; reduced motion.
   - User may leave; copy says the result will wait in Try-ons.
   - Success → in-app notice «بیا یه نظر بده!» linking to that result (Q22); result auto-saved to Try-ons.
   - Failure after final attempt → distinct "returning" then "returned" message with the amount; never a success notice.
6. Result (FR-RES-09..16)
   - Image with garment, brand, date, store link if valid, proportionate AI-generated/no-fit-guarantee notice.
   - Actions: zoom, download, regenerate (same inputs, new price confirmation), change outfit (keeps valid photo; expired photo needs a new pick), compare, share, delete, back to gallery, go to store.
   - Share is file-only via the device share sheet or download; never a link (Q14).
   - Deleting a result never deletes the photo or set-aside item.
   - Unwanted-but-successful results: no automatic refund; help explains support review (Q13).
7. Voice (FR-UX-10, FR-UX-13, FR-UX-16, FR-UX-18): warm old-bazaar seller, informal «تو», short sentences, no purchase pressure or fit promise; money and consent copy explicit. AI notice uses the fixed `ai_notice` text; Persian copy never uses «تن‌خور» for "fit" because it reads as the brand name; no claim from PRD §2.4.

KEY ACCEPTANCE SCENARIOS: include AC-DSC-1..4, AC-TRY-1..6, AC-RES-1..4 from the PRD verbatim in intent, plus:
- Guest presses «یه تن بزن» → signs in → returns to same style with same target.
- Balance 3, cost 5 → "2 tokens short" → top-up → same review → confirm → 5 tokens debited once.
- Non-allowlisted link → refused, upload offered, no fetch.

EDGE CASES: garment withdrawn while a user is on its detail; licence expired mid-flow; network drop after confirm; photo expired before regenerate; allowlisted store removed mid-flow; maintenance or emergency stop active at confirm.

OUT OF SCOPE: Try-ons gallery, compare, Set aside, My Photos, profile (008); landing and PWA (014); notification list (013); widget (015).

SUCCESS CRITERIA:
- A guest can go from gallery to confirmed try-on with sign-in in one uninterrupted flow on mobile.
- GR1, GR8, GR9 hold at 0 in end-to-end tests of this journey.
- Journey J1 and J2 pass on Android Chrome, iOS Safari and desktop Chromium in FA and EN, light and dark.
- KPIs K1, K2, K3, K4, K5 are measurable from emitted events.
```

---

## 008 — Buyer library

```text
/speckit.specify
Feature 008 — Tankhor buyer library: the Try-ons gallery, visual comparison of up to three results, the Set aside list «نشون‌کرده‌ها», My Photos (up to 5, one default), the optional height/weight profile, and buyer navigation.

SOURCES (read before writing):
@docs/tankhor-prd.md — E6 (FR-LIB-01..14, AC-LIB-1..4), FR-DSC-03, §10.1, §13 library events, FR-UX-10
@docs/tankhor-technical-spec.md — §4.8–4.13 (P10–P13, P45, P46), F06, F15, F17, F19, U08, U31, U33, U34
@docs/tankhor-decisions-qa.md — Q30, Q40, Q41

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-LIB-* / AC-LIB-*. Respect OUT OF SCOPE.

WHY:
Shoppers decide over time and across shops. They need their results, the clothes they're considering, and their photos in one calm place, and a simple way to see options side by side before leaving to buy.

ACTORS: Signed-in buyer; Guest (only for the sign-in prompt on save).

DEPENDS ON: 002, 004, 007.

IN SCOPE:
1. Try-ons («تن‌زده‌ها») (FR-LIB-01, 02)
   - All successful results until the user deletes them; no promotional subtitle; in-flight/failed items, if shown, clearly separated.
   - Actions per result: open, download, file share, delete, select for compare, go to store if purchasable.
   - Paging and back preserve scroll and compare selection.
   - Empty state invites discovery or creating a try-on.
2. Compare (FR-LIB-03..05)
   - 1 to 3 results from any stores; a 4th shows a limit message and keeps the 3.
   - Each item shows its own garment and store link only; never price, fabric, rating or commercial data.
   - On mobile every item reachable (scroll or tabs). Removing from compare never deletes a result.
   - Reachable from Try-ons and from a result; no duplicate menu.
3. Set aside («نشون‌کرده‌ها») (FR-LIB-06..08, Q30)
   - Keep garments for later, tried or not. Guest save → sign-in → that garment saved. Saving twice keeps one entry.
   - Open, try on, go to store, remove. Unavailable items show a clear state. Removing never deletes photos or results.
   - Separate from Try-ons and from Studio Saved.
4. My Photos (FR-LIB-09..11, Q40)
   - Up to 5 permanent photos, one default; temporary photos marked with remaining lifetime.
   - Adding a sixth permanent photo asks which to remove. Deleting the default clears it or asks for a replacement.
5. Profile (FR-LIB-12, 13, Q41)
   - Only height and weight, both optional; small yellow "!" badge with accessible name "Optional profile completion" until filled; no repeated pop-ups.
   - The UI must not claim these improve results unless the active provider uses them.
6. Buyer navigation (FR-LIB-14)
   - Explore, + Try It On, My Photos, Try-ons, Set aside, Profile, Account & wallet, Help.
   - Account icon and token balance always visible with a producible-count estimate based on real prices.
   - No link to Studio.

KEY ACCEPTANCE SCENARIOS: AC-LIB-1..4 plus:
- Set-aside garment tried on → appears in both lists; removing from Set aside keeps the result.
- Guest taps "set aside" → signs in → item saved; tapping again doesn't duplicate.

EDGE CASES: result whose brand store link became invalid; set-aside garment whose licence expired; deleting a photo that is a default and used by an in-flight job; very large Try-ons history (hundreds of items).

OUT OF SCOPE: creating try-ons (007); photo validation and consent mechanics (004); notification list (013).

SUCCESS CRITERIA:
- Compare never exceeds 3; never shows commercial data.
- 0 cases where removing a library item deletes another object.
- Profile and photo limits enforced as specified in automated tests.
- All screens pass FA/EN, light/dark and WCAG 2.2 AA checks.
```

---

## 009 — Studio lobby and step 1 selection

```text
/speckit.specify
Feature 009 — Tankhor Studio lobby and build step 1: the cinematic public lobby (poster, model ring, sample showcase), the three build entries with sign-in at build start, and step 1 where sellers select styles and models in one shared selection with live pricing.

SOURCES (read before writing):
@docs/tankhor-prd.md — §4 personas PS4, PS5, E7 (FR-LOB-01..11, AC-LOB-1..3), E8 (FR-SEL-01..13, AC-SEL-1..3), §9 J3, §10.1, §10.3, FR-UX-09, FR-UX-11
@docs/tankhor-technical-spec.md — §5.1–5.4 (P15, P16, P47, P48), F07, F08, F22–F24, F26, U11, U12, U41–U43, U52
@docs/tankhor-decisions-qa.md — Q08, Q15, Q34, Q36

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-LOB-* / FR-SEL-* / AC-* IDs. User stories with P1/P2 priorities and Given/When/Then scenarios. Respect OUT OF SCOPE.

WHY:
Sellers must feel they've walked into a professional agency and be able to explore quality before committing. Starting from any inspiration — a blank project, a model they like, or a sample — must be one tap, and cost must be visible from the first glance.

ACTORS: Guest seller, Signed-in seller.

DEPENDS ON: 002 (sign-in with origin), 003 (prices), 006 (models, styles, samples).

IN SCOPE:
1. Lobby (FR-LOB-01..07)
   - Order: cinematic poster → model ring → sample showcase.
   - Poster: rotating media, muted autoplay, pause control, poster fallback; models in it are not selectable.
   - Ring: rotate, focus/hover reveals a model's styles around it; touch and explicit controls including stop on mobile; reduced motion stops auto-rotation.
   - Showcase: sample projects; browse their image/video set on hover, focus or tap.
   - Create Project always reachable (small at top, more prominent on scroll); small account control; no full workspace sidebar over the lobby.
   - Professional voice («شما»).
2. Build entries and sign-in (FR-LOB-08..11, Q08, Q34)
   - Browsing is public; entering the build requires sign-in first with the origin preserved.
   - Create Project → step 1, Styles view, empty selection.
   - Ring model → step 1 with all of that model's styles preselected; seller can remove some or all and add others; count and estimated cost visible before continuing; picking costs nothing.
   - Sample project → step 2 with sample styles and sample product images (step 2 behaviour itself is feature 010; this feature hands off the preloaded state).
3. Step 1 selection (FR-SEL-01..13)
   - Opens on Styles; no "model or style?" interstitial. Styles and Models are two views on one shared selection.
   - Style cards: media, price (base for default output type + style premium), select, save, open detail.
   - Opening a style detail or model profile never selects or changes cost.
   - Style detail: three separate actions — select for project, save style, view model.
   - Model profile: select one, several or all styles; save model separately; "select all" states count and cost and never duplicates already-selected styles.
   - Filters: styles by product type and mood; models by age group, gender, skin tone, appearance; collapsible panel; removable active chips persist.
   - Summary: items, count, running total from first selection; prices visible without hover on touch.
   - Selection survives switching views, opening details/profiles, visiting Saved and coming back.
   - Only compatible style–model combinations offered.
   - Project limit 50 cards shown before it is reached; selection beyond is blocked with explanation (Q15).
   - Next → step 2 with valid selections.
   - No public likes or ratings; views, detail opens, selections are recorded separately for internal analytics.

KEY ACCEPTANCE SCENARIOS: AC-LOB-1..3, AC-SEL-1..3 plus:
- Guest picks sample → cancels sign-in → back in lobby with that sample highlighted.
- Seller opens 10 style details → nothing selected, total unchanged.

EDGE CASES: model with no styles; style becomes unlicensed while selected; price version changes during selection (totals refresh with notice); 48 selected then "select all" on a 5-style model; poster media fails.

OUT OF SCOPE: step 2 assembly and Create (010); generation (011); Saved management screens (012); content entry (006).

SUCCESS CRITERIA:
- All three entries land in the right step with the right preloaded state after sign-in in automated tests.
- 0 implicit selections from viewing.
- Selection persists across all navigation in step 1.
- Lobby passes reduced-motion and WCAG 2.2 AA pause/stop checks.
```

---

## 010 — Studio step 2 assembly and Create

```text
/speckit.specify
Feature 010 — Tankhor Studio build step 2: assigning product images to style cards, duplicating cards for more products or colours, choosing image or video and recolour per card, Back/Undo/auto-draft, and the Create confirmation that debits tokens and starts generation.

SOURCES (read before writing):
@docs/tankhor-prd.md — E9 (FR-ASM-01..19, AC-ASM-1..5), §9 J3, J4, §10.1, §10.3, §11.3, FR-UX-11, FR-UX-13
@docs/tankhor-technical-spec.md — §5.5, §5.6 (P17, P18), F09, F10, F25, F29, U13–U16, U44, U45, U48
@docs/tankhor-decisions-qa.md — Q05, Q06, Q15, Q18, Q32, Q33, Q36

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-ASM-* / AC-ASM-*. User stories with priorities. Respect OUT OF SCOPE.

WHY:
This is where sellers save the most time: upload a product once and get many variants, with exact, visible cost and no way to be charged for something incomplete.

ACTORS: Signed-in seller.

DEPENDS ON: 003 (debit, price snapshot), 004 (upload validation, library, retention setting), 009 (selection handoff).

IN SCOPE:
1. Cards and slots (FR-ASM-01..04)
   - "Step 2 of 2"; there is no step 3.
   - One card per style instance, one product slot, one output.
   - Empty slot covered by the velvet curtain; tapping offers Choose from library or Upload new image; no URL input.
   - Curtain stays open after assignment; closes again when the image is removed.
   - Sample-project entry: cards start with sample images.
2. Output type and price (FR-ASM-05, 06, Q18, Q36)
   - Each card chooses image or video; recolour is an optional per-card add-on.
   - Card price = output-type base + style premium + recolour add-on (if on). Each card shows its price; project total always visible.
3. Card actions (FR-ASM-07..10)
   - Replace image, remove image only, delete card with its assignment, duplicate, set exact colour (value + swatch). Each named clearly.
   - Duplicate = same model, pose, settings; empty image slot.
   - One upload may be assigned to many cards, each with its own colour.
   - 50-card limit applies to duplication.
   - Removing images or cards never deletes library files.
4. Navigation and recovery (FR-ASM-11..14, Q32)
   - Back → step 1 keeping images, colours, copies and selections; returning shows compatible assignments intact; removing a style that affects filled cards shows the effect before applying.
   - Undo reverts at least the last edit on this page (remove image, delete card, replace image); Undo never navigates.
   - Save project → Saved → Assembled projects (no expiry).
   - Leaving without saving keeps an auto-draft for 30 days from last activity; expiry never touches library files or saved projects.
5. Create (FR-ASM-15..18, Q33)
   - Any empty card → Create disabled; empty cards highlighted with a count.
   - All cards filled → confirmation with output count, per-card breakdown (type, premium, recolour), total, balance, and an optional "Add another style?" path.
   - Confirm with enough balance → one debit per card (rules from 003) and generation starts (005); navigation to the generation page (011).
   - Shortfall → top-up → back to the exact assembly; confirm still required. No direct project payment.
   - Price change since display → reconfirm.
6. Uploads (FR-ASM-19): same limits as buyers; retention per the seller's setting (004), stated at upload.
7. Voice: professional («شما»), precise verbs, itemised money copy. Studio `ai_notice` (review before use; no fidelity or sales guarantee); no claim from PRD §2.4 (FR-UX-16).

KEY ACCEPTANCE SCENARIOS: AC-ASM-1..5 plus:
- Card with video + recolour on a premium style shows base(video) + premium + recolour.
- Confirm double-tap → one debit per card.
- Seller removes in step 1 a style that has 3 filled cards → warned before applying.

EDGE CASES: library image deleted elsewhere while assigned; temporary upload expires before Create; balance changes in another tab; 50 cards reached by duplication; price version changes mid-assembly; network loss during Create.

OUT OF SCOPE: generation page and Projects (011); Saved management screens (012); upload validation internals (004).

SUCCESS CRITERIA:
- 0 projects created with empty cards (GR9 equivalent for Studio).
- Totals always equal the sum of card prices in property tests across random card configurations.
- Auto-draft recoverable at day 29 and gone at day 31 in tests.
- One-upload-five-colours scenario completes with five priced outputs.
```

---

## 011 — Studio generation page and Projects

```text
/speckit.specify
Feature 011 — Tankhor Studio generation and Projects: the per-card reveal page after Create, honest progress, per-output and final notifications, and the two-group Projects area (In Production, Completed) with outputs browsable by project, model or style.

SOURCES (read before writing):
@docs/tankhor-prd.md — E10 (FR-GEN-01..13, AC-GEN-1..3), §11.3, §10.2, FR-UX-11
@docs/tankhor-technical-spec.md — §5.7, §5.9 (P19, P20), F11, F27, U17, U49
@docs/tankhor-decisions-qa.md — Q16, Q17, Q22, Q29

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-GEN-* / AC-GEN-*. Respect OUT OF SCOPE.

WHY:
Sellers want the first good output as soon as it exists, not when the slowest card finishes, and they need to trust that failures are reported honestly and refunded.

ACTORS: Signed-in seller.

DEPENDS ON: 005 (engine and outcome events), 010 (Create).

IN SCOPE:
1. Generation page (FR-GEN-01..07)
   - Separate page after confirm (not a build step); curtain covers each output in production.
   - Per-card state: queued, processing, ready, failed.
   - Tapping an unfinished curtain: opens slightly, Tankhor logo flashes, hand closes it; optional text "Generating output"; no fake outputs; screen-reader status; reduced motion.
   - Ready outputs revealed and downloadable immediately; real counter "N of M outputs ready".
   - Leaving never stops generation; project reachable from Projects.
   - Failed card: its own tokens returned (rule from 003/005); others unaffected.
   - In-app notice per ready output and one final notice when all cards are final; with failures the final notice shows the split ("6 ready, 2 failed, 24 tokens returned"), never "all ready".
2. Projects (FR-GEN-08..11, Q16)
   - Exactly two groups: In Production, Completed; switchable in-page; reachable from the sidebar.
   - A project moves to Completed when all cards are final; partly/fully failed projects show a result summary; zero-success projects visibly flagged.
   - Project detail: outputs, per-card state, downloads, tokens debited and returned, Share actions (Share flow itself is 012).
   - Outputs browsable by project, model or style; outputs carry categories and tags.
3. Outputs (FR-GEN-12, 13, Q17)
   - Images 2048 px long edge (WebP/JPEG), video MP4 1080p ≤ 10 s, no watermark.
   - Outputs persist regardless of input retention.

KEY ACCEPTANCE SCENARIOS: AC-GEN-1..3 plus:
- Seller closes the page after confirm, returns via Projects → sees current per-card states.
- All cards succeed → final notice "All project outputs are ready."

EDGE CASES: every card fails; emergency stop while some cards are queued (queued ones still run — they were confirmed); output downloaded while project still in production; very large project (50 cards) rendering.

OUT OF SCOPE: Share submission and review (012); notification list UI (013); engine internals (005).

SUCCESS CRITERIA:
- First ready output visible without waiting for others in 100% of multi-card tests.
- 0 "all ready" notices for projects with failures.
- Downloaded files match output spec in automated checks.
```

---

## 012 — Studio library, Saved, store profile, Share and publication review

```text
/speckit.specify
Feature 012 — Tankhor Studio assets and publishing: product library management, the Saved area (models, styles, assembled projects), the Studio dashboard/sidebar, the store profile, Share to Tankhor with admin review and immediate withdrawal, and paid Share billing built but OFF.

SOURCES (read before writing):
@docs/tankhor-prd.md — E11 (FR-STO-01..13, AC-STO-1..4), FR-CON-26, §11.4, §10.4, K9
@docs/tankhor-technical-spec.md — §5.8, §5.10–5.13 (P21–P24, P49), P35, F12, F28, F29, U18, U19, U46, U47, U50, U51
@docs/tankhor-decisions-qa.md — Q06, Q19, Q35

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-STO-* / AC-STO-* / FR-CON-26. Respect OUT OF SCOPE.

WHY:
Sellers come back to reuse assets and setups; they need them organised. Share closes the two-sided loop: seller content becomes shopper inspiration — but only after review, and with seller control to withdraw instantly.

ACTORS: Signed-in seller, Admin reviewer.

DEPENDS ON: 004 (library storage and retention), 006 (publishing into public gallery), 011 (outputs).

IN SCOPE:
1. Product library (FR-STO-01..04): upload in bulk or gradually; categories and tags; product type; in/out of stock; edit metadata; delete with dependency notice; retention setting visible (default ON).
2. Saved (FR-STO-05, 06): three independent tabs — Models, Styles, Assembled projects. Saved assemblies resume in step 2; saved models/styles open selection or detail. Removing a save never deletes projects or library files. Invalid references show a clear state and replacement option; generation can never start with an invalid reference.
3. Studio dashboard (FR-STO-07): one sidebar — Projects (In Production, Completed), Saved, Library, Wallet, Account; subtle on lobby, explicit in workspace; mobile collapsible with same destinations; navigation preserves work.
4. Store profile (FR-STO-08, Q19): store name, store link, related info; one store per account; never required to create projects; small completion badge.
5. Share to Tankhor (FR-STO-09..13, Q35)
   - Submit one specific image or video from the generation page or a completed project; one output never publishes the whole project.
   - Requires store name + valid link, checked at review; seller warned if missing.
   - States: not submitted → pending review → approved (public) / rejected (with reason) → withdrawn.
   - Cancel before approval prevents publication.
   - Withdraw after publication hides the output from all public display immediately; the dialog says copies already downloaded by others cannot be recalled.
   - Free at launch; paid Share billing fully built behind its OFF flag (001), with a test invoice path.
6. Publication review (FR-CON-26)
   - Admin queue with output, target item, store, licence status; approve or reject per output with a reason; history kept; separate from generation.
   - Approved outputs appear in the public Try It On surface in the place defined by content rules (006).

KEY ACCEPTANCE SCENARIOS: AC-STO-1..4 plus:
- Seller cancels a pending Share → never becomes public.
- Admin rejects with reason → seller sees the reason on that output.
- Paid Share flag ON in test → seller sees price and is charged once; OFF → free.

EDGE CASES: Share submitted for an output whose style licence later expires; store link becomes invalid after approval; seller deletes account while an output is published (withdrawn on final deletion); same output submitted twice.

OUT OF SCOPE: generation (011); public gallery placement design (006/007); reports (013).

SUCCESS CRITERIA:
- 0 outputs public without approval.
- Withdrawal effective on the next public request in 100% of tests.
- Saved items independent of each other and of projects/library in deletion tests.
- Paid Share path passes ON and OFF tests.
```

---

## 013 — Notifications, help, maintenance, reports and operations

```text
/speckit.specify
Feature 013 — Tankhor operations and communication: in-app notification centre, help content, maintenance banners, owner reports and dashboards, user management (block, deactivate), backups and restore, and mandatory operational alerts.

SOURCES (read before writing):
@docs/tankhor-prd.md — E13 (FR-NTF-01, 03, 05, 06), FR-CON-20, FR-CON-21, FR-CON-35, FR-CON-38, §5 (north star, K1–K10, GR1–GR9, §5.4, §5.5), §12.3, §12.9, §13.3 dashboards, §16.4
@docs/tankhor-technical-spec.md — P27, P29, P31, P32, P43, P44, §13, U24, U26
@docs/tankhor-decisions-qa.md — Q20, Q22, Q24, Q27

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-* / K* / GR* IDs. Respect OUT OF SCOPE.

WHY:
Users must find out when their work is ready and when parts of the service are down. The owner must see whether the product works (KPIs), whether money is right (guardrails), and must be able to recover from disaster within committed targets.

ACTORS: User (buyer/seller), Admin, System.

DEPENDS ON: 001, 003, 005, 007, 011.

IN SCOPE:
1. In-app notifications (FR-NTF-01, Q22)
   - Types: try-on ready, Studio output ready, Studio project final summary (with failure split), tokens returned, Share approved/rejected.
   - Each deep-links to its subject; a notification list keeps them for later; read/unread state.
   - Only in-app at launch (external channels are feature 015).
2. Help (FR-NTF-03): public help content in FA and EN for both surfaces, including how pricing, returns, deletion/forfeiture and privacy work, and how to reach support (a visible contact channel; ticketing is OFF).
3. Maintenance and stops (FR-NTF-05, 06, Q20): banners stating which surface is affected and what still works; independent maintenance for Try It On and Studio; during an emergency stop, clear "new generation paused" messaging while running work completes.
4. Reports (FR-CON-20, §5, Q24)
   - North star (weekly successful generations used) and K1–K10 with definitions, window, refresh time and lineage.
   - Guardrails GR1–GR9 as live indicators with breach alerts.
   - Finance: cash in, recognised revenue, unspent-token liability, forfeited tokens, AI cost at invoice-date FX, estimated profit with method shown.
   - Catalogue per garment and brand: views, detail opens, try-ons, store clicks. Studio: model/style views vs selections vs real use.
   - Dashboards: product funnels, quality, catalogue, Studio usage, finance, operations (PRD §13.3).
5. User management (FR-CON-21): search; view account, store, projects, ledger; temporary block (duration, reason) vs permanent deactivation; block keeps balance and stops spending; everything audited.
6. Backups and recovery (FR-CON-35, Q27): daily full backups plus continuous change capture; recovery point 1 hour, recovery time 4 hours; backups kept 30 days; restore drills recorded with results.
7. Mandatory alerts (FR-CON-38): cost cap reached, provider down/failover, SMS delivery degraded, unknown-payment backlog, ledger drift, backup failure, guardrail breach. Routed to admins; never silenced by user notification settings.

KEY ACCEPTANCE SCENARIOS:
- Try-on succeeds while user browses → notice appears; tapping opens the result; later visible in the notification list.
- Project with 2 failures → final notice shows split.
- Studio maintenance on → Studio banner shown, Try It On unaffected.
- AC-CON-6: restore drill restores within 4 hours with ≤ 1 hour data loss.
- Ledger drift in nightly check → alert raised and wallet frozen.
- Admin blocks a user for 7 days → user cannot spend; balance unchanged; block lifts automatically.

EDGE CASES: notification for an item the user later deleted; burst of 50 per-output notices for one project (grouping must stay readable); KPI definitions changed (history of definitions); restore requires replaying payments that arrived after the restore point.

OUT OF SCOPE: push/SMS/email and ticketing (015); event capture inside each surface (each feature emits its own events); AI provider config (005).

SUCCESS CRITERIA:
- Every KPI and guardrail in PRD §5 visible with definitions.
- Restore drill meets RPO 1 h / RTO 4 h.
- 100% of listed alert conditions fire in drills.
- Notifications deep-link correctly in 100% of tested types.
```

---

## 014 — Landing and PWA

```text
/speckit.specify
Feature 014 — Tankhor landing page and installable app: the curtain intro and two-surface choice on the single domain, and PWA installation and offline behaviour on the target platforms.

SOURCES (read before writing):
@docs/tankhor-prd.md — E1 (FR-LND-01..12, AC-LND-1..4), E16 (FR-PWA-01..05), FR-UX-09, FR-UX-15..18, §12.1 NFR-PERF-01, 02, §12.7
@docs/tankhor-technical-spec.md — §2, §2.1 (P01), §11.3, F01, F13, F30, U30, U53
@docs/tankhor-decisions-qa.md — Q01, Q02, Q04, Q37, Q38

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-LND-* / FR-PWA-* / AC-LND-*. Respect OUT OF SCOPE.

WHY:
The landing page is the brand's first impression and the only switch between the two experiences. Installation makes Tankhor feel like an app on phones, without promising what the web can't do.

ACTORS: Visitor, Signed-in user.

DEPENDS ON: 006 (editable hero content), 007 and 009 (destinations).

IN SCOPE:
1. Landing (FR-LND-01..12)
   - Logo, dark velvet-brown curtain, sky or owner-chosen media behind it; scrolling draws the curtain aside, reveals the name Tankhor, slogan above.
   - Two large tiles below: «یه تن بزن» → gallery with matching transition; «استودیو» → Studio lobby.
   - Optional animated preview per tile on hover/focus; explicit tap equivalent on touch.
   - Short visual intro of both services; footer with contact channel and legal links.
   - Never wait for animation or video to navigate; reduced-motion version; deep links skip the intro.
   - All surfaces on one domain with paths (Q01); Try It On never links to Studio.
   - Brand and motion assets from the in-house designer (Q04) with reduced-motion stills.
   - Hero, slogan, intro and footer editable via 006. The slogan is not approved yet: it comes only from the content panel, never hard-coded; the shared landing keys and `landing_ai_notice` are in Technical Spec §12.7.
   - Logo brief starts from direction A (curtain gap); until the designer delivers, the name is rendered as text, never as a stand-in logo.
2. PWA (FR-PWA-01..05, Q37)
   - Installable on Android Chrome, iOS Safari (Add to Home Screen) and desktop Chromium; Firefox works as a website.
   - Install is never required; installed app opens signed-in and restores work.
   - No offline generation; no notification permission implied by install.
   - Offline: clear offline state; never shows a purchase or generation as successful without server confirmation; clean recovery when back online.
3. Performance targets (proposed, to confirm in alpha): landing and gallery LCP ≤ 2.5 s at p75 on a mid-range phone over 4G; INP ≤ 200 ms.

KEY ACCEPTANCE SCENARIOS: AC-LND-1..4 plus:
- iOS Safari user adds to home screen → opens signed in on last surface.
- Device goes offline during top-up redirect → no success shown until confirmed online.

EDGE CASES: hero video blocked by data-saver; user switches language on landing (layout mirrors, tiles keep meaning); installed app after account deletion request (restore path via sign-in).

OUT OF SCOPE: gallery and lobby internals (007, 009); push notifications (015).

SUCCESS CRITERIA:
- Journey F01 passes on all target platforms in FA/EN, light/dark, reduced motion.
- PWA install and reopen verified on each target platform.
- Performance targets met or formally revised at end of alpha.
```

---

## 015 — Built-but-OFF capabilities

```text
/speckit.specify
Feature 015 — Tankhor capabilities that must be fully built, integrated and tested at launch but shipped disabled: partner store try-on widget and API, percentage rollout, per-user usage limits, ticketing, and external notification channels (push, SMS, email).

SOURCES (read before writing):
@docs/tankhor-prd.md — §0.4 (definition of P0-OFF), §6.2, E15 (FR-WID-01..07, AC-WID-1..2), FR-CON-33, FR-CON-34, FR-NTF-02, FR-NTF-04, RK8, RK11, RK13, AS5
@docs/tankhor-technical-spec.md — §11.1 (P28), P41, P42, §7.2, F31, U27, U55
@docs/tankhor-decisions-qa.md — Q21, Q22

RULES FOR THIS SPEC:
- WHAT and WHY only. Q01–Q41 final. [NEEDS CLARIFICATION: …] only for genuine gaps. Cite FR-* / AC-* IDs. Respect OUT OF SCOPE.
- "OFF" means: real, end-to-end behaviour exists and is tested in both states; when OFF, users see nothing of it (no dead buttons), and APIs answer "not enabled".

WHY:
These capabilities shape identity, wallet, consent and messaging. Building them now avoids redesigning core flows later; shipping them OFF keeps launch risk low and lets the owner turn each on deliberately with impact preview (001).

ACTORS: Partner store, Shopper on a partner site, Admin, Signed-in user.

DEPENDS ON: all previous features.

IN SCOPE:
1. Partner widget and API (FR-WID-01..07, Q21)
   - A partner embeds Try It On on its product pages for its own garments.
   - Shopper signs in to Tankhor with SMS OTP and pays from their own Tankhor wallet, including top-up, within the partner flow.
   - Sign-in and top-up must work when browsers block third-party cookies (use a separate window or full redirect, not an embedded-only session).
   - Partner configuration: credentials, allowed domains, garment reference; stores pay nothing per try-on.
   - Results also appear in the shopper's Try-ons.
   - A sample partner store passes the full flow and the OFF state.
   - Not white-label Tankhor; no Studio API.
2. Percentage rollout (FR-CON-33)
   - Admin releases a change to a percentage of accounts; assignment is stable per account; rollback tested.
3. Per-user usage limits (FR-CON-34)
   - Configurable caps (e.g. generations per period) with clear user messages; designed to counter abuse (RK13).
4. Ticketing (FR-NTF-04)
   - Users open tickets from help or a result; admins handle them; tickets can be linked when an admin needs to view private media (004 rule).
5. External notifications (FR-NTF-02, Q22)
   - Push, SMS, email for the same notification types as in-app (013).
   - Explicit opt-in per channel; opt-out at any time; installing the app grants nothing.
   - Delivery contracts per channel before enabling.
6. Enablement
   - Each capability has its flag (001), impact preview, health check and an audited enable/disable history.

KEY ACCEPTANCE SCENARIOS: AC-WID-1, AC-WID-2 plus:
- Rollout at 10% → same accounts stay in the group across sessions; rollback restores all.
- Usage cap 5/day enabled → 6th confirm refused with clear message; balance untouched.
- Ticket opened → admin can link it to view the user's photo; view logged.
- User opts into push → receives try-on ready push; opts out → stops.
- Every capability OFF → no visible UI, APIs answer "not enabled".

EDGE CASES: partner domain not on allowed list; shopper without balance in widget; flag turned OFF while a widget session is active; usage cap lowered mid-day; notification channel provider down.

OUT OF SCOPE: paid Share billing (012); any change to core decisions (Q21 stands until the owner revisits it).

SUCCESS CRITERIA:
- Each capability passes automated ON and OFF tests end to end.
- 0 visible traces of OFF capabilities in production UI.
- Widget flow passes with third-party cookies blocked on Safari and Chrome.
```
