# MUSE World Generation System Specification

**Status:** PoC architecture specification
**Audience:** MUSE implementation agents, reviewers, and future maintainers
**Primary purpose:** Alignment and grounding for implementation
**Scope:** World generation, validation, optimization, simulation stepping, perturbation, and debug visualization
**Out of scope:** Civilization simulation, NPC simulation, settlement placement, trade, politics, multiplayer, fog of war, cartographic presentation, production persistence, and final game UI

---

## 1\. Normative language

The terms **MUST**, **MUST NOT**, **SHOULD**, **SHOULD NOT**, and **MAY** are normative\.

This document intentionally distinguishes:

- **normative decisions** — implementation is expected to follow them;
- **PoC simplifications** — intentionally limited choices made to maximize learning speed;
- **experimental questions** — explicitly unresolved and not authorization for individual agents to redesign the architecture\.

When implementation work exposes a conflict with a normative decision, the agent MUST stop and escalate rather than silently work around the contract\.

---

## 2\. Product hypothesis

MUSE’s world layer is a **general, compositional world generator and simulator**, not a fixed fantasy\-map generator\.

The system is intended to determine whether a relatively small set of generic spatial and numerical primitives can express a broad range of terrestrial, fantasy, and science\-fiction worlds while remaining:

1. visually believable;
2. causally understandable;
3. deterministically measurable;
4. steerable by optimization;
5. evolvable after generation;
6. extensible to novel world mechanics without adding bespoke engine subsystems\.

The critical architectural principle is:

> **Rust implements the algebra. YAML + CEL implements the world.**

The engine should know as little as practical about concepts such as mountains, rainfall, plate tectonics, rivers, vegetation, or mana\. Those concepts should be expressed as compositions of generic primitives\.

The PoC is successful only if this architectural hypothesis is demonstrated, not merely if a plausible\-looking map is produced\.

---

## 3\. Conceptual boundaries

### 3\.1 The world layer

The world layer owns physical and world\-specific state such as:

- spatial geometry and topology;
- elevation and potential fields;
- broad geological structure;
- temperature and forcing fields;
- wind and other vector fields;
- moisture and precipitation;
- runoff and hydrology;
- vegetation or other ecology fields;
- generic materials/properties where needed;
- arbitrary fantasy/science\-fiction fields such as mana;
- temporal state needed to evolve those systems\.

### 3\.2 Civilization and NPC layers

Civilizations and NPCs are explicitly outside world generation\.

They MAY later consume world state to decide where to settle, travel, build, extract resources, or expand\. They MAY later feed modifications back into the world through perturbations such as deforestation, mining, dams, irrigation, pollution, or magical activity\.

The world generator MUST NOT contain civilization\-level objectives such as:

- trade route quality;
- distance between capitals;
- political borders;
- city density;
- road networks;
- strategic military balance\.

Those systems belong above the world layer\.

### 3\.3 Rendering and cartography

Production cartography, raster tiling, vector tiles, OpenStreetMap\-style interfaces, cultural map styling, and civilization\-specific presentation are downstream concerns\.

However, **debug visualization is a PoC requirement**\. Visual feedback is part of correctness because numerical metrics can reward worlds that are obviously unreasonable to a human observer\.

---

## 4\. End\-to\-end architecture

The intended generation pipeline is:

```text
user worldbuilding conversation
        |
        v
       LLM
        |
        +--------------------+
        |                    |
        v                    v
   world_spec           validation_spec
        |                    |
        v                    v
  world compiler       validator compiler
        |                    |
        v                    v
   WorldProgram         ValidationProgram
        |
        v
 candidate parameters + stable seed
        |
        v
   generate / relax
        |
        v
      World
        |
        v
 discriminator(world)
        |
        v
constraints + metrics + objectives + score
        |
        v
     optimizer
        |
        +---- mutates candidate parameters ----+
                                               |
                                               +--> regenerate
```

Once a world is accepted:

```text
accepted world
    |
    +--> step simulation
    |
    +--> perturb state
    |
    +--> step simulation
    |
    +--> inspect downstream consequences
```

The optimizer MUST normally search parameter values declared by `world_spec`\. It MUST NOT rewrite the structural world specification on every candidate evaluation\.

---

## 5\. Specification languages

### 5\.1 Choice

MUSE uses:

- **YAML** for document structure;
- **CEL** for bounded expressions\.

MUSE MUST NOT implement a custom textual lexer/parser for the PoC\.

YAML is treated as a serialized abstract syntax tree, not as the computational language itself\. CEL is used where compact arithmetic, comparisons, and predicates are useful\.

### 5\.2 CEL responsibilities

CEL MAY be used for:

- pointwise arithmetic over bound field values;
- scalar arithmetic;
- booleans and predicates;
- bounded scoring expressions;
- simple vector component expressions if exposed by the host\.

CEL MUST NOT be responsible for:

- traversing all world cells;
- graph algorithms;
- hydrology traversal;
- connected\-component computation;
- diffusion/advection solvers;
- arbitrary filesystem or network operations;
- unbounded iteration;
- arbitrary code execution\.

Expensive spatial operations are native engine operators or metrics and are exposed to YAML/CEL through typed ports or compact scalar results\.

---

## 6\. World specification

A `world_spec` describes:

- top\-level world geometry;
- parameters and their search ranges;
- declarative recipes;
- generation program;
- simulation step program;
- named fields and outputs;
- optional authored constants/anchors supported by the available algebra\.

A `world_spec` describes **what computations define this world**, not the desired quality of the generated result\. Desired resulting properties belong in `validation_spec` whenever practical\.

### 6\.1 Parameters

Parameters MAY be fixed or searchable\.

Example:

```yaml
version: 1

parameters:
  uplift_strength:
    type: scalar
    default: 1.0
    search:
      min: 0.2
      max: 3.0
      mutation_sigma: 0.08

  sea_level:
    type: scalar
    default: 0.0
    search:
      min: -0.5
      max: 0.5
      mutation_sigma: 0.05

  plate_count:
    type: integer
    default: 12
    search:
      min: 6
      max: 24
```

The compiler MUST extract the searchable parameter space into an explicit `ParameterSpace` consumed by the optimizer\.

### 6\.2 Recipes

A recipe is a declarative, reusable dataflow fragment\.

Recipes MUST be data\. Domain recipes MUST NOT require a Rust recompilation in the normal PoC workflow\.

Example:

```yaml
version: 1
name: orographic_precipitation

inputs:
  elevation: scalar_field
  wind: vector_field
  moisture: scalar_field

parameters:
  strength:
    type: scalar
    default: 1.0

nodes:
  elevation_gradient:
    op: gradient
    args:
      field: $input.elevation

  upslope:
    op: pointwise
    inputs:
      wind: $input.wind
      gradient: $node.elevation_gradient
    expr: max(dot(wind, gradient), 0.0)

  precipitation:
    op: pointwise
    inputs:
      moisture: $input.moisture
      upslope: $node.upslope
    params:
      strength: $param.strength
    expr: moisture * upslope * strength

outputs:
  precipitation: $node.precipitation
```

