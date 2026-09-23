use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use muse_ops::Value;
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
    },
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Wave1Generate {
            spec,
            output,
            level,
            seed,
        } => generate(spec, output, level, seed),
    }
}

fn generate(spec: PathBuf, output: PathBuf, level: u8, seed: u64) -> Result<()> {
    let source =
        fs::read_to_string(&spec).with_context(|| format!("reading {}", spec.display()))?;
    let program = muse_spec::compile_program(&source, "generate")
        .with_context(|| format!("compiling {}", spec.display()))?;
    let mesh = muse_geom::icosphere(level).context("constructing icosphere")?;
    let mut state = WorldState {
        mesh,
        fields: BTreeMap::new(),
        networks: BTreeMap::new(),
        parameters: BTreeMap::new(),
        step: 0,
        seed,
    };
    let outputs = muse_ops::execute(&program, &state).context("executing compiled Program")?;
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
            generate(spec_path.clone(), path.clone(), 2, seed).unwrap();
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
}
