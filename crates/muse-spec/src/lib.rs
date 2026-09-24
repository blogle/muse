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

/// The selected program and its deterministic top-level numeric defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Compilation {
    pub program: Program,
    pub parameters: BTreeMap<String, f64>,
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
    parameters: BTreeMap<String, ParameterSpec>,
    #[serde(default)]
    recipes: BTreeMap<String, Recipe>,
    programs: BTreeMap<String, Section>,
}

#[derive(Clone, Debug, Default, Deserialize)]
struct Recipe {
    #[serde(default)]
    inputs: BTreeMap<String, String>,
    #[serde(default)]
    parameters: BTreeMap<String, ParameterSpec>,
    #[serde(default)]
    nodes: BTreeMap<String, NodeSpec>,
    #[serde(default)]
    outputs: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize)]
struct ParameterSpec {
    #[serde(rename = "type", default = "scalar_type")]
    ty: String,
    #[serde(default)]
    default: Option<serde_json::Value>,
}

fn scalar_type() -> String {
    "scalar".to_owned()
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
    compile_document(doc, None).map(|result| result.program)
}

/// Compile a document and retain its top-level numeric parameter defaults.
pub fn compile_with_defaults(source: &str) -> Result<Compilation, CompileError> {
    let doc: Document = serde_saphyr::from_str(source)
        .map_err(|error| CompileError::new(ErrorCategory::MalformedYaml, "", error.to_string()))?;
    compile_document(doc, None)
}

/// Compile one named program from a YAML document.
pub fn compile_program(source: &str, program_name: &str) -> Result<Program, CompileError> {
    let doc: Document = serde_saphyr::from_str(source)
        .map_err(|error| CompileError::new(ErrorCategory::MalformedYaml, "", error.to_string()))?;
    compile_document(doc, Some(program_name)).map(|result| result.program)
}

/// Compile one named program and retain top-level numeric parameter defaults.
pub fn compile_program_with_defaults(
    source: &str,
    program_name: &str,
) -> Result<Compilation, CompileError> {
    let doc: Document = serde_saphyr::from_str(source)
        .map_err(|error| CompileError::new(ErrorCategory::MalformedYaml, "", error.to_string()))?;
    compile_document(doc, Some(program_name))
}

