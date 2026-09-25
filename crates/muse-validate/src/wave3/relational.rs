//! Generic cross-field association and paired-response metrics.

use thiserror::Error;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Association {
    /// Pearson correlation, or `None` when either selected field has zero variance.
    pub correlation: Option<f64>,
    pub samples: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExpectedDirection {
    Positive,
    Negative,
    Null,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResponseRelationship {
    pub samples: usize,
    pub mean_cause: f64,
    pub mean_response: f64,
    pub covariance: f64,
    pub correlation: Option<f64>,
    /// Least-squares response-per-cause slope; absent for zero cause variance.
    pub slope: Option<f64>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum RelationalError {
    #[error("input lengths differ")]
    LengthMismatch,
    #[error("no samples selected")]
    EmptySelection,
    #[error("input contains a nonfinite value")]
    NonFinite,
    #[error("association tolerance must be finite and nonnegative")]
    InvalidTolerance,
}

/// Compute masked Pearson association in stable input order.
/// A conditional subset is represented by the mask selecting the observations
/// satisfying the condition; omitting it includes every observation.
pub fn association(
    a: &[f64],
    b: &[f64],
    mask: Option<&[bool]>,
) -> Result<Association, RelationalError> {
    let selected = selected_pairs(a, b, mask)?;
    let samples = selected.len();
    let (ma, mb) = means(&selected);
    let mut aa = 0.0;
    let mut bb = 0.0;
    let mut ab = 0.0;
    for &(x, y) in &selected {
        let (dx, dy) = (x - ma, y - mb);
        aa += dx * dx;
        bb += dy * dy;
        ab += dx * dy;
    }
    if !ma.is_finite() || !mb.is_finite() || !aa.is_finite() || !bb.is_finite() || !ab.is_finite() {
        return Err(RelationalError::NonFinite);
    }
    let correlation = if aa == 0.0 || bb == 0.0 {
        None
    } else {
        Some((ab / (aa * bb).sqrt()).clamp(-1.0, 1.0))
    };
    Ok(Association {
        correlation,
        samples,
    })
}

/// Evaluate whether an association has the requested sign. Null means its
/// magnitude is at most `tolerance`; undefined (zero-variance) is null.
pub fn expected_direction(
    association: Association,
    expected: ExpectedDirection,
    tolerance: f64,
) -> Result<bool, RelationalError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(RelationalError::InvalidTolerance);
    }
    Ok(match (association.correlation, expected) {
        (None, ExpectedDirection::Null) => true,
        (None, _) => false,
        (Some(value), ExpectedDirection::Positive) => value > tolerance,
        (Some(value), ExpectedDirection::Negative) => value < -tolerance,
        (Some(value), ExpectedDirection::Null) => value.abs() <= tolerance,
    })
}

/// Summarize how paired cause deltas relate to response deltas.
pub fn response_relationship(
    cause_delta: &[f64],
    response_delta: &[f64],
    mask: Option<&[bool]>,
) -> Result<ResponseRelationship, RelationalError> {
    let selected = selected_pairs(cause_delta, response_delta, mask)?;
    let samples = selected.len();
    let (mean_cause, mean_response) = means(&selected);
    let mut cause_ss = 0.0;
    let mut response_ss = 0.0;
    let mut cross = 0.0;
    for &(cause, response) in &selected {
        let dc = cause - mean_cause;
        let dr = response - mean_response;
        cause_ss += dc * dc;
        response_ss += dr * dr;
        cross += dc * dr;
    }
    if !mean_cause.is_finite()
        || !mean_response.is_finite()
        || !cause_ss.is_finite()
        || !response_ss.is_finite()
        || !cross.is_finite()
    {
        return Err(RelationalError::NonFinite);
    }
    let covariance = cross / samples as f64;
    let correlation = if cause_ss == 0.0 || response_ss == 0.0 {
        None
    } else {
        Some((cross / (cause_ss * response_ss).sqrt()).clamp(-1.0, 1.0))
    };
    let slope = (cause_ss != 0.0).then(|| cross / cause_ss);
    if !covariance.is_finite()
        || correlation.is_some_and(|value| !value.is_finite())
        || slope.is_some_and(|value| !value.is_finite())
    {
        return Err(RelationalError::NonFinite);
    }
    Ok(ResponseRelationship {
        samples,
        mean_cause,
        mean_response,
        covariance,
        correlation,
        slope,
    })
}

fn selected_pairs(
    a: &[f64],
    b: &[f64],
    mask: Option<&[bool]>,
) -> Result<Vec<(f64, f64)>, RelationalError> {
    if a.len() != b.len() || mask.is_some_and(|m| m.len() != a.len()) {
        return Err(RelationalError::LengthMismatch);
    }
    if a.iter().chain(b).any(|value| !value.is_finite()) {
        return Err(RelationalError::NonFinite);
    }
    let selected: Vec<_> = a
        .iter()
        .zip(b)
        .enumerate()
        .filter(|(i, _)| mask.is_none_or(|m| m[*i]))
        .map(|(_, (&x, &y))| (x, y))
        .collect();
    if selected.is_empty() {
        return Err(RelationalError::EmptySelection);
    }
    Ok(selected)
}

fn means(values: &[(f64, f64)]) -> (f64, f64) {
    let (mut a, mut b) = (0.0, 0.0);
    for &(x, y) in values {
        a += x;
        b += y;
    }
    let count = values.len() as f64;
    (a / count, b / count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_negative_and_null_associations_are_deterministic() {
        let x = [1.0, 2.0, 3.0, 4.0];
        let positive = association(&x, &[2.0, 4.0, 6.0, 8.0], None).unwrap();
        let negative = association(&x, &[8.0, 6.0, 4.0, 2.0], None).unwrap();
        let null = association(&x, &[1.0, -1.0, -1.0, 1.0], None).unwrap();
        assert_eq!(positive.correlation, Some(1.0));
        assert_eq!(negative.correlation, Some(-1.0));
        assert_eq!(null.correlation, Some(0.0));
        assert!(expected_direction(positive, ExpectedDirection::Positive, 0.0).unwrap());
        assert!(expected_direction(negative, ExpectedDirection::Negative, 0.0).unwrap());
        assert!(expected_direction(null, ExpectedDirection::Null, 1e-12).unwrap());
        assert_eq!(
            positive,
            association(&x, &[2.0, 4.0, 6.0, 8.0], None).unwrap()
        );
    }

    #[test]
    fn masks_support_conditionals_and_response_metrics() {
        let mask = [true, true, false, false];
        let result =
            association(&[1.0, 2.0, 3.0, 4.0], &[3.0, 6.0, 0.0, -1.0], Some(&mask)).unwrap();
        assert_eq!(result.samples, 2);
        assert_eq!(result.correlation, Some(1.0));
        let response =
            response_relationship(&[1.0, 2.0, 3.0, 4.0], &[2.0, 4.0, 6.0, 8.0], Some(&mask))
                .unwrap();
        assert_eq!(response.correlation, Some(1.0));
        assert_eq!(response.slope, Some(2.0));
        assert_eq!(response.samples, 2);
    }

    #[test]
    fn zero_variance_and_invalid_inputs_are_reported() {
        assert_eq!(
            association(&[1.0, 1.0], &[2.0, 3.0], None)
                .unwrap()
                .correlation,
            None
        );
        assert_eq!(
            response_relationship(&[1.0, 1.0], &[2.0, 3.0], None)
                .unwrap()
                .slope,
            None
        );
        assert_eq!(
            association(&[1.0], &[1.0, 2.0], None),
            Err(RelationalError::LengthMismatch)
        );
        assert_eq!(
            association(&[1.0], &[1.0], Some(&[])),
            Err(RelationalError::LengthMismatch)
        );
        assert_eq!(
            association(&[f64::NAN], &[1.0], None),
            Err(RelationalError::NonFinite)
        );
        assert_eq!(
            association(&[1.0], &[1.0], Some(&[false])),
            Err(RelationalError::EmptySelection)
        );
    }
}
