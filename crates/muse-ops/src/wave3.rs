//! Wave 3 per-operator implementation seam.
//! Each listed contract has an explicit unsupported stub until its owning worker lands.
use crate::OperatorError;

pub const STUB_OPERATOR_IDS: &[&str] = &[
    "region_vector",
    "boundary_normal_component",
    "boundary_tangential_component",
    "divergence",
    "vector_add",
    "vector_subtract",
    "scalar_vector_multiply",
];

pub fn not_implemented(operator_id: &str) -> OperatorError {
    OperatorError::NotImplemented(operator_id.to_owned())
}