Recipes MAY compose other recipes using `use:`\.

Recipe recursion is forbidden\. Recursive inclusion MUST fail compilation\.

### 6\.3 Programs

A world defines at least two programs:

- `generate` — constructs or derives the initial world;
- `step` — advances mutable world state by one simulation step\.

Example shape:

```yaml
programs:
  generate:
    nodes:
      terrain:
        use: terrain_generation
      climate:
        use: climate
        inputs:
          elevation: $node.terrain.elevation

    outputs:
      elevation: $node.terrain.elevation
      temperature: $node.climate.temperature

  step:
    nodes:
      moisture_next:
        use: moisture_step
        inputs:
          moisture: $state.moisture
          vegetation: $state.vegetation
          wind: $state.wind

    updates:
      moisture: $node.moisture_next.moisture
```

The `step` program consumes the current `WorldState` and produces explicit state updates\. Calling `step` N times means N simulation steps\.

There is intentionally no general\-purpose imperative scripting model in the PoC\.

---

## 7\. Validation specification

A `validation_spec` describes **what world characteristics are desired**\.

It is compiled separately from `world_spec`\.

It consists of:

- named metrics;
- hard constraints;
- soft objectives;
- optional weights;
- later, potentially, counterfactual experiments\.

Example:

```yaml
version: 1

metrics:
  mountain_fraction:
    op: fraction_above
    field: slope
    threshold: 0.18

  mana_fault_correlation:
    op: correlation
    a: mana
    b: fault_proximity

constraints:
  enough_land:
    expr: metrics.land_fraction > 0.15

objectives:
  mountainousness:
    expr: 1.0 - min(abs(metrics.mountain_fraction - 0.60) / 0.60, 1.0)
    weight: 1.0

  fault_aligned_mana:
    expr: clamp(metrics.mana_fault_correlation, 0.0, 1.0)
    weight: 1.0
```

### 7\.1 Validator result

The discriminator MUST return structured output, not only a scalar:

```json
{
  "valid": true,
  "score": 0.82,
  "metrics": {
    "mountain_fraction": 0.61,
    "mana_fault_correlation": 0.77
  },
  "objectives": {
    "mountainousness": 0.98,
    "fault_aligned_mana": 0.77
  },
  "constraints": {
    "enough_land": true
  }
}
```

For the first PoC, the overall score MAY be a weighted mean of objectives when all hard constraints pass and zero otherwise\.

The structured objective vector MUST be retained even if a scalar score is produced\.

---

## 8\. Spatial representation

### 8\.1 PoC canonical surface

The PoC uses a **fixed subdivided icosphere** with stable cell identities\.

Target initial resolution: approximately **10,242 surface cells**\.

The implementation SHOULD use an established maintained spherical\-mesh crate rather than implementing subdivision from scratch\.

Canonical topology MUST be independent of:

- plate locations;
- terrain;
- climate;
- world seed;
- optimizer candidate parameters\.

Generated geographic concepts such as plates, biomes, drainage basins, and faults are features defined **on** the fixed substrate; they are not the substrate itself\.

### 8\.2 Stable identity

Every cell MUST have a stable integer ID for a given mesh version and resolution\.

Stable IDs support:

- deterministic randomness;
- reproducible snapshots;
- field storage;
- graph/network references;
- future incremental computation;
- visualization;
- perturbation targeting\.

### 8\.3 Vectors

World vectors SHOULD use 3\-D Cartesian vectors projected onto the local tangent plane when representing surface directions\.

This avoids longitude/pole singularities and makes vector logic globally uniform\.

### 8\.4 Field storage

The PoC uses dense structure\-of\-arrays\-style storage\.

Canonical conceptual types:

```rust
pub type CellId = u32;

pub enum Field {
    Scalar(Vec<f64>),
    Vector(Vec<glam::DVec3>),
    Bool(Vec<bool>),
    Category(Vec<u32>),
    Index(Vec<u32>),
}

pub struct Network {
    pub edges: Vec<[CellId; 2]>,
    pub values: Option<Vec<f64>>,
}

pub struct WorldState {
    pub mesh: Mesh,
    pub fields: BTreeMap<String, Field>,
    pub networks: BTreeMap<String, Network>,
    pub parameters: BTreeMap<String, f64>,
    pub step: u64,
    pub seed: u64,
}
```

The exact Rust surface MAY differ where necessary, but the semantic model above is normative\.

The PoC SHOULD prefer `Vec<T>` plus slices and Rayon over a generalized tensor framework\.

`ndarray` is not a baseline dependency\. It MAY be approved later if genuine regular 2\-D/3\-D domains appear\.

---

## 9\. Execution algebra

The engine contains a small stable set of generic operators\. World/domain meaning is expressed by composing them\.

### 9\.1 Initial native operator vocabulary

The initial registry SHOULD contain approximately:

```text
constant
noise
voronoi_labels

pointwise
vector_expr

neighbor_sample
gradient
laplacian

diffuse
advect

boundary_strength
distance_to_mask

flow_direction
accumulate
network_threshold

reduce
iterate / relax
```

Exact naming is less important than preserving generic semantics\.

### 9\.2 Operator expectations

An operator:

- declares typed inputs and outputs;
- declares accepted parameters;
- is deterministic for identical inputs and semantic RNG keys;
- operates on generic fields/networks;
- contains no domain concept such as `rain`, `mountain`, `mana`, or `tectonic_plate` in its engine\-level semantics\.

### 9\.3 Pointwise expressions

`pointwise` evaluates CEL once per cell over named scalar inputs and parameters\.

`vector_expr` evaluates generic vector math and MUST project surface\-direction results onto the local tangent plane\.

### 9\.4 Numerical operators

Generic numerical algorithms such as gradients, diffusion, advection, and graph accumulation MAY be implemented as optimized native kernels because they are reusable mathematical capabilities rather than world\-domain logic\.

If a declarative recipe cannot be expressed efficiently or clearly with the current operator set, the default response is **not** to add a domain\-specific Rust recipe\.

Instead, implementation MUST determine whether a reusable general operator is missing\.

---

## 10\. Missing\-capability protocol

Agents assigned declarative recipes MUST NOT add Rust domain code\.

If blocked, they stop and produce:

```text
BLOCKER

recipe:
missing capability:
why existing operators cannot express it:
proposed generic operator:
input signature:
output signature:
examples of at least two unrelated domains that could reuse it:
minimal acceptance test:
```

A coordinator decides whether to:

1. add the generic operator;
2. simplify the recipe;
3. reject the requested behavior for the PoC\.

This protocol is intentional: algebra deficiencies are experimental evidence\.

---

