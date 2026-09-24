//! Frozen shared data, compiler IR, and operator descriptor contracts.

use std::collections::BTreeMap;

use glam::DVec3;
use serde::{Deserialize, Serialize};

pub type CellId = u32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Mesh {
    pub positions: Vec<DVec3>,
    pub triangles: Vec<[CellId; 3]>,
    pub neighbors: Vec<Vec<CellId>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Field {
    Scalar(Vec<f64>),
    Vector(Vec<DVec3>),
    Bool(Vec<bool>),
    Category(Vec<u32>),
    Index(Vec<u32>),
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Network {
    pub edges: Vec<[CellId; 2]>,
    pub values: Option<Vec<f64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct WorldState {
    pub mesh: Mesh,
    pub fields: BTreeMap<String, Field>,
    pub networks: BTreeMap<String, Network>,
    pub parameters: BTreeMap<String, f64>,
    pub step: u64,
    pub seed: u64,
}

pub type NodeId = String;
pub type OperatorId = String;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum ValueType {
    Scalar,
    ScalarField,
    VectorField,
    BoolField,
    CategoryField,
    IndexField,
    Network,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum ValueRef {
    Input(String),
    Parameter(String),
    NodeOutput { node: NodeId, output: String },
    State(String),
    LiteralScalar(f64),
}

/// Opaque CEL source handle. `muse-spec` validates the source with `cel::Program::compile`;
/// `muse-ops` resolves it through `source()` and creates the executable `cel::Program`.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompiledExpressionHandle(String);

impl CompiledExpressionHandle {
    pub fn from_source(source: impl Into<String>) -> Self {
        Self(source.into())
    }

    pub fn source(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CompiledNode {
    pub id: NodeId,
    pub op: OperatorId,
    pub args: BTreeMap<String, ValueRef>,
    pub cel: Vec<CompiledExpressionHandle>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct StateUpdate {
    pub field: String,
    pub value: ValueRef,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Program {
    pub nodes: Vec<CompiledNode>,
    pub updates: Vec<StateUpdate>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PortSpec {
    pub name: &'static str,
    pub ty: ValueType,
    pub required: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OperatorDescriptor {
    pub id: &'static str,
    pub inputs: &'static [PortSpec],
    pub outputs: &'static [PortSpec],
}

const NO_PORTS: &[PortSpec] = &[];
const FIELD_IN: &[PortSpec] = &[PortSpec {
    name: "field",
    ty: ValueType::ScalarField,
    required: true,
}];
const FIELD_OUT: &[PortSpec] = &[PortSpec {
    name: "value",
    ty: ValueType::ScalarField,
    required: true,
}];
const BOOL_OUT: &[PortSpec] = &[PortSpec {
    name: "value",
    ty: ValueType::BoolField,
    required: true,
}];
const VECTOR_BINARY_IN: &[PortSpec] = &[
    PortSpec {
        name: "a",
        ty: ValueType::VectorField,
        required: true,
    },
    PortSpec {
        name: "b",
        ty: ValueType::VectorField,
        required: true,
    },
];
const VECTOR_OUT: &[PortSpec] = &[PortSpec {
    name: "value",
    ty: ValueType::VectorField,
    required: true,
}];
const BOOL_IN: &[PortSpec] = &[PortSpec {
    name: "mask",
    ty: ValueType::BoolField,
    required: true,
}];
const CATEGORY_IN: &[PortSpec] = &[PortSpec {
    name: "labels",
    ty: ValueType::CategoryField,
    required: true,
}];
const INDEX_OUT: &[PortSpec] = &[PortSpec {
    name: "value",
    ty: ValueType::IndexField,
    required: true,
}];
const NETWORK_OUT: &[PortSpec] = &[PortSpec {
    name: "value",
    ty: ValueType::Network,
    required: true,
}];
const POTENTIAL_IN: &[PortSpec] = &[PortSpec {
    name: "potential",
    ty: ValueType::ScalarField,
    required: true,
}];
const VALUES_IN: &[PortSpec] = &[PortSpec {
    name: "values",
    ty: ValueType::ScalarField,
    required: true,
}];
const RECEIVERS_IN: &[PortSpec] = &[
    PortSpec {
        name: "values",
        ty: ValueType::ScalarField,
        required: true,
    },
    PortSpec {
        name: "receivers",
        ty: ValueType::IndexField,
        required: true,
    },
];
const ADVECT_IN: &[PortSpec] = &[
    PortSpec {
        name: "field",
        ty: ValueType::ScalarField,
        required: true,
    },
    PortSpec {
        name: "velocity",
        ty: ValueType::VectorField,
        required: true,
    },
];

/// Stable Wave 1 descriptor list. Expression-driven named/variadic bindings for
/// pointwise and vector_expr use expression-binding schemas, not fixed ports.
/// Scalar configuration values (for example rate, iterations, threshold, scale,
/// and count) are outside this ValueType contract and are validated as scalar
/// compiler arguments rather than descriptor ports.
pub static OPERATOR_DESCRIPTORS: &[OperatorDescriptor] = &[
    OperatorDescriptor {
        id: "constant",
        inputs: NO_PORTS,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "noise",
        inputs: NO_PORTS,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "correlated_noise",
        inputs: NO_PORTS,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "smooth_radius",
        inputs: FIELD_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "voronoi_labels",
        inputs: NO_PORTS,
        outputs: &[PortSpec {
            name: "value",
            ty: ValueType::CategoryField,
            required: true,
        }],
    },
    OperatorDescriptor {
        id: "pointwise",
        inputs: NO_PORTS,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "vector_expr",
        inputs: NO_PORTS,
        outputs: VECTOR_OUT,
    },
    OperatorDescriptor {
        id: "neighbor_sample",
        inputs: FIELD_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "gradient",
        inputs: FIELD_IN,
        outputs: VECTOR_OUT,
    },
    OperatorDescriptor {
        id: "laplacian",
        inputs: FIELD_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "diffuse",
        inputs: FIELD_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "advect",
        inputs: ADVECT_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "boundary_strength",
        inputs: CATEGORY_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "distance_to_mask",
        inputs: BOOL_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "flow_direction",
        inputs: POTENTIAL_IN,
        outputs: INDEX_OUT,
    },
    OperatorDescriptor {
        id: "accumulate",
        inputs: RECEIVERS_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "network_threshold",
        inputs: FIELD_IN,
        outputs: NETWORK_OUT,
    },
    OperatorDescriptor {
        id: "reduce",
        inputs: VALUES_IN,
        outputs: &[PortSpec {
            name: "value",
            ty: ValueType::Scalar,
            required: true,
        }],
    },
    OperatorDescriptor {
        id: "threshold",
        inputs: FIELD_IN,
        outputs: BOOL_OUT,
    },
    OperatorDescriptor {
        id: "vector_dot",
        inputs: VECTOR_BINARY_IN,
        outputs: FIELD_OUT,
    },
    OperatorDescriptor {
        id: "vector_magnitude",
        inputs: &[PortSpec {
            name: "field",
            ty: ValueType::VectorField,
            required: true,
        }],
        outputs: FIELD_OUT,
    },
];

pub fn operator_descriptor(id: &str) -> Option<&'static OperatorDescriptor> {
    OPERATOR_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_snapshots_deserialize_and_have_valid_references() {
        for (path, expected_step) in [
            ("../../fixtures/snapshots/canonical-small.json", 0),
            ("../../fixtures/snapshots/canonical-small-step1.json", 1),
        ] {
            let data = std::fs::read_to_string(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(path),
            )
            .unwrap();
            let state: WorldState = serde_json::from_str(&data).unwrap();
            assert_eq!(state.step, expected_step);
            let count = state.mesh.positions.len();
            assert!(!state.mesh.positions.is_empty());
            assert!(
                state
                    .mesh
                    .triangles
                    .iter()
                    .flatten()
                    .all(|&id| (id as usize) < count)
            );
            assert!(
                state
                    .mesh
                    .neighbors
                    .iter()
                    .flatten()
                    .all(|&id| (id as usize) < count)
            );
            let rivers = &state.networks["rivers"];
            assert!(
                rivers
                    .edges
                    .iter()
                    .flatten()
                    .all(|&id| (id as usize) < count)
            );
            assert_eq!(rivers.values.as_ref().unwrap().len(), rivers.edges.len());
            assert_eq!(state.mesh.neighbors.len(), count);
            for name in ["elevation", "precipitation"] {
                match &state.fields[name] {
                    Field::Scalar(values) => assert_eq!(values.len(), count),
                    _ => panic!("{name} must be scalar"),
                }
            }
            match &state.fields["wind"] {
                Field::Vector(values) => assert_eq!(values.len(), count),
                _ => panic!("wind must be vector"),
            }
        }
    }

    #[test]
    fn registry_is_exact_and_unique() {
        let required = [
            "constant",
            "noise",
            "correlated_noise",
            "smooth_radius",
            "voronoi_labels",
            "pointwise",
            "vector_expr",
            "neighbor_sample",
            "gradient",
            "laplacian",
            "diffuse",
            "advect",
            "boundary_strength",
            "distance_to_mask",
            "flow_direction",
            "accumulate",
            "network_threshold",
            "reduce",
            "threshold",
            "vector_dot",
            "vector_magnitude",
        ];
        let ids: Vec<_> = OPERATOR_DESCRIPTORS
            .iter()
            .map(|descriptor| descriptor.id)
            .collect();
        assert_eq!(ids, required);
        assert_eq!(
            ids.iter().collect::<std::collections::BTreeSet<_>>().len(),
            ids.len()
        );
        assert!(required.iter().all(|id| operator_descriptor(id).is_some()));
    }

    #[test]
    fn critical_operator_ports_match_wave1_semantics() {
        let port = |operator: &str, name: &str| {
            operator_descriptor(operator)
                .unwrap()
                .inputs
                .iter()
                .find(|port| port.name == name)
                .copied()
                .unwrap()
        };
        assert_eq!(port("accumulate", "receivers").ty, ValueType::IndexField);
        assert_eq!(port("accumulate", "values").ty, ValueType::ScalarField);
        assert_eq!(port("advect", "velocity").ty, ValueType::VectorField);
        assert_eq!(
            port("flow_direction", "potential").ty,
            ValueType::ScalarField
        );
        assert_eq!(operator_descriptor("pointwise").unwrap().inputs, NO_PORTS);
        assert_eq!(operator_descriptor("vector_expr").unwrap().inputs, NO_PORTS);
        assert_eq!(
            operator_descriptor("threshold").unwrap().inputs[0].ty,
            ValueType::ScalarField
        );
        assert_eq!(
            operator_descriptor("threshold").unwrap().outputs[0].ty,
            ValueType::BoolField
        );
        assert_eq!(
            operator_descriptor("vector_dot").unwrap().inputs,
            VECTOR_BINARY_IN
        );
        assert_eq!(
            operator_descriptor("vector_dot").unwrap().outputs[0].ty,
            ValueType::ScalarField
        );
        assert_eq!(
            operator_descriptor("vector_magnitude").unwrap().inputs[0].ty,
            ValueType::VectorField
        );
        assert_eq!(
            operator_descriptor("vector_magnitude").unwrap().outputs[0].ty,
            ValueType::ScalarField
        );
    }

    #[test]
    fn contracts_round_trip_json() {
        let program = Program {
            nodes: vec![CompiledNode {
                id: "n".into(),
                op: "constant".into(),
                args: BTreeMap::new(),
                cel: vec![CompiledExpressionHandle::from_source("x + 1")],
            }],
            updates: vec![StateUpdate {
                field: "x".into(),
                value: ValueRef::LiteralScalar(1.0),
            }],
        };
        let encoded = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<Program>(&encoded).unwrap(), program);
        assert_eq!(program.nodes[0].cel[0].source(), "x + 1");
        let vector = Field::Vector(vec![DVec3::new(1.0, 2.0, 3.0)]);
        assert_eq!(
            serde_json::from_str::<Field>(&serde_json::to_string(&vector).unwrap()).unwrap(),
            vector
        );
    }
}
