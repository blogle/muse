# Verification

## Verdict FAIL

## Candidate

- Repository: `blogle/muse`
- Frozen base: `b1f144b2fd7e967c98d10a44c8ed0ff1ec95716f`
- Candidate branch: `wave1/compiler` (checked out locally as `anvil/muse-wave1-c-verifier-c245b6ff`; `origin/wave1/compiler` pointed at the same commit)
- Candidate commits reviewed: `139f6dc71dbb683d8b2b66b3b4cea320656c83f1`, `8d2d0082a3868ac7e6daf9bba9cc9d8f0fc0c2b6`, `f8f073f89f9235f6baa336f4a8a80c5200d1cd56`
- Candidate HEAD: `f8f073f89f9235f6baa336f4a8a80c5200d1cd56`

## Ownership/dependencies

- `git diff --name-status <base>...HEAD` shows committed implementation changes only under `crates/muse-spec/**` and `fixtures/specs/**`. No committed root manifest/lockfile changes, `muse-types` edits, other-crate/runtime edits, or contract changes were found.
- `crates/muse-types/src/lib.rs` descriptor and IR definitions are unchanged from the frozen base.
- Direct leaf dependencies in `crates/muse-spec/Cargo.toml` are `cel`, `petgraph`, `serde`, `serde-saphyr`, `serde_json`, `thiserror`, and dev dependency `insta`, each declared using `workspace = true`; `muse-types` is the existing local shared crate dependency.
- Cargo resolved the added leaf dependencies and modified the working-tree `Cargo.lock` by adding the `muse-spec` dependency list (`insta`, `serde_json`, `thiserror`). This is uncommitted and was not included in this verification commit. Coordinator integration note: review and commit the lockfile update during integration if desired.
- Initial `git status --short --branch`: clean at candidate HEAD. After the Cargo checks: only `M Cargo.lock` (resolution-generated); no implementation files changed.

## Compile-stage review

- YAML deserialization is implemented with `serde_saphyr::from_str` (`src/lib.rs:124-135`); parse errors become `MalformedYaml`.
- Structs model documents, sections, recipes, parameters, and nodes (`src/lib.rs:60-120`). Semantic checks exist for operators, references, required/unknown arguments, selected parameter/reference types, and required expression pieces. However, declared recipe port types are not validated as schema values: `expand_nodes` only queues a binding type check when `parse_type(expected)` returns `Some` (`src/lib.rs:544-551`). `parse_type` returns `None` for unsupported spellings (`src/lib.rs:895-906`), and `None` is silently skipped. Thus an invalid declared recipe type is accepted instead of producing a model/schema error. This is the acceptance-blocking finding.
- Recipe expansion occurs before reference resolution (`src/lib.rs:155-169`). Reference resolution checks supported inputs, parameters, node IDs/output names, state references, and scalar literals (`src/lib.rs:724-813`).
- A `petgraph::DiGraph` is built from node-output references; `toposort` detects cycles and produces ordered `Program.nodes` (`src/lib.rs:427-455`).
- Operator argument checking uses frozen `operator_descriptor` signatures plus a compiler-local scalar config schema (`src/lib.rs:244-319`, `702-721`). `muse-types` descriptors remain unchanged.
- CEL source is validated through public `cel::Program::compile` and preserved in `CompiledExpressionHandle::from_source` (`src/lib.rs:397-407`). No runtime operators are invoked.
- The requested broad pipeline is materially represented, but invalid declared recipe port types pass through its model-validation stage; verdict therefore fails.

## Recipe expansion

- `expand_nodes` recursively traverses recipe node maps, substitutes input and parameter references, qualifies internal node references by use-site namespace, and inserts expanded ordinary nodes (`src/lib.rs:491-659`).
- It binds every declared input and rejects missing/extra input names; it binds declared parameters from use-site values or defaults and rejects missing/extra parameter names (`src/lib.rs:525-595`). Declared input type checking only runs for recognized type strings, as described above.
- Recipe output aliases are installed compiler-locally and expanded recursively during resolution (`src/lib.rs:610-615`, `766-777`, `815-842`). Recipe recursion is rejected using an inclusion stack (`src/lib.rs:509-516`, `596-609`).
- `valid-recipe-inclusion.yaml` includes `smoother` twice. Its snapshot shows distinct `first.filtered` and `second.filtered` nodes, with the latter dependent on the former; this is evidence of real expansion and substitution, not only recursion checking.
- The fixture test recompiles the inclusion fixture and compares debug representations, then checks the stable node IDs and dependency reference (`src/lib.rs:979-991`).

