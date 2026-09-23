# W1-E Browser Debug Viewer — Result

## Implementation

- Added a TypeScript/Vite and Three.js/WebGL viewer that loads both committed canonical snapshots directly as static JSON.
- Builds indexed mesh geometry from snapshot positions and triangles, colors generic scalar fields with automatic min/max scaling, draws projected vector arrows and network edges, and reports triangle cell ID plus its selected scalar average on hover.
- Added snapshot, scalar, vector, and network selectors, camera reset, and a visible numeric range legend.
- Added a Playwright smoke test that exercises scalar switching, vector/network overlays, step selection, console-error detection, and deterministic screenshots under `test-results/viewer-elevation.png` and `test-results/viewer-overlays.png`.

## Preview

Run `pnpm dev` from `apps/viewer` inside `nix develop`. The viewer listens on **port 4173**.

## Validation

Run from `apps/viewer` inside `nix develop`:

- `pnpm install --frozen-lockfile` — passed.
- `pnpm typecheck` — passed.
- `pnpm build` — passed (Vite reports the expected bundled Three.js chunk is slightly over 500 kB).
- `pnpm test` — blocked in this environment: Chromium closes the Playwright page during viewer startup before the selectors become available. Thus screenshots were not produced in this run; they are emitted to the deterministic paths above when the smoke test completes.
- `git diff --check` — passed.

No fixture schemas or shared contracts were changed.
