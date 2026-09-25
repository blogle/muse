//! Owner: paired scalar comparison metrics and implementations.
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonMetric {
    Delta,
    MeanAbsoluteDelta,
    RmsDelta,
    CorrelationDeltaCauseEffect,
    SignAgreement,
    MaskedDeltaMean,
    InsideOutsideResponseRatio,
    ExactUnchangedOutsideDownstreamClosure,
}

/// Why a metric has no numerical value for otherwise valid input.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UndefinedReason {
    EmptyInput,
    EmptyMask,
    ZeroVariance,
    ZeroDenominator,
}

/// A defined scalar result or an explicit explanation why it is undefined.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case", tag = "status", content = "value")]
pub enum MetricOutcome {
    Defined(f64),
    Undefined(UndefinedReason),
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum ComparisonError {
    #[error("paired inputs have different lengths: {left} and {right}")]
    LengthMismatch { left: usize, right: usize },
    #[error("input {input} contains a non-finite value at index {index}")]
    NonFinite { input: &'static str, index: usize },
    #[error("mask length {mask} does not match data length {data}")]
    MaskLengthMismatch { mask: usize, data: usize },
}

/// A validated, deterministic pair of scalar fields.
#[derive(Clone, Debug, PartialEq)]
pub struct PairedScalarComparison {
    delta: Vec<f64>,
}

impl PairedScalarComparison {
    /// Construct intervention-minus-baseline deltas after validating both fields.
    pub fn new(baseline: &[f64], intervention: &[f64]) -> Result<Self, ComparisonError> {
        validate_pair("baseline", baseline, "intervention", intervention)?;
        Ok(Self {
            delta: baseline
                .iter()
                .zip(intervention)
                .map(|(before, after)| after - before)
                .collect(),
        })
    }

    /// Elementwise intervention-minus-baseline values, in stable input order.
    pub fn delta(&self) -> &[f64] {
        &self.delta
    }

    pub fn mean_absolute_delta(&self) -> MetricOutcome {
        mean_abs(&self.delta)
    }

    pub fn rms_delta(&self) -> MetricOutcome {
        if self.delta.is_empty() {
            return MetricOutcome::Undefined(UndefinedReason::EmptyInput);
        }
        let mean_square =
            self.delta.iter().map(|value| value * value).sum::<f64>() / self.delta.len() as f64;
        MetricOutcome::Defined(mean_square.sqrt())
    }

    /// Fraction of positions whose nonzero delta signs agree. Zero/zero agrees;
    /// zero/nonzero does not.
    pub fn sign_agreement(&self, other_delta: &[f64]) -> Result<MetricOutcome, ComparisonError> {
        validate_pair("delta", &self.delta, "other_delta", other_delta)?;
        if self.delta.is_empty() {
            return Ok(MetricOutcome::Undefined(UndefinedReason::EmptyInput));
        }
        let agreements = self
            .delta
            .iter()
            .zip(other_delta)
            .filter(|(left, right)| left.signum() == right.signum())
            .count();
        Ok(MetricOutcome::Defined(
            agreements as f64 / self.delta.len() as f64,
        ))
    }

    /// Signed mean delta for values selected by `mask`.
    pub fn masked_delta_mean(&self, mask: &[bool]) -> Result<MetricOutcome, ComparisonError> {
        validate_mask(mask, self.delta.len())?;
        let (sum, count) = self
            .delta
            .iter()
            .zip(mask)
            .filter(|(_, selected)| **selected)
            .fold((0.0, 0usize), |(sum, count), (value, _)| {
                (sum + value, count + 1)
            });
        if count == 0 {
            return Ok(MetricOutcome::Undefined(UndefinedReason::EmptyMask));
        }
        Ok(MetricOutcome::Defined(sum / count as f64))
    }

    /// Mean absolute response inside the mask divided by mean absolute response outside.
    pub fn inside_outside_response_ratio(
        &self,
        mask: &[bool],
    ) -> Result<MetricOutcome, ComparisonError> {
        validate_mask(mask, self.delta.len())?;
        let (inside, outside) = self.delta.iter().zip(mask).fold(
            ((0.0, 0usize), (0.0, 0usize)),
            |(mut inside, mut outside), (value, selected)| {
                let group = if *selected { &mut inside } else { &mut outside };
                group.0 += value.abs();
                group.1 += 1;
                (inside, outside)
            },
        );
        if inside.1 == 0 || outside.1 == 0 {
            return Ok(MetricOutcome::Undefined(UndefinedReason::EmptyMask));
        }
        let outside_mean = outside.0 / outside.1 as f64;
        if outside_mean == 0.0 {
            return Ok(MetricOutcome::Undefined(UndefinedReason::ZeroDenominator));
        }
        Ok(MetricOutcome::Defined(
            (inside.0 / inside.1 as f64) / outside_mean,
        ))
    }

    /// Whether every unmasked cell is bit-for-bit unchanged between the two fields.
    pub fn exact_unchanged_outside(
        &self,
        baseline: &[f64],
        intervention: &[f64],
        changed_region: &[bool],
    ) -> Result<bool, ComparisonError> {
        validate_pair("baseline", baseline, "intervention", intervention)?;
        validate_mask(changed_region, baseline.len())?;
        Ok(baseline
            .iter()
            .zip(intervention)
            .zip(changed_region)
            .all(|((before, after), changed)| *changed || before.to_bits() == after.to_bits()))
    }
}

/// Pearson correlation between paired cause and effect deltas.
pub fn correlation_cause_effect_deltas(
    cause_delta: &[f64],
    effect_delta: &[f64],
) -> Result<MetricOutcome, ComparisonError> {
    validate_pair("cause_delta", cause_delta, "effect_delta", effect_delta)?;
    if cause_delta.is_empty() {
        return Ok(MetricOutcome::Undefined(UndefinedReason::EmptyInput));
    }
    let count = cause_delta.len() as f64;
    let cause_mean = cause_delta.iter().sum::<f64>() / count;
    let effect_mean = effect_delta.iter().sum::<f64>() / count;
    let (covariance, cause_variance, effect_variance) = cause_delta.iter().zip(effect_delta).fold(
        (0.0, 0.0, 0.0),
        |(cov, cause_var, effect_var), (cause, effect)| {
            let cause_diff = cause - cause_mean;
            let effect_diff = effect - effect_mean;
            (
                cov + cause_diff * effect_diff,
                cause_var + cause_diff.powi(2),
                effect_var + effect_diff.powi(2),
            )
        },
    );
    if cause_variance == 0.0 || effect_variance == 0.0 {
        return Ok(MetricOutcome::Undefined(UndefinedReason::ZeroVariance));
    }
    Ok(MetricOutcome::Defined(
        covariance / (cause_variance * effect_variance).sqrt(),
    ))
}

fn mean_abs(values: &[f64]) -> MetricOutcome {
    if values.is_empty() {
        return MetricOutcome::Undefined(UndefinedReason::EmptyInput);
    }
    MetricOutcome::Defined(
        values.iter().map(|value| value.abs()).sum::<f64>() / values.len() as f64,
    )
}

fn validate_pair(
    left_name: &'static str,
    left: &[f64],
    right_name: &'static str,
    right: &[f64],
) -> Result<(), ComparisonError> {
    if left.len() != right.len() {
        return Err(ComparisonError::LengthMismatch {
            left: left.len(),
            right: right.len(),
        });
    }
    for (input, values) in [(left_name, left), (right_name, right)] {
        if let Some(index) = values.iter().position(|value| !value.is_finite()) {
            return Err(ComparisonError::NonFinite { input, index });
        }
    }
    Ok(())
}

fn validate_mask(mask: &[bool], data_len: usize) -> Result<(), ComparisonError> {
    if mask.len() != data_len {
        return Err(ComparisonError::MaskLengthMismatch {
            mask: mask.len(),
            data: data_len,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn comparison_metric_roundtrips_json() {
        let metric = ComparisonMetric::InsideOutsideResponseRatio;
        let json = serde_json::to_string(&metric).unwrap();
        assert_eq!(
            serde_json::from_str::<ComparisonMetric>(&json).unwrap(),
            metric
        );
    }

    #[test]
    fn computes_small_vector_metrics_and_is_repeatable() {
        let comparison =
            PairedScalarComparison::new(&[1.0, 2.0, 3.0, 4.0], &[2.0, 1.0, 3.0, 6.0]).unwrap();
        assert_eq!(comparison.delta(), &[1.0, -1.0, 0.0, 2.0]);
        assert_eq!(
            comparison.mean_absolute_delta(),
            MetricOutcome::Defined(1.0)
        );
        assert_eq!(
            comparison.rms_delta(),
            MetricOutcome::Defined(6.0_f64.sqrt() / 2.0)
        );
        assert_eq!(
            comparison.sign_agreement(&[1.0, -2.0, 0.0, 3.0]).unwrap(),
            MetricOutcome::Defined(1.0)
        );
        assert_eq!(
            comparison.delta(),
            PairedScalarComparison::new(&[1.0, 2.0, 3.0, 4.0], &[2.0, 1.0, 3.0, 6.0])
                .unwrap()
                .delta()
        );
    }

    #[test]
    fn computes_correlation_masks_ratio_and_exact_unchanged() {
        let comparison =
            PairedScalarComparison::new(&[0.0, 0.0, 0.0, 0.0], &[2.0, -2.0, 1.0, 1.0]).unwrap();
        assert_eq!(
            correlation_cause_effect_deltas(&[1.0, 2.0, 3.0], &[2.0, 4.0, 6.0]).unwrap(),
            MetricOutcome::Defined(1.0)
        );
        assert_eq!(
            comparison
                .masked_delta_mean(&[true, true, false, false])
                .unwrap(),
            MetricOutcome::Defined(0.0)
        );
        assert_eq!(
            comparison
                .inside_outside_response_ratio(&[true, true, false, false])
                .unwrap(),
            MetricOutcome::Defined(2.0)
        );
        assert!(
            comparison
                .exact_unchanged_outside(
                    &[0.0, 0.0, 0.0, 0.0],
                    &[2.0, -2.0, 0.0, 0.0],
                    &[true, true, false, false]
                )
                .unwrap()
        );
        assert!(
            !comparison
                .exact_unchanged_outside(&[0.0, 0.0], &[1.0, 0.0], &[false, false])
                .unwrap()
        );
    }

    #[test]
    fn reports_invalid_and_undefined_cases_explicitly() {
        assert_eq!(
            PairedScalarComparison::new(&[1.0], &[]),
            Err(ComparisonError::LengthMismatch { left: 1, right: 0 })
        );
        assert_eq!(
            PairedScalarComparison::new(&[f64::NAN], &[0.0]),
            Err(ComparisonError::NonFinite {
                input: "baseline",
                index: 0
            })
        );
        assert_eq!(
            correlation_cause_effect_deltas(&[1.0, 1.0], &[2.0, 3.0]).unwrap(),
            MetricOutcome::Undefined(UndefinedReason::ZeroVariance)
        );
        let comparison = PairedScalarComparison::new(&[0.0, 0.0], &[0.0, 0.0]).unwrap();
        assert_eq!(
            comparison.masked_delta_mean(&[false, false]).unwrap(),
            MetricOutcome::Undefined(UndefinedReason::EmptyMask)
        );
        assert_eq!(
            comparison
                .inside_outside_response_ratio(&[true, false])
                .unwrap(),
            MetricOutcome::Undefined(UndefinedReason::ZeroDenominator)
        );
        assert_eq!(
            comparison.sign_agreement(&[0.0]).unwrap_err(),
            ComparisonError::LengthMismatch { left: 2, right: 1 }
        );
        let empty = PairedScalarComparison::new(&[], &[]).unwrap();
        assert_eq!(
            empty.mean_absolute_delta(),
            MetricOutcome::Undefined(UndefinedReason::EmptyInput)
        );
        assert_eq!(
            empty.rms_delta(),
            MetricOutcome::Undefined(UndefinedReason::EmptyInput)
        );
        assert_eq!(
            empty.inside_outside_response_ratio(&[]).unwrap(),
            MetricOutcome::Undefined(UndefinedReason::EmptyMask)
        );
        assert_eq!(
            comparison.masked_delta_mean(&[true]).unwrap_err(),
            ComparisonError::MaskLengthMismatch { mask: 1, data: 2 }
        );
    }
}
