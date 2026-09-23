//! Dense, eager execution for the frozen MUSE operator algebra.

use std::collections::BTreeMap;

use glam::DVec3;
use muse_types::{CellId, Field, Mesh, Network, Program, ValueRef, WorldState};
use rayon::prelude::*;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum OperatorError {
    #[error("invalid input: {0}")]
    Invalid(String),
    #[error("operator {0} is not implemented in Wave 1")]
    NotImplemented(String),
    #[error("CEL error: {0}")]
    Cel(String),
    #[error("unsupported operator configuration: {0}")]
    UnsupportedConfiguration(String),
    #[error("missing external input `{0}`")]
    MissingInput(String),
    #[error("external input `{0}` is not referenced by the program")]
    UnknownInput(String),
    #[error("external input `{name}` has the wrong runtime type; expected {expected}")]
    InputType {
        name: String,
        expected: &'static str,
    },
}

pub type Result<T> = std::result::Result<T, OperatorError>;
type CelBindings = (BTreeMap<String, Vec<f64>>, BTreeMap<String, f64>);

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Scalar(f64),
    Field(Field),
    Network(Network),
}

/// Runtime values supplied for `ValueRef::Input` at execution time.
#[derive(Clone, Debug, PartialEq)]
pub enum RuntimeValue {
    Scalar(f64),
    Field(Field),
    Network(Network),
}

#[derive(Clone, Debug, Default)]
pub struct FieldStore {
    values: BTreeMap<String, Value>,
}

impl FieldStore {
    pub fn insert(&mut self, name: impl Into<String>, value: Value) -> Option<Value> {
        self.values.insert(name.into(), value)
    }
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.values.get(name)
    }
    pub fn scalar_field(&self, name: &str) -> Result<&[f64]> {
        match self.values.get(name) {
            Some(Value::Field(Field::Scalar(v))) => Ok(v),
            _ => Err(OperatorError::Invalid(format!(
                "{name} is not a scalar field"
            ))),
        }
    }
    pub fn into_values(self) -> BTreeMap<String, Value> {
        self.values
    }
}

fn validate_mesh(mesh: &Mesh) -> Result<()> {
    if mesh.positions.len() != mesh.neighbors.len() {
        return Err(OperatorError::Invalid(
            "mesh position/neighbor length mismatch".into(),
        ));
    }
    let n = mesh.positions.len();
    if mesh.neighbors.iter().flatten().any(|&i| i as usize >= n) {
        return Err(OperatorError::Invalid("neighbor id out of bounds".into()));
    }
    Ok(())
}

pub fn constant(mesh: &Mesh, value: f64) -> Vec<f64> {
    vec![value; mesh.positions.len()]
}

/// Stateless node/cell keyed noise. Each cell's sample is independent of execution order.
pub fn noise(mesh: &Mesh, seed: u64, node_key: &str, scale: f64) -> Vec<f64> {
    let mut h = blake3::Hasher::new();
    h.update(&seed.to_le_bytes());
    h.update(node_key.as_bytes());
    let key = h.finalize();
    let noise = fastnoise_lite::FastNoiseLite::with_seed(i32::from_le_bytes(
        key.as_bytes()[..4].try_into().unwrap(),
    ));
    mesh.positions
        .par_iter()
        .enumerate()
        .map(|(i, p)| {
            let mut h = blake3::Hasher::new();
            h.update(key.as_bytes());
            h.update(&(i as u32).to_le_bytes());
            let cell_key = h.finalize();
            let jitter = u32::from_le_bytes(cell_key.as_bytes()[..4].try_into().unwrap()) as f64
                / u32::MAX as f64;
            f64::from(noise.get_noise_3d(p.x + jitter, p.y + i as f64, p.z - jitter)) * scale
        })
        .collect()
}

pub fn neighbor_sample(mesh: &Mesh, values: &[f64]) -> Result<Vec<f64>> {
    validate_mesh(mesh)?;
    if values.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(mesh
        .neighbors
        .par_iter()
        .map(|ns| {
            if ns.is_empty() {
                0.0
            } else {
                ns.iter().map(|&j| values[j as usize]).sum::<f64>() / ns.len() as f64
            }
        })
        .collect())
}

