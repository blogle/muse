# Verification

## Verdict PASS

The original verification at `45389a3` returned **FAIL** for the unsupported declared-type hole recorded under Findings. Repair commit `3a1d62daf5aba7c302bc4105390f3589f8227f18` closes that finding: every declared top-level and recipe input/parameter type is validated before compilation proceeds. The final re-verification below passed.

## Candidate

- Repository: `blogle/muse`
- Frozen base: `b1f144b2fd7e967c98d10a44c8ed0ff1ec95716f`
- Candidate branch: `wave1/compiler` (checked out locally as `anvil/muse-wave1-c-verifier-c245b6ff`; `origin/wave1/compiler` pointed at the same commit)
- Candidate commits reviewed: `139f6dc71dbb683d8b2b66b3b4cea320656c83f1`, `8d2d0082a3868ac7e6daf9bba9cc9d8f0fc0c2b6`, `f8f073f89f9235f6baa336f4a8a80c5200d1cd56`, `3a1d62daf5aba7c302bc4105390f3589f8227f18`
- Repair candidate HEAD: `3a1d62daf5aba7c302bc4105390f3589f8227f18`

## Ownership/dependencies

- `git diff --name-status <base>...3a1d62d` shows committed implementation changes only under `crates/muse-spec/**` and `fixtures/specs/**`. No committed root manifest/lockfile changes, `muse-types` edits, other-crate/runtime edits, or contract changes were found.
- `crates/muse-types/src/lib.rs` descriptor and IR definitions are unchanged from the frozen base.
- Direct leaf dependencies in `crates/muse-spec/Cargo.toml` are `cel`, `petgraph`, `serde`, `serde-saphyr`, `serde_json`, `thiserror`, and dev dependency `insta`, each declared using `workspace = true`; `muse-types` is the existing local shared crate dependency.
- Cargo resolved the added leaf dependencies and modified the working-tree `Cargo.lock` by adding the `muse-spec` dependency list (`insta`, `serde_json`, `thiserror`). This is uncommitted and was not included in this verification commit. Coordinator integration note: review and commit the lockfile update during integration if desired.
- The verifier worktree had the prior uncommitted `M Cargo.lock` resolution update before this re-verification and still has only that unrelated working-tree change; the repair commit has no committed root lockfile changes.

## Compile-stage review

- YAML deserialization is implemented with `serde_saphyr::from_str` (`src/lib.rs:124-135`); parse errors become `MalformedYaml`.
- Structs model documents, sections, recipes, parameters, and nodes (`src/lib.rs:60-120`). `compile_document` calls `validate_declared_types` before selecting/compiling a program (`src/lib.rs:137-139`). That validator checks all top-level inputs and parameters and every recipe's inputs and parameters, including unused recipes (`src/lib.rs:491-509`). Unsupported spellings produce `TypeMismatch` at the declaration path through `declared_type` (`src/lib.rs:512-521`).
- Recipe expansion occurs before reference resolution (`src/lib.rs:159-170`). Reference resolution checks supported inputs, parameters, node IDs/output names, state references, and scalar literals (`src/lib.rs:758-847`).
- A `petgraph::DiGraph` is built from node-output references; `toposort` detects cycles and produces ordered `Program.nodes` (`src/lib.rs:428-455`).
- Operator argument checking uses frozen `operator_descriptor` signatures plus a compiler-local scalar config schema (`src/lib.rs:244-319`, `736-755`). `muse-types` descriptors remain unchanged.
- CEL source is validated through public `cel::Program::compile` and preserved in `CompiledExpressionHandle::from_source` (`src/lib.rs:400-410`). No runtime operators are invoked.
- The previously identified declared-type model-validation gap is closed. Validation is global to the document, prior to program selection, and reports the invalid declaration path/value.

## Recipe expansion

- `expand_nodes` recursively traverses recipe node maps, substitutes input and parameter references, qualifies internal node references by use-site namespace, and inserts expanded ordinary nodes (`src/lib.rs:525-693`).
- It binds every declared input and rejects missing/extra input names; it binds declared parameters from use-site values or defaults and rejects missing/extra parameter names (`src/lib.rs:559-652`). Each input and parameter binding is type-checked, and declaration spellings are validated before expansion.
- Recipe output aliases are installed compiler-locally and expanded recursively during resolution (`src/lib.rs:644-649`, `800-811`, `849-876`). Recipe recursion is rejected using an inclusion stack (`src/lib.rs:543-550`, `630-643`).
- `valid-recipe-inclusion.yaml` includes `smoother` twice. Its snapshot shows distinct `first.filtered` and `second.filtered` nodes, with the latter dependent on the former; this is evidence of real expansion and substitution, not only recursion checking.
- The fixture test recompiles the inclusion fixture and compares debug representations, then checks the stable node IDs and dependency reference (`src/lib.rs:979-991`).

## Type/CEL/error review

