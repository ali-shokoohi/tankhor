# Test-First Validation Contract — Feature 001

This is a test plan, not test implementation or passing evidence. Preserve the exact names
below in later tasks/tests. Every implementation task must follow its driving test task,
with the intended failure observed before production code. Commit messages retain AC/FR/Q IDs.

Layers: **U** pure unit/proptest; **M** mocked port interactions; **F** transactional fake
behaviour; **C** same fake/real contract suite; **I** real adapter integration; **H** HTTP/
Askama template; **E** browser E2E. Testcontainers PostgreSQL and wiremock are the only
external integration systems in tests; no live SMS/payment/AI vendor calls.

## 1. Complete acceptance-to-test map

| Acceptance | Exact named test | Layers | PRD trace |
| --- | --- | --- | --- |
| AC-ADM-001 | console_rejects_non_admin_access | H,E | FR-CON-01; Q01 |
| AC-ADM-002 | console_requires_completed_second_factor | M,F,I,H,E | FR-CON-02; Q23 |
| AC-ADM-003 | end_user_session_does_not_grant_admin_access | F,H | FR-CON-01–02; NFR-SEC-05 |
| AC-ADM-004 | admin_addition_requires_another_admin_and_audit | M,F,C,I,H | FR-CON-02–04/37 |
| AC-ADM-005 | admin_recovery_preserves_second_factor_requirement | F,I,H,E | FR-CON-02–03/37 |
| AC-ADM-006 | last_admin_lockout_has_no_bypass | F,H | FR-CON-02–03; Q23 |
| AC-ADM-007 | admins_have_equal_permissions_and_distinct_attribution | F,H | FR-CON-04/37 |
| AC-ADM-008 | initial_settings_have_decided_values_and_complete_metadata | U,F,C,I,H | FR-CON-32; FR-UX-01 |
| AC-ADM-009 | setting_rejects_values_outside_declared_policy | U,M,H | FR-CON-32; FR-UX-08 |
| AC-ADM-010 | saving_setting_creates_draft_only | F,C,I,H | FR-CON-05/32; Q32 |
| AC-ADM-011 | preview_uses_exact_draft_without_live_effects | M,F,H,E | FR-CON-06/09/11 |
| AC-ADM-012 | shorter_lifetime_previews_existing_item_impact | U,M,F,H | FR-CON-32; P41; owner policy |
| AC-ADM-013 | settings_cannot_weaken_security_or_financial_integrity | U,M,F,H | FR-CON-02/32/37 |
| AC-ADM-014 | editing_approved_draft_invalidates_approval | U,F,C,I | FR-CON-05–07 |
| AC-ADM-015 | concurrent_setting_edits_do_not_silently_overwrite | F,C,I,H | FR-CON-05/07/37 |
| AC-ADM-016 | approved_change_publishes_only_at_authorised_time | F,C,I,H,E | FR-CON-05/07/32/37 |
| AC-ADM-017 | scheduled_publish_rejects_invalidated_approval | F,C,I | FR-CON-05/07 |
| AC-ADM-018 | repeated_publication_has_one_effect | F,C,I | FR-CON-05/37 |
| AC-ADM-019 | rollback_creates_new_version_and_preserves_history | F,C,I,H,E | FR-CON-05/08/37 |
| AC-ADM-020 | urgent_foundation_change_still_requires_review | M,F,H | FR-CON-05/07/10/32 |
| AC-ADM-021 | foundation_actions_have_complete_attributed_audit | M,F,C,I | FR-CON-03–05/37 |
| AC-ADM-022 | admin_can_search_foundation_audit | F,C,I,H | FR-CON-01/37 |
| AC-ADM-023 | audit_entries_cannot_be_edited_or_deleted | C,I,H | FR-CON-37; NFR-SEC-09 |
| AC-ADM-024 | audit_excludes_authentication_secrets_and_private_data | U,I,H | FR-CON-37; NFR-SEC-03; §13.1 |
| AC-ADM-025 | unauditable_admin_action_is_not_reported_successful | M,F,I,H | FR-CON-03–05/37 |
| AC-ADM-026 | foundation_screens_support_both_languages_and_all_themes | H,E | FR-CON-11; FR-UX-01–02 |
| AC-ADM-027 | language_and_theme_switch_preserve_unsaved_input | H,E | FR-UX-03 |
| AC-ADM-028 | persian_direction_and_digits_preserve_identifiers | H,E | FR-UX-01/04–05 |
| AC-ADM-029 | foundation_brand_and_contrast_follow_approved_baseline | U,H,E | FR-UX-06–07/17; Q02–Q03 |
| AC-ADM-030 | foundation_tasks_are_keyboard_and_assistive_accessible | H,E | FR-UX-08–09 |
| AC-ADM-031 | foundation_copy_has_fa_en_parity_and_context | U,H | FR-UX-01/14 |
| AC-ADM-032 | foundation_tasks_work_without_optional_enhancements | H,E | FR-CON-05; FR-UX-08–09 |
| AC-ADM-033 | six_launch_flags_start_off | U,F,C,I,H | FR-CON-32; §6.2 |
| AC-ADM-034 | flag_changes_use_versioned_review_and_history | F,C,I,H | FR-CON-05–08/32 |
| AC-ADM-035 | failed_or_unknown_health_blocks_enablement | M,F,H | FR-CON-32 |
| AC-ADM-036 | unbuilt_capability_cannot_be_enabled_by_flag | F,H,E | FR-CON-32; §0.4/6.2 |
| AC-ADM-037 | settings_accept_inclusive_bounds_without_changing_defaults | U,F,C,I | FR-CON-05/32; owner bounds |
| AC-ADM-038 | upload_format_policy_requires_jpeg_and_known_formats | U,F,C,I,H | FR-CON-32; Q05; owner policy |
| AC-ADM-039 | card_limit_preview_preserves_existing_projects | U,F,H | FR-CON-32; Q15; owner policy |
| AC-ADM-040 | photo_limit_preview_preserves_existing_photos | U,F,H | FR-CON-32; Q40; owner policy |
| AC-ADM-041 | upload_policy_preview_applies_only_to_new_uploads | U,F,H | FR-CON-32; Q05; owner policy |
| AC-ADM-042 | attempt_limit_preview_preserves_existing_units | U,F,H | FR-CON-32; Q29; owner policy |
| AC-ADM-043 | recovery_revokes_all_admin_sessions_and_old_factors | M,F,C,I,H,E | FR-CON-02–03; NFR-SEC-05 |
| AC-ADM-044 | scheduled_change_blocks_when_live_baseline_changes | F,C,I | FR-CON-05–07/32 |
| AC-ADM-045 | overdue_publication_rechecks_prerequisites_and_runs_once | M,F,C,I,E | FR-CON-05–07/32/37 |
| AC-ADM-046 | recovery_window_decrease_keeps_pending_end_dates | U,F,C,I,H | FR-CON-32; P41; OI8 |
| AC-ADM-047 | visual_identity_values_have_one_source_and_checked_contrast | U,H,E | FR-UX-20/07 |

