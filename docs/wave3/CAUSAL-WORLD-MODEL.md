# Wave 3 Causal World Model — Canonical Implementation Contract

This document freezes the Wave 3 contracts for implementation workers. It is normative and supplements the generic-algebra model in `SPEC.md`.

## Scope and exclusions

Wave 3 demonstrates that worlds are causal compositions of generic fields. Rust provides typed spatial/numerical algebra; YAML and CEL define worlds. Field roles are `forcing`, `state`, and `derived`; rendering assignments are descriptive metadata only. Physical-time persistence is not required: generation may solve a static or statistical equilibrium. No target-world recipes or domain-specific Rust systems are part of this foundation. The engine has no Star/Sun runtime concept: outside influences are ordinary forcing fields, and multiple sources combine by ordinary algebra.

## Geometry, vectors, and numerical conventions

The substrate is a unit sphere. Configured spatial scales are radians or normalized angular surface distances, never graph-hop counts in public Wave 3 APIs. Surface vectors are world-space `DVec3` values tangent to the unit sphere, with no implicit latitude/longitude basis. Tangency is `abs(dot(p,v)) <= 1e-10` for unit position `p`; all valid operator outputs are finite.

Vectors compared across cells are shortest-geodesic parallel transported to the reference tangent plane first. Coincident points use tangent projection. Nearly antipodal or degenerate cases use deterministic projection/fallback and must not generate NaN. The canonical undirected edge is lower stable `CellId` to higher `CellId`; edge tangent points source-to-target.

Boundary normal relative motion is evaluated only on cross-label edges: use the canonical source tangent plane as reference, parallel-transport target velocity to source, take relative velocity `transport(v_target) - v_source`, and dot with the canonical source-to-target edge tangent at source. Positive means approach/convergence; negative means separation/divergence. Negating both velocities negates the result. Signed tangential interaction is the same relative velocity dotted with the source-oriented edge-perpendicular `normalize(p_source cross edge_tangent)`; it reverses under velocity negation. Unsigned shear is its absolute magnitude and is invariant under velocity negation.

Edge-to-cell values use geometry-weighted aggregation over canonical edge order and stable cell order, with deterministic sequential reductions. `region_vector(labels, magnitude)` derives an ambient 3D direction from `(world seed, operator node id, category id)`, projects it tangent at each cell, normalizes and scales it. If projection norm is at most `1e-12`, use a deterministic secondary axis sequence (X, then Y, then Z, each projected), then zero only if all are degenerate.

Gradient and divergence share immutable mesh geometry/calibration semantics. Reductions use stable traversal/chunk/tree ordering, never scheduler-dependent Rayon fold ordering. Advection retains Wave 2 behavior: each cell blends its value with the selected upstream neighbor by velocity magnitude clamped to `[0,1]`; it is not guaranteed conservative unless a future operator explicitly declares conservation.

## Determinism and equilibrium

Canonical-build determinism is promised only where the existing canonical path promises exact snapshots: identical recipe, effective parameters, mesh, seed, pinned toolchain/build and supported platform produce byte-identical output. Cross-platform floating-point bit identity is not promised; comparisons use numerical/causal tolerances.

The algebra must express bounded source/transport/sink/relaxation systems that converge to stable or statistically stable equilibrium where the configured model admits it. Accelerated relaxation/fixed-point iteration is permitted and is not physical time. Iteration must have an explicit hard cap and convergence test of at least RMS or max field delta, reporting `converged`, `iterations`, and `final_delta`. Non-convergence is a bounded reported result.

## Generic operator contracts

Descriptors and compiler schemas are stable and deterministic. Wave 3 declares `region_vector(labels: category_field, magnitude: scalar) -> vector_field`; `boundary_normal_component(labels: category_field, velocity: vector_field) -> scalar_field`; `boundary_tangential_component` with the same signature; `divergence(vector_field) -> scalar_field`; `vector_add` and `vector_subtract` over two vector fields; `scalar_vector_multiply(scalar_field, vector_field) -> vector_field`; `vector_dot` over two vector fields; `vector_magnitude(vector_field) -> scalar_field`; and `gradient(scalar_field) -> vector_field`. Stubs may report unsupported until an owning implementation lands. Existing Wave 1/2 behavior remains unchanged.

## Shared metadata and provenance

Serializable metadata comprises semantic role; display name, description, units, recommended palette, zero meaning, render role, and vector render mode; channels `geometry_displacement`, `base_material`, `material_override`, `line_overlay`, `scalar_overlay`, `vector_overlay`; and per-node provenance (node id, operator id/kind, input node/reference IDs, effective arguments/parameters and output binding). Run identity records mesh identifier, seed, program hash, effective parameters, canonical build identity/version and intervention declaration. An intervention declares changed parameter and/or forcing substitutions only. Baseline and intervention use the same structural program wherever possible. Metadata cannot affect generation or validation except provenance/identity checks.

## Target model recipes (declarative only)

A. Temperate Terrestrial: `plate_id -> plate_velocity -> boundary_convergence/shear -> tectonic_forcing -> elevation -> insolation -> temperature/temp gradient -> wind -> moisture transport -> precipitation -> runoff/flow/discharge -> vegetation -> biome`.

B. Tectonically Active: `plate_id`, `plate_velocity`, `boundary_convergence`, `boundary_shear`, `tectonic_forcing`, `elevation`, `slope`. Zero-motion and velocity-reversal interventions are hard causal invariants.

C. Composite External Forcing: `forcing_a`, `forcing_b`, `forcing_total`, `temperature`, `temp_gradient`, `wind`, `moisture`, `precipitation`, `vegetation`, `biome`; test A-only, B-only, A+B and A+2B. `forcing_total` must be exact ordinary algebra.

D. Localized Disturbance: generic forcing affects elevation, temperature and vegetation damage with radial geodesic locality. Zero amplitude reproduces control. No event-specific Rust.

## Paired validation and corpus

Causal claims require paired baseline/intervention execution on the same seed/config except declared intervention changes. Comparison contracts include delta, mean absolute delta, RMS delta, correlation of cause/effect deltas, sign agreement, masked delta mean, inside/outside response ratio and exact unchanged assertions outside the intervention's downstream DAG closure when deterministic eager execution permits. Canonical seeds are `1, 7, 17, 29, 43, 71`; level 5 has 10,242 cells. Acceptance includes at least one lower resolution and cross-resolution numerical/causal tolerances.

## Viewer contract

The viewer remains domain-agnostic and offers: consolidated default rendering from generic metadata/channels; causal overlays for generic vectors/scalars/networks disclosing direction-only versus magnitude encoding; and raw inspection with legends for every field/network type. Cell inspection can show values and immediate graph provenance/upstream node IDs. No terrain/climate naming knowledge belongs in viewer logic.

## Architecture and bounded work

Do not add domain-specific Rust systems (including star, orbit, tectonics, atmosphere, climate, biome or river systems), physical/angular-radius-as-hop interpretations, implicit vector bases, scheduler-order-dependent reductions, unbounded iterations, or claims of conservative advection. Keep work proportional to mesh/edges/iteration cap; validate input lengths and finiteness before kernels. Preserve dense eager execution and do not add incrementality or dependencies.

## Completion criteria

Each operator has a unique stable descriptor and compiler-visible typed schema; shared metadata and numerical contracts roundtrip; stubs compile and report unsupported; old fixtures/snapshots remain stable; paired validation exercises the frozen seed corpus and at least two resolutions; deterministic canonical snapshots and cross-platform tolerance checks are explicit; no domain-specific Rust semantics are introduced.
