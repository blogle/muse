# Verification

## Verdict

FAIL

## Candidate

- Branch: `wave1/viewer`
- Candidate implementation commit: `5100b1c7e275d11c848a059d21eb2f168a3935d2`
- Frozen contract base: `b1f144b2fd7e967c98d10a44c8ed0ff1ec95716f`
- Tracker reference: MUSE-12 (not accessed or updated)

## Ownership/dependency review

- `git diff --name-status b1f144b2fd7e967c98d10a44c8ed0ff1ec95716f..5100b1c7e275d11c848a059d21eb2f168a3935d2` lists only additions/modifications under `apps/viewer/**`.
- No Rust crates, root Cargo/Nix files, fixtures, shared contract/schema files, or backend code changed in the candidate diff.
- Frontend runtime dependency is Three.js. Development/test tooling is TypeScript, Vite, and Playwright (`@playwright/test` pinned to 1.63.0). No frontend framework, backend, WASM, WebGPU, MapLibre, or tile system was introduced.
- The candidate's `scripts/run-playwright.mjs` validates standard `PLAYWRIGHT_BROWSERS_PATH` and `FONTCONFIG_FILE` variables without scanning `/nix/store`.
- Fetched current `master` as `FETCH_HEAD` for compatibility inspection only; its `flake.nix` exports `PLAYWRIGHT_BROWSERS_PATH` from `playwright-driver.browsers` and `FONTCONFIG_FILE` from the pinned fonts configuration. No branch was merged.

## Generic semantics review

- `src/main.ts` fetches committed fixture JSON directly from the static fixture directory; it uses no Rust/live-server backend protocol.
- Snapshot field choices are derived from each snapshot's `Scalar` and `Vector` payloads, and network choices from the generic networks map. Positions and triangle indices drive mesh construction; field values drive scalar colors; vector payloads drive projected arrows; network edge pairs drive line segments.
- No domain-specific rendering branches for elevation, precipitation, wind, rivers, or other domain meanings were found. Domain names appear in the smoke test to choose representative fixture data.
- The data schema is consumed generically and was not redesigned.

## Commands executed

In `apps/viewer`, inside `nix develop`, with explicit paths evaluated from the candidate's pinned Nix inputs and exported in the verifier shell:

```text
PLAYWRIGHT_BROWSERS_PATH=/nix/store/3av8irm8x5vvrqhkbp5l22dpc5m8fsg9-playwright-browsers
FONTCONFIG_FILE=/nix/store/vf2512adbirk4l16r8byxws78yi1lxrx-fonts.conf
pnpm install --frozen-lockfile       PASS
pnpm typecheck                       PASS
pnpm build                           PASS (Vite emitted the >500 kB chunk advisory)
pnpm test                            PASS (1 Playwright test)
git diff --check                     PASS
```

Nix path evaluation/build commands from `apps/viewer/RESULT.md` were independently run and returned the paths above. The browser test reported one passing test and no collected browser console errors, page errors, or failed requests. Build/test regenerated no tracked screenshot changes; the working tree was clean before this verification report was added.

## Playwright evidence

- Smoke test loads both canonical snapshots, asserts the scalar choices, selects elevation and precipitation, enables wind and rivers, checks the canvas, selects the second step, and captures the two expected screenshots.
- The test collects `console` errors, uncaught page errors, and failed requests; all remained empty.
- `apps/viewer/test-results/viewer-elevation.png` and `apps/viewer/test-results/viewer-overlays.png` exist at their expected candidate-owned paths.
- A separate focused browser interaction moved the pointer over the rendered mesh and observed `Cell 0 · elevation: 0.10000`; it recorded no console errors.
- Reset-camera behavior is implemented by restoring the camera position/target and updating OrbitControls (`src/main.ts`, `resetCamera`).

## Human visual review

Inspected both generated PNGs directly.

- `viewer-elevation.png`: scalar variation is readily visible across the mesh; the numeric elevation min/max is visible; controls, legend, and canvas are readable and not overlapping.
- `viewer-overlays.png`: precipitation variation remains visible; three pale vector arrows and red network edges are distinguishable and visible; controls and legend remain readable.
- Hover is not part of the static screenshots, so it was checked with the focused browser interaction above and showed a cell ID and selected scalar value.
- **Failure:** the rendered object is a single flat triangle, not a recognizable globe/spherical mesh. `fixtures/snapshots/canonical-small.json` has three positions and one triangle (lines 2–5), and the viewer directly emits those positions and indices as geometry (`src/main.ts`, lines 148–152). Both screenshots visibly show this triangular patch. The second fixture is also the same minimal mesh. Thus the candidate does not satisfy the required globe/spherical-mesh presentation against the committed canonical fixture(s).

## Acceptance checklist

- [x] Loads committed canonical JSON fixtures without a Rust/live-server protocol.
- [ ] Globe/spherical mesh is visibly rendered from positions and triangles — **FAIL:** one flat triangular patch from the canonical three-vertex/one-triangle fixture.
- [x] Scalar dropdown exists; scalar selection updates visible colors.
- [x] Automatic scalar min/max range is visible.
- [x] Vector `None`/dropdown exists; selected vector renders visible arrows.
- [x] Network `None`/dropdown exists; selected network renders visible line overlay.
- [x] Reset camera control exists and resets camera/target in source.
- [x] Hover displays hit cell ID and selected scalar value.
- [x] Multiple fixture steps are selectable.
- [x] Canvas remains visible through control changes.
- [x] Required smoke actions ran without browser console errors; expected screenshots were generated and inspected.
- [x] Candidate changes are confined to `apps/viewer/**`; dependency and generic-schema constraints pass.

## Findings

1. **Blocking visual acceptance failure — no globe/spherical mesh.** The canonical fixture has only three vertex positions and one triangle, while the viewer draws that geometry directly. The inspected screenshots consequently depict a flat triangle rather than a globe. This is a FAIL even though scalar shading and overlays are visible.

## Coordinator notes

Automated frontend gates, ownership/dependency review, generic semantics review, overlays, legend, and hover inspection passed. W1-E cannot receive PASS until the required visual globe/spherical-mesh criterion is met and independently re-verified. No implementation files or fixtures were changed during verification.