The existing spec maps all 38 local FR requirements to these 47 ACs. This plan retains those
links; no renaming or replacing a named test with a weaker snapshot-only assertion.

US2 establishes OI8's pure/preview contract. After Publish exists, US3 proves that the
published recovery-window policy is new-requests-only and preserves an existing end-date
fixture; it does not implement account deletion, expiry scheduling or other owning contexts.
SC-009 covers all six policy groups including this exception.

Tool installation, harness configuration, fixture startup and SQLx metadata preparation are
**setup**, never failing business tests. Resolve those prerequisites before running a red
behavioral assertion. Each split implementation has a focused earlier test; service splits
have mock and fake drivers. Existing seed membership is rerun as a US6 regression, not
reimplemented to manufacture red. An already-green acceptance check needs no code change.

## 2. Additional driving tests by layer

### Workspace and pure domain

Before a Cargo workspace exists, write/run `workspace_skeleton_requires_inherited_lints`
and `architecture_policy_rejects_forbidden_context_edges` in the repository-check harness;
observe missing structure/policy failures. Then add the minimum manifests/toolchain/config.
Every domain type still needs its own failing Rust test before implementation.

- `uuid_v7_ids_reject_wrong_version`, `locale_direction_is_derived`, `clock_contract`.
- `settings_accept_inclusive_bounds_without_changing_defaults` proptest on all nine ranges,
  outside/wrong-kind cases; no arithmetic overflow or implicit units.
  `upload_format_policy_requires_jpeg_and_known_formats` checks all subsets
  and unknown formats. `draft_hash_is_canonical` tests sorted sets/UTC/time edits.
