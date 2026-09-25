//! Dense, eager execution for the frozen MUSE operator algebra.

use std::collections::BTreeMap;

use glam::DVec3;
use muse_types::{CellId, Field, Mesh, Network, Program, ValueRef, WorldState};
use rayon::prelude::*;
use thiserror::Error;

pub mod wave3;

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

/// A discrete component's size and unit-sphere geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ComponentSummary {
    pub cell_count: usize,
    pub area: f64,
    pub perimeter: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ComponentMeasure {
    Area,
    Perimeter,
    CellCount,
}

/// Label true cells by connected component, ordered by each component's minimum cell ID.
pub fn connected_components(mesh: &Mesh, mask: &[bool]) -> Result<Vec<u32>> {
    validate_component_mesh(mesh)?;
    let n = mesh.positions.len();
    if mask.len() != n || n > u32::MAX as usize {
        return Err(OperatorError::Invalid(
            "mask length mismatch or mesh too large".into(),
        ));
    }
    let mut labels = vec![0; n];
    let mut next_id = 1u32;
    let mut queue = std::collections::VecDeque::new();
    for start in 0..n {
        if !mask[start] || labels[start] != 0 {
            continue;
        }
        labels[start] = next_id;
        queue.push_back(start);
        while let Some(i) = queue.pop_front() {
            for &neighbor in &mesh.neighbors[i] {
                let j = neighbor as usize;
                if mask[j] && labels[j] == 0 {
                    labels[j] = next_id;
                    queue.push_back(j);
                }
            }
        }
        next_id += 1;
    }
    Ok(labels)
}

/// Summarize positive category labels using spherical triangle vertex shares and
/// unique undirected geodesic edges. Index zero in the result is background.
pub fn component_summaries(mesh: &Mesh, labels: &[u32]) -> Result<Vec<ComponentSummary>> {
    validate_component_mesh(mesh)?;
    let n = mesh.positions.len();
    if labels.len() != n {
        return Err(OperatorError::Invalid("label length mismatch".into()));
    }
    let max_label = labels.iter().copied().max().unwrap_or(0) as usize;
    let unique_labels: std::collections::BTreeSet<_> = labels.iter().copied().collect();
    if unique_labels.iter().filter(|&&label| label != 0).count() != max_label {
        return Err(OperatorError::Invalid(
            "component labels must be dense with background zero".into(),
        ));
    }
    let mut summaries = vec![ComponentSummary::default(); max_label + 1];
    for &label in labels {
        summaries[label as usize].cell_count += 1;
    }
    let mut vertex_area = vec![0.0; n];
    for &[ai, bi, ci] in &mesh.triangles {
        let [a, b, c] = [ai, bi, ci].map(|id| id as usize);
        if a >= n || b >= n || c >= n || a == b || b == c || c == a {
            return Err(OperatorError::Invalid("invalid mesh triangle".into()));
        }
        let (a, b, c) = (
            mesh.positions[a].normalize(),
            mesh.positions[b].normalize(),
            mesh.positions[c].normalize(),
        );
        let area = 2.0
            * a.dot(b.cross(c))
                .abs()
                .atan2(1.0 + a.dot(b) + b.dot(c) + c.dot(a));
        for id in [ai, bi, ci] {
            vertex_area[id as usize] += area / 3.0;
        }
    }
    for (i, &label) in labels.iter().enumerate() {
        summaries[label as usize].area += vertex_area[i];
    }
    for (i, adjacent) in mesh.neighbors.iter().enumerate() {
        for &neighbor in adjacent {
            let j = neighbor as usize;
            if i >= j || labels[i] == labels[j] {
                continue;
            }
            let length = mesh.positions[i]
                .normalize()
                .dot(mesh.positions[j].normalize())
                .clamp(-1.0, 1.0)
                .acos();
            if labels[i] != 0 {
                summaries[labels[i] as usize].perimeter += length;
            }
            if labels[j] != 0 {
                summaries[labels[j] as usize].perimeter += length;
            }
        }
    }
    Ok(summaries)
}

/// Broadcast a component summary measure to cells; label zero always broadcasts zero.
pub fn component_measure_broadcast(
    labels: &[u32],
    summaries: &[ComponentSummary],
    measure: ComponentMeasure,
) -> Result<Vec<f64>> {
    let expected_len = labels.iter().copied().max().unwrap_or(0) as usize + 1;
    if summaries.len() != expected_len {
        return Err(OperatorError::Invalid(
            "component labels and summaries are not aligned".into(),
        ));
    }
    Ok(labels
        .iter()
        .map(|&label| {
            if label == 0 {
                return 0.0;
            }
            let summary = summaries[label as usize];
            match measure {
                ComponentMeasure::Area => summary.area,
                ComponentMeasure::Perimeter => summary.perimeter,
                ComponentMeasure::CellCount => summary.cell_count as f64,
            }
        })
        .collect())
}

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

