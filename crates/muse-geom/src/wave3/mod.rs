//! Shared numerical conventions for Wave 3 surface-vector operators.
pub mod calibration;
pub mod reductions;
pub mod transport;

pub use transport::{
    DEGENERACY_EPSILON, TANGENCY_TOLERANCE, boundary_normal_relative_motion, parallel_transport,
    project_tangent,
};
