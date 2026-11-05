# Tankhor — agent rules

## Read first
- Product: docs/tankhor-prd.md · Behaviour detail: docs/tankhor-technical-spec.md · Decisions (final): docs/tankhor-decisions-qa.md · Stack and architecture: docs/tankhor-rust-implementation-pack.md
- Active feature: .specify/feature.json → specs/<feature>/spec.md, plan.md, tasks.md

## Sources of truth
- Read docs/README.md first.
- Only files marked "authoritative" in docs/README.md decide.
- Never read docs/fa/ for decisions.
- Never create versioned copies of documents.
- Older versions exist only as git tags.

## How you work
1. TDD always. Write the failing test, run it, see it fail for the right reason, then write the minimum code. Never write production code first.
2. One task from tasks.md at a time. Tick it only when its tests pass.
3. Run `cargo nextest run -p <crate>` after every change; run integration tests (`cargo nextest run --profile integration -p infra-postgres`) when touching adapters or SQL.
4. Never weaken a test to make it pass. If a test is wrong, explain why in the commit message.

## Architecture rules
- Context crates (admin, identity, wallet, media, generation, catalog, tryon, studio, notify, reporting): domain/, application/, ports/ only. No sqlx, axum, aws-*, reqwest.
- New persistence = new method on a repository port + fake in testkit + Postgres impl in infra-postgres + contract test for both.
- Transactions spanning repositories use UnitOfWork.
- Tokens and Irr newtypes only; never raw integers for money.
- `#[cfg_attr(any(test, feature = "mocks"), mockall::automock)]` goes ABOVE `#[async_trait::async_trait]`.
- HTML: askama templates, htmx partials, CSS logical properties, every string from Fluent (fa + en).
- JSON: only under /api/v1 via OpenApiRouter with #[utoipa::path]; problem+json errors; regenerate docs/openapi.json.

## Never
- Never mark an unbuilt feature as done or ship a stub behind a flag.
- Never change a decided value (Q01–Q41) without the owner.
- Never log phone numbers, OTPs, keys, photos or payment data.
- Never call a vendor HTTP API from a test; use wiremock.

## Commands
- Unit + mock + HTTP: `cargo nextest run`
- Integration: `cargo nextest run --profile integration` (needs Docker)
- Coverage: `cargo llvm-cov nextest`
- Mutation (wallet): `cargo mutants -p wallet`
- OpenAPI: `cargo run -p app -- openapi > docs/openapi.json`
- Local stack: `docker compose -f deploy/compose.yml up -d`
