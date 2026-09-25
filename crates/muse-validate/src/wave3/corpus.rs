//! Owner: canonical paired-validation seed/resolution corpus and fixtures.
use serde::{Deserialize, Serialize};

/// Normative seed corpus for causal validation.
pub const WAVE3_SEEDS: [u64; 6] = [1, 7, 17, 29, 43, 71];
/// Level-five canonical spherical mesh cell count.
pub const LEVEL_FIVE_CELL_COUNT: usize = 10_242;
/// Number of cells in the next coarser canonical icosphere resolution.
pub const LEVEL_FOUR_CELL_COUNT: usize = 2_562;
/// Current schema version for serialized corpus descriptors.
pub const CORPUS_SCHEMA_VERSION: u32 = 1;

/// A canonical mesh resolution included in the Wave 3 corpus.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CorpusResolution {
    pub level: u8,
    pub cell_count: usize,
}

/// Resolutions are ordered from coarse to fine for stable corpus enumeration.
pub const WAVE3_RESOLUTIONS: [CorpusResolution; 2] = [
    CorpusResolution {
        level: 4,
        cell_count: LEVEL_FOUR_CELL_COUNT,
    },
    CorpusResolution {
        level: 5,
        cell_count: LEVEL_FIVE_CELL_COUNT,
    },
];

/// One same-seed baseline/intervention comparison in the canonical corpus.
///
/// `case_id` is stable across runs and derived solely from the schema version,
/// resolution, and seed. The two members of a pair share all three values.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedCorpusCase {
    pub schema_version: u32,
    pub case_id: String,
    pub seed: u64,
    pub resolution: CorpusResolution,
    pub baseline: String,
    pub intervention: String,
}

/// Enumerate baseline/intervention pairs in resolution order, then seed order.
///
/// The labels are generic execution roles; this corpus does not prescribe a
/// world, intervention, or validation threshold.
pub fn canonical_paired_cases() -> Vec<PairedCorpusCase> {
    WAVE3_RESOLUTIONS
        .into_iter()
        .flat_map(|resolution| {
            WAVE3_SEEDS.into_iter().map(move |seed| PairedCorpusCase {
                schema_version: CORPUS_SCHEMA_VERSION,
                case_id: format!(
                    "wave3-v{}-l{}-seed{}",
                    CORPUS_SCHEMA_VERSION, resolution.level, seed
                ),
                seed,
                resolution,
                baseline: "baseline".to_owned(),
                intervention: "intervention".to_owned(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_corpus_is_frozen() {
        assert_eq!(WAVE3_SEEDS, [1, 7, 17, 29, 43, 71]);
        assert_eq!(LEVEL_FIVE_CELL_COUNT, 10_242);
        assert_eq!(LEVEL_FOUR_CELL_COUNT, 2_562);
    }

    #[test]
    fn paired_cases_are_stable_unique_and_cover_each_resolution_and_seed() {
        let cases = canonical_paired_cases();
        assert_eq!(cases.len(), WAVE3_SEEDS.len() * WAVE3_RESOLUTIONS.len());
        let ids: std::collections::BTreeSet<_> =
            cases.iter().map(|case| case.case_id.as_str()).collect();
        assert_eq!(ids.len(), cases.len());

        for (resolution_index, resolution) in WAVE3_RESOLUTIONS.iter().enumerate() {
            let start = resolution_index * WAVE3_SEEDS.len();
            let resolution_cases = &cases[start..start + WAVE3_SEEDS.len()];
            assert!(
                resolution_cases
                    .iter()
                    .all(|case| case.resolution == *resolution)
            );
            assert_eq!(
                resolution_cases
                    .iter()
                    .map(|case| case.seed)
                    .collect::<Vec<_>>(),
                WAVE3_SEEDS
            );
            assert!(
                resolution_cases
                    .iter()
                    .all(|case| case.baseline == "baseline" && case.intervention == "intervention")
            );
        }
        assert_eq!(cases, canonical_paired_cases());
    }

    #[test]
    fn case_descriptors_roundtrip_as_versioned_json() {
        let cases = canonical_paired_cases();
        let encoded = serde_json::to_string(&cases).unwrap();
        let decoded: Vec<PairedCorpusCase> = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, cases);
        assert!(
            decoded
                .iter()
                .all(|case| case.schema_version == CORPUS_SCHEMA_VERSION)
        );
    }
}
