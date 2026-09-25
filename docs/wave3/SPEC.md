# Wave 3 causal world-model specification

Status: Proposed acceptance contract  
Date: 2026-09-24  
Scope: Top-level world-model architecture, target models, validation protocol, viewer acceptance, and implementation decomposition.

## 1. Executive decision

Wave 3 MUST establish MUSE as a **causal field-composition system**, not merely a procedural terrain generator.

The governing model is:

    exogenous forcing fields
        -> planetary/state-like fields
        -> directional transport and interactions
        -> derived diagnostics
        -> observable world

Causality in Wave 3 is demonstrated through **controlled interventions and ablations**. A target model does not pass merely because fields correlate or a globe looks plausible. When a declared cause is changed while the relevant upstream inputs are held fixed, the downstream fields MUST change in the expected direction and locality.

Wave 3 MUST preserve the project maxim:

> Rust implements the algebra. YAML + CEL implements the world.

Generic spatial, vector, graph, transport, validation, and rendering capabilities belong in reusable Rust algebra. Plate tectonics, climate, localized disturbance, vegetation, biome, and external forcing semantics MUST remain declarative compositions unless a capability is demonstrably generic beyond the target recipe.

Wave 3 does **not** model upstream celestial bodies or orbital mechanics. External influences are represented only through their effects on the planet: for example radiative/insolation fields, tidal or directional forcing fields, rotation/circulation proxies, or localized spatial disturbances. Multiple upstream influences are represented by multiple declarative fields and ordinary field composition.

Wave 3 MUST retain the accepted Wave 1 execution model: whole-field dense values, deterministic topological execution, and approved data parallelism. It MUST NOT introduce Timely/Differential, a custom incremental engine, or dirty-region scheduling merely to satisfy this wave.

Explicit temporal state stepping is deferred. Wave 3 causal acceptance is intervention-based over static/eager evaluations of the existing program model. Time evolution MAY be revisited after the static causal contracts are demonstrated.

## 2. Relationship to prior waves

### 2.1 Wave 1 remains authoritative for execution

The accepted Wave 1 execution ADR remains in force. Wave 3 programs MUST use the existing whole-field Program/operator boundary and deterministic topological execution. Localized interventions MAY require reevaluating complete dependent fields.

Wave 3 MUST benchmark before introducing execution-model complexity.

### 2.2 Wave 2 remains authoritative for accepted terrain primitives

The Wave 2 terrain synthesis ADR remains a valid, narrower decision about terrain primitives. Its component algebra, spherical geometry metrics, region scalar attributes, correlated fields, smoothing semantics, and related validator work remain useful Wave 3 building blocks.

Wave 3 supersedes Wave 2 only as the **top-level world-model acceptance architecture**. It does not invalidate useful Wave 2 primitives.

Wave 2 rejected the experimental region-associated tangent vector from the initial production terrain algebra because no terrain acceptance case justified unresolved frame, transport, or seam semantics. Wave 3 introduces concrete directional use cases—plate motion, wind, transport, gradients, and flux—so vector semantics MUST now be designed explicitly. Wave 3 MUST NOT simply revive the earlier experimental contract without specifying those semantics.

## 3. Architectural model

### 3.1 Field-first causality

A causal relationship MUST be represented by named typed field dependencies wherever practical.

Examples:

    radiative_forcing -> temperature
    plate_velocity -> boundary_convergence -> tectonic_forcing -> elevation
    elevation -> elevation_gradient -> runoff direction
    wind + moisture -> moisture transport -> precipitation
    temperature + moisture -> vegetation suitability -> biome

A domain-specific Rust object such as a Plate, ClimateSystem, AsteroidImpact, BiomeSystem, Star, or Moon MUST NOT be introduced when the same behavior can be expressed through generic typed operators and declarative recipes.

### 3.2 Exogenous forcing

An **exogenous forcing** is a field or recipe parameter that represents an imposed cause whose upstream source is outside the modeled planet.

Examples MAY include:

- radiative or insolation fields;
- tidal or other scalar/vector forcing fields;
- rotation/circulation proxies;
- localized scalar or vector disturbances;
- user-specified environmental gradients.