## 11\. Compilation model

The specification compiler owns semantic compilation, not textual parsing\.

Pipeline:

```text
YAML parser
    |
    v
serde models
    |
    v
reference resolution
    |
    v
recipe expansion
    |
    v
type checking
    |
    v
CEL compilation
    |
    v
dependency graph construction
    |
    v
topological ordering
    |
    v
WorldProgram / ValidationProgram IR
```

The compiler MUST reject at least:

- unknown operators;
- unknown references;
- duplicate node names;
- missing required arguments;
- extra/unknown parameters where the schema forbids them;
- invalid CEL;
- field type mismatches;
- invalid recipe recursion;
- illegal dependency cycles outside explicitly supported iteration constructs\.

Errors MUST include the YAML path when practical\.

Example:

```text
programs.generate.nodes.rain.inputs.wind:
expected vector_field, got scalar_field
```

The compiler MUST NOT attempt creative recovery from invalid specifications\.

---

## 12\. Generation model

World generation is a causal dataflow over the same state representation used at runtime\.

A baseline world may follow approximately:

```text
stable plate regions
      |
      v
boundary / convergence forcing
      |
      v
elevation
      |
      +-------------------+
      |                   |
      v                   v
 temperature             slope
      |                   |
      v                   |
     wind                 |
      |                   |
      +--------+----------+
               |
               v
       moisture / precipitation
               |
               v
             runoff
               |
               v
          flow direction
               |
               v
           discharge
               |
               v
          vegetation
```

The exact PoC physics are intentionally simplified\.

The required property is not scientific fidelity\. It is that downstream results are causally related to upstream state in a way that is visually and mechanically understandable\.

### 12\.1 Generation versus literal history

The PoC MUST NOT simulate billions of years merely to obtain initial state\.

Generation MAY use:

- analytical initialization;
- stable procedural fields;
- direct derivation;
- accelerated relaxation;
- fixed\-point iteration;
- coarse equilibrium solving\.

The world representation, however, SHOULD remain usable directly by the runtime `step` program once accepted\.

---

## 13\. Baseline declarative world systems

The PoC SHOULD attempt to express the following as YAML/CEL compositions over the generic algebra\.

### 13\.1 Terrain

Required outputs:

```text
plate_id
plate_boundary
tectonic_forcing
elevation
land_mask
slope
```

Suggested causal composition:

```text
stable plate seeds / plate labels
        |
        v
category boundaries
        |
        v
boundary strength
        |
        v
diffuse uplift/subsidence forcing
        |
        +--> stable broad noise
        |
        v
     elevation
        |
        v
       slope
```

The PoC does not require a realistic mantle or full plate tectonics model\.

### 13\.2 Climate

Required outputs:

```text
insolation
temperature
wind
moisture
precipitation
```

Suggested composition:

```text
latitude + stellar forcing
        |
        v
   insolation
        |
        +--> elevation adjustment
        |
        v
   temperature
        |
        v
heuristic prevailing wind
        |
        v
moisture advection
        |
        +--> elevation gradient / upslope
        |
        v
 precipitation
```

### 13\.3 Hydrology

Required outputs:

```text
runoff
flow_to
discharge
river_mask
```

Suggested composition:

```text
precipitation + vegetation
        |
        v
      runoff
        |
        +--> elevation/potential
        |
        v
 flow direction graph
        |
        v
    accumulation
        |
        v
     discharge
        |
        v
    river_mask
```

The first PoC MAY simplify depression handling\. It MUST NOT falsely claim physically complete hydrology\.

### 13\.4 Vegetation

Required output:

```text
vegetation in [0, 1]
```

It SHOULD respond to temperature and precipitation/moisture\.

The `step` program SHOULD include at least one feedback from vegetation into moisture and/or runoff so that deforestation has downstream consequences\.

### 13\.5 Fantasy field: mana

Mana is the canonical extensibility test\.

Required outputs:

```text
fault_proximity
mana_source
mana
mana_heat
```

Suggested composition:

```text
tectonic boundary / forcing
        |
        v
    mana source
        |
        +--> fault proximity modifies transport
        |
        v
   diffusion / flow
        |
        v
       mana
        |
        v
    heat coupling
```

The Rust engine MUST NOT contain a special `ManaSystem` or `ManaField` semantic type\.

If mana cannot be expressed without bespoke domain infrastructure, that is evidence against the current algebra\.

---

## 14\. Open\-ended fantasy/science\-fiction mechanics

The general model is that unusual mechanics should often compile into combinations of:

- fields;
- sources and sinks;
- transport;
- local transformations;
- couplings;
- networks;
- constants/forcing functions;
- state transitions where later supported\.

Examples:

### 14\.1 Multiple suns

The climate system SHOULD consume stellar/insolation forcing rather than assume exactly one sun\.

Two suns can therefore be represented as multiple forcing contributions rather than a separate climate engine\.

### 14\.2 Alternate gravity

Systems such as hydrology SHOULD conceptually depend on an effective potential/downhill field rather than hard\-code every assumption into named Earth concepts where practical\.

### 14\.3 Underground magical currents

A fictional current can be represented as a scalar/vector/network transport field whose conductivity or pathway is coupled to existing structure such as faults\.

### 14\.4 Limits

The system does not promise arbitrary physics\.

A world mechanic that requires full volumetric fluid dynamics, moving meshes, topology\-changing geometry, or another fundamentally new degree of freedom may exceed the PoC algebra\.

The desired behavior is to find a useful executable analogue where possible, not to claim universal physical simulation\.

---

## 15\. Simulation stepping

The accepted initial world is stateful\.

MUSE MUST NOT regenerate the entire world from seed every turn as its canonical runtime semantics\.

The world evolves:

```text
World[t] + perturbations/events
        |
        v
      step
        |
        v
World[t+1]
```

For the PoC, `step` MAY eagerly recompute complete fields\.

Incremental recomputation is not a requirement for the core architectural hypothesis\.

---

## 16\. Perturbations

The first PoC needs a deliberately small perturbation vocabulary\.

At minimum:

### 16\.1 Field scale

```yaml
type: field_scale
field: vegetation
region:
  center_cell: 1234
  radius_hops: 12
factor: 0.1
```

### 16\.2 Field add

```yaml
type: field_add
field: mana
region:
  center_cell: 4321
  radius_hops: 5
amount: 1.0
```

The event mutates direct state only\.

It MUST NOT contain scripted consequences such as “then increase river flow” or “then warm rainfall\.”

Consequences must emerge from subsequent world\-program execution\.

### 16\.3 Canonical perturbation tests

The integrated PoC SHOULD demonstrate:

1. **Deforestation**
   - vegetation decreases;
   - moisture/runoff changes;
   - downstream hydrological/climatic values change\.
2. **Mana injection**
   - mana increases locally;
   - the field propagates according to generic transport;
   - coupled heat/climate state changes\.

