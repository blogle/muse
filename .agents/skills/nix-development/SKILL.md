---
name: nix-development
description: Use this skill for every Nix, flake, dev-shell, reproducible-build, cache, CI, packaging, OCI-image, or build-environment task in this repository. Read it before changing flake.nix or choosing Nix versus native build commands, even when the request only mentions entering the environment, a cache, or a build failure.
---

# Nix Development

Use Nix to define reproducible toolchains, environments, and artifacts. Use the
native build system for fast mutable development loops. Read
[`references/guidelines.md`](references/guidelines.md) for the complete project
guidance before work involving derivations, CI, images, caching, or parallelism.
Read `../../../docs/development-policy.md` before dependency or performance
decisions; it is the canonical repository policy.

## Required workflow

1. Enter the environment with `nix develop`; its shell must not build the application.
2. Run native validation inside that environment: for Rust, begin with `cargo check`, then targeted tests, then broader checks.
3. Keep mutable caches, including `target/`, in stable writable locations outside the Nix store. Preserve them across resumable work; do not routinely clean them.
4. Use `nix build` only for reproducibility, packaging, deployment, or CI boundaries.
5. Make source inputs and derivations proportional to their semantic scope. Keep stable dependencies separate from frequently changed source and configuration.

## Quality gates

- Do not make a dev shell depend on the application package through `inputsFrom` or an equivalent build-triggering input.
- Do not use the whole repository as a derivation source when a smaller source set is meaningful.
- Control total Nix, Cargo/compiler, and linker concurrency; measure before adding linker or compiler tweaks.
- Keep portable artifacts free of `target-cpu=native`.
- Keep fast validation separate from production optimization. When Crane/CI is introduced, build Cargo dependencies once and reuse the artifact across checks.
- For OCI work, retain stable content-addressed layers, avoid recursive metadata rewrites and Docker archive round-trips, and keep writable state at runtime.
