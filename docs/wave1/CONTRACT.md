# Wave 1 frozen contract

Downstream workstreams MUST NOT modify these coordinator-owned definitions:

- `crates/muse-types/src/lib.rs`: `CellId`, `Mesh`, `Field`, `Network`, `WorldState`;
- `crates/muse-types/src/lib.rs`: compiler IR `NodeId`, `OperatorId`, `ValueType`, `ValueRef`, `CompiledExpressionHandle`, `CompiledNode`, `StateUpdate`, and `Program`;
- `crates/muse-types/src/lib.rs`: `PortSpec`, `OperatorDescriptor`, `OPERATOR_DESCRIPTORS`, and `operator_descriptor`;
- root `Cargo.toml` workspace membership and `[workspace.dependencies]` allowlist.

Operator descriptor ports are conservative compiler-checking signatures. `pointwise` and `vector_expr` have no fixed input ports: `muse-spec` validates named expression bindings against their CEL binding schema. Scalar configuration arguments (such as `rate`, `iterations`, `threshold`, `scale`, and `count`) are not representable by the frozen `ValueType` set and are validated by compiler argument rules, not `PortSpec`.

For CEL, W1-C validates source text with the approved `cel::Program::compile` API and stores it in `CompiledExpressionHandle::from_source`. W1-D obtains that source with `source()`, compiles it to `cel::Program`, then uses the public `execute(&cel::Context)` API. This keeps CEL parser/compiler types out of shared IR while leaving a resolvable execution path. No CEL-specific parser internals are exposed. The incremental experiment has an independent Cargo workspace and dependencies local to its manifest.