The algebra SHOULD prefer ordinary ScalarField and VectorField composition. Wave 3 MUST NOT add an ExternalForcing runtime type unless concrete evidence shows that ordinary field composition cannot express a required target-model contract.

A recipe MAY compose any number of forcing fields. The algebra MUST NOT assume one sun, one moon, one source direction, Earth-like latitude forcing, or any other fixed upstream source model.

### 3.3 Conceptual field classes

Wave 3 uses the following conceptual classes. They do not by themselves require new runtime types.

| Class | Examples | Typical field type |
|---|---|---|
| State-like / primary | elevation, temperature, moisture, plate labels | ScalarField, CategoryField |
| Directional / transport | plate velocity, wind, flux | VectorField |
| Derived diagnostics | slope, gradients, convergence, shear | ScalarField or VectorField |
| Selection / classification | land, snow, masks, biome | BoolField, CategoryField |
| Connectivity / routing | flow target, river network | IndexField, Network |

Derived diagnostics SHOULD be exportable into snapshots even when they are not persistent state. A validation or debugging workflow MUST be able to inspect the intermediate field that establishes a claimed causal relationship.

### 3.4 Vector semantics

When direction matters, a VectorField MUST be used rather than a scalar proxy.

For tangent quantities on the sphere:

- values MUST be finite;
- the contract MUST state whether inputs are intrinsically tangent or projected tangent;
- tangent outputs MUST be approximately orthogonal to the surface normal under a frozen tolerance;
- any projection, interpolation, transport, boundary comparison, or seam behavior MUST be explicit;
- deterministic operators MUST be invariant to neighbor ordering and thread scheduling within the accepted reproducibility contract.

Vector visualization is part of acceptance, not merely debugging infrastructure.

### 3.5 Static causal interventions

Wave 3 does not require a state transition of the form state(t + dt). Instead, causality is tested through paired or grouped evaluations.

For a control configuration C and intervention I:

    delta(field) = field(I) - field(C)

Target validation MUST specify which upstream variables differ, which variables are held fixed, which downstream deltas are expected, and where those deltas are expected spatially.

A causal test MUST NOT claim success solely from an observational correlation when an intervention is available.

## 4. Canonical target models

Wave 3 defines exactly four canonical target models. These targets are acceptance scenarios, not reusable Rust subsystems.

### 4.1 Target A — Temperate terrestrial integration

**Purpose:** prove that generic algebra composes into an understandable end-to-end terrestrial world.

Minimum causal chain:

    regional / plate structure
        -> tectonic forcing
        -> elevation
        -> exogenous insolation + elevation lapse effect
        -> temperature
        -> wind + moisture transport
        -> precipitation
        -> runoff / discharge
        -> vegetation / biome

#### Required inspectable outputs

The canonical recipe MUST expose at least:

- plate_id or equivalent coherent region labels;
- plate_velocity;
- boundary_convergence;
- elevation;
- insolation or total radiative forcing;
- temperature;
- wind;
- moisture;
- precipitation;
- runoff;
- discharge;
- vegetation or vegetation suitability;
- biome.

It SHOULD additionally expose elevation_gradient, temperature_gradient, slope, moisture flux, shear, land/ocean masks, and river network fields where available.

#### Baseline validation

The target MUST demonstrate:

- plausible and nondegenerate spherical land/ocean geometry using area, connected-component, and perimeter metrics;
- non-flat elevation;
- temperature response with the expected sign to insolation/radiative forcing;
- a negative elevation effect on temperature when broad forcing is controlled or otherwise conditioned;
- finite tangent wind;
- nondegenerate moisture and precipitation;
- coherent drainage routing;
- discharge that accumulates downstream under the frozen routing convention;
- spatially coherent vegetation/biome output driven by climate suitability rather than independent cell noise.

Simple unconditional correlation MUST NOT substitute for a conditional relationship when elevation, latitude/forcing, or another confounder materially changes the interpretation.

#### Mandatory perturbations

At minimum:

