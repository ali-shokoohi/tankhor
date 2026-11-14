CREATE TABLE change_drafts (
    id uuid PRIMARY KEY,
    setting_key text REFERENCES settings(key),
    flag_key text REFERENCES flags(key),
    baseline_setting_version uuid,
    baseline_flag_version uuid,
    head_revision_id uuid,
    edit_counter bigint NOT NULL DEFAULT 0 CHECK (edit_counter >= 0),
    creator_id uuid NOT NULL REFERENCES admins(id),
    created_at timestamptz NOT NULL,
    CHECK ((setting_key IS NULL) <> (flag_key IS NULL)),
    CHECK ((baseline_setting_version IS NULL) <> (baseline_flag_version IS NULL)),
    CHECK ((setting_key IS NULL) = (baseline_setting_version IS NULL)),
    FOREIGN KEY (baseline_setting_version, setting_key)
        REFERENCES setting_versions(id, setting_key),
    FOREIGN KEY (baseline_flag_version, flag_key)
        REFERENCES flag_versions(id, flag_key)
);
CREATE UNIQUE INDEX change_drafts_one_open_per_subject ON change_drafts (
    COALESCE(setting_key, flag_key)
);
CREATE TABLE change_revisions (
    id uuid PRIMARY KEY,
    change_id uuid NOT NULL REFERENCES change_drafts(id),
    revision_number bigint NOT NULL CHECK (revision_number > 0),
    setting_key text,
    flag_key text,
    numeric_value bigint,
    format_mask integer CHECK (format_mask BETWEEN 1 AND 15 AND (format_mask & 1) = 1),
    boolean_value boolean,
    reason_code text NOT NULL CHECK (reason_code = 'draft_change'),
    scheduled_at timestamptz,
    scheduled_zone text CHECK (scheduled_zone IS NULL OR btrim(scheduled_zone) <> ''),
    version_hash bytea NOT NULL CHECK (octet_length(version_hash) = 32),
    created_by uuid NOT NULL REFERENCES admins(id),
    created_at timestamptz NOT NULL,
    CHECK ((setting_key IS NULL) <> (flag_key IS NULL)),
    CHECK ((numeric_value IS NOT NULL)::integer + (format_mask IS NOT NULL)::integer
        + (boolean_value IS NOT NULL)::integer = 1),
    CHECK ((setting_key IS NOT NULL) = (boolean_value IS NULL)),
    CHECK ((scheduled_at IS NULL) = (scheduled_zone IS NULL)),
    UNIQUE (change_id, revision_number),
    UNIQUE (id, change_id),
    UNIQUE (id, change_id, revision_number, setting_key, flag_key),
    UNIQUE (id, change_id, setting_key, flag_key),
    CHECK ((setting_key IS NULL) OR (format_mask IS NOT NULL) = (setting_key = 'upload_formats')),
    CHECK (numeric_value IS NULL OR (setting_key = 'buyer_temp_photo_days' AND numeric_value BETWEEN 1 AND 30)
        OR (setting_key = 'studio_temp_upload_days' AND numeric_value BETWEEN 7 AND 90)
        OR (setting_key = 'studio_auto_draft_days' AND numeric_value BETWEEN 1 AND 90)
        OR (setting_key = 'account_recovery_days' AND numeric_value BETWEEN 14 AND 90)
        OR (setting_key = 'studio_max_cards' AND numeric_value BETWEEN 1 AND 100)
        OR (setting_key = 'buyer_max_permanent_photos' AND numeric_value BETWEEN 1 AND 10)
        OR (setting_key = 'upload_max_mb' AND numeric_value BETWEEN 1 AND 25)
        OR (setting_key = 'upload_min_shortest_edge_px' AND numeric_value BETWEEN 256 AND 1024)
        OR (setting_key = 'generation_max_attempts' AND numeric_value BETWEEN 1 AND 3))
);
ALTER TABLE change_drafts ADD CONSTRAINT change_drafts_head_fk
    FOREIGN KEY (head_revision_id, id)
        REFERENCES change_revisions(id, change_id)
        DEFERRABLE INITIALLY DEFERRED;
CREATE FUNCTION require_draft_head() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT FROM change_drafts WHERE id = NEW.change_id AND head_revision_id = NEW.id
    ) THEN
        RAISE EXCEPTION 'change draft must reference an immutable head revision';
    END IF;
    RETURN NEW;
END
$$;
CREATE CONSTRAINT TRIGGER change_revisions_head_required
    AFTER INSERT ON change_revisions DEFERRABLE INITIALLY DEFERRED
    FOR EACH ROW EXECUTE FUNCTION require_draft_head();
