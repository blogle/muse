//! Owner: canonical paired-validation seed/resolution corpus and fixtures.

/// Normative seed corpus for causal validation.
pub const WAVE3_SEEDS: [u64; 6] = [1, 7, 17, 29, 43, 71];
/// Level-five canonical spherical mesh cell count.
pub const LEVEL_FIVE_CELL_COUNT: usize = 10_242;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_corpus_is_frozen() {
        assert_eq!(WAVE3_SEEDS, [1, 7, 17, 29, 43, 71]);
        assert_eq!(LEVEL_FIVE_CELL_COUNT, 10_242);
    }
}