1. **Lapse perturbation:** increasing the configured elevation-temperature coupling MUST strengthen the conditional altitude/temperature effect with the expected sign.
2. **Wind ablation:** setting the wind/transport contribution to zero MUST materially change the moisture and/or precipitation distribution while leaving unrelated upstream terrain inputs fixed.
3. **Tectonic ablation:** setting tectonic forcing to zero MUST reduce boundary-associated relief under a frozen metric while preserving the same region labels.

Numeric effect-size thresholds MUST be calibrated and frozen before holdout evaluation.

#### Visual acceptance

The consolidated surface MUST show coherent land/ocean, terrain relief, biome/material differentiation, and river structure. Relevant causal overlays MUST make plate velocity, boundary convergence, wind, and at least one climate-gradient or transport diagnostic inspectable.

This target demonstrates compositional causality; it does not claim calibrated Earth realism.

### 4.2 Target B — Tectonically active world

**Purpose:** prove that MUSE can represent directional plate motion and its consequences rather than merely coloring plate-shaped regions.

#### Required causal structure

The recipe MUST include:

- coherent plate/region labels as a CategoryField;
- plate_velocity as a VectorField;
- generic relative-vector comparison across category boundaries;
- signed convergence/divergence as a ScalarField;
- tangential/shear interaction as a ScalarField or other explicitly justified generic diagnostic;
- declarative conversion of those diagnostics into tectonic forcing;
- elevation derived in part from that forcing.

Plate motion MUST NOT be represented only as a per-region scalar.

#### Baseline validation

The target MUST demonstrate:

- plate velocity is finite and tangent under the frozen vector contract;
- boundary interaction is approximately zero away from boundaries;
- stronger positive normal convergence is associated with stronger positive uplift/relief under the recipe convention;
- divergence produces the configured lowering/rift tendency;
- stronger tangential relative motion produces a stronger shear diagnostic;
- boundary diagnostics are deterministic under adjacency order and thread scheduling;
- not every boundary is forced into the same ridge morphology.

The precise relationship MAY be measured through conditional means, regression, binned response, or another preregistered statistic. The test form MUST be frozen before final calibration.

#### Mandatory perturbations

1. **Zero-motion ablation:** plate_velocity = 0 MUST collapse convergence/divergence and shear diagnostics while leaving plate labels unchanged.
2. **Velocity reversal:** multiplying all plate velocities by -1 MUST reverse convergence/divergence signs under the defined orientation while preserving boundary locations.
3. **Magnitude response:** if the recipe claims monotonic forcing with velocity magnitude, a preregistered scale perturbation MUST verify that claim.

#### Visual acceptance

The causal overlay MUST show plate velocity vectors, boundaries, convergence/divergence, and shear. The surface view MUST show that uplift/rift structure follows the appropriate interaction classes rather than every boundary indiscriminately.

No TectonicsSystem or Plate runtime class is permitted solely to satisfy this target.

### 4.3 Target C — Asymmetric external-forcing climate

**Purpose:** prove that MUSE composes multiple exogenous forcing fields and propagates their effects through climate without modeling the upstream celestial bodies or assuming one Earth-like forcing source.

#### Required forcing configurations

The canonical target MUST define two independent radiative forcing components, A and B, with distinct spatial structure and evaluate at least:

- A only;
- B only;
- A + B;
- A + 2B.

The immediate forcing composition MUST match the declarative field algebra exactly. Downstream climate response is not required to be linear.

The target MUST NOT require Star, Moon, Orbit, or celestial-body runtime entities.

#### Required downstream fields

At minimum:

- total radiative/insolation forcing;
- temperature;
- temperature_gradient or an equivalent inspectable derivative;
- wind;
- moisture;
- precipitation;
- vegetation suitability and/or biome.

#### Baseline validation

The target MUST demonstrate:

- stronger local radiative forcing generally raises local temperature relative to an appropriate control;
- the temperature-gradient structure changes when forcing geometry changes;
- wind and moisture transport respond to the altered thermal structure under the chosen recipe;
- precipitation changes downstream;
- vegetation/biome output shifts consistently with changed climate suitability.

The validator MUST distinguish primary local response from secondary transport effects. Regions where the forcing field changes strongly SHOULD exhibit a stronger direct temperature response than regions where forcing is unchanged, subject to a preregistered spatial-response metric.

