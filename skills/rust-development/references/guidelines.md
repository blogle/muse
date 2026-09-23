# Rust Development and Build Guidelines

## Goals and edit loop

Optimize development builds for compilation speed, preserve incremental state,
use `cargo check` whenever machine code is unnecessary, and keep development,
CI, and production profiles distinct. Build dependencies once and reuse them;
pay expensive LLVM optimization only where runtime performance matters. Measure
before adding exotic linker or compiler configuration.

Keep `target/` as valuable mutable state. Do not routinely delete or recreate it
per command, sandbox, or agent interaction. Persistent work can use
`repo/target/`; ephemeral isolated workers should use a persistent per-worktree
path such as `/cache/rust/<repository>/<worktree>/target`. Never share a mutable
incremental target directory between unrelated concurrent worktrees.

The default ladder is: affected-package `cargo check`, targeted debug test,
crate-level tests, workspace tests, then full project validation. Agents should
not rebuild unrelated integration suites during a local iteration, but full
validation is required before completion.

## Profiles and caches

Persistent development uses `incremental = true`, low optimization, many codegen
units, and line-table debug information. Full debug symbols are opt-in through a
separate debugging profile. Fast CI should inherit development behavior but use
`incremental = false`; fast runnable/E2E artifacts can use low release
optimization without LTO; production uses separate, measured optimization such
as thin LTO and fewer codegen units.

Incremental compilation and compiler-output caching solve different problems.
Persistent human or agent work favors incremental compilation and a stable target
directory. Ephemeral CI favors non-incremental compilation, sccache where useful,
and shared dependency artifacts. Do not globally enable both without understanding
which cache helps the workload. Compiler caches complement Cargo reuse, Crane
artifacts, and Nix binary caches; keep their keys stable across equivalent
checkouts.

Do not use `-C target-cpu=native` for reproducible or distributed output. Keep a
portable artifact and use explicit CPU classes only when deployment compatibility
is guaranteed. Treat PGO as a late measured optimization using representative
workloads, not default machinery.

## Linking, parallelism, dependencies, and architecture

Start with modern Rust defaults. Measure link time separately from compile time;
only add mold, wild, or custom rust-lld configuration if linking is a demonstrated
bottleneck. Consider total concurrency across CI jobs, Nix derivations, Cargo,
codegen units, and linker threads. More threads can oversubscribe CPU, memory,
bandwidth, or I/O, so benchmark representative clean and incremental builds.

Every dependency costs resolution, compilation, disk, invalidation, security
surface, and sometimes binary size. Keep dependencies and enabled features narrow,
disable unnecessary defaults, and remove unused dependencies. Be especially wary
of proc macros, code generation, build scripts, C/C++ dependencies, and broad
feature graphs. Crate boundaries should reflect coherent architecture, with build
parallelism and invalidation as secondary considerations; do not create microcrates
only for build speed.

## CI and agent work

When using Nix and Crane, construct `buildDepsOnly` Cargo artifacts once and
reuse them for clippy, tests, nextest, builds, docs, and fast executables. Let
Nix and shared binary caches distribute those derivations. Keep format/lint,
type checking, targeted/full tests, fast runnable artifacts, and production
artifacts separate. Do not make staging or E2E wait for maximum LLVM optimization.

Agents work best with this cycle: edit, check affected scope, targeted test,
inspect failure, edit, targeted test, broader validation, project-wide check.
Do not perform clean builds repeatedly. Provision missing native dependencies
through the environment and distinguish code failures from environment failures.
Experimental accelerators such as a parallel rustc frontend or Cranelift can be
optional development experiments only after validating project compatibility;
production stays on supported LLVM unless deliberately changed.

## Anti-patterns and rule of thumb

Avoid routine `target/` deletion, full-workspace builds after every edit, release
builds for ordinary development, maximum optimization in PR validation, treating
incremental compilation and sccache as interchangeable, shared targets across
worktrees, `target-cpu=native` for portable CI, independently rebuilding
dependencies for each check, blind huge `-j` values, unmeasured custom linkers,
excessive microcrates, unused dependencies, broad defaults, and E2E blocked on
maximum optimization.

Ask: "What is the cheapest compilation step that can tell me whether this change
is correct?" For release work, ask what measurable deployment or runtime benefit
justifies the additional build cost.
