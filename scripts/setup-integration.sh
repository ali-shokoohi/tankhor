#!/usr/bin/env bash
# T018: export only an independently validated owned, SQL-ready fixture endpoint.
set -euo pipefail
root="$(dirname "$(dirname "$(realpath "$0")")")"
runner="${FIXTURE_RUNNER:-$root/target/debug/fixture-runner}"
if [[ -n "${FIXTURE_CONTAINER:-}" ]]; then
    exec "$runner" export-ready
fi
cargo build --quiet --manifest-path "$root/Cargo.toml" -p testkit --bin fixture-runner
exec "$runner" shared-setup
