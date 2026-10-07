use std::{env, error::Error, fs, path::PathBuf};

use psionic_train::{run_psion_reference_pilot, PsionReferencePilotConfig, TrainingLoopBudget};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/psion_reference_pilot"));
    fs::create_dir_all(&output_dir)?;

    let mut config = PsionReferencePilotConfig::reference()?;
    apply_env_overrides(&mut config)?;
    let run = run_psion_reference_pilot(root.as_path(), &config)?;
    run.write_to_dir(&output_dir)?;

    println!(
        "psion reference pilot completed: stage={} checkpoint={} output={}",
        run.stage_receipt.stage_id,
        run.checkpoint_artifact.manifest.checkpoint_ref,
        output_dir.display()
    );
    println!(
        "held-out loss milli: initial={} final={}",
        run.initial_held_out_loss_milli, run.final_held_out_loss_milli
    );

    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .ancestors()
        .nth(2)
        .map(PathBuf::from)
        .ok_or_else(|| "failed to resolve workspace root".into())
}

fn apply_env_overrides(config: &mut PsionReferencePilotConfig) -> Result<(), Box<dyn Error>> {
    let max_steps = optional_env_u64("PSION_REFERENCE_PILOT_MAX_STEPS")?;
    let steps_per_window = optional_env_u64("PSION_REFERENCE_PILOT_STEPS_PER_WINDOW")?;
    let windows_per_cadence = optional_env_u64("PSION_REFERENCE_PILOT_WINDOWS_PER_CADENCE")?;
    if max_steps.is_some() || steps_per_window.is_some() || windows_per_cadence.is_some() {
        config.budget = TrainingLoopBudget::new(
            max_steps.unwrap_or(config.budget.max_steps),
            steps_per_window.unwrap_or(config.budget.steps_per_window),
            windows_per_cadence.unwrap_or(config.budget.windows_per_cadence),
        )?;
    }

    if let Some(step_duration_ms) = optional_env_u64("PSION_REFERENCE_PILOT_STEP_DURATION_MS")? {
        if step_duration_ms == 0 {
            return Err("PSION_REFERENCE_PILOT_STEP_DURATION_MS must be greater than zero".into());
        }
        config.step_duration_ms = step_duration_ms;
    }

    Ok(())
}

fn optional_env_u64(name: &str) -> Result<Option<u64>, Box<dyn Error>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value.parse::<u64>()?)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error) => Err(Box::new(error)),
    }
}
