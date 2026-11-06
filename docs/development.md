# Tankhor — developer toolchain

This guide records the pinned tool versions for this repository and how to verify them.
Requirements and decisions live in the authoritative documents listed in `docs/README.md`.

## Pinned versions

| Tool | Version | Verify with |
| --- | --- | --- |
| Rust toolchain (rustc/cargo) | 1.98.1, edition 2024 | `rustc --version` (selected by `rust-toolchain.toml`) |
| rustfmt / clippy / llvm-tools | from the 1.98.1 toolchain | `rustup component list --installed --toolchain 1.98.1` |
| cargo-nextest | **0.9.151** (exact; setup scripts are experimental) | `cargo nextest --version` |
| cargo-deny | 0.20.2 | `cargo deny --version` |
| cargo-llvm-cov | 0.9.1 | `cargo llvm-cov --version` |
| cargo-mutants | 27.1.0 | `cargo mutants --version` |
| sqlx-cli | 0.9.0 | `cargo sqlx --version` |
| Python | 3 (3.14 verified); standard library only | `python3 --version` |
| Docker Engine / Compose | 29.x / v5.x | `docker --version`, `docker compose version` |
| Node.js / npm | 26.11.1 / 11.20.0 | `node --version`, `npm --version` |
| just | 1.59.0 | `just --version` |

Playwright and axe are test-only npm packages under `e2e/` and are set up separately,
before the browser suite is built.

## Installation

```sh
rustup toolchain install 1.98.1 --profile minimal \
  --component rustfmt --component clippy --component llvm-tools-preview
cargo install cargo-nextest --version 0.9.151 --locked
cargo install cargo-deny --version 0.20.2 --locked
cargo install cargo-llvm-cov --version 0.9.1 --locked
cargo install cargo-mutants --version 27.1.0 --locked
cargo install sqlx-cli --version 0.9.0 --locked --no-default-features --features rustls,postgres
cargo install just --version 1.59.0 --locked
```

Install Node.js 26.11.1 from the official release archive
(`node-v26.11.1-linux-x64.tar.xz`, checked against the release `SHASUMS256.txt`), and put
its `bin/` directory on `PATH` ahead of any system Node.

Do not install a newer cargo-nextest: the integration profile uses experimental setup
scripts, and their configuration syntax is pinned to 0.9.151.

## Verification

Run every command in the table above before running any task that uses that tool. A
missing or mismatched tool blocks setup; it is never a failing test.