These are integration tests for causality, not claims of Earth\-system accuracy\.

---

## 17\. Determinism and randomness

Identical:

```text
engine build
world_spec
parameter assignment
world seed
```

MUST produce the same PoC world snapshot on the supported deterministic execution path\.

### 17\.1 Semantic random keys

Randomness MUST NOT depend on mutable global execution order\.

Random draws SHOULD be semantically keyed by values such as:

```text
world_seed
operator_node_id
stable_cell_or_feature_id
simulation_step
draw_index
```

The implementation MAY derive independent deterministic streams using a stable hash/key derivation function and a pinned deterministic RNG\.

### 17\.2 Forbidden randomness

Engine code MUST NOT call ambient/thread\-local randomness for world\-state decisions\.

This includes casual use of `thread_rng()` or equivalent APIs whose sequence depends on execution ordering\.

---

## 18\. Optimization model

Optimization searches the `ParameterSpace` declared by the world specification\.

The optimizer owns a candidate parameter assignment or genome; it does not mutate the YAML structure itself during ordinary search\.

### 18\.1 PoC optimization goal

The first goal is not to build an advanced optimizer\.

The goal is to show that changing declared parameters can steer visible world properties while preserving coherent generation\.

Example:

```text
initial mountain_fraction = 0.31
requested target           = 0.60
optimized result           ~= 0.60
```

The result must still satisfy hard constraints and remain visually plausible\.

### 18\.2 Established optimization libraries

Agents MUST NOT implement generic established optimization algorithms from scratch when a maintained approved crate provides the required capability\.

The exact first optimization algorithm MAY be fixed by the coordinator based on the final implementation stack\.

### 18\.3 Search locality

A desirable genotype property is:

> small parameter changes usually produce bounded, related phenotype changes.

The PoC SHOULD avoid treating a global seed as a normal mutation gene because reseeding tends to destroy search locality\.

---

## 19\. Validation metrics

Initial native metric vocabulary SHOULD be intentionally small:

```text
mean
variance
quantile
fraction_above
fraction_below
correlation
component_count
largest_component_fraction
network_density
```

Additional generic metrics MAY be added only when required by a concrete validation target\.

CEL combines native metric results into constraints/objectives\.

### 19\.1 Human visual review

A passing metric score is insufficient\.

The PoC MUST expose generated states visually so a human reviewer can identify obvious pathologies such as:

- nonsensical rivers;
- extreme field discontinuities;
- meaningless noise masquerading as terrain;
- degenerate optimization exploits;
- mana correlation that is numerically high but visually trivial\.

Human visual feedback is a required feedback mechanism during recipe tuning\.

---

## 20\. Incremental computation

### 20\.1 Mainline PoC position

The project has **not** rejected incrementality\.

It has rejected building a custom incremental framework before proving it is needed\.

The baseline execution path is:

```text
compiled recipe DAG
        |
        v
topological sequence
        |
        v
whole-field operators
        |
        v
dense slices / Vec<T>
        |
        v
Rayon parallel execution
```

This path SHOULD remain as a simple full\-recompute correctness reference even if incrementality is later adopted\.

### 20\.2 Timely/Differential experiment

Timely/Differential Dataflow is an explicitly approved research spike, not a baseline dependency until accepted\.

A bounded spike SHOULD compare the same representative computation using:

1. dense/Rayon execution;
2. Timely/Differential where naturally expressible\.

Representative pipeline:

```text
scalar source
→ pointwise transform
→ neighborhood propagation
→ iterative relaxation
→ aggregate metric
```

Then apply a localized change affecting approximately 1% of the spatial domain\.

The comparison SHOULD report:

- correctness equivalence;
- source and adapter/glue complexity;
- initial computation runtime;
- localized update runtime;
- peak memory;
- fit with the compiled operator interface\.

Differential SHOULD be adopted only if it:

1. does not materially complicate the compiler/operator abstraction;
2. yields meaningful localized\-update benefit;
3. does not force dense numerical operators into unnatural relational encodings\.

No agent is authorized to independently adopt Timely/Differential into the production PoC path\.

---

## 21\. Parallel execution and performance

### 21\.1 Default parallelism

Whole\-field CPU work SHOULD use Rayon where trivially parallel\.

Agents MUST NOT build a custom thread pool or work\-stealing scheduler\.

Manual synchronization SHOULD be avoided unless profiling demonstrates a concrete need\.

### 21\.2 GPU compute

GPU computation is not a baseline PoC requirement\.

WebGPU or native GPU kernels MAY be investigated later if profiling demonstrates that the core hypothesis is being obscured by compute cost\.

GPU acceleration MUST NOT be introduced merely because an operation is theoretically parallelizable\.

---

## 22\. Debug visualization

Visualization is a core PoC feature\.

### 22\.1 Architecture

The visualizer SHOULD be browser based\.

The simulation/compiler/validator MAY remain native Rust for performance and simplicity\.

The viewer consumes a generic serialized snapshot and MUST NOT need to understand domain semantics\.

Conceptual boundary:

```text
native Rust engine
      |
      | JSON/binary snapshot later
      v
browser viewer
      |
      v
WebGL/WebGPU rendering
```

### 22\.2 PoC implementation

The initial viewer MAY use TypeScript \+ Three\.js/WebGL\.

WASM is optional for the first spike\. The architecture should not require simulation code to run in the browser\.

### 22\.3 Required capabilities

The PoC viewer MUST support:

- globe display;
- scalar field selector and coloring;
- optional vector field arrows;
- optional network overlays;
- timestep/snapshot selection;
- hovered cell ID and current value;
- deterministic loading of canonical fixtures\.

It SHOULD make these visually distinguishable:

- large\-scale relief;
- plate/fault structures;
- rain\-shadow\-like precipitation differences;
- river concentration;
- fault\-aligned mana\.

### 22\.4 Viewer semantics

The viewer understands generic data types:

```text
mesh
scalar_field
vector_field
categorical_field
network
```

It SHOULD NOT contain special\-case rendering logic for “mana,” “rain,” “mountains,” etc\., beyond user\-configurable display defaults\.

---

## 23\. Repository structure

Recommended PoC structure:

```text
/crates
  /muse-types
  /muse-geom
  /muse-spec
  /muse-ops
  /muse-validate
  /muse-optimize
  /muse-cli

/specs
  /recipes
    /terrain
    /climate
    /hydro
    /fantasy
  /worlds
  /validation
  /events

/apps
  /viewer

/fixtures
/scripts
/docs
```

Shared contracts, root manifests, schemas, and operator registry signatures are coordinator\-owned\.

Work agents MUST NOT modify other workstream\-owned areas unless explicitly instructed\.

---

## 24\. Dependency governance

### 24\.1 Principle

> **Build almost nothing generic ourselves.**

If a mature, maintained, composable crate solves commodity infrastructure, use it rather than assigning an agent to recreate that infrastructure\.

