//! Paired-run declarations, identity checks, and provenance dependency closure.
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use muse_types::{InterventionDeclaration, NodeProvenance, SnapshotIdentity};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct PairedRunIdentity {
    pub seed: u64,
    pub mesh_identifier: String,
    pub baseline_program_hash: String,
    pub intervention_program_hash: String,
    pub intervention: InterventionDeclaration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InterventionVerification {
    /// Provenance node IDs whose outputs may change because of the intervention.
    pub downstream_node_ids: BTreeSet<String>,
    /// Output bindings eligible for exact-unchanged checks in deterministic runs.
    pub exact_unchanged_bindings: BTreeSet<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum InterventionError {
    #[error("intervention declaration is empty")]
    EmptyIntervention,
    #[error("{0} differs between paired runs")]
    IdentityMismatch(&'static str),
    #[error("parameter substitution for `{0}` is undeclared or has the wrong value")]
    UndeclaredParameter(String),
    #[error("parameter `{0}` is declared as changed but is unchanged or missing")]
    UnappliedParameter(String),
    #[error("forcing substitution binding `{0}` is absent from provenance")]
    MissingForcingBinding(String),
    #[error("provenance contains duplicate node id `{0}`")]
    DuplicateNode(String),
    #[error("provenance node `{node}` references missing input node `{input}`")]
    MissingNode { node: String, input: String },
    #[error("provenance dependency graph contains a cycle")]
    Cycle,
}

/// Verify the frozen run identities and return the causal node closure.
///
/// Forcing declaration values are treated as the intervention's replacement
/// values. The shared identity type does not carry forcing values from either
/// run, so this function verifies their binding and closure, not the value delta.
pub fn verify_intervention_pair(
    baseline: &SnapshotIdentity,
    intervention: &SnapshotIdentity,
    baseline_provenance: &[NodeProvenance],
    intervention_provenance: &[NodeProvenance],
) -> Result<InterventionVerification, InterventionError> {
    if baseline.seed != intervention.seed {
        return Err(InterventionError::IdentityMismatch("seed"));
    }
    if baseline.mesh_identifier != intervention.mesh_identifier {
        return Err(InterventionError::IdentityMismatch("mesh"));
    }
    if baseline.program_hash != intervention.program_hash {
        return Err(InterventionError::IdentityMismatch("program"));
    }
    if baseline.canonical_build_identity != intervention.canonical_build_identity {
        return Err(InterventionError::IdentityMismatch("canonical build"));
    }

    let declaration = intervention
        .intervention
        .as_ref()
        .ok_or(InterventionError::EmptyIntervention)?;
    if declaration.parameter_substitutions.is_empty()
        && declaration.forcing_substitutions.is_empty()
    {
        return Err(InterventionError::EmptyIntervention);
    }
    if baseline.intervention.is_some() {
        return Err(InterventionError::IdentityMismatch("baseline intervention"));
    }
    verify_parameter_substitutions(baseline, intervention, declaration)?;

    let baseline_nodes = index_and_validate(baseline_provenance)?;
    let intervention_nodes = index_and_validate(intervention_provenance)?;
    let mut starts = BTreeSet::new();
    for binding in declaration.forcing_substitutions.keys() {
        if !baseline_provenance
            .iter()
            .any(|node| node.output_binding == *binding)
            || !intervention_provenance
                .iter()
                .any(|node| node.output_binding == *binding)
        {
            return Err(InterventionError::MissingForcingBinding(binding.clone()));
        }
        starts.insert(binding.clone());
    }
    // Parameter substitutions affect any node whose effective arguments mention
    // the parameter key. Provenance arguments are serialized strings by contract.
    for parameter in declaration.parameter_substitutions.keys() {
        for node in intervention_provenance {
            if node.effective_args.contains_key(parameter) {
                starts.insert(node.output_binding.clone());
            }
        }
    }

    let downstream_node_ids = downstream_closure(intervention_provenance, &starts)?;
    let all_bindings: BTreeSet<_> = baseline_provenance
        .iter()
        .map(|node| node.output_binding.clone())
        .collect();
    let affected_bindings: BTreeSet<_> = intervention_provenance
        .iter()
        .filter(|node| downstream_node_ids.contains(&node.node_id))
        .map(|node| node.output_binding.clone())
        .collect();
    let exact_unchanged_bindings = all_bindings
        .intersection(
            &intervention_provenance
                .iter()
                .map(|node| node.output_binding.clone())
                .collect(),
        )
        .filter(|binding| !affected_bindings.contains(*binding))
        .cloned()
        .collect();
    // Keep both validated indexes live as a sanity check that provenance node
    // identities are present in each run before returning the computed closure.
    let _ = (baseline_nodes, intervention_nodes);
    Ok(InterventionVerification {
        downstream_node_ids,
        exact_unchanged_bindings,
    })
}

fn verify_parameter_substitutions(
    baseline: &SnapshotIdentity,
    intervention: &SnapshotIdentity,
    declaration: &InterventionDeclaration,
) -> Result<(), InterventionError> {
    for (name, value) in &intervention.effective_parameters {
        if baseline.effective_parameters.get(name) != Some(value)
            && declaration.parameter_substitutions.get(name) != Some(value)
        {
            return Err(InterventionError::UndeclaredParameter(name.clone()));
        }
    }
    for name in declaration.parameter_substitutions.keys() {
        if baseline.effective_parameters.get(name) == intervention.effective_parameters.get(name)
            || intervention.effective_parameters.get(name)
                != declaration.parameter_substitutions.get(name)
        {
            return Err(InterventionError::UnappliedParameter(name.clone()));
        }
    }
    // A parameter removed from the effective assignment is also a substitution.
    for name in baseline.effective_parameters.keys() {
        if !intervention.effective_parameters.contains_key(name)
            && !declaration.parameter_substitutions.contains_key(name)
        {
            return Err(InterventionError::UndeclaredParameter(name.clone()));
        }
    }
    Ok(())
}

fn index_and_validate(
    provenance: &[NodeProvenance],
) -> Result<BTreeMap<&str, &NodeProvenance>, InterventionError> {
    let mut nodes = BTreeMap::new();
    for node in provenance {
        if nodes.insert(node.node_id.as_str(), node).is_some() {
            return Err(InterventionError::DuplicateNode(node.node_id.clone()));
        }
    }
    for node in provenance {
        for input in &node.input_node_ids {
            if !nodes.contains_key(input.as_str()) {
                return Err(InterventionError::MissingNode {
                    node: node.node_id.clone(),
                    input: input.clone(),
                });
            }
        }
    }
    // Kahn's algorithm also makes cycle detection independent of input order.
    let mut indegree: BTreeMap<&str, usize> = nodes.keys().map(|id| (*id, 0)).collect();
    let mut consumers: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
    for node in provenance {
        for input in &node.input_node_ids {
            *indegree
                .get_mut(node.node_id.as_str())
                .expect("indexed node") += 1;
            consumers
                .entry(input.as_str())
                .or_default()
                .push(node.node_id.as_str());
        }
    }
    let mut ready: VecDeque<_> = indegree
        .iter()
        .filter_map(|(id, degree)| (*degree == 0).then_some(*id))
        .collect();
    let mut visited = 0;
    while let Some(id) = ready.pop_front() {
        visited += 1;
        for consumer in consumers.get(id).into_iter().flatten() {
            let degree = indegree.get_mut(consumer).expect("indexed consumer");
            *degree -= 1;
            if *degree == 0 {
                ready.push_back(consumer);
            }
        }
    }
    if visited != nodes.len() {
        return Err(InterventionError::Cycle);
    }
    Ok(nodes)
}

fn downstream_closure(
    provenance: &[NodeProvenance],
    start_bindings: &BTreeSet<String>,
) -> Result<BTreeSet<String>, InterventionError> {
    index_and_validate(provenance)?;
    let binding_nodes: BTreeMap<_, _> = provenance
        .iter()
        .map(|node| (node.output_binding.as_str(), node.node_id.as_str()))
        .collect();
    let mut closure = BTreeSet::new();
    let mut pending = VecDeque::new();
    for binding in start_bindings {
        if let Some(node_id) = binding_nodes.get(binding.as_str()) {
            pending.push_back(*node_id);
        }
    }
    while let Some(node_id) = pending.pop_front() {
        if !closure.insert(node_id.to_owned()) {
            continue;
        }
        // Validation above guarantees every queued ID is present.
        for consumer in provenance.iter().filter(|candidate| {
            candidate
                .input_node_ids
                .iter()
                .any(|input| input == node_id)
        }) {
            pending.push_back(consumer.node_id.as_str());
        }
    }
    Ok(closure)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(intervention: Option<InterventionDeclaration>) -> SnapshotIdentity {
        SnapshotIdentity {
            mesh_identifier: "mesh-v1".into(),
            seed: 7,
            program_hash: "program".into(),
            effective_parameters: BTreeMap::from([("gain".into(), "1".into())]),
            canonical_build_identity: "build".into(),
            intervention,
        }
    }

    fn declaration(parameter: bool, forcing: bool) -> InterventionDeclaration {
        InterventionDeclaration {
            parameter_substitutions: if parameter {
                BTreeMap::from([("gain".into(), "2".into())])
            } else {
                BTreeMap::new()
            },
            forcing_substitutions: if forcing {
                BTreeMap::from([("forcing".into(), "changed".into())])
            } else {
                BTreeMap::new()
            },
        }
    }

    fn node(id: &str, output: &str, inputs: &[&str], args: &[(&str, &str)]) -> NodeProvenance {
        NodeProvenance {
            node_id: id.into(),
            operator_id: "pointwise".into(),
            operator_kind: "generic".into(),
            input_node_ids: inputs.iter().map(|input| (*input).into()).collect(),
            effective_args: args
                .iter()
                .map(|(key, value)| ((*key).into(), (*value).into()))
                .collect(),
            output_binding: output.into(),
        }
    }

    fn dag() -> Vec<NodeProvenance> {
        vec![
            node("src", "forcing", &[], &[]),
            node("mid", "response", &["src"], &[("gain", "2")]),
            node("end", "result", &["mid"], &[]),
            node("other", "unrelated", &[], &[]),
        ]
    }

    #[test]
    fn verifies_valid_pair_and_deterministic_dag_closure() {
        let base = identity(None);
        let mut changed = identity(Some(declaration(true, true)));
        changed
            .effective_parameters
            .insert("gain".into(), "2".into());
        let verified = verify_intervention_pair(&base, &changed, &dag(), &dag()).unwrap();
        assert_eq!(
            verified.downstream_node_ids,
            BTreeSet::from(["src".into(), "mid".into(), "end".into()])
        );
        assert_eq!(
            verified.exact_unchanged_bindings,
            BTreeSet::from(["unrelated".into()])
        );
        let mut reversed = dag();
        reversed.reverse();
        assert_eq!(
            verify_intervention_pair(&base, &changed, &dag(), &reversed).unwrap(),
            verified
        );
    }

    #[test]
    fn rejects_undeclared_parameter_substitution_and_identity_mismatches() {
        let base = identity(None);
        let mut changed = identity(Some(declaration(false, true)));
        changed
            .effective_parameters
            .insert("gain".into(), "2".into());
        assert_eq!(
            verify_intervention_pair(&base, &changed, &dag(), &dag()),
            Err(InterventionError::UndeclaredParameter("gain".into()))
        );
        for mismatch in ["seed", "mesh", "program", "canonical build"] {
            let mut changed = identity(Some(declaration(false, true)));
            match mismatch {
                "seed" => changed.seed += 1,
                "mesh" => changed.mesh_identifier.push('x'),
                "program" => changed.program_hash.push('x'),
                _ => changed.canonical_build_identity.push('x'),
            }
            assert_eq!(
                verify_intervention_pair(&base, &changed, &dag(), &dag()),
                Err(InterventionError::IdentityMismatch(mismatch))
            );
        }
    }

    #[test]
    fn rejects_empty_intervention_and_reports_invalid_graphs() {
        let base = identity(None);
        let empty = identity(Some(declaration(false, false)));
        assert_eq!(
            verify_intervention_pair(&base, &empty, &dag(), &dag()),
            Err(InterventionError::EmptyIntervention)
        );
        let mut missing = dag();
        missing[1].input_node_ids = vec!["absent".into()];
        assert!(matches!(
            verify_intervention_pair(
                &base,
                &identity(Some(declaration(false, true))),
                &dag(),
                &missing
            ),
            Err(InterventionError::MissingNode { .. })
        ));
        let cycle = vec![
            node("a", "forcing", &["b"], &[]),
            node("b", "other", &["a"], &[]),
        ];
        assert_eq!(
            verify_intervention_pair(
                &base,
                &identity(Some(declaration(false, true))),
                &cycle,
                &cycle
            ),
            Err(InterventionError::Cycle)
        );
    }
}
