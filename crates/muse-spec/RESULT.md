# W1-C Result

## Files changed

- `crates/muse-spec/src/lib.rs`, `crates/muse-spec/Cargo.toml`
- `fixtures/specs/*.yaml` and `fixtures/specs/snapshots/*.snap`

## Implemented

- YAML deserialization, operator descriptor lookup, reference resolution, port checks, recipe recursion detection, dependency graph cycle checking/topological ordering, CEL source validation, and frozen `Program` construction.
- Twelve fixture snapshots, including deterministic debug output for successful compilation and error text for rejected fixtures.

## Validation

- `nix develop --command cargo check -p muse-spec` — passed.
- `nix develop --command cargo nextest run -p muse-spec` — passed (2 tests).
- `nix develop --command cargo clippy -p muse-spec --all-targets -- -D warnings` — passed.
- `nix develop --command cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

## Deviations / blockers

The compiler currently handles the Wave 1 linear recipe-node inclusion subset; recipe input/output substitution and complete binding type validation are not implemented. Inputs/parameters are represented as maps of names to declarations, and the frozen IR does not encode program outputs. These are remaining W1-C acceptance gaps.