fn validate_component_mesh(mesh: &Mesh) -> Result<()> {
    validate_mesh(mesh)?;
    let n = mesh.positions.len();
    for (i, adjacent) in mesh.neighbors.iter().enumerate() {
        let mut unique = std::collections::BTreeSet::new();
        for &neighbor in adjacent {
            let j = neighbor as usize;
            if j >= n {
                return Err(OperatorError::Invalid("neighbor id out of bounds".into()));
            }
            if j == i || !unique.insert(neighbor) {
                return Err(OperatorError::Invalid(
                    "self or duplicate neighbor entry".into(),
                ));
            }
            if !mesh.neighbors[j].contains(&(i as CellId)) {
                return Err(OperatorError::Invalid("asymmetric mesh adjacency".into()));
            }
        }
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

/// Immutable geometric data shared by spatial operators for one mesh execution.
#[derive(Clone, Debug)]
pub struct MeshCalibration {
    positions: Vec<DVec3>,
    areas: Vec<f64>,
    adjacency: Vec<Vec<usize>>,
    max_edge: f64,
}

impl MeshCalibration {
    pub fn new(mesh: &Mesh) -> Result<Self> {
        validate_mesh(mesh)?;
        let positions = mesh
            .positions
            .iter()
            .map(|p| {
                let length = p.length();
                if !p.is_finite() || !length.is_finite() || length <= 1e-15 {
                    return None;
                }
                Some(*p / length)
            })
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                OperatorError::Invalid("mesh contains invalid spherical position".into())
            })?;
        if positions.is_empty() {
            return Err(OperatorError::Invalid(
                "mesh must contain spherical vertices".into(),
            ));
        }
        let mut areas = vec![0.0; positions.len()];
        for triangle in &mesh.triangles {
            let [a, b, c] = triangle.map(|id| id as usize);
            if a >= positions.len()
                || b >= positions.len()
                || c >= positions.len()
                || a == b
                || b == c
                || a == c
            {
                return Err(OperatorError::Invalid("invalid mesh triangle".into()));
            }
            let (pa, pb, pc) = (positions[a], positions[b], positions[c]);
            let excess = 2.0
                * pa.dot(pb.cross(pc))
                    .abs()
                    .atan2(1.0 + pa.dot(pb) + pb.dot(pc) + pc.dot(pa));
            if !excess.is_finite() || excess <= 0.0 {
                return Err(OperatorError::Invalid(
                    "degenerate spherical triangle".into(),
                ));
            }
            for id in [a, b, c] {
                areas[id] += excess / 3.0;
            }
        }
        if !positions.is_empty() && areas.iter().any(|a| !a.is_finite() || *a <= 0.0) {
            return Err(OperatorError::Invalid(
                "mesh has no valid spherical vertex areas".into(),
            ));
        }
        let mut adjacency = mesh
            .neighbors
            .iter()
            .map(|ns| {
                let mut values = ns.iter().map(|&j| j as usize).collect::<Vec<_>>();
                values.sort_unstable();
                values.dedup();
                values
            })
            .collect::<Vec<_>>();
        for (i, ns) in adjacency.iter().enumerate() {
            if ns.iter().any(|&j| adjacency[j].binary_search(&i).is_err()) {
                return Err(OperatorError::Invalid(
                    "mesh adjacency must be symmetric".into(),
                ));
            }
        }
        let normalized_positions = &positions;
        let max_edge = adjacency
            .iter()
            .enumerate()
            .flat_map(|(i, ns)| {
                ns.iter().map(move |&j| {
                    normalized_positions[i]
                        .dot(normalized_positions[j])
                        .clamp(-1.0, 1.0)
                        .acos()
                })
            })
            .fold(0.0_f64, f64::max);
        for ns in &mut adjacency {
            ns.shrink_to_fit();
        }
        let total_area = areas.iter().sum::<f64>();
        if !total_area.is_finite() || total_area <= 0.0 {
            return Err(OperatorError::Invalid(
                "invalid total spherical area".into(),
            ));
        }
        Ok(Self {
            positions,
            areas,
            adjacency,
            max_edge,
        })
    }

    pub fn smooth_radius(&self, values: &[f64], radius: f64) -> Result<Vec<f64>> {
        if values.len() != self.positions.len() {
            return Err(OperatorError::Invalid("field length mismatch".into()));
        }
        if values.iter().any(|v| !v.is_finite()) {
            return Err(OperatorError::Invalid(
                "field contains non-finite values".into(),
            ));
        }
        if !radius.is_finite() || radius < 0.0 {
            return Err(OperatorError::Invalid(
                "radius must be finite and nonnegative".into(),
            ));
        }
        if radius == 0.0 {
            return Ok(values.to_vec());
        }
        let r2 = radius * radius;
        let mut output = vec![0.0; values.len()];
        output
            .par_iter_mut()
            .enumerate()
            .try_for_each(|(i, target)| -> Result<()> {
                let mut seen = vec![false; values.len()];
                let mut frontier = vec![i];
                seen[i] = true;
                let mut weighted = 0.0;
                let mut total = 0.0;
                let mut cursor = 0;
                while cursor < frontier.len() {
                    let j = frontier[cursor];
                    cursor += 1;
                    let dot = self.positions[i].dot(self.positions[j]).clamp(-1.0, 1.0);
                    let d = dot.acos();
                    if d <= radius {
                        let q = (d * d / r2).min(1.0);
                        let w = (1.0 - q).powi(2) * self.areas[j];
                        weighted += w * values[j];
                        total += w;
                    }
                    if d <= radius + self.max_edge {
                        for &k in &self.adjacency[j] {
                            if !seen[k] {
                                seen[k] = true;
                                frontier.push(k);
                            }
                        }
                    }
                }
                if !total.is_finite() || total <= 0.0 || !weighted.is_finite() {
                    return Err(OperatorError::Invalid(
                        "invalid smoothing weight sum".into(),
                    ));
                }
                *target = weighted / total;
                Ok(())
            })?;
        let area_sum: f64 = self.areas.iter().sum();
        let mean_in = values
            .iter()
            .zip(&self.areas)
            .map(|(v, a)| v * a)
            .sum::<f64>()
            / area_sum;
        let mean_out = output
            .iter()
            .zip(&self.areas)
            .map(|(v, a)| v * a)
            .sum::<f64>()
            / area_sum;
        let delta = mean_in - mean_out;
        output.iter_mut().for_each(|v| *v += delta);
        if !delta.is_finite() || output.iter().any(|value| !value.is_finite()) {
            return Err(OperatorError::Invalid(
                "smoothing produced non-finite values".into(),
            ));
        }
        Ok(output)
    }

    /// Number of positively weighted source/destination pairs at this radius.
    pub fn neighborhood_count(&self, radius: f64) -> Result<usize> {
        if !radius.is_finite() || radius < 0.0 {
            return Err(OperatorError::Invalid(
                "radius must be finite and nonnegative".into(),
            ));
        }
        if radius == 0.0 {
            return Ok(self.positions.len());
        }
        let mut count = 0;
        for i in 0..self.positions.len() {
            let mut seen = vec![false; self.positions.len()];
            let mut pending = vec![i];
            seen[i] = true;
            let mut cursor = 0;
            while cursor < pending.len() {
                let j = pending[cursor];
                cursor += 1;
                let d = self.positions[i]
                    .dot(self.positions[j])
                    .clamp(-1.0, 1.0)
                    .acos();
                if d < radius {
                    count += 1;
                }
                if d <= radius + self.max_edge {
                    for &k in &self.adjacency[j] {
                        if !seen[k] {
                            seen[k] = true;
                            pending.push(k);
                        }
                    }
                }
            }
        }
        Ok(count)
    }

    pub fn area_weighted_mean_std(&self, values: &[f64]) -> Result<(f64, f64)> {
        if values.len() != self.areas.len() {
            return Err(OperatorError::Invalid("field length mismatch".into()));
        }
        let area = self.areas.iter().sum::<f64>();
        let mean = values
            .iter()
            .zip(&self.areas)
            .map(|(v, a)| v * a)
            .sum::<f64>()
            / area;
        let variance = values
            .iter()
            .zip(&self.areas)
            .map(|(v, a)| a * (v - mean).powi(2))
            .sum::<f64>()
            / area;
        if !mean.is_finite() || !variance.is_finite() {
            return Err(OperatorError::Invalid(
                "non-finite area-weighted statistics".into(),
            ));
        }
        Ok((mean, variance.sqrt()))
    }
}

