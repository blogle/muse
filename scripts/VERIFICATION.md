# Verification

## Verdict FAIL

The candidate passes the Rust, policy, Nix evaluation/build, Divan, Coz, and
ownership checks. The frontend/Playwright environment does not fully satisfy
the W1-A acceptance requirement: the shell exports no stable Playwright browser
or fontconfig environment, and the package version provided by Nix differs from
the viewer's pinned Playwright version. A minimal launch worked through the
Nix-packaged CLI on this host, but that does not establish a deterministic
environment for the viewer's locked package. See Findings.

## Candidate

- Ref under review: `wave1/tooling`
- Frozen base: `b1f144b2fd7e967c98d10a44c8ed0ff1ec95716`
- Candidate HEAD: `fa5670974f175af99585ee8cecd3eef642523a9a`
- Verification was performed against the actual Git diff and commands, not the
  claims in `scripts/RESULT.md`.

## Ownership review

Base-to-HEAD changes are limited to permitted W1-A files:

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
- Playwright environment details and the acceptance gap are recorded under
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

Additional checks:

- Entered the development shell and checked `rustc`, Cargo, nextest, cargo-deny,
  Node, pnpm, Playwright CLI, Coz, and perf availability. Rust reports
  1.98.1; all listed tools were found on this x86_64-linux host.
- Confirmed `target/` is writable and is not in `/nix/store`.
- `nix flake check` evaluated and ran the x86_64-linux checks successfully;
  incompatible aarch64-darwin and aarch64-linux systems were omitted by Nix.
- `cargo deny check` emitted non-failing duplicate-version and unmatched
  license-allowance warnings; advisories, bans, licenses, and sources passed.

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
  launched Chromium; the screenshot file was produced. Environment inspection
  nevertheless found no `PLAYWRIGHT_BROWSERS_PATH`, `FONTCONFIG_FILE`, or
  `FONTCONFIG_PATH`, and `fc-match` is not available in the shell. The viewer
  pins `@playwright/test` 1.58.2 while the shell CLI reports 1.63.0.

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
- [ ] Frontend Node/pnpm and Playwright dependencies provide a reproducible,
  viewer-compatible launch environment. Nix CLI launch passed, but pinned
  browser/fontconfig environment is not exported and Nix CLI version differs
  from the viewer lock.
- [x] All required acceptance commands passed, including `git diff --check`.

## Findings

1. **FAIL — deterministic Playwright environment for Agent E is not established.**
   `flake.nix` installs `playwright-driver` and `playwright-test`, but the shell
   exports no `PLAYWRIGHT_BROWSERS_PATH`, skip-download setting, or fontconfig
   path/file. `fc-match` is absent. The normal Nix `playwright screenshot`
   command did launch a Nix-provided Chromium for `about:blank` in this run, but
   that CLI is version 1.63.0 while `apps/viewer/package.json` pins
   `@playwright/test` 1.58.2. This does not verify that the viewer's locked
   package launches the matching Nix browser deterministically. Minimal root
   tooling correction: expose a stable browser path (or executable) and a
   usable fontconfig file/path derived from the pinned Nix package paths, and
   align the browser/driver version with the viewer's pinned Playwright
   package. Do not rely on a viewer helper scanning arbitrary `/nix/store`
   entries.

2. **PASS — cargo-deny warnings are non-fatal.** Existing duplicate transitive
   versions and an unmatched `ISC` license allowance were reported, while all
   policy categories completed successfully. They do not violate the requested
   policy checks.

## Coordinator notes

The Nix Playwright CLI smoke launch proves the packaged CLI/browser pair can
start on this host. The explicit environment inspection and version comparison
leave the viewer's locked Playwright launch path unresolved; the candidate is
therefore **FAIL** pending the root-tooling correction identified above. All
other W1-A acceptance items verified here passed.
