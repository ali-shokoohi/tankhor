# Tankhor (تن‌خور) — agent rules

These rules apply in every session and every Spec Kit command. If anything here conflicts with the constitution (`.specify/memory/constitution.md`), the constitution wins — stop and report the conflict.

## 0. Precedence over global rules

This file overrides `~/.config/opencode/AGENTS.md` for this repository wherever they differ:
- The stop report (section 9) and red/green test evidence (section 2) are mandatory, even under "no recap / no closing summary".
- Quiet flags and head/tail are fine, but always show the failing test's assertion/panic lines and the final pass/fail summary.
- Code-graph tools (codebase-memory-mcp) are preferred for navigation. After editing files, confirm the index is fresh before relying on graph results; for files created in this session, read them directly.
- For structural questions (callers, call chains, change impact, routes, dead code), delegate to @graph instead of grep+read loops.

## 1. Sources of truth

1. Read `docs/README.md` first. Only files marked **authoritative** there decide anything:
   `docs/tankhor-prd.md` · `docs/tankhor-technical-spec.md` · `docs/tankhor-decisions-qa.md` · `docs/tankhor-rust-implementation-pack.md`.
2. Active feature: `.specify/feature.json` → `specs/<feature>/` (`spec.md`, `plan.md`, `tasks.md`, `contracts/`, `data-model.md`). During implementation, `tasks.md` order and wording are authoritative.
3. Never read `docs/fa/` for decisions (derived translations). `docs/brand/brandbook.html` is input; UI follows tech spec §12, not the brand book directly.
4. Never create versioned copies (`*-v2.md`, `*-old.md`, `*-final.md`). Older versions exist only as git tags.
5. Decisions Q01–Q41 are final. Never change a decided value. If sources disagree or something is missing: **stop and report**; do not choose or guess.

## 2. TDD protocol (non-negotiable)

1. One task from `tasks.md` at a time, in order.
2. **Red:** write the test only, run the narrowest command (`cargo nextest run -p <crate> <test>`), and show the failure. It must fail for the intended missing behaviour. A missing tool, wrong path or broken fixture is not valid red — fix the setup and re-run.
3. **Green:** minimum production code to pass; re-run that test, then the crate's full suite.
4. **Refactor** only while green.
5. Tick `[X]` in `tasks.md` only after the task's own evidence exists. Never tick ahead.
6. Never weaken, delete, rename or `#[ignore]` a test to get green. If a test looks wrong, stop and explain.
7. Guardrail tests carry their GR ID in the test name (e.g. `gr1_double_tap_debits_once`).

## 3. Architecture rules

- **Crates.** Context crates (`admin`, `identity`, `wallet`, `media`, `generation`, `catalog`, `tryon`, `studio`, `notify`, `reporting`) and `kernel` contain `domain/`, `application/`, `ports/` only — no `sqlx`, `axum`, `aws-*`, `reqwest` or vendor crates (`cargo deny` enforces it). Adapters live in `infra-*`. `web` and `worker` call application services only. `app` is the only composition root.
- **Ports.** Every dependency on storage, time, ids, randomness, crypto challenges or any external service is a port:
  ```rust
  #[cfg_attr(any(test, feature = "mocks"), mockall::automock)]
  #[async_trait::async_trait]
  pub trait SomeRepository: Send + Sync { /* … */ }
  ```
  `automock` always goes **above** `async_trait`.
- **Required per port:** mockall mock · in-memory fake in `testkit` (feature `fakes`) · shared contract suite in `testkit` run against the fake and every real adapter · real adapter in `infra-*`.
  **Order:** the contract suite passes on the fake **before** any real adapter is written; mock and fake tests for a service come **before** that service's implementation.
