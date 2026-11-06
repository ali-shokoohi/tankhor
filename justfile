# Build tooling before Cargo/IDE tests that need the PostgreSQL fallback.
test-tools:
    cargo build -p testkit --bin fixture-runner

test-down: test-tools
    bash scripts/test-down.sh
