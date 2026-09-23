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
}

pub type Result<T> = std::result::Result<T, OperatorError>;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
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

/// Run the IR in its supplied topological order; expression argument conventions are
/// deliberately read from the node's named bindings and the frozen `args` contract.
pub fn execute(program: &Program, state: &WorldState) -> Result<FieldStore> {
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
                _ => Err(OperatorError::Invalid(format!(
                    "missing field argument {name}"
                ))),
            }
        };
        let output = match node.op.as_str() {
            "constant" => Value::Field(Field::Scalar(constant(&state.mesh, scalar("value")?))),
            "noise" => Value::Field(Field::Scalar(noise(
                &state.mesh,
                state.seed,
                &node.id,
                scalar("scale").unwrap_or(1.0),
            ))),
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
                Field::Scalar(v) => Value::Field(Field::Scalar(diffuse(
                    &state.mesh,
                    &v,
                    scalar("rate")?,
                    scalar("iterations")? as usize,
                )?)),
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
            "reduce" => match field("values")? {
                Field::Scalar(v) => Value::Scalar(reduce(&v, "mean")?),
                _ => return Err(OperatorError::Invalid("values must be scalar".into())),
            },
            "voronoi_labels" | "advect" | "network_threshold" => {
                return Err(OperatorError::NotImplemented(node.op.clone()));
            }
            "pointwise" | "vector_expr" => return Err(OperatorError::Cel(
                "expression execution requires CEL binding configuration; use execute_expression"
                    .into(),
            )),
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
    }
    #[test]
    fn uniform_and_diffusion_invariants() {
        let m = line_mesh(6);
        assert!(
            laplacian(&m, &[4.; 6])
                .unwrap()
                .iter()
                .all(|x| x.abs() < 1e-12)
        );
        assert_eq!(diffuse(&m, &[4.; 6], 0.5, 3).unwrap(), vec![4.; 6]);
        assert_eq!(boundary_strength(&m, &[1; 6]).unwrap(), vec![0.; 6]);
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
    }
}
