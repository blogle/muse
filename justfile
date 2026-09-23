default:
    @just --list

nextest:
    cargo nextest run --workspace

check:
    ./scripts/check.sh

fmt:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

deny:
    cargo deny check

build-release:
    cargo build --workspace --release

bench:
    ./scripts/bench.sh

flake-check:
    nix flake check

profile-coz *args:
    ./scripts/profile-coz.sh {{args}}
