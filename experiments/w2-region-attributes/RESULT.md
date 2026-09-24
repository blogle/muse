# MUSE-27: generic region attributes (experimental)

## Status and recommendation

Prototype only; do not merge directly. The generic operators are useful for
coherent category-conditioned fields, and this terrain reaches the requested
land fraction with a dominant connected land body and ocean body. Recommend
keeping the operators experimental and iterating on boundary-motion semantics
and map diagnostics before promoting them.

No Plate, Continent, or Tectonics domain types were added.

## APIs and exact math

- `region_scalar(labels, seed, node_id, scale, offset) -> Result<Vec<f64>>`
  hashes `seed.to_le_bytes()`, the byte length and UTF-8 bytes of `node_id`,
  and `label.to_le_bytes()` with BLAKE3. The first eight digest bytes interpreted
  as little-endian `u64`, divided by `u64::MAX`, give a stable `u` in `[0,1]`.
  Every cell with that label receives exactly `offset + scale * u`; therefore
  the value is shared by all cells of a region. Non-finite scale/offset reject.
- `region_tangent_vector(mesh, labels, seed, node_id, magnitude) -> Result<Vec<DVec3>>`
  hashes the seed, length-prefixed node id bytes, and label bytes. Three consecutive digest
  `i32` little-endian components, each divided by `i32::MAX`, are normalized
  into one deterministic ambient direction per label. At each cell, project
  that direction into its tangent plane and normalize to `magnitude`. If the
  projection is degenerate, project X (or Y when `|normal.x| >= 0.8`) instead.
  Mesh/label lengths and finite nonnegative magnitude are checked.
- `boundary_relative_normal(mesh, labels, vectors) -> Result<Vec<f64>>` visits
  each cell's unlike-label neighbors. For directed edge `i -> j`, let `n` be
  the normalized projection of `position[j]` into the tangent plane at `i`.
  Average `(vectors[i] - vectors[j]) dot n` across unlike-label neighbors. The
  sign is positive for convergence/closing motion and negative for separation;
  cells with no qualifying boundary neighbors receive zero. Invalid lengths,
  mesh indices, or non-finite vectors reject.

All per-category seeds depend only on the world seed, node ID, and category
label, never iteration order or cell position. Parallel evaluation is per cell.

## Terrain

`terrain.yaml` uses 9 `voronoi_labels` categories, the region scalar as broad
base elevation, deterministic per-label tangent motion at magnitude 0.7,
signed boundary interaction multiplied by 0.03, generic `noise` relief at 0.003,
and an elevation threshold of 0.5. Land is the thresholded elevation field.
The categorical field is also persisted as `region_labels` so operators can be
reused by other consumers.

## Runtime and measured results

Command: `cargo run -p muse-cli --quiet --bin muse -- wave1-generate --spec
experiments/w2-region-attributes/terrain.yaml --output ... --level 5 --seed 27`
inside `nix develop`. One cold wrapper invocation took **2.179 s wall-clock**;
this includes Nix shell invocation and Cargo startup, not an isolated kernel
benchmark.

Two independent level-5 generations were byte-identical (10,242 cells;
SHA-256 `34df2ada1372cfeab08e21fa0b1e285a3c63d283bf3530a779267d32ceecf187`).
Measured directly from the output mesh and fields:

| Metric | Result |
|---|---:|
| Land fraction | 0.31195 |
| Land components | 2 |
| Largest land component / total land | 0.99969 |
| Ocean components | 1 |
| Largest ocean component / total ocean | 1.00000 |
| Land/ocean cross-edge perimeter proxy | 527 undirected mesh edges |
| Boundary interaction min / max | -1.31856 / 1.36013 |
| Positive / negative interaction cells | 594 / 541 |

The project validator does not currently provide a perimeter metric, so the
mesh cross-edge count is used as its proxy. The signed interaction includes
both convergence and divergence as required.

## Visual inspection

Inspected an equirectangular land/ocean rendering and a coarse ASCII raster of
the level-5 output. The view shows a coherent broad landmass, a smaller
disconnected land patch, and one continuous ocean body. The coarse projection
has some edge/polar raster aliasing. Component counts and fractions show low
fragmentation for this configuration. Relative improvement over the initial
terrain cannot be quantified because no corresponding baseline generation or
metrics were recorded in this experiment.

## Reuse coverage

`region_scalar_reuses_unrelated_category_labels` and
`region_motion_and_boundaries_reuse_unrelated_categories` exercise scalar,
vector, and boundary use on unrelated category sets. Labels are opaque `u32`
data, with no domain-specific assumptions.
