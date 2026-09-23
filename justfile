default:
    @just --list

nextest:
    cargo nextest run --workspace

fmt:
    cargo fmt --all -- --check

clippy:
    cargo clippy --workspace --all-targets --all-features -- -D warnings

deny:
    cargo deny check

build-release:
    cargo build --workspace --release

bench:
    cargo bench --workspace

flake-check:
    nix flake check

profile-coz:
    @printf '%s\n' 'profile-coz is unavailable: MUSE has no canonical simulation workload yet.' >&2
    @false
