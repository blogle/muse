//! Paired-world comparison API contract. Numerical implementations are owned by follow-up.
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedRunIdentity {
    pub seed: u64,
    pub mesh_identifier: String,
    pub baseline_program_hash: String,
    pub intervention_program_hash: String,
    pub intervention: muse_types::InterventionDeclaration,
}

/// Normative seed corpus for causal validation.
pub const WAVE3_SEEDS: [u64; 6] = [1, 7, 17, 29, 43, 71];
/// Level-five canonical spherical mesh cell count.
pub const LEVEL_FIVE_CELL_COUNT: usize = 10_242;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn paired_contract_roundtrips_and_corpus_is_frozen() {
        let metric = ComparisonMetric::InsideOutsideResponseRatio;
        let json = serde_json::to_string(&metric).unwrap();
        assert_eq!(
            serde_json::from_str::<ComparisonMetric>(&json).unwrap(),
            metric
        );
        assert_eq!(WAVE3_SEEDS, [1, 7, 17, 29, 43, 71]);
        assert_eq!(LEVEL_FIVE_CELL_COUNT, 10_242);
    }
}