fn compile_document(doc: Document, selected: Option<&str>) -> Result<Compilation, CompileError> {
    validate_declared_types(&doc)?;
    let parameters = top_level_defaults(&doc.parameters)?;
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
    let mut specs = BTreeMap::new();
    let mut aliases = BTreeMap::new();
    let mut binding_checks = Vec::new();
    expand_nodes(
        &section.nodes,
        &doc.recipes,
        &base,
        "",
        &BTreeMap::new(),
        &BTreeMap::new(),
        &mut Vec::new(),
        &mut specs,
        &mut aliases,
        &mut binding_checks,
    )?;
    let mut compiled = BTreeMap::<String, CompiledNode>::new();
    let mut node_types = BTreeMap::<String, ValueType>::new();
    let mut node_outputs = BTreeMap::<String, BTreeSet<String>>::new();
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
            node_outputs.insert(
                node_name.clone(),
                descriptor
                    .outputs
                    .iter()
                    .map(|port| port.name.to_owned())
                    .collect(),
            );
        }
    }
    for (binding_path, reference, expected) in &binding_checks {
        let actual = reference_type(
            reference,
            &node_types,
            &doc.inputs,
            &doc.parameters,
            &aliases,
        );
        if actual != Some(*expected) {
            return Err(CompileError::new(
                ErrorCategory::TypeMismatch,
                binding_path,
                format!(
                    "expected {expected:?}, got {} from {reference}",
                    actual.map_or("unknown".to_owned(), |value| format!("{value:?}"))
                ),
            ));
        }
    }
    for (alias, reference) in &aliases {
        let mut dependencies = BTreeSet::new();
        resolve(
            reference,
            &doc.inputs,
            &doc.parameters,
            &node_types,
            &mut dependencies,
            &format!("{base}.nodes.{alias}"),
            &aliases,
            &node_outputs,
        )?;
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
            let config_type = config_argument(op, argument);
            let port = descriptor.inputs.iter().find(|port| port.name == argument);
            if port.is_none() && config_type.is_none() {
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
                &aliases,
                &node_outputs,
            )?;
            if let Some(port) = port {
                check_type(
                    port.ty,
                    &value,
                    &text,
                    &format!("{path}.args.{argument}"),
                    &node_types,
                    &doc.inputs,
                    &doc.parameters,
                    &aliases,
                )?;
            } else {
                check_type(
                    config_type.unwrap(),
                    &value,
                    &text,
                    &format!("{path}.args.{argument}"),
                    &node_types,
                    &doc.inputs,
                    &doc.parameters,
                    &aliases,
                )?;
            }
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
        for &(key, required, _) in config_arguments(op) {
            if required && !spec.args.contains_key(key) {
                return Err(CompileError::new(
                    ErrorCategory::MissingArgument,
                    format!("{path}.args.{key}"),
                    format!("required scalar configuration for {op} is missing"),
                ));
            }
        }
        if matches!(op, "pointwise" | "vector_expr")
            && let Some(name) = spec
                .inputs
                .keys()
                .find(|name| spec.params.contains_key(*name))
        {
            return Err(CompileError::new(
                ErrorCategory::UnknownArgument,
                format!("{path}.params.{name}"),
                format!("binding {name} is declared as both input and parameter"),
            ));
        }
        for (argument, text) in &spec.inputs {
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
                &aliases,
                &node_outputs,
            )?;
            let binding_path = format!("{path}.inputs.{argument}");
            if op == "vector_expr" {
                check_expression_binding_type(
                    &value,
                    text,
                    &binding_path,
                    &node_types,
                    &doc.inputs,
                    &doc.parameters,
                    &aliases,
                )?;
            } else {
                check_type(
                    ValueType::ScalarField,
                    &value,
                    text,
                    &binding_path,
                    &node_types,
                    &doc.inputs,
                    &doc.parameters,
                    &aliases,
                )?;
            }
            args.insert(argument.clone(), value);
        }
        for (argument, text) in &spec.params {
            if op != "pointwise" && op != "vector_expr" {
                return Err(CompileError::new(
                    ErrorCategory::UnknownArgument,
                    format!("{path}.params.{argument}"),
                    format!("operator {op} does not accept expression parameters"),
                ));
            }
            let value = resolve(
                text,
                &doc.inputs,
                &doc.parameters,
                &node_types,
                &mut dependency_names,
                &format!("{path}.params.{argument}"),
                &aliases,
                &node_outputs,
            )?;
            check_type(
                ValueType::Scalar,
                &value,
                text,
                &format!("{path}.params.{argument}"),
                &node_types,
                &doc.inputs,
                &doc.parameters,
                &aliases,
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
    for (id, compiled_node) in &compiled {
        for value in compiled_node.args.values() {
            if let ValueRef::NodeOutput { node, .. } = value
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
            &aliases,
            &node_outputs,
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
            &aliases,
            &node_outputs,
        )?;
    }
    Ok(Compilation {
        program: Program { nodes, updates },
        parameters,
    })
}

fn top_level_defaults(
    parameters: &BTreeMap<String, ParameterSpec>,
) -> Result<BTreeMap<String, f64>, CompileError> {
    parameters
        .iter()
        .filter_map(|(name, parameter)| parameter.default.as_ref().map(|default| (name, default)))
        .map(|(name, default)| {
            default
                .as_f64()
                .map(|value| (name.clone(), value))
                .ok_or_else(|| {
                    CompileError::new(
                        ErrorCategory::TypeMismatch,
                        format!("parameters.{name}.default"),
                        "expected numeric scalar default",
                    )
                })
        })
        .collect()
}

fn validate_declared_types(doc: &Document) -> Result<(), CompileError> {
    for (name, ty) in &doc.inputs {
        declared_type(ty, &format!("inputs.{name}"))?;
    }
    for (name, parameter) in &doc.parameters {
        declared_type(&parameter.ty, &format!("parameters.{name}.type"))?;
    }
    for (recipe_name, recipe) in &doc.recipes {
        for (name, ty) in &recipe.inputs {
            declared_type(ty, &format!("recipes.{recipe_name}.inputs.{name}"))?;
        }
        for (name, parameter) in &recipe.parameters {
            declared_type(
                &parameter.ty,
                &format!("recipes.{recipe_name}.parameters.{name}.type"),
            )?;
        }
    }
    Ok(())
}

fn declared_type(value: &str, path: &str) -> Result<ValueType, CompileError> {
    parse_type(value).ok_or_else(|| {
        CompileError::new(
            ErrorCategory::TypeMismatch,
            path,
            format!(
                "unsupported declared type {value}; expected scalar, scalar_field, vector_field, bool_field, category_field, index_field, or network"
            ),
        )
    })
}

#[allow(clippy::too_many_arguments)]
fn expand_nodes(
    source: &BTreeMap<String, NodeSpec>,
    recipes: &BTreeMap<String, Recipe>,
    path: &str,
    namespace: &str,
    input_bindings: &BTreeMap<String, String>,
    parameter_bindings: &BTreeMap<String, String>,
    stack: &mut Vec<String>,
    output: &mut BTreeMap<String, NodeSpec>,
    aliases: &mut BTreeMap<String, String>,
    binding_checks: &mut Vec<(String, String, ValueType)>,
) -> Result<(), CompileError> {
    for (id, spec) in source {
        let full_id = if namespace.is_empty() {
            id.clone()
        } else {
            format!("{namespace}.{id}")
        };
        if let Some(recipe_name) = &spec.use_recipe {
            let use_path = format!("{path}.nodes.{full_id}.use");
            if stack.contains(recipe_name) {
                return Err(CompileError::new(
                    ErrorCategory::RecipeRecursion,
                    use_path,
                    format!("recursive recipe inclusion {recipe_name}"),
                ));
            }
            let recipe = recipes.get(recipe_name).ok_or_else(|| {
                CompileError::new(
                    ErrorCategory::UnknownReference,
                    &use_path,
                    format!("unknown recipe {recipe_name}"),
                )
            })?;
            let bindings = &spec.inputs;
            for input in recipe.inputs.keys() {
                if !bindings.contains_key(input) {
                    return Err(CompileError::new(
                        ErrorCategory::MissingArgument,
                        format!("{path}.nodes.{full_id}.inputs.{input}"),
                        "recipe input binding is required",
                    ));
                }
            }
            for input in bindings.keys() {
                if !recipe.inputs.contains_key(input) {
                    return Err(CompileError::new(
                        ErrorCategory::UnknownArgument,
                        format!("{path}.nodes.{full_id}.inputs.{input}"),
                        "recipe declares no such input",
                    ));
                }
            }
            for (name, expected) in &recipe.inputs {
                let declaration_path = format!("recipes.{recipe_name}.inputs.{name}");
                let expected_type = declared_type(expected, &declaration_path)?;
                binding_checks.push((
                    format!("{path}.nodes.{full_id}.inputs.{name}"),
                    substitute(&bindings[name], input_bindings, parameter_bindings),
                    expected_type,
                ));
            }
            let mut bound_inputs = input_bindings.clone();
            for (name, value) in bindings {
                bound_inputs.insert(
                    name.clone(),
                    substitute(value, input_bindings, parameter_bindings),
                );
            }
            let mut bound_params = parameter_bindings.clone();
            for (name, parameter) in &recipe.parameters {
                if let Some(value) = spec.params.get(name) {
                    let declaration_path = format!("recipes.{recipe_name}.parameters.{name}.type");
                    let expected_type = declared_type(&parameter.ty, &declaration_path)?;
                    binding_checks.push((
                        format!("{path}.nodes.{full_id}.params.{name}"),
                        substitute(value, input_bindings, parameter_bindings),
                        expected_type,
                    ));
                    bound_params.insert(
                        name.clone(),
                        substitute(value, input_bindings, parameter_bindings),
                    );
                } else if let Some(value) = &parameter.default {
                    bound_params.insert(
                        name.clone(),
                        value.as_f64().map(|n| n.to_string()).unwrap_or_default(),
                    );
                } else {
                    return Err(CompileError::new(
                        ErrorCategory::MissingArgument,
                        format!("{path}.nodes.{full_id}.params.{name}"),
                        "recipe parameter requires a binding",
                    ));
                }
            }
            for name in spec.params.keys() {
                if !recipe.parameters.contains_key(name) {
                    return Err(CompileError::new(
                        ErrorCategory::UnknownArgument,
                        format!("{path}.nodes.{full_id}.params.{name}"),
                        "recipe declares no such parameter",
                    ));
                }
            }
            stack.push(recipe_name.clone());
            expand_nodes(
                &recipe.nodes,
                recipes,
                path,
                &full_id,
                &bound_inputs,
                &bound_params,
                stack,
                output,
                aliases,
                binding_checks,
            )?;
            stack.pop();
            for (name, value) in &recipe.outputs {
                aliases.insert(
                    format!("{full_id}.{name}"),
                    qualify_ref(value, &full_id, &bound_inputs, &bound_params),
                );
            }
        } else {
            if output.contains_key(&full_id) {
                return Err(CompileError::new(
                    ErrorCategory::DuplicateNode,
                    format!("{path}.nodes.{full_id}"),
                    "duplicate expanded node",
                ));
            }
            let mut expanded = spec.clone();
            for raw in expanded.args.values_mut() {
                if let Some(text) = raw.as_str() {
                    *raw = serde_json::Value::String(qualify_ref(
                        text,
                        namespace,
                        input_bindings,
                        parameter_bindings,
                    ));
                }
            }
            expanded.inputs = expanded
                .inputs
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        qualify_ref(v, namespace, input_bindings, parameter_bindings),
                    )
                })
                .collect();
            expanded.params = expanded
                .params
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        qualify_ref(v, namespace, input_bindings, parameter_bindings),
                    )
                })
                .collect();
            expanded.op = spec.op.clone();
            output.insert(full_id, expanded);
        }
    }
    Ok(())
}

