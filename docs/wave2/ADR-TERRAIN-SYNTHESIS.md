# ADR: Synthesis of spatial and region terrain primitives

Status: Proposed for production implementation
Date: 2026-09-24
Scope: Architecture only; no production operators are implemented here.

## 1. Executive decision

Adopt both continuous-scale and region-associated terrain models. They represent
complementary causes: region attributes establish coherent macro geography,
while correlated fields provide nonuniform broad and local relief. Adopt
`region_scalar`, `boundary_relative_normal`, and `correlated_noise` with revised
contracts. Adopt `smooth_radius`'s scale-based user contract, but reimplement it
with a bounded-cost kernel rather than the experimental diffusion-iteration
mapping. Reject `region_tangent_vector` from the initial production algebra;
there is no demonstrated canonical-terrain need that justifies introducing
vector-frame/seam semantics yet. Promote one deterministic discrete-component
algebra shared by generation and validation. It must operate on the mesh's
existing adjacency and spherical geometry, rather than maintaining separate
component traversals.

Evidence available for this decision: the experiment reports supplied for
spatial-fields level 5 / seed 1 show 0.420 land fraction, five land components
with largest land 0.990, ocean largest 1.000, boundary/slope correlation 0.346,
and larger radius reducing fragmentation/perimeter. The region-attributes
report shows 0.312 land fraction, two land components with largest about
0.9997 of land, one ocean component, and signed boundary interaction with both
positive and negative values. Both report deterministic duplicate generation.
These are promising single-seed observations, not calibrated acceptance
thresholds. The spatial-fields configuration's main weakness is that its
radius can induce hundreds or thousands of diffusion iterations at level 5
and recomputes mesh calibration.

## 2. Operator decisions

| Experimental operator/family | Decision | Rationale / production boundary |
|---|---|---|
| `smooth_radius` | **ACQUIRE WITH REVISED CONTRACT** | Scale in mesh/physical units is interpretable and radius reduced fragmentation. Preserve radius semantics, not the repeated-diffusion implementation. Specify scale normalization, bounded error/tolerance, and cacheable mesh calibration. |
| `correlated_noise` | **ACQUIRE WITH REVISED CONTRACT** | Adds broad/local continuous variation complementary to region labels. Fix keyed deterministic sampling, mesh/seed identity, scale and amplitude semantics; do not claim statistical isotropy until measured. |
| `region_scalar` | **ACQUIRE WITH REVISED CONTRACT** | Generic useful mapping from category labels to scalar attributes. Define total mapping/default behavior, stable labels, and reject missing/nonfinite values. |
| `region_tangent_vector` | **REJECT** (initial production scope) | Region-local tangent vectors require a declared frame, transport/interpolation rules and discontinuity behavior. No supplied terrain evidence requires it; reconsider for wind/flow recipes with a concrete use case. |
| `boundary_relative_normal` | **ACQUIRE WITH REVISED CONTRACT** | Signed positive/negative interaction is useful and expressive. Define orientation from ordered region values and a canonical tangent-plane normal; explicitly define equal-label, invalid-neighbor, and multi-boundary aggregation behavior. |
| `connected_components(mask) -> CategoryField` | **ACQUIRE WITH REVISED CONTRACT** | Shared primitive is justified, but IDs must be deterministic and the same labeling implementation must support generation and validation. Empty-mask semantics and adjacency are explicit below. |
| `component_area(labels/mask)` | **ACQUIRE WITH REVISED CONTRACT** | Derive from spherical cell areas, not cell counts. Prefer compact per-component summary plus broadcast rather than separate implementations. |
| `component_perimeter(labels/mask)` | **ACQUIRE WITH REVISED CONTRACT** | Derive from spherical edge geometry, with each unlike-label boundary edge counted once per requested category boundary. |
| `component_measure_broadcast(labels, measure) -> ScalarField` | **ACQUIRE WITH REVISED CONTRACT** | Useful composable mechanism (e.g. area-driven regional attributes), with exact component count/label alignment checks. Avoid exposing mutable or unstable component identity as persistent state. |

### Proposed typed signatures and semantics

These are DSL-level contracts, not frozen Rust signatures. `CategoryField`,
`BoolField`, `ScalarField`, and `VectorField` are the existing typed field
families. All field outputs have exactly one value per mesh cell.

