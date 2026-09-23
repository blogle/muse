# Result

## Scope completed

Established the coordinator-owned shared state, compiler IR, operator descriptor registry, canonical snapshot fixtures, crate/workspace scaffolding, and frozen contract note. Incremental-runtime dependency setup is blocked pending a safely verified compatible Timely/Differential pair.

## Files changed

- `Cargo.toml`, `Cargo.lock`: workspace membership and coordinator-approved glam serde feature.
- `crates/muse-types/**`: shared types, IR, descriptor registry, and contract tests.
- `crates/muse-{geom,spec,ops,validate,cli}/**`: minimal compileable crate scaffolds.
- `fixtures/snapshots/canonical-small*.json`, `fixtures/specs/README.md`.
- `apps/viewer/README.md`, `scripts/README.md`.
- `docs/wave1/CONTRACT.md`, `docs/wave1/CONTRACT_RESULT.md`.
- `experiments/incremental-runtime/BLOCKER.md`.

## Commands executed

- `nix develop --command cargo check --workspace`
- `nix develop --command cargo fmt --all`
- `nix develop --command cargo check -p muse-types`
- `nix develop --command cargo nextest run -p muse-types`
- `nix develop --command cargo nextest run --workspace`
- `nix develop --command cargo fmt --all -- --check`
- `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `nix develop --command cargo deny check`
- `nix flake check`
- `git diff --check`
- Isolated experiment investigation: `cargo generate-lockfile`, `cargo check --manifest-path experiments/incremental-runtime/Cargo.toml`, and `cargo tree --manifest-path experiments/incremental-runtime/Cargo.toml -d` (candidate versions documented in `BLOCKER.md`).

## Test results

- Shared types: 4 passed, including fixture deserialization, in-bounds mesh/network references and network-value lengths, field/vector lengths, exact registry membership and uniqueness, critical signatures, and serde round trips.
- Workspace: 5 passed.
- Workspace cargo check and clippy: passed.
- cargo-deny: passed; existing duplicate-version and unmatched-license allowances were warnings only.
- Incremental dependency experiment: candidate compile failed on upstream Timely/Columnar API incompatibilities; see blocker.
- `nix flake check`: passed after the new source files were staged/tracked; one initial run before staging failed because Nix's git-source cleaning omitted untracked crates.

## Acceptance criteria

- [x] Required Rust crates and top-level directories are present.
- [x] Shared state, IR, and operator contracts compile and serialize.
- [x] Registry contains exactly the 16 requested ids; critical signatures are tested.
- [x] Both canonical JSON fixtures pass structural/length assertions.
- [x] W1-C/W1-D CEL boundary is documented using public `cel::Program::compile` and `execute` APIs.
- [ ] Incremental experiment manifest/versions: blocked pending compatible pair.
- [x] `nix flake check` passed for available x86_64-linux checks.
- [x] `git diff --check` passed before completion.

## Benchmarks / measurements

None; no runtime algorithms were implemented.

## Deviations

- Added serde feature to the centrally-approved glam workspace dependency as authorized.
- Operator descriptors leave expression-driven pointwise/vector_expr bindings empty; scalar configuration arguments are validated through compiler argument rules and not represented as fake typed ports.
- Timely/Differential are not committed as dependencies; a blocker is recorded instead of guessing/overriding their transitive graph.

## Blockers

- `experiments/incremental-runtime/BLOCKER.md`: tested versions did not produce a compatible buildable Timely/Differential dependency graph without unapproved transitive interventions.

## Follow-up observations

- Re-run `nix flake check` once new source files are staged/tracked.
- W1-C should implement explicit argument validation for scalar configuration keys and CEL expression bindings in addition to fixed descriptor port checks.
