# Viewer

Run the browser debug viewer from this directory with `pnpm dev` inside
`nix develop`. The dev server is available at <http://localhost:4173>.

The viewer reads the committed JSON files under `../../fixtures/snapshots/`.
Run `pnpm test` for the Playwright smoke test and `pnpm build` for a production
bundle.
