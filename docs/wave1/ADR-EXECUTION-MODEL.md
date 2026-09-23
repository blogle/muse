# ADR: Execution model for MUSE PoC

Status: Accepted
Date: 2026-09-23

## Context

W1-F compared dense/Rayon execution with Timely/Differential on a synthetic
10-step numeric field pipeline: pointwise `tanh`, followed by ten ring-neighbor
passes computing `(2 * center + left + right) / 4`, then a mean and count of
values strictly above `0.5`. It tested initial computation and localized input
changes affecting 1% and then 25% of the cells, at 10,000 and 100,000 cells.
The adoption question was whether Differential was materially better for
localized changes while remaining correct, natural for numeric fields, and
compatible in complexity with MUSE's whole-field Program/operator boundary.

## Options considered

A. **Whole-field dense fields + deterministic topological execution + Rayon.**
   Recompute each required whole field in dependency order.
B. **Timely/Differential.** Use persistent dataflow state and differential
   updates as the PoC execution model.
C. **Whole-field now, Differential candidate for a later graph/query
   subsystem.** Keep the PoC boundary simple and revisit only for a workload
   with a deliberate stateful boundary.

## Evidence

W1-F reported the following medians (one warm-up, five measured repetitions):

| Cells | Scenario | Dense/Rayon | Differential |
|---:|---|---:|---:|
| 10k | Initial | 2.340 ms | 190.083 ms |
| 10k | 1% update | 1.821 ms | 3.952 ms |
| 10k | 25% update | 1.461 ms | 76.952 ms |
| 100k | Initial | 4.302 ms | 2101.023 ms |
| 100k | 1% update | 3.909 ms | 34.697 ms |
| 100k | 25% update | 3.846 ms | 822.797 ms |

Correctness matched within mean absolute tolerance `1e-10`; threshold counts
matched exactly. Differential updates used the same persistent graph and input
session. Its initial timing excludes graph construction, but includes input
insertion, epoch advancement/flush, progress to the probe, and caller-observed
summary extraction. Update timing includes retractions/insertions, epoch
advancement/flush, progress, and summary extraction.

The Differential adapter/glue was 147 LOC. Its explicit lifecycle concepts
were `InputSession`, worker, epochs, `Collection`, keyed `join_map`, keyed
`reduce`, signed update/retraction diffs, `ProbeHandle`/frontiers, progress
driving with `step`, and persistent output capture. The fixed numeric
recurrence also required relational joins/reductions, and output aggregation
was maintained by caller-side state.

All five W1-F adoption criteria failed:

1. Boundary complexity: 147 adapter/glue LOC and lifecycle state outside the
   frozen operator boundary.
2. Material benefit for 1% updates: Differential took 3.952 ms vs 1.821 ms at
   10k, and 34.697 ms vs 3.909 ms at 100k.
3. Natural numeric iteration: ten direct ring passes became keyed joins and
   grouped reductions, with an ordered numeric adapter.
4. Initial evaluation: Differential took 190.083 ms vs 2.340 ms at 10k, and
   2101.023 ms vs 4.302 ms at 100k, despite excluding graph construction.
5. Acceptable PoC complexity: the measured glue includes numeric adaptation,
   frontier advancement, signed multiset output capture, and probe driving.

## Decision

Choose **Option A** for the PoC: whole-field dense fields, deterministic
topological execution, and Rayon. Do not adopt Timely/Differential for the PoC
and do not create a custom incremental or dirty-region framework.

Option B is rejected because Differential did not improve the measured 1%
updates, was substantially slower for initial computation, and added lifecycle
and adapter complexity that does not fit the frozen Program/operator boundary.
Option C remains a revisit path, not the current architecture.

## Consequences

- Wave 2 recipes target the existing Program/dense Operator boundary.
- Localized edits recompute the required whole fields for the PoC.
- Benchmark before optimizing.
- No custom incrementality is introduced.
- CEL and native-kernel performance remain independently observable.
- The W1-F experiment stays isolated and is not merged into normal workspace
  crates.

## Revisit criteria

Revisit Differential if a representative workload introduces persistent
graph/query/aggregate state with sparse updates and demonstrates an end-to-end
material speedup, including adapter, conversion, and observation costs, without
forcing an unsuitable change to the frozen operator boundary. Such a workload
must fit a deliberate stateful boundary and be measured against the
whole-field dense/Rayon reference at representative sizes and update
distributions. A synthetic localized update alone is insufficient evidence.
