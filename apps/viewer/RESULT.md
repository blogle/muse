# W1-E Browser Debug Viewer — Result

## Implementation

- Added a TypeScript/Vite and Three.js/WebGL viewer that loads both committed canonical snapshots directly as static JSON.
- Builds indexed mesh geometry from snapshot positions and triangles, colors generic scalar fields with automatic min/max scaling, draws projected vector arrows and network edges, and reports triangle cell ID plus its selected scalar average on hover.
- Added snapshot, scalar, vector, and network selectors, camera reset, and a visible numeric range legend.
- Added a Playwright smoke test that exercises scalar switching, vector/network overlays, step selection, console-error detection, and deterministic screenshots under `test-results/viewer-elevation.png` and `test-results/viewer-overlays.png`.
- The Playwright launcher requires explicit `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` and `FONTCONFIG_FILE` environment variables and validates both paths. It does not guess browser or font paths.

## Preview

Run `pnpm dev` from `apps/viewer` inside `nix develop`. The viewer listens on **port 4173**.

## Validation

Run from `apps/viewer` inside `nix develop`:

```sh
export PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH=/nix/store/i08h6m15rm5n2w13lc2s5kq5v607q626-playwright-chromium-headless-shell/chrome-headless-shell-linux64/chrome-headless-shell
export FONTCONFIG_FILE=/nix/store/pyghkr26krhfj4hbr3xz2sh3xbip048s-fonts.conf
pnpm install --frozen-lockfile
pnpm typecheck
pnpm build
pnpm test
git diff --check
```

All commands passed. Vite reports the bundled Three.js chunk is slightly over 500 kB.

Playwright browser diagnosis: before repair, the test was pointed at `/bin/chromium` (Chromium 153), which navigated to the Vite page and then aborted with `FATAL:third_party/skia/src/ports/SkFontMgr_FontConfigInterface.cpp:163] Not implemented.` and `SIGABRT`; stderr also reported `Fontconfig error: Cannot load default config file: File not found`. The Nix shell provides a Playwright-matched headless-shell and a fonts.conf but does not currently export their paths. With those paths passed explicitly as `PLAYWRIGHT_CHROMIUM_EXECUTABLE_PATH` and `FONTCONFIG_FILE`, the smoke passes with real WebGL/SwiftShader rendering and no browser console errors. **Integration dependency:** W1-A should export stable values for these two environment variables from `nix develop`; the viewer test wrapper intentionally does not scan the Nix store to infer them.

Passing screenshots:

- `apps/viewer/test-results/viewer-elevation.png` (57 KB)
- `apps/viewer/test-results/viewer-overlays.png` (65 KB)

No fixture schemas or shared contracts were changed.
