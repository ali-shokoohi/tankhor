#!/usr/bin/env bash
# Compose bind-backed secrets retain host ownership. Stage private service-readable copies
# in memory before the upstream entrypoint drops privileges; never chmod the host originals.
set -euo pipefail
set +x
for role in admin migration runtime preparation; do
    install -o postgres -g postgres -m 0600 "/run/secrets/postgres_$role" "/run/tankhor-secrets/postgres_$role"
done
chown postgres:postgres /run/tankhor-secrets
export POSTGRES_PASSWORD_FILE=/run/tankhor-secrets/postgres_admin
exec /usr/local/bin/docker-entrypoint.sh "$@"
