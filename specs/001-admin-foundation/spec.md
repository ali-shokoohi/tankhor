# Feature Specification: Tankhor Admin Foundation

**Feature Branch**: `main` (existing branch; no branch-creation hook configured)

**Created**: 2026-11-03

**Status**: Ready for planning — formal clarification review completed

**Input**: User description: "Feature 001 — Tankhor admin foundation: secure Owner Console
access, versioned settings, feature flags, audit log, and the bilingual/theme shell every
later feature builds on."

## Clarifications

### Session 2026-11-03

- Q: When an admin account is recovered, what should happen to its existing console
  sessions and registered passkeys or authenticator-app setup? → A: End all of the
  recovered admin's console sessions and invalidate all their old second-factor
  credentials; require fresh enrolment before console access.
- Q: If another publication changes the same live setting or flag after a scheduled
  change was approved, should that scheduled change still go live? → A: Block the
  scheduled change and require fresh preview and approval against the latest live value.
- Q: If the service is unavailable at a scheduled publication time, what should happen
  when it recovers? → A: Publish once on recovery if approval, live-value checks, impact
  information and required health checks still pass; otherwise block and explain why.

### Session 2026-11-05

- OI8: lower recovery window applies only to new deletion requests (owner)

## User Scenarios & Testing *(mandatory)*

The actors are Admin and System, which applies approved scheduled changes. All admins have
equal full permissions at launch. Story priorities express delivery order within this
feature, not a downgrade of the PRD's P0 launch obligations. Each scenario has a local
acceptance ID and a named behavioural test; these are test specifications, not passing-test
or implementation claims. (PRD: FR-CON-04, FR-CON-05; §0.4)

### User Story 1 - Enter and Recover the Console Securely (Priority: P1)

As an admin, I can enter the separate Owner Console only after proving my admin identity
and completing the required second factor. Another admin can add or recover an admin
without making the console accessible to ordinary users. (PRD: FR-CON-01–04)

**Why this priority**: Every later administration action depends on controlled, attributable
access. (PRD: FR-CON-01–04, FR-CON-37)

**Independent Test**: With a provisioned admin and a non-admin identity, exercise access,
both supported second-factor paths, addition and recovery without any settings or product
features. The console admits only fully verified admins. (PRD: FR-CON-01–04)

**Acceptance Scenarios**:

1. **AC-ADM-001** (`console_rejects_non_admin_access`): **Given** a signed-out visitor or
   non-admin, **When** they open a protected console destination directly, **Then** no
   protected content or action is available; the console remains a separate experience
   at its own path on Tankhor's single domain. (PRD: FR-CON-01; Q01)
2. **AC-ADM-002** (`console_requires_completed_second_factor`): **Given** an admin whose
   primary identity verification succeeded, **When** the second factor is absent, invalid
   or expired, **Then** access stays closed; a valid passkey permits access, and a valid
   authenticator-app code permits access through the fallback. (PRD: FR-CON-02; Q23)
3. **AC-ADM-003** (`end_user_session_does_not_grant_admin_access`): **Given** an ordinary
   end-user session, **When** its holder opens the console, **Then** it does not count as
   an admin session or completed admin second factor. (PRD: FR-CON-01–02; NFR-SEC-05)
4. **AC-ADM-004** (`admin_addition_requires_another_admin_and_audit`): **Given** a fully
   verified admin, **When** they add another admin, **Then** the addition is attributed
   and audited, and the new admin must complete their own second factor before access;
   self-registration by a non-admin is refused. (PRD: FR-CON-02–04, FR-CON-37)
5. **AC-ADM-005** (`admin_recovery_preserves_second_factor_requirement`): **Given** a
   locked-out admin and another fully verified admin, **When** the latter authorises
   recovery, **Then** the recovery records both parties and its reason, and the recovered
   admin must freshly enrol and complete the required second factor before access.
   (PRD: FR-CON-02–03, FR-CON-37; session clarification: recovery revocation)
6. **AC-ADM-006** (`last_admin_lockout_has_no_bypass`): **Given** the only remaining admin
   has lost both factors and no other admin can authorise recovery, **When** access or
   self-recovery is attempted, **Then** the console remains locked; there is no automatic
   removal of the second-factor requirement. (PRD: FR-CON-02–03; Q23)
7. **AC-ADM-007** (`admins_have_equal_permissions_and_distinct_attribution`): **Given**
   two fully verified admins, **When** each performs the same permitted administrative
   action, **Then** both have equal permissions and their actions identify them separately.
   (PRD: FR-CON-04, FR-CON-37)
8. **AC-ADM-043** (`recovery_revokes_all_admin_sessions_and_old_factors`): **Given** the
   recovering admin has existing console sessions and registered second-factor credentials,
   **When** another fully verified admin authorises recovery, **Then** every existing
   console session and old passkey/authenticator-app credential of the recovered admin
   becomes invalid immediately; none can grant access, and fresh enrolment and verification
   are required. Other admins' sessions remain unaffected. (PRD: FR-CON-02–03; NFR-SEC-05;
   session clarification: recovery revocation)

---

### User Story 2 - Inspect and Safely Prepare Settings (Priority: P1)

As an admin, I can see each setting's meaning, current value, permitted values and history,
and prepare a change with an honest impact preview before it affects the live product.
(PRD: FR-CON-05–07, FR-CON-32)

**Why this priority**: Retention and operating limits must be explainable and changeable
without silent data loss or weakening product safeguards. (PRD: FR-CON-32)

**Independent Test**: Starting with the ten initial settings, prepare a lifetime change,
inspect its impact and approval status, and confirm that the live value remains unchanged.
No media expiry or generation execution is needed to test this management journey.
(PRD: FR-CON-05–07, FR-CON-32)

**Acceptance Scenarios**:

1. **AC-ADM-008** (`initial_settings_have_decided_values_and_complete_metadata`): **Given**
   the initial registry, **When** an admin inspects it, **Then** all ten settings below have
   their decided defaults, a stable key, FA/EN label, unit, permitted values, effective time
   and attributable history. (PRD: FR-CON-32, FR-UX-01; §10.1)
2. **AC-ADM-009** (`setting_rejects_values_outside_declared_policy`): **Given** a setting
   with its approved permitted-value policy, **When** an admin submits a value outside it
   or of the wrong kind, **Then** the change cannot advance, the relevant field explains
   the rule in the selected language, and valid input and the live value are preserved.
   The exact policies are the inclusive bounds and format choices below.
   (PRD: FR-CON-32, FR-UX-08; owner clarification Q1: A, supplied bounds)