CREATE INDEX change_revisions_draft_order ON change_revisions(change_id, revision_number DESC);
CREATE TRIGGER change_revisions_immutable_update BEFORE UPDATE ON change_revisions
    FOR EACH ROW EXECUTE FUNCTION reject_registry_history_mutation();
CREATE TRIGGER change_revisions_immutable_delete BEFORE DELETE ON change_revisions
    FOR EACH ROW EXECUTE FUNCTION reject_registry_history_mutation();
CREATE TRIGGER change_revisions_immutable_truncate BEFORE TRUNCATE ON change_revisions
    FOR EACH STATEMENT EXECUTE FUNCTION reject_registry_history_mutation();

CREATE TABLE change_previews (
    id uuid PRIMARY KEY,
    change_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    setting_key text,
    flag_key text,
    baseline_setting_version uuid,
    baseline_flag_version uuid,
    policy_identity text NOT NULL CHECK (btrim(policy_identity) <> ''),
    source_identity text NOT NULL CHECK (btrim(source_identity) <> ''),
    effect_rule text NOT NULL CHECK (btrim(effect_rule) <> ''),
    assessed_at timestamptz NOT NULL,
    reviewed_by uuid NOT NULL REFERENCES admins(id),
    version_hash bytea NOT NULL CHECK (octet_length(version_hash) = 32),
    readiness text CHECK (readiness IN ('not-delivered', 'unknown', 'failed', 'ready')),
    CHECK ((setting_key IS NULL) <> (flag_key IS NULL)),
    CHECK ((baseline_setting_version IS NULL) <> (baseline_flag_version IS NULL)),
    CHECK ((setting_key IS NULL) = (baseline_setting_version IS NULL)),
    UNIQUE (id, change_id),
    UNIQUE (id, change_id, revision_id, setting_key, flag_key,
        baseline_setting_version, baseline_flag_version),
    FOREIGN KEY (revision_id, change_id)
        REFERENCES change_revisions(id, change_id),
    FOREIGN KEY (baseline_setting_version, setting_key)
        REFERENCES setting_versions(id, setting_key),
    FOREIGN KEY (baseline_flag_version, flag_key)
        REFERENCES flag_versions(id, flag_key)
);
CREATE TABLE change_preview_evidence (
    preview_id uuid NOT NULL REFERENCES change_previews(id),
    evidence_index integer NOT NULL CHECK (evidence_index >= 0),
    population text NOT NULL CHECK (btrim(population) <> ''),
    known_count bigint CHECK (known_count >= 0),
    PRIMARY KEY (preview_id, evidence_index)
);
CREATE TABLE change_approvals (
    id uuid PRIMARY KEY,
    preview_id uuid NOT NULL,
    change_id uuid NOT NULL,
    revision_id uuid NOT NULL,
    setting_key text,
    flag_key text,
    baseline_setting_version uuid,
    baseline_flag_version uuid,
    approver_id uuid NOT NULL REFERENCES admins(id),
    approved_at timestamptz NOT NULL,
    FOREIGN KEY (preview_id, change_id)
        REFERENCES change_previews(id, change_id),
    UNIQUE (id, approver_id)
);
CREATE INDEX change_previews_exact_revision ON change_previews(change_id, revision_id, assessed_at DESC);
CREATE INDEX change_approvals_exact_revision ON change_approvals(change_id, revision_id, approved_at DESC);
DO $$
DECLARE table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY['change_drafts', 'change_revisions', 'change_previews',
        'change_preview_evidence', 'change_approvals']
    LOOP
        EXECUTE format('REVOKE ALL ON TABLE %I FROM tankhor_fixture_runtime', table_name);
        EXECUTE format('GRANT SELECT, INSERT ON TABLE %I TO tankhor_fixture_runtime', table_name);
    END LOOP;
END
$$;
GRANT UPDATE (head_revision_id, edit_counter) ON change_drafts TO tankhor_fixture_runtime;
DO $$
DECLARE table_name text;
BEGIN
    FOREACH table_name IN ARRAY ARRAY['change_previews', 'change_preview_evidence', 'change_approvals']
    LOOP
        EXECUTE format('CREATE TRIGGER %I BEFORE UPDATE ON %I FOR EACH ROW EXECUTE FUNCTION reject_registry_history_mutation()',
            table_name || '_immutable_update', table_name);
        EXECUTE format('CREATE TRIGGER %I BEFORE DELETE ON %I FOR EACH ROW EXECUTE FUNCTION reject_registry_history_mutation()',
            table_name || '_immutable_delete', table_name);
        EXECUTE format('CREATE TRIGGER %I BEFORE TRUNCATE ON %I FOR EACH STATEMENT EXECUTE FUNCTION reject_registry_history_mutation()',
            table_name || '_immutable_truncate', table_name);
    END LOOP;
END
$$;