#### Mandatory perturbations

1. **Component removal:** removing B from A + B MUST reproduce the A-only result deterministically.
2. **Component scaling:** A + 2B MUST produce exactly the configured forcing delta before downstream climate transforms.
3. **Spatial response:** downstream temperature delta MUST have the expected sign and stronger direct response in the materially forced region under the frozen statistic.

This target proves composable exogenous forcing. It does not simulate the bodies that caused those forcings.

### 4.4 Target D — Localized disturbance world

**Purpose:** prove that a generic localized exogenous perturbation can affect multiple planetary subsystems without domain-specific disturbance code.

The target MUST use generic spatial field construction. It MUST NOT require an Asteroid, Volcano, Explosion, Crater, or similar Rust type.

Possible declarative disturbance components include:

- elevation disturbance;
- temperature disturbance;
- vegetation/suitability disturbance;
- optional directional forcing where generic vector primitives justify it.

#### Baseline validation

The target MUST demonstrate:

- strongest direct response occurs in or near the configured forcing footprint;
- configured spatial decay behaves monotonically or according to the explicitly chosen footprint contract;
- distant unforced regions remain substantially closer to the control world than the directly forced region under a frozen error metric;
- changed elevation produces a measurable local drainage/runoff response;
- changed temperature and/or vegetation forcing produces the expected local suitability/biome response;
- outputs remain finite and deterministic.

#### Mandatory perturbations

1. **Removal:** removing the disturbance MUST reproduce the deterministic baseline.
2. **Magnitude scaling:** if the footprint amplitude is scaled, directly affected fields MUST respond monotonically where the recipe claims monotonicity.
3. **Locality:** the preregistered affected mask MUST show larger downstream delta than the preregistered distant-control mask, except for explicitly modeled transport pathways.

Wave 3 acceptance for this target is static. Recovery, propagation through time, and persistent damage state are future work.

## 5. Common validation protocol

Every canonical target MUST define validation in five layers.

### 5.1 Structural validation

Structural checks MUST include, as applicable:

- correct field type;
- one value per mesh cell for whole-field outputs;
- finite numeric values;
- valid category/index references;
- mesh validity;
- tangent-vector invariants;
- deterministic duplicate generation;
- deterministic operator behavior under supported thread scheduling.

A structurally invalid model MUST fail before statistical or visual acceptance.

### 5.2 Statistical and geometric validation

Targets MUST use metrics appropriate to their fields, including where applicable:

- mean, variance, and quantiles;
- spherical area and area fraction;
- connected-component area distributions;
- perimeter and perimeter/area metrics;
- nondegeneracy checks;
- network connectivity and routing structure;
- vector magnitude distributions.

Spherical geometry MUST use the shared mesh/component conventions established by prior waves rather than cell-count approximations where area matters.

### 5.3 Relational validation

Relational checks MUST specify the expected direction and conditioning.

Examples:

- temperature increases with radiative forcing;
- at comparable forcing, temperature decreases with elevation;
- convergent boundaries produce greater uplift response than neutral/divergent boundaries;
- wetter suitable regions support greater vegetation response.

When a simple correlation is materially confounded, the validation MUST use a conditional comparison, residualized relationship, stratification, multivariate fit, matched bins, or another preregistered approach.

Relational evidence alone does not satisfy a causal requirement that has a defined intervention.

### 5.4 Perturbational and ablation validation

Every major causal subsystem MUST expose at least one intervention.

Each perturbation specification MUST freeze:

- control configuration;
- intervention configuration;
- fields/parameters changed;
- fields/parameters required to remain identical;
- downstream field(s) expected to change;
- sign, locality, or monotonicity of expected response;
- statistic used to measure the response;
- calibration rule for the final numeric threshold.

The validation harness SHOULD support comparison of paired worlds and derived delta fields directly. Recipe authors MUST NOT implement bespoke Rust solely to compare intervention outputs.

### 5.5 Visual validation

Visual review is mandatory but cannot override a failed machine constraint.

For each target, acceptance captures MUST include:

- consolidated surface view;
- the most relevant causal vector overlay(s);
- the most relevant signed scalar or diagnostic overlay(s);
- raw field inspection sufficient to reconcile the rendered surface with the underlying data.