MUSE\-specific custom code should be concentrated in:

- the world algebra;
- spatial/numerical semantics not provided directly by generic crates;
- the spec compiler;
- MUSE validation semantics;
- declarative recipes\.

### 24\.2 Baseline approved dependencies

The root workspace SHOULD centrally pin the approved baseline dependency set through `[workspace.dependencies]` and `Cargo.lock`\.

Baseline direction:

|Capability                  |Approved dependency            |
|----------------------------|-------------------------------|
|Serialization               |`serde`                        |
|JSON snapshots              |`serde_json`                   |
|YAML                        |`serde-saphyr`                 |
|CEL                         |`cel`                          |
|Vector math                 |`glam`                         |
|Spherical mesh              |`hexasphere`                   |
|CPU data parallelism        |`rayon`                        |
|Generic DAG/graph algorithms|`petgraph`                     |
|Procedural noise            |`fastnoise-lite`               |
|Deterministic RNG           |`rand_chacha`                  |
|Stable hash/key derivation  |`blake3`                       |
|Library errors              |`thiserror`                    |
|App/CLI error context       |`anyhow`                       |
|CLI                         |`clap`                         |
|Structured logging          |`tracing`, `tracing-subscriber`|
|Property tests              |`proptest`                     |
|Snapshot tests              |`insta`                        |
|Microbenchmarks             |`divan`                        |

Leaf crates SHOULD reference workspace dependencies rather than introduce versions locally\.

Only the coordinator edits the workspace dependency allowlist\.

### 24\.3 Approved on demand

These are not baseline dependencies but MAY be approved by the coordinator when a concrete need arises:

|Dependency                        |Trigger                                                         |
|----------------------------------|----------------------------------------------------------------|
|`uom`                             |runtime/compiler unit system enters PoC scope                   |
|`ndarray`                         |real regular 2-D/3-D domains appear                             |
|`schemars`                        |generated JSON schema becomes useful                            |
|`argmin`                          |optimization requires an established generic algorithm/framework|
|`timely` / `differential-dataflow`|the incremental spike passes adoption criteria                  |
|`wgpu`                            |Rust/WASM viewer or GPU compute is justified                    |
|`wasm-bindgen`                    |Rust actually moves into the browser                            |
|`bytemuck`                        |zero-copy GPU/binary buffers are needed                         |
|`smallvec`                        |profiling demonstrates adjacency allocation pressure            |

### 24\.4 Forbidden direct choices for the PoC

The following are denied as direct dependencies unless the coordinator explicitly changes the architecture:

**Alternatives to standardized choices**

```text
serde_yaml                    -> use serde-saphyr
nalgebra / cgmath / vek       -> use glam
noise / simdnoise             -> use fastnoise-lite
criterion                     -> use Divan for the PoC
```

**Uncontrolled randomness**

Direct ambient\-randomness usage is forbidden in engine code\. Deterministic engine randomness must flow through the approved keyed RNG abstraction\.

**Async runtimes**

```text
tokio
async-std
smol
```

No async runtime is required by the simulation core\.

**Architecture\-changing dependencies**

```text
ECS frameworks
native rendering/game engines
runtime plugin loaders
new database engines
new persistence formats
FFI subsystems
CUDA/OpenCL bindings
Timely/Differential outside the approved spike
```

These are not judgments that the libraries are bad; they are architecture decisions that implementation agents are not authorized to make\.

### 24\.5 New dependency protocol

When required functionality is not available in the approved set, an agent MUST stop and create a dependency request containing:

```text
capability:
why required:
approved crates considered:
candidate crate:
version:
maintenance evidence:
license:
exact API expected to be used:
alternative if rejected:
```

Implementation resumes only after coordinator approval\.

---

## 25\. Development environment

The development environment is frozen and reproducible\.

### 25\.1 Nix

The supported entry point is:

```bash
nix develop
```

Nix defines:

- Rust toolchain;
- build/test tools;
- profiling tools;
- frontend toolchain;
- native libraries\.

Entering `nix develop` SHOULD NOT build the application itself\.

Mutable development state such as `target/` MUST remain outside immutable Nix derivations\.

### 25\.2 Crane

Crane SHOULD provide reproducible Rust build/check derivations and dependency\-artifact reuse at CI/reproducibility boundaries\.

Development agents SHOULD use native Cargo commands inside `nix develop` for fast edit\-feedback cycles rather than repeatedly invoking `nix build` after every edit\.

### 25\.3 Rust development loop

Agents SHOULD follow:

```text
edit
 ↓
cargo check -p affected-crate
 ↓
targeted test
 ↓
crate tests
 ↓
workspace validation
```

Persistent `target/` state MUST NOT be routinely deleted\.

Separate development, CI, runnable\-E2E, and production Cargo profiles SHOULD be used where needed rather than forcing one profile to serve all purposes\.

---

## 26\. Testing infrastructure

### 26\.1 Test runner

`cargo-nextest` is the standard workspace test runner\.

### 26\.2 Property testing

`proptest` SHOULD be used for invariants such as:

- mesh adjacency symmetry;
- unit sphere positions;
- tangent vector projection;
- diffusion stability properties;
- valid flow receivers;
- compiler rejection of malformed references\.

### 26\.3 Snapshot testing

`insta` SHOULD be used where stable textual/structured snapshots provide useful regression coverage, especially for:

- compiled IR;
- compiler diagnostics;
- validator output;
- compact canonical fixture summaries\.

### 26\.4 Frontend testing

The browser viewer SHOULD use Playwright for smoke/integration tests\.

A canonical test SHOULD:

1. load a known snapshot;
2. switch scalar fields;
3. toggle a network overlay;
4. verify no browser console errors;
5. capture screenshots for review\.

---

## 27\. Benchmarking and profiling

### 27\.1 Divan

Divan is the default PoC benchmark harness\.

It is chosen for fast agent\-driven feedback and compact benchmark authoring\.

Divan SHOULD benchmark isolated operations such as:

- gradient;
- diffusion;
- advection;
- accumulation;
- compilation;
- full candidate generation\.

Criterion is not a baseline PoC dependency\. It MAY be added later if statistically rigorous CI performance\-regression gating becomes useful\.

### 27\.2 `perf`

Linux `perf` SHOULD be available in the Nix profiling environment for conventional runtime hotspot analysis\.

### 27\.3 Coz

Coz SHOULD be available as a profiling tool for causal profiling of native Rust workloads\.

Its role is to answer:

> If this code path became faster, would end-to-end simulation throughput actually improve?

Coz SHOULD be provisioned as a tool rather than forced into the baseline runtime dependency graph\.

A standard profiling command SHOULD run a canonical simulation workload and emit agent\-readable textual output where possible\.

### 27\.4 Optimization rule

Agents MUST NOT introduce nontrivial performance optimization solely from intuition\.

