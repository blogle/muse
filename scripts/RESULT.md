# W1-A Result

## Scope completed

Completed the Wave 1 tooling work: pinned Nix development shell, shared-artifact
Crane checks, Cargo development/CI/release/Coz profiles, check and benchmark
commands, and Coz profiling entry point.

## Files changed

- `Cargo.toml` — completed `ci` settings and added the required `ci-release`
  and optimized line-table `coz` profiles.
- `flake.nix` — added a shared-`cargoArtifacts` workspace check derivation.
- `justfile` — exposed the check and benchmark scripts and wired the Coz script.
- `scripts/check.sh` — runs workspace check, nextest, formatting, Clippy, and
  cargo-deny without clearing caches.
- `scripts/bench.sh` — delegates to `cargo bench --workspace` and forwards
  caller arguments.
- `scripts/profile-coz.sh` — checks host/tool support, builds a Coz profile
  binary, accepts a binary target and workload arguments, and writes a `.coz`
  profile.
- `scripts/RESULT.md` — this report.

No application/runtime source, fixtures, `muse-types`, shared contract, root
workspace membership, or workspace dependency declarations were changed.

## Commands executed

- `nix develop --command rustc --version`
- `nix develop --command cargo check --workspace`
- `nix develop --command cargo nextest run --workspace`
- `nix develop --command cargo fmt --all -- --check`
- `nix develop --command cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `nix develop --command cargo deny check`
- `nix develop --command ./scripts/check.sh`
- `nix develop --command ./scripts/bench.sh`
- `nix develop --command cargo bench --manifest-path /tmp/opencode/muse-divan-smoke/Cargo.toml --bench smoke --offline`
- `nix develop --command cargo deny --manifest-path /tmp/opencode/muse-wildcard-policy-fixture/Cargo.toml --config /home/anvil/workspace/muse-wave1-a-tooling/deny.toml check bans`
- `nix develop --command ./scripts/profile-coz.sh muse --help`
- `nix flake check`
- `git diff --check`
- Shell/tool availability and writable `target/` checks inside `nix develop`.

The two `/tmp/opencode` projects were disposable acceptance fixtures and are
outside the repository.

## Test results

- Workspace `cargo check`: passed.
- Nextest: 5 tests passed, 0 skipped.
- Formatting: passed.
- Clippy with warnings denied: passed.
- cargo-deny: passed; existing duplicate-version and unmatched-license warnings
  remain non-failing.
- `nix flake check`: passed; all checks for the current x86_64-linux system
  passed. Nix reports other systems omitted because they are incompatible with
  this host.
- `scripts/check.sh`: passed all five Cargo quality commands, `nix flake check`,
  and `git diff --check`.
- Divan smoke benchmark: completed using a temporary trivial Divan target; this
  proved the approved `divan` dependency and Cargo bench invocation are usable.
  The measured ~1 ns figure is only a plumbing smoke result, not a performance
  measurement.
- Wildcard policy fixture: cargo-deny rejected temporary `anyhow = "*"` with
  `error[wildcard]` as required.
- Coz smoke invocation: built with `coz` profile (`optimized + debuginfo`),
  accepted a supplied target/workload argument, emitted a `.coz` profile, and
  produced textual Coz output.
- `target/` is writable at the checkout path, outside `/nix/store`.
- On initial shell entry, the application binary was absent; entering the
  tooling shell did not build the application.

## Acceptance criteria checklist

- [x] `nix develop` exposes pinned Rust 1.98.1 and required development tools.
- [x] Crane has reproducibility/check derivations with shared dependency
  artifacts.
- [x] Persistent dev, non-incremental CI, and specified `ci-release` profiles
  are configured.
- [x] Nextest is the configured and exercised workspace test runner.
- [x] cargo-deny policy rejects wildcard dependency versions.
- [x] Divan benchmark command plumbing and trivial target validated.
- [x] Linux dev shell provides `perf`; Coz is available and profiled through
  `scripts/profile-coz.sh` without the Rust Coz annotation crate.
- [x] Node, pnpm, Playwright test and browser packages are present in the dev
  shell.
- [x] Required acceptance commands and `git diff --check` pass.
- [x] No application/runtime source code changed.

## Benchmarks/measurements

No project performance benchmark was run because the workspace has no existing
Divan benchmark target. The disposable Divan smoke target ran successfully to
validate plumbing. Coz was exercised with the current `muse` binary's `--help`
workload solely to validate profile generation and text output.

## Deviations

None. No dependency declarations or lockfile entries were changed.

## Blockers

None.

## Follow-up observations

The workspace currently has no maintained Divan target or canonical simulation
workload. The benchmark and Coz commands accept targets/workloads for future
implementation work without imposing a harness or simulator-specific behavior.