3. **AC-ADM-010** (`saving_setting_creates_draft_only`): **Given** the auto-draft lifetime
   is 30 days and 7 days is permitted, **When** an admin saves a change to 7 days, **Then**
   it is visibly a draft and the live value remains 30 until preview, approval and publish.
   (PRD: FR-CON-05, FR-CON-32; Q32)
4. **AC-ADM-011** (`preview_uses_exact_draft_without_live_effects`): **Given** a settings
   draft, **When** the admin previews it, **Then** the preview identifies that exact version,
   distinguishes proposed from live values and shows operational effects without executing
   them; the console preview works on mobile, tablet and desktop in both languages and
   all theme choices. (PRD: FR-CON-06, FR-CON-09, FR-CON-11)
5. **AC-ADM-012** (`shorter_lifetime_previews_existing_item_impact`): **Given** a shorter
   lifetime is proposed, **When** impact is previewed, **Then** the admin sees the number
   of existing items whose expiry would change and those already due under the proposed
   policy, recalculated from each item's original reference time rather than publication
   time. A shorter account deletion recovery window is the exception: it changes no
   existing deletion end date and applies only to deletion requests made after
   publication (AC-ADM-046). The shorter lifetime applies to existing and new items only
   when published and effective; the preview explains that items already past the new limit become due for
   the next sweep, not deletion during drafting or preview. Unavailable counts are not
   presented as zero and block publication. (PRD: FR-CON-32; Technical Specification P41;
   owner clarification Q2: A and supplied lowering rules)
6. **AC-ADM-013** (`settings_cannot_weaken_security_or_financial_integrity`): **Given**
   any setting or flag change, **When** it would bypass mandatory second factors, access
   restrictions, audit protection or financial integrity, **Then** it is refused regardless
   of admin permissions or urgency. (PRD: FR-CON-02, FR-CON-32, FR-CON-37)
7. **AC-ADM-014** (`editing_approved_draft_invalidates_approval`): **Given** an approved
   draft, **When** any proposed value or effective time is edited, **Then** its approval
   becomes invalid and the changed version needs fresh preview and approval before
   publication, including scheduled publication. (PRD: FR-CON-05–07)
8. **AC-ADM-015** (`concurrent_setting_edits_do_not_silently_overwrite`): **Given** two
   admins editing the same draft version, **When** one saves first and the other submits
   the stale version, **Then** the second admin sees a conflict rather than overwriting
   the first change, and neither change becomes live. (PRD: FR-CON-05, FR-CON-07, FR-CON-37)
9. **AC-ADM-037** (`settings_accept_inclusive_bounds_without_changing_defaults`):
   **Given** each of the nine numeric settings, **When** its minimum and maximum are
   submitted as separate draft changes, **Then** both boundaries are accepted, values
   below the minimum or above the maximum are rejected, and none of these submissions
   changes the decided initial live value before publication. Every numeric setting
   has a finite upper bound. (PRD: FR-CON-05, FR-CON-32; §10.1;
   owner clarification Q1: A, supplied bounds)
10. **AC-ADM-038** (`upload_format_policy_requires_jpeg_and_known_formats`): **Given**
    the upload-format setting, **When** any subset of JPEG, PNG, WebP and HEIC is proposed,
    **Then** it is permitted only if JPEG remains enabled; an empty set, removal of JPEG
    or addition of any other format is rejected. Initially all four remain enabled.
    (PRD: FR-CON-32; §10.1; Q05; owner-supplied format policy)
11. **AC-ADM-039** (`card_limit_preview_preserves_existing_projects`): **Given** a
    proposed card-limit decrease and existing drafts, saved projects and confirmed
    projects above it, **When** impact is previewed, **Then** it explains that drafts
    and saved projects stay intact but cannot add cards or start generation until at
    or below the new limit; confirmed projects are unaffected. At the limit, creation
    is permitted but an addition that exceeds it is not. (PRD: FR-CON-32; §10.1; Q15;
    owner-supplied lowering rules)
12. **AC-ADM-040** (`photo_limit_preview_preserves_existing_photos`): **Given** a
    proposed permanent-photo-limit decrease, **When** impact is previewed, **Then**
    it explains that users keep all existing photos and cannot add a permanent photo
    while at or above the new limit; they may add one once below it. No existing photo
    is deleted by the limit change. (PRD: FR-CON-32; §10.1; Q40;
    owner-supplied lowering rules)
13. **AC-ADM-041** (`upload_policy_preview_applies_only_to_new_uploads`): **Given** a
    change to upload size, permitted formats or minimum shortest edge, **When** impact
    is previewed, **Then** it identifies the new-upload policy and states that stored
    files are neither re-validated nor removed because of that change, including a
    stored file which would fail the newly proposed limits. (PRD: FR-CON-32; §10.1; Q05;
    owner-supplied upload-change rules)
14. **AC-ADM-042** (`attempt_limit_preview_preserves_existing_units`): **Given** a
    proposed attempt-limit change from three to one, **When** impact is previewed,
    **Then** it explains that units queued after the change becomes live receive the
    new limit, previously queued units retain their assigned limit, and processing
    units keep the limit they started with. No allowed value exceeds three total
    attempts. (PRD: FR-CON-32; §10.1; Q29; owner-supplied attempt-change rules)
15. **AC-ADM-046** (`recovery_window_decrease_keeps_pending_end_dates`): **Given** an
    account pending deletion with end date D, **When** an admin publishes a shorter
    recovery window, **Then** D is unchanged; a deletion requested after publish uses the
    new window. (PRD: FR-CON-32; §10.1; R26; owner decision 2026-11-05, OI8)

---

### User Story 3 - Publish, Schedule and Roll Back Changes (Priority: P1)

As an admin, I can publish a reviewed setting or flag change now or later, see which version
is live, and restore a prior version without hiding history. (PRD: FR-CON-05–10)

**Why this priority**: Safe preparation has value only if the approved change, and no other
change, can take effect and be reversed transparently. (PRD: FR-CON-05, FR-CON-07–08)

**Independent Test**: Using an approved setting draft, publish it immediately and at a
displayed future time, then roll back. Verify live values, versions and history without
needing any later business capability. (PRD: FR-CON-05, FR-CON-08, FR-CON-37)

**Acceptance Scenarios**:

1. **AC-ADM-016** (`approved_change_publishes_only_at_authorised_time`): **Given** a
   previewed and approved draft, **When** publication is confirmed now or scheduled for
   a displayed date, time and time zone, **Then** only that approved version becomes live,
   never before the authorised effective time; a new version and audit event identify it.
   (PRD: FR-CON-05, FR-CON-07, FR-CON-32, FR-CON-37)