Performance work SHOULD be supported by:

1. a representative benchmark or profile;
2. a proposed change;
3. before/after measurement\.

---

## 28\. Cargo/Nix quality gates

The repository SHOULD provide fixed commands, not agent\-selected variants\.

At completion, the PoC MUST pass the equivalent of:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace
cargo deny check
cargo build --workspace --release

cd apps/viewer
pnpm test
pnpm build

cd ../..
nix flake check
./scripts/poc.sh
```

Expensive full gates need not run after every edit\. Agents SHOULD test narrowly first and run the full suite before declaring work complete\.

---

## 29\. Dependency\-policy enforcement

The root workspace dependency table acts as an allowlist\.

Leaf crates MUST use:

```toml
rayon.workspace = true
glam.workspace = true
```

rather than independently selecting versions\.

`cargo-deny` SHOULD enforce at least:

- no wildcard versions;
- no unknown registries;
- no unapproved git dependencies;
- yanked/advisory checks;
- explicit license policy;
- warnings for duplicate transitive versions rather than indiscriminately failing on them\.

Git dependencies are denied by default\. A necessary exception requires coordinator approval and a pinned commit\.

---

## 30\. Agent execution policy

Implementation agents are build workers against frozen contracts, not autonomous architects\.

Every coding\-agent task SHOULD include:

```text
You are implementing a pre-specified component.

Do not redesign architecture.
Do not modify shared contracts.
Do not add features not required by acceptance criteria.
Do not refactor unrelated code.
Do not add dependencies unless explicitly authorized.
Do not implement generic infrastructure already supplied by approved dependencies.
Do not implement domain-specific Rust code when assigned declarative recipes.

Work in this order:
1. Read frozen interfaces and tests.
2. Add/complete required tests.
3. Implement the minimum code necessary to pass them.
4. Run task-specific acceptance tests.
5. Run applicable broader tests.
6. Produce RESULT.md.

If blocked by a missing shared capability:
STOP and produce BLOCKER.md.
Do not work around the contract.
```

`RESULT.md` SHOULD contain:

- files changed;
- commands run;
- test results;
- acceptance\-criteria status;
- known deviations;
- blockers\.

### 30\.1 Verification agents

Producer agents SHOULD NOT self\-certify completion\.

A separate verifier SHOULD run the exact acceptance procedure against the produced artifact/branch\.

Recommended flow:

```text
worker
  ↓
implementation
  ↓
verifier
  ↓
PASS
or structured failure report
  ↓
one bounded repair cycle
  ↓
coordinator escalation if still failing
```

Verifiers evaluate outputs and tests, not the worker’s narrative reasoning\.

---

## 31\. Commodity\-algorithm rule

Agents MUST distinguish MUSE\-specific semantics from commodity algorithms\.

Legitimate custom MUSE work includes:

```text
how a gradient should operate on our spherical cell representation
how our operator type system compiles YAML
how declarative world systems compose
how MUSE metrics expose world properties
```

Agents SHOULD NOT independently implement:

```text
YAML parsers
icosphere subdivision
vector/matrix libraries
thread pools
Perlin/Simplex/Voronoi noise algorithms
generic graph containers/topological sort
generic optimizers
random number generators
benchmark statistics frameworks
snapshot-test infrastructure
```

If established maintained code exists, use it\.

---

## 32\. PoC integration contract

The repository SHOULD expose a single integration script:

```bash
./scripts/poc.sh
```

The script SHOULD procedurally:

1. build the project;
2. run required tests;
3. compile canonical world and validation specs;
4. generate a baseline world;
5. validate the baseline;
6. optimize toward a target;
7. validate the optimized result;
8. step the accepted world;
9. apply deforestation;
10. step and measure consequences;
11. apply mana injection;
12. step and measure consequences;
13. export snapshots for the viewer;
14. run viewer smoke tests;
15. write a machine\-readable summary\.

Canonical output snapshots SHOULD include:

```text
baseline.json
optimized.json
deforested.json
mana_injected.json
```

The summary SHOULD contain:

- baseline score;
- optimized score;
- baseline/optimized candidate parameters;
- objective/metric changes;
- perturbation before/after measurements;
- artifact locations;
- test/gate status\.

---

## 33\. PoC execution scope

The PoC is a **1–2 day go/no\-go experiment**, assuming heavy agentic coding and horizontal delegation\.

It is not a conventional multi\-week implementation plan\.

### 33\.1 In scope

- \~10k\-cell fixed spherical world;
- YAML \+ CEL world recipes;
- YAML \+ CEL validation specs;
- generic Rust operator algebra;
- terrain;
- simplified climate;
- simplified hydrology;
- vegetation;
- mana extensibility case;
- simple parameter optimization;
- native simulation stepping;
- two perturbations;
- browser visualization;
- deterministic fixtures;
- tests/benchmarks/profiling scaffolding\.

### 33\.2 Explicitly out of scope

Unless effectively free, do not build:

- civilization simulation;
- NPC graph;
- settlements;
- trade;
- resources/economy gameplay;
- political control;
- multiplayer;
- map tiles;
- production cartography;
- fog of war;
- geological strata simulation;
- realistic sedimentation;
- groundwater solver;
- adaptive mesh refinement;
- distributed execution;
- production database/persistence;
- production event sourcing;
- GPU compute;
- sophisticated optimizer portfolio;
- custom incremental engine\.

---

## 34\. Canonical target worlds

At minimum, the architecture SHOULD be able to produce two substantially different declarative worlds using the same engine\.

### 34\.1 Control world

Characteristics:

- one stellar source;
- moderate tectonic forcing;
- recognizable large\-scale elevation;
- temperature decreasing broadly toward poles and with altitude;
- moisture/precipitation patterns;
- coherent river accumulation\.

### 34\.2 Fantasy stress world

Characteristics:

- altered stellar forcing, potentially two stellar sources;
- high tectonic activity;
- mana originating from tectonic/fault structure;
- mana transported by generic field/network operations;
- mana coupled to heat;
- climate responding to resulting heat field;
- target world biased toward mountainous/cold conditions through validation/optimization\.

The Rust engine MUST NOT contain world\-specific code for the second case\.

This comparison is one of the strongest tests of the algebra hypothesis\.

---

## 35\. Acceptance gates for important subsystems

### 35\.1 Geometry

For a canonical icosphere level:

- expected cell count matches the selected library/representation;
- all position vectors lie on the unit sphere within tolerance;
- neighbor relations are symmetric;
- cell IDs are deterministic;
- tangent projection yields vectors orthogonal to the radial direction within tolerance\.

### 35\.2 Operators

Representative gates:

- constant field preserves exact value;
- deterministic noise is identical for identical semantic keys;
- gradient of a constant field is approximately zero;
- Laplacian of a constant field is approximately zero;
- diffusion of a constant field remains constant;
- diffusion does not increase variance under the selected stable scheme;
- vector expressions produce tangent vectors where required;
- flow receiver is self or a neighbor;
- downhill flow receivers have lower potential where not self;
- accumulation matches known synthetic graph totals;
- no operator emits NaN/Inf for valid fixture inputs\.

### 35\.3 Terrain recipe

For a fixed fixture:

- no NaN/Inf;
- nondegenerate land/ocean fractions;
- nontrivial elevation variance;
- slopes are statistically stronger near generated plate boundaries than far away;
- visual output shows coherent large\-scale relief rather than white noise\.

### 35\.4 Climate/hydrology recipe

For a fixed fixture:

- equatorial or high\-insolation regions are warmer on average than low\-insolation regions absent overriding mechanics;
- higher elevation is cooler conditional on similar forcing;
- positive upslope exposure produces greater precipitation than negative upslope exposure under the same broad moisture regime;
- discharge distribution is highly skewed relative to local runoff;
- river mask is sparse but nonempty;
- vegetation remains bounded;
- increasing stellar forcing raises mean equilibrium temperature in the simplified model\.

### 35\.5 Fantasy recipe

- mana is nontrivial;
- mana is measurably associated with the intended tectonic/fault structure;
- coupling mana heat changes another downstream field;
- implementation contains no domain\-specific Rust mana subsystem\.

### 35\.6 Optimization

For an agreed target metric:

- initial candidate is materially away from target;
- fixed\-budget optimization measurably improves target fit;
- hard constraints remain satisfied;
- optimized world remains visually coherent\.

### 35\.7 Perturbation

- field mutation occurs only in the specified direct region;
- subsequent state changes appear in dependent fields without scripted consequence handlers;
- differences are visible and measurable;
- rerunning the same event from the same snapshot reproduces the same result\.

---

## 36\. Human feedback loop

The intended tuning loop is:

```text
edit YAML recipe / parameters
       |
       v