A visual reviewer SHOULD be able to explain a major surface feature by moving from observable surface -> causal overlay -> raw field.

## 6. Calibration, preregistration, and holdout discipline

Wave 3 MUST separate test-form design from numeric threshold selection.

Before calibration, each target MUST preregister:

- recipe/configuration variants;
- control and intervention pairs;
- calibration seeds;
- at least one distinct holdout seed set;
- mesh levels used for scale checks;
- validation metric definitions;
- sign/locality/monotonicity expectations;
- known negative controls where relevant.

Calibration seeds MAY be used to choose numeric acceptance intervals and effect-size thresholds.

After thresholds are frozen:

- holdout seeds MUST be evaluated without weakening thresholds;
- failures MUST be investigated as model or contract failures, not repaired by silently tuning the discriminator on holdout outputs;
- a threshold change MUST be versioned with the recipe/validator configuration and requires recalibration;
- final acceptance artifacts MUST record recipe version, validator version, seed set, mesh level, and intervention configuration.

The same seed and unchanged upstream inputs MUST be used for paired causal comparisons unless the target explicitly defines another pairing rule.

## 7. Cross-model invariants

All four targets MUST satisfy:

- identical inputs produce deterministic duplicate output under the accepted reproducibility contract;
- no NaN or Inf values;
- whole-field lengths match the mesh;
- tangent vectors are finite and approximately orthogonal to the surface normal under a frozen tolerance;
- deterministic operators are invariant to supported thread scheduling and canonical neighbor ordering;
- canonical level 5 uses 10,242 positions;
- at least one lower-resolution evaluation checks scale/resolution consistency;
- generation runtime is bounded and measured;
- expensive generic operators have observable benchmarks;
- target recipes introduce no recipe-local or domain-specific Rust;
- every major causal subsystem has an ablation;
- causal intermediate fields can be exported to snapshots for debugging and acceptance review.

Cross-resolution acceptance MUST compare scale-aware metrics rather than requiring cellwise identity across different meshes.

## 8. Required generic algebra capability families

Wave 3 specifies capability families, not premature Rust signatures. Before adding an operator, an implementation ticket MUST check whether existing generic algebra already satisfies the target acceptance tests.

### 8.1 Region-associated vectors

MUSE requires a generic way to derive deterministic vector attributes from category/region labels.

The production contract MUST define:

- how a region-associated direction is keyed;
- whether the source direction is represented in ambient 3D or another frame;
- how the vector is projected to the local tangent plane;
- behavior near projection degeneracy;
- magnitude semantics;
- determinism across cells, scheduling, and adjacency order;
- whether interpolation/transport between cells is required.

The contract MUST demonstrate at least two unrelated reuse domains beyond tectonic plate velocity before being considered sufficiently generic.

### 8.2 Tangent and vector pointwise algebra

Existing tangent projection, vector magnitude, vector dot, vector expression, and pointwise capabilities MUST be reused where adequate.

Wave 3 MUST support declarative vector-scalar composition sufficient to express flux-like fields such as:

    moisture_flux = wind * moisture

No dedicated MoistureFlux operator is justified if generic algebra can express it.

### 8.3 Relative vector interaction across category boundaries

Wave 3 requires generic boundary comparison of neighboring directional fields.

The capability MUST be able to derive, with explicit sign/orientation rules:

- normal relative component for convergence/divergence;
- tangential relative component for shear.

The contract MUST define:

- canonical cross-boundary edge orientation;
- tangent-plane boundary direction or normal construction;
- how vectors at the two incident cells are compared;
- edge weighting;
- multi-edge aggregation;
- equal-label and off-boundary output;
- malformed adjacency behavior;
- deterministic reduction order.

The Rust API MUST NOT use tectonic domain terminology if the same operator can describe region-relative vector interaction in another domain.

### 8.4 Gradients and differential diagnostics

Existing gradient behavior SHOULD be reused.

Gradient outputs used in validation MUST be inspectable as VectorField values.

A divergence operator MAY be added only if a canonical target acceptance test cannot be expressed cleanly with existing primitives. Wave 3 MUST NOT add differential operators merely for mathematical completeness.

