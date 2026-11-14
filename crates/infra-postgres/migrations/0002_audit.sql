-- Typed, safe, append-only administrative history (data-model §6).
-- No free-text snapshots, secret payloads or cascading history mutations.

CREATE TABLE audit_log (
    id UUID PRIMARY KEY,
    event_key UUID NOT NULL UNIQUE,
    actor_type TEXT NOT NULL CHECK (actor_type = 'admin'),
    actor_id UUID NOT NULL,
    action TEXT NOT NULL CHECK (
        action IN (
            'signed_in', 'enrollment_started', 'enrollment_confirmed', 'invited',
            'recovered', 'published', 'scheduled_cancelled', 'foundation_seeded',
            'draft_saved', 'previewed'
        )
    ),
    subject_type TEXT NOT NULL CHECK (subject_type IN ('admin', 'publication', 'change')),
    subject_id UUID NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    reason_code TEXT NOT NULL CHECK (
        reason_code IN (
            'authentication', 'admin_addition', 'admin_recovery', 'admin_revoked',
            'scheduled_publication', 'foundation_seed', 'draft_change', 'preview_assessment'
        )
    ),
    before_eligibility TEXT CHECK (
        before_eligibility IN ('invited', 'active', 'recovering', 'deactivated', 'revoked')
    ),
    after_eligibility TEXT CHECK (
        after_eligibility IN ('invited', 'active', 'recovering', 'deactivated', 'revoked')
    ),
    before_eligible BOOLEAN,
    after_eligible BOOLEAN,
    factor TEXT CHECK (factor IN ('passkey', 'totp')),
    approver_id UUID,
    executed_by TEXT CHECK (executed_by = 'system_worker'),
    scheduled_at TIMESTAMPTZ,
    executed_at TIMESTAMPTZ,
    revision_id UUID,
    baseline_version_id UUID,
    revision_digest BYTEA CHECK (revision_digest IS NULL OR octet_length(revision_digest) = 32),
    before_value TEXT,
    after_value TEXT,
    draft_scheduled_at TIMESTAMPTZ,
    draft_scheduled_zone TEXT,
    CONSTRAINT audit_log_draft_schedule_complete CHECK (
        (draft_scheduled_at IS NULL) = (draft_scheduled_zone IS NULL)
    ),
    CONSTRAINT audit_log_change_identity_complete CHECK (
        (revision_id IS NULL) = (baseline_version_id IS NULL)
    ),
    CONSTRAINT audit_log_schedule_complete CHECK (
        num_nonnulls(executed_by, approver_id, scheduled_at, executed_at) IN (0, 4)
    ),
    CONSTRAINT audit_log_schedule_finite CHECK (
        executed_at IS NULL OR executed_at >= scheduled_at
    )
);

CREATE INDEX audit_log_occurred_id ON audit_log (occurred_at, id);
CREATE INDEX audit_log_actor_occurred_id ON audit_log (actor_id, occurred_at, id);
CREATE INDEX audit_log_action_occurred_id ON audit_log (action, occurred_at, id);
CREATE INDEX audit_log_subject_occurred_id ON audit_log (subject_id, occurred_at, id);

CREATE FUNCTION reject_audit_log_mutation() RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY INVOKER
AS $$
BEGIN
    RAISE EXCEPTION 'audit history is append-only';
END;
$$;

CREATE TRIGGER audit_log_reject_update_delete
BEFORE UPDATE OR DELETE ON audit_log
FOR EACH ROW EXECUTE FUNCTION reject_audit_log_mutation();

CREATE TRIGGER audit_log_reject_truncate
BEFORE TRUNCATE ON audit_log
FOR EACH STATEMENT EXECUTE FUNCTION reject_audit_log_mutation();

REVOKE ALL ON audit_log FROM PUBLIC;
REVOKE ALL ON audit_log FROM tankhor_fixture_runtime;
GRANT SELECT, INSERT ON audit_log TO tankhor_fixture_runtime;
REVOKE ALL ON FUNCTION reject_audit_log_mutation() FROM PUBLIC;
