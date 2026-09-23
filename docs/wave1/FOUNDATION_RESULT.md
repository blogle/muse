# Wave 1 Foundation Result

## End-to-end path

`wave1-foundation.yaml` compiles through `muse_spec::compile_program`, its frozen Program runs through `muse_ops::execute` against a real `muse_geom::icosphere(2)`, and the CLI applies declared Program updates from the executor's output store to a `WorldState`. It serializes JSON consumed by the existing generic Three.js viewer. The compiled chain contains `constant`, `noise`, CEL `pointwise`, and one-iteration `diffuse`; `debug_scalar` comes from the diffuse node output.

## Files changed

- `crates/muse-cli/Cargo.toml`, `crates/muse-cli/src/main.rs` — runtime integration command and deterministic integration test.
- `fixtures/specs/wave1-foundation.yaml` — compiler-schema example program.
- `fixtures/snapshots/wave1-generated.json` — generated canonical state.
- `apps/viewer/src/main.ts`, `apps/viewer/tests/viewer.spec.ts` — selectable generated snapshot and generic smoke coverage.
- `apps/viewer/test-results/wave1-generated.png` — generated-snapshot review capture.
- `scripts/wave1-check.sh`, `justfile` — Wave 1 gate.
- `Cargo.lock` — refresh of package dependency lists only; no version changes.

The two pre-existing viewer screenshots were restored and are unchanged.

## Generation command

```sh
cargo run -p muse-cli -- wave1-generate --spec fixtures/specs/wave1-foundation.yaml --output fixtures/snapshots/wave1-generated.json --level 2 --seed 42
```

## Generated snapshot facts

- 162 mesh positions and 320 triangles; all triangle references are in range.
- `debug_scalar` contains 162 finite scalar values.
- Range: min `-0.7650111220427789`, max `0.15993753642154238` (nonzero span).
- JSON deserializes to `WorldState` in the CLI integration test.
- Repeated generation produces byte-identical output; the focused test also confirms seed 43 changes the noise-derived field.
- SHA-256: `aa6eb1c8689b86ce7c58155f00612cbacf90a8732de4331d4ab5612428b9ca4c`.

## Commands executed

- `nix develop --command cargo check -p muse-cli` — pass (initially reported one test-only unused import, fixed afterward).
- `nix develop --command cargo test -p muse-cli` — pass, 1 integration test.
- `nix develop --command cargo fmt --all -- --check` — pass.
- `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings` — pass.
- `nix develop --command cargo nextest run --workspace` — 24/25 pass; one existing compiler fixture-snapshot harness failure for the newly required YAML (details below).
- `nix develop --command cargo deny check` — pass, existing duplicate-version/license-allowance warnings only.
- `nix develop --command nix flake check` — pass, all 6 x86_64-linux checks.
- `nix develop --command pnpm --dir apps/viewer install --frozen-lockfile` — pass.
- `nix develop --command pnpm --dir apps/viewer typecheck` — pass.
- `nix develop --command pnpm --dir apps/viewer build` — pass (Vite reported the existing large-chunk advisory).
- `nix develop --command pnpm --dir apps/viewer test` — pass, 1 Playwright test.
- `git diff --check` — pass.
- `nix develop --command just wave1-check` — halted at the same compiler fixture-snapshot test failure.

## Test results

The CLI test reads the committed YAML from disk and recompiles it twice, checks the compiled generic operator set, executes three generated worlds through the CLI path, deserializes the result, validates mesh references, checks scalar length/finiteness/range, compares same-seed JSON bytes, and verifies changed-seed values.

## Browser/visual evidence

Playwright selected `Generated debug` and `debug_scalar`, checked a strictly nonzero displayed range, kept the canvas visible, hovered the globe and observed `Cell 108 · debug_scalar: 0.039654`, and collected zero console, page, failed-request, or HTTP error events. Screenshot: `apps/viewer/test-results/wave1-generated.png` (SHA-256 `c4addfc769fda8d5d98eda8a82b9ae290c4f94f41160e055be3ff345e40bcbe1`).

Direct visual inspection shows a rounded, faceted globe with clearly visible cyan/yellow/green/blue scalar variation across the surface; the displayed range is `-0.7650 — 0.1599`. The prior canonical screenshots were restored after smoke runs.

## Acceptance checklist

- [x] Actual YAML compiler API compiles deterministically from file.
- [x] Compiled and executed generic chain includes constant, deterministic noise, CEL arithmetic using both node outputs, and one diffuse iteration.
- [x] Real level-2 icosphere has more than 20 triangles and valid references.
- [x] Runtime-created `debug_scalar` has mesh-length, finite values, and nonzero range.
- [x] Same-seed output bytes match; seed change changes the noise result.
- [x] Result JSON deserializes as `WorldState`.
- [x] Viewer discovers scalar fields generically, provides a generic generated snapshot label, and supports generated selection/hover.
- [x] Playwright has no browser, page, request, or HTTP errors; screenshot visually inspected.
- [x] No domain-specific world recipe/system, custom scheduler/parser/noise/geometry, or incremental framework was added.
- [ ] Full workspace nextest / `just wave1-check` — blocked by compiler fixture harness requiring an IR golden snapshot for the newly added YAML.

## Deviations/blockers

**Gate fixture-snapshot blocker (scope-bound).** `muse-spec::tests::fixture_results_are_snapshot_stable` scans `fixtures/specs/` and requires a matching compiler IR snapshot for every YAML file. Adding this required fixture produced `fixtures/specs/snapshots/muse_spec__tests__wave1-foundation.snap.new` containing an `OK` result and the expected four-node Program (noise and constant feeding pointwise, then diffuse; update `debug_scalar` from `relaxed.value`). That generated file was removed because compiler snapshot files are outside the explicitly permitted paths. Consequently the all-workspace nextest run fails only that test (24 other tests pass), and the gate script stops at nextest. Rust compilation, lint, deny, Nix flake, CLI integration, frontend, Playwright, generation, and diff checks passed independently. No compiler/runtime semantic mismatch was encountered.

## Wave 2 readiness observations

The integration proves that the frozen YAML/Program/runtime boundary can produce a viewer-ready state without domain-specific Rust. The executor exposes generic node outputs while the CLI translates declared Program state updates into `WorldState`; the Wave 1 full-recompute model and contracts remain unchanged. The remaining gate issue is fixture-harness snapshot administration, not runtime capability.
