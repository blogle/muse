//! Owner: bounded equilibrium diagnostics and future iteration hooks.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquilibriumDiagnostics {
    pub converged: bool,
    pub iterations: usize,
    pub final_delta: f64,
}

/// TODO(owner: equilibrium): bounded runner registration is not part of this contract freeze.
pub fn validate_diagnostics(value: EquilibriumDiagnostics, hard_cap: usize) -> bool {
    value.iterations <= hard_cap && value.final_delta.is_finite() && value.final_delta >= 0.0
}
