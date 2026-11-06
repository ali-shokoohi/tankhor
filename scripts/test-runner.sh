#!/usr/bin/env bash
# T023: Cargo sees discovery, normal executables and test binaries through this runner.
set -euo pipefail
binary="$1"
shift
name="$(basename "$binary")"
case "$name" in
    fixture_bootstrap_*|fixture-runner*) exec "$binary" "$@" ;;
esac
case "$binary" in
    */deps/integration_*) ;;
    *) exec "$binary" "$@" ;;
esac
for argument in "$@"; do
    case "$argument" in
        --list|--help|-h) exec "$binary" "$@" ;;
    esac
done
root="$(dirname "$(dirname "$(realpath "$0")")")"
exec "$root/target/debug/fixture-runner" run-child "$binary" "$@"