pub fn gradient(mesh: &Mesh, values: &[f64]) -> Result<Vec<DVec3>> {
    validate_mesh(mesh)?;
    if values.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(mesh
        .positions
        .par_iter()
        .zip(&mesh.neighbors)
        .enumerate()
        .map(|(i, (p, ns))| {
            if ns.is_empty() {
                return DVec3::ZERO;
            }
            let mut g = DVec3::ZERO;
            for &j in ns {
                let q = mesh.positions[j as usize];
                let d = q - *p;
                let d2 = d.length_squared();
                if d2 > 1e-24 {
                    g += d * ((values[j as usize] - values[i]) / d2);
                }
            }
            let tangent = g - *p * g.dot(*p) / p.length_squared().max(1e-24);
            tangent / ns.len() as f64
        })
        .collect())
}

pub fn laplacian(mesh: &Mesh, values: &[f64]) -> Result<Vec<f64>> {
    let means = neighbor_sample(mesh, values)?;
    Ok(means.iter().zip(values).map(|(m, v)| m - v).collect())
}

pub fn diffuse(mesh: &Mesh, values: &[f64], rate: f64, iterations: usize) -> Result<Vec<f64>> {
    if !(0.0..=1.0).contains(&rate) {
        return Err(OperatorError::Invalid(
            "diffusion rate must be in [0,1]".into(),
        ));
    }
    let mut state = values.to_vec();
    for _ in 0..iterations {
        let avg = neighbor_sample(mesh, &state)?;
        state
            .par_iter_mut()
            .zip(avg)
            .for_each(|(v, a)| *v = (1.0 - rate) * *v + rate * a);
    }
    Ok(state)
}

pub fn boundary_strength(mesh: &Mesh, labels: &[u32]) -> Result<Vec<f64>> {
    validate_mesh(mesh)?;
    if labels.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(mesh
        .neighbors
        .par_iter()
        .enumerate()
        .map(|(i, ns)| {
            if ns.is_empty() {
                0.0
            } else {
                ns.iter()
                    .filter(|&&j| labels[j as usize] != labels[i])
                    .count() as f64
                    / ns.len() as f64
            }
        })
        .collect())
}

pub fn distance_to_mask(mesh: &Mesh, mask: &[bool]) -> Result<Vec<f64>> {
    validate_mesh(mesh)?;
    if mask.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    let mut d = vec![f64::INFINITY; mask.len()];
    let mut frontier = Vec::new();
    for (i, &m) in mask.iter().enumerate() {
        if m {
            d[i] = 0.0;
            frontier.push(i);
        }
    }
    let mut level = 0.0;
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for i in frontier {
            for &j in &mesh.neighbors[i] {
                let j = j as usize;
                if d[j].is_infinite() {
                    d[j] = level + 1.0;
                    next.push(j);
                }
            }
        }
        frontier = next;
        level += 1.0;
    }
    Ok(d.into_iter()
        .map(|v| if v.is_infinite() { 0.0 } else { v })
        .collect())
}

pub fn flow_direction(mesh: &Mesh, potential: &[f64]) -> Result<Vec<CellId>> {
    validate_mesh(mesh)?;
    if potential.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(mesh
        .neighbors
        .par_iter()
        .enumerate()
        .map(|(i, ns)| {
            ns.iter()
                .copied()
                .filter(|&j| potential[j as usize] < potential[i])
                .min_by(|&a, &b| potential[a as usize].total_cmp(&potential[b as usize]))
                .unwrap_or(i as CellId)
        })
        .collect())
}

pub fn accumulate(receivers: &[CellId], values: &[f64]) -> Result<Vec<f64>> {
    if receivers.len() != values.len() || receivers.iter().any(|&r| r as usize >= values.len()) {
        return Err(OperatorError::Invalid("invalid receiver field".into()));
    }
    let n = values.len();
    let mut out = values.to_vec();
    let mut remaining = vec![0usize; n];
    for (i, &r) in receivers.iter().enumerate() {
        if i as CellId != r {
            remaining[r as usize] += 1;
        }
    }
    let mut leaves: Vec<usize> = (0..n).filter(|&i| remaining[i] == 0).collect();
    let mut done = 0;
    while let Some(i) = leaves.pop() {
        done += 1;
        let r = receivers[i] as usize;
        if r != i {
            out[r] += out[i];
            remaining[r] -= 1;
            if remaining[r] == 0 {
                leaves.push(r);
            }
        }
    }
    if done != n {
        return Err(OperatorError::Invalid(
            "receiver graph contains a cycle".into(),
        ));
    }
    Ok(out)
}

