//! Deterministic metrics and CEL-based validation for world states.

use std::collections::{BTreeMap, HashMap, VecDeque};

use cel::{Context, Program};
use muse_types::{Field, WorldState};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub mod wave3;
/// Backward-compatible name for the initial paired Wave 3 contracts.
pub mod paired {
    pub use crate::wave3::*;
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("invalid validation YAML: {0}")]
    Yaml(String),
    #[error("{path}: {message}")]
    Spec { path: String, message: String },
    #[error("{path}: invalid CEL: {message}")]
    CelCompile { path: String, message: String },
    #[error("{path}: CEL evaluation failed: {message}")]
    CelEvaluate { path: String, message: String },
    #[error("{path}: expected {expected}, got {actual}")]
    ResultType {
        path: String,
        expected: &'static str,
        actual: String,
    },
    #[error("metric {name}: {message}")]
    Metric { name: String, message: String },
    #[error("score calculation: {0}")]
    Score(String),
}

#[derive(Clone, Debug)]
enum MetricDef {
    Mean(String),
    Variance(String),
    Quantile(String, f64),
    FractionAbove(String, f64),
    FractionBelow(String, f64),
    Correlation(String, String),
    ComponentCount(String),
    LargestComponentFraction(String),
    Area(String),
    AreaFraction(String),
    Perimeter(String),
    LargestComponentAreaFraction(String),
    LargestComponentPerimeter(String),
    ComponentAreaMean(String),
    ComponentAreaVariance(String),
    PerimeterAreaRatio(String),
}

#[derive(Debug)]
struct Expression {
    name: String,
    program: Program,
}

#[derive(Debug)]
struct Objective {
    expression: Expression,
    weight: f64,
}