2. **AC-ADM-017** (`scheduled_publish_rejects_invalidated_approval`): **Given** a scheduled
   change whose approval was invalidated by editing, **When** its scheduled time arrives,
   **Then** it does not publish and the admin can see that fresh preview and approval are
   required. (PRD: FR-CON-05, FR-CON-07)
3. **AC-ADM-018** (`repeated_publication_has_one_effect`): **Given** the same approved
   publication is submitted or processed twice, **When** both attempts finish, **Then**
   there is one effective change and one successful publication event; no duplicate live
   version is created for that publication. (PRD: FR-CON-05, FR-CON-37)
4. **AC-ADM-019** (`rollback_creates_new_version_and_preserves_history`): **Given** a
   published change, **When** another admin selects a prior version and completes preview,
   approval and publication, **Then** the old value becomes live as a new version, and the
   original change and rollback remain visible with their actors. (PRD: FR-CON-05, FR-CON-08,
   FR-CON-37)
5. **AC-ADM-020** (`urgent_foundation_change_still_requires_review`): **Given** an urgent
   setting or flag change, **When** an admin attempts to make it live, **Then** impact and
   confirmation are still required and the normal approval rules cannot be skipped.
   (PRD: FR-CON-05, FR-CON-07, FR-CON-10, FR-CON-32)
6. **AC-ADM-044** (`scheduled_change_blocks_when_live_baseline_changes`): **Given** an
   approved scheduled setting or flag change, **When** another publication changes the
   same live value before the scheduled change takes effect, **Then** the scheduled
   change is blocked rather than overwriting that intervening decision; fresh preview
   and approval against the latest live value are required. A change to an unrelated
   setting or flag does not by itself invalidate this approval. (PRD: FR-CON-05–07,
   FR-CON-32; session clarification: scheduled-change baseline)
7. **AC-ADM-045** (`overdue_publication_rechecks_prerequisites_and_runs_once`): **Given**
   a scheduled publication missed because the service was unavailable, **When** service
   resumes, **Then** the change publishes once without another confirmation if its exact
   version approval, latest live-value checks, impact information and required health
   checks all still pass; otherwise it remains blocked with the reason and required
   action visible. Repeated recovery attempts cannot duplicate the publication. A
   successful overdue publication takes effect when actually published, not retroactively,
   and records both scheduled and actual publication times. (PRD: FR-CON-05–07,
   FR-CON-32, FR-CON-37; session clarification: overdue publication)

---

### User Story 4 - Investigate an Attributable, Protected History (Priority: P1)

As an admin, I can establish who performed a foundation action, what changed and why,
without being able to rewrite or erase the evidence. (PRD: FR-CON-04, FR-CON-37)

**Why this priority**: Equal full permissions must not remove individual accountability.
(PRD: FR-CON-04, FR-CON-37)

**Independent Test**: Perform foundation actions, search their records, and attempt to
change or delete a record. The full history remains accessible only to verified admins
and unaltered. (PRD: FR-CON-01, FR-CON-37; NFR-SEC-09)

**Acceptance Scenarios**:

1. **AC-ADM-021** (`foundation_actions_have_complete_attributed_audit`): **Given** admin
   sign-in, addition, recovery, draft, preview, approval, scheduling, publication and rollback
   actions, **When** their records are inspected, **Then** each identifies actor, action,
   subject, time and reason, plus before/after values where relevant; sign-in records the
   factor used. (PRD: FR-CON-03–05, FR-CON-37)
2. **AC-ADM-022** (`admin_can_search_foundation_audit`): **Given** records from multiple
   admins and subjects, **When** an admin searches by actor, action, subject or time period,
   **Then** matching records and their change history are available without exposing
   protected records to non-admins. (PRD: FR-CON-01, FR-CON-37)
3. **AC-ADM-023** (`audit_entries_cannot_be_edited_or_deleted`): **Given** an audit record,
   **When** any admin attempts to alter or delete it, **Then** the attempt is refused and
   the original record remains unchanged. (PRD: FR-CON-37; NFR-SEC-09)
4. **AC-ADM-024** (`audit_excludes_authentication_secrets_and_private_data`): **Given**
   a foundation action, **When** its audit and diagnostic records are examined, **Then**
   they identify the actor without phone numbers, authentication codes, factor secrets,
   keys, photos or payment data. (PRD: FR-CON-37; §13.1; NFR-SEC-03)
5. **AC-ADM-025** (`unauditable_admin_action_is_not_reported_successful`): **Given**
   an attributable action cannot have its required audit recorded, **When** it is attempted,
   **Then** it does not grant access or report a successful change with missing evidence,
   and the admin sees a recoverable failure. (PRD: FR-CON-03–05, FR-CON-37)

---

### User Story 5 - Work in Either Language and Any Theme (Priority: P1)

As an admin, I can complete every foundation task in Persian or English, on a small or
large screen, in a suitable theme and with assistive input, without losing my work.
(PRD: FR-CON-11, FR-UX-01–09, FR-UX-14)

**Why this priority**: A bilingual, accessible shell is a launch requirement, not a later
translation or styling phase. (PRD: FR-CON-11, FR-UX-01–02, FR-UX-08)

**Independent Test**: Exercise sign-in and recovery screens, settings, flags, preview,
approval and audit views in both languages and each theme choice, including keyboard-only
operation. (PRD: FR-CON-11, FR-UX-01–09)

**Acceptance Scenarios**:

1. **AC-ADM-026** (`foundation_screens_support_both_languages_and_all_themes`): **Given**
   any foundation screen, **When** Persian or English and light, dark or system are chosen,
   **Then** the screen is complete and usable in every combination; system follows the
   device's appearance preference. (PRD: FR-CON-11, FR-UX-01–02)
2. **AC-ADM-027** (`language_and_theme_switch_preserve_unsaved_input`): **Given** valid
   unsaved input or a selected draft, **When** language or theme changes, **Then** input,
   selection, draft identity and live values remain unchanged. (PRD: FR-UX-03)
3. **AC-ADM-028** (`persian_direction_and_digits_preserve_identifiers`): **Given** the
   console in Persian, **When** its forms and tables are read or traversed by keyboard,
   **Then** reading and focus order follow RTL, prose uses Persian digits, IDs/URLs/hex
   values retain their own direction and Latin digits, logos and photos are not mirrored,
   and only directional icons flip. English follows LTR. (PRD: FR-UX-01, FR-UX-04–05)
4. **AC-ADM-029** (`foundation_brand_and_contrast_follow_approved_baseline`): **Given**
   either visual theme, **When** the console is reviewed, **Then** it carries the Tankhor
   parent brand and Vazirmatn with its Latin, uses the approved velvet-brown/sky-blue
   palette, and meets the required text and meaningful-control contrast; the decorative
   border is never a control's only boundary and small white text is not placed on light
   sky blue. (PRD: FR-UX-06–07, FR-UX-17; Q02–Q03)
