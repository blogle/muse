# Wave 3 ownership

`CAUSAL-WORLD-MODEL.md` is normative. Shared types/descriptors/compiler schemas are frozen and coordinator-owned. Workers own only their assigned files/directories below; do not edit another row's ownership path or a shared registration point.

| Workstream | Exclusive ownership |
| --- | --- |
| Geometry transport | `crates/muse-geom/src/wave3/transport.rs` |
| Geometry reductions | `crates/muse-geom/src/wave3/reductions.rs` |
| Geometry calibration | `crates/muse-geom/src/wave3/calibration.rs` |
| Region vector | `crates/muse-ops/src/wave3/region_vector.rs` |
| Boundary normal | `crates/muse-ops/src/wave3/boundary_normal.rs` |
| Boundary tangential/shear | `crates/muse-ops/src/wave3/boundary_tangential.rs` |
| Divergence | `crates/muse-ops/src/wave3/divergence.rs` |
| Vector algebra | `crates/muse-ops/src/wave3/vector_algebra.rs` |
| Equilibrium diagnostics/iteration | `crates/muse-ops/src/wave3/equilibrium.rs` |
| Paired scalar comparisons | `crates/muse-validate/src/wave3/comparison.rs` |
| Seed/resolution corpus | `crates/muse-validate/src/wave3/corpus.rs` |
| Relational causal metrics | `crates/muse-validate/src/wave3/relational.rs` |
| Intervention pairing/identity | `crates/muse-validate/src/wave3/intervention.rs` |
| Render channel UI | `apps/viewer/src/wave3/renderChannels.ts` |
| Raw inspector/provenance UI | `apps/viewer/src/wave3/rawInspector.ts` |
| Causal overlay UI | `apps/viewer/src/wave3/causalOverlays.ts` |
| Viewer E2E tests | `apps/viewer/tests/wave3/` |
| Target A recipes | `specs/wave3/target-a-temperate-terrestrial/` |
| Target B recipes | `specs/wave3/target-b-tectonically-active/` |
| Target C recipes | `specs/wave3/target-c-composite-external-forcing/` |
| Target D recipes | `specs/wave3/target-d-localized-disturbance/` |
| Shared controls | `specs/wave3/controls/` |

The `mod.rs`, crate roots, and viewer `src/main.ts` are already wired for this fan-out. No follow-up implementation worker should need to modify those shared registration files.