- The enforced reference forms are `$input.<name>`, `$param.<name>`, `$node.<node_id>.<output>`, and `$state.<field_name>` (`src/lib.rs:768-847`); unsupported references fail rather than being interpreted as scripts. Scalar numeric literals are accepted where scalar values are expected.
- `$param` misses produce `UnknownParameter`, separately from `UnknownReference` (`src/lib.rs:779-789`).
- Every remaining `parse_type` use was inspected: `declared_type` uses it to reject invalid declarations; recipe expansion calls `declared_type` for recipe input/parameter bindings; `check_type` and `reference_type` use it to retrieve types for already-validated top-level declarations. The upfront pass covers these latter calls before they can encounter an unsupported type string.
- `pointwise` expression inputs are checked as `ScalarField`; `vector_expr` inputs as `VectorField`; expression params are checked as `Scalar` (`src/lib.rs:351-366`, `367-395`). Shared input/parameter binding names are rejected (`src/lib.rs:321-330`). No implicit coercion path was found.
- Scalar configuration keys are declared in `config_arguments`; required entries are checked for presence and accepted entries are checked as scalar (`src/lib.rs:244-300`, `312-319`, `736-755`). Unknown keys fail as `UnknownArgument`. Descriptor-backed required arguments are checked separately.
- All required categories are defined and exercised through the fixture/snapshot harness: `UnknownOperator`, `UnknownReference`, `DuplicateNode`, `MissingArgument`, `UnknownArgument`, `UnknownParameter`, `TypeMismatch`, `RecipeRecursion`, `DependencyCycle`, `InvalidCel`, and `MalformedYaml`.
- Error snapshots contain category and YAML path, and where relevant expected/actual types or offending names. Malformed YAML reports parser location rather than a YAML key path, as expected for a syntax error.
- CEL compilation is syntax/compile validation using the public API; validated source text, not a CEL parser object, is stored in frozen IR. No runtime operator execution was observed.

## Commands executed

- `nix develop --command cargo check -p muse-spec` — passed (finished `dev` profile successfully).
- `nix develop --command cargo nextest run -p muse-spec` — passed, 3 tests, including unsupported declared-type site checks and all fixture snapshots.
- `nix develop --command cargo clippy -p muse-spec --all-targets -- -D warnings` — passed.
- `nix develop --command cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `git status --short --branch` — captured after verification; only the previously generated, uncommitted `M Cargo.lock` remains.

## Fixtures/snapshots

- Required original fixture coverage is present: `valid-minimal`, `valid-pointwise`, `error-unknown-op`, `error-unknown-ref`, `error-duplicate-node`, `error-missing-arg`, `error-unknown-arg`, `error-type-mismatch`, `error-recipe-recursion`, `error-cycle`, `error-invalid-cel`, and `error-malformed-yaml`.
- Additional fixtures cover unknown parameters, missing scalar config, duplicate expression binding names, invalid expression binding type, recipe input binding type, unsupported type declaration in an unused recipe schema, and valid multiple-use recipe inclusion. Each YAML fixture has an Insta snapshot through `fixture_results_are_snapshot_stable`.
- `error-unknown-type.yaml` plus its snapshot produce `TypeMismatch at recipes.unsupported_schema.inputs.source` and identify offending `temperature_field` and supported types. A focused unit test additionally checks path-aware rejection for unsupported top-level input, top-level parameter, and recipe parameter declarations (`src/lib.rs:977-997`); the fixture covers recipe input declarations.
- Snapshot assertions are executed on every fixture; the test explicitly compares repeated compilation debug output for valid recipe inclusion and checks that dependencies precede consumers.

## Acceptance checklist

- [x] Committed ownership restricted to `crates/muse-spec/**` and `fixtures/specs/**`.
- [x] Leaf direct dependencies use approved workspace dependencies; no root dependency allowlist edit.
- [x] Frozen descriptors and IR unchanged.
- [x] Real recipe inclusion expansion, namespace, substitution, aliases, recursion prevention, and multiple-use evidence.
- [x] Compiler-side variadic expression binding/type checks and static scalar config checks.
- [x] Required error categories and fixture snapshots.
- [x] Public CEL compile API, retained source handle, and no runtime operator execution.
- [x] Deterministic insertion/order reasoning: specs and compiled nodes are iterated from `BTreeMap`; graph node insertion follows sorted spec keys and edge insertion follows sorted compiled entries/arguments; repeated output is snapshot-checked and tested.
- [x] Validate unsupported declared types for top-level inputs/parameters and recipe inputs/parameters with path-aware `TypeMismatch` errors.
- [x] Required Cargo/Nix and diff checks pass.

## Findings

1. **Resolved historical FAIL (45389a3): unsupported declared type spellings bypassed checks.** Repair `3a1d62d` adds global declaration validation and `declared_type`; unknown strings now fail with path-aware `TypeMismatch`. Inspected and tested top-level input, top-level parameter, recipe input, and recipe parameter declarations. The `error-unknown-type` fixture/snapshot covers a recipe input; the focused unit test covers top-level input/parameter and recipe parameter. No remaining bypass was found.

## Coordinator notes

- No Lific/tracker access was performed.
- The `Cargo.lock` modification is generated during approved leaf dependency resolution and is intentionally excluded from this commit; coordinate lockfile integration separately.
- Repair commit `3a1d62d` was independently inspected and verified; final verdict PASS.
