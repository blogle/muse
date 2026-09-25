//! Wave 3 paired-world validation contracts split by exclusive follow-up ownership.
pub mod comparison;
pub mod corpus;
pub mod intervention;
pub mod relational;

pub use comparison::ComparisonMetric;
pub use corpus::{LEVEL_FIVE_CELL_COUNT, WAVE3_SEEDS};
pub use intervention::PairedRunIdentity;
