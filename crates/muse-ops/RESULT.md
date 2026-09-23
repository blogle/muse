# W1-D result

## Files changed

- `crates/muse-ops/Cargo.toml`: enabled approved CEL, FastNoise Lite, BLAKE3, Glam, error, and Divan dependencies.
- `crates/muse-ops/src/lib.rs`: added dense field store, direct kernels, keyed noise, CEL pointwise evaluation, topological program execution, and focused tests.
- `crates/muse-ops/benches/operators.rs`: Divan workloads for CEL pointwise, noise, gradient, diffusion, and accumulation.

## Validation

Commands run in `nix develop`:

- `cargo check -p muse-ops` — passed.
- `cargo nextest run -p muse-ops` — passed (5 tests).
- `cargo bench -p muse-ops` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p muse-ops --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.

## Divan observations

Measurements from the optimized bench run (100 samples; machine-local, not thresholds):

| Workload | Median |
| --- | ---:|
| CEL pointwise, 10k cells, `x * 2.0 + 1.0` (program compiled once) | 24.95 ms |
| CEL pointwise, 100k cells | 305.3 ms |
| Noise, 10k | 387.4 µs |
| Noise, 100k | 3.202 ms |
| Gradient, 10k synthetic chain | 259.4 µs |
| Diffuse, 10k synthetic chain, one iteration | 347.1 µs |
| Accumulate, 10k synthetic chain | 141.1 µs |

CEL cost scales near-linearly at roughly 2.5–3.1 microseconds per cell on this
machine. This is materially slower than native field kernels and deserves
attention, but it does not show pathological growth over the measured 10x
workload increase. No custom expression evaluator was substituted.

## Acceptance status and known deviations

- Actual named scalar-binding CEL execution is provided by `PointwiseProgram`; the benchmark compiles `x * 2.0 + 1.0` before timing field execution.
- Implemented low-level kernels include constant, deterministic noise, neighbor average, gradient, Laplacian, diffusion, boundary strength, graph distance, flow direction, accumulation, reduction, and tangent projection.
- The generic `Program` dispatcher is not yet complete for all required semantics: `pointwise`/`vector_expr` are not wired through `CompiledNode` expression binding metadata; reduce dispatch uses mean; operation argument encoding is absent from the frozen `ValueRef` shape. Direct pointwise API is covered.
- The direct kernel surface currently leaves vector expression evaluation to caller-provided vectors and tangent projection; full CEL vector expression evaluation is not implemented.
- `voronoi_labels`, `advect`, and `network_threshold` return explicit NotImplemented from the dispatcher.
- Rayon single-/multi-thread equivalence acceptance has not been added for every parallel kernel.

These gaps are recorded rather than hidden; complete acceptance still needs follow-up against the compiler's binding/configuration representation.