### 8.5 Advection and transport

Existing advection/transport semantics MUST be evaluated against the climate target before new transport machinery is added.

Acceptance MUST answer:

- whether the operator respects tangent flow semantics;
- whether transport direction/sign is unambiguous;
- whether boundary/neighbor selection is deterministic;
- whether repeated composition produces sufficiently broad climate structure;
- whether performance is acceptable at level 5.

If inadequate, the replacement MUST remain a generic transport operator rather than a climate subsystem.

### 8.6 Generic forcing composition

Ordinary field algebra SHOULD represent multiple forcing components.

The target design SHOULD use:

    total_forcing = forcing_a + forcing_b + ...

rather than introducing an external-source registry or celestial object model.

A specialized forcing type requires separate architecture evidence.

### 8.7 Localized spatial footprints

Localized disturbance recipes SHOULD be composed from existing generic masks, distance-to-mask, smooth/correlated fields, pointwise expressions, and geometry inputs where possible.

A new footprint primitive is justified only if the target cannot express its preregistered locality/decay contract with the existing generic algebra.

### 8.8 Hydrology and networks

Existing flow direction, accumulation, threshold/network, and related primitives MUST be evaluated before adding new hydrology code.

Hydrology Rust MUST remain generic graph/field algebra. River naming, biome semantics, and terrestrial recipe logic belong in YAML/CEL.

### 8.9 Vegetation and biome

Vegetation and biome MUST remain recipe/data-driven.

Generic thresholding, scoring, categorical classification, and expression composition MAY be extended if necessary, but Wave 3 MUST NOT introduce a hard-coded Rust biome system.

## 9. Viewer and acceptance artifact contract

The Wave 3 viewer is both a human acceptance surface and a causal debugging tool. It MUST provide three complementary modes.

### 9.1 Consolidated Surface View

This is the default acceptance presentation.

It MUST support, when corresponding fields exist:

- elevation displacement, hillshade, normal-based relief, or another clearly legible terrain treatment;
- land/ocean surface differentiation;
- biome/material/albedo mapping;
- snow/ice visualization;
- river/network overlay;
- optional cloud or atmospheric overlay only when actual data fields exist.

Lighting is presentation metadata. It MUST NOT imply that MUSE simulated a celestial body merely because the surface is lit.

The consolidated view MUST use multiple world fields simultaneously. It MUST NOT reduce the planet to one arbitrary scalar palette.

### 9.2 Causal Overlay View

The viewer MUST be able to overlay relevant causal diagnostics, including when produced:

- plate velocity arrows;
- plate/region boundaries;
- convergence/divergence;
- shear;
- wind;
- temperature gradient;
- moisture flux;
- river networks;
- precipitation or other scalar heatmaps/contours.

Vector overlays MUST document whether arrow length represents magnitude or is normalized for direction-only display.

### 9.3 Raw Field Inspector

The existing generic debugging capabilities remain required:

- scalar fields;
- Bool masks;
- categorical/index fields;
- vector fields;
- networks;
- exact field type;
- min/max or appropriate range;
- cell/face inspection.

Palette legends MUST be semantic:

- signed scalar: zero-centered diverging palette with negative/zero/positive meaning;
- sequential scalar: low/high meaning;
- categorical/index: colors have no ordinal meaning unless explicitly declared;
- Bool: explicit true/false labels.

A click/hover inspector SHOULD show a useful cross-section of causal state for the selected location, not merely the active display field.

The consolidated surface view MUST NOT replace raw field inspection.

## 10. Performance and determinism

Wave 3 retains the existing performance philosophy:

- benchmark representative whole recipes and expensive generic operators;
- use existing Nix development and Cargo quality gates;
- use Divan for baseline micro/whole-path benchmarks where applicable;
- use Coz or another already-approved profiling approach to identify causal bottlenecks before optimization;
- prefer established composable crates over custom computational frameworks when a generic implementation gap is demonstrated;
- do not adopt a new execution framework merely because intervention evaluations recompute fields.

At level 5, the canonical workload remains 10,242 positions.

Each implementation phase MUST report:

