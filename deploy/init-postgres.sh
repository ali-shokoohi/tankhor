#!/usr/bin/env bash
# T027: developer-only initialization. Runtime never owns schema/database objects.
set -euo pipefail
set +x
for role in migration runtime preparation; do
    password="$(<"/run/tankhor-secrets/postgres_$role")"
    if [[ ! "$password" =~ ^[[:xdigit:]]{64}$ ]]; then
        printf '%s\n' 'developer database credentials must be generated 256-bit hex values' >&2
        exit 1
    fi
done
# No password in argv, environment, stdout or SQL statement logs. psql expands these
# quoted values from stdin; generated hex credentials contain no SQL metacharacters.
{
    printf '%s\n' "SET log_statement = 'none';" "SET log_min_error_statement = 'panic';"
    for role in tankhor_migration tankhor_runtime tankhor_preparation; do
        password="$(<"/run/tankhor-secrets/postgres_${role#tankhor_}")"
        printf 'CREATE ROLE %s LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS PASSWORD '\''%s'\'';\n' "$role" "$password"
    done
    printf '%s\n' \
        'ALTER DATABASE tankhor OWNER TO tankhor_migration;' \
        'REVOKE CREATE ON SCHEMA public FROM PUBLIC;' \
        'ALTER SCHEMA public OWNER TO tankhor_migration;'
} | psql --no-psqlrc --quiet --set ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB"
unset password
