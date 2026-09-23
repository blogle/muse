# MUSE Development Policy

## Dependency governance

The root `[workspace.dependencies]` table is the sole direct Rust dependency
authority. Packages use only `dependency.workspace = true`; only the coordinator
changes versions or adds entries. A missing capability requires a
`DEPENDENCY_REQUEST.md` containing: capability required, why required, approved
dependencies considered, proposed crate name/version, maintenance evidence,
license, exact API expected, and alternative if rejected.

Do not add denied proof-of-concept dependencies, git dependencies, unsafe code,
or the approved-on-demand dependencies without coordinator approval. Inspect the
approved dependency set before adding generic infrastructure.

## Architecture and performance

Compile YAML/CEL into a `petgraph` DAG, execute whole contiguous fields in
topological order with Rayon, and model explicit iteration blocks as repeated
subsets. Use the approved crates for vectors, icospheres, noise, deterministic
keyed randomness, tests, snapshots, and benchmarks. CEL evaluates bounded scalar
expressions, never field traversal. Do not introduce incremental scheduling,
manual parallelism, generic replacements for approved libraries, or speculative
crate boundaries.

Measure before optimizing: Divan, then perf, then Coz. `profile-coz` remains an
intentional failing placeholder until a canonical simulation workload exists.

## Workflow

Nix defines the pinned environment and reproducible artifacts; native Cargo is
the hot loop. Keep `target/` persistent and untracked. Use cargo-nextest as the
test runner. Run the authoritative commands documented in the README before
completion.
