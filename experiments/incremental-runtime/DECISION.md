# Question

Is Timely/Differential a materially better execution model than dense `Vec<f64>` + Rayon for localized changes through this 10-step numeric field pipeline?

# Implementations

- **Dense/Rayon:** pointwise `tanh`, then 10 full ring passes, each `(2 * center + left + right) / 4`, then mean and count strictly above `0.5`.
- **Timely/Differential:** one persistent worker/dataflow per size. The input is keyed by cell. A static edge relation is keyed by source and has self/left/right contributions of weights 2/1/1. The pipeline is explicitly unrolled as 10 `join_map` + `reduce` stages. Each reduce emits weighted contribution sum divided by four. Input changes are negative old records plus positive new records, applied to the same source session and graph.
- At 1%, indices `[0, N/100)` change. The following update changes `[N/100, N/4)`, resulting in exactly the first 25% changed relative to the original field.
- Differential output is observed as signed updates and maintained as a multiset keyed by `(cell, Number)`. Mean and threshold count are extracted from the current positive multiplicities outside the dataflow; this extraction is included in update timing.

# Dependency versions

- `differential-dataflow = 0.25.1`
- `timely = 0.31.0`
- Resolved `columnar = 0.13.2`
- `rayon = 1.12.0`
- `serde = 1.0.229` with derive, for Timely exchange serialization of the local adapter

The pair compiles without transitive overrides, `[patch]`, forks, or vendoring. The isolated package has its own lockfile and `[workspace]` and does not affect root dependencies.

# Correctness

Tolerance: absolute difference in mean `<= 1e-10`; threshold counts must match exactly. The synthetic generator and all transforms produce finite, non-NaN values. `Number(f64)` implements Eq/Ord using `total_cmp` semantics, and its comparison equality is consistent with that total ordering.

Automated tests compare the persistent Differential output to dense output for initial, 1%, and 25% states at both 10,000 and 100,000 cells. All pass. Representative observed mean errors from the release harness were zero at 10k and at most approximately `2.3e-16` at 100k. Threshold counts match exactly in every case.

# Measurements

Release executable, one warm-up, then five measured repetitions; medians shown. Dense initial timings measure full field computation and summary. Differential graph construction is excluded from initial timing; initial timing includes input insertion, epoch advancement/flush, progress to probe, and caller-observed summary extraction. Differential update timings include retractions/insertions, epoch advancement/flush, progress, and summary extraction. Dense update timings recompute the full field and summary after each scenario.

| Cells | Scenario | Changed cells | Dense median | Differential median |
|---:|---|---:|---:|---:|
| 10,000 | Initial | 0 | 2.340 ms | 190.083 ms |
| 10,000 | 1% update | 100 | 1.821 ms | 3.952 ms |
| 10,000 | 25% update | 2,500 | 1.461 ms | 76.952 ms |
| 100,000 | Initial | 0 | 4.302 ms | 2,101.023 ms |
| 100,000 | 1% update | 1,000 | 3.909 ms | 34.697 ms |
| 100,000 | 25% update | 25,000 | 3.846 ms | 822.797 ms |

Peak RSS was sampled from `/proc/<pid>/status` every 50 ms during the complete repeated release harness: **392,308 KiB**. This is the process peak over both sizes and repetitions, not a per-variant isolated maximum.

Production source LOC: **264** (nonblank, non-comment lines before tests). Differential adapter/glue LOC: **147**, counting the local ordered numeric adapter and conversion/serialization plus `current_summary`, the persistent Differential graph, epoch/probe management, and current-output capture/extraction. This count excludes imports, tests, and comments; tests and comments are also excluded from total source LOC.

The caller must understand **10 framework concepts/types**: (1) `InputSession`, (2) Timely `Worker`, (3) timestamp epochs, (4) Differential `Collection`, (5) keyed `join_map`, (6) keyed `reduce`, (7) signed update/retraction diffs, (8) `ProbeHandle`/frontiers, (9) progress driving with `step`, and (10) persistent output capture via `Rc<RefCell<BTreeMap<...>>>`. The custom `Number` adapter and its conversion/serialization logic are additional caller-owned complexity.

Peak memory measurement: practical process-level RSS sampling; per-framework attribution was not attempted.

# Complexity observations

- The fixed recurrence is translated into 10 keyed relational joins/reductions and a static three-edge relation per source, rather than direct indexed neighbor reads. The expression is valid but less direct than the dense recurrence.
- `Number(f64)` is necessary because Differential's `Data` imposes `Ord`; the `total_cmp` wrapper and serde derives are isolated experiment glue.
- Persistent output observation needs signed multiset maintenance by `(cell, Number)` because updates arrive as retractions/insertions, then a caller-side aggregation pass.
- The first persistent attempt stalled because the edge collection's input frontier was left at epoch 1. The output probe remained behind later scalar epochs. Advancing and flushing the static edge input to `u64::MAX` releases that frontier beyond all scalar epochs (1–3). A bounded million-step diagnostic now panics with probe/input frontier evidence if progress stalls. A 100-cell initial/update smoke test then completed and matched dense; 10k and 100k subsequently completed. This is frontier/progress lifecycle complexity a caller must own.
- Output aggregation currently occurs outside Differential. This is included in update timing and means the experiment's caller adapter, not Differential alone, supplies the returned aggregate.
- **Frozen Operator interface fit:** a whole-field calculation can be hidden behind the unchanged field-in/field-out interface, but this measured incremental behavior cannot be naturally exposed through it unchanged. Persistent worker/session lifetime and delta-update inputs are not represented by the frozen boundary; exposing those benefits requires separate stateful lifecycle/update semantics.

# Adoption criteria

1. **Differential does not materially increase operator/compiler boundary complexity — FAIL.** Evidence: 147 adapter/glue LOC; the adapter must manage a persistent worker/session, explicit epochs/probe progress, ordered scalar wrapper, and caller-side output state. Those lifecycle concepts are outside the frozen operator boundary.
2. **1% localized updates show a clearly material measured speedup — FAIL.** At 10k, Differential takes 3.952 ms vs dense 1.821 ms; at 100k, 34.697 ms vs dense 3.909 ms. Differential is slower by approximately 2.2x and 8.9x respectively.
3. **Iterative numeric field computation remains natural rather than relational bookkeeping — FAIL.** Each of 10 direct ring steps becomes a keyed join and grouped reduction, and numeric output needs a custom orderable adapter.
4. **Full initial evaluation is not catastrophically slower — FAIL.** Differential takes 190.083 ms vs dense 2.340 ms at 10k (~81x), and 2,101.023 ms vs 4.302 ms at 100k (~488x), with graph construction excluded from the Differential figure.
5. **Implementation/glue complexity is acceptable for a 1–2 day PoC — FAIL.** The 147 glue LOC excludes tests and includes ordered-number semantics, static-edge frontier advancement, signed multiset capture, and probe driving; the fixed-point equation itself is simpler in dense code.

# Recommendation

REJECT-FOR-POC

# Revisit evidence

Revisit if a representative workload introduces persistent graph/query/aggregate state whose updates are sparse and for which Differential demonstrates an end-to-end material speedup, including caller observation and conversion costs, without requiring the frozen operator boundary to change. Re-measure against the same whole-field dense/Rayon reference at production-relevant sizes and update distributions.