5. **AC-ADM-030** (`foundation_tasks_are_keyboard_and_assistive_accessible`): **Given**
   any foundation task, **When** performed with keyboard, assistive reading, zoom/reflow
   or reduced motion, **Then** the task remains completable with visible, unobscured focus,
   named controls, field-linked non-colour-only errors, no traps, and at least 44 × 44 px
   touch targets; any non-essential auto-motion over five seconds can be paused or stopped.
   (PRD: FR-UX-08–09)
6. **AC-ADM-031** (`foundation_copy_has_fa_en_parity_and_context`): **Given** every visible
   label, validation message, status and accessible name, **When** both language versions
   are reviewed, **Then** matching managed messages exist for the relevant surface and
   situation, with no missing or unexplained fallback copy. (PRD: FR-UX-01, FR-UX-14)
7. **AC-ADM-032** (`foundation_tasks_work_without_optional_enhancements`): **Given**
   optional interaction or motion enhancements are unavailable, **When** an admin completes
   a foundation task, **Then** the same action, validation and result remain accessible
   without a loss of input or a bypass of approval. (PRD: FR-CON-05, FR-UX-08–09)
8. **AC-ADM-047** (`visual_identity_values_have_one_source_and_checked_contrast`): **Given**
   the console's visual identity values, **When** the foundation is built and tested,
   **Then** every value comes from the single declared source, and every declared
   text/background pair is verified automatically against WCAG 2.2 AA (≥ 4.5:1 normal,
   ≥ 3:1 large text); a failing pair or a value defined outside that source fails the
   check. (PRD: FR-UX-20, FR-UX-07)

---

### User Story 6 - Inspect Disabled Capabilities Before Enablement (Priority: P2)

As an admin, I can see the six launch flags, their history and readiness, without accidentally
enabling a capability that is unbuilt or unhealthy. (PRD: FR-CON-32; §3.3, §6.2)

**Why this priority**: This establishes safe enablement controls after the access, review
and audit foundation; it does not deliver the capabilities those controls will govern.
(PRD: FR-CON-05, FR-CON-32; §0.4)

**Independent Test**: Inspect the six initial OFF records, prepare a flag draft, and verify
the approval and readiness gates against explicitly supplied readiness outcomes. No
underlying optional product capability is needed for this management test. (PRD: FR-CON-32)

**Acceptance Scenarios**:

1. **AC-ADM-033** (`six_launch_flags_start_off`): **Given** the initial flag register,
   **When** it is inspected, **Then** it contains exactly the six named flags below, all
   OFF, with no optional capability represented as already delivered. (PRD: FR-CON-32; §6.2)
2. **AC-ADM-034** (`flag_changes_use_versioned_review_and_history`): **Given** a flag,
   **When** an admin inspects or prepares a change, **Then** its name, description, state,
   impact preview, health result and history are available, and saving follows the same
   draft/preview/approval/publication rules as settings. (PRD: FR-CON-05–08, FR-CON-32)
3. **AC-ADM-035** (`failed_or_unknown_health_blocks_enablement`): **Given** an enablement
   draft with a failed or unavailable health result, **When** publication is attempted,
   **Then** the live flag remains OFF and the unmet prerequisite is explained rather than
   treated as a passing check. (PRD: FR-CON-32)
4. **AC-ADM-036** (`unbuilt_capability_cannot_be_enabled_by_flag`): **Given** the behaviour
   behind a launch flag has not been delivered and verified, **When** enablement is attempted,
   **Then** it is blocked and registration of the flag is not counted as completion of the
   capability. (PRD: FR-CON-32; §0.4, §3.3, §6.2)

### Edge Cases

- Two admins save the same draft: reject the stale edit, preserve the accepted draft and
  require review of the conflict. (AC-ADM-015; PRD: FR-CON-05, FR-CON-07)
- An approved scheduled change is edited before its due time: do not publish it under the
  stale approval. (AC-ADM-014, AC-ADM-017; PRD: FR-CON-07)
- Another publication changes the live value targeted by an approved scheduled change:
  block that scheduled publication pending fresh preview and approval against the latest
  live value; unrelated changes do not invalidate its approval. (AC-ADM-044;
  PRD: FR-CON-05–07, FR-CON-32; session clarification: scheduled-change baseline)
- The service is unavailable at a scheduled publication time: on recovery, recheck all
  publication prerequisites and publish once if they pass; otherwise leave it blocked
  with the reason visible. Do not backdate the live effect to the missed scheduled time.
  (AC-ADM-045; PRD: FR-CON-05–07, FR-CON-32, FR-CON-37;
  session clarification: overdue publication)
- The last admin loses both factors: remain locked without a self-recovery exception to
  Q23; ordinary recovery still requires another admin. (AC-ADM-006; PRD: FR-CON-02–03)
- A recovered admin's lost device still has a console session or old second-factor
  credential: refuse both immediately after authorised recovery; fresh enrolment is
  required without affecting another admin's access. (AC-ADM-043; PRD: FR-CON-02–03;
  NFR-SEC-05; session clarification: recovery revocation)
- A setting is invalid, impact cannot be determined, or a flag check fails: show the reason
  and preserve the current live state and valid input. (AC-ADM-009, AC-ADM-012, AC-ADM-035;
  PRD: FR-CON-32, FR-UX-08)
- A shorter lifetime makes existing items immediately due: the impact must be explicit;
  the published shorter lifetime applies to existing and new items, measured from the
  original reference time, never restarted from publication. Drafting and preview do not
  expire or delete anything; already-overdue items are handled at the next sweep after
  the change becomes live. A shorter account deletion recovery window applies only to
  deletion requests made after publication; accounts already pending deletion keep the
  end date they were shown. (AC-ADM-012, AC-ADM-046; PRD: FR-CON-32; Technical Specification P41;
  owner clarification Q2: A and supplied lowering rules)
- A lower card/photo limit is below current use: preserve existing projects/photos and
  show the applicable addition/creation restriction, not a destructive cleanup.
  (AC-ADM-039–040; PRD: FR-CON-32; §10.1; owner-supplied lowering rules)
- A stored file no longer meets a changed upload policy: keep it without re-validation;
  a processing generation unit keeps its original attempt allowance. Explain these
  boundaries in the proposed change's impact preview. (AC-ADM-041–042;
  PRD: FR-CON-32; §10.1; owner-supplied change rules)
- A publication is retried or audit recording fails: no duplicate successful publication,
  no untraceable successful action. (AC-ADM-018, AC-ADM-025; PRD: FR-CON-05, FR-CON-37)
