# Test-First Validation Contract — Feature 001

This is a test plan, not test implementation or passing evidence. Preserve the exact names
below in later tasks/tests. Every implementation task must follow its driving test task,
with the intended failure observed before production code. Commit messages retain AC/FR/Q IDs.

Layers: **U** pure unit/proptest; **M** mocked port interactions; **F** transactional fake
behaviour; **C** same fake/real contract suite; **I** real adapter integration; **H** HTTP/
Askama template; **E** browser E2E. Testcontainers PostgreSQL/MinIO and wiremock are the only
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

The existing spec maps all 37 local FR requirements to these ACs. This plan retains those
links; no renaming or replacing a named test with a weaker snapshot-only assertion.

## 2. Additional driving tests by layer

### Workspace and pure domain

Before a Cargo workspace exists, write/run `workspace_skeleton_requires_inherited_lints`
and `architecture_policy_rejects_forbidden_context_edges` in the repository-check harness;
observe missing structure/policy failures. Then add the minimum manifests/toolchain/config.
Every domain type still needs its own failing Rust test before implementation.

- `uuid_v7_ids_reject_wrong_version`, `locale_direction_is_derived`, `fixed_clock_advances_explicitly`.
- `numeric_setting_bounds_are_inclusive` proptest on all nine ranges, outside/wrong-kind cases;
  no arithmetic overflow or implicit units. `format_subsets_require_jpeg` checks all subsets
  and unknown formats. `draft_hash_is_canonical` tests sorted sets/UTC/time edits.
- `new_revision_cannot_reuse_old_approval`, `rollback_does_not_erase_history`,
  `unknown_impact_is_not_zero`, `lifetime_preview_uses_original_reference_time` with clock
  boundary cases. Preview rules must not execute future-context operations.

### Application mocks/fakes and port contracts

- Run every suite in [application-ports.md](application-ports.md) against its named fake
  and real implementation. A CI inventory checks that no port lacks either runner.
- `uow_rollback_discards_all_repository_writes`, `uow_drop_does_not_publish_working_state`,
  `wrong_store_tx_handle_is_rejected`, `audit_failure_aborts_access_and_publication`.
- `validation_does_not_invoke_publish_or_vendor`, `failed_second_factor_never_issues_session`,
  `recovery_only_revokes_target_admin`, `epoch_race_cannot_issue_usable_old_session`.
- `sms_failover_reuses_challenge_and_expiry`, `resend_does_not_reset_abuse_limits`,
  `invalid_admin_does_not_trigger_public_registration`, `sms_acceptance_is_not_primary_proof`.

### PostgreSQL integration

- `runtime_cannot_update_delete_or_truncate_audit`, `audit_guards_reject_privileged_dml`,
  `runtime_cannot_disable_guards_or_assume_owner`, `history_cannot_cascade_mutation`.
  Use restricted runtime connections; separately exercise trigger protection.
- `concurrent_draft_cas_has_one_winner`, `aba_live_version_invalidates_approval`,
  `unrelated_subject_change_preserves_approval`, `publication_and_audit_commit_together`.
- `worker_and_http_publish_commit_one_effect`, `expired_lease_rejects_stale_worker`,
  `crash_before_commit_leaves_no_live_change`, `lost_commit_ack_resolves_by_publication_id`,
  `overdue_effect_is_not_backdated`, `unknown_prerequisite_persists_blocked_reason`.
- `otp_and_totp_consumption_are_single_use_under_race`, `recovery_races_with_login_safely`,
  `seed_rerun_preserves_published_values`, `database_isolation_covers_every_pool_connection`.

Owner-approved fixtures share labelled, detached Postgres/MinIO across the nextest run;
`#[sqlx::test]` and the canonical workspace migrator create a fresh database per test. Close
all extra pools before cleanup. Successful SQLx cleanup is best effort and must be tested;
failed databases remain until the same case is rerun or infrastructure removed. No broad
SQLx cleanup sweep runs from nextest children. Serialize separate local runs reusing a server
because SQLx database names derive from test paths. MinIO fixtures isolate and clean buckets;
signed-media behaviour remains Feature 004, not a fake 001 capability.

