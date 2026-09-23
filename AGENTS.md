# Agent Instructions

Repository-local skills are mandatory process guidance:

- Read `skills/nix-development/SKILL.md` before Nix, build-environment, reproducibility, cache, CI, or packaging work.
- Read `skills/rust-development/SKILL.md` before Rust source, Cargo manifest, profile, dependency, test, lint, or build work.
- Read `skills/skill-creator/SKILL.md` before creating or modifying any skill under `skills/`.

Use the narrowest valid validation first. Keep `target/` persistent and untracked; do not routinely run `cargo clean`. Enter the reproducible tool environment with `nix develop`, then use native Cargo commands for the edit loop.
