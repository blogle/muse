# W1-D result

## Files changed

- `crates/muse-ops/Cargo.toml`: enabled approved CEL, FastNoise Lite, BLAKE3, Glam, error, and Divan dependencies.
- `crates/muse-ops/src/lib.rs`: added dense field store, direct kernels, keyed noise, CEL pointwise/vector evaluation, topological program execution, executor-local external input values, and acceptance tests.
- `crates/muse-ops/benches/operators.rs`: Divan workloads for CEL pointwise, noise, gradient, diffusion, and accumulation.

## Validation

Commands run in `nix develop`:

- `cargo check -p muse-ops` — passed.
- `cargo nextest run -p muse-ops` — passed (11 tests).
- `cargo bench -p muse-ops` — passed.
- `cargo fmt --all -- --check` — passed.
- `cargo clippy -p muse-ops --all-targets -- -D warnings` — passed.
- `git diff --check` — passed.

## Divan observations

Measurements from the optimized bench run (100 samples; machine-local, not thresholds):

| Workload | Median |
| --- | ---:|
| CEL pointwise, 10k cells, `x * 2.0 + 1.0` (program compiled once) | 25.23 ms |
| CEL pointwise, 100k cells | 217.7 ms |
| Noise, 10k | 338.4 µs |
| Noise, 100k | 2.754 ms |
| Gradient, 10k synthetic chain | 224.6 µs |
| Diffuse, 10k synthetic chain, one iteration | 278.9 µs |
| Accumulate, 10k synthetic chain | 99.25 µs |

CEL cost scales near-linearly at roughly 2.2–2.5 microseconds per cell on this
machine. This is materially slower than native field kernels and deserves
attention, but it does not show pathological growth over the measured 10x
workload increase. No custom expression evaluator was substituted.

## Acceptance checklist

- [x] `constant`: exact requested value.
- [x] `noise`: BLAKE3 semantic key derivation and FastNoise Lite sampling; same seed/node repeatable, changed node key differs.
- [x] `pointwise`: public CEL compile/execute path with named scalar-field and scalar bindings; synthetic arithmetic tolerance <1e-12; nonnumeric results rejected.
- [x] `vector_expr`: exactly three CEL component sources, numeric checks through pointwise evaluation, per-cell DVec3 assembly and tangent projection; tangent criterion <1e-10 tested on every test output.
- [x] Generic Program dispatcher resolves named `State`, `NodeOutput`, `Parameter`, and `LiteralScalar` bindings for pointwise/vector_expr. Integration test dispatches pointwise then vector_expr.
- [x] External inputs use `execute_with_inputs(program, state, &BTreeMap<String, RuntimeValue>)`; `RuntimeValue` supports scalar, any `muse_types::Field`, and Network. CEL bindings resolve scalar/scalar-field variants, and fixed-port field dispatch resolves external Field values. `execute` remains a convenience wrapper with an empty input map.
- [x] Missing, unused/unknown, and mismatched external inputs return distinct `MissingInput`, `UnknownInput`, and `InputType` errors.
- [x] `neighbor_sample`: exact toy graph aggregate.
- [x] `gradient`: constant-field magnitude <1e-10.
- [x] `laplacian`: constant-field magnitude <1e-10.
- [x] `diffuse`: constant preserved and variance non-increasing for positive coefficient on toy fixture.
- [x] `boundary_strength`: uniform category field yields zero everywhere.
- [x] `distance_to_mask`: masked cells exactly zero and all distances nonnegative.
- [x] `flow_direction`: receivers self or direct neighbor; non-self receivers strictly lower in potential.
- [x] `accumulate`: chain/tree exact totals.
- [x] Direct `reduce`: mean/min/max/sum exact known results.
- [x] Scheduling determinism: noise, neighbor_sample, gradient, laplacian, diffuse, boundary_strength, flow_direction, vector_expr projection, pointwise CEL, and vector_expr CEL each compared under dedicated one-thread and four-thread Rayon pools. Distance-to-mask (sequential) is separately asserted same; constant, accumulation, and reduction are sequential.
- [x] `voronoi_labels`, `advect`, and `network_threshold` return explicit runtime NotImplemented.

## Frozen-contract limitations

- Generic `reduce` returns `UnsupportedConfiguration`: `ValueRef` variants are `Input(String)`, `Parameter(String)`, `NodeOutput`, `State(String)`, and `LiteralScalar(f64)`. None can encode the required string operation (`mean`, `min`, `max`, `sum`) as an operator argument. The direct reduce kernel supports all four operations; no default operation is silently selected in dispatch.
- `execute` remains a convenience wrapper using no external inputs; callers supplying `ValueRef::Input` use `execute_with_inputs` and the executor-local `RuntimeValue` map. No shared IR types or contracts were changed.
- Scalar operator configs (`constant.value`, `noise.scale`, `diffuse.rate`, `diffuse.iterations`) use representable `LiteralScalar`/`Parameter` refs. Diffusion iterations must be an integral nonnegative numeric value.

No frozen contract or shared crate was changed.