fn substitute(
    raw: &str,
    inputs: &BTreeMap<String, String>,
    params: &BTreeMap<String, String>,
) -> String {
    if let Some(name) = raw.strip_prefix("$input.") {
        inputs.get(name).cloned().unwrap_or_else(|| raw.to_owned())
    } else if let Some(name) = raw.strip_prefix("$param.") {
        params.get(name).cloned().unwrap_or_else(|| raw.to_owned())
    } else {
        raw.to_owned()
    }
}

fn qualify_ref(
    raw: &str,
    namespace: &str,
    inputs: &BTreeMap<String, String>,
    params: &BTreeMap<String, String>,
) -> String {
    if let Some(name) = raw.strip_prefix("$input.") {
        return inputs.get(name).cloned().unwrap_or_else(|| raw.to_owned());
    }
    if let Some(name) = raw.strip_prefix("$param.") {
        return params.get(name).cloned().unwrap_or_else(|| raw.to_owned());
    }
    if let Some(reference) = raw.strip_prefix("$node.") {
        let Some((node, output)) = reference.rsplit_once('.') else {
            return raw.to_owned();
        };
        if namespace.is_empty() {
            raw.to_owned()
        } else {
            format!("$node.{namespace}.{node}.{output}")
        }
    } else {
        raw.to_owned()
    }
}

