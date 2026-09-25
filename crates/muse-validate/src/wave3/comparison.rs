//! Owner: paired scalar comparison metrics and implementations.
use serde::{Deserialize, Serialize};

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
}