- Language or theme changes while an error or unsaved edit is present: preserve the work,
  translate the message and maintain understandable focus. (AC-ADM-027–028, AC-ADM-030–031;
  PRD: FR-UX-01–04, FR-UX-08, FR-UX-14)

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The Owner Console MUST be a separate admin-only experience at its own path
  on the single Tankhor domain; every protected destination and action MUST refuse
  non-admin access. (PRD: FR-CON-01; Q01; acceptance: AC-ADM-001)
- **FR-002**: Access MUST require verified admin identity plus a completed second factor:
  passkey preferred, authenticator-app code as fallback. Missing, invalid or expired proof
  MUST NOT admit an admin. (PRD: FR-CON-02; Q23; acceptance: AC-ADM-002)
- **FR-003**: Admin sessions MUST be separate from end-user sessions; ordinary sign-in
  MUST NOT grant console access. (PRD: FR-CON-01–02; NFR-SEC-05; acceptance: AC-ADM-003)
- **FR-004**: Addition and recovery MUST be authorised by another verified admin and
  audited with actor, affected admin and reason; recovered or newly added admins MUST
  complete the required second factor themselves. Authorised recovery MUST immediately
  invalidate all of the recovered admin's existing console sessions and old second-factor
  credentials and require fresh enrolment before access; other admins' sessions MUST
  remain unaffected. Without another admin, recovery MUST NOT bypass the access rule.
  (PRD: FR-CON-02–03, FR-CON-37; NFR-SEC-05; acceptance: AC-ADM-004–006, AC-ADM-043;
  session clarification: recovery revocation)
- **FR-005**: All admins MUST have equal full permissions, with separate attribution for
  each person's actions. (PRD: FR-CON-04; acceptance: AC-ADM-007)
- **FR-006**: The registry MUST contain the ten initial settings below with stable key,
  FA/EN label, unit, live value, permitted range or set, effective-from time, and history
  of actor, time, previous value, new value and reason. (PRD: FR-CON-32; §10.1;
  acceptance: AC-ADM-008)
- **FR-007**: Settings MUST reject values outside their declared permitted range or set
  and explain the rule without losing valid input. Numeric bounds in Initial Settings
  MUST be inclusive and finite; no setting may exceed its listed maximum. Defaults MUST
  remain as decided until a reviewed change becomes live, and attempts MUST never exceed
  three total. (PRD: FR-CON-32; §10.1; Q29; acceptance: AC-ADM-009, AC-ADM-037;
  owner clarification Q1: A, supplied bounds)
- **FR-008**: Saving a setting or flag MUST create a clearly identified draft only;
  the live value MUST remain unchanged until the required publication steps complete.
  (PRD: FR-CON-05, FR-CON-32; acceptance: AC-ADM-010, AC-ADM-034)
- **FR-009**: Preview MUST identify the exact draft, distinguish it from live state and
  show effects without executing them. The management experience MUST be previewable on
  mobile/tablet/desktop, in both languages and display modes. (PRD: FR-CON-06, FR-CON-09,
  FR-CON-11; acceptance: AC-ADM-011)
- **FR-010**: A published shorter lifetime MUST apply to existing and new items from its
  effective time. Existing expiry MUST be recalculated using the original reference time:
  upload time for temporary photos/uploads and last activity for Studio auto-drafts. A
  shorter account deletion recovery window MUST apply only to deletion requests made after
  publication; accounts already pending deletion MUST keep the end date they were shown.
  Publication MUST NOT restart an item's age. Preview MUST show the number of existing items whose expiry would change
  and those already due under the proposed lifetime; unknown impact MUST block publication.
  Preview MUST explain that items past the new limit are purged or finalised by their
  owning lifecycle at the next sweep after the change becomes live. Drafting and preview
  MUST NOT execute expiry or deletion. Execution remains the owning later feature's
  responsibility, not a new media or account-deletion capability here.
  (PRD: FR-CON-32; §10.1; Technical Specification P41; acceptance: AC-ADM-012, AC-ADM-046;
  owner clarification Q2: A and supplied lowering rules; owner decision 2026-11-05, OI8)
- **FR-011**: Neither settings nor flags, including urgent changes, MUST provide a way
  to weaken mandatory security or financial integrity. (PRD: FR-CON-02, FR-CON-32,
  FR-CON-37; acceptance: AC-ADM-013)
- **FR-012**: Publication MUST require preview and approval of the exact draft version;
  editing any proposed content or effective time MUST invalidate its approval and require
  fresh preview and approval. Approval MUST identify the affected live values used for
  that review. (PRD: FR-CON-05–07; acceptance: AC-ADM-014, AC-ADM-044)
- **FR-013**: Approved changes MUST support immediate or scheduled publication with an
  unambiguous effective time. Invalidated approvals MUST block scheduled publication;
  a scheduled change MUST also be blocked if another publication has changed the same
  affected live setting or flag since approval. Fresh preview and approval against the
  latest live value MUST be required before that change can publish; unrelated changes
  MUST NOT by themselves invalidate approval. A blocked change MUST remain identifiable
  to admins with the reason and required review visible. In all publication paths,
  repeating the same publication MUST NOT create a second effective version or second
  successful publication event. (PRD: FR-CON-05, FR-CON-07, FR-CON-32, FR-CON-37;
  acceptance: AC-ADM-016–018, AC-ADM-044; session clarification: scheduled-change baseline)
- **FR-014**: Rollback MUST follow preview, approval and publication, restore selected
  earlier values as a new version, and retain all prior history and executed actions.
  (PRD: FR-CON-05, FR-CON-08; acceptance: AC-ADM-019)
- **FR-015**: Urgent foundation changes MUST still show impact and require confirmation
  and approval; urgency MUST NOT waive safeguards. (PRD: FR-CON-05, FR-CON-07, FR-CON-10,
  FR-CON-32; acceptance: AC-ADM-020)
- **FR-016**: Concurrent edits MUST NOT silently overwrite a newer draft; the stale editor
  MUST be told of the conflict before continuing. (PRD: FR-CON-05, FR-CON-07;
  acceptance: AC-ADM-015)
- **FR-017**: The flag register MUST initially contain exactly the six launch flags below,
  all OFF. (PRD: FR-CON-32; §6.2; acceptance: AC-ADM-033)
- **FR-018**: Each flag MUST expose name, description, on/off state, impact preview, health
  check result and attributable version history, and use the settings change workflow.
  (PRD: FR-CON-05–08, FR-CON-32; acceptance: AC-ADM-034)
