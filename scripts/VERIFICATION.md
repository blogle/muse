# Verification

## Verdict PASS

The original W1-A verification found the deterministic Playwright environment
missing and returned FAIL. Repair commit `98a7cdf` resolves that finding: the
Nix shell exports a pinned browser path, disables browser downloads, supplies
fontconfig and DejaVu fonts, and successfully launches Chromium through a
temporary package matching the Nix-provided Playwright 1.63.0 without a custom
executable path or store scan. The relevant W1-A gates were rerun and passed.

## Candidate

- Ref under review: `wave1/tooling`
- Frozen base: `b1f144b2fd7e967c98d10a44c8ed0ff1ec95716`
- Original candidate: `fa5670974f175af99585ee8cecd3eef642523a9a`
- Re-verification candidate: `98a7cdfdd0ed0b81def05399e59442b0a642f113`
- Verifier re-verification commit: this report-update commit (SHA in final response).
- Verification was performed against the actual Git diff and commands, not the
  claims in `scripts/RESULT.md`.

## Re-verification after `98a7cdf`

Inspected `git diff dba1612..98a7cdf -- flake.nix scripts/RESULT.md` directly.
The repair delta is limited to those two permitted files. `flake.nix` now:

- Sets `PLAYWRIGHT_BROWSERS_PATH` from
  `${pkgs.playwright-driver.browsers}`.
- Sets `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD = "1"`.
- Provides `FONTCONFIG_FILE` from `pkgs.makeFontsConf`, configured with pinned
  minimal DejaVu fonts and no impure font directories/includes.
- Adds `pkgs.fontconfig` to the shell.

`scripts/RESULT.md` reports Playwright 1.63.0. Independent verification
confirmed the actual CLI version and matched it with an external temporary
`@playwright/test` 1.63.0 package. No application/viewer files were changed.

## Ownership review

Implementation base-to-candidate changes are limited to permitted W1-A files:

```text
M Cargo.toml
M flake.nix
M justfile
A scripts/RESULT.md
A scripts/bench.sh
A scripts/check.sh
A scripts/profile-coz.sh
```

There are no crate, application, runtime, fixture, or source-code changes.
`Cargo.lock` is unchanged. The root `[workspace.dependencies]` table is byte-
for-byte unchanged between base and HEAD. `docs/wave1/CONTRACT.md` is unchanged.
Workspace membership and resolver are unchanged.

## Profiles/Nix review

- `[profile.dev]` has `opt-level=0`, `incremental=true`,
  `codegen-units=256`, and `debug="line-tables-only"`.
- `[profile.ci]` inherits `dev`, with incremental disabled and all required
  `ci` values explicit: `opt-level=0`, `codegen-units=256`, and line-table
  debug info.
- `[profile.ci-release]` inherits `release`, with `opt-level=1`,
  `incremental=false`, `codegen-units=256`, `lto=false`, and `debug=false`.
- `rust-toolchain.toml` pins Rust 1.98.1; the Nix shell reported rustc
  1.98.1. The flake lock pins nixpkgs and Crane revisions.
- `nix develop` is a `mkShell` of tools and does not depend on an application
  package. `nix develop --command true` succeeded without invoking a product
  build.
- `flake.nix` creates `cargoArtifacts` once with `buildDepsOnly`, passes those
  artifacts to the check, Clippy, nextest, release, and bench derivations, and
  defines a workspace `check` derivation.
- `target/` resolved to the writable checkout path
  `/home/anvil/workspace/muse-wave1-a-verifier/target`, outside `/nix/store`.
- The shell includes Node, pnpm, Playwright tooling/browser packages, and
  Linux-only `perf`; Coz is included as a tool. No distributed cache,
  deployment, or OCI changes are present.
- Playwright environment re-verification and resolution are recorded under
  Findings.

## Commands executed

All required commands passed:

