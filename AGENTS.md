# Agent Instructions

Repository-local skills are mandatory process guidance:

- Read `.agents/skills/nix-development/SKILL.md` before Nix, build-environment, reproducibility, cache, CI, or packaging work.
- Read `.agents/skills/rust-development/SKILL.md` before Rust source, Cargo manifest, profile, dependency, test, lint, or build work.
- Read `.agents/skills/skill-creator/SKILL.md` before creating or modifying any skill under `.agents/skills/`.

Read `docs/development-policy.md` before dependency, architecture, performance, or
profiling work. Only the coordinator edits root `[workspace.dependencies]`; use
`DEPENDENCY_REQUEST.md` for a missing capability. Keep `target/` persistent and
untracked; do not routinely run `cargo clean`. Enter `nix develop`, then use
native Cargo commands for the edit loop.
