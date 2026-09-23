# Verification

## Verdict

PASS

## Candidate

- Repository: `https://github.com/blogle/muse`
- Frozen base: `b1f144b2fd7e967c98d10a44c8ed0ff1ec95716f`
- Candidate HEAD inspected: `2b1eb463f0467d830685606218325f316de3797b` (`wave1/operators`)
- Candidate commits: `1edd53e`, `ee94395`, `2b1eb463f0467d830685606218325f316de3797b`

## Ownership / dependency review

- `git diff b1f144b2fd7e967c98d10a44c8ed0ff1ec95716f...HEAD --name-status` contains only `crates/muse-ops/Cargo.toml`, `RESULT.md`, `benches/operators.rs`, and `src/lib.rs`.
- Root manifests/lockfile, `muse-types`, `muse-spec`, fixtures, contract docs, and other crates have no candidate changes.
- `crates/muse-ops/Cargo.toml` direct dependencies use workspace-approved dependencies; the local `muse-types` path dependency is the existing internal crate.
- No unsafe, `ndarray`, GPU, async/distributed execution, custom scheduler, or domain-specific subsystem was found. Runtime `NotImplemented` is restricted to `voronoi_labels`, `advect`, and `network_threshold` (`src/lib.rs`, operator dispatch).
- Initial status was clean. Cargo validation added only the `muse-ops` package dependency entry to root `Cargo.lock`; after capturing that generated delta, it was restored. Final status before this report contained no implementation or lockfile changes.

## Runtime / dispatch review

- `FieldStore` stores dense `Value`s in a `BTreeMap`; operators consume/produce whole dense fields (`src/lib.rs:49-72`). Dispatch walks `Program.nodes` in the provided topological order without custom task scheduling (`src/lib.rs:407-641`). Rayon is used for whole-field operations.
- Executor-local external values are provided through `RuntimeValue` and `execute_with_inputs` (`src/lib.rs:41-47, 413-433`). References resolve for scalar arguments and fields, with clear missing/unknown/type errors. Tests prove a pointwise `ValueRef::Input` scalar field produces `[11, 12, 13, 14]`, external scalar CEL binding works, missing input errors, and fixed-port `neighbor_sample` can resolve an input field (`tests::external_scalar_field_and_scalar_inputs_resolve_in_cel_and_fixed_ports`, `tests::external_input_errors_distinguish_missing_unknown_and_type`). Frozen IR is unchanged.
- All required direct kernels are present: `constant`, `noise`, `pointwise`, `vector_expression`/`vector_expr`, `neighbor_sample`, `gradient`, `laplacian`, `diffuse`, `boundary_strength`, `distance_to_mask`, `flow_direction`, `accumulate`, and `reduce` (`src/lib.rs:87-405`). Direct `reduce` handles `mean`, `min`, `max`, and `sum` (`src/lib.rs:291-304`); generic dispatch returns `UnsupportedConfiguration` instead of silently choosing a mode (`src/lib.rs:614-616`).

## CEL / determinism review

- `PointwiseProgram::compile` uses approved `cel::Program::compile`; every cell evaluation uses public `cel::Program::execute(&cel::Context)` (`src/lib.rs:317-379`). It binds named scalar fields and scalar parameters and rejects nonnumeric results. Program expression bindings resolve `State`, `NodeOutput`, `Parameter`, `LiteralScalar`, and supported `Input` values (`src/lib.rs:485-549`). No custom parser/evaluator was found.
- `vector_expression` requires exactly three CEL sources, evaluates numeric components, forms `DVec3`, and projects onto the tangent plane (`src/lib.rs:381-405`). Tests reject wrong component count and boolean results.
- Noise derives key material from world seed and operator node key using BLAKE3, seeds approved FastNoise Lite, and derives per-cell input jitter from keyed BLAKE3 material (`src/lib.rs:91-113`). There is no mutable global RNG stream; cell mapping is independent and parallel scheduling cannot reorder semantic inputs.

## Commands executed

All commands were run independently as requested:

- `nix develop --command cargo check -p muse-ops` — passed.
- `nix develop --command cargo nextest run -p muse-ops` — passed, 11/11 tests.
- `nix develop --command cargo clippy -p muse-ops --all-targets -- -D warnings` — passed.
- `nix develop --command cargo bench -p muse-ops` — passed; Divan ran all operator benchmarks.
- `nix develop --command cargo fmt --all -- --check` — passed.
- `git diff --check` — passed (before and after restoring generated root lockfile churn).
- `git status --short --branch` — clean before validation; validation produced only `M Cargo.lock`; restored; clean after restoration and before writing this report.

