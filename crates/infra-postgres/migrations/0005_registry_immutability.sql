-- Runtime can seed/read registries and move checked pointers, never rewrite policy/history.

CREATE FUNCTION reject_registry_history_mutation() RETURNS TRIGGER
LANGUAGE plpgsql
SECURITY INVOKER
AS $$
BEGIN
    RAISE EXCEPTION 'registry version history is immutable';
END;
$$;

CREATE TRIGGER setting_versions_reject_update_delete
BEFORE UPDATE OR DELETE ON setting_versions
FOR EACH ROW EXECUTE FUNCTION reject_registry_history_mutation();
CREATE TRIGGER setting_versions_reject_truncate
BEFORE TRUNCATE ON setting_versions
FOR EACH STATEMENT EXECUTE FUNCTION reject_registry_history_mutation();
CREATE TRIGGER flag_versions_reject_update_delete
BEFORE UPDATE OR DELETE ON flag_versions
FOR EACH ROW EXECUTE FUNCTION reject_registry_history_mutation();
CREATE TRIGGER flag_versions_reject_truncate
BEFORE TRUNCATE ON flag_versions
FOR EACH STATEMENT EXECUTE FUNCTION reject_registry_history_mutation();

GRANT SELECT, INSERT ON settings, setting_versions, flags, flag_versions
    TO tankhor_fixture_runtime;
GRANT UPDATE (current_version_id) ON settings TO tankhor_fixture_runtime;
GRANT UPDATE (current_version_id) ON flags TO tankhor_fixture_runtime;
REVOKE ALL ON FUNCTION reject_registry_history_mutation() FROM PUBLIC;
