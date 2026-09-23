# muse

MUSE compiler and execution-engine bootstrap with a reproducible Nix environment.

## Development

`nix develop` provides the Rust toolchain and ordinary development tools. It does
not build the application. Cargo remains the fast native edit loop and preserves
its untracked `target/` directory between commands.

```sh
nix develop
cargo check --workspace
cargo nextest run --workspace
```

Before completing Rust changes, run formatting, the narrowest relevant tests,
then broader validation:

```sh
cargo nextest run --workspace
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo deny check
cargo build --workspace --release
cargo bench --workspace
nix flake check
```

`just` exposes aliases for these commands; the commands above remain authoritative.
`just profile-coz` intentionally fails until a canonical simulation workload exists.

See `AGENTS.md`, `docs/development-policy.md`, and the repository-local skills for
mandatory agent workflow.