- `editing_approved_draft_invalidates_approval`,
  `rollback_creates_new_version_and_preserves_history`,
  `shorter_lifetime_previews_existing_item_impact` with original-reference-time, known/unknown
  and clock boundary cases. Preview rules must not execute future-context operations.
- In the existing approval-domain driver: `impact_comparison_ignores_assessment_time`,
  `changed_destructive_impact_requires_reapproval`, `unavailable_impact_blocks_review`.
   Reuse the same cases in the pure typed impact-policy tests, immediate and scheduled mock/fake
  workflows: later assessment time alone preserves eligibility, changed comparable fields
  require new preview/approval, unavailable required evidence blocks. See application-ports
  §4 for exact fields/decision order; no TTL or additional population capability is added.

### Application mocks/fakes and port contracts

- U3 extends existing drivers only: T109/T110 drive durable monotonic admin-epoch policy;
  T123–T126 and the unchanged shared-suite PostgreSQL runner T205/T206 require every
  committed admin revocation/deactivation/recovery to atomically advance the epoch and
  reactivation/restoration never to reset/reuse it. Check separate committed invalidation
  then restoration, fresh reads, recovery, stale CAS/non-increasing invalidation refusal, rollback and
  overflow failing closed. These repository cases test persistence, not worker orchestration.
  T366/T367 drive pure captured/current-epoch comparison; T384/T385 drive
  `deactivated_then_reactivated_scheduler_cancels_publication`,
  `revoked_then_restored_scheduler_cancels_publication` and
  `unchanged_scheduler_epoch_executes_publication`. With due time, active restored eligibility,
  current approval and all other gates valid, any epoch mismatch cancels with visible reason,
  one scheduler/system-worker/approver audit and no live/success effect; unchanged epoch
  executes once. Preserve the same cases in T396's existing real publication harness.
- Checker modes: the original traceability driver accepts planned links with unfinished
  future symbols, checks planned-artifact/driver/completion links for completed work only,
  and rejects missing required planned artifacts or preceding-driver evidence for that work.
  No actual-symbol port inventory is required in partial mode. Final rejects any missing actual port symbol, runner, direct mock
  interaction or real/fake feature graph. Test valid partial, premature-completion, invalid
  final and complete-final fixtures. The full actual-symbol check runs at the final gate.
- `totp_provisioning_is_once_authenticated_and_no_store` belongs to the original
  BeginTotpEnrollment mock/fake drivers and existing enrollment HTTP driver. Assert matched
  authenticated target/stage/browser/session/epoch proof, one atomic presentation winner,
  `Cache-Control: no-store`, no redisplay on repeat/concurrency/refresh or through query/error
  paths, and no log/audit/bundle/URL/other delivery. No other UI secret is permitted.
  SessionStore's same fake/real contract covers the conditional presented marker; a lost
  response needs a fresh superseding ceremony, never redisplay of the same secret.
- Run every suite in [application-ports.md](application-ports.md) against its named fake
  and real implementation. A CI inventory checks that no port lacks either runner.
- `unit_of_work_contract` covers cross-repository rollback, drop isolation and wrong-store
  handles; `audit_failure_aborts_access` covers access; publication atomicity
  is separately checked by `publication_and_audit_commit_together` after Publish exists.