## Type/CEL/error review

- The enforced reference forms are `$input.<name>`, `$param.<name>`, `$node.<node_id>.<output>`, and `$state.<field_name>` (`src/lib.rs:734-813`); unsupported references fail rather than being interpreted as scripts. Scalar numeric literals are accepted where scalar values are expected.
- `$param` misses produce `UnknownParameter`, separately from `UnknownReference` (`src/lib.rs:746-756`).
- `pointwise` expression inputs are checked as `ScalarField`; `vector_expr` inputs as `VectorField`; expression params are checked as `Scalar` (`src/lib.rs:350-365`, `367-395`). Shared input/parameter binding names are rejected (`src/lib.rs:320-330`). No implicit coercion path was found.
- Scalar configuration keys are declared in `config_arguments`; required entries are checked for presence and accepted entries are checked as scalar (`src/lib.rs:246-300`, `311-319`, `702-721`). Unknown keys fail as `UnknownArgument`. Descriptor-backed required arguments are checked separately.
- All required categories are defined and exercised through the fixture/snapshot harness: `UnknownOperator`, `UnknownReference`, `DuplicateNode`, `MissingArgument`, `UnknownArgument`, `UnknownParameter`, `TypeMismatch`, `RecipeRecursion`, `DependencyCycle`, `InvalidCel`, and `MalformedYaml`.
- Error snapshots contain category and YAML path, and where relevant expected/actual types or offending names. Malformed YAML reports parser location rather than a YAML key path, as expected for a syntax error.
- CEL compilation is syntax/compile validation using the public API; validated source text, not a CEL parser object, is stored in frozen IR. No runtime operator execution was observed.

## Commands executed

- `nix develop --command cargo check -p muse-spec` — passed (finished `dev` profile successfully; the first parallel invocation waited on a shared build lock, then completed).
- `nix develop --command cargo nextest run -p muse-spec` — passed, 2 tests.
- `nix develop --command cargo clippy -p muse-spec --all-targets -- -D warnings` — passed.
- `nix develop --command cargo fmt --all -- --check` — passed.
- `git diff --check` — passed.
- `git status --short --branch` — captured before and after checks; after checks showed only resolution-generated `M Cargo.lock`.

## Fixtures/snapshots

- Required original fixture coverage is present: `valid-minimal`, `valid-pointwise`, `error-unknown-op`, `error-unknown-ref`, `error-duplicate-node`, `error-missing-arg`, `error-unknown-arg`, `error-type-mismatch`, `error-recipe-recursion`, `error-cycle`, `error-invalid-cel`, and `error-malformed-yaml`.
- Additional fixtures cover unknown parameters, missing scalar config, duplicate expression binding names, invalid expression binding type, recipe input binding type, and valid multiple-use recipe inclusion. Each YAML fixture has an Insta snapshot through `fixture_results_are_snapshot_stable`.
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
- [ ] Validate every declared recipe input type and reject unsupported recipe type spellings. Currently unsupported values are silently skipped at `src/lib.rs:544-551` via `parse_type` (`src/lib.rs:895-906`).
- [x] Required Cargo/Nix and diff checks pass.

## Findings

1. **FAIL — invalid recipe input type declarations are accepted.** In `crates/muse-spec/src/lib.rs:544-551`, recipe binding checks are added only under `if let Some(expected_type) = parse_type(expected)`. `parse_type` returns `None` for an unsupported type string (`src/lib.rs:895-906`), and the compiler continues without an error. For example, changing `fixtures/specs/valid-recipe-inclusion.yaml`'s `recipes.smoother.inputs.field` value from `scalar_field` to `scalar_filed` leaves the type-check list empty for that binding and allows compilation to continue. This contradicts the required schema/model validation and allows misspelled port declarations to bypass the compiler's recipe binding type contract. Verdict: FAIL pending rejection of unsupported recipe declared types and fixture/snapshot coverage.

## Coordinator notes

- No Lific/tracker access was performed.
- The `Cargo.lock` modification is generated during approved leaf dependency resolution and is intentionally excluded from this commit; coordinate lockfile integration separately.
- No implementation repair was made. The one model-validation gap above requires an implementation follow-up before acceptance.