pub fn reduce(values: &[f64], operation: &str) -> Result<f64> {
    if values.is_empty() {
        return Err(OperatorError::Invalid("cannot reduce empty field".into()));
    }
    match operation {
        "sum" => Ok(values.iter().sum()),
        "mean" => Ok(values.iter().sum::<f64>() / values.len() as f64),
        "min" => Ok(values.iter().copied().fold(f64::INFINITY, f64::min)),
        "max" => Ok(values.iter().copied().fold(f64::NEG_INFINITY, f64::max)),
        _ => Err(OperatorError::Invalid(format!(
            "unknown reduction {operation}"
        ))),
    }
}

pub fn vector_expr(mesh: &Mesh, vectors: &[DVec3]) -> Result<Vec<DVec3>> {
    if vectors.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(vectors
        .par_iter()
        .zip(&mesh.positions)
        .map(|(v, p)| *v - *p * (v.dot(*p) / p.length_squared().max(1e-24)))
        .collect())
}

/// Validated CEL program reused to execute the whole dense field.
pub struct PointwiseProgram(cel::Program);

impl PointwiseProgram {
    pub fn compile(source: &str) -> Result<Self> {
        cel::Program::compile(source)
            .map(Self)
            .map_err(|e| OperatorError::Cel(e.to_string()))
    }

    pub fn execute(
        &self,
        mesh: &Mesh,
        bindings: &BTreeMap<String, Vec<f64>>,
        parameters: &BTreeMap<String, f64>,
    ) -> Result<Vec<f64>> {
        if bindings
            .values()
            .any(|values| values.len() != mesh.positions.len())
        {
            return Err(OperatorError::Invalid(
                "pointwise binding length mismatch".into(),
            ));
        }
        (0..mesh.positions.len())
            .into_par_iter()
            .map(|i| {
                let mut context = cel::Context::default();
                for (name, values) in bindings {
                    context
                        .add_variable(name, values[i])
                        .map_err(|e| OperatorError::Cel(e.to_string()))?;
                }
                for (name, &value) in parameters {
                    context
                        .add_variable(name, value)
                        .map_err(|e| OperatorError::Cel(e.to_string()))?;
                }
                let value = self
                    .0
                    .execute(&context)
                    .map_err(|e| OperatorError::Cel(e.to_string()))?;
                match value {
                    cel::Value::Float(v) => Ok(v),
                    cel::Value::Int(v) => Ok(v as f64),
                    cel::Value::UInt(v) => Ok(v as f64),
                    _ => Err(OperatorError::Cel(
                        "pointwise CEL expression must return a number".into(),
                    )),
                }
            })
            .collect()
    }
}

pub fn pointwise(
    mesh: &Mesh,
    source: &str,
    bindings: &BTreeMap<String, Vec<f64>>,
    parameters: &BTreeMap<String, f64>,
) -> Result<Vec<f64>> {
    PointwiseProgram::compile(source)?.execute(mesh, bindings, parameters)
}

/// Evaluate the three component expressions and project their result into each
/// cell's local tangent plane.
pub fn vector_expression(
    mesh: &Mesh,
    sources: &[String],
    bindings: &BTreeMap<String, Vec<f64>>,
    parameters: &BTreeMap<String, f64>,
) -> Result<Vec<DVec3>> {
    if sources.len() != 3 {
        return Err(OperatorError::Cel(format!(
            "vector_expr requires exactly 3 CEL expressions for x/y/z components; got {}",
            sources.len()
        )));
    }
    let x = pointwise(mesh, &sources[0], bindings, parameters)?;
    let y = pointwise(mesh, &sources[1], bindings, parameters)?;
    let z = pointwise(mesh, &sources[2], bindings, parameters)?;
    let vectors: Vec<_> = x
        .into_iter()
        .zip(y)
        .zip(z)
        .map(|((x, y), z)| DVec3::new(x, y, z))
        .collect();
    vector_expr(mesh, &vectors)
}

/// Run the IR in its supplied topological order; expression argument conventions are
/// deliberately read from the node's named bindings and the frozen `args` contract.
pub fn execute(program: &Program, state: &WorldState) -> Result<FieldStore> {
    execute_with_inputs(program, state, &BTreeMap::new())
}