Fixture-driving tests (owner planning clarification 2026-11-04; FR-CON-37/NFR-SEC-09 integration
isolation and constitution Principle I):
- `integration_setup_is_idempotent_and_waits_for_both_services`,
  `setup_exports_only_matching_tests_ready_endpoints`,
  `fixture_reuse_accepts_only_owned_nonproduction_containers`.
- `sqlx_cases_get_distinct_migrated_databases`, `failed_database_survives_until_same_test_rerun`,
  `successful_database_cleanup_closes_all_app_pools`,
  `local_shared_server_rejects_concurrent_independent_runs`.
- `fallback_runner_sets_only_child_environment`,
  `fallback_runner_executes_same_test_and_propagates_failure`,
  `discovery_and_non_test_commands_do_not_start_fixtures`,
  `fallback_guard_outlives_child_test_process`.
- `minio_bucket_names_are_isolated_and_cleanup_covers_panics`,
  `ci_cleanup_matches_only_current_run_labels`, `test_down_refuses_an_active_local_run`,
  `setup_metadata_does_not_expose_fixture_credentials`.

See [integration-fixtures.md](integration-fixtures.md) for nextest 0.9.151 setup/profile,
connection-budget, retention, safe plain-Cargo/IDE runner and teardown contracts.

### Crypto and wiremock integration

- `kavenegar_lookup_uses_approved_post_template`, `sms_ir_verify_uses_sensitive_header`,
  provider acceptance/rejection/timeout/malformed-response cases for both real adapters.
- `vendor_error_redaction_removes_key_paths_and_bodies`, `vendor_redirects_are_refused`,
  `provider_timeout_does_not_create_second_valid_code`; no real HTTP vendor calls.
- `webauthn_rejects_wrong_origin_and_replayed_state`, `webauthn_stale_credential_snapshot_restarts`,
  `valid_zero_counter_passkey_is_supported`, `credential_updates_are_persisted` with real
  locally generated signed assertions, not only mocked success.
- `totp_enrollment_step_cannot_be_replayed`, `totp_wrong_envelope_key_fails_closed`,
  `secret_envelope_binds_admin_and_epoch`, `expired_primary_proof_cannot_start_factor_flow`.

### HTTP/templates/OpenAPI/browser

- `csrf_is_required_for_every_mutating_console_form`, `untrusted_return_target_is_refused`,
  `webauthn_hidden_options_are_html_escaped`, `auth_pages_do_not_reflect_or_cache_secrets`.
- `openapi_snapshot_matches_router`, `all_json_routes_are_documented_under_v1`,
  `health_and_problem_responses_match_schema`, `schema_endpoint_describes_itself_finitely`,
  `offline_openapi_export_needs_no_database`, `production_docs_and_assets_require_admin`,
  `swagger_makes_no_external_asset_or_validator_requests`.
- Instantiated template tests, not only source grep: one h1, labels/alt/focus/errors, all
  Fluent keys, six locale/theme choices, RTL identifiers/digits, unrounded contrast and
  preference switching with unsaved forms. Browser tests additionally cover keyboard,
  reduced motion, reflow, no-htmx/no-JS TOTP and missing passkey scripting without bypass.
- `J5_foundation_safe_change_and_recovery`: real composition/Postgres/crypto with SMS real
  HTTP adapters pointed at wiremock by an isolated test harness; prepare/publish harmless
  upload-limit change, find audit, rollback, recover an admin and reject old sessions.
  No pricing/AI-incident claims. Physical iOS Safari/Android Chrome checks remain release
  evidence in addition to Playwright Chromium/WebKit/Firefox and axe.

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

Require observed red-to-green evidence, all applicable layers, shared contract runners,
architecture negatives, current SQLx metadata and offline OpenAPI parity before marking
later tasks complete. Kernel coverage ≥90%; future wallet/generation/identity same floor
when implemented. Mutation testing applies to delivered wallet/generation, not a fabricated
empty module. Proposed other-crate/mutation thresholds stay proposals.

Default/ci profile filters exclude `integration_*` binaries; integration selects them
explicitly. Fast HTTP/wiremock suites may run in ci without Docker. CI documents present
feature scope and fails full-product releases with absent contexts/journeys; it never counts
absent tests as passing. All fixture secrets/phone inputs are synthetic and remain out of logs.
