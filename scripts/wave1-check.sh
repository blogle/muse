#!/usr/bin/env bash
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo deny check
nix flake check

snapshot=fixtures/snapshots/wave1-generated.json
expected=$(mktemp)
trap 'rm -f "$expected"' EXIT
cp "$snapshot" "$expected"
cargo run -p muse-cli -- wave1-generate --spec fixtures/specs/wave1-foundation.yaml --output "$snapshot" --level 2 --seed 42
cmp "$expected" "$snapshot"

pnpm --dir apps/viewer install --frozen-lockfile
pnpm --dir apps/viewer typecheck
pnpm --dir apps/viewer build
pnpm --dir apps/viewer test
git diff --check
