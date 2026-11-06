#!/usr/bin/env bash
# T025: exact fixture scope only; no Docker prune or SQLx database sweep.
set -euo pipefail
root="$(dirname "$(dirname "$(realpath "$0")")")"
exec "$root/target/debug/fixture-runner" cleanup
