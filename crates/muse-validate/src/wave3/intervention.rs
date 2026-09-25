//! Owner: intervention declarations, run pairing, and identity checks.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedRunIdentity {
    pub seed: u64,
    pub mesh_identifier: String,
    pub baseline_program_hash: String,
    pub intervention_program_hash: String,
    pub intervention: muse_types::InterventionDeclaration,
}
