//! Bounded, deterministic fixed-point relaxation diagnostics.

/// Field-delta statistic used to decide whether a relaxation step converged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConvergenceCriterion {
    /// Square root of the mean squared component-wise change.
    Rms,
    /// Largest absolute component-wise change.
    MaxDelta,
}

/// Result of a bounded relaxation run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquilibriumDiagnostics {
    pub converged: bool,
    pub iterations: usize,
    pub final_delta: f64,
}

/// Errors returned when a relaxation run cannot be evaluated safely.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquilibriumError {
    ShapeMismatch { current: usize, next: usize },
    NonFiniteInput,
    NonFiniteDelta,
    InvalidTolerance,
}

/// Computes the selected finite-field delta. Empty fields have a zero delta.
pub fn field_delta(
    current: &[f64],
    next: &[f64],
    criterion: ConvergenceCriterion,
) -> Result<f64, EquilibriumError> {
    if current.len() != next.len() {
        return Err(EquilibriumError::ShapeMismatch {
            current: current.len(),
            next: next.len(),
        });
    }
    if current.iter().chain(next).any(|value| !value.is_finite()) {
        return Err(EquilibriumError::NonFiniteInput);
    }

    let mut sum_squared = 0.0;
    let mut maximum: f64 = 0.0;
    for (&before, &after) in current.iter().zip(next) {
        let delta = (after - before).abs();
        if !delta.is_finite() {
            return Err(EquilibriumError::NonFiniteDelta);
        }
        sum_squared += delta * delta;
        maximum = maximum.max(delta);
    }

    let delta = match criterion {
        ConvergenceCriterion::Rms if current.is_empty() => 0.0,
        ConvergenceCriterion::Rms => (sum_squared / current.len() as f64).sqrt(),
        ConvergenceCriterion::MaxDelta => maximum,
    };
    if delta.is_finite() {
        Ok(delta)
    } else {
        Err(EquilibriumError::NonFiniteDelta)
    }
}

/// Applies `step` until its delta is within `tolerance` or `hard_cap` is reached.
/// The input slice is the initial state; the returned vector is the final state.
/// A zero hard cap performs no steps and reports the initial field as non-converged.
pub fn relax<F>(
    initial: &[f64],
    tolerance: f64,
    hard_cap: usize,
    criterion: ConvergenceCriterion,
    mut step: F,
) -> Result<(Vec<f64>, EquilibriumDiagnostics), EquilibriumError>
where
    F: FnMut(&[f64]) -> Vec<f64>,
{
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(EquilibriumError::InvalidTolerance);
    }
    if initial.iter().any(|value| !value.is_finite()) {
        return Err(EquilibriumError::NonFiniteInput);
    }

    let mut state = initial.to_vec();
    let mut diagnostics = EquilibriumDiagnostics {
        converged: false,
        iterations: 0,
        final_delta: 0.0,
    };
    for _ in 0..hard_cap {
        let next = step(&state);
        let delta = field_delta(&state, &next, criterion)?;
        diagnostics.iterations += 1;
        diagnostics.final_delta = delta;
        diagnostics.converged = delta <= tolerance;
        state = next;
        if diagnostics.converged {
            break;
        }
    }
    Ok((state, diagnostics))
}

/// Checks that a diagnostic record is valid and respects the caller's hard cap.
pub fn validate_diagnostics(value: EquilibriumDiagnostics, hard_cap: usize) -> bool {
    value.iterations <= hard_cap && value.final_delta.is_finite() && value.final_delta >= 0.0
}

#[cfg(test)]
mod tests {
    use super::{ConvergenceCriterion, EquilibriumError, field_delta, relax};

    #[test]
    fn relaxes_to_a_fixed_point_and_reports_iterations() {
        let (state, diagnostics) =
            relax(&[0.0], 0.01, 20, ConvergenceCriterion::MaxDelta, |state| {
                vec![state[0] + (1.0 - state[0]) * 0.5]
            })
            .unwrap();
        assert!(diagnostics.converged);
        assert!(diagnostics.iterations <= 20);
        assert!(diagnostics.final_delta <= 0.01);
        assert!((state[0] - 1.0).abs() <= 0.01);
    }

    #[test]
    fn reports_non_convergence_at_hard_cap() {
        let (_, diagnostics) = relax(&[0.0], 0.0, 3, ConvergenceCriterion::MaxDelta, |state| {
            vec![state[0] + 1.0]
        })
        .unwrap();
        assert!(!diagnostics.converged);
        assert_eq!(diagnostics.iterations, 3);
        assert_eq!(diagnostics.final_delta, 1.0);
    }

    #[test]
    fn computes_rms_and_max_delta() {
        let before = [0.0, 0.0, 0.0];
        let after = [3.0, 4.0, 0.0];
        assert_eq!(
            field_delta(&before, &after, ConvergenceCriterion::Rms),
            Ok(25.0_f64.sqrt() / 3.0_f64.sqrt())
        );
        assert_eq!(
            field_delta(&before, &after, ConvergenceCriterion::MaxDelta),
            Ok(4.0)
        );
    }

    #[test]
    fn empty_and_mismatched_fields_are_handled() {
        assert_eq!(field_delta(&[], &[], ConvergenceCriterion::Rms), Ok(0.0));
        assert_eq!(
            field_delta(&[1.0], &[], ConvergenceCriterion::MaxDelta),
            Err(EquilibriumError::ShapeMismatch {
                current: 1,
                next: 0
            })
        );
    }

    #[test]
    fn rejects_non_finite_values_and_enforces_cap_deterministically() {
        assert_eq!(
            field_delta(&[f64::NAN], &[0.0], ConvergenceCriterion::Rms),
            Err(EquilibriumError::NonFiniteInput)
        );
        let run = || {
            relax(&[0.0], 0.0, 2, ConvergenceCriterion::Rms, |state| {
                vec![state[0] + 1.0]
            })
            .unwrap()
        };
        let first = run();
        assert_eq!(first, run());
        assert_eq!(first.1.iterations, 2);
    }
}
