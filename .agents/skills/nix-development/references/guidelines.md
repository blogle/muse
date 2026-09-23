# Nix Development and Build Guidelines

## Goals and principles

Use Nix for reproducible environments and artifacts, not as a replacement for
fast native development loops. Keep immutable dependency construction separate
from mutable development state, make rebuild cost proportional to change scope,
and prefer content-addressed reuse over monolithic reconstruction. Treat caches
as part of the architecture, avoid filesystem-wide transformations of large Nix
closures, and keep fast validation separate from maximally optimized production
builds.

## Development environments and mutable state

`nix develop` establishes compilers, package managers, linters, test runners,
and native libraries. It must not build the application merely to enter the
shell: prefer `mkShell` with tool packages, and avoid `inputsFrom` application
packages when that realizes expensive derivations. Prefer direnv with nix-direnv
for interactive work when practical.

Keep mutable compiler and package-manager caches outside immutable derivations.
Examples include `target/`, `.cache/`, `build/`, and language-specific
incremental databases. Preserve safe caches across commands and resumable
sessions. Do not routinely clean them. The normal boundary is:

```text
Nix reproducible toolchain and dependencies
  -> native build system
  -> persistent mutable build state
```

Use `nix build` at reproducibility, packaging, deployment, and CI boundaries,
not after each local source edit.

## Derivations and caching

Structure derivations by change frequency: toolchain, dependencies,
application, then runtime configuration. Give each derivation the smallest
meaningful source set; Rust dependency builds normally need manifests, lockfile,
build scripts, and dependency metadata, not unrelated documentation or assets.
One-line changes should invalidate work close to their semantic scope.

Use complementary cache layers: developer mutable cache, compiler/build-system
cache, Nix store, trusted shared binary cache, and OCI registry blobs. Keep cache
keys deterministic and free of checkout paths, timestamps, unrelated source, and
ephemeral values. CI and shared builders should download trusted binary-cache
outputs rather than rebuild them.

Nix derivations and build systems both parallelize. Configure outer Nix jobs and
inner compiler/linker jobs together to avoid CPU, memory, and I/O oversubscription.

## OCI and CI

Organize OCI images by volatility: base runtime, tools, large runtime
dependencies, language dependencies, application, runtime scripts/configuration,
then OCI metadata. Stable cached layers are preferable to a frequently changing
monolith. Keep image filesystem content immutable and writable caches, workspaces,
databases, and user data in runtime storage.

Do not recursively chown, chmod, copy, or rewrite large dependency closures.
Set ownership while constructing small writable roots. Prefer native
content-addressed OCI construction and individual-layer reuse over Docker archive
load/save round trips. Put Entrypoint, Cmd, Env, User, WorkingDir, labels, and
ports in OCI configuration rather than rewriting filesystem trees.

When CI gains Crane, build shared Cargo dependencies once and reuse them for
format/lint, checks, tests, docs, builds, and images. Use separate derivations
for fast feedback and production artifacts. Validated artifacts should be reused
in later stages where configurations intentionally match.

## Production and anti-patterns

Production may trade compile time for measured runtime performance, but must not
make developers or normal PR validation pay that cost. A revision can produce a
fast validation artifact, a portable optimized artifact, and an architecture-
optimized artifact. Do not use `target-cpu=native` for portable reproducible
artifacts; select explicit guaranteed deployment CPU classes instead.

Avoid application builds on shell entry, immutable hot caches, routine cleans,
whole-repository source inputs, independently rebuilding dependencies per CI
check, recursive closure rewrites, archive round trips, volatile configuration
combined with huge stable content, uncontrolled nested parallelism, and treating
few OCI layers as inherently better.

Ask: "If I change one line, what work becomes invalid?" The answer should match
the scope of that line: configuration nearly nothing, application source the
application, dependency changes the affected dependency closure, and a toolchain
or nixpkgs update may be expensive.