- deterministic duplicate evidence;
- relevant operator/recipe wall time;
- cold/warm distinctions when mesh calibration or caches apply;
- any allocations or passes that dominate runtime;
- cross-resolution behavior where scale semantics are claimed.

No Wave 3 target may pass by relaxing determinism or validation to hide a performance problem.

## 11. Non-goals and deferred work

Wave 3 explicitly does **not** require:

- explicit celestial body representation;
- orbital mechanics;
- upstream astrophysical simulation;
- a full atmospheric or ocean PDE solver;
- explicit time stepping or persistent state transitions;
- geological time evolution;
- physically calibrated SI-unit Earth simulation;
- custom incremental computation;
- domain-specific Rust Plate, Climate, Asteroid, Volcano, Star, Moon, River, Vegetation, or Biome systems;
- photorealistic rendering;
- production art assets;
- causality claims based only on observational correlation.

### 11.1 Revisit criterion for time

Time/state transition MAY become a later architecture wave when a target explicitly requires propagation, recovery, hysteresis, or persistent state that cannot be represented as a static intervention comparison.

A future time proposal MUST define the state boundary, integration semantics, determinism expectations, and representative performance workload before changing the accepted execution model.

## 12. Implementation decomposition

Wave 3 SHOULD be executed through bounded parallel work with independent verification.

### Phase 0 — contract freeze and validation harness design

Ownership:

- coordinator: shared typed contracts, shared operator descriptor changes, validation comparison contract;
- validation owner: paired-world/delta evaluation design;
- no target recipe implementation until the required shared contracts are frozen.

Deliverables:

- vector/frame semantics;
- intervention/paired-world validation schema;
- target-model fixture layout;
- preregistration and holdout format;
- agreed snapshot/export requirements.

Shared registries and root dependency policy remain coordinator-owned.

### Phase 1 — generic capability acquisition, parallel

#### Track 1: vector/frame algebra

Owns deterministic region-associated vectors, tangent projection semantics, vector-scalar composition gaps, and direct unit/property tests.

Must prove generic reuse beyond tectonics.

#### Track 2: boundary vector interaction

Owns canonical cross-category relative normal/tangential decomposition and deterministic aggregation.

Depends on frozen vector/frame semantics but SHOULD otherwise remain independent.

#### Track 3: perturbation validation harness

Owns paired control/intervention execution or comparison, delta metrics, masked effect metrics, and result reporting.

Must not encode target-specific physics in the validator.

#### Track 4: consolidated viewer groundwork

Owns multi-field surface composition, semantic legends, vector-overlay semantics, and richer inspection.

Must remain compatible with generic snapshots and raw-field debugging.

These tracks SHOULD minimize source overlap so independent agents can work concurrently.

### Phase 2 — canonical target recipes, parallel after contracts

#### Track A: tectonically active target

Owns declarative plate-region/motion recipe, tectonic interventions, target-specific validation fixture, and acceptance snapshots.

No Rust unless a previously approved generic capability ticket blocks the recipe.

#### Track B: asymmetric external-forcing climate target

Owns forcing A/B recipes, climate composition, perturbation matrix, validation fixture, and causal overlays.

Must not model celestial bodies.

#### Track C: localized disturbance target

Owns generic footprint composition, control/intervention recipes, locality validation, and acceptance snapshots.

Must not add disturbance-domain Rust.

#### Track D: temperate terrestrial integration

Owns integration across accepted terrain, climate, hydrology, vegetation/biome composition and the required ablations.

It MAY consume outputs of the more focused target tracks but MUST NOT duplicate their generic Rust.

### Phase 3 — calibration, holdout, and final acceptance

Owns:

- preregistered multi-seed corpus execution;
- numeric threshold calibration;
- holdout execution after freeze;
- cross-resolution checks;
- consolidated surface captures;
- causal overlay captures;
- cross-model performance report;
- final Wave 3 acceptance report.

Thresholds and recipes MUST be versioned together.

### 12.1 Blocker protocol

If a recipe agent cannot express a required behavior with accepted algebra, it MUST stop and report:

