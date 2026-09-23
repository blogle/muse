# W1-E Browser Debug Viewer — Result

## Implementation

- Added a TypeScript/Vite and Three.js/WebGL viewer that loads both committed canonical snapshots directly as static JSON.
- Builds indexed mesh geometry from snapshot positions and triangles, colors generic scalar fields with automatic min/max scaling, draws projected vector arrows and network edges, and reports triangle cell ID plus its selected scalar average on hover.
- Added snapshot, scalar, vector, and network selectors, camera reset, and a visible numeric range legend.
- Added a Playwright smoke test that exercises scalar switching, vector/network overlays, step selection, console-error detection, and deterministic screenshots under `test-results/viewer-elevation.png` and `test-results/viewer-overlays.png`.
- Pins `@playwright/test` to **1.63.0**, matching the Nix shell, and uses Playwright's standard browser lookup through `PLAYWRIGHT_BROWSERS_PATH`.
- The Playwright launcher requires `PLAYWRIGHT_BROWSERS_PATH` and `FONTCONFIG_FILE` and validates both paths. It does not guess browser or font paths.

## Preview

Run `pnpm dev` from `apps/viewer` inside `nix develop`. The viewer listens on **port 4173**.

## Validation

Run from `apps/viewer` inside `nix develop`. The browser directory comes from Nix evaluation of the repository's pinned `playwright-driver.browsers`; the fonts.conf path comes from evaluating and realizing `makeFontsConf` against the pinned Nix inputs:

```sh
export PLAYWRIGHT_BROWSERS_PATH=/nix/store/3av8irm8x5vvrqhkbp5l22dpc5m8fsg9-playwright-browsers
export FONTCONFIG_FILE=/nix/store/vf2512adbirk4l16r8byxws78yi1lxrx-fonts.conf
pnpm install --frozen-lockfile
pnpm typecheck
pnpm build
pnpm test
git diff --check
```

Nix inspection commands used to obtain those paths:

```sh
nix eval --impure --raw --expr 'let pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux; in pkgs.playwright-driver.browsers.outPath'
nix build --impure --no-link --print-out-paths --expr 'let pkgs = (builtins.getFlake (toString ./.)).inputs.nixpkgs.legacyPackages.x86_64-linux; in pkgs.makeFontsConf { fontconfig = pkgs.fontconfig; fontDirectories = [ pkgs.dejavu_fonts ]; }'
```

All package and smoke commands passed. Vite reports the bundled Three.js chunk is slightly over 500 kB.

Playwright browser diagnosis: before repair, the test was pointed at `/bin/chromium` (Chromium 153), which navigated to the Vite page and then aborted with `FATAL:third_party/skia/src/ports/SkFontMgr_FontConfigInterface.cpp:163] Not implemented.` and `SIGABRT`; stderr also reported `Fontconfig error: Cannot load default config file: File not found`. The smoke pins `@playwright/test` 1.63.0, matching the Nix shell, and resolves its browser through standard `PLAYWRIGHT_BROWSERS_PATH`; with `FONTCONFIG_FILE` set it passes with real WebGL/SwiftShader rendering and no browser console errors. **Integration dependency:** W1-A should export stable values for `PLAYWRIGHT_BROWSERS_PATH` and `FONTCONFIG_FILE` from `nix develop`; the viewer wrapper only validates these environment variables and does not scan the Nix store.

Passing screenshots:

- `apps/viewer/test-results/viewer-elevation.png` (65 KB)
- `apps/viewer/test-results/viewer-overlays.png` (65 KB)

No fixture schemas or shared contracts were changed.