```text
connected_components(mask: BoolField, connectivity: Connectivity = mesh_adjacency)
    -> CategoryField
component_stats(labels: CategoryField, measure: ComponentMeasure)
    -> ComponentValues                 # scalar-per-component, internal typed value
component_measure_broadcast(labels: CategoryField, measure: ComponentMeasure)
    -> ScalarField

region_scalar(labels: CategoryField, values: Map<Category, FiniteScalar>,
              default: FiniteScalar? = none) -> ScalarField
boundary_relative_normal(labels: CategoryField,
                         values: Map<Category, FiniteScalar>,
                         scale: FiniteScalar = 1) -> ScalarField
correlated_noise(seed: Key, scale: Length, amplitude: FiniteScalar = 1,
                 mean: FiniteScalar = 0) -> ScalarField
smooth_radius(field: ScalarField, radius: Length,
              boundary: BoundaryMode = mesh_closed,
              tolerance: Bounded? = calibrated_default) -> ScalarField
```

`connected_components` labels false cells with a reserved background category
and true connected components with dense IDs ordered by the minimum cell index
in each component. Connectivity is the mesh neighbor graph, symmetric and
deduplicated; malformed adjacency is a mesh error, not silently repaired.
Component summaries use spherical cell areas and spherical edge lengths.
Broadcast maps the summary back to members and fails on label/summary mismatch.
Where the current DSL cannot represent an internal `ComponentValues` type,
implement the summary+broadcast as one operator initially; do not add a
user-visible general-purpose value type solely for this feature.

`region_scalar` maps every represented category exactly once. Missing entries
fail compilation unless an explicit finite default is supplied; unused extra
entries may be accepted. Category IDs must be stable within the producing
program. `boundary_relative_normal` compares adjacent category values, uses the
signed difference (higher minus lower in canonical edge orientation), and
aggregates incident-edge contributions with spherical edge weighting and
normalization by total incident boundary length. It returns zero off a boundary
and for equal values. The production name/definition must not imply a geometric
normal vector: this operator is a signed scalar displacement relative to a
boundary. If the experiment actually emits a vector, rename it and specify
vector-frame semantics before implementation.

`correlated_noise` is deterministic for the same mesh, seed, scale, and
configuration, independent of thread scheduling. Its scale is a correlation
length, not an iteration count; amplitude is a linear multiplier and mean is
added after unit-variance normalization. Noise is keyed by stable mesh/cell
identity and seed, never by execution order. `smooth_radius` preserves the
input's area-weighted global mean (within numerical tolerance), has no-flux
closed-mesh boundary behavior, and radius zero is identity. Radius denotes
physical correlation/smoothing length under mesh-calibrated discretization.
Calibration depends only on immutable mesh geometry and is reusable across
fields and repeated graph evaluations.

## 3. Shared discrete-component algebra

Promote the component labeling and spherical summary mechanism as a common
mesh primitive. Generation can use labels for region-associated fields and
validation can use the same labels for counts, component distributions, areas,
perimeters, and largest-component metrics. The shared contract is the algebra
above; user-visible operators need only be exposed where recipes consume them.
The primitive belongs in runtime algebra because generation needs connected
labels and component-conditioned values. Validation-only statistics remain
aggregations over that shared labeling/geometry core, not a second flood-fill.

For Boolean masks, validation treats true cells as the selected set and false
cells as its complement, invoking the same labeling procedure on each. For
arbitrary category fields, touching cells of the same category form components.
Spherical cell areas and edge lengths are the sole measures for geometry
metrics. Determinism requirements: sorted start-cell traversal, canonical
neighbor order, stable dense IDs, finite results, and invariant output under
thread scheduling. Whether IDs use `u32` or another existing category width is
an implementation compatibility question; IDs are not semantic across changed
meshes.

## 4. Canonical terrain recipe

Use causal, named intermediate fields rather than one opaque noise expression:

```text
regions       = coherent_region_labels(seed, region_scale)
base          = region_scalar(regions, buoyancy_by_region)
boundary_lift = boundary_relative_normal(regions, buoyancy_by_region, lift_scale)
broad         = correlated_noise(seed.child("broad"), broad_scale, broad_amplitude)
relief        = correlated_noise(seed.child("relief"), relief_scale, relief_amplitude)
macro         = base + boundary_lift + smooth_radius(broad, macro_radius)
elevation     = macro + smooth_radius(relief, relief_radius)
land          = threshold(elevation, sea_level)
slope         = vector_magnitude(gradient(elevation))
```

