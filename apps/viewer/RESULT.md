# W1-E Browser Debug Viewer — Result

## Implementation

- Added a TypeScript/Vite and Three.js/WebGL viewer that loads both committed canonical snapshots directly as static JSON.
- Builds indexed mesh geometry from snapshot positions and triangles, colors generic scalar fields with automatic min/max scaling, draws projected vector arrows and network edges, and reports triangle cell ID plus its selected scalar average on hover.
- Added snapshot, scalar, vector, and network selectors, camera reset, and a visible numeric range legend.
- Added a Playwright smoke test that exercises scalar switching, vector/network overlays, step selection, console-error detection, and deterministic screenshots under `test-results/viewer-elevation.png` and `test-results/viewer-overlays.png`.
- The Playwright launcher selects the Nix-provided Chromium headless-shell and fonts.conf from `/nix/store` when available. In other environments it falls back to Playwright's normal browser discovery.

## Preview

Run `pnpm dev` from `apps/viewer` inside `nix develop`. The viewer listens on **port 4173**.

## Validation

Run from `apps/viewer` inside `nix develop`:

- `nix develop --command bash -lc 'pnpm install --frozen-lockfile && pnpm typecheck && pnpm build && pnpm test'` — passed.
- `git diff --check` — passed.

Playwright browser diagnosis: before repair, the test was pointed at `/bin/chromium` (Chromium 153), which navigated to the Vite page and then aborted with `FATAL:third_party/skia/src/ports/SkFontMgr_FontConfigInterface.cpp:163] Not implemented.` and `SIGABRT`; stderr also reported `Fontconfig error: Cannot load default config file: File not found`. The Nix shell provides a Playwright-matched headless-shell under its `playwright-browsers` store path and a `*-fonts.conf`, but did not export the font config path. `scripts/run-playwright.mjs` discovers these Nix-provided artifacts and passes `MUSE_PLAYWRIGHT_CHROMIUM` and `FONTCONFIG_FILE` to the test process. The smoke now passes with real WebGL/SwiftShader rendering and no browser console errors.

Passing screenshots:

- `apps/viewer/test-results/viewer-elevation.png` (57 KB)
- `apps/viewer/test-results/viewer-overlays.png` (65 KB)

No fixture schemas or shared contracts were changed.
