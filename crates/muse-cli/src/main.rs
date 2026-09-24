use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use muse_ops::{RuntimeValue, Value};
use muse_types::WorldState;

#[derive(Parser)]
#[command(name = "muse")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Compile and execute a YAML Program, writing its resulting WorldState.
    Wave1Generate {
        #[arg(long)]
        spec: PathBuf,
        #[arg(long)]
        output: PathBuf,
        #[arg(long, default_value_t = 2)]
        level: u8,
        #[arg(long, default_value_t = 42)]
        seed: u64,
        /// Override a compiled top-level numeric default (repeatable; last value wins).
        #[arg(long = "param", value_name = "NAME=VALUE")]
        params: Vec<String>,
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Wave1Generate {
            spec,
            output,
            level,
            seed,
            params,
        } => generate(spec, output, level, seed, &params),
    }
}

fn generate(
    spec: PathBuf,
    output: PathBuf,
    level: u8,
    seed: u64,
    overrides: &[String],
) -> Result<()> {
    let source =
        fs::read_to_string(&spec).with_context(|| format!("reading {}", spec.display()))?;
    let compilation = muse_spec::compile_program_with_defaults(&source, "generate")
        .with_context(|| format!("compiling {}", spec.display()))?;
    let program = compilation.program;
    let mut parameters = compilation.parameters;
    for override_value in overrides {
        let (name, value) = override_value
            .split_once('=')
            .with_context(|| format!("invalid --param {override_value:?}; expected NAME=VALUE"))?;
        if name.is_empty() {
            bail!("invalid --param {override_value:?}; parameter name is empty");
        }
        let value = value.parse::<f64>().with_context(|| {
            format!("invalid --param value {value:?} for {name}; expected finite number")
        })?;
        if !value.is_finite() {
            bail!("invalid --param value for {name}; value must be finite");
        }
        let parameter = parameters
            .get_mut(name)
            .with_context(|| format!("--param {name} is unknown or has no compiled default"))?;
        *parameter = value;
    }
    let mesh = muse_geom::icosphere(level).context("constructing icosphere")?;
    let mut state = WorldState {
        mesh,
        fields: BTreeMap::new(),
        networks: BTreeMap::new(),
        parameters,
        step: 0,
        seed,
    };
    let mut inputs = BTreeMap::new();
    for name in program
        .nodes
        .iter()
        .flat_map(|node| node.args.values())
        .filter_map(|arg| {
            if let muse_types::ValueRef::Input(name) = arg {
                Some(name.as_str())
            } else {
                None
            }
        })
    {
        if inputs.contains_key(name) {
            continue;
        }
        let component = match name {
            "position_x" => Some(0),
            "position_y" => Some(1),
            "position_z" => Some(2),
            _ => None,
        };
        if let Some(component) = component {
            let values = state
                .mesh
                .positions
                .iter()
                .map(|position| position[component])
                .collect();
            inputs.insert(
                name.to_owned(),
                RuntimeValue::Field(muse_types::Field::Scalar(values)),
            );
        }
    }
    let outputs = muse_ops::execute_with_inputs(&program, &state, &inputs)
        .context("executing compiled Program")?;
    for update in &program.updates {
        let muse_types::ValueRef::NodeOutput { node, output } = &update.value else {
            bail!(
                "state update {} must refer to a Program node output",
                update.field
            );
        };
        let value = outputs.get(&format!("{node}.{output}")).with_context(|| {
            format!(
                "Program update {} references missing output {node}.{output}",
                update.field
            )
        })?;
        let Value::Field(field) = value else {
            bail!("Program update {} is not a field", update.field);
        };
        state.fields.insert(update.field.clone(), field.clone());
    }
    if state.fields.is_empty() {
        bail!("compiled Program produced no state updates");
    }
    let json = serde_json::to_vec_pretty(&state).context("serializing WorldState")?;
    fs::write(&output, json).with_context(|| format!("writing {}", output.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use muse_types::Field;

    const SPEC: &str = "../../fixtures/specs/wave1-foundation.yaml";

    #[test]
    fn generated_world_is_valid_deterministic_and_seeded() {
        let spec_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(SPEC);
        let source = fs::read_to_string(&spec_path).unwrap();
        let program_a = muse_spec::compile_program(&source, "generate").unwrap();
        let program_b = muse_spec::compile_program(&source, "generate").unwrap();
        assert_eq!(program_a, program_b);
        let operators = program_a
            .nodes
            .iter()
            .map(|n| n.op.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            operators,
            ["constant", "diffuse", "noise", "pointwise"].into()
        );

        let run = |seed| {
            let path =
                std::env::temp_dir().join(format!("muse-wave1-{seed}-{}.json", std::process::id()));
            generate(spec_path.clone(), path.clone(), 2, seed, &[]).unwrap();
            let bytes = fs::read(&path).unwrap();
            let state: WorldState = serde_json::from_slice(&bytes).unwrap();
            let _ = fs::remove_file(path);
            assert!(state.mesh.triangles.len() > 20);
            let count = state.mesh.positions.len();
            assert!(
                state
                    .mesh
                    .triangles
                    .iter()
                    .flatten()
                    .all(|id| (*id as usize) < count)
            );
            let Field::Scalar(values) = &state.fields["debug_scalar"] else {
                panic!("debug_scalar must be scalar")
            };
            assert_eq!(values.len(), count);
            assert!(values.iter().all(|value| value.is_finite()));
            let (min, max) = values
                .iter()
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
                    (lo.min(*v), hi.max(*v))
                });
            assert!(max > min);
            (bytes, values.clone())
        };
        let (first, values) = run(42);
        let (second, _) = run(42);
        let (_, changed_seed_values) = run(43);
        assert_eq!(first, second);
        assert_ne!(values, changed_seed_values);
    }

    const GEOMETRY_SPEC: &str = r#"version: 1
inputs:
  position_z: scalar_field
parameters:
  multiplier: {type: scalar, default: 2.0}
  untouched: {type: scalar, default: 7.0}
  no_default: {type: scalar}
programs:
  generate:
    nodes:
      transform:
        op: pointwise
        inputs: {x: $input.position_z}
        params: {scale: $param.multiplier}
        expr: x * scale
    updates: {height: $node.transform.value}
"#;

    fn run_spec(source: &str, level: u8, params: &[&str]) -> Result<Vec<u8>> {
        let spec = std::env::temp_dir().join(format!("muse-cli-spec-{}.yaml", std::process::id()));
        let output = std::env::temp_dir().join(format!("muse-cli-out-{}.json", std::process::id()));
        fs::write(&spec, source)?;
        let overrides = params
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>();
        let result = generate(spec.clone(), output.clone(), level, 42, &overrides)
            .and_then(|()| fs::read(&output).context("reading generated output"));
        let _ = fs::remove_file(spec);
        let _ = fs::remove_file(output);
        result
    }

    #[test]
    fn defaults_overrides_geometry_and_determinism() {
        let bytes = run_spec(GEOMETRY_SPEC, 2, &[]).unwrap();
        let state: WorldState = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(state.parameters["multiplier"], 2.0);
        assert_eq!(state.parameters["untouched"], 7.0);
        assert!(!state.parameters.contains_key("no_default"));
        let changed = run_spec(GEOMETRY_SPEC, 2, &["multiplier=3.5"]).unwrap();
        let changed_state: WorldState = serde_json::from_slice(&changed).unwrap();
        assert_eq!(changed_state.parameters["multiplier"], 3.5);
        assert_eq!(changed_state.parameters["untouched"], 7.0);
        let Field::Scalar(values) = &state.fields["height"] else {
            panic!("height must be scalar")
        };
        assert!(values.iter().any(|value| *value != values[0]));
        assert_eq!(bytes, run_spec(GEOMETRY_SPEC, 2, &[]).unwrap());
        assert_eq!(
            changed,
            run_spec(GEOMETRY_SPEC, 2, &["multiplier=3.5"]).unwrap()
        );
        let repeated = run_spec(GEOMETRY_SPEC, 2, &["multiplier=1.0", "multiplier=4.0"]).unwrap();
        let repeated_state: WorldState = serde_json::from_slice(&repeated).unwrap();
        assert_eq!(repeated_state.parameters["multiplier"], 4.0);
    }

    #[test]
    fn rejects_invalid_parameter_overrides() {
        for invalid in [
            "unknown=1",
            "no_default=2",
            "multiplier=nope",
            "multiplier=NaN",
            "multiplier=inf",
        ] {
            assert!(
                run_spec(GEOMETRY_SPEC, 2, &[invalid]).is_err(),
                "accepted {invalid}"
            );
        }
    }

    #[test]
    fn clap_accepts_repeatable_param_options_in_order() {
        let cli = Cli::try_parse_from([
            "muse",
            "wave1-generate",
            "--spec",
            "spec.yaml",
            "--output",
            "out.json",
            "--param",
            "multiplier=1",
            "--param",
            "multiplier=2",
        ])
        .unwrap();
        let Command::Wave1Generate { params, .. } = cli.command;
        assert_eq!(params, ["multiplier=1", "multiplier=2"]);
    }

    #[test]
    fn only_referenced_geometry_inputs_are_supplied_and_level_five_is_full_size() {
        let unknown = GEOMETRY_SPEC.replace("position_z", "other_input");
        assert!(
            format!("{:#}", run_spec(&unknown, 2, &[]).unwrap_err())
                .contains("missing external input `other_input`")
        );
        let bytes = run_spec(GEOMETRY_SPEC, 5, &[]).unwrap();
        let state: WorldState = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(state.mesh.positions.len(), 10_242);
        let Field::Scalar(values) = &state.fields["height"] else {
            panic!("height must be scalar")
        };
        assert_eq!(values.len(), state.mesh.positions.len());
    }
}