`coherent_region_labels` is a recipe-level use of the existing category
generator, not a new operator commitment. Choose distinct deterministic child
keys so changing one scale does not perturb the other field. Use bounded
amplitudes and an explicit sea level. This keeps broad regional buoyancy and
local relief independently tunable. The signed boundary field may be omitted
if calibration shows it creates narrow ridges or excessive perimeter; it must
earn inclusion by discriminator improvement over the base+correlated-field
ablation.

Continuous and region models are complementary, not substitutes. Region-only
has demonstrated strong component coherence but risks categorical blocks;
continuous-only has demonstrated less reliable connectivity (five land
components despite a dominant 0.990 component in the reported run). The
production recipe combines their strengths and exposes ablations for
calibration.

## 5. Validator / discriminator recipe

Use existing scalar and spherical Boolean metrics in a discriminator. Candidate
metric names are illustrative; use the production validator's supported names.
Hard constraints should be expressed as calibrated ranges, not visual review:

```yaml
metrics:
  land_fraction:       { op: area_fraction, field: land }
  land_largest:        { op: largest_component_area_fraction, field: land }
  ocean_largest:       { op: largest_component_area_fraction, field: ocean }
  land_components:     { op: component_count, field: land }
  land_perimeter_area: { op: perimeter_area_ratio, field: land }
  elevation_variance:  { op: variance, field: elevation }
  boundary_slope:      { op: correlation, a: boundary_strength, b: slope }
constraints:
  plausible_land_area: calibrated range for land_fraction
  dominant_land:       lower bound or calibrated component distribution for land_largest
  connected_ocean:     lower bound for ocean_largest
  bounded_fragmentation: calibrated bound for land_perimeter_area
  nonflat_elevation:   calibrated range for elevation_variance
  boundary_response:   calibrated association interval for boundary_slope
```

Hard requirements are: (1) land area fraction lies in the accepted empirical
interval; (2) largest land area fraction or full component-area distribution
meets the selected coherence class; (3) largest ocean area fraction meets its
continuity class; (4) perimeter/area compactness (or a normalized
perimeter-to-area ratio) bounds fragmentation; (5) elevation variance excludes
flat and pathological terrain; and (6) boundary/slope association meets a
calibrated interval or directional minimum when boundary lift is enabled.
Use spherical area metrics, not largest cell-count fraction, for acceptance.
Correlation is undefined/unstable for near-constant inputs; validator must
return a validation error or an explicitly specified sentinel, never silently
accept a non-finite result. A zero boundary field means this constraint is
disabled by recipe configuration, not that the correlation requirement passes.

Do not set final numeric cutoffs from the supplied one-seed results. Calibration
procedure: generate a preregistered matrix spanning canonical level 5 and
supported seeds; include each single-family recipe, combined recipe, and
ablations for boundary lift and each scale. Record all spherical metrics,
runtime, and deterministic repeat equality. Have terrain owners classify
acceptable/rejectable samples blind to recipe, then choose intervals that
separate those classes with margin and check neighboring mesh levels. Freeze
the dataset, recipe/configuration, and metric version alongside the thresholds;
recalibrate only through a versioned discriminator change. Require thresholds
to reject known-fragmented and flat controls while retaining the accepted
coherent-region and combined samples.

## 6. Performance requirements

Do not ship the experimental `smooth_radius` implementation that turns radius
into O((radius/h)^2) diffusion iterations. Retain its scale contract and replace
the kernel with a bounded-cost mesh smoothing/filter method, validated against
the current implementation as a reference on small meshes. Candidate kernels
must document approximation error, stability, area weighting, and behavior at
large radii; choose based on measured level-5 throughput and output quality,
not theoretical complexity alone.

Cache immutable mesh calibration (edge/area-derived scales or weights) by
canonical mesh identity and smoothing configuration. It must not be rebuilt
per operator invocation. Cache lifetime/invalidation follows mesh identity;
never key only by mesh level. Measure cold and warm evaluation separately.
`correlated_noise` must also avoid per-cell expensive mesh-wide recalibration.
Use the existing whole-field execution model and approved parallelism; do not
add incremental scheduling or a new dependency as an optimization shortcut.

Acceptance performance gate: at canonical level 5, benchmark the full terrain
recipe and individual operators with Divan, report cold/warm calibration,
allocation and wall time, and compare against the experiment baseline. No
radius may imply hundreds/thousands of sequential diffusion passes. Set an
absolute latency budget after measuring the current canonical workload; until
then require asymptotic bounded pass count and demonstrate that the combined
recipe does not dominate total generation runtime. Preserve bitwise
determinism across duplicate runs (or document any deliberate floating-point
determinism boundary and test it).

