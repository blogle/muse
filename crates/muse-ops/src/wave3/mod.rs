//! Per-family Wave 3 dispatch seams. Implementations are owned by the corresponding files.
use crate::OperatorError;

pub mod boundary_normal;
pub mod boundary_tangential;
pub mod divergence;
pub mod equilibrium;
pub mod region_vector;
pub mod vector_algebra;

pub const STUB_OPERATOR_IDS: &[&str] = &[
    "region_vector",
    "boundary_normal_component",
    "boundary_tangential_component",
    "divergence",
    "vector_add",
    "vector_subtract",
    "scalar_vector_multiply",
];

pub(crate) fn not_implemented(operator_id: &str) -> OperatorError {
    OperatorError::NotImplemented(operator_id.to_owned())
}