```text
nix develop --command cargo check --workspace                         PASS
nix develop --command cargo nextest run --workspace                    PASS (5 passed, 0 skipped)
nix develop --command cargo fmt --all -- --check                       PASS
nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings  PASS
nix develop --command cargo deny check                                  PASS
nix flake check                                                         PASS
git diff --check                                                        PASS
nix develop --command true                                              PASS
```

Re-verification commands after repair commit `98a7cdf`:

```text
nix develop --command bash -lc 'printf "PLAYWRIGHT_BROWSERS_PATH=%s\\nPLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=%s\\nFONTCONFIG_FILE=%s\\n" "$PLAYWRIGHT_BROWSERS_PATH" "$PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD" "$FONTCONFIG_FILE"; test -d "$PLAYWRIGHT_BROWSERS_PATH"; test -f "$FONTCONFIG_FILE"; command -v fc-match; fc-match sans-serif; playwright --version; node --version; pnpm --version; case "$PLAYWRIGHT_BROWSERS_PATH" in /nix/store/*) ;; *) exit 1;; esac; test "$PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD" = 1'  PASS
nix develop --command playwright screenshot --browser chromium about:blank /tmp/opencode/playwright-verifier-fresh-shell.png  PASS
nix develop --command pnpm --dir /tmp/opencode/muse-playwright-verifier install --config.browser-download=false  PASS
nix develop --command pnpm --dir /tmp/opencode/muse-playwright-verifier test  PASS (1 passed)
nix develop --command cargo check --workspace  PASS
nix develop --command cargo nextest run --workspace  PASS (5 passed, 0 skipped)
nix flake check  PASS
git diff --check  PASS
```

Additional checks:

- Entered the development shell and checked `rustc`, Cargo, nextest, cargo-deny,
  Node, pnpm, Playwright CLI, Coz, and perf availability. Rust reports
  1.98.1; all listed tools were found on this x86_64-linux host.
- Confirmed `target/` is writable and is not in `/nix/store`.
- `nix flake check` evaluated and ran the x86_64-linux checks successfully;
  incompatible aarch64-darwin and aarch64-linux systems were omitted by Nix.
- `cargo deny check` emitted non-failing duplicate-version and unmatched
  license-allowance warnings; advisories, bans, licenses, and sources passed.
- After the repair, `nix flake check` passed again; all declared checks were
  evaluated, with cached derivations resulting in zero rebuild checks. It again
  omitted incompatible aarch64-darwin and aarch64-linux systems.
- Rechecked the full base-to-candidate ownership diff after repair. Only the
  permitted W1-A tooling files plus this verification report are changed;
  `Cargo.lock`, `docs/wave1/CONTRACT.md`, workspace dependency declarations,
  and application/runtime source remain unchanged.

## Policy/bench/Coz evidence

- A temporary project outside the repository used the real `divan` 0.1.21
  dependency, a `harness=false` bench, `divan::main()`, and a `#[divan::bench]`
  function. `nix develop --command cargo bench --manifest-path
  /tmp/opencode/muse-divan-independent/Cargo.toml --bench smoke --offline`
  compiled and ran the benchmark. `scripts/bench.sh` delegates to
  `cargo bench --workspace "$@"`; there is no alternate benchmark harness and
  no Criterion dependency.
- A disposable external manifest with direct dependency `anyhow="*"` was
  checked using the repository's `deny.toml`. `cargo deny check bans` rejected
  it with `error[wildcard]`, as required. No fixture was placed in the repo.
- `scripts/profile-coz.sh` has explicit Linux and missing-Coz failures, requires
  a supplied workspace binary target and forwards the remaining workload
  arguments, builds using `--profile coz`, sets a configurable `.coz` output
  path, and invokes Coz against that target. The `coz` profile is optimized
  (`opt-level=2`) with line-table debug information. There is no Rust Coz
  annotation crate in the workspace dependency table.