## 7. Follow-up implementation tickets

| Ticket | Owner | Dependencies | Acceptance tests | Parallelization boundary |
|---|---|---|---|---|
| MUSE-29: Shared component labeling and spherical summaries | Runtime algebra owner | Coordinator approval of shared type/descriptor changes; this ADR | Deterministic labels; empty/all-true/all-false masks; disconnected and seam-adjacent fixtures; spherical component area/perimeter agrees with validator reference; malformed adjacency rejected | Must land before both region-component generation and validator deduplication. Own component core and its direct unit tests. |
| MUSE-30: Correlated field and bounded smooth-radius kernels | Spatial operators owner | MUSE-29 not required; mesh geometry API; dependency policy | Keyed duplicate reproducibility, scale ordering/response, conservation/area weighting for smoothing, radius-zero identity, bounded work at level 5; Divan cold/warm benchmarks | Independent of component labeling; share only finalized field/seed contracts, not source files. |
| MUSE-31: Region scalar and signed boundary interaction | Region operators owner | MUSE-29; stable category semantics | Complete map/default validation; finite-value checks; positive/negative signed cases; equal-region zero; no-boundary behavior; deterministic edge aggregation | Can design in parallel with MUSE-30 after API contract review; implementation waits for shared category/component contract where relevant. |
| MUSE-32: Terrain recipe integration and calibration corpus | Terrain/discriminator owner | MUSE-29, MUSE-30, MUSE-31 | Level-5 multi-seed corpus; ablations; recipe determinism; documented calibration ranges; hard constraints reject flat/fragmented controls and accept calibrated positives | Starts after operator contracts freeze; may prepare corpus tooling independently, but thresholds cannot freeze before all kernels integrate. |
| MUSE-33: Validator component-core consolidation | Validator owner | MUSE-29 | Existing spherical metrics preserved; same shared labels/edge convention as generation; tests for land/ocean complement and area/perimeter consistency | Can proceed in parallel with MUSE-30/31 after MUSE-29 API is stable. |

Only coordinator modifies root `[workspace.dependencies]` and shared frozen
type/descriptor registries. Any missing capability follows `DEPENDENCY_REQUEST.md`.

## 8. Migration from current W2-E / W2-F

Treat experiment code and outputs as evidence, not production APIs. First freeze
the revised signatures and component identity rules. Move the component
traversal/geometry logic behind the shared mesh primitive, then rewire validator
metrics to consume it without changing their published meanings. Port
`region_scalar` and signed boundary semantics with compiler/runtime tests.
Port correlated noise's keying and scale semantics. Replace repeated-diffusion
`smooth_radius` internals with the measured bounded-cost method while retaining
radius-facing recipe configuration. Keep experiment fixtures as reference
inputs where licensing/maintenance permits, and compare outputs statistically,
not require identical values when kernel semantics intentionally improve.
Finally migrate the canonical recipe behind a versioned recipe change, run the
calibration procedure, and update discriminator thresholds atomically with the
recipe version. No production YAML/operator implementation is part of this
architecture decision.

## 9. Risks and open questions

- The experiment commit objects and `RESULT.md` files were not present in this
  checkout, so this decision uses the supplied result summaries; obtain the
  reports before implementation to confirm operator details and exact
  measurement definitions.
- The spatial-only run's five land components conflict with a strict
  one-landmass requirement despite its dominant component fraction. Product
  semantics must decide whether islands are acceptable; calibration must
  encode that choice.
- Boundary/slope correlation 0.346 is a single observed association, not
  proof of causal usefulness. Compare ablations and avoid a hard positive
  minimum unless robust across seeds.
- Confirm whether `boundary_relative_normal` produces signed scalar or vector
  values in the experiment; current decision assumes scalar signed interaction.
- Cross-platform bitwise determinism of floating-point smoothing may be
  incompatible with parallel reductions; specify an acceptable reproducibility
  contract before implementation.
- Correlation length and smoothing radius may be redundant if both kernels
  perform similar filtering. Test frequency response and remove one only if
  recipes cannot independently control broad versus local structure.
- Validate physical length calibration across mesh refinements and ensure
  category labels remain stable enough for declarative attribute maps.