compile
       |
       v
generate / step
       |
       +--> automated metrics/tests
       |
       +--> browser visualization
       |
       v
adjust declarative composition
```

When automated gates pass but the world looks visibly wrong, the first response SHOULD be to adjust declarative recipe composition/parameters or improve a generic operator if genuinely necessary\.

Agents SHOULD NOT hide visual defects by altering only the validator\.

---

## 37\. Success criteria

The project continues only if the integrated PoC gives convincing evidence for the following four claims\.

### 37\.1 Composition

Terrain, climate, hydrology, vegetation, and mana are substantially declarative YAML/CEL compositions over generic Rust operators\.

A large collection of Rust domain systems is failure, even if the generated map looks attractive\.

### 37\.2 Steerability

Optimization visibly and measurably moves the **same underlying seeded world program** toward a requested target using declared parameters\.

### 37\.3 Causality

A direct state perturbation such as deforestation creates intuitive downstream field changes through declared dependencies rather than event\-specific consequence scripting\.

### 37\.4 Open semantics

A fictional mechanic such as mana participates in the same operator/data model and influences ordinary world systems without requiring a bespoke engine subsystem\.

---

## 38\. Kill criteria

The 1–2 day PoC SHOULD be considered a negative result if most of the following are true:

- domain recipes require substantial Rust implementations;
- YAML composition is intolerably verbose or brittle for even the small target systems;
- each new fantasy mechanic requires a new semantic engine type;
- generated worlds look arbitrary rather than causally structured;
- small parameter changes mostly destroy/reseed the phenotype;
- simple optimization cannot noticeably steer visible properties;
- perturbations produce no meaningful downstream effects;
- perturbations create uncontrollable instability;
- visual inspection consistently disagrees with validation metrics;
- runtime is so poor at \~10k cells that experimentation is impractical even with ordinary parallelism;
- the generic operator set expands rapidly into a disguised list of world\-domain systems\.

A negative result is an acceptable PoC outcome\. The project MUST NOT be prolonged merely to protect sunk cost\.

---

## 39\. Deferred design questions

The following remain explicitly deferred until the PoC provides evidence:

- whether Timely/Differential should become the execution substrate;
- whether fields should later use hierarchical or adaptive spatial resolution;
- whether a vertical column domain is needed;
- whether materials/layers deserve first\-class algebra types;
- whether a unit system should be compiled into the DSL;
- whether WebGPU compute should accelerate field operators;
- whether optimization should use CMA\-ES, evolutionary multi\-objective methods, MAP\-Elites, Bayesian optimization, or mixed methods;
- whether counterfactual validation should become a first\-class validation\-spec feature;
- how geological provenance/resources should be represented;
- persistence/event\-log design for long\-running games;
- historical map playback;
- civilization/NPC feedback into environmental simulation\.

No implementation agent may resolve these by unilateral architecture changes\.

---

## 40\. Design rationale summary

The system intentionally favors a **small compiler plus reusable numerical machinery** over a conventional configurable planet simulator\.

The desired layering is:

```text
conversation / world intent
        |
        v
YAML + CEL world specification
        |
        v
MUSE compiler
        |
        v
generic operator graph
        |
        +-------------------------+
        |                         |
        v                         v
established crates        MUSE-specific kernels
(mesh/math/graphs/        (only where the algebra
parallel/RNG/etc.)         genuinely requires them)
        |                         |
        +-------------+-----------+
                      |
                      v
                 WorldState
                      |
          +-----------+-----------+
          |                       |
          v                       v
   deterministic metrics     browser visualization
          |                       |
          +-----------+-----------+
                      |
                      v
               optimization / review
```

The engine is successful when world semantics primarily live in declarative dataflow and can be changed without recompiling Rust\.

The PoC should remain aggressively simple wherever simplicity does not reduce the expressiveness being tested\.

---

## Appendix A — Example compact world specification

The exact schema may evolve, but the following illustrates the intended style and separation of concerns\.

```yaml
version: 1

world:
  geometry:
    type: icosphere
    level: 5

parameters:
  plate_count:
    type: integer
    default: 12

  uplift_strength:
    type: scalar
    default: 1.0
    search: { min: 0.2, max: 3.0, mutation_sigma: 0.08 }

  sea_level:
    type: scalar
    default: 0.0
    search: { min: -0.5, max: 0.5, mutation_sigma: 0.05 }

  stellar_strength:
    type: scalar
    default: 1.0
    search: { min: 0.4, max: 1.6, mutation_sigma: 0.05 }

  mana_strength:
    type: scalar
    default: 1.0
    search: { min: 0.0, max: 3.0, mutation_sigma: 0.08 }