## Numerical acceptance criteria

- [x] `constant` is exact (`tests::constant_and_noise_are_stable_and_node_keyed`).
- [x] Noise with the same world seed/node key is exact; changing the node key changes the tested output (`tests::constant_and_noise_are_stable_and_node_keyed`).
- [x] Pointwise known arithmetic is within `1e-12` (`tests::pointwise_executes_cel_over_named_dense_bindings`).
- [x] Vector expression tangent residual is below `1e-10` (`tests::generic_program_dispatches_pointwise_then_tangent_vector_expr`).
- [x] Neighbor sample toy graph is exact (`tests::graph_kernels_have_expected_values`).
- [x] Gradient of a constant field is below `1e-10` (`tests::uniform_and_diffusion_invariants`).
- [x] Laplacian of a constant field is below `1e-10` (`tests::uniform_and_diffusion_invariants`).
- [x] Diffusion preserves a constant field within `1e-10` (`tests::uniform_and_diffusion_invariants`).
- [x] Diffusion variance is non-increasing for a positive coefficient on the synthetic fixture (`tests::uniform_and_diffusion_invariants`).
- [x] Boundary strength for uniform categories is exactly zero (`tests::uniform_and_diffusion_invariants`).
- [x] Distance-to-mask gives masked cells exactly zero and nonnegative distances for the tested connected toy graph (`tests::distance_and_flow_are_valid`).
- [x] Flow receivers are self or direct neighbors; every non-self receiver has strictly lower potential (`tests::distance_and_flow_are_valid`).
- [x] Accumulation chain/tree synthetic totals are exact (`tests::graph_kernels_have_expected_values`).
- [x] Direct reduce mean/min/max/sum results are exact (`tests::graph_kernels_have_expected_values`).
- [x] Generic reduce does not silently select a mode (`tests::generic_reduce_reports_unrepresentable_operation_configuration`).
- [x] External input acceptance cases listed above pass (`tests::external_scalar_field_and_scalar_inputs_resolve_in_cel_and_fixed_ports`, `tests::external_input_errors_distinguish_missing_unknown_and_type`).

## Parallel correctness

`tests::parallel_operators_are_thread_count_deterministic` compares noise, neighbor sampling, gradient, Laplacian, diffusion, boundary strength, flow direction, vector projection, pointwise CEL, and vector-expression CEL in dedicated one-thread and four-thread Rayon pools. Distance-to-mask is also compared and is sequential. Constant, accumulation, and reduction are sequential. The pools exist only in the test harness; runtime has no custom scheduler.

## Benchmarks / performance

`crates/muse-ops/benches/operators.rs` benchmarks `PointwiseProgram::execute` with compiled CEL source `x * 2.0 + 1.0` over actual 10k and 100k field values, plus actual noise, gradient, one-iteration diffuse, and accumulation kernels. Gradient/diffuse use 10k synthetic chain topology rather than duplicated icosphere construction; accumulation uses a synthetic 10k receiver chain. No reduce/surrogate benchmark or alternate operator implementation is used.

Observed optimized Divan medians from this run:

| Workload | Median |
| --- | ---: |
| CEL pointwise, 10k | 33.12 ms |
| CEL pointwise, 100k | 248.6 ms |
| Noise, 10k | 367.6 µs |
| Gradient, 10k | 222.6 µs |
| Diffuse, 10k, one iteration | 305.4 µs |
| Accumulate, 10k | 147.8 µs |

CEL overhead is materially greater than native kernels (about 2.5 µs/cell at 10k and 2.49 µs/cell at 100k), but the 10x input increase is near-linear and does not show catastrophic per-cell overhead that blocks Wave 1. No replacement expression engine was introduced.

## Findings

- No blocking findings. All required representable behavior, ownership/dependency constraints, tests, and quality commands passed.

## Coordinator notes

- Frozen reduce-mode limitation: the frozen `ValueRef` set cannot carry the required string reduction mode. The direct API correctly supports all four modes; generic dispatch correctly returns explicit `UnsupportedConfiguration`. This is a frozen-contract limitation, not silent/faked behavior.
- Root lock integration: `cargo check` caused Cargo to add the W1-D package/dependency entry to the root `Cargo.lock`; this generated integration delta was recorded, then restored and excluded from verifier changes. Coordinator should integrate/retain the package lock entry when desired.