fn config_arguments(op: &str) -> &'static [(&'static str, bool, ValueType)] {
    match op {
        "constant" => &[("value", true, ValueType::Scalar)],
        "noise" => &[("scale", false, ValueType::Scalar)],
        "voronoi_labels" => &[("count", true, ValueType::Scalar)],
        "diffuse" => &[
            ("rate", true, ValueType::Scalar),
            ("iterations", true, ValueType::Scalar),
        ],
        "network_threshold" => &[("threshold", true, ValueType::Scalar)],
        "threshold" => &[("threshold", true, ValueType::Scalar)],
        _ => &[],
    }
}

fn config_argument(op: &str, name: &str) -> Option<ValueType> {
    config_arguments(op)
        .iter()
        .find(|(key, _, _)| *key == name)
        .map(|(_, _, ty)| *ty)
}

#[allow(clippy::too_many_arguments)]
fn resolve(
    raw: &str,
    inputs: &BTreeMap<String, String>,
    parameters: &BTreeMap<String, ParameterSpec>,
    nodes: &BTreeMap<String, ValueType>,
    dependencies: &mut BTreeSet<String>,
    path: &str,
    aliases: &BTreeMap<String, String>,
    node_outputs: &BTreeMap<String, BTreeSet<String>>,
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
        let (node, output) = reference.rsplit_once('.').ok_or_else(|| {
            CompileError::new(
                ErrorCategory::UnknownReference,
                path,
                format!("expected $node.<node_id>.<output>, got {raw}"),
            )
        })?;
        if let Some(alias) = aliases.get(&format!("{node}.{output}")) {
            let target = alias_target(alias, aliases, path)?;
            return resolve(
                &target,
                inputs,
                parameters,
                nodes,
                dependencies,
                path,
                aliases,
                node_outputs,
            );
        }
        if !nodes.contains_key(node) {
            return Err(CompileError::new(
                ErrorCategory::UnknownReference,
                path,
                format!("unknown node output {raw}"),
            ));
        }
        if !node_outputs
            .get(node)
            .is_some_and(|outputs| outputs.contains(output))
        {
            return Err(CompileError::new(
                ErrorCategory::UnknownReference,
                path,
                format!("node {node} has no output {output}"),
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

fn alias_target<'a>(
    start: &'a str,
    aliases: &'a BTreeMap<String, String>,
    path: &str,
) -> Result<String, CompileError> {
    let mut current = start.to_owned();
    let mut visited = BTreeSet::new();
    loop {
        let Some(reference) = current.strip_prefix("$node.") else {
            return Ok(current);
        };
        let Some((node, output)) = reference.rsplit_once('.') else {
            return Ok(current);
        };
        let key = format!("{node}.{output}");
        let Some(next) = aliases.get(&key) else {
            return Ok(current);
        };
        if !visited.insert(key.clone()) {
            return Err(CompileError::new(
                ErrorCategory::DependencyCycle,
                path,
                format!("recipe output aliases contain a cycle at {key}"),
            ));
        }
        current = next.clone();
    }
}

#[allow(clippy::too_many_arguments)]
fn check_type(
    expected: ValueType,
    value: &ValueRef,
    raw: &str,
    path: &str,
    nodes: &BTreeMap<String, ValueType>,
    inputs: &BTreeMap<String, String>,
    parameters: &BTreeMap<String, ParameterSpec>,
    aliases: &BTreeMap<String, String>,
) -> Result<(), CompileError> {
    let actual = match value {
        ValueRef::NodeOutput { node, output } => aliases
            .get(&format!("{node}.{output}"))
            .and_then(|raw| reference_type(raw, nodes, inputs, parameters, aliases))
            .or_else(|| nodes.get(node).copied()),
        ValueRef::LiteralScalar(_) => Some(ValueType::Scalar),
        ValueRef::Input(name) => inputs.get(name).and_then(|ty| parse_type(ty)),
        ValueRef::Parameter(name) => parameters
            .get(name)
            .and_then(|parameter| parse_type(&parameter.ty)),
        ValueRef::State(_) => None,
    };
    if let Some(actual) = actual.filter(|actual| *actual != expected) {
        return Err(CompileError::new(
            ErrorCategory::TypeMismatch,
            path,
            format!("expected {expected:?}, got {actual:?} from {raw}"),
        ));
    }
    if actual.is_none() {
        let declared = match value {
            ValueRef::Input(name) => inputs.get(name).map(String::as_str),
            ValueRef::Parameter(name) => {
                parameters.get(name).map(|parameter| parameter.ty.as_str())
            }
            _ => None,
        };
        if let Some(declared) = declared {
            return Err(CompileError::new(
                ErrorCategory::TypeMismatch,
                path,
                format!(
                    "expected {expected:?}, got unsupported declared type {declared} from {raw}"
                ),
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_expression_binding_type(
    value: &ValueRef,
    raw: &str,
    path: &str,
    nodes: &BTreeMap<String, ValueType>,
    inputs: &BTreeMap<String, String>,
    parameters: &BTreeMap<String, ParameterSpec>,
    aliases: &BTreeMap<String, String>,
) -> Result<(), CompileError> {
    let actual = match value {
        ValueRef::NodeOutput { node, output } => aliases
            .get(&format!("{node}.{output}"))
            .and_then(|raw| reference_type(raw, nodes, inputs, parameters, aliases))
            .or_else(|| nodes.get(node).copied()),
        ValueRef::Input(name) => inputs.get(name).and_then(|ty| parse_type(ty)),
        ValueRef::Parameter(name) => parameters
            .get(name)
            .and_then(|parameter| parse_type(&parameter.ty)),
        ValueRef::LiteralScalar(_) => Some(ValueType::Scalar),
        ValueRef::State(_) => None,
    };
    if !matches!(
        actual,
        Some(ValueType::ScalarField | ValueType::VectorField)
    ) {
        return Err(CompileError::new(
            ErrorCategory::TypeMismatch,
            path,
            format!(
                "expected ScalarField or VectorField, got {} from {raw}",
                actual.map_or("unknown".to_owned(), |value| format!("{value:?}"))
            ),
        ));
    }
    Ok(())
}

fn parse_type(value: &str) -> Option<ValueType> {
    match value {
        "scalar" => Some(ValueType::Scalar),
        "scalar_field" => Some(ValueType::ScalarField),
        "vector_field" => Some(ValueType::VectorField),
        "bool_field" => Some(ValueType::BoolField),
        "category_field" => Some(ValueType::CategoryField),
        "index_field" => Some(ValueType::IndexField),
        "network" => Some(ValueType::Network),
        _ => None,
    }
}

fn reference_type(
    raw: &str,
    nodes: &BTreeMap<String, ValueType>,
    inputs: &BTreeMap<String, String>,
    parameters: &BTreeMap<String, ParameterSpec>,
    aliases: &BTreeMap<String, String>,
) -> Option<ValueType> {
    if let Some(name) = raw.strip_prefix("$input.") {
        return inputs.get(name).and_then(|ty| parse_type(ty));
    }
    if let Some(name) = raw.strip_prefix("$param.") {
        return parameters.get(name).and_then(|spec| parse_type(&spec.ty));
    }
    if let Some(name) = raw.strip_prefix("$node.") {
        let (node, output) = name.rsplit_once('.').unwrap_or((name, "value"));
        if let Some(alias) = aliases.get(&format!("{node}.{output}")) {
            return reference_type(alias, nodes, inputs, parameters, aliases);
        }
        return nodes.get(node).copied();
    }
    raw.parse::<f64>().ok().map(|_| ValueType::Scalar)
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
    fn unsupported_types_fail_at_each_schema_declaration_site() {
        for (source, expected_path) in [
            (
                "inputs:\n  unused: temperature_field\nprograms:\n  generate: {}\n",
                "inputs.unused",
            ),
            (
                "parameters:\n  unused:\n    type: temperature_field\nprograms:\n  generate: {}\n",
                "parameters.unused.type",
            ),
            (
                "recipes:\n  unused:\n    parameters:\n      p:\n        type: temperature_field\nprograms:\n  generate: {}\n",
                "recipes.unused.parameters.p.type",
            ),
        ] {
            let error = compile(source).unwrap_err();
            assert_eq!(error.category, ErrorCategory::TypeMismatch);
            assert!(error.to_string().contains(expected_path));
            assert!(error.to_string().contains("temperature_field"));
        }
    }

    #[test]
    fn vector_expr_accepts_scalar_field_bindings_and_reports_wrong_types_at_binding() {
        let source = "inputs:\n  position_x: scalar_field\n  position_y: scalar_field\n  position_z: scalar_field\nprograms:\n  generate:\n    nodes:\n      velocity:\n        op: vector_expr\n        inputs:\n          x: $input.position_x\n          y: $input.position_y\n          z: $input.position_z\n        expr: x\n";
        assert_eq!(compile(source).unwrap().nodes.len(), 1);

        let wrong_type = source.replace("position_x: scalar_field", "position_x: scalar");
        let error = compile(&wrong_type).unwrap_err();
        assert_eq!(error.category, ErrorCategory::TypeMismatch);
        assert!(
            error
                .to_string()
                .contains("programs.generate.nodes.velocity.inputs.x")
        );
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
            let expected_category = match path.file_stem().unwrap().to_str().unwrap() {
                "error-unknown-op" => Some("UnknownOperator"),
                "error-unknown-ref" => Some("UnknownReference"),
                "error-duplicate-node" => Some("DuplicateNode"),
                "error-missing-arg" | "error-missing-config" => Some("MissingArgument"),
                "error-unknown-arg" | "error-duplicate-binding" => Some("UnknownArgument"),
                "error-earth-threshold-missing-config" => Some("MissingArgument"),
                "error-earth-vector-magnitude-unknown-config" => Some("UnknownArgument"),
                "error-unknown-parameter" => Some("UnknownParameter"),
                "error-type-mismatch"
                | "error-earth-vector-dot-type"
                | "error-recipe-binding"
                | "error-expression-binding"
                | "error-unknown-type" => Some("TypeMismatch"),
                "error-recipe-recursion" => Some("RecipeRecursion"),
                "error-cycle" => Some("DependencyCycle"),
                "error-invalid-cel" => Some("InvalidCel"),
                "error-malformed-yaml" => Some("MalformedYaml"),
                _ => None,
            };
            if let Some(category) = expected_category {
                assert!(
                    result.contains(category),
                    "{} did not report {category}: {result}",
                    path.display()
                );
            }
            if path.file_stem().unwrap() == "error-unknown-type" {
                assert!(result.contains("recipes.unsupported_schema.inputs.source"));
                assert!(result.contains("temperature_field"));
                assert!(result.contains("expected scalar, scalar_field, vector_field"));
            }
            if path.file_stem().unwrap() == "valid-recipe-inclusion" {
                let repeated = compile(&source).unwrap();
                let first = compile(&source).unwrap();
                assert_eq!(format!("{first:#?}"), format!("{repeated:#?}"));
                assert_eq!(first.nodes[0].id, "first.filtered");
                assert_eq!(first.nodes[1].id, "second.filtered");
                assert_eq!(
                    first.nodes[1].args["field"],
                    ValueRef::NodeOutput {
                        node: "first.filtered".into(),
                        output: "value".into()
                    }
                );
            }
            if path.file_stem().unwrap() == "error-recipe-binding" {
                let error = compile(&source).unwrap_err();
                assert_eq!(error.category, ErrorCategory::TypeMismatch);
                assert!(
                    error
                        .to_string()
                        .contains("programs.generate.nodes.broken.inputs.field")
                );
            }
            if path.file_stem().unwrap() == "valid-earth-operators" {
                let first = compile_with_defaults(&source).unwrap();
                let repeated = compile_with_defaults(&source).unwrap();
                assert_eq!(first, repeated);
                assert_eq!(
                    first.parameters,
                    BTreeMap::from([("cutoff".to_owned(), 0.5), ("unused".to_owned(), 2.0)])
                );
                assert_eq!(first.program.nodes.len(), 3);
            }
            insta::with_settings!({snapshot_path => "../../../fixtures/specs/snapshots"}, {
                insta::assert_snapshot!(path.file_stem().unwrap().to_string_lossy().into_owned(), result);
            });
        }
    }
}