/// Execute a topologically ordered program with executor-local external inputs.
pub fn execute_with_inputs(
    program: &Program,
    state: &WorldState,
    inputs: &BTreeMap<String, RuntimeValue>,
) -> Result<FieldStore> {
    let referenced_inputs: std::collections::BTreeSet<_> = program
        .nodes
        .iter()
        .flat_map(|node| node.args.values())
        .filter_map(|value| match value {
            ValueRef::Input(name) => Some(name.as_str()),
            _ => None,
        })
        .collect();
    if let Some(unused) = inputs
        .keys()
        .find(|name| !referenced_inputs.contains(name.as_str()))
    {
        return Err(OperatorError::UnknownInput(unused.clone()));
    }
    let mut store = FieldStore::default();
    for node in &program.nodes {
        let scalar = |name: &str| -> Result<f64> {
            match node.args.get(name) {
                Some(ValueRef::LiteralScalar(v)) => Ok(*v),
                Some(ValueRef::Parameter(p)) => state
                    .parameters
                    .get(p)
                    .copied()
                    .ok_or_else(|| OperatorError::Invalid(format!("missing parameter {p}"))),
                Some(ValueRef::Input(input)) => match inputs.get(input) {
                    Some(RuntimeValue::Scalar(value)) => Ok(*value),
                    Some(_) => Err(OperatorError::InputType {
                        name: input.clone(),
                        expected: "Scalar",
                    }),
                    None => Err(OperatorError::MissingInput(input.clone())),
                },
                _ => Err(OperatorError::Invalid(format!(
                    "missing scalar argument {name}"
                ))),
            }
        };
        let field = |name: &str| -> Result<Field> {
            match node.args.get(name) {
                Some(ValueRef::State(s)) => state
                    .fields
                    .get(s)
                    .cloned()
                    .ok_or_else(|| OperatorError::Invalid(format!("missing state field {s}"))),
                Some(ValueRef::NodeOutput { node: n, output }) => {
                    match store.get(&format!("{n}.{output}")) {
                        Some(Value::Field(f)) => Ok(f.clone()),
                        _ => Err(OperatorError::Invalid(format!(
                            "missing output {n}.{output}"
                        ))),
                    }
                }
                Some(ValueRef::Input(input)) => match inputs.get(input) {
                    Some(RuntimeValue::Field(field)) => Ok(field.clone()),
                    Some(_) => Err(OperatorError::InputType {
                        name: input.clone(),
                        expected: "Field",
                    }),
                    None => Err(OperatorError::MissingInput(input.clone())),
                },
                _ => Err(OperatorError::Invalid(format!(
                    "missing field argument {name}"
                ))),
            }
        };
        let expression_bindings = || -> Result<CelBindings> {
            let mut fields = BTreeMap::new();
            let mut scalars = BTreeMap::new();
            for (name, reference) in &node.args {
                match reference {
                    ValueRef::LiteralScalar(value) => {
                        scalars.insert(name.clone(), *value);
                    }
                    ValueRef::Parameter(parameter) => {
                        let value = state.parameters.get(parameter).copied().ok_or_else(|| {
                            OperatorError::Invalid(format!("missing parameter {parameter}"))
                        })?;
                        scalars.insert(name.clone(), value);
                    }
                    ValueRef::Input(input) => match inputs.get(input) {
                        Some(RuntimeValue::Scalar(value)) => {
                            scalars.insert(name.clone(), *value);
                        }
                        Some(RuntimeValue::Field(Field::Scalar(values))) => {
                            fields.insert(name.clone(), values.clone());
                        }
                        Some(_) => {
                            return Err(OperatorError::InputType {
                                name: input.clone(),
                                expected: "Scalar or ScalarField",
                            });
                        }
                        None => return Err(OperatorError::MissingInput(input.clone())),
                    },
                    ValueRef::State(field_name) => match state.fields.get(field_name) {
                        Some(Field::Scalar(values)) => {
                            fields.insert(name.clone(), values.clone());
                        }
                        Some(_) => {
                            return Err(OperatorError::Invalid(format!(
                                "CEL binding {name} must reference a scalar field"
                            )));
                        }
                        None => {
                            return Err(OperatorError::Invalid(format!(
                                "missing state field {field_name}"
                            )));
                        }
                    },
                    ValueRef::NodeOutput {
                        node: source_node,
                        output,
                    } => match store.get(&format!("{source_node}.{output}")) {
                        Some(Value::Field(Field::Scalar(values))) => {
                            fields.insert(name.clone(), values.clone());
                        }
                        Some(_) => {
                            return Err(OperatorError::Invalid(format!(
                                "CEL binding {name} must reference a scalar field"
                            )));
                        }
                        None => {
                            return Err(OperatorError::Invalid(format!(
                                "missing node output {source_node}.{output}"
                            )));
                        }
                    },
                }
            }
            Ok((fields, scalars))
        };
        let output = match node.op.as_str() {
            "constant" => Value::Field(Field::Scalar(constant(&state.mesh, scalar("value")?))),
            "noise" => {
                let scale = match node.args.get("scale") {
                    Some(_) => scalar("scale")?,
                    None => 1.0,
                };
                Value::Field(Field::Scalar(noise(&state.mesh, state.seed, &node.id, scale)))
            }
            "neighbor_sample" => match field("field")? {
                Field::Scalar(v) => Value::Field(Field::Scalar(neighbor_sample(&state.mesh, &v)?)),
                _ => return Err(OperatorError::Invalid("field must be scalar".into())),
            },
            "gradient" => match field("field")? {
                Field::Scalar(v) => Value::Field(Field::Vector(gradient(&state.mesh, &v)?)),
                _ => return Err(OperatorError::Invalid("field must be scalar".into())),
            },
            "laplacian" => match field("field")? {
                Field::Scalar(v) => Value::Field(Field::Scalar(laplacian(&state.mesh, &v)?)),
                _ => return Err(OperatorError::Invalid("field must be scalar".into())),
            },
            "diffuse" => match field("field")? {
                Field::Scalar(v) => {
                    let count = scalar("iterations")?;
                    if count < 0.0 || count.fract() != 0.0 || count > usize::MAX as f64 {
                        return Err(OperatorError::Invalid("diffuse iterations must be a nonnegative integer".into()));
                    }
                    Value::Field(Field::Scalar(diffuse(&state.mesh, &v, scalar("rate")?, count as usize)?))
                }
                _ => return Err(OperatorError::Invalid("field must be scalar".into())),
            },
            "boundary_strength" => match field("labels")? {
                Field::Category(v) => {
                    Value::Field(Field::Scalar(boundary_strength(&state.mesh, &v)?))
                }
                _ => {
                    return Err(OperatorError::Invalid(
                        "labels must be category field".into(),
                    ));
                }
            },
            "distance_to_mask" => match field("mask")? {
                Field::Bool(v) => Value::Field(Field::Scalar(distance_to_mask(&state.mesh, &v)?)),
                _ => return Err(OperatorError::Invalid("mask must be boolean field".into())),
            },
            "flow_direction" => match field("potential")? {
                Field::Scalar(v) => Value::Field(Field::Index(flow_direction(&state.mesh, &v)?)),
                _ => return Err(OperatorError::Invalid("potential must be scalar".into())),
            },
            "accumulate" => {
                let a = field("receivers")?;
                let b = field("values")?;
                match (a, b) {
                    (Field::Index(r), Field::Scalar(v)) => {
                        Value::Field(Field::Scalar(accumulate(&r, &v)?))
                    }
                    _ => {
                        return Err(OperatorError::Invalid(
                            "accumulate requires index receivers and scalar values".into(),
                        ));
                    }
                }
            }
            "reduce" => return Err(OperatorError::UnsupportedConfiguration(
                "reduce requires a string operation (mean/min/max/sum), but frozen ValueRef represents only scalar literals, fields, parameters, and references; dispatch cannot select an operation".into(),
            )),
            "voronoi_labels" | "advect" | "network_threshold" => {
                return Err(OperatorError::NotImplemented(node.op.clone()));
            }
            "pointwise" => {
                if node.cel.len() != 1 {
                    return Err(OperatorError::Cel(format!("pointwise node {} requires exactly 1 CEL expression; got {}", node.id, node.cel.len())));
                }
                let (bindings, parameters) = expression_bindings()?;
                Value::Field(Field::Scalar(pointwise(&state.mesh, node.cel[0].source(), &bindings, &parameters)?))
            }
            "vector_expr" => {
                let (bindings, parameters) = expression_bindings()?;
                let sources: Vec<_> = node.cel.iter().map(|expression| expression.source().to_owned()).collect();
                Value::Field(Field::Vector(vector_expression(&state.mesh, &sources, &bindings, &parameters)?))
            }
            _ => {
                return Err(OperatorError::Invalid(format!(
                    "unknown operator {}",
                    node.op
                )));
            }
        };
        store.insert(format!("{}.value", node.id), output);
    }
    Ok(store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use muse_types::{CompiledExpressionHandle, CompiledNode, StateUpdate};
    fn line_mesh(n: usize) -> Mesh {
        Mesh {
            positions: (0..n).map(|i| DVec3::new(i as f64, 0.0, 0.0)).collect(),
            triangles: vec![],
            neighbors: (0..n)
                .map(|i| {
                    [i.checked_sub(1), (i + 1 < n).then_some(i + 1)]
                        .into_iter()
                        .flatten()
                        .map(|v| v as u32)
                        .collect()
                })
                .collect(),
        }
    }
    #[test]
    fn constant_and_noise_are_stable_and_node_keyed() {
        let m = line_mesh(16);
        assert_eq!(constant(&m, 2.5), vec![2.5; 16]);
        assert_eq!(noise(&m, 7, "a", 1.0), noise(&m, 7, "a", 1.0));
        assert_ne!(noise(&m, 7, "a", 1.0), noise(&m, 7, "b", 1.0));
    }
    #[test]
    fn pointwise_executes_cel_over_named_dense_bindings() {
        let m = line_mesh(4);
        let mut b = BTreeMap::new();
        b.insert("x".into(), vec![0., 1., 2., 3.]);
        let out = pointwise(&m, "x * 2.0 + 1.0", &b, &BTreeMap::new()).unwrap();
        for (actual, expected) in out.iter().zip([1., 3., 5., 7.]) {
            assert!((actual - expected).abs() < 1e-12);
        }
    }
    #[test]
    fn graph_kernels_have_expected_values() {
        let m = line_mesh(3);
        assert_eq!(
            neighbor_sample(&m, &[2., 4., 8.]).unwrap(),
            vec![4., 5., 4.]
        );
        assert_eq!(
            accumulate(&[1, 2, 2], &[1., 2., 3.]).unwrap(),
            vec![1., 3., 6.]
        );
        assert_eq!(reduce(&[1., 2., 3.], "mean").unwrap(), 2.);
        assert_eq!(reduce(&[1., 2., 3.], "min").unwrap(), 1.);
        assert_eq!(reduce(&[1., 2., 3.], "max").unwrap(), 3.);
        assert_eq!(reduce(&[1., 2., 3.], "sum").unwrap(), 6.);
        assert_eq!(
            accumulate(&[1, 3, 3, 3], &[1., 2., 3., 4.]).unwrap(),
            vec![1., 3., 3., 10.]
        );
    }
    #[test]
    fn uniform_and_diffusion_invariants() {
        let m = line_mesh(6);
        let constant_gradient = gradient(&m, &[4.; 6]).unwrap();
        assert!(constant_gradient.iter().all(|v| v.length() < 1e-10));
        assert!(
            laplacian(&m, &[4.; 6])
                .unwrap()
                .iter()
                .all(|x| x.abs() < 1e-12)
        );
        assert_eq!(diffuse(&m, &[4.; 6], 0.5, 3).unwrap(), vec![4.; 6]);
        assert_eq!(boundary_strength(&m, &[1; 6]).unwrap(), vec![0.; 6]);
        let before = [0., 1., 0., 3., -1., 2.];
        let after = diffuse(&m, &before, 0.2, 1).unwrap();
        let variance = |values: &[f64]| {
            let mean = values.iter().sum::<f64>() / values.len() as f64;
            values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / values.len() as f64
        };
        assert!(variance(&after) <= variance(&before) + 1e-12);
    }
    #[test]
    fn distance_and_flow_are_valid() {
        let m = line_mesh(4);
        assert_eq!(
            distance_to_mask(&m, &[true, false, false, false]).unwrap(),
            vec![0., 1., 2., 3.]
        );
        assert_eq!(
            flow_direction(&m, &[3., 2., 1., 0.]).unwrap(),
            vec![1, 2, 3, 3]
        );
        let receivers = flow_direction(&m, &[3., 2., 1., 0.]).unwrap();
        for (i, &r) in receivers.iter().enumerate() {
            assert!(r as usize == i || m.neighbors[i].contains(&r));
            if r as usize != i {
                assert!([3., 2., 1., 0.][r as usize] < [3., 2., 1., 0.][i]);
            }
        }
        let distances = distance_to_mask(&m, &[true, false, false, false]).unwrap();
        assert_eq!(distances[0], 0.0);
        assert!(distances[1..].iter().all(|d| *d >= 0.0));
    }

    fn sphere_mesh() -> Mesh {
        let positions = vec![DVec3::X, DVec3::Y, DVec3::Z, -DVec3::X];
        Mesh {
            positions,
            triangles: vec![],
            neighbors: vec![vec![1, 2, 3], vec![0, 2], vec![0, 1, 3], vec![0, 2]],
        }
    }

    #[test]
    fn generic_program_dispatches_pointwise_then_tangent_vector_expr() {
        let mesh = sphere_mesh();
        let state = WorldState {
            mesh: mesh.clone(),
            fields: BTreeMap::from([("source".into(), Field::Scalar(vec![1.0, 2.0, 3.0, 4.0]))]),
            networks: BTreeMap::new(),
            parameters: BTreeMap::from([("factor".into(), 2.0)]),
            step: 0,
            seed: 11,
        };
        let program = Program {
            nodes: vec![
                CompiledNode {
                    id: "p".into(),
                    op: "pointwise".into(),
                    args: BTreeMap::from([
                        ("x".into(), ValueRef::State("source".into())),
                        ("factor".into(), ValueRef::Parameter("factor".into())),
                        ("offset".into(), ValueRef::LiteralScalar(1.0)),
                    ]),
                    cel: vec![CompiledExpressionHandle::from_source("x * factor + offset")],
                },
                CompiledNode {
                    id: "v".into(),
                    op: "vector_expr".into(),
                    args: BTreeMap::from([
                        (
                            "x".into(),
                            ValueRef::NodeOutput {
                                node: "p".into(),
                                output: "value".into(),
                            },
                        ),
                        ("zero".into(), ValueRef::LiteralScalar(0.0)),
                    ]),
                    cel: vec![
                        CompiledExpressionHandle::from_source("x"),
                        CompiledExpressionHandle::from_source("zero"),
                        CompiledExpressionHandle::from_source("1.0"),
                    ],
                },
            ],
            updates: Vec::<StateUpdate>::new(),
        };
        let output = execute(&program, &state).unwrap();
        assert_eq!(
            output.scalar_field("p.value").unwrap(),
            &[3.0, 5.0, 7.0, 9.0]
        );
        let Value::Field(Field::Vector(vectors)) = output.get("v.value").unwrap() else {
            panic!("expected vector field");
        };
        for (p, v) in mesh.positions.iter().zip(vectors) {
            assert!(p.dot(*v).abs() < 1e-10);
        }
    }

    #[test]
    fn external_scalar_field_and_scalar_inputs_resolve_in_cel_and_fixed_ports() {
        let mesh = line_mesh(4);
        let state = WorldState {
            mesh,
            fields: BTreeMap::new(),
            networks: BTreeMap::new(),
            parameters: BTreeMap::new(),
            step: 0,
            seed: 0,
        };
        let program = Program {
            nodes: vec![
                CompiledNode {
                    id: "p".into(),
                    op: "pointwise".into(),
                    args: BTreeMap::from([
                        ("x".into(), ValueRef::Input("x".into())),
                        ("bias".into(), ValueRef::Input("bias".into())),
                    ]),
                    cel: vec![CompiledExpressionHandle::from_source("x + bias")],
                },
                CompiledNode {
                    id: "n".into(),
                    op: "neighbor_sample".into(),
                    args: BTreeMap::from([("field".into(), ValueRef::Input("x".into()))]),
                    cel: vec![],
                },
            ],
            updates: vec![],
        };
        let inputs = BTreeMap::from([
            (
                "x".into(),
                RuntimeValue::Field(Field::Scalar(vec![1.0, 2.0, 3.0, 4.0])),
            ),
            ("bias".into(), RuntimeValue::Scalar(10.0)),
        ]);
        let output = execute_with_inputs(&program, &state, &inputs).unwrap();
        assert_eq!(
            output.scalar_field("p.value").unwrap(),
            &[11.0, 12.0, 13.0, 14.0]
        );
        assert_eq!(
            output.scalar_field("n.value").unwrap(),
            &[2.0, 2.0, 3.0, 3.0]
        );
    }

    #[test]
    fn external_input_errors_distinguish_missing_unknown_and_type() {
        let state = WorldState {
            mesh: line_mesh(2),
            fields: BTreeMap::new(),
            networks: BTreeMap::new(),
            parameters: BTreeMap::new(),
            step: 0,
            seed: 0,
        };
        let program = Program {
            nodes: vec![CompiledNode {
                id: "p".into(),
                op: "pointwise".into(),
                args: BTreeMap::from([("x".into(), ValueRef::Input("x".into()))]),
                cel: vec![CompiledExpressionHandle::from_source("x * 2.0")],
            }],
            updates: vec![],
        };
        assert!(
            matches!(execute(&program, &state), Err(OperatorError::MissingInput(name)) if name == "x")
        );
        assert!(matches!(
            execute_with_inputs(&program, &state, &BTreeMap::from([("unused".into(), RuntimeValue::Scalar(1.0))])),
            Err(OperatorError::UnknownInput(name)) if name == "unused"
        ));

        let fixed_port_program = Program {
            nodes: vec![CompiledNode {
                id: "n".into(),
                op: "neighbor_sample".into(),
                args: BTreeMap::from([("field".into(), ValueRef::Input("field".into()))]),
                cel: vec![],
            }],
            updates: vec![],
        };
        assert!(matches!(
            execute_with_inputs(&fixed_port_program, &state, &BTreeMap::from([("field".into(), RuntimeValue::Scalar(2.0))])),
            Err(OperatorError::InputType { name, expected: "Field" }) if name == "field"
        ));
    }

    #[test]
    fn vector_expr_rejects_wrong_component_count_and_nonnumeric_results() {
        let m = sphere_mesh();
        assert!(
            vector_expression(&m, &["1.0".into()], &BTreeMap::new(), &BTreeMap::new())
                .unwrap_err()
                .to_string()
                .contains("exactly 3")
        );
        assert!(
            vector_expression(
                &m,
                &["true".into(), "0.0".into(), "0.0".into()],
                &BTreeMap::new(),
                &BTreeMap::new()
            )
            .unwrap_err()
            .to_string()
            .contains("number")
        );
    }

    #[test]
    fn parallel_operators_are_thread_count_deterministic() {
        let mesh = sphere_mesh();
        let values = vec![0.25, 1.5, -2.0, 3.0];
        let labels = vec![0, 0, 1, 1];
        let mask = vec![true, false, false, true];
        let bindings = BTreeMap::from([("x".into(), values.clone())]);
        let expr = PointwiseProgram::compile("x * 1.25 + 0.5").unwrap();
        let single = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let multi = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        let run = |pool: &rayon::ThreadPool| {
            pool.install(|| {
                (
                    noise(&mesh, 44, "node", 1.0),
                    neighbor_sample(&mesh, &values).unwrap(),
                    gradient(&mesh, &values).unwrap(),
                    laplacian(&mesh, &values).unwrap(),
                    diffuse(&mesh, &values, 0.2, 2).unwrap(),
                    boundary_strength(&mesh, &labels).unwrap(),
                    flow_direction(&mesh, &values).unwrap(),
                    vector_expr(&mesh, &[DVec3::ONE; 4]).unwrap(),
                    expr.execute(&mesh, &bindings, &BTreeMap::new()).unwrap(),
                    vector_expression(
                        &mesh,
                        &["x".into(), "0.0".into(), "1.0".into()],
                        &bindings,
                        &BTreeMap::new(),
                    )
                    .unwrap(),
                )
            })
        };
        assert_eq!(run(&single), run(&multi));
        assert_eq!(
            distance_to_mask(&mesh, &mask).unwrap(),
            single.install(|| distance_to_mask(&mesh, &mask).unwrap())
        );
    }

    #[test]
    fn generic_reduce_reports_unrepresentable_operation_configuration() {
        let mesh = line_mesh(2);
        let state = WorldState {
            mesh,
            fields: BTreeMap::from([("x".into(), Field::Scalar(vec![1., 2.]))]),
            networks: BTreeMap::new(),
            parameters: BTreeMap::new(),
            step: 0,
            seed: 0,
        };
        let program = Program {
            nodes: vec![CompiledNode {
                id: "r".into(),
                op: "reduce".into(),
                args: BTreeMap::from([("values".into(), ValueRef::State("x".into()))]),
                cel: vec![],
            }],
            updates: vec![],
        };
        assert!(matches!(
            execute(&program, &state),
            Err(OperatorError::UnsupportedConfiguration(_))
        ));
    }
}