1. the missing generic capability;
2. the smallest proposed type/signature semantics;
3. at least two unrelated reuse domains where applicable;
4. a machine-testable acceptance contract;
5. why existing operators cannot compose to satisfy the requirement.

The agent MUST NOT solve the blocker with target-specific Rust.

A shared-capability change MUST receive an independent verifier before target work resumes.

## 13. Risks and open questions

1. **Vector frame semantics.** Region-associated directional attributes on a sphere require a contract that remains understandable across cells and boundaries. Wave 3 MUST settle this before tectonic acceptance.
2. **Boundary geometry.** A canonical normal/tangent definition must remain deterministic on the discrete spherical mesh and avoid orientation ambiguity.
3. **Transport adequacy.** The current advection primitive may be too local or simplistic for broad climate structure. It must be measured against Target C before replacement.
4. **Conditional validation.** Simple correlations can be misleading when forcings covary. The validation harness may need generic stratified or multivariate metrics.
5. **Biome representation.** A sufficiently generic declarative classifier may require better table/category composition, but that must not become a hard-coded biome engine.
6. **Surface rendering semantics.** The viewer needs a stable mapping from semantic fields to surface layers without baking one target recipe into viewer code.
7. **Calibration overfitting.** Multi-seed calibration without holdout discipline can reproduce the Wave 2 risk of tuning toward a handful of attractive worlds.
8. **Static causality limits.** Intervention comparisons prove directional dependency in the program graph, not temporal physical causation. The documentation and UI MUST avoid implying more.
9. **Resolution sensitivity.** Vector and transport behavior may change more substantially across mesh levels than scalar smoothing. Cross-resolution metrics must be designed before claiming scale robustness.
10. **Performance multiplication.** Perturbational acceptance requires several full evaluations per seed. The system should optimize generic kernels, not weaken the intervention protocol.

## 14. Wave 3 exit checklist

Wave 3 is complete only when all items below are satisfied.

### Architecture

- [ ] The Wave 3 field/vector/intervention contracts are frozen and documented.
- [ ] Accepted Wave 1 whole-field execution remains intact.
- [ ] Useful Wave 2 terrain primitives remain reusable without being mistaken for the full world model.
- [ ] No canonical target required domain-specific recipe-local Rust.
- [ ] No canonical target required external celestial-body or orbital simulation.

### Target models

- [ ] Target A: temperate terrestrial integration exists as a declarative recipe/configuration.
- [ ] Target B: tectonically active world exists as a declarative recipe/configuration.
- [ ] Target C: asymmetric external-forcing climate exists as a declarative recipe/configuration.
- [ ] Target D: localized disturbance world exists as a declarative recipe/configuration.
- [ ] Required causal intermediates are exportable and inspectable.

### Validation

- [ ] Every target passes structural validation.
- [ ] Every target has statistical/geometric acceptance metrics.
- [ ] Every target has preregistered relational tests.
- [ ] Every major subsystem has at least one machine-executable intervention/ablation.
- [ ] Calibration seeds and configurations were preregistered.
- [ ] Numeric thresholds were frozen before holdout evaluation.
- [ ] Holdout seeds pass without discriminator weakening.
- [ ] Deterministic negative/ablation controls behave as specified.
- [ ] At least one lower-resolution comparison supports claimed scale semantics.

### Viewer

- [ ] Consolidated Surface View exists and combines multiple semantic world fields.
- [ ] Causal Overlay View exposes the target-relevant vector and signed scalar diagnostics.
- [ ] Raw Field Inspector remains available.
- [ ] Palette legends communicate signed, sequential, categorical, and Bool semantics.
- [ ] Vector overlays communicate normalized-vs-magnitude arrow semantics.
- [ ] Cell/location inspection exposes relevant causal state.
- [ ] Acceptance captures exist for each target.

### Performance and reproducibility

- [ ] Canonical level-5 runs use 10,242 positions.
- [ ] Duplicate generation meets the accepted deterministic reproducibility contract.
- [ ] No output contains NaN or Inf.
- [ ] Whole-recipe and expensive-operator timings are recorded.
- [ ] No intervention requires a custom incremental execution framework.
- [ ] Any intentionally deferred capability is recorded with evidence and a revisit criterion.