- `validation_does_not_invoke_publish_or_vendor`, `failed_second_factor_never_issues_session`,
  `recovery_only_revokes_target_admin`, `recovery_races_with_login_safely_state_contract`.
- `sms_failover_reuses_challenge_and_expiry` covers unchanged abuse budgets, same challenge
  on failover and zero sends for invalid/non-admin identity; no public registration.
  `sms_acceptance_is_not_primary_proof` covers transport versus identity proof.
- BootstrapAdmin: `bootstrap_admin_uses_ports_and_one_audited_uow` and
  `bootstrap_admin_creates_restricted_eligibility_not_recovery_or_access` precede service
  implementation before real storage. `initial_provisioning_is_operator_authorized_not_public_registration`
  subsequently tests only CLI parsing/config/delegation with the completed service.
- GetEnrollment: `get_enrollment_validates_authorization_and_primary_proof_without_writes` and
  `get_enrollment_refuses_invalid_state_and_returns_safe_choices` precede the service and
  enrollment HTML route. Read protected AdminRepository authorization and matching verified
  phone-OTP proof from SessionStore. Validate stage/browser/session/purpose/target/current
  epoch and expiry, reject missing/consumed/revoked state,
  expose only safe choices and assert no mutations, token issuance or full session.
- `passkey_consumption_is_single_use_under_race_state_contract` and
  `audit_failure_aborts_access_state_contract` belong to the original passkey mock/fake
  drivers, alongside credential CAS/counter persistence, replay and recovery/audit rollback
  cases in their original TOTP/recovery drivers; no later repair slice. Real race acceptance
  remains separate. SessionStore's shared fake/real contract also covers all pending proofs
  and encrypted full WebAuthn/TOTP ceremony state; there is no AuthStateRepository contract.
- Complete safe audit fields and audit-failure rollback are driven in each original
  passkey/TOTP/enrollment/invitation/recovery/SaveDraft/Preview/Approve/SchedulePublish/
  Publish/Rollback mock/fake pair, before service code. Preserve named `audit_*_has_complete_safe_fields_once`
  cases: actor/action/subject/time/reason/allowlisted before-after/factor where applicable,
  scheduled/actual times for publication, one event and no unaudited success. Aggregate,
  OTP-event and real-storage audit acceptance remain separate.
- Readiness is pure domain policy with delivered/pass/fail/unknown/provenance cases and a
  code-owned registry marking all six absent capabilities not delivered; no runtime health
  override. Test it before Preview/Approve/Publish consumers, not in a later repair pass.
- Each of ListAdmins/ListSettings/ListFlags/GetSetting/GetVersion/GetChange/GetPublication
  has a separate mock/fake driver for current-admin authorization, exact key/ID or bounded
  cursor, missing subject, safe returned state and zero writes. GetChange includes draft head/
  review/baseline; GetPublication includes pending/blocked/cancelled/completed and scheduled/actual times.

### PostgreSQL integration

- Extend existing SchedulePublish and ScheduledPublish drivers, shared publication contract
  and original PostgreSQL runner, not new task slices: persist scheduling admin/captured
  epoch and approver, execute on behalf of that admin, validate current approval, and cover
  `revoked_scheduler_cancels_publication`, `recovered_scheduler_cancels_publication`,
  `deactivated_scheduler_cancels_publication`, `cancelled_schedule_requires_different_admin`
  and `scheduled_cancellation_is_audited_once_or_rolled_back`. Check a scheduler different
  from approver, active happy path, valid admin/invalid approval (blocked), invalid admin even
  with unavailable evidence (cancelled), recovery versus finalization under the same admin
  lock, terminal retry, new linked reschedule by another admin and no live/success event.
  Cancellation/success audit uses actor = scheduling admin, executed_by = system worker,
  approver recorded and a visible cancellation reason. Cancellation audit failure commits
  neither cancellation nor live effect and reports no unaudited success.