- **FR-019**: A failed or unavailable health check MUST block enablement and explain the
  unmet prerequisite without changing the live flag. (PRD: FR-CON-32; acceptance: AC-ADM-035)
- **FR-020**: An undelivered capability MUST NOT be enabled or labelled complete merely
  because its flag exists. Registration here MUST NOT be presented as delivery of a P0-OFF
  capability. (PRD: FR-CON-32; §0.4, §3.3, §6.2; acceptance: AC-ADM-036)
- **FR-021**: Audit MUST record foundation sign-ins and factor used, admin additions and
  recoveries, drafts, previews, approvals, schedules, publications and rollbacks. Each record
  MUST identify actor, action, subject, time, reason and before/after where relevant.
  (PRD: FR-CON-03–05, FR-CON-37; acceptance: AC-ADM-021)
- **FR-022**: Verified admins MUST be able to search audit history by actor, action,
  subject and time period. Non-admins MUST NOT see it. (PRD: FR-CON-01, FR-CON-37;
  acceptance: AC-ADM-022)
- **FR-023**: Audit history MUST be append-only: no admin may alter or delete entries.
  (PRD: FR-CON-37; NFR-SEC-09; acceptance: AC-ADM-023)
- **FR-024**: Audit and diagnostic records MUST exclude phone numbers, authentication
  codes, factor secrets, keys, photos and payment data. A foundation action MUST NOT be
  considered successful if its required audit evidence cannot be recorded.
  (PRD: FR-CON-37; §13.1; NFR-SEC-03; acceptance: AC-ADM-024–025)
- **FR-025**: Every foundation screen MUST support complete Persian/RTL and English/LTR
  versions and light/dark/system themes, with system following device preference.
  (PRD: FR-CON-11, FR-UX-01–02; acceptance: AC-ADM-026)
- **FR-026**: Language or theme switching MUST preserve unsaved input, selected work,
  live data and draft identity. (PRD: FR-UX-03; acceptance: AC-ADM-027)
- **FR-027**: Persian reading and focus order MUST follow RTL; IDs, URLs and hex values
  MUST retain their own direction. Prose MUST use Persian digits in FA, codes and IDs
  Latin digits. Logos/photos MUST NOT mirror; only directional icons may flip.
  (PRD: FR-UX-04–05; acceptance: AC-ADM-028)
- **FR-028**: The console MUST carry the Tankhor parent brand, use Vazirmatn with its Latin,
  and follow the approved velvet-brown/sky-blue palette. Normal text contrast MUST be at
  least 4.5:1, large text at least 3:1, and meaningful control boundaries/focus at least
  3:1, measured without rounding. The approved `borderControl` and `onCurtain` roles MUST
  be respected; `border` MUST NOT be a control's sole boundary, and small white text on
  light sky is prohibited. (PRD: FR-UX-06–07, FR-UX-17; Q02–Q03; acceptance: AC-ADM-029)
- **FR-029**: Foundation tasks MUST meet WCAG 2.2 AA and Tankhor's additional 44 × 44 px
  target and reduced-motion rules, including keyboard operation, visible unobscured focus,
  named controls, meaningful image descriptions, field-linked non-colour-only errors,
  zoom/reflow, no traps and pause/stop for non-essential auto-motion over five seconds.
  Hover-only actions MUST have touch and keyboard equivalents. (PRD: FR-UX-08–09;
  acceptance: AC-ADM-030)
- **FR-030**: All UI copy, including errors, status and accessible names, MUST use managed
  FA/EN messages identified by surface and situation; missing translations MUST be visible
  during review rather than silently treated as complete. (PRD: FR-UX-01, FR-UX-14;
  acceptance: AC-ADM-031)
- **FR-031**: Every foundation task MUST remain usable without optional interaction or
  motion enhancements, with equivalent validation, review and results. (PRD: FR-CON-05,
  FR-UX-08–09; constitution Principle V; acceptance: AC-ADM-032)
- **FR-032**: Upload-format choices MUST be limited to JPEG, PNG, WebP and HEIC. JPEG MUST
  always remain enabled; admins may choose any subset containing JPEG, never an empty
  set or an additional format. All four MUST initially be enabled. (PRD: FR-CON-32;
  §10.1; Q05; acceptance: AC-ADM-038; owner-supplied format policy)
- **FR-033**: A card-limit decrease's impact preview MUST explain that existing drafts
  and saved projects remain intact, cannot add cards while above the new limit and cannot
  start generation until at or below it; no addition may exceed the limit. It MUST state
  that confirmed projects are unaffected. (PRD: FR-CON-32; §10.1; Q15;
  acceptance: AC-ADM-039; owner-supplied lowering rules)
- **FR-034**: A permanent-photo-limit decrease's impact preview MUST explain that existing
  photos are kept and adding a permanent photo is blocked while the user's count is at
  or above the new limit. (PRD: FR-CON-32; §10.1; Q40; acceptance: AC-ADM-040;
  owner-supplied lowering rules)
- **FR-035**: An upload size, format or shortest-edge change's impact preview MUST state
  that the new policy applies only to new uploads after it becomes live; stored files
  are never re-validated because of the change. (PRD: FR-CON-32; §10.1; Q05;
  acceptance: AC-ADM-041; owner-supplied upload-change rules)
- **FR-036**: An attempt-limit change's impact preview MUST state that the new allowance
  applies to units queued after it becomes live, previously queued units retain their
  assigned limit, and processing units keep the limit they started with.
  (PRD: FR-CON-32; §10.1; Q29; acceptance: AC-ADM-042; owner-supplied attempt-change rules)
- **FR-037**: A publication missed during service unavailability MUST be retried on
  recovery, without requiring another confirmation if its exact-version approval,
  live-value checks, impact information and required health checks still pass. It MUST
  publish at most once; if any prerequisite fails or is unavailable it MUST remain
  blocked with the reason and required action visible. Successful overdue publication
  MUST take effect at actual publication, never retroactively, and MUST record both
  scheduled and actual publication times. (PRD: FR-CON-05–07, FR-CON-32, FR-CON-37;
  acceptance: AC-ADM-018, AC-ADM-045; session clarification: overdue publication)
- **FR-038**: Visual identity values MUST come from one source; text/background contrast
  MUST be verified automatically against WCAG 2.2 AA. (PRD: FR-UX-20; acceptance:
  AC-ADM-047)

#### Initial Settings

Initial live values remain as decided. The owner supplied the following inclusive bounds
for the configurable settings in PRD §10.1; these are permitted configuration values, not
replacements for the defaults. Every numeric setting has a finite upper bound. All ten
settings MUST include FR-006's metadata. No other feature's behaviour is implemented merely
by registering its setting. (PRD: FR-CON-32; §10.1; owner clarification Q1: A, supplied bounds)