#[derive(Debug)]
pub struct ValidationProgram {
    metrics: BTreeMap<String, MetricDef>,
    constraints: Vec<Expression>,
    objectives: Vec<Objective>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ValidationResult {
    pub valid: bool,
    pub score: f64,
    pub metrics: BTreeMap<String, f64>,
    pub objectives: BTreeMap<String, f64>,
    pub constraints: BTreeMap<String, bool>,
}

#[derive(Deserialize)]
struct Spec {
    #[serde(default)]
    metrics: BTreeMap<String, serde_json::Value>,
    #[serde(default)]
    constraints: BTreeMap<String, ExprSpec>,
    #[serde(default)]
    objectives: BTreeMap<String, ObjectiveSpec>,
}
#[derive(Deserialize)]
struct ExprSpec {
    expr: String,
}
#[derive(Deserialize)]
struct ObjectiveSpec {
    expr: String,
    #[serde(default = "default_weight")]
    weight: f64,
}
fn default_weight() -> f64 {
    1.0
}

/// Compile a standalone validation YAML document.
pub fn compile(source: &str) -> Result<ValidationProgram, ValidationError> {
    let spec: Spec =
        serde_saphyr::from_str(source).map_err(|e| ValidationError::Yaml(e.to_string()))?;
    let mut metrics = BTreeMap::new();
    for (name, raw) in spec.metrics {
        metrics.insert(name.clone(), parse_metric(&name, raw)?);
    }
    let compile_expr =
        |kind: &str, name: String, src: String| -> Result<Expression, ValidationError> {
            let path = format!("{kind}.{name}.expr");
            let program = Program::compile(&src).map_err(|e| ValidationError::CelCompile {
                path: path.clone(),
                message: e.to_string(),
            })?;
            for variable in program.references().variables() {
                let var = variable.to_string();
                if var != "metrics" {
                    return Err(ValidationError::Spec {
                        path,
                        message: format!("unknown variable '{var}'"),
                    });
                }
            }
            for reference in metric_references(&src) {
                if !metrics.contains_key(&reference) {
                    return Err(ValidationError::Spec {
                        path: format!("{kind}.{name}.expr"),
                        message: format!("unknown metric '{reference}'"),
                    });
                }
            }
            Ok(Expression { name, program })
        };
    let constraints = spec
        .constraints
        .into_iter()
        .map(|(name, expr)| compile_expr("constraints", name, expr.expr))
        .collect::<Result<Vec<_>, _>>()?;
    let mut objectives = Vec::new();
    for (name, item) in spec.objectives {
        if !item.weight.is_finite() || item.weight < 0.0 {
            return Err(ValidationError::Spec {
                path: format!("objectives.{name}.weight"),
                message: "weight must be finite and nonnegative".into(),
            });
        }
        objectives.push(Objective {
            expression: compile_expr("objectives", name, item.expr)?,
            weight: item.weight,
        });
    }
    Ok(ValidationProgram {
        metrics,
        constraints,
        objectives,
    })
}

fn metric_references(source: &str) -> Vec<String> {
    source
        .match_indices("metrics.")
        .filter_map(|(start, _)| {
            let tail = &source[start + 8..];
            let end = tail
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(tail.len());
            (end > 0).then(|| tail[..end].to_owned())
        })
        .collect()
}

fn parse_metric(name: &str, raw: serde_json::Value) -> Result<MetricDef, ValidationError> {
    let path = format!("metrics.{name}");
    let obj = raw.as_object().ok_or_else(|| ValidationError::Spec {
        path: path.clone(),
        message: "expected metric mapping".into(),
    })?;
    let op = obj
        .get("op")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ValidationError::Spec {
            path: path.clone(),
            message: "missing string op".into(),
        })?;
    let field = || {
        obj.get("field")
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or_else(|| ValidationError::Spec {
                path: path.clone(),
                message: "missing string field".into(),
            })
    };
    let number = |key: &str| {
        obj.get(key)
            .and_then(serde_json::Value::as_f64)
            .filter(|v| v.is_finite())
            .ok_or_else(|| ValidationError::Spec {
                path: format!("{path}.{key}"),
                message: "expected finite number".into(),
            })
    };
    match op {
        "mean" => Ok(MetricDef::Mean(field()?)),
        "variance" => Ok(MetricDef::Variance(field()?)),
        "quantile" => {
            let f = field()?;
            let q = number("q")?;
            if !(0.0..=1.0).contains(&q) {
                return Err(ValidationError::Spec {
                    path: format!("{path}.q"),
                    message: "q must be in [0,1]".into(),
                });
            }
            Ok(MetricDef::Quantile(f, q))
        }
        "fraction_above" => Ok(MetricDef::FractionAbove(field()?, number("threshold")?)),
        "fraction_below" => Ok(MetricDef::FractionBelow(field()?, number("threshold")?)),
        "correlation" => {
            let a = obj
                .get("a")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ValidationError::Spec {
                    path: path.clone(),
                    message: "missing string a".into(),
                })?;
            let b = obj
                .get("b")
                .and_then(|v| v.as_str())
                .ok_or_else(|| ValidationError::Spec {
                    path: path.clone(),
                    message: "missing string b".into(),
                })?;
            Ok(MetricDef::Correlation(a.into(), b.into()))
        }
        "component_count" => Ok(MetricDef::ComponentCount(field()?)),
        "largest_component_fraction" => Ok(MetricDef::LargestComponentFraction(field()?)),
        "area" => Ok(MetricDef::Area(field()?)),
        "area_fraction" => Ok(MetricDef::AreaFraction(field()?)),
        "perimeter" => Ok(MetricDef::Perimeter(field()?)),
        "largest_component_area_fraction" => Ok(MetricDef::LargestComponentAreaFraction(field()?)),
        "largest_component_perimeter" => Ok(MetricDef::LargestComponentPerimeter(field()?)),
        "component_area_mean" => Ok(MetricDef::ComponentAreaMean(field()?)),
        "component_area_variance" => Ok(MetricDef::ComponentAreaVariance(field()?)),
        "perimeter_area_ratio" => Ok(MetricDef::PerimeterAreaRatio(field()?)),
        _ => Err(ValidationError::Spec {
            path,
            message: format!("unsupported metric op '{op}'"),
        }),
    }
}

