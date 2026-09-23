#!/usr/bin/env bash
set -euo pipefail

cargo check --workspace
cargo nextest run --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check
nix flake check
git diff --check
