use std::{env, error::Error, fs, path::PathBuf};

use psionic_train::{
    run_psion_accelerated_reference_pilot_with_live_visualization,
    PsionGoogleSingleNodeLiveVisualizationWriter, PsionReferencePilotConfig,
    RemoteTrainingArtifactSourceKind, TrainingLoopBudget,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let output_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target/psion_accelerated_reference_pilot"));
    fs::create_dir_all(&output_dir)?;

    let mut config = PsionReferencePilotConfig::accelerated_single_node()?;
    apply_env_overrides(&mut config)?;
    let mut live_visualization_writer = PsionGoogleSingleNodeLiveVisualizationWriter::try_start(
        output_dir.as_path(),
        config.run_id.as_str(),
        "The Google single-node accelerated Psion reference lane started and is emitting live visualization bundles.",
    )?;
    let run = match run_psion_accelerated_reference_pilot_with_live_visualization(
        root.as_path(),
        &config,
        live_visualization_writer.as_mut(),
    ) {
        Ok(run) => run,
        Err(error) => {
            if let Some(writer) = live_visualization_writer.as_mut() {
                let _ = writer.finish_failure(format!(
                    "The accelerated Psion reference lane failed before sealing its final receipts: {error}"
                ));
            }
            return Err(Box::new(error));
        }
    };
    run.write_to_dir(output_dir.as_path())?;
    run.write_to_dir_with_prefix(output_dir.as_path(), "psion_accelerated_reference_pilot")?;
    if let Some(writer) = live_visualization_writer.as_mut() {
        writer.record_source_artifact(
            "stage_receipt",
            "psion_accelerated_reference_pilot_stage_receipt.json",
            Some(run.stage_receipt.receipt_digest.clone()),
            RemoteTrainingArtifactSourceKind::RuntimeOwned,
            true,
            vec![String::from("receipt.psion.pretrain_stage.v1")],
            "The accelerated stage receipt remains authoritative for delivered execution and accelerator posture.",
        )?;
        writer.record_source_artifact(
            "observability_receipt",
            "psion_accelerated_reference_pilot_observability_receipt.json",
            Some(run.observability_receipt.observability_digest.clone()),
            RemoteTrainingArtifactSourceKind::RuntimeOwned,
            true,
            vec![String::from("psion.pretrain_run_observability_receipt.v1")],
            "The accelerated observability receipt remains authoritative for throughput and cost summary facts.",
        )?;
        writer.record_source_artifact(
            "checkpoint_manifest",
            "psion_accelerated_reference_pilot_checkpoint_manifest.json",
            Some(run.checkpoint_artifact.manifest.stable_digest()),
            RemoteTrainingArtifactSourceKind::RuntimeOwned,
            true,
            vec![String::from(
                "psion_reference_pilot_checkpoint_manifest.v1",
            )],
            "The checkpoint manifest remains authoritative for the promoted checkpoint identity surfaced in the live viewer.",
        )?;
        writer.finish_success(format!(
            "The accelerated Psion reference lane completed {} optimizer steps and sealed checkpoint `{}` for the live viewer.",
            run.step_receipts.len(),
            run.checkpoint_artifact.manifest.checkpoint_ref
        ))?;
    }

    println!(
        "psion accelerated reference pilot completed: stage={} checkpoint={} output={}",
        run.stage_receipt.stage_id,
        run.checkpoint_artifact.manifest.checkpoint_ref,
        output_dir.display()
    );
    println!(
        "delivered backend={}",
        run.stage_receipt
            .delivered_execution
            .as_ref()
            .map(|execution| execution.runtime_backend.as_str())
            .unwrap_or("unknown")
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