- Existing pure publication-time and schedule-form drivers test Asia/Tehran default/local→UTC,
  valid Europe/Berlin neighboring times, 2026-03-29 02:30 gap and 2026-10-25 02:30 overlap
  rejection, past schedules and ISO 8601 explicit-offset JSON decoding (`Z`, positive/negative offset;
  naive/malformed rejected). Use explicit Clock fixtures and pinned embedded IANA rules;
  set now to 2026-01-01T00:00:00Z for Berlin gap/overlap tests so past-time rejection cannot
  mask the intended invalid-local-time failure. Test past-time rejection separately.
  Apply rejection to new/revised input, not already-authorized overdue work. No JSON route
  is added; this is a pure input/DTO format contract.
- `runtime_cannot_update_delete_or_truncate_audit`, `audit_guards_reject_privileged_dml`,
  `runtime_cannot_disable_guards_or_assume_owner`,
  `runtime_cannot_rewrite_registry_policy_or_version_history` (including no history cascade).
  Use restricted runtime connections; separately exercise trigger protection.
- `registry_seed_and_draft_commit_with_audit_or_not_at_all` (one-winner draft CAS), `aba_live_version_invalidates_approval`,
  `unrelated_subject_change_preserves_approval`, `publication_and_audit_commit_together`.
- `worker_and_http_publish_commit_one_effect`, `stale_publication_state_rejects_stale_worker`,
  `crash_before_commit_leaves_no_live_change`, `lost_commit_ack_resolves_by_publication_id`,
  `overdue_effect_is_not_backdated`, `unknown_prerequisite_persists_blocked_reason`.
- `otp_and_totp_consumption_are_single_use_under_race`, `recovery_races_with_login_safely`,
  `seed_rerun_preserves_published_values`, `sqlx_cases_get_distinct_migrated_databases`.

The original publication contract/service drivers include concurrent retries, stale
conditional finalization, ABA/unrelated baselines, blocked reasons, actual times and bounded
stable-ID unknown-outcome resolution. The original PostgreSQL runner repeats that shared
contract plus independent-connection concurrency cases; no later SQL repair pairs.

Owner-approved fixtures share labelled, detached PostgreSQL across the nextest run;
`#[sqlx::test]` and the canonical workspace migrator create a fresh database per test. Close
all extra pools before cleanup. Successful SQLx cleanup is best effort and must be tested;
failed databases remain until the same case is rerun or infrastructure removed. No broad
SQLx cleanup sweep runs from nextest children. Serialize separate local runs reusing a server
because SQLx database names derive from test paths. MinIO startup/readiness and bucket
fixtures (including panic cleanup) move to feature 004; retain the Compose service, but
claim no MinIO or storage-behavior coverage in 001.

Fixture-driving tests (owner planning clarification 2026-11-04; FR-CON-37/NFR-SEC-09 integration
isolation and constitution Principle I):
- `postgres_fixture_startup_is_idempotent_and_waits_for_readiness`,
  `fixture_roles_are_idempotent_and_runtime_is_not_owner`,
  `setup_exports_only_matching_tests_ready_endpoints`,
  `fixture_ownership_rejects_foreign_resources`.
  Lifecycle helper binaries use fixture_bootstrap_* names and the dedicated fixture-bootstrap
  profile, with no shared setup or fallback launcher. Implement the export wrapper after
  ownership/startup/role tests are green and the export driver's intended failure is observed;
  run the live-run lease driver/implementation, then activate and smoke-test shared setup
  only after both export and lease are green. Assertions belong to the
  existing `nextest_profiles_select_storage_tests_only` and endpoint-export test tasks.
