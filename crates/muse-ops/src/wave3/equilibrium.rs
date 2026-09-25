//! Bounded fixed-point iteration and generic equilibrium diagnostics.

/// Summary of a bounded equilibrium solve.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EquilibriumDiagnostics {
    pub converged: bool,
    pub iterations: usize,
    pub final_delta: f64,
}

/// Field-delta norm used to decide whether an iteration has converged.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ConvergenceCriterion {
    /// Root mean square of element-wise changes.
    Rms { tolerance: f64 },
    /// Largest absolute element-wise change.
    Max { tolerance: f64 },
}

impl ConvergenceCriterion {
    fn tolerance(self) -> f64 {
        match self {
            Self::Rms { tolerance } | Self::Max { tolerance } => tolerance,
        }
    }

    fn measure(self, previous: &[f64], next: &[f64]) -> f64 {
        match self {
            Self::Rms { .. } => {
                let sum_squares = previous.iter().zip(next).fold(0.0, |sum, (a, b)| {
                    let delta = (b - a).abs();
                    sum + delta * delta
                });
                (sum_squares / previous.len() as f64).sqrt()
            }
            Self::Max { .. } => previous
                .iter()
                .zip(next)
                .map(|(a, b)| (b - a).abs())
                .fold(0.0, f64::max),
        }
    }
}

/// Input or update failure during a fixed-point solve.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EquilibriumError {
    InvalidTolerance,
    ShapeMismatch { expected: usize, actual: usize },
    NonFiniteValue,
    NonFiniteDelta,
}

/// Apply a deterministic update until its field delta meets `criterion` or
/// `hard_cap` updates have been performed. The callback returns the next field;
/// no scheduling or reduction parallelism is introduced by this routine.
pub fn solve_fixed_point<F>(
    initial: &[f64],
    hard_cap: usize,
    criterion: ConvergenceCriterion,
    mut update: F,
) -> Result<(Vec<f64>, EquilibriumDiagnostics), EquilibriumError>
where
    F: FnMut(&[f64]) -> Vec<f64>,
{
    let tolerance = criterion.tolerance();
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(EquilibriumError::InvalidTolerance);
    }
    ensure_finite(initial)?;

    let mut current = initial.to_vec();
    let mut diagnostics = EquilibriumDiagnostics {
        converged: false,
        iterations: 0,
        final_delta: 0.0,
    };

    for _ in 0..hard_cap {
        let next = update(&current);
        if next.len() != current.len() {
            return Err(EquilibriumError::ShapeMismatch {
                expected: current.len(),
                actual: next.len(),
            });
        }
        ensure_finite(&next)?;

        let delta = criterion.measure(&current, &next);
        if !delta.is_finite() {
            return Err(EquilibriumError::NonFiniteDelta);
        }
        diagnostics.iterations += 1;
        diagnostics.final_delta = delta;
        diagnostics.converged = delta <= tolerance;
        current = next;
        if diagnostics.converged {
            break;
        }
    }

    Ok((current, diagnostics))
}

fn ensure_finite(field: &[f64]) -> Result<(), EquilibriumError> {
    if field.iter().all(|value| value.is_finite()) {
        Ok(())
    } else {
        Err(EquilibriumError::NonFiniteValue)
    }
}

/// Check that diagnostics are finite, nonnegative, and within the caller's cap.
pub fn validate_diagnostics(value: EquilibriumDiagnostics, hard_cap: usize) -> bool {
    value.iterations <= hard_cap && value.final_delta.is_finite() && value.final_delta >= 0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converges_and_is_repeatable() {
        let run = || {
            solve_fixed_point(
                &[0.0, 0.0],
                20,
                ConvergenceCriterion::Rms { tolerance: 0.01 },
                |x| x.iter().map(|value| value * 0.5 + 1.0).collect(),
            )
            .unwrap()
        };
        let first = run();
        assert_eq!(first, run());
        assert!(first.1.converged);
        assert!(first.1.iterations <= 20);
        assert!(first.1.final_delta <= 0.01);
    }

    #[test]
    fn cap_reports_bounded_non_convergence() {
        let (field, diagnostics) = solve_fixed_point(
            &[0.0],
            3,
            ConvergenceCriterion::Max { tolerance: 0.0 },
            |x| vec![x[0] + 1.0],
        )
        .unwrap();
        assert_eq!(field, vec![3.0]);
        assert_eq!(diagnostics.iterations, 3);
        assert_eq!(diagnostics.final_delta, 1.0);
        assert!(!diagnostics.converged);
        assert!(validate_diagnostics(diagnostics, 3));
        assert!(!validate_diagnostics(diagnostics, 2));
    }

    #[test]
    fn rms_and_max_deltas_are_correct() {
        let (_, rms) = solve_fixed_point(
            &[0.0, 0.0],
            1,
            ConvergenceCriterion::Rms { tolerance: 0.0 },
            |_| vec![3.0, 4.0],
        )
        .unwrap();
        let (_, max) = solve_fixed_point(
            &[0.0, 0.0],
            1,
            ConvergenceCriterion::Max { tolerance: 0.0 },
            |_| vec![3.0, 4.0],
        )
        .unwrap();
        assert_eq!(rms.final_delta, 12.5_f64.sqrt());
        assert_eq!(max.final_delta, 4.0);
    }

    #[test]
    fn rejects_bad_shapes_and_nonfinite_fields() {
        assert_eq!(
            solve_fixed_point(
                &[1.0],
                2,
                ConvergenceCriterion::Max { tolerance: 1.0 },
                |_| vec![]
            ),
            Err(EquilibriumError::ShapeMismatch {
                expected: 1,
                actual: 0
            })
        );
        assert_eq!(
            solve_fixed_point(
                &[f64::NAN],
                1,
                ConvergenceCriterion::Max { tolerance: 1.0 },
                |_| vec![]
            ),
            Err(EquilibriumError::NonFiniteValue)
        );
        assert_eq!(
            solve_fixed_point(
                &[0.0],
                1,
                ConvergenceCriterion::Max { tolerance: 1.0 },
                |_| vec![f64::INFINITY]
            ),
            Err(EquilibriumError::NonFiniteValue)
        );
    }

    #[test]
    fn enforces_zero_cap_without_calling_update() {
        let (field, diagnostics) = solve_fixed_point(
            &[2.0],
            0,
            ConvergenceCriterion::Max { tolerance: 0.0 },
            |_| panic!("update must not run at a zero cap"),
        )
        .unwrap();
        assert_eq!(field, vec![2.0]);
        assert_eq!(diagnostics.iterations, 0);
        assert!(!diagnostics.converged);
    }
}