- Independently ran `COZ_PROFILE=/tmp/opencode/muse-independent.coz nix develop
  --command ./scripts/profile-coz.sh muse --help`. The build completed in the
  `coz` profile, Coz accepted the forwarded `--help` workload argument, and
  wrote an inspectable ASCII `.coz` file. This validates plumbing only, not a
  canonical simulation profile.
- `nix develop --command playwright screenshot --browser chromium about:blank
  /tmp/opencode/playwright-blank.png` succeeded via the Nix-provided CLI and
  launched Chromium; that first-run inspection found no stable browser/
  fontconfig environment and is the historical failure resolved below.
- After repair, independent fresh-shell inspection confirmed:
  `PLAYWRIGHT_BROWSERS_PATH=/nix/store/3av8irm8x5vvrqhkb5p22dpc5m8fsg9-playwright-browsers`,
  `PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD=1`, and
  `FONTCONFIG_FILE=/nix/store/rv0bff9w24x9raqx25556lqcgnh07v00-fonts.conf`.
  The browser path is a directory, the config is a file, and `fc-match
  sans-serif` resolves to DejaVu Sans. The shell `playwright --version` reports
  1.63.0.
- Independently created `/tmp/opencode/muse-playwright-verifier` with
  `@playwright/test` 1.63.0 and a minimal `about:blank` browser test. From
  inside `nix develop`, pnpm installed it with browser downloads disabled, and
  `pnpm test` launched Chromium and passed (1 test). The test uses Playwright's
  default browser fixture; it contains no browser executable override or
  `/nix/store` scan. This directly validates the exported browser path with the
  matching Playwright package.

## Acceptance criteria checklist

- [x] Permitted-file ownership; no runtime source edits.
- [x] Root workspace dependencies, workspace shape, Cargo.lock, and frozen
  contract unchanged.
- [x] Required development, CI, and CI-release profile intent preserved.
- [x] Tooling-only `nix develop`, pinned Rust/tools, Crane dependency reuse,
  check derivation, and mutable target outside the Nix store.
- [x] Linux perf and Coz tools; no distributed cache/deployment/OCI changes.
- [x] Nextest is the workspace test runner and required nextest command passed.
- [x] cargo-deny passes and rejects the wildcard direct-dependency fixture.
- [x] Divan harness plumbing executes a real temporary Divan benchmark; no
  Criterion.
- [x] Coz script host failure, optimized line-table profile, supplied target
  and workload args, profile output, and no annotation crate reviewed.
- [x] Frontend Node/pnpm and Playwright dependencies provide a reproducible
  launch environment: stable pinned browser path, browser
  downloads disabled, deterministic fontconfig, and matching 1.63.0 package
  launches Chromium without custom executable configuration or store scanning.
- [x] All required acceptance commands passed, including `git diff --check`.

## Findings

1. **RESOLVED — deterministic Playwright environment for Agent E.** The
   original finding at verifier commit `dba1612` recorded absent browser and
   fontconfig environment variables and the viewer/Nix Playwright version
   mismatch, and correctly returned FAIL at that point. Repair `98a7cdf`
   exports the pinned browser path, disables downloads, supplies DejaVu-backed
   fontconfig, and was independently validated with a matching 1.63.0 Playwright
   test package. Chromium launched through Playwright's default browser fixture
   without scanning the store or setting an executable path.

2. **PASS — cargo-deny warnings are non-fatal.** Existing duplicate transitive
   versions and an unmatched `ISC` license allowance were reported, while all
   policy categories completed successfully. They do not violate the requested
   policy checks.

## Coordinator notes

The previous FAIL is retained above as historical evidence and is resolved by
`98a7cdf`. The repaired environment passed independent browser-path,
download-disable, fontconfig, package-version, and browser-launch checks. The
re-run Rust/Nix gates passed, the repair diff remains within W1-A ownership, and
all W1-A acceptance criteria are now **PASS**.