/// One compact-support spherical convolution. For d < radius, w=(1-(d/radius)^2)^2.
pub fn smooth_radius(mesh: &Mesh, values: &[f64], radius: f64) -> Result<Vec<f64>> {
    MeshCalibration::new(mesh)?.smooth_radius(values, radius)
}

pub fn correlated_noise(
    mesh: &Mesh,
    seed: u64,
    node_key: &str,
    radius: f64,
    amplitude: f64,
    mean: f64,
) -> Result<Vec<f64>> {
    let calibration = MeshCalibration::new(mesh)?;
    correlated_noise_calibrated(&calibration, mesh, seed, node_key, radius, amplitude, mean)
}

pub fn correlated_noise_calibrated(
    calibration: &MeshCalibration,
    mesh: &Mesh,
    seed: u64,
    node_key: &str,
    radius: f64,
    amplitude: f64,
    mean: f64,
) -> Result<Vec<f64>> {
    if !amplitude.is_finite() || !mean.is_finite() {
        return Err(OperatorError::Invalid(
            "amplitude and mean must be finite".into(),
        ));
    }
    let smooth = calibration.smooth_radius(&noise(mesh, seed, node_key, 1.0), radius)?;
    let (actual_mean, stddev) = calibration.area_weighted_mean_std(&smooth)?;
    if stddev <= 1e-12 {
        return Ok(vec![mean; smooth.len()]);
    }
    let output = smooth
        .into_iter()
        .map(|v| mean + amplitude * (v - actual_mean) / stddev)
        .collect::<Vec<_>>();
    if output.iter().any(|value| !value.is_finite()) {
        return Err(OperatorError::Invalid(
            "correlated noise produced non-finite values".into(),
        ));
    }
    Ok(output)
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

/// Assign one deterministic scalar to every category, keyed by seed, node ID, and label.
/// Background category zero always maps to `offset` without keyed variation.
pub fn region_scalar(
    labels: &[u32],
    seed: u64,
    node_id: &str,
    scale: f64,
    offset: f64,
) -> Result<Vec<f64>> {
    if !scale.is_finite() || !offset.is_finite() {
        return Err(OperatorError::Invalid(
            "region_scalar scale and offset must be finite".into(),
        ));
    }
    let mut cache = BTreeMap::new();
    for &label in labels {
        cache.entry(label).or_insert_with(|| {
            if label == 0 {
                return offset;
            }
            let mut hash = blake3::Hasher::new();
            hash.update(&seed.to_le_bytes());
            hash.update(&(node_id.len() as u64).to_le_bytes());
            hash.update(node_id.as_bytes());
            hash.update(&label.to_le_bytes());
            let bits = u64::from_le_bytes(hash.finalize().as_bytes()[..8].try_into().unwrap());
            let unit = (bits >> 11) as f64 / ((1u64 << 53) as f64);
            offset + scale * (2.0 * unit - 1.0)
        });
    }
    let output: Vec<_> = labels.iter().map(|label| cache[label]).collect();
    if output.iter().any(|value| !value.is_finite()) {
        return Err(OperatorError::Invalid(
            "region_scalar result is not finite".into(),
        ));
    }
    Ok(output)
}

/// Aggregate incident signed value differences over unique cross-category geodesic edges.
pub fn boundary_signed_difference(
    mesh: &Mesh,
    labels: &[u32],
    values: &[f64],
    scale: f64,
) -> Result<Vec<f64>> {
    validate_component_mesh(mesh)?;
    let n = mesh.positions.len();
    if labels.len() != n || values.len() != n {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    if !scale.is_finite() || values.iter().any(|v| !v.is_finite()) {
        return Err(OperatorError::Invalid(
            "boundary inputs must be finite".into(),
        ));
    }
    let mut edges = Vec::new();
    for i in 0..n {
        for &neighbor in &mesh.neighbors[i] {
            let j = neighbor as usize;
            if i >= j || labels[i] == labels[j] {
                continue;
            }
            let pi = mesh.positions[i];
            let pj = mesh.positions[j];
            if !pi.is_finite()
                || !pj.is_finite()
                || pi.length_squared() == 0.0
                || pj.length_squared() == 0.0
            {
                return Err(OperatorError::Invalid(
                    "boundary edge has invalid geometry".into(),
                ));
            }
            let length = pi.normalize().dot(pj.normalize()).clamp(-1.0, 1.0).acos();
            let difference = (values[j] - values[i]) * scale;
            edges.push((i, j, difference, length));
        }
    }
    // Accumulate each cell's incident edges in canonical CellId-pair order,
    // independent of the mesh's neighbor-vector ordering.
    edges.sort_unstable_by_key(|&(i, j, _, _)| (i, j));
    let mut sums = vec![0.0; n];
    let mut weights = vec![0.0; n];
    for (i, j, difference, length) in edges {
        sums[i] += difference * length;
        sums[j] -= difference * length;
        weights[i] += length;
        weights[j] += length;
    }
    let output: Vec<_> = sums
        .iter()
        .zip(weights)
        .map(|(sum, weight)| if weight == 0.0 { 0.0 } else { sum / weight })
        .collect();
    if output.iter().any(|value| !value.is_finite()) {
        return Err(OperatorError::Invalid(
            "boundary result is not finite".into(),
        ));
    }
    Ok(output)
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

pub fn threshold(mesh: &Mesh, values: &[f64], t: f64) -> Result<Vec<bool>> {
    if values.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(values.par_iter().map(|value| *value >= t).collect())
}

pub fn vector_dot(mesh: &Mesh, a: &[DVec3], b: &[DVec3]) -> Result<Vec<f64>> {
    if a.len() != mesh.positions.len() || b.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(a.par_iter().zip(b).map(|(a, b)| a.dot(*b)).collect())
}

pub fn vector_magnitude(mesh: &Mesh, values: &[DVec3]) -> Result<Vec<f64>> {
    if values.len() != mesh.positions.len() {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    Ok(values.par_iter().map(|value| value.length()).collect())
}

pub fn voronoi_labels(mesh: &Mesh, seed: u64, node_id: &str, count: usize) -> Result<Vec<u32>> {
    validate_mesh(mesh)?;
    let n = mesh.positions.len();
    if count == 0 || count > n || n > u32::MAX as usize {
        return Err(OperatorError::Invalid(
            "voronoi count must be positive and no greater than mesh cell count".into(),
        ));
    }
    let mut ranked: Vec<_> = (0..n)
        .map(|cell| {
            let mut hash = blake3::Hasher::new();
            hash.update(&seed.to_le_bytes());
            hash.update(&(node_id.len() as u64).to_le_bytes());
            hash.update(node_id.as_bytes());
            hash.update(&(cell as u32).to_le_bytes());
            (*hash.finalize().as_bytes(), cell)
        })
        .collect();
    ranked.sort_unstable_by(|(a, ai), (b, bi)| a.cmp(b).then(ai.cmp(bi)));
    let seeds: Vec<_> = ranked
        .into_iter()
        .take(count)
        .map(|(_, id)| mesh.positions[id].normalize())
        .collect();
    Ok(mesh
        .positions
        .par_iter()
        .map(|position| {
            let p = position.normalize();
            seeds
                .iter()
                .enumerate()
                .fold((0usize, f64::NEG_INFINITY), |best, (label, seed)| {
                    let dot = p.dot(*seed);
                    if dot > best.1 { (label, dot) } else { best }
                })
                .0 as u32
        })
        .collect())
}

pub fn advect(mesh: &Mesh, values: &[f64], velocity: &[DVec3]) -> Result<Vec<f64>> {
    validate_mesh(mesh)?;
    let n = mesh.positions.len();
    if values.len() != n || velocity.len() != n {
        return Err(OperatorError::Invalid("field length mismatch".into()));
    }
    if values.iter().any(|v| !v.is_finite()) || velocity.iter().any(|v| !v.is_finite()) {
        return Err(OperatorError::Invalid(
            "advect inputs must be finite".into(),
        ));
    }
    Ok((0..n)
        .into_par_iter()
        .map(|i| {
            let p = mesh.positions[i].normalize();
            let v = velocity[i];
            let w = v.length().clamp(0.0, 1.0);
            if w == 0.0 || mesh.neighbors[i].is_empty() {
                return values[i];
            }
            let upstream = -v.normalize();
            let source = mesh.neighbors[i]
                .iter()
                .filter_map(|&id| {
                    let j = id as usize;
                    let delta = mesh.positions[j].normalize() - p;
                    let tangent = delta - p * delta.dot(p);
                    let len2 = tangent.length_squared();
                    (len2 > 1e-24).then_some((id, tangent.dot(upstream) / len2.sqrt()))
                })
                .min_by(|(ja, da), (jb, db)| db.total_cmp(da).then_with(|| ja.cmp(jb)))
                .map_or(i, |(id, _)| id as usize);
            (1.0 - w) * values[i] + w * values[source]
        })
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
    let calibration = if program
        .nodes
        .iter()
        .any(|n| n.op == "smooth_radius" || n.op == "correlated_noise")
    {
        Some(MeshCalibration::new(&state.mesh)?)
    } else {
        None
    };
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
            "smooth_radius" => match field("field")? {
                Field::Scalar(v) => Value::Field(Field::Scalar(calibration.as_ref().unwrap().smooth_radius(&v, scalar("radius")?)?)),
                _ => return Err(OperatorError::Invalid("field must be scalar".into())),
            },
            "correlated_noise" => Value::Field(Field::Scalar(correlated_noise_calibrated(
                calibration.as_ref().unwrap(), &state.mesh, state.seed, &node.id, scalar("radius")?,
                match node.args.get("amplitude") { Some(_) => scalar("amplitude")?, None => 1.0 },
                match node.args.get("mean") { Some(_) => scalar("mean")?, None => 0.0 },
            )?)),
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
            "region_scalar" => match field("labels")? {
                Field::Category(labels) => {
                    let scale = if node.args.contains_key("scale") {
                        scalar("scale")?
                    } else {
                        1.0
                    };
                    let offset = if node.args.contains_key("offset") {
                        scalar("offset")?
                    } else {
                        0.0
                    };
                    Value::Field(Field::Scalar(region_scalar(
                        &labels, state.seed, &node.id, scale, offset,
                    )?))
                }
                _ => return Err(OperatorError::Invalid("labels must be category field".into())),
            },
            "boundary_signed_difference" => match (field("labels")?, field("values")?) {
                (Field::Category(labels), Field::Scalar(values)) => {
                    let scale = if node.args.contains_key("scale") {
                        scalar("scale")?
                    } else {
                        1.0
                    };
                    Value::Field(Field::Scalar(boundary_signed_difference(
                        &state.mesh,
                        &labels,
                        &values,
                        scale,
                    )?))
                }
                _ => {
                    return Err(OperatorError::Invalid(
                        "boundary_signed_difference requires category labels and scalar values"
                            .into(),
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
            "threshold" => match field("field")? {
                Field::Scalar(v) => Value::Field(Field::Bool(threshold(&state.mesh, &v, scalar("threshold")?)?)),
                _ => return Err(OperatorError::Invalid("field must be scalar".into())),
            },
            "vector_dot" => match (field("a")?, field("b")?) {
                (Field::Vector(a), Field::Vector(b)) => Value::Field(Field::Scalar(vector_dot(&state.mesh, &a, &b)?)),
                _ => return Err(OperatorError::Invalid("vector_dot requires vector fields".into())),
            },
            "vector_magnitude" => match field("field")? {
                Field::Vector(v) => Value::Field(Field::Scalar(vector_magnitude(&state.mesh, &v)?)),
                _ => return Err(OperatorError::Invalid("field must be vector".into())),
            },
            "voronoi_labels" => {
                let count = scalar("count")?;
                if !count.is_finite() || count.fract() != 0.0 || count <= 0.0 || count > usize::MAX as f64 {
                    return Err(OperatorError::Invalid("voronoi count must be a positive integer".into()));
                }
                Value::Field(Field::Category(voronoi_labels(&state.mesh, state.seed, &node.id, count as usize)?))
            }
            "advect" => match (field("field")?, field("velocity")?) {
                (Field::Scalar(v), Field::Vector(velocity)) => Value::Field(Field::Scalar(advect(&state.mesh, &v, &velocity)?)),
                _ => return Err(OperatorError::Invalid("advect requires scalar field and vector velocity".into())),
            },
            "reduce" => return Err(OperatorError::UnsupportedConfiguration(
                "reduce requires a string operation (mean/min/max/sum), but frozen ValueRef represents only scalar literals, fields, parameters, and references; dispatch cannot select an operation".into(),
            )),
            "network_threshold" => {
                return Err(OperatorError::NotImplemented(node.op.clone()));
            }
            "region_vector" | "boundary_normal_component" | "boundary_tangential_component"
            | "divergence" | "vector_add" | "vector_subtract" | "scalar_vector_multiply" => {
                return Err(wave3::not_implemented(&node.op));
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
    fn connected_components_are_dense_ordered_and_validate_adjacency() {
        let mut mesh = line_mesh(6);
        assert_eq!(
            connected_components(&mesh, &[false; 6]).unwrap(),
            vec![0; 6]
        );
        assert_eq!(connected_components(&mesh, &[true; 6]).unwrap(), vec![1; 6]);
        assert_eq!(
            connected_components(&mesh, &[true, true, false, true, true, false]).unwrap(),
            vec![1, 1, 0, 2, 2, 0]
        );
        let mask = [true, false, true, false, true, false];
        let expected = connected_components(&mesh, &mask).unwrap();
        for neighbors in &mut mesh.neighbors {
            neighbors.reverse();
        }
        assert_eq!(connected_components(&mesh, &mask).unwrap(), expected);
        mesh.neighbors[0].push(1);
        assert!(connected_components(&mesh, &mask).is_err());
        mesh.neighbors[0].pop();
        mesh.neighbors[0].clear();
        assert!(connected_components(&mesh, &mask).is_err());
        mesh.neighbors[0] = vec![1, 1];
        assert!(connected_components(&mesh, &mask).is_err());
        mesh.neighbors[0] = vec![99];
        assert!(connected_components(&mesh, &mask).is_err());
    }

    #[test]
    fn region_scalar_is_category_keyed_and_composes_with_components() {
        let labels = connected_components(&line_mesh(5), &[true, true, false, true, true]).unwrap();
        let first = region_scalar(&labels, 4, "region", 2.0, 1.0).unwrap();
        assert_eq!(first[0], first[1]);
        assert_eq!(first[3], first[4]);
        assert_eq!(
            first[2], 1.0,
            "background label zero maps exactly to offset"
        );
        assert_ne!(first[0], first[3]);
        assert_eq!(
            first,
            region_scalar(&labels, 4, "region", 2.0, 1.0).unwrap()
        );
        assert_ne!(
            first,
            region_scalar(&labels, 5, "region", 2.0, 1.0).unwrap()
        );
        assert_ne!(first, region_scalar(&labels, 4, "other", 2.0, 1.0).unwrap());
        let unrelated = region_scalar(&[20, 20, 99, 99], 4, "region", 2.0, 1.0).unwrap();
        assert_eq!(unrelated[0], unrelated[1]);
        assert_eq!(unrelated[2], unrelated[3]);
        assert_ne!(
            region_scalar(&[2, 2], 4, "region", 1.0, 0.0).unwrap()[0],
            region_scalar(&[200, 200], 4, "region", 1.0, 0.0).unwrap()[0]
        );
        assert!(region_scalar(&labels, 4, "region", f64::NAN, 0.0).is_err());
        assert_eq!(
            region_scalar(&[0, 0], 99, "different-node", 100.0, -3.5).unwrap(),
            vec![-3.5; 2]
        );
    }

    #[test]
    fn signed_boundary_difference_is_weighted_normalized_and_order_independent() {
        let mesh = Mesh {
            positions: vec![DVec3::X, DVec3::Y, DVec3::Z],
            triangles: vec![],
            neighbors: vec![vec![2, 1], vec![0, 2], vec![0, 1]],
        };
        let labels = [1, 2, 2];
        let values = [0.0, 2.0, 2.0];
        let out = boundary_signed_difference(&mesh, &labels, &values, 1.0).unwrap();
        assert_eq!(out[0], 2.0);
        assert_eq!(out[1], -2.0);
        assert_eq!(out[2], -2.0);
        assert_eq!(
            boundary_signed_difference(&mesh, &labels, &[3.0; 3], 1.0).unwrap(),
            vec![0.0; 3]
        );
        assert_eq!(
            boundary_signed_difference(&mesh, &[1; 3], &values, 1.0).unwrap(),
            vec![0.0; 3]
        );
        assert_eq!(
            boundary_signed_difference(&mesh, &labels, &values, -1.0).unwrap(),
            vec![-2.0, 2.0, 2.0]
        );
        let mut reversed = mesh.clone();
        for neighbors in &mut reversed.neighbors {
            neighbors.reverse();
        }
        assert_eq!(
            out,
            boundary_signed_difference(&reversed, &labels, &values, 1.0).unwrap()
        );
        let order_sensitive = Mesh {
            positions: vec![DVec3::X, DVec3::Y, DVec3::Z, -DVec3::Y],
            triangles: vec![],
            neighbors: vec![vec![1, 2, 3], vec![0], vec![0], vec![0]],
        };
        let order_sensitive_labels = [1, 2, 3, 4];
        let order_sensitive_values = [0.0, 1.0e16, 1.0, -1.0e16];
        let canonical_result = boundary_signed_difference(
            &order_sensitive,
            &order_sensitive_labels,
            &order_sensitive_values,
            1.0,
        )
        .unwrap();
        let mut permuted = order_sensitive.clone();
        permuted.neighbors[0] = vec![1, 3, 2];
        assert_eq!(
            canonical_result,
            boundary_signed_difference(
                &permuted,
                &order_sensitive_labels,
                &order_sensitive_values,
                1.0,
            )
            .unwrap()
        );
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(2)
            .build()
            .unwrap();
        assert_eq!(
            out,
            pool.install(|| boundary_signed_difference(&mesh, &labels, &values, 1.0))
                .unwrap()
        );
        let weighted = Mesh {
            positions: vec![DVec3::X, DVec3::Y, (DVec3::X + DVec3::Y).normalize()],
            triangles: vec![],
            neighbors: vec![vec![1, 2], vec![0], vec![0]],
        };
        let aggregate =
            boundary_signed_difference(&weighted, &[1, 2, 2], &[0.0, 2.0, 4.0], 1.0).unwrap();
        assert!((aggregate[0] - 8.0 / 3.0).abs() < 1e-12);
        let mut malformed = mesh;
        malformed.neighbors[0].pop();
        assert!(boundary_signed_difference(&malformed, &labels, &values, 1.0).is_err());
    }

    #[test]
    fn spherical_components_match_unit_sphere_area_and_broadcast_measures() {
        let mesh = muse_geom::icosphere(2).unwrap();
        let labels = connected_components(&mesh, &vec![true; mesh.positions.len()]).unwrap();
        let summaries = component_summaries(&mesh, &labels).unwrap();
        assert_eq!(summaries[1].cell_count, mesh.positions.len());
        assert!((summaries[1].area - 4.0 * std::f64::consts::PI).abs() < 1e-10);
        assert_eq!(summaries[1].perimeter, 0.0);
        for measure in [
            ComponentMeasure::Area,
            ComponentMeasure::Perimeter,
            ComponentMeasure::CellCount,
        ] {
            let broadcast = component_measure_broadcast(&labels, &summaries, measure).unwrap();
            let expected = match measure {
                ComponentMeasure::Area => summaries[1].area,
                ComponentMeasure::Perimeter => summaries[1].perimeter,
                ComponentMeasure::CellCount => mesh.positions.len() as f64,
            };
            assert_eq!(broadcast, vec![expected; mesh.positions.len()]);
        }
        assert!(
            component_measure_broadcast(&labels, &summaries[..1], ComponentMeasure::Area).is_err()
        );

        let mask: Vec<_> = (0..mesh.positions.len()).map(|i| i % 3 != 0).collect();
        let labels = connected_components(&mesh, &mask).unwrap();
        let summaries = component_summaries(&mesh, &labels).unwrap();
        let area = summaries
            .iter()
            .skip(1)
            .map(|summary| summary.area)
            .sum::<f64>();
        let selected_area = mesh
            .triangles
            .iter()
            .map(|[ai, bi, ci]| {
                let (a, b, c) = (
                    mesh.positions[*ai as usize].normalize(),
                    mesh.positions[*bi as usize].normalize(),
                    mesh.positions[*ci as usize].normalize(),
                );
                let share = 2.0
                    * a.dot(b.cross(c))
                        .abs()
                        .atan2(1.0 + a.dot(b) + b.dot(c) + c.dot(a))
                    / 3.0;
                [*ai, *bi, *ci]
                    .iter()
                    .filter(|&&i| mask[i as usize])
                    .count() as f64
                    * share
            })
            .sum::<f64>();
        assert!((area - selected_area).abs() < 1e-12);

        let mesh = Mesh {
            positions: vec![DVec3::X, DVec3::Y, DVec3::Z],
            triangles: vec![],
            neighbors: vec![vec![1], vec![0, 2], vec![1]],
        };
        let labels = [1, 0, 0];
        let summaries = component_summaries(&mesh, &labels).unwrap();
        assert!((summaries[1].perimeter - std::f64::consts::FRAC_PI_2).abs() < 1e-12);
        assert_eq!(
            component_measure_broadcast(&labels, &summaries, ComponentMeasure::CellCount).unwrap(),
            vec![1.0, 0.0, 0.0]
        );
    }
    #[test]
    fn constant_and_noise_are_stable_and_node_keyed() {
        let m = line_mesh(16);
        assert_eq!(constant(&m, 2.5), vec![2.5; 16]);
        assert_eq!(noise(&m, 7, "a", 1.0), noise(&m, 7, "a", 1.0));
        assert_ne!(noise(&m, 7, "a", 1.0), noise(&m, 7, "b", 1.0));
    }

    #[test]
    fn spherical_radius_kernel_preserves_identity_constants_mean_and_determinism() {
        let mesh = muse_geom::icosphere(2).unwrap();
        let calibration = MeshCalibration::new(&mesh).unwrap();
        let field: Vec<_> = (0..mesh.positions.len())
            .map(|i| (i as f64 * 0.71).sin())
            .collect();
        assert_eq!(calibration.smooth_radius(&field, 0.0).unwrap(), field);
        let constant = calibration
            .smooth_radius(&vec![3.25; field.len()], 0.3)
            .unwrap();
        assert!(constant.iter().all(|v| (*v - 3.25).abs() < 1e-12));
        let smoothed = calibration.smooth_radius(&field, 0.8).unwrap();
        let (before, _) = calibration.area_weighted_mean_std(&field).unwrap();
        let (after, _) = calibration.area_weighted_mean_std(&smoothed).unwrap();
        assert!((before - after).abs() < 1e-12);
        assert_eq!(smoothed, calibration.smooth_radius(&field, 0.8).unwrap());
        let one = rayon::ThreadPoolBuilder::new()
            .num_threads(1)
            .build()
            .unwrap();
        let four = rayon::ThreadPoolBuilder::new()
            .num_threads(4)
            .build()
            .unwrap();
        assert_eq!(
            one.install(|| calibration.smooth_radius(&field, 0.8).unwrap()),
            four.install(|| calibration.smooth_radius(&field, 0.8).unwrap())
        );
        assert!(
            calibration.area_weighted_mean_std(&smoothed).unwrap().1
                < calibration.area_weighted_mean_std(&field).unwrap().1
        );
    }

    #[test]
    fn correlated_noise_normalizes_and_is_keyed() {
        let mesh = muse_geom::icosphere(2).unwrap();
        let calibration = MeshCalibration::new(&mesh).unwrap();
        let a = correlated_noise(&mesh, 12, "a", 0.12, 2.0, 3.0).unwrap();
        let repeat = correlated_noise(&mesh, 12, "a", 0.12, 2.0, 3.0).unwrap();
        let b = correlated_noise(&mesh, 12, "b", 0.12, 2.0, 3.0).unwrap();
        assert_eq!(a, repeat);
        assert_ne!(a, b);
        let (mean, stddev) = calibration.area_weighted_mean_std(&a).unwrap();
        assert!((mean - 3.0).abs() < 1e-12);
        assert!((stddev - 2.0).abs() < 1e-10);
        assert_eq!(
            correlated_noise(&mesh, 1, "x", 1.0, 0.0, -4.0).unwrap(),
            vec![-4.0; mesh.positions.len()]
        );
        assert_eq!(
            correlated_noise(&mesh, 1, "x", 1.0e10, 2.0, -4.0).unwrap(),
            vec![-4.0; mesh.positions.len()]
        );
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
    fn generic_algebra_kernels_and_dispatch_are_validated() {
        let mesh = sphere_mesh();
        assert_eq!(
            threshold(&mesh, &[0.0, 1.0, 2.0, 3.0], 2.0).unwrap(),
            vec![false, false, true, true]
        );
        assert_eq!(
            vector_dot(
                &mesh,
                &[DVec3::X, DVec3::Y, DVec3::Z, DVec3::ONE],
                &[DVec3::X, DVec3::ONE, DVec3::Z, DVec3::ONE]
            )
            .unwrap(),
            vec![1.0, 1.0, 1.0, 3.0]
        );
        assert_eq!(
            vector_magnitude(&mesh, &[DVec3::X, DVec3::Y, DVec3::Z, DVec3::splat(2.0)]).unwrap(),
            vec![1.0, 1.0, 1.0, 2.0 * 3.0_f64.sqrt()]
        );
        assert!(vector_dot(&mesh, &[], &[DVec3::ZERO; 4]).is_err());
        assert!(voronoi_labels(&mesh, 1, "v", 0).is_err());
        assert!(voronoi_labels(&mesh, 1, "v", 5).is_err());
        let labels = voronoi_labels(&mesh, 1, "v", 3).unwrap();
        assert_eq!(labels, voronoi_labels(&mesh, 1, "v", 3).unwrap());
        assert!(labels.iter().all(|label| *label < 3));
        assert_eq!(
            labels
                .into_iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            3
        );
        let values = [0.0, 1.0, 2.0, 3.0];
        let zero = [DVec3::ZERO; 4];
        assert_eq!(advect(&mesh, &values, &zero).unwrap(), values);
        assert_eq!(
            advect(&mesh, &[4.0; 4], &[DVec3::X; 4]).unwrap(),
            vec![4.0; 4]
        );
        let synthetic_velocity = [DVec3::Y, DVec3::ZERO, DVec3::ZERO, DVec3::ZERO];
        assert_eq!(
            advect(&mesh, &[0.0, 10.0, 20.0, 30.0], &synthetic_velocity).unwrap()[0],
            20.0
        );
        assert!(advect(&mesh, &values, &[DVec3::ZERO; 3]).is_err());
        assert!(advect(&mesh, &[f64::NAN; 4], &zero).is_err());

        let state = WorldState {
            mesh: mesh.clone(),
            fields: BTreeMap::from([
                ("scalar".into(), Field::Scalar(values.to_vec())),
                ("va".into(), Field::Vector(vec![DVec3::X; 4])),
                ("vb".into(), Field::Vector(vec![DVec3::Y; 4])),
            ]),
            networks: BTreeMap::new(),
            parameters: BTreeMap::new(),
            step: 0,
            seed: 9,
        };
        let node = |id: &str, op: &str, args| CompiledNode {
            id: id.into(),
            op: op.into(),
            args,
            cel: vec![],
        };
        let program = Program {
            nodes: vec![
                node(
                    "vor",
                    "voronoi_labels",
                    BTreeMap::from([("count".into(), ValueRef::LiteralScalar(2.0))]),
                ),
                node(
                    "thr",
                    "threshold",
                    BTreeMap::from([
                        ("field".into(), ValueRef::State("scalar".into())),
                        ("threshold".into(), ValueRef::LiteralScalar(2.0)),
                    ]),
                ),
                node(
                    "dot",
                    "vector_dot",
                    BTreeMap::from([
                        ("a".into(), ValueRef::State("va".into())),
                        ("b".into(), ValueRef::State("vb".into())),
                    ]),
                ),
                node(
                    "mag",
                    "vector_magnitude",
                    BTreeMap::from([("field".into(), ValueRef::State("va".into()))]),
                ),
                node(
                    "adv",
                    "advect",
                    BTreeMap::from([
                        ("field".into(), ValueRef::State("scalar".into())),
                        ("velocity".into(), ValueRef::State("va".into())),
                    ]),
                ),
            ],
            updates: vec![],
        };
        let output = execute(&program, &state).unwrap();
        assert!(matches!(
            output.get("vor.value"),
            Some(Value::Field(Field::Category(_)))
        ));
        assert_eq!(
            output.get("thr.value"),
            Some(&Value::Field(Field::Bool(vec![false, false, true, true])))
        );
        assert_eq!(output.scalar_field("dot.value").unwrap(), &[0.0; 4]);
        assert_eq!(output.scalar_field("mag.value").unwrap(), &[1.0; 4]);
        assert!(
            output
                .scalar_field("adv.value")
                .unwrap()
                .iter()
                .all(|v| v.is_finite())
        );
        assert_eq!(mesh, state.mesh);
    }

    #[test]
    fn advect_equal_alignment_chooses_lowest_cell_id() {
        let mesh = Mesh {
            positions: vec![
                DVec3::X,
                DVec3::new(0.0, 1.0, 1.0).normalize(),
                DVec3::new(0.0, 1.0, -1.0).normalize(),
            ],
            triangles: vec![],
            neighbors: vec![vec![2, 1], vec![], vec![]],
        };
        let values = [0.0, 11.0, 22.0];
        let velocity = [-DVec3::Y, DVec3::ZERO, DVec3::ZERO];

        // Cells 1 and 2 have exactly equal upstream alignment. Deliberately list
        // cell 2 first to ensure selection is based on CellId, not neighbor order.
        assert_eq!(advect(&mesh, &values, &velocity).unwrap()[0], 11.0);
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
            assert!(v.is_finite());
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
        let vor_single = single.install(|| voronoi_labels(&mesh, 99, "parallel", 3).unwrap());
        let vor_multi = multi.install(|| voronoi_labels(&mesh, 99, "parallel", 3).unwrap());
        let adv_single = single.install(|| advect(&mesh, &values, &[DVec3::Y; 4]).unwrap());
        let adv_multi = multi.install(|| advect(&mesh, &values, &[DVec3::Y; 4]).unwrap());
        assert_eq!(vor_single, vor_multi);
        assert_eq!(adv_single, adv_multi);
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
