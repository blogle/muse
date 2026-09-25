//! Serializable metadata and provenance contracts shared by Wave 3 workers.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SemanticRole {
    Forcing,
    State,
    Derived,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RenderChannel {
    GeometryDisplacement,
    BaseMaterial,
    MaterialOverride,
    LineOverlay,
    ScalarOverlay,
    VectorOverlay,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VectorRenderMode {
    DirectionOnly,
    DirectionAndMagnitude,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FieldDisplayMetadata {
    pub semantic_role: Option<SemanticRole>,
    pub display_name: Option<String>,
    pub description: Option<String>,
    pub units: Option<String>,
    pub recommended_palette: Option<String>,
    pub zero_meaning: Option<String>,
    pub render_role: Option<RenderChannel>,
    pub vector_render_mode: Option<VectorRenderMode>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct NodeProvenance {
    pub node_id: String,
    pub operator_id: String,
    pub operator_kind: String,
    pub input_node_ids: Vec<String>,
    pub effective_args: std::collections::BTreeMap<String, String>,
    pub output_binding: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionDeclaration {
    pub parameter_substitutions: std::collections::BTreeMap<String, String>,
    pub forcing_substitutions: std::collections::BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct SnapshotIdentity {
    pub mesh_identifier: String,
    pub seed: u64,
    pub program_hash: String,
    pub effective_parameters: std::collections::BTreeMap<String, String>,
    pub canonical_build_identity: String,
    pub intervention: Option<InterventionDeclaration>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn roundtrip<T>(value: &T)
    where
        T: Serialize + for<'de> Deserialize<'de> + PartialEq + std::fmt::Debug,
    {
        let json = serde_json::to_string(value).unwrap();
        assert_eq!(&serde_json::from_str::<T>(&json).unwrap(), value);
    }
    #[test]
    fn metadata_provenance_and_run_identity_roundtrip() {
        roundtrip(&SemanticRole::Derived);
        roundtrip(&FieldDisplayMetadata {
            semantic_role: Some(SemanticRole::Derived),
            display_name: Some("field".into()),
            description: Some("description".into()),
            units: Some("u".into()),
            recommended_palette: Some("viridis".into()),
            zero_meaning: Some("none".into()),
            render_role: Some(RenderChannel::VectorOverlay),
            vector_render_mode: Some(VectorRenderMode::DirectionOnly),
        });
        roundtrip(&NodeProvenance {
            node_id: "n".into(),
            operator_id: "gradient".into(),
            operator_kind: "spatial".into(),
            input_node_ids: vec!["src".into()],
            effective_args: Default::default(),
            output_binding: "out".into(),
        });
        roundtrip(&SnapshotIdentity {
            mesh_identifier: "icosphere-l2".into(),
            seed: 7,
            program_hash: "hash".into(),
            effective_parameters: Default::default(),
            canonical_build_identity: "toolchain".into(),
            intervention: Some(InterventionDeclaration {
                parameter_substitutions: Default::default(),
                forcing_substitutions: Default::default(),
            }),
        });
    }
}