- Owner-approved nextest sequencing correction (2026-11-06): extend the existing T008 driver
  to cover fast-only, bootstrap-present and complete genuine-target inventories, and load/list
  all four profiles in the real workspace. Retain final default/ci exclusions, disjoint
  storage selectors, no bootstrap setup and bounded concurrency assertions. Reject package/
  binary predicates matching no current target; inactive storage profiles select `none()`.
  T009 installs scaffolding only; T013 adds bootstrap selectors/exclusions/group assignment
  alongside the first real bootstrap driver, before its red run. T021 adds
  `shared_fixture_exports_owned_ready_postgres` in
  `crates/testkit/tests/integration_shared_fixture.rs` alongside integration selectors,
  exclusions/group assignment and shared-hook activation, after bootstrap/export/lease are
  green. The smoke check verifies the inherited password-free endpoint's ownership and ready
  PostgreSQL connection through tested helpers, without startup/fallback or new policy in
  its body. T032 requires both complete final selection checks and real bootstrap/shared-smoke
  execution. Empty inactive profiles or probe-only success are never fixture coverage; no
  future adapter or Phase 2 test is pulled into Phase 1.
- `sqlx_cases_get_distinct_migrated_databases` covers all pools, failed-database retention
  until same-case rerun and successful cleanup after closing app pools;
  `local_shared_server_rejects_concurrent_independent_runs`.
- `fallback_runner_sets_only_child_environment` covers same-test execution, child status,
  discovery/non-test/bootstrap pass-through without fixture startup, child-only endpoints
  and container-guard lifetime until the child exits.
- `ci_cleanup_matches_only_current_run_labels` (including active-run refusal),
  `setup_exports_only_matching_tests_ready_endpoints` (including secret-free exports).

See [integration-fixtures.md](integration-fixtures.md) for nextest 0.9.151 setup/profile,
connection-budget, retention, safe plain-Cargo/IDE runner and teardown contracts.

### Crypto and wiremock integration

- `sms_sender_contract_kavenegar` and `sms_sender_contract_sms_ir` cover approved POST
  templates/parameters, sensitive header/key-path handling, acceptance/rejection/timeout/
  malformed responses, redirects refused and key-path/body redaction for both real adapters.
  `sms_failover_reuses_challenge_and_expiry` covers no second valid code on timeout/failover;
  no real HTTP vendor calls.
- `webauthn_rejects_wrong_origin_and_replayed_state` and
  `valid_zero_counter_passkey_is_supported` include stale-state rejection and credential
  update results; `console_requires_completed_second_factor` covers persisted updates with
  the shared `admin_authenticator_contract_real_crypto` runner using real
  locally generated signed assertions, not only mocked success.
- `totp_fallback_rejects_replayed_or_expired_proof` covers consumed enrollment steps;
  `totp_wrong_envelope_key_fails_closed`, `secret_envelope_binds_admin_and_epoch`,
  `primary_proof_is_browser_bound_and_one_use` cover protected key/state and expiry cases.

### HTTP/templates/OpenAPI/browser

- `csrf_is_required_for_every_mutating_console_form`,
  `sign_in_page_is_localized_no_store_and_uses_safe_return_target`,
  `passkey_begin_form_escapes_public_options_and_never_exposes_server_state` cover CSRF,
  unsafe-return refusal, escaped options and auth no-store/non-reflection behavior.
- `csp_allows_only_local_console_scripts`, `oversized_body_never_reaches_application`,
  `concurrency_limit_rejects_excess_and_releases_permits`: independent HTTP red/green pairs,
  with authenticated and anonymous paths, HTML/JSON responses and cancellation cases.
- `fake_foundation_shell_and_health_start_without_network`: foundational app smoke under
  outbound-network denial, no storage/provider credentials and no management stubs.
- `openapi_snapshot_matches_router`, `all_json_routes_are_documented_under_v1`,
  `health_and_problem_responses_match_schema`, `schema_endpoint_describes_itself_finitely`,
  `offline_openapi_export_needs_no_database`, `production_docs_and_assets_require_admin`,
  `swagger_makes_no_external_asset_or_validator_requests`.
