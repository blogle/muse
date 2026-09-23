# W1-C Result

## Files changed

- `crates/muse-spec/src/lib.rs`, `crates/muse-spec/Cargo.toml`
- `fixtures/specs/*.yaml` and `fixtures/specs/snapshots/*.snap`

## Implemented

- Recipe `use:` expansion binds declared inputs and parameters, detects recursion, namespaces node IDs, checks declared binding types, and resolves recipe outputs as compile-time aliases.
- Expression bindings enforce scalar-field inputs for `pointwise`, vector-field inputs for `vector_expr`, scalar-only parameters, and reject colliding input/parameter names.
- Static scalar configuration schemas enforce accepted keys, scalar types, and required values without altering `PortSpec`.
- References validate operator output names; `petgraph` provides cycle detection/topological sorting; CEL source is validated and retained in the frozen IR handle.
- Required invalid-category fixtures and snapshots plus valid minimal, pointwise, and multiple-use recipe fixtures are included. Repeated recipe compilation asserts identical debug output and verifies alias-expanded dependency order.

## Validation

- `nix develop --command cargo check -p muse-spec` — passed.
- `nix develop --command cargo nextest run -p muse-spec` — passed (2 tests; fixture test validates expected categories and repeated recipe IR determinism).
- `nix develop --command cargo clippy -p muse-spec --all-targets -- -D warnings` — passed.
- `nix develop --command cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.

## Deviations / blockers

No known acceptance gaps remain for the bounded W1-C schema implemented here. Program outputs are validated as compile-time aliases/references and intentionally are not added to the frozen `Program` IR.