programs:
  generate:
    nodes:
      plates:
        op: voronoi_labels
        args:
          count: $param.plate_count

      boundary:
        op: boundary_strength
        args:
          labels: $node.plates

      uplift:
        op: diffuse
        args:
          field: $node.boundary
          rate: 0.18
          iterations: 12

      roughness:
        op: noise
        args:
          scale: 0.15

      elevation:
        op: pointwise
        inputs:
          uplift: $node.uplift
          roughness: $node.roughness
        params:
          uplift_strength: $param.uplift_strength
        expr: uplift * uplift_strength + roughness * 0.2

      slope:
        op: gradient
        args:
          field: $node.elevation

      land:
        op: pointwise
        inputs:
          elevation: $node.elevation
        params:
          sea_level: $param.sea_level
        expr: elevation > sea_level

      insolation:
        op: pointwise
        builtins:
          latitude: $geometry.latitude
        params:
          stellar_strength: $param.stellar_strength
        expr: stellar_strength * max(cos(latitude), 0.0)

      temperature:
        op: pointwise
        inputs:
          insolation: $node.insolation
          elevation: $node.elevation
        expr: insolation - 0.25 * max(elevation, 0.0)

      wind:
        op: vector_expr
        builtins:
          latitude: $geometry.latitude
          east: $geometry.east
        expr:
          x: east.x * cos(latitude * 3.0)
          y: east.y * cos(latitude * 3.0)
          z: east.z * cos(latitude * 3.0)

      moisture_seed:
        op: pointwise
        inputs:
          land: $node.land
        expr: land ? 0.1 : 1.0

      moisture:
        op: advect
        args:
          field: $node.moisture_seed
          velocity: $node.wind
          steps: 8

      elevation_gradient:
        op: gradient
        args:
          field: $node.elevation

      precipitation:
        op: pointwise
        inputs:
          moisture: $node.moisture
          wind: $node.wind
          elevation_gradient: $node.elevation_gradient
        expr: moisture * (0.1 + max(dot(wind, elevation_gradient), 0.0))

      runoff:
        op: pointwise
        inputs:
          precipitation: $node.precipitation
        expr: max(precipitation - 0.05, 0.0)

      flow_to:
        op: flow_direction
        args:
          potential: $node.elevation

      discharge:
        op: accumulate
        args:
          values: $node.runoff
          receivers: $node.flow_to

      fault_proximity:
        op: distance_to_mask
        args:
          mask: $node.boundary

      mana_source:
        op: pointwise
        inputs:
          boundary: $node.boundary
        params:
          mana_strength: $param.mana_strength
        expr: boundary * mana_strength

      mana:
        op: diffuse
        args:
          field: $node.mana_source
          rate: 0.12
          iterations: 10

      mana_heat:
        op: pointwise
        inputs:
          mana: $node.mana
        expr: mana * 0.08

      final_temperature:
        op: pointwise
        inputs:
          base: $node.temperature
          mana_heat: $node.mana_heat
        expr: base + mana_heat

    outputs:
      plate_id: $node.plates
      plate_boundary: $node.boundary
      elevation: $node.elevation
      slope: $node.slope
      land_mask: $node.land
      temperature: $node.final_temperature
      wind: $node.wind
      moisture: $node.moisture
      precipitation: $node.precipitation
      runoff: $node.runoff
      flow_to: $node.flow_to
      discharge: $node.discharge
      fault_proximity: $node.fault_proximity
      mana: $node.mana

  step:
    nodes:
      vegetation_effect:
        op: pointwise
        inputs:
          vegetation: $state.vegetation
        expr: clamp(vegetation, 0.0, 1.0)

      runoff_next:
        op: pointwise
        inputs:
          precipitation: $state.precipitation
          vegetation: $node.vegetation_effect
        expr: max(precipitation * (1.0 - 0.35 * vegetation), 0.0)

      discharge_next:
        op: accumulate
        args:
          values: $node.runoff_next
          receivers: $state.flow_to

    updates:
      runoff: $node.runoff_next
      discharge: $node.discharge_next
```

This example is illustrative, not a commitment that every operator signature shown above is final\.

---

## Appendix B — Example validation specification

```yaml
version: 1

metrics:
  land_fraction:
    op: fraction_above
    field: land_mask
    threshold: 0.5

  mountain_fraction:
    op: fraction_above
    field: slope
    threshold: 0.18

  mana_fault_correlation:
    op: correlation
    a: mana
    b: plate_boundary

  river_density:
    op: fraction_above
    field: discharge
    threshold: 0.70

constraints:
  enough_land:
    expr: metrics.land_fraction >= 0.15

  not_all_land:
    expr: metrics.land_fraction <= 0.85

objectives:
  mountain_target:
    expr: 1.0 - min(abs(metrics.mountain_fraction - 0.60) / 0.60, 1.0)
    weight: 1.0

  fault_aligned_mana:
    expr: clamp(metrics.mana_fault_correlation, 0.0, 1.0)
    weight: 0.7

  nondegenerate_rivers:
    expr: clamp(metrics.river_density / 0.05, 0.0, 1.0)
    weight: 0.3
```

---

## Appendix C — Standard implementation\-agent dependency policy

```text
DEPENDENCY POLICY

1. Use only dependencies present in the workspace allowlist.

2. Before implementing a generic algorithm or infrastructure,
   inspect the approved dependency list for an existing implementation.

3. You may not independently implement:
   - parsers
   - spherical subdivision
   - vector/matrix libraries
   - generic graph containers/topological sorting
   - generic thread pools/work stealing
   - generic optimization algorithms
   - RNG algorithms
   - procedural noise algorithms
   - benchmarking infrastructure
   - property/snapshot-testing infrastructure

4. If required functionality is missing, stop and create a dependency request.

5. Do not add a local abstraction over an approved crate unless the project
   contract requires a stable boundary or two concrete call sites demonstrate
   the need.

6. Do not add unsafe code.

7. Do not manually parallelize work that can be expressed through Rayon.

8. Do not optimize a path without benchmark/profile evidence.

9. Do not solve a local issue by changing project architecture.
```

---

## Appendix D — Standard work\-agent completion format

```markdown
# RESULT

## Files changed
- ...

## Commands run
- ...

## Tests
- PASS/FAIL: ...

## Acceptance criteria
- [x] ...
- [ ] ...

## Known deviations
- None / ...

## Blockers
- None / ...
```

---

## Appendix E — PoC decision record

The following decisions are intentional for the first integrated spike:

- fixed spherical substrate, not dynamically generated topology;
- approximately 10k cells, not production\-scale resolution;
- dense eager execution as correctness baseline;
- Rayon, not a hand\-rolled scheduler;
- Timely/Differential only through a bounded evidence\-based spike;
- YAML \+ CEL, not a custom language;
- domain recipes as data, not Rust;
- browser debug viewer, not production cartography;
- Divan for fast microbenchmark feedback;
- `perf` \+ Coz for profiling;
- Nix \+ Crane for reproducible environment/build boundaries;
- aggressive use of established maintained dependencies;
- one\-to\-two\-day go/no\-go target;
- kill the project if the algebra/composition/steerability/causality hypotheses are not convincing\.