/// Evaluate all native metrics and CEL expressions against a world state.
pub fn evaluate(
    program: &ValidationProgram,
    world: &WorldState,
) -> Result<ValidationResult, ValidationError> {
    let mut metrics = BTreeMap::new();
    for (name, def) in &program.metrics {
        let value = match def {
            MetricDef::Mean(f) => mean(scalar(world, f, name)?)?,
            MetricDef::Variance(f) => variance(scalar(world, f, name)?)?,
            MetricDef::Quantile(f, q) => quantile(scalar(world, f, name)?, *q),
            MetricDef::FractionAbove(f, t) => {
                scalar(world, f, name)?.iter().filter(|x| **x > *t).count() as f64
                    / scalar(world, f, name)?.len() as f64
            }
            MetricDef::FractionBelow(f, t) => {
                scalar(world, f, name)?.iter().filter(|x| **x < *t).count() as f64
                    / scalar(world, f, name)?.len() as f64
            }
            MetricDef::Correlation(a, b) => {
                correlation(scalar(world, a, name)?, scalar(world, b, name)?, name)?
            }
            MetricDef::ComponentCount(f) => {
                component_sizes(world, bools(world, f, name)?).len() as f64
            }
            MetricDef::LargestComponentFraction(f) => {
                let values = bools(world, f, name)?;
                let total = values.iter().filter(|v| **v).count();
                if total == 0 {
                    0.0
                } else {
                    component_sizes(world, values)
                        .into_iter()
                        .max()
                        .unwrap_or(0) as f64
                        / total as f64
                }
            }
            MetricDef::Area(f)
            | MetricDef::AreaFraction(f)
            | MetricDef::Perimeter(f)
            | MetricDef::LargestComponentAreaFraction(f)
            | MetricDef::LargestComponentPerimeter(f)
            | MetricDef::ComponentAreaMean(f)
            | MetricDef::ComponentAreaVariance(f)
            | MetricDef::PerimeterAreaRatio(f) => {
                let mask = bools(world, f, name)?;
                let stats = spherical_stats(world, mask, name)?;
                match def {
                    MetricDef::Area(_) => stats.area,
                    MetricDef::AreaFraction(_) => stats.area_fraction,
                    MetricDef::Perimeter(_) => stats.perimeter,
                    MetricDef::LargestComponentAreaFraction(_) => stats.largest_area_fraction,
                    MetricDef::LargestComponentPerimeter(_) => stats.largest_perimeter,
                    MetricDef::ComponentAreaMean(_) => stats.component_area_mean,
                    MetricDef::ComponentAreaVariance(_) => stats.component_area_variance,
                    MetricDef::PerimeterAreaRatio(_) => stats.perimeter_area_ratio,
                    _ => unreachable!(),
                }
            }
        };
        if !value.is_finite() {
            return Err(metric_error(name, "result is nonfinite"));
        }
        metrics.insert(name.clone(), value);
    }
    let mut context = Context::default();
    let vals: HashMap<String, f64> = metrics.iter().map(|(k, v)| (k.clone(), *v)).collect();
    context.add_variable_from_value("metrics", vals);
    let mut constraints = BTreeMap::new();
    for expr in &program.constraints {
        let value = expr
            .program
            .execute(&context)
            .map_err(|e| ValidationError::CelEvaluate {
                path: format!("constraints.{}", expr.name),
                message: e.to_string(),
            })?;
        match value {
            cel::Value::Bool(v) => {
                constraints.insert(expr.name.clone(), v);
            }
            other => {
                return Err(ValidationError::ResultType {
                    path: format!("constraints.{}", expr.name),
                    expected: "bool",
                    actual: format!("{other:?}"),
                });
            }
        }
    }
    let valid = constraints.values().all(|v| *v);
    let mut objectives = BTreeMap::new();
    let mut weighted = 0.0;
    let mut total_weight = 0.0;
    for objective in &program.objectives {
        let expr = &objective.expression;
        let value = expr
            .program
            .execute(&context)
            .map_err(|e| ValidationError::CelEvaluate {
                path: format!("objectives.{}", expr.name),
                message: e.to_string(),
            })?;
        let number = match value {
            cel::Value::Float(v) => v,
            cel::Value::Int(v) => v as f64,
            cel::Value::UInt(v) => v as f64,
            other => {
                return Err(ValidationError::ResultType {
                    path: format!("objectives.{}", expr.name),
                    expected: "finite number",
                    actual: format!("{other:?}"),
                });
            }
        };
        if !number.is_finite() {
            return Err(metric_error(&expr.name, "objective result is nonfinite"));
        }
        objectives.insert(expr.name.clone(), number);
        if !valid {
            continue;
        }
        let term = number * objective.weight;
        if !term.is_finite() {
            return Err(ValidationError::Score(format!(
                "objective '{}' weighted term is nonfinite",
                expr.name
            )));
        }
        let next_weighted = weighted + term;
        if !next_weighted.is_finite() {
            return Err(ValidationError::Score(
                "weighted objective sum is nonfinite".into(),
            ));
        }
        weighted = next_weighted;

        let next_total_weight = total_weight + objective.weight;
        if !next_total_weight.is_finite() {
            return Err(ValidationError::Score(
                "total objective weight is nonfinite".into(),
            ));
        }
        total_weight = next_total_weight;
    }
    let score = if !valid || total_weight == 0.0 {
        0.0
    } else {
        let score = weighted / total_weight;
        if !score.is_finite() {
            return Err(ValidationError::Score(
                "weighted objective mean is nonfinite".into(),
            ));
        }
        score
    };
    Ok(ValidationResult {
        valid,
        score,
        metrics,
        objectives,
        constraints,
    })
}