- **Transactions** spanning repositories go through `UnitOfWork`.
- **Time and ids:** never `Utc::now()` / `Uuid::now_v7()` in domain or application code — use `Clock` and `IdGenerator`.
- **Adapter mode:** one application-wide mode, `real` or `fake`; never mixed. Production startup must reject fakes; release builds never include the `mocks`/`fakes` features.
- **Money:** `Tokens` and `Irr` newtypes only; never raw integers; no implicit conversion.
- **Dependencies:** never add a crate that `plan.md` doesn't list — stop and ask.

## 4. Web, API and brand

- **HTML:** axum + askama + htmx with progressive enhancement (every interaction also works as a full-page request). `lang`/`dir` from locale; CSS logical properties; every string from Fluent with keys in both `fa` and `en`. No CDN assets.
- **JSON:** only under `/api/v1`, registered via `OpenApiRouter` with `#[utoipa::path]`; errors as `application/problem+json`; regenerate `docs/openapi.json` (the only file under `docs/` you may write).
- **Security:** CSRF on every form and htmx request; strict CSP; bounded request bodies; private media only via short-lived signed URLs.
- **Visual values** come only from `crates/web/design/tokens.toml`; no hex colours anywhere else; contrast is tested against WCAG 2.2 AA.
- **Naming:** the product is **Tankhor / تن‌خور** (with ZWNJ). Never "Taneh"/«تنه». Use «تن‌خور» only as the brand name, never as a common noun or a description of how clothes fit.
- **Copy:** Try It On voice is warm, informal «تو»; Studio voice is professional «شما». No fit or size promises. No emoji in finance, consent, deletion or error copy.

## 5. Data, secrets and tests

- Never log or store in plain text: phone numbers, OTP codes, keys, tokens, photos, payment data. The only secret ever shown in UI is the one-time TOTP enrollment secret/QR (authenticated ceremony, `no-store`, never logged, never re-displayable).
- Tests never call real external services: use fakes or `wiremock`.
- Integration fixtures: one shared Postgres per nextest run via setup script; `#[sqlx::test]` gives each test its own database; process-scoped testcontainers fallback when `DATABASE_URL` is unset. Follow `contracts/integration-fixtures.md`.
- After any change to checked SQL: `cargo sqlx prepare --workspace` before green.

## 6. What you must not do

- Edit `spec.md`, `plan.md`, `contracts/`, `data-model.md`, the constitution or `docs/` during implementation (only tick checkboxes in `tasks.md`). Spec problems → stop and report with task ID.
- Write stubs, placeholders, `TODO`/`FIXME`, `todo!()`, `unimplemented!()`, dead code, or ship anything unbuilt behind a flag.
- Commit, tag, push or rewrite git history — the owner reviews and commits.
- Ask a product question and then guess the answer — stop and ask.

## 7. Stop conditions

Stop and report immediately when: the requested scope is finished; a red fails for the wrong reason and can't be fixed within the task; sources conflict or information is missing; a new dependency is needed; or the session is getting long (finish the current red/green pair cleanly first).

## 8. Commands

| Purpose | Command |
| --- | --- |
| Format | `cargo fmt --check` |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` |
| Unit / mock / fake / HTTP | `cargo nextest run` (or `-p <crate>`) |
| Integration (Docker) | `cargo nextest run --profile integration` |
| Dependency rules | `cargo deny check` |
| SQL metadata | `cargo sqlx prepare --workspace` |
| OpenAPI | `cargo run -p app -- openapi > docs/openapi.json` |
| Coverage | `cargo llvm-cov nextest` |
| Mutation | `cargo mutants -p wallet` · `cargo mutants -p generation` |
| Local stack | `docker compose -f deploy/compose.yml up -d` |

Toolchain: rustc 1.98.1 · cargo-nextest 0.9.151 (pinned; setup scripts are experimental) · cargo-deny 0.20.2 · sqlx-cli 0.9.0.

## 9. Report format at every stop

1. Tasks completed (IDs). 2. Evidence: task · test · red reason · green ✓. 3. Files changed by crate. 4. Checkpoint command results. 5. Deviations (should be none). 6. Open questions/blockers. 7. Next task ID.
