-- Fixed editable business settings and immutable typed versions (data-model §2/§3).

CREATE TABLE settings (
    key TEXT PRIMARY KEY CHECK (key IN (
        'buyer_temp_photo_days', 'studio_temp_upload_days', 'studio_auto_draft_days',
        'account_recovery_days', 'studio_max_cards', 'buyer_max_permanent_photos',
        'upload_max_mb', 'upload_min_shortest_edge_px', 'generation_max_attempts',
        'upload_formats'
    )),
    label_key TEXT NOT NULL,
    unit TEXT NOT NULL,
    min_value BIGINT,
    max_value BIGINT,
    current_version_id UUID,
    CONSTRAINT settings_fixed_policy CHECK (
        label_key = 'setting-' || key || '-label'
        AND (
            (key = 'buyer_temp_photo_days' AND unit = 'days_from_original_upload' AND min_value = 1 AND max_value = 30)
            OR (key = 'studio_temp_upload_days' AND unit = 'days_from_original_upload' AND min_value = 7 AND max_value = 90)
            OR (key = 'studio_auto_draft_days' AND unit = 'days_from_last_activity' AND min_value = 1 AND max_value = 90)
            OR (key = 'account_recovery_days' AND unit = 'days_from_deletion_request' AND min_value = 14 AND max_value = 90)
            OR (key = 'studio_max_cards' AND unit = 'cards' AND min_value = 1 AND max_value = 100)
            OR (key = 'buyer_max_permanent_photos' AND unit = 'photos' AND min_value = 1 AND max_value = 10)
            OR (key = 'upload_max_mb' AND unit = 'megabytes' AND min_value = 1 AND max_value = 25)
            OR (key = 'upload_min_shortest_edge_px' AND unit = 'pixels' AND min_value = 256 AND max_value = 1024)
            OR (key = 'generation_max_attempts' AND unit = 'total_attempts' AND min_value = 1 AND max_value = 3)
            OR (key = 'upload_formats' AND unit = 'format_mask' AND min_value IS NULL AND max_value IS NULL)
        )
    )
);

CREATE TABLE setting_versions (
    id UUID PRIMARY KEY,
    setting_key TEXT NOT NULL REFERENCES settings(key) ON DELETE RESTRICT,
    numeric_value BIGINT,
    format_mask INTEGER,
    publication_id UUID,
    actor_ref UUID NOT NULL,
    reason_code TEXT NOT NULL CHECK (reason_code IN (
        'foundation_seed', 'draft_change', 'scheduled_publication'
    )),
    applied_at TIMESTAMPTZ NOT NULL,
    UNIQUE (publication_id, setting_key),
    UNIQUE (setting_key, id),
    CONSTRAINT setting_versions_exactly_one_typed_value CHECK (
        num_nonnulls(numeric_value, format_mask) = 1
    ),
    CONSTRAINT setting_versions_numeric_policy CHECK (
        numeric_value IS NULL OR (
            (setting_key = 'buyer_temp_photo_days' AND numeric_value BETWEEN 1 AND 30)
            OR (setting_key = 'studio_temp_upload_days' AND numeric_value BETWEEN 7 AND 90)
            OR (setting_key = 'studio_auto_draft_days' AND numeric_value BETWEEN 1 AND 90)
            OR (setting_key = 'account_recovery_days' AND numeric_value BETWEEN 14 AND 90)
            OR (setting_key = 'studio_max_cards' AND numeric_value BETWEEN 1 AND 100)
            OR (setting_key = 'buyer_max_permanent_photos' AND numeric_value BETWEEN 1 AND 10)
            OR (setting_key = 'upload_max_mb' AND numeric_value BETWEEN 1 AND 25)
            OR (setting_key = 'upload_min_shortest_edge_px' AND numeric_value BETWEEN 256 AND 1024)
            OR (setting_key = 'generation_max_attempts' AND numeric_value BETWEEN 1 AND 3)
        )
    ),
    CONSTRAINT setting_versions_format_policy CHECK (
        format_mask IS NULL OR (
            setting_key = 'upload_formats'
            AND format_mask BETWEEN 1 AND 15
            AND (format_mask & 1) = 1
        )
    ),
    CONSTRAINT setting_versions_value_matches_subject CHECK (
        (setting_key = 'upload_formats') = (format_mask IS NOT NULL)
    )
);

ALTER TABLE settings
    ADD CONSTRAINT settings_matching_live_version
    FOREIGN KEY (key, current_version_id)
    REFERENCES setting_versions(setting_key, id)
    DEFERRABLE INITIALLY DEFERRED;

CREATE FUNCTION require_setting_live_version() RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY INVOKER
AS $$
DECLARE
    live_version UUID;
BEGIN
    SELECT current_version_id INTO live_version FROM settings WHERE key = NEW.key;
    IF live_version IS NULL THEN
        RAISE EXCEPTION 'setting live version is required at commit';
    END IF;
    RETURN NULL;
END;
$$;

CREATE CONSTRAINT TRIGGER settings_live_version_required
AFTER INSERT OR UPDATE OF current_version_id ON settings
DEFERRABLE INITIALLY DEFERRED
FOR EACH ROW EXECUTE FUNCTION require_setting_live_version();

CREATE INDEX setting_versions_subject_applied
    ON setting_versions (setting_key, applied_at, id);

REVOKE ALL ON settings, setting_versions FROM PUBLIC;
REVOKE ALL ON settings, setting_versions FROM tankhor_fixture_runtime;
REVOKE ALL ON FUNCTION require_setting_live_version() FROM PUBLIC;
