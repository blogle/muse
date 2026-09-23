//! YAML and CEL specification compiler for the frozen Wave 1 `Program` IR.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use cel::Program as CelProgram;
use muse_types::{
    CompiledExpressionHandle, CompiledNode, Program, StateUpdate, ValueRef, ValueType,
    operator_descriptor,
};
use petgraph::algo::toposort;
use petgraph::graph::{DiGraph, NodeIndex};
use serde::Deserialize;
use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
#[error("{category}{path}: {message}")]
pub struct CompileError {
    pub category: ErrorCategory,
    pub path: String,
    pub message: String,
}

impl CompileError {
    fn new(category: ErrorCategory, path: impl Into<String>, message: impl Into<String>) -> Self {
        let path = path.into();
        Self {
            category,
            path: if path.is_empty() {
                String::new()
            } else {
                format!(" at {path}")
            },
            message: message.into(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCategory {
    UnknownOperator,
    UnknownReference,
    DuplicateNode,
    MissingArgument,
    UnknownArgument,
    UnknownParameter,
    TypeMismatch,
    RecipeRecursion,
    DependencyCycle,
    InvalidCel,
    MalformedYaml,
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

#[derive(Debug, Deserialize)]
struct Document {
    #[serde(default)]
    inputs: BTreeMap<String, String>,
    #[serde(default)]
    parameters: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    recipes: BTreeMap<String, Recipe>,
    programs: BTreeMap<String, Section>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Recipe {
    #[serde(default)]
    nodes: BTreeMap<String, NodeSpec>,
    #[serde(default)]
    _outputs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Section {
    #[serde(default)]
    nodes: BTreeMap<String, NodeSpec>,
    #[serde(default)]
    outputs: BTreeMap<String, String>,
    #[serde(default)]
    updates: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
struct NodeSpec {
    #[serde(default)]
    op: Option<String>,
    #[serde(default)]
    #[serde(rename = "use")]
    use_recipe: Option<String>,
    #[serde(default)]
    args: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    inputs: BTreeMap<String, String>,
    #[serde(default)]
    params: BTreeMap<String, String>,
    #[serde(default)]
    expr: Option<String>,
}

/// Compile a YAML document. If multiple programs are present, `program_name`
/// selects one by name; deterministic map ordering makes diagnostics repeatable.
pub fn compile(source: &str) -> Result<Program, CompileError> {
    let doc: Document = serde_saphyr::from_str(source)
        .map_err(|error| CompileError::new(ErrorCategory::MalformedYaml, "", error.to_string()))?;
    compile_document(doc, None)
}

/// Compile one named program from a YAML document.
pub fn compile_program(source: &str, program_name: &str) -> Result<Program, CompileError> {
    let doc: Document = serde_saphyr::from_str(source)
        .map_err(|error| CompileError::new(ErrorCategory::MalformedYaml, "", error.to_string()))?;
    compile_document(doc, Some(program_name))
}

fn compile_document(doc: Document, selected: Option<&str>) -> Result<Program, CompileError> {
    let (name, section) = match selected {
        Some(name) => doc.programs.get_key_value(name).ok_or_else(|| {
            CompileError::new(
                ErrorCategory::UnknownReference,
                format!("programs.{name}"),
                "program does not exist",
            )
        })?,
        None => doc.programs.iter().next().ok_or_else(|| {
            CompileError::new(
                ErrorCategory::UnknownReference,
                "programs",
                "expected at least one program",
            )
        })?,
    };
    let base = format!("programs.{name}");
    let mut specs = section.nodes.clone();
    expand_recipes(&mut specs, &doc.recipes, &base, &mut Vec::new())?;
    let mut compiled = BTreeMap::<String, CompiledNode>::new();
    let mut node_types = BTreeMap::<String, ValueType>::new();
    for (node_name, spec) in &specs {
        if let Some(op) = &spec.op {
            let descriptor = operator_descriptor(op).ok_or_else(|| {
                CompileError::new(
                    ErrorCategory::UnknownOperator,
                    format!("{base}.nodes.{node_name}.op"),
                    format!("unknown operator {op}"),
                )
            })?;
            node_types.insert(
                node_name.clone(),
                descriptor
                    .outputs
                    .first()
                    .map(|port| port.ty)
                    .unwrap_or(ValueType::Scalar),
            );
        }
    }
    for (node_name, spec) in &specs {
        let path = format!("{base}.nodes.{node_name}");
        let op = spec.op.as_deref().ok_or_else(|| {
            CompileError::new(ErrorCategory::UnknownOperator, &path, "expected op")
        })?;
        let descriptor = operator_descriptor(op).ok_or_else(|| {
            CompileError::new(
                ErrorCategory::UnknownOperator,
                format!("{path}.op"),
                format!("unknown operator {op}"),
            )
        })?;
        let mut args = BTreeMap::new();
        let mut dependency_names = BTreeSet::new();
        for (argument, raw) in &spec.args {
            if !descriptor.inputs.iter().any(|port| port.name == argument) {
                return Err(CompileError::new(
                    ErrorCategory::UnknownArgument,
                    format!("{path}.args.{argument}"),
                    format!("unknown argument for {op}"),
                ));
            }
            let text = raw
                .as_str()
                .map(str::to_owned)
                .or_else(|| raw.as_f64().map(|v| v.to_string()))
                .ok_or_else(|| {
                    CompileError::new(
                        ErrorCategory::UnknownArgument,
                        format!("{path}.args.{argument}"),
                        "expected reference or scalar literal",
                    )
                })?;
            let value = resolve(
                &text,
                &doc.inputs,
                &doc.parameters,
                &node_types,
                &mut dependency_names,
                &format!("{path}.args.{argument}"),
            )?;
            let expected = descriptor
                .inputs
                .iter()
                .find(|port| port.name == argument)
                .unwrap()
                .ty;
            check_type(
                expected,
                &value,
                &text,
                &format!("{path}.args.{argument}"),
                &node_types,
            )?;
            args.insert(argument.clone(), value);
        }
        for port in descriptor.inputs.iter().filter(|port| port.required) {
            if !args.contains_key(port.name) {
                return Err(CompileError::new(
                    ErrorCategory::MissingArgument,
                    format!("{path}.args.{}", port.name),
                    format!("required argument for {op} is missing"),
                ));
            }
        }
        for (argument, text) in spec.inputs.iter().chain(spec.params.iter()) {
            if op != "pointwise" && op != "vector_expr" {
                return Err(CompileError::new(
                    ErrorCategory::UnknownArgument,
                    format!("{path}.inputs.{argument}"),
                    format!("operator {op} does not accept named expression bindings"),
                ));
            }
            let value = resolve(
                text,
                &doc.inputs,
                &doc.parameters,
                &node_types,
                &mut dependency_names,
                &format!("{path}.inputs.{argument}"),
            )?;
            args.insert(argument.clone(), value);
        }
        let mut cel = Vec::new();
        if let Some(expression) = &spec.expr {
            CelProgram::compile(expression).map_err(|error| {
                CompileError::new(
                    ErrorCategory::InvalidCel,
                    format!("{path}.expr"),
                    error.to_string(),
                )
            })?;
            cel.push(CompiledExpressionHandle::from_source(expression));
        }
        if matches!(op, "pointwise" | "vector_expr")
            && (spec.inputs.is_empty() || spec.expr.is_none())
        {
            return Err(CompileError::new(
                ErrorCategory::MissingArgument,
                &path,
                "expression operators require inputs and expr",
            ));
        }
        compiled.insert(
            node_name.clone(),
            CompiledNode {
                id: node_name.clone(),
                op: op.to_owned(),
                args,
                cel,
            },
        );
    }
    let mut graph = DiGraph::<String, ()>::new();
    let indices: BTreeMap<_, _> = specs
        .keys()
        .map(|id| (id.clone(), graph.add_node(id.clone())))
        .collect();
    for (id, spec) in &specs {
        let values = spec
            .args
            .values()
            .filter_map(serde_json::Value::as_str)
            .chain(spec.inputs.values().map(String::as_str))
            .chain(spec.params.values().map(String::as_str));
        for raw in values {
            if let Some(node) = raw.strip_prefix("$node.").and_then(|v| v.split('.').next())
                && let (Some(&from), Some(&to)) = (indices.get(node), indices.get(id))
            {
                graph.add_edge(from, to, ());
            }
        }
    }
    let order = toposort(&graph, None).map_err(|_| {
        CompileError::new(
            ErrorCategory::DependencyCycle,
            format!("{base}.nodes"),
            "dependency graph contains a cycle",
        )
    })?;
    let ordered = order
        .into_iter()
        .map(|index: NodeIndex| graph[index].clone())
        .collect::<Vec<_>>();
    let nodes = ordered
        .iter()
        .map(|id| compiled.remove(id).unwrap())
        .collect();
    let mut updates = Vec::new();
    for (field, raw) in &section.updates {
        let mut deps = BTreeSet::new();
        let value = resolve(
            raw,
            &doc.inputs,
            &doc.parameters,
            &node_types,
            &mut deps,
            &format!("{base}.updates.{field}"),
        )?;
        updates.push(StateUpdate {
            field: field.clone(),
            value,
        });
    }
    for (field, raw) in &section.outputs {
        let mut deps = BTreeSet::new();
        resolve(
            raw,
            &doc.inputs,
            &doc.parameters,
            &node_types,
            &mut deps,
            &format!("{base}.outputs.{field}"),
        )?;
    }
    Ok(Program { nodes, updates })
}

fn expand_recipes(
    nodes: &mut BTreeMap<String, NodeSpec>,
    recipes: &BTreeMap<String, Recipe>,
    path: &str,
    stack: &mut Vec<String>,
) -> Result<(), CompileError> {
    let originals = std::mem::take(nodes);
    for (id, mut spec) in originals {
        if let Some(recipe_name) = spec.use_recipe.take() {
            if stack.contains(&recipe_name) {
                return Err(CompileError::new(
                    ErrorCategory::RecipeRecursion,
                    format!("{path}.nodes.{id}.use"),
                    format!("recursive recipe inclusion {recipe_name}"),
                ));
            }
            let recipe = recipes.get(&recipe_name).ok_or_else(|| {
                CompileError::new(
                    ErrorCategory::UnknownReference,
                    format!("{path}.nodes.{id}.use"),
                    format!("unknown recipe {recipe_name}"),
                )
            })?;
            stack.push(recipe_name.clone());
            let mut nested = recipe.nodes.clone();
            expand_recipes(&mut nested, recipes, path, stack)?;
            stack.pop();
            for (nested_id, nested_spec) in nested {
                let prefixed = format!("{id}.{nested_id}");
                if nodes.insert(prefixed.clone(), nested_spec).is_some() {
                    return Err(CompileError::new(
                        ErrorCategory::DuplicateNode,
                        format!("{path}.nodes.{prefixed}"),
                        "duplicate expanded node",
                    ));
                }
            }
        } else if nodes.insert(id.clone(), spec).is_some() {
            return Err(CompileError::new(
                ErrorCategory::DuplicateNode,
                format!("{path}.nodes.{id}"),
                "duplicate node",
            ));
        }
    }
    Ok(())
}

fn resolve(
    raw: &str,
    inputs: &BTreeMap<String, String>,
    parameters: &BTreeMap<String, serde_json::Value>,
    nodes: &BTreeMap<String, ValueType>,
    dependencies: &mut BTreeSet<String>,
    path: &str,
) -> Result<ValueRef, CompileError> {
    if let Some(name) = raw.strip_prefix("$input.") {
        return inputs
            .get(name)
            .map(|_| ValueRef::Input(name.to_owned()))
            .ok_or_else(|| {
                CompileError::new(
                    ErrorCategory::UnknownReference,
                    path,
                    format!("unknown input {name}"),
                )
            });
    }
    if let Some(name) = raw.strip_prefix("$param.") {
        return parameters
            .get(name)
            .map(|_| ValueRef::Parameter(name.to_owned()))
            .ok_or_else(|| {
                CompileError::new(
                    ErrorCategory::UnknownParameter,
                    path,
                    format!("unknown parameter {name}"),
                )
            });
    }
    if let Some(reference) = raw.strip_prefix("$node.") {
        let (node, output) = reference.split_once('.').unwrap_or((reference, "value"));
        if !nodes.contains_key(node) {
            return Err(CompileError::new(
                ErrorCategory::UnknownReference,
                path,
                format!("unknown node output {raw}"),
            ));
        }
        dependencies.insert(node.to_owned());
        return Ok(ValueRef::NodeOutput {
            node: node.to_owned(),
            output: output.to_owned(),
        });
    }
    if let Some(name) = raw.strip_prefix("$state.") {
        return Ok(ValueRef::State(name.to_owned()));
    }
    if let Ok(number) = raw.parse::<f64>() {
        return Ok(ValueRef::LiteralScalar(number));
    }
    Err(CompileError::new(
        ErrorCategory::UnknownReference,
        path,
        format!("unsupported reference {raw}"),
    ))
}

fn check_type(
    expected: ValueType,
    value: &ValueRef,
    raw: &str,
    path: &str,
    nodes: &BTreeMap<String, ValueType>,
) -> Result<(), CompileError> {
    let actual = match value {
        ValueRef::NodeOutput { node, .. } => nodes.get(node).copied(),
        ValueRef::LiteralScalar(_) => Some(ValueType::Scalar),
        _ => None,
    };
    if let Some(actual) = actual.filter(|actual| *actual != expected) {
        return Err(CompileError::new(
            ErrorCategory::TypeMismatch,
            path,
            format!("expected {expected:?}, got {actual:?} from {raw}"),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_yaml_is_categorized() {
        let error = compile("programs: [").unwrap_err();
        assert_eq!(error.category, ErrorCategory::MalformedYaml);
        assert!(error.to_string().starts_with("MalformedYaml"));
    }

    #[test]
    fn fixture_results_are_snapshot_stable() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/specs");
        for entry in std::fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("yaml") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let result = match compile(&source) {
                Ok(program) => format!("OK\n{program:#?}"),
                Err(error) => format!("ERROR\n{error}"),
            };
            insta::with_settings!({snapshot_path => "../../../fixtures/specs/snapshots"}, {
                insta::assert_snapshot!(path.file_stem().unwrap().to_string_lossy().into_owned(), result);
            });
        }
    }
}
