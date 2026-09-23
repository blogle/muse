---
name: rust-development
description: Use this skill for every Rust or Cargo task in this repository, including source edits, Cargo.toml changes, dependencies, profiles, compilation, tests, linting, CI, performance, linking, or release artifacts. Read it before running Cargo commands or changing Rust build configuration, even for a small one-file fix.
---

# Rust Development

Optimize the edit loop for fast, deterministic feedback while preserving a
separate path for reproducible CI and optimized production artifacts. Read
[`references/guidelines.md`](references/guidelines.md) for the complete project
guidance before changing profiles, caching, CI, parallelism, dependencies, or
release behavior.

## Required workflow

1. Work inside `nix develop`; use native Cargo, not Nix application builds, for iteration.
2. Preserve the untracked, per-worktree `target/` directory and incremental state. Never routinely run `cargo clean` or share one mutable target directory across unrelated concurrent worktrees.
3. Start with `cargo check` for the affected package or scope, then targeted tests, crate/workspace tests, and finally project-wide validation.
4. Use incremental debug development builds for persistent work. Treat incremental compilation and `sccache` as different tools: persistent development favors incremental state; ephemeral CI favors non-incremental builds, sccache, and shared dependency artifacts.

## Quality gates

- Run `cargo fmt -- --check`, `cargo check`, targeted or full `cargo test`, and `cargo clippy -- -D warnings` before completion when applicable.
- Keep development, fast CI, runnable CI, and production profiles separate. Do not impose release or maximum optimization on ordinary edits or PR checks.
- Do not use `-C target-cpu=native` for portable or reproducible output; use explicit supported CPU classes only when deployment compatibility is guaranteed.
- Control combined CI, Nix, Cargo, codegen, and linker parallelism. Measure compile and link bottlenecks before adding custom linkers or exotic compiler settings.
- Keep dependencies and enabled features narrow. When Crane/CI is added, build dependency artifacts once and reuse them for clippy, tests, docs, and builds.
