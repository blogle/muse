# W1-F Timely/Differential adoption spike result

## Status

Completed within the experiment-only ownership boundary. Recommendation: **REJECT-FOR-POC** (see `DECISION.md`). No production dependency or operator interface was changed.

## Files

- `experiments/incremental-runtime/Cargo.toml` — isolated manifest and exact direct framework pins
- `experiments/incremental-runtime/Cargo.lock` — resolved experiment-local graph
- `experiments/incremental-runtime/src/main.rs` — dense/Rayon baseline, persistent Differential pipeline, tests, and repeated timing harness
- `experiments/incremental-runtime/DECISION.md` — criteria/evidence and recommendation
- `experiments/incremental-runtime/RESULT.md` — this report

The pre-existing `BLOCKER.md` was removed after the compatible pair and caller implementation were verified.

## Dependency evidence

`differential-dataflow 0.25.1` uses `timely 0.31.0` and resolves `columnar 0.13.2`; no transitive intervention is present. `serde 1.0.229` supports Timely exchange serialization of `Number(f64)`, while its ordering implementation is local and uses `total_cmp`.

## Correctness and execution lifecycle

- Mean tolerance: absolute `1e-10`; threshold counts exact.
- Initial/1%/25% scenarios match at 10,000 and 100,000 cells.
- Updates happen on the same graph and scalar `InputSession` per run. The 25% epoch adds edits for indices 1% through 25% after the first 1% epoch, resulting in exactly 25% net changed input cells.
- Tiny persistent 100-cell initial and 1% update test passes. The progress defect was a non-advanced static edge-input frontier; advancing/flushing it to `u64::MAX` before scalar epochs allowed the downstream probe to progress. Probe driving has a bounded diagnostic guard.
- All other details and observed numeric values are in `DECISION.md`.

## Measurements

The release harness ran one warm-up and five measured repetitions per dataset size, reporting medians for all scenarios. Timings, graph-construction accounting, summary-extraction accounting, process RSS sampling method, LOC, and adoption criteria are recorded in `DECISION.md`.

## Checks

All required experiment-local commands passed:

```text
nix develop --command cargo check --manifest-path experiments/incremental-runtime/Cargo.toml
nix develop --command cargo test --manifest-path experiments/incremental-runtime/Cargo.toml
nix develop --command cargo clippy --manifest-path experiments/incremental-runtime/Cargo.toml --all-targets -- -D warnings
nix develop --command cargo fmt --manifest-path experiments/incremental-runtime/Cargo.toml -- --check
nix develop --command experiments/incremental-runtime/target/release/incremental-runtime-spike
git diff --check
```

The release timing harness used the exact 10k/100k sizes and initial, 1%, and 25% scenarios. Full Cargo tests include a separate persistent 100-cell frontier smoke test and 10k/100k correctness coverage. Commit and push status are reported by the coordinator after repository review.