fn metric_error(name: &str, message: &str) -> ValidationError {
    ValidationError::Metric {
        name: name.into(),
        message: message.into(),
    }
}
fn scalar<'a>(
    world: &'a WorldState,
    field: &str,
    metric: &str,
) -> Result<&'a [f64], ValidationError> {
    let values = match world.fields.get(field) {
        Some(Field::Scalar(v)) => v.as_slice(),
        Some(_) => {
            return Err(metric_error(
                metric,
                &format!("field '{field}' is not scalar"),
            ));
        }
        None => return Err(metric_error(metric, &format!("missing field '{field}'"))),
    };
    if values.is_empty() {
        return Err(metric_error(metric, &format!("field '{field}' is empty")));
    }
    if values.len() != world.mesh.positions.len() {
        return Err(metric_error(
            metric,
            &format!(
                "field '{field}' length {} does not match mesh length {}",
                values.len(),
                world.mesh.positions.len()
            ),
        ));
    }
    if values.iter().any(|v| !v.is_finite()) {
        return Err(metric_error(
            metric,
            &format!("field '{field}' contains nonfinite input"),
        ));
    }
    Ok(values)
}
fn bools<'a>(
    world: &'a WorldState,
    field: &str,
    metric: &str,
) -> Result<&'a [bool], ValidationError> {
    match world.fields.get(field) {
        Some(Field::Bool(v)) if v.len() == world.mesh.positions.len() => Ok(v),
        Some(Field::Bool(v)) => Err(metric_error(
            metric,
            &format!(
                "field '{field}' length {} does not match mesh length {}",
                v.len(),
                world.mesh.positions.len()
            ),
        )),
        Some(_) => Err(metric_error(
            metric,
            &format!("field '{field}' is not bool"),
        )),
        None => Err(metric_error(metric, &format!("missing field '{field}'"))),
    }
}
fn mean(v: &[f64]) -> Result<f64, ValidationError> {
    let result = v.iter().sum::<f64>() / v.len() as f64;
    if result.is_finite() {
        Ok(result)
    } else {
        Err(metric_error("mean", "result is nonfinite"))
    }
}
fn variance(v: &[f64]) -> Result<f64, ValidationError> {
    let m = mean(v)?;
    let r = v.iter().map(|x| (x - m).powi(2)).sum::<f64>() / v.len() as f64;
    if r.is_finite() {
        Ok(r)
    } else {
        Err(metric_error("variance", "result is nonfinite"))
    }
}
fn quantile(v: &[f64], q: f64) -> f64 {
    let mut s = v.to_vec();
    s.sort_by(f64::total_cmp);
    let pos = q * (s.len() - 1) as f64;
    let lo = pos.floor() as usize;
    let hi = pos.ceil() as usize;
    s[lo] + (s[hi] - s[lo]) * (pos - lo as f64)
}
fn correlation(a: &[f64], b: &[f64], name: &str) -> Result<f64, ValidationError> {
    if a.len() != b.len() {
        return Err(metric_error(name, "correlation input lengths differ"));
    }
    let ma = mean(a)?;
    let mb = mean(b)?;
    let va = a.iter().map(|x| (x - ma).powi(2)).sum::<f64>() / a.len() as f64;
    let vb = b.iter().map(|x| (x - mb).powi(2)).sum::<f64>() / b.len() as f64;
    if va == 0.0 || vb == 0.0 {
        return Err(metric_error(name, "correlation input has zero variance"));
    }
    let cov = a
        .iter()
        .zip(b)
        .map(|(x, y)| (x - ma) * (y - mb))
        .sum::<f64>()
        / a.len() as f64;
    Ok(cov / (va * vb).sqrt())
}
fn component_sizes(world: &WorldState, mask: &[bool]) -> Vec<usize> {
    let mut seen = vec![false; mask.len()];
    let mut sizes = Vec::new();
    for start in 0..mask.len() {
        if !mask[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        let mut queue = VecDeque::from([start]);
        let mut size = 0;
        while let Some(i) = queue.pop_front() {
            size += 1;
            if let Some(adj) = world.mesh.neighbors.get(i) {
                for &n in adj {
                    let n = n as usize;
                    if n < mask.len() && mask[n] && !seen[n] {
                        seen[n] = true;
                        queue.push_back(n);
                    }
                }
            }
        }
        sizes.push(size);
    }
    sizes
}

#[derive(Default)]
struct SphericalStats {
    area: f64,
    area_fraction: f64,
    perimeter: f64,
    largest_area_fraction: f64,
    largest_perimeter: f64,
    component_area_mean: f64,
    component_area_variance: f64,
    perimeter_area_ratio: f64,
}

fn spherical_stats(
    world: &WorldState,
    mask: &[bool],
    metric: &str,
) -> Result<SphericalStats, ValidationError> {
    let n = mask.len();
    if world.mesh.neighbors.len() != n {
        return Err(metric_error(
            metric,
            "mesh neighbor count does not match positions",
        ));
    }
    let mut vertex_area = vec![0.0; n];
    for triangle in &world.mesh.triangles {
        let [ai, bi, ci] = *triangle;
        let (a, b, c) = (ai as usize, bi as usize, ci as usize);
        if a >= n || b >= n || c >= n {
            return Err(metric_error(metric, "mesh triangle index is out of range"));
        }
        let (a, b, c) = (
            world.mesh.positions[a].normalize(),
            world.mesh.positions[b].normalize(),
            world.mesh.positions[c].normalize(),
        );
        let numerator = a.dot(b.cross(c)).abs();
        let denominator = 1.0 + a.dot(b) + b.dot(c) + c.dot(a);
        let share = 2.0 * numerator.atan2(denominator) / 3.0;
        for index in [ai, bi, ci] {
            vertex_area[index as usize] += share;
        }
    }
    let mut component_id = vec![usize::MAX; n];
    let mut components: Vec<Vec<usize>> = Vec::new();
    for start in 0..n {
        if !mask[start] || component_id[start] != usize::MAX {
            continue;
        }
        let id = components.len();
        component_id[start] = id;
        let mut members = Vec::new();
        let mut queue = VecDeque::from([start]);
        while let Some(i) = queue.pop_front() {
            members.push(i);
            for &neighbor in &world.mesh.neighbors[i] {
                let j = neighbor as usize;
                if j < n && mask[j] && component_id[j] == usize::MAX {
                    component_id[j] = id;
                    queue.push_back(j);
                }
            }
        }
        components.push(members);
    }
    let component_areas: Vec<f64> = components
        .iter()
        .map(|vertices| vertices.iter().map(|&i| vertex_area[i]).sum())
        .collect();
    let area: f64 = if mask.iter().any(|value| *value) {
        mask.iter()
            .enumerate()
            .filter(|(_, value)| **value)
            .map(|(i, _)| vertex_area[i])
            .sum()
    } else {
        0.0
    };
    let mut component_perimeters = vec![0.0; components.len()];
    for (i, adjacent) in world.mesh.neighbors.iter().enumerate() {
        if !mask[i] {
            continue;
        }
        for &neighbor in adjacent {
            let j = neighbor as usize;
            if j >= n || i >= j || mask[j] {
                continue;
            }
            let p = world.mesh.positions[i].normalize();
            let q = world.mesh.positions[j].normalize();
            component_perimeters[component_id[i]] += p.dot(q).clamp(-1.0, 1.0).acos();
        }
    }
    let perimeter: f64 = if component_perimeters.is_empty() {
        0.0
    } else {
        component_perimeters.iter().sum()
    };
    let largest_area = component_areas.iter().copied().fold(0.0, f64::max);
    let largest_index = component_areas
        .iter()
        .enumerate()
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(i, _)| i);
    let component_area_mean = if component_areas.is_empty() {
        0.0
    } else {
        component_areas.iter().sum::<f64>() / component_areas.len() as f64
    };
    let component_area_variance = if component_areas.is_empty() {
        0.0
    } else {
        component_areas
            .iter()
            .map(|v| (v - component_area_mean).powi(2))
            .sum::<f64>()
            / component_areas.len() as f64
    };
    Ok(SphericalStats {
        area,
        area_fraction: area / (4.0 * std::f64::consts::PI),
        perimeter,
        largest_area_fraction: if area == 0.0 {
            0.0
        } else {
            largest_area / area
        },
        largest_perimeter: largest_index
            .map(|i| component_perimeters[i])
            .unwrap_or(0.0),
        component_area_mean,
        component_area_variance,
        perimeter_area_ratio: if area == 0.0 {
            0.0
        } else {
            perimeter / area.sqrt()
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn world() -> WorldState {
        WorldState {
            mesh: serde_json::from_value(serde_json::json!({"positions":[[0.0,0.0,0.0],[1.0,0.0,0.0],[2.0,0.0,0.0],[3.0,0.0,0.0]],"triangles":[],"neighbors":[[1],[0,2],[1],[]]})).unwrap(),
            fields: BTreeMap::from([
                ("left".into(), Field::Scalar(vec![1.0, 2.0, 3.0, 4.0])),
                ("right".into(), Field::Scalar(vec![2.0, 4.0, 6.0, 8.0])),
                ("negative".into(), Field::Scalar(vec![8.0, 6.0, 4.0, 2.0])),
                ("mask".into(), Field::Bool(vec![true, true, false, true])),
            ]),
            networks: BTreeMap::new(),
            parameters: BTreeMap::new(),
            step: 0,
            seed: 0,
        }
    }
    fn run(yaml: &str) -> ValidationResult {
        evaluate(&compile(yaml).unwrap(), &world()).unwrap()
    }

    #[test]
    fn scalar_metrics_and_correlations_are_exact() {
        let result = run(
            "metrics:\n  mean: {op: mean, field: left}\n  variance: {op: variance, field: left}\n  q0: {op: quantile, field: left, q: 0}\n  qh: {op: quantile, field: left, q: 0.5}\n  q1: {op: quantile, field: left, q: 1}\n  above: {op: fraction_above, field: left, threshold: 2}\n  below: {op: fraction_below, field: left, threshold: 3}\n  pos: {op: correlation, a: left, b: right}\n  neg: {op: correlation, a: left, b: negative}\n",
        );
        assert_eq!(result.metrics["mean"], 2.5);
        assert_eq!(result.metrics["variance"], 1.25);
        assert_eq!(result.metrics["q0"], 1.0);
        assert_eq!(result.metrics["qh"], 2.5);
        assert_eq!(result.metrics["q1"], 4.0);
        assert_eq!(result.metrics["above"], 0.5);
        assert_eq!(result.metrics["below"], 0.5);
        assert_eq!(result.metrics["pos"], 1.0);
        assert_eq!(result.metrics["neg"], -1.0);
    }
    #[test]
    fn components_cel_score_and_snapshot_are_deterministic() {
        let source = "metrics:\n  mean: {op: mean, field: left}\n  components: {op: component_count, field: mask}\n  largest: {op: largest_component_fraction, field: mask}\nconstraints:\n  enough: {expr: 'metrics.mean > 2'}\nobjectives:\n  quality: {expr: 'metrics.mean / 5.0', weight: 2}\n  connected: {expr: 'metrics.largest', weight: 1}\n";
        let program = compile(source).unwrap();
        let a = evaluate(&program, &world()).unwrap();
        let b = evaluate(&compile(source).unwrap(), &world()).unwrap();
        assert_eq!(a, b);
        assert_eq!(a.metrics["components"], 2.0);
        assert_eq!(a.metrics["largest"], 2.0 / 3.0);
        assert_eq!(a.score, (0.5 * 2.0 + 2.0 / 3.0) / 3.0);
        insta::assert_snapshot!(serde_json::to_string_pretty(&a).unwrap());
        let fail = run(
            "metrics: {}\nconstraints:\n  no: {expr: 'false'}\nobjectives:\n  one: {expr: '1'}\n",
        );
        assert_eq!(fail.score, 0.0);
        assert!(!fail.valid);
    }

    #[test]
    fn largest_component_fraction_is_zero_without_true_cells() {
        let mut empty_mask = world();
        empty_mask
            .fields
            .insert("mask".into(), Field::Bool(vec![false; 4]));
        let result = evaluate(
            &compile("metrics:\n  largest: {op: largest_component_fraction, field: mask}\n")
                .unwrap(),
            &empty_mask,
        )
        .unwrap();
        assert_eq!(result.metrics["largest"], 0.0);
    }

    fn spherical_metrics(mesh: muse_types::Mesh, mask: Vec<bool>) -> BTreeMap<String, f64> {
        let world = WorldState {
            mesh,
            fields: BTreeMap::from([("mask".into(), Field::Bool(mask))]),
            ..world()
        };
        let source = "metrics:\n  area: {op: area, field: mask}\n  area_fraction: {op: area_fraction, field: mask}\n  perimeter: {op: perimeter, field: mask}\n  largest_area: {op: largest_component_area_fraction, field: mask}\n  largest_perimeter: {op: largest_component_perimeter, field: mask}\n  mean: {op: component_area_mean, field: mask}\n  variance: {op: component_area_variance, field: mask}\n  ratio: {op: perimeter_area_ratio, field: mask}\n";
        evaluate(&compile(source).unwrap(), &world).unwrap().metrics
    }

    #[test]
    fn canonical_mesh_area_covers_sphere_and_masks_have_stable_metrics() {
        for level in [0, 2] {
            let mesh = muse_geom::icosphere(level).unwrap();
            let full = spherical_metrics(mesh.clone(), vec![true; mesh.positions.len()]);
            assert!((full["area"] - 4.0 * std::f64::consts::PI).abs() < 1e-10);
            assert!((full["area_fraction"] - 1.0).abs() < 1e-12);
            assert_eq!(full["perimeter"], 0.0);
            let empty = spherical_metrics(mesh.clone(), vec![false; mesh.positions.len()]);
            assert!(empty.values().all(|value| *value == 0.0));
            assert_eq!(
                spherical_metrics(mesh.clone(), vec![false; mesh.positions.len()]),
                empty
            );
            insta::assert_snapshot!(format!("level_{level}: {full:?}\nempty: {empty:?}"));
        }
    }

    #[test]
    fn coherent_mask_outperforms_fragmented_mask_and_edges_are_counted_once() {
        let mesh = muse_geom::icosphere(2).unwrap();
        let mut order = Vec::new();
        let mut seen = vec![false; mesh.positions.len()];
        let mut queue = VecDeque::from([0]);
        seen[0] = true;
        while let Some(i) = queue.pop_front() {
            order.push(i);
            for &neighbor in &mesh.neighbors[i] {
                let j = neighbor as usize;
                if !seen[j] {
                    seen[j] = true;
                    queue.push_back(j);
                }
            }
        }
        let mut coherent = vec![false; seen.len()];
        let mut fragmented = vec![false; seen.len()];
        for &i in order.iter().take(40) {
            coherent[i] = true;
        }
        for &i in order.iter().step_by(4).take(40) {
            fragmented[i] = true;
        }
        let a = spherical_metrics(mesh.clone(), coherent);
        let b = spherical_metrics(mesh.clone(), fragmented);
        assert!(a["ratio"] < b["ratio"]);
        assert!(a["largest_area"] > b["largest_area"]);

        let mut one_sided = mesh.clone();
        for adjacent in &mut one_sided.neighbors {
            adjacent.clear();
        }
        for i in 0..one_sided.positions.len() {
            for &j in &mesh.neighbors[i] {
                if (i as u32) < j {
                    one_sided.neighbors[i].push(j);
                }
            }
        }
        let mask: Vec<bool> = vec![true; mesh.positions.len() / 2]
            .into_iter()
            .chain(std::iter::repeat(false))
            .take(mesh.positions.len())
            .collect();
        assert!(
            (spherical_metrics(mesh.clone(), mask.clone())["perimeter"]
                - spherical_metrics(one_sided, mask)["perimeter"])
                .abs()
                < 1e-12
        );
    }

    #[test]
    fn spherical_metrics_report_missing_and_wrong_type_fields() {
        let mesh = muse_geom::icosphere(0).unwrap();
        for field_value in [None, Some(Field::Scalar(vec![1.0; mesh.positions.len()]))] {
            let mut fields = BTreeMap::new();
            if let Some(value) = field_value {
                fields.insert("mask".into(), value);
            }
            let state = WorldState {
                mesh: mesh.clone(),
                fields,
                ..world()
            };
            assert!(matches!(
                evaluate(
                    &compile("metrics:\n  a: {op: area, field: mask}\n").unwrap(),
                    &state
                ),
                Err(ValidationError::Metric { .. })
            ));
        }
    }

    #[test]
    fn zero_objective_weights_and_invalid_weights_follow_contract() {
        let no_objectives = run("metrics: {}\n");
        assert!(no_objectives.valid);
        assert_eq!(no_objectives.score, 0.0);

        let zero_weights = run(
            "metrics: {}\nobjectives:\n  first: {expr: '1.0', weight: 0}\n  second: {expr: '2.0', weight: 0}\n",
        );
        assert!(zero_weights.valid);
        assert_eq!(zero_weights.score, 0.0);

        assert!(compile("objectives:\n  negative: {expr: '1.0', weight: -1}\n").is_err());
        // serde-saphyr/serde_json cannot represent YAML's .inf as a JSON number;
        // rejection during compilation is the required behavior either way.
        assert!(compile("objectives:\n  infinite: {expr: '1.0', weight: .inf}\n").is_err());
    }

    #[test]
    fn correlation_field_names_obey_yaml_string_resolution() {
        assert!(compile("metrics:\n  corr: {op: correlation, a: left, b: y}\n").is_err());
        assert!(compile("metrics:\n  corr: {op: correlation, a: left, b: \"y\"}\n").is_ok());
        assert!(compile("metrics:\n  corr: {op: correlation, a: left, b: right}\n").is_ok());
    }

    #[test]
    fn score_overflow_is_a_structured_error() {
        let evaluate_spec = |source: &str| evaluate(&compile(source).unwrap(), &world());

        // Each objective result and weight is finite, but their product overflows.
        assert!(matches!(
            evaluate_spec("objectives:\n  product: {expr: '1e308', weight: 2.0}\n"),
            Err(ValidationError::Score(_))
        ));

        // Products remain finite individually, but their cumulative numerator overflows.
        assert!(matches!(
            evaluate_spec(
                "objectives:\n  first: {expr: '1e308', weight: 1.0}\n  second: {expr: '1e308', weight: 1.0}\n"
            ),
            Err(ValidationError::Score(_))
        ));

        // Both terms remain finite, but the accumulated declared weights overflow.
        assert!(matches!(
            evaluate_spec(
                "objectives:\n  first: {expr: '1e-308', weight: 1e308}\n  second: {expr: '1e-308', weight: 1e308}\n"
            ),
            Err(ValidationError::Score(_))
        ));
    }

    #[test]
    fn errors_are_structured_for_invalid_spec_inputs_and_expression_types() {
        assert!(compile("metrics: [").is_err());
        assert!(compile("constraints:\n  bad: {expr: 'metrics.absent > 0'}\n").is_err());
        assert!(compile("constraints:\n  bad: {expr: 'metrics.x >'}\n").is_err());
        assert!(matches!(
            evaluate(
                &compile("constraints:\n  bad: {expr: '1'}\n").unwrap(),
                &world()
            ),
            Err(ValidationError::ResultType { .. })
        ));
        assert!(matches!(
            evaluate(
                &compile("objectives:\n  bad: {expr: 'true'}\n").unwrap(),
                &world()
            ),
            Err(ValidationError::ResultType { .. })
        ));
        assert!(matches!(
            evaluate(
                &compile("metrics:\n  c: {op: correlation, a: left, b: mask}\n").unwrap(),
                &world()
            ),
            Err(ValidationError::Metric { .. })
        ));
        let mut bad = world();
        bad.fields.insert("left".into(), Field::Scalar(vec![1.0]));
        assert!(matches!(
            evaluate(
                &compile("metrics:\n  m: {op: mean, field: left}\n").unwrap(),
                &bad
            ),
            Err(ValidationError::Metric { .. })
        ));
        let mut empty = world();
        empty.fields.insert("left".into(), Field::Scalar(vec![]));
        assert!(matches!(
            evaluate(
                &compile("metrics:\n  m: {op: mean, field: left}\n").unwrap(),
                &empty
            ),
            Err(ValidationError::Metric { .. })
        ));
        let mut nonfinite = world();
        nonfinite
            .fields
            .insert("left".into(), Field::Scalar(vec![f64::NAN; 4]));
        assert!(matches!(
            evaluate(
                &compile("metrics:\n  m: {op: mean, field: left}\n").unwrap(),
                &nonfinite
            ),
            Err(ValidationError::Metric { .. })
        ));
        assert!(matches!(
            evaluate(
                &compile("metrics:\n  c: {op: correlation, a: left, b: left}\n").unwrap(),
                &WorldState {
                    fields: BTreeMap::from([("left".into(), Field::Scalar(vec![1.0; 4]))]),
                    ..world()
                }
            ),
            Err(ValidationError::Metric { .. })
        ));
        assert!(matches!(
            evaluate(
                &compile("metrics:\n  m: {op: mean, field: absent}\n").unwrap(),
                &world()
            ),
            Err(ValidationError::Metric { .. })
        ));
        assert!(matches!(
            evaluate(
                &compile("metrics:\n  m: {op: mean, field: mask}\n").unwrap(),
                &world()
            ),
            Err(ValidationError::Metric { .. })
        ));
    }
}
