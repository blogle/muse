# MUSE-26 — Scale-aware spherical fields (experiment)

## Answer

**Yes, with an important resolution caveat.** Radius-calibrated graph diffusion
gave broadly comparable feature scale at levels 2 and 5 when measured at a
matching angular separation. The 162-cell level-2 mesh is too coarse for this
recipe's land fraction and plate-boundary correlation targets; level 5 satisfies
the numeric terrain targets and yields a dominant connected landmass and ocean.

## API and math

The prototype is in `muse-ops` and is intentionally generic:

```rust
smooth_radius(mesh: &Mesh, values: &[f64], radius: f64) -> Result<Vec<f64>>
correlated_noise(mesh: &Mesh, seed: u64, node_key: &str,
                 radius: f64, amplitude: f64) -> Result<Vec<f64>>
```

Both are registered operators with scalar-field output. The YAML forms are
`smooth_radius { field, radius }` and `correlated_noise { radius, amplitude }`.
Radius is finite, positive angular distance in radians on the unit sphere.
`smooth_radius` extracts each undirected edge once, measures
`acos(clamp(normalize(p_i) dot normalize(p_j), -1, 1))`, and uses the median
`h`. With fixed neighbor-mixing rate `r0 = 0.2`, the iteration count is

```text
n = max(1, ceil((radius / h)^2 / r0))
u(k+1)[i] = (1-r0) u(k)[i] + r0 mean(u(k)[neighbors(i)])
```

This is a graph heat-kernel approximation: diffusion time grows with squared
angular radius, while `h` compensates for mesh spacing. It has no terrain or
domain semantics. Invalid/degenerate edge geometry, non-finite field values,
non-positive radius, and step-count overflow return errors.

`correlated_noise` calls existing keyed noise with a derived base key
`{node_key}:correlated-base`, applies `smooth_radius`, subtracts the population
mean, divides by population standard deviation, and multiplies by `amplitude`.
Finite degenerate output (variance <= 1e-24), or a non-finite normalization
result, is defined as all zeros; this avoids unstable division and preserves
deterministic execution. Non-finite amplitude is an error. The zero-variance
path is unit-tested directly. A non-degenerate fixture tests mean zero,
requested standard deviation, and stable/different node keys.

## Terrain recipe and level metrics

`terrain.yaml` declares 12 spherical Voronoi plates, three independent
correlated-noise scales (0.62, 0.30, 0.12 rad with amplitudes 1.0, 0.42,
0.18), and a secondary plate-boundary term at gain 0.8; thresholding makes
land and gradient magnitude produces slope. No domain-specific Rust was added.
`analyze.py` calculates connected components over mesh neighbors, the exact
number of unique land/ocean crossing edges as perimeter, Pearson correlation
of slope with plate-boundary strength, one-hop elevation Moran-style
autocorrelation, and correlation at the graph hop count nearest the broad
requested radius. Screenshot files are vertex-colored equirectangular SVGs.

Seed 1, identical parameters:

| Metric | Level 2 | Level 5 |
|---|---:|---:|
| Cells | 162 | 10,242 |
| Land fraction | 0.593 | 0.420 |
| Connected components (land + ocean) | 2 | 6 |
| Land components | 1 | 5 |
| Largest land component / land | 1.000 | 0.990 |
| Largest ocean component / ocean | 1.000 | 1.000 |
| Land/ocean perimeter edges | 44 | 1,027 |
| Slope / plate-boundary correlation | 0.021 | 0.346 |
| Median geodesic edge `h` (radians) | 0.3142 | 0.03741 |
| Broad-radius graph hops (`0.62 / h`) | 2 | 17 |
| Elevation correlation at that graph separation | 0.754 | 0.578 |
| One-hop elevation autocorrelation | 0.889 | 0.993 |

The near-one level-5 one-hop correlation is expected simply because its
neighbors are much closer together physically; it must not be compared directly
to level 2. At approximately the same 0.62-radian graph separation, correlation
is of the same broad order (0.75 vs 0.58), and both screenshots show a few broad
masses rather than cell-scale speckle. The 30% relative difference is not a
precision guarantee; graph topology, keyed per-cell samples, and only two hops
of support at level 2 remain visible. Level 2 fails the requested land-fraction
and correlation criteria, while level 5 clears both and is substantially
unfragmented by the component-size measures.

![Level 2 terrain](level2.svg)

![Level 5 terrain](level5.svg)

## Radius sensitivity (level 5, seed 1)

Changing only broad radius with remaining recipe values fixed:

| Broad radius | Radius hops | Lag correlation | Land fraction | Components | Largest land fraction | Boundary/slope corr. | Perimeter edges |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 0.35 | 9 | 0.685 | 0.438 | 16 | 0.961 | 0.220 | 1,361 |
| 0.62 | 17 | 0.578 | 0.420 | 6 | 0.990 | 0.346 | 1,027 |
| 0.90 | 24 | 0.499 | 0.420 | 5 | 0.974 | 0.382 | 775 |

Larger broad radii reduce disconnected land/ocean fragments and perimeter,
while the broad-lag correlation declines because the comparison distance itself
increases. Boundary gain sensitivity was also material: at 0.12, boundary/slope
correlation was -0.011 on level 5; raising it to 0.8 produced 0.346 and kept
land fraction 0.420. The boundary is a secondary additive term, not the base
land signal, but the gain should be treated as a tuning knob rather than a
universal constant.

## Performance and validation

Divan optimized benchmark, level-5 10,242-cell icosphere, 10 samples:

| Operator | Fastest | Median | Mean | Slowest |
|---|---:|---:|---:|---:|
| `smooth_radius(radius=0.2)` | 69.93 ms | 73.24 ms | 75.09 ms | 95.51 ms |
| `correlated_noise(radius=0.2)` | 59.21 ms | 78.00 ms | 77.63 ms | 89.92 ms |

These are machine-specific and include mesh-edge calibration on every smoothing
call. The sample timing is noisy; `correlated_noise` additionally creates keyed
base noise and normalizes. On level 5 the formula requests 1,374 iterations for
radius 0.62 versus 358 for radius 0.2 (about 3.8x); broad smoothing can cost
hundreds of ms at level 5. An appropriate follow-up, outside this prototype,
would be caching mesh calibration and measuring the kernel with representative
workloads.

Checks run: `cargo check --workspace`; `cargo nextest run --workspace` (39
passed); `cargo clippy --workspace --all-targets --all-features -- -D warnings`;
`cargo fmt --all -- --check`; targeted level-5 Divan benchmarks; full real CLI
generation at levels 2 and 5. Duplicate level-5 seed-1 output SHA-256 matched
exactly (`faba6715d3db35794dde95c939062345c65f4533355149d1a0578faf16cea4d4`).

## Recommendation and failure modes

The production-worthy subset is the generic `smooth_radius` contract plus
radius-normalized `correlated_noise`, with deterministic zero output for
near-degenerate variance. Keep the stable rate/formula and document that this
is a graph approximation, not an exact spherical convolution. The experiment
demonstrates declarative multiscale composition through three correlated-noise
nodes and pointwise addition; no octave operator is needed.

Known limitations: behavior depends on connected, reciprocal-like neighborhood
graphs (the current implementation counts listed undirected edges once and uses
the existing neighbor mean); irregular meshes make median-edge calibration only
representative; very large radii can be expensive or rejected on step overflow;
very small radii still execute one discrete step and therefore cannot resolve
sub-edge scales; low-resolution threshold topology changes sharply; keyed noise
uses cell identity so cross-resolution samples are not pointwise nested; and
centered normalization makes amplitude statistical rather than an absolute
unfiltered-noise scale.