| Setting | Default | Min | Max | Unit | Source |
| --- | --- | --- | --- | --- | --- |
| Buyer temporary photo lifetime | 14 | 1 | 30 | Days from upload | FR-CON-32; §10.1; R15 |
| Studio temporary upload lifetime | 30 | 7 | 90 | Days from upload | FR-CON-32; §10.1; Q06 |
| Studio auto-draft lifetime | 30 | 1 | 90 | Days from last activity | FR-CON-32; §10.1; Q32 |
| Account deletion recovery window | 30 | 14 | 90 | Days from deletion request | FR-CON-32; §10.1; R26 |
| Maximum cards per Studio project | 50 | 1 | 100 | Cards | FR-CON-32; §10.1; Q15 |
| Permanent personal photos per buyer | 5 | 1 | 10 | Photos | FR-CON-32; §10.1; Q40 |
| Maximum upload size | 15 | 1 | 25 | MB | FR-CON-32; §10.1; Q05 |
| Minimum upload shortest edge | 512 | 256 | 1024 | Pixels | FR-CON-32; §10.1; Q05 |
| Maximum internal attempts per unit | 3 | 1 | 3 | Total attempts, not additional retries | FR-CON-32; §10.1; Q29 |

The tenth setting is **accepted upload formats**: initially JPEG, PNG, WebP and HEIC; any
subset containing JPEG is permitted. JPEG cannot be disabled, at least one format remains
enabled, and no other format can be added through settings. (PRD: FR-CON-32; §10.1; Q05;
owner-supplied format policy; acceptance: AC-ADM-038)

#### Published Setting Effects

Impact previews MUST describe the owner-approved policies below. "After publication" means
after the reviewed change becomes live, including its scheduled effective time. Foundation
work records and previews these policies; their enforcement belongs to the later feature
owning the affected photos, accounts, projects, uploads or generation units. No execution
of those out-of-scope capabilities is added to Feature 001. (PRD: FR-CON-05, FR-CON-32;
§10.1; owner-supplied change rules; acceptance: AC-ADM-012, AC-ADM-039–042)

| Setting group | Published policy | Foundation acceptance |
| --- | --- | --- |
| Shorter lifetimes | Recalculate existing expiry from original upload or last-activity time, never from publication. Items past the new limit are processed at the next sweep after the change is live, under the owning lifecycle. | AC-ADM-012 |
| Lower account deletion recovery window | Apply only to deletion requests made after the change is live. Accounts already pending deletion keep the end date they were shown. (owner decision 2026-11-05) | AC-ADM-046 |
| Lower card limit | Keep existing drafts and saved projects intact. Above-limit projects cannot add cards or start generation until reduced to the limit or below. At-limit projects may start but cannot add beyond the limit. Already-confirmed projects are unaffected. | AC-ADM-039 |
| Lower permanent-photo limit | Keep existing photos. A user cannot add a permanent photo until their count is below the limit. | AC-ADM-040 |
| Upload size, formats, shortest edge | Apply changes only to new uploads after the change is live. Never re-validate stored files because of changed settings. | AC-ADM-041 |
| Attempt limit | Assign the new limit to units queued after the change is live. Previously queued units keep their assigned limit; processing units keep their starting limit. | AC-ADM-042 |

#### Initial Flag Register

All six start OFF. Descriptions and readiness results must distinguish registration from
delivery; behaviour behind these flags remains outside Feature 001. (PRD: FR-CON-32; §6.2)

| Flag | Initial state | Source |
| --- | --- | --- |
| Partner widget/API | OFF | FR-CON-32; §6.2 |
| Percentage rollout | OFF | FR-CON-32; §6.2 |
| Per-user usage limits | OFF | FR-CON-32; §6.2 |
| Ticketing | OFF | FR-CON-32; §6.2 |
| External notifications | OFF | FR-CON-32; §6.2 |
| Paid Share billing | OFF | FR-CON-32; §6.2 |

### Key Entities *(include if feature involves data)*

- **Admin**: An individually identified console operator with full launch permissions
  and second-factor enrolment. Addition/recovery identifies the authorising other admin;
  recovery invalidates all prior second-factor credentials and requires fresh enrolment.
  (PRD: FR-CON-01–04)
- **Admin session**: The separate, verified console-access state, including completion of
  the required second factor; an ordinary user session is not a substitute. All of an
  admin's existing console sessions become invalid immediately upon authorised recovery.
  (PRD: FR-CON-01–02; NFR-SEC-05)
- **Setting**: A named tunable with bilingual meaning, unit, live value, permitted values,
  effective time and attributable change history. (PRD: FR-CON-32)
- **Feature flag**: A named optional-capability control with description, live state,
  impact preview, health result and history. (PRD: FR-CON-32)
- **Change draft and approval**: Proposed setting/flag values and timing, a version identity,
  impact review, the affected live values reviewed, and approval of that exact version;
  editing invalidates approval, and a scheduled change cannot publish against an
  intervening changed live value without fresh preview and approval.
  (PRD: FR-CON-05–07)
- **Publication and version**: The attributable event making approved values effective
  now or later; rollback creates a new event referencing earlier values. An overdue
  publication retains its scheduled time and records its actual publication time;
  recovery retries cannot create duplicate effective versions. (PRD: FR-CON-05,
  FR-CON-08, FR-CON-37)
- **Audit entry**: Protected, searchable evidence of actor, action, subject, time, reason
  and relevant before/after, without authentication secrets or private content.
  (PRD: FR-CON-37; NFR-SEC-09)
- **Managed message**: Persian and English wording for a surface and situation, including
  accessible names and validation messages. (PRD: FR-UX-01, FR-UX-14)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In all access acceptance cases, 100% of non-admin attempts and admin attempts
  without a valid completed second factor are refused; both permitted factor paths admit
  eligible admins and ordinary sessions confer no console access. After authorised recovery,
  100% of that admin's prior console sessions and second-factor credentials are rejected;
  console access requires fresh enrolment and verification.
  (PRD: FR-CON-01–03; evidence: AC-ADM-001–006, AC-ADM-043)
- **SC-002**: 100% of successful setting/flag publications have an exact-version preview,
  valid approval, authorised effective time and attributable publication record. No draft
  save, invalidated schedule or repeated publication causes an unintended live change;
  100% of scheduled changes whose affected live value changed after approval are blocked
  until fresh preview and approval against the latest live value. Overdue publications
  take effect once on recovery only if all current prerequisites pass; otherwise they
  remain blocked, and no successful overdue change is backdated.
  (PRD: FR-CON-05–10; evidence: AC-ADM-010–011, AC-ADM-014–020, AC-ADM-034, AC-ADM-044–045)
