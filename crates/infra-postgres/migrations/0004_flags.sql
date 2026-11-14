-- Six registered launch capabilities; registration is not delivery or enablement.

CREATE TABLE flags (
    key TEXT PRIMARY KEY CHECK (key IN (
        'partner_widget_api', 'percentage_rollout', 'per_user_usage_limits',
        'ticketing', 'external_notifications', 'paid_share_billing'
    )),
    label_key TEXT NOT NULL CHECK (label_key = 'flag-' || key || '-label'),
    description_key TEXT NOT NULL CHECK (description_key = 'flag-' || key || '-description'),
    current_version_id UUID
);

CREATE TABLE flag_versions (
    id UUID PRIMARY KEY,
    flag_key TEXT NOT NULL REFERENCES flags(key) ON DELETE RESTRICT,
    enabled BOOLEAN NOT NULL,
    publication_id UUID,
    actor_ref UUID NOT NULL,
    reason_code TEXT NOT NULL CHECK (reason_code IN (
        'foundation_seed', 'draft_change', 'scheduled_publication'
    )),
    applied_at TIMESTAMPTZ NOT NULL,
    UNIQUE (publication_id, flag_key),
    UNIQUE (flag_key, id)
);

ALTER TABLE flags
    ADD CONSTRAINT flags_matching_live_version
    FOREIGN KEY (key, current_version_id)
    REFERENCES flag_versions(flag_key, id)
    DEFERRABLE INITIALLY DEFERRED;

CREATE FUNCTION require_flag_live_version() RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY INVOKER
AS $$
DECLARE
    live_version UUID;
BEGIN
    SELECT current_version_id INTO live_version FROM flags WHERE key = NEW.key;
    IF live_version IS NULL THEN
        RAISE EXCEPTION 'flag live version is required at commit';
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER flags_live_version_required
AFTER INSERT OR UPDATE OF current_version_id ON flags
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION require_flag_live_version();

CREATE INDEX flag_versions_subject_applied
    ON flag_versions (flag_key, applied_at, id);

REVOKE ALL ON flags, flag_versions FROM PUBLIC;
REVOKE ALL ON flags, flag_versions FROM tankhor_fixture_runtime;
REVOKE ALL ON FUNCTION require_flag_live_version() FROM PUBLIC;
