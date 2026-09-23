# muse

Minimal Rust crate bootstrap with a reproducible Nix development shell.

## Development

`nix develop` provides the Rust toolchain and ordinary development tools. It does
not build the application. Cargo remains the fast native edit loop and preserves
its untracked `target/` directory between commands.

```sh
nix develop
cargo check
cargo test
```

Before completing Rust changes, run formatting, the narrowest relevant tests,
then broader validation:

```sh
cargo fmt -- --check
cargo check
cargo test
cargo clippy -- -D warnings
nix flake check
```

See `AGENTS.md` and the repository-local skills for mandatory agent workflow.