- **SC-003**: All ten initial settings expose the required bilingual labels, units,
  owner-approved finite bounds or permitted format choices, and history. Both boundaries
  of every numeric range are accepted; out-of-range values, disallowed format sets and
  any unknown lifetime impact block publication. Initial defaults remain unchanged.
  (PRD: FR-CON-32; evidence: AC-ADM-008–009, AC-ADM-012–013, AC-ADM-037–038)
- **SC-004**: Exactly six initial flags are OFF; 100% of enablement attempts with failed,
  unknown or undelivered readiness are blocked without representing a capability as built.
  (PRD: FR-CON-32; §6.2; evidence: AC-ADM-033–036)
- **SC-005**: Every successful foundation administrative action has searchable, individually
  attributed evidence; 0 audit-edit/deletion attempts succeed and 0 inspected records
  expose the prohibited private data or secrets.
  (PRD: FR-CON-03–04, FR-CON-37; evidence: AC-ADM-021–025)
- **SC-006**: Every foundation screen passes the FA/EN × light/dark/system review at mobile,
  tablet and desktop sizes; all language/theme switching cases retain unsaved input and
  correct direction, and 100% of reviewed UI messages have both language versions.
  (PRD: FR-CON-06, FR-CON-11, FR-UX-01–07, FR-UX-14; evidence: AC-ADM-026–029, AC-ADM-031)
- **SC-007**: Every foundation task passes keyboard-only and assistive-reading walkthroughs
  and applicable WCAG 2.2 AA checks, with 0 critical automated accessibility findings and
  no known AA failures; the additional target-size and reduced-motion rules also pass.
  Automated scanning alone does not establish conformance.
  (PRD: FR-UX-08–09; evidence: AC-ADM-030, AC-ADM-032)
- **SC-008**: In acceptance walkthroughs, an admin can prepare, preview, approve, publish,
  find the resulting audit evidence and roll back a permitted setting change without
  engineering assistance or direct modification of stored records.
  (PRD: FR-CON-05–08, FR-CON-32, FR-CON-37; evidence: AC-ADM-010–011, AC-ADM-016, AC-ADM-019,
  AC-ADM-021–022)
- **SC-009**: 100% of reviewed changes across the five published-policy groups explain
  their existing-item and new-item effects before approval. Lifetime previews identify
  next-sweep consequences; other previews explain the preservation of existing projects,
  photos, stored uploads and assigned attempt allowances as applicable, without executing
  those downstream actions. (PRD: FR-CON-32; §10.1;
  evidence: AC-ADM-012, AC-ADM-039–042; owner-supplied change rules)

## Assumptions

- **Owner clarifications**: Q1: A is resolved by the supplied nine finite numeric ranges
  and JPEG-mandatory format-subset policy, recorded in Initial Settings. Q2: A is resolved
  by original-reference-time recalculation and the next-sweep rule for shorter lifetimes.
  The owner also supplied preservation rules for card/photo limits, new-upload-only
  validation and future-queued-unit attempt allowances. These explicitly approved
  configuration policies preserve all initial defaults and do not silently redefine
  Q01–Q41. (PRD: FR-CON-32; §10.1; acceptance: AC-ADM-009, AC-ADM-012, AC-ADM-037–042)
- **Sources and authority**: This specification uses `docs/tankhor-prd.md` §0.4, §3.3, §7,
  E14, E17, §10 and §12.4; `docs/tankhor-technical-spec.md` §8 and §12.1–12.6;
  `docs/tankhor-decisions-qa.md` Q01–Q41, especially Q01–Q03 and Q23; and the project
  constitution. Product behaviour follows the PRD and final decisions. (PRD: §0.1, §7)
- **Scope**: This feature delivers admin access and recovery, the ten-setting registry,
  six-flag register, their review/publication history, foundation audit and the bilingual,
  accessible theme shell. There is no requirement here to build end-user sign-in (002),
  money/pricing (003), media processing or expiry (004), AI providers/emergency stop (005),
  content pages/catalogue or a general translation editor (006), reports/operations (013),
  or the behaviour behind the OFF capabilities (later features). Their names/defaults are
  registered here only. The new change-effect policies are recorded and explained in the
  management experience here; operational enforcement remains with those owning features.
  (PRD: FR-CON-01–11, FR-CON-32, FR-CON-37; §6.1–6.2)
- **Initial access dependency**: Acceptance begins with an owner-authorised initial admin
  identity and enrolled factors. Public admin self-registration and a last-admin bypass
  are not authorised. The prompt's illustrative "correct password" means successful
  primary identity verification, not permission to add password-based sign-in contrary
  to Q09. End-user authentication journeys remain outside this feature.
  (PRD: FR-CON-01–03; §10.4; Q09, Q23)
- **Approval actors**: No rule requires a second person to approve ordinary setting/flag
  changes; the same admin may perform the distinct preview, approval and publication steps.
  Adding or recovering an admin still requires another admin. No new permission tiers or
  mandatory two-person publishing rule are introduced. (PRD: FR-CON-03–07)
- **Impact/readiness dependencies**: Counts and capability health must be supplied by the
  relevant owning functionality when available. A genuinely empty population may report
  zero; absent functionality or unavailable data must report unknown and cannot pass the
  associated publication gate. This feature does not claim later capabilities are built.
  (PRD: FR-CON-32; §0.4, §6.2)
- **Copy and brand dependencies**: Foundation screens use managed bilingual messages;
  general content-panel editing and production brand-asset delivery remain separate work.
  No separate console logo or speculative brand asset is introduced.
  (PRD: FR-UX-06–07, FR-UX-14–15, FR-UX-17; Q03–Q04)
- **Audit reasons**: Deliberate changes, additions and recoveries require a human reason;
  routine sign-in or scheduled execution can have a descriptive event reason rather than
  an unnecessary text-entry step. Secret values are never captured as before/after content.
  (PRD: FR-CON-03–04, FR-CON-37; §13.1)
- **Guardrail boundary**: The foundation disallows bypass controls; it does not claim to
  implement or prove the wallet, private-media, AI-budget or generation behaviours of
  GR1–GR9, which remain acceptance obligations of their owning features.
  (PRD: FR-CON-32; §5.3)
- **Traceability and test-first delivery**: Every local AC names its driving behavioural
  test. Planning must preserve those names and citations, and tasks must place the test
  before its implementation. Source AC-CON-1's hero-video editing and the rest of J5's
  pricing/incident journey remain later work; only the shared management pattern is covered
  here. (PRD: FR-CON-05–10, FR-CON-37; AC-CON-1; J5; constitution Principles I and VIII)