- `swagger_initializes_under_strict_csp_without_external_requests` is one additional
  assertion in the existing `console_locale_theme_keyboard_and_axe_matrix` E2E task, not
  another task: load protected /api/docs through real sign-in, prove Swagger initialization,
  same-origin schema retrieval/rendered health operation, and zero CSP violations/off-origin
  attempted requests in both backend projects. Capture violations and attempted requests
  before navigation; do not relax CSP or bypass authorization.
- Instantiated template tests, not only source grep: one h1, labels/alt/focus/errors, all
  Fluent keys, six locale/theme choices, RTL identifiers/digits, unrounded contrast and
  preference switching with unsaved forms. Browser tests additionally cover keyboard,
  reduced motion, reflow, no-htmx/no-JS TOTP and missing passkey scripting without bypass.
- `J5_foundation_safe_change_and_recovery`: real composition/Postgres/crypto with SMS real
  HTTP adapters pointed at wiremock by an isolated test harness; prepare/publish harmless
  upload-limit change, find audit, rollback, recover an admin and reject old sessions.
  No pricing/AI-incident claims. Physical iOS Safari/Android Chrome checks remain release
  evidence in addition to Playwright Chromium/WebKit/Firefox and axe.

Install pinned Playwright and axe, configure isolated fake/real projects and prove the
test-only fixture plumbing before these browser assertions. Tooling and missing harness
failures are setup blockers. Browser-driven UI corrections are separate focused pairs for
focus return, rendered contrast and reflow; do not mix npm configuration with web changes.

## 3. Guardrail ownership — explicit future gates

These required tests are **not implemented or satisfied by 001**. Keep them in owning feature
plans and in full-product release-readiness checks; never add empty tests to make the gate green.

| Guardrail | Required future named test | Owning feature |
| --- | --- | --- |
| GR1 | GR1_one_debit_per_confirmed_unit | 003/005 |
| GR2 | GR2_one_credit_per_verified_payment | 003 |
| GR3 | GR3_final_failure_has_exactly_one_recorded_return | 003/005 |
| GR4 | GR4_cached_balance_matches_ledger | 003 |
| GR5 | GR5_private_media_never_has_permanent_public_url | 004 |
| GR6 | GR6_private_admin_view_requires_linked_ticket | 004 |
| GR7 | GR7_ai_cost_cap_stops_new_work | 005 |
| GR8 | GR8_failed_unit_never_notifies_success | 005/013 |
| GR9 | GR9_generation_requires_confirmed_garment_and_price | 005/007/010 |

Foundation AC-ADM-013 prevents a settings bypass; it does not prove the underlying money,
media or generation invariant. J1–J5/full GR coverage remains a product-release blocker.

## 4. Completion evidence and CI

- T460–T462 rerun the original flag policy/workflow assertions as regression validation;
  they do not promise new production code or require manufactured red. A genuinely new
  flag-specific case must be named and test-driven before any additional production change.
  AdminAuthenticator has one real port adapter (T224); T216/T218/T220/T222 are component
  dependencies, not independent adapters requiring separate copies of the shared suite.
Require observed red-to-green evidence, all applicable layers, shared contract runners,
architecture negatives, current SQLx metadata and offline OpenAPI parity before marking
later tasks complete. Kernel coverage ≥90%; future wallet/generation/identity same floor
when implemented. Mutation testing applies to delivered wallet/generation, not a fabricated
empty module. Proposed other-crate/mutation thresholds stay proposals.

Every retained checked-query adapter includes the completion criterion **sqlx metadata
prepared and offline check passes before green**, on the current migrated isolated database;
there is no separate later metadata task. The final two metadata setup/check
tasks verify complete-workspace freshness; they do not introduce the first query metadata.

Default/ci profile filters exclude `integration_*` and `fixture_bootstrap_*` binaries;
fixture-bootstrap selects only the latter without shared setup, then integration selects
only the former after its setup dependencies pass. Fast HTTP/wiremock suites may run in ci without Docker. CI documents present
feature scope and fails full-product releases with absent contexts/journeys; it never counts
absent tests as passing. All fixture secrets/phone inputs are synthetic and remain out of logs.
