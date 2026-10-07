use std::{error::Error, fs, path::PathBuf};

use psionic_train::{
    build_parameter_golf_homegolf_visualization_bundle_v2,
    build_parameter_golf_xtrain_quick_eval_report,
    build_parameter_golf_xtrain_visualization_bundle_v2, sample_google_live_visualization_bundle,
    sample_google_live_visualization_bundle_v2, sample_google_summary_only_visualization_bundle,
    sample_google_summary_only_visualization_bundle_v2,
    sample_parameter_golf_distributed_live_visualization_bundle,
    sample_parameter_golf_distributed_live_visualization_bundle_v2,
    sample_parameter_golf_live_visualization_bundle,
    sample_parameter_golf_live_visualization_bundle_v2, sample_remote_training_run_index,
    sample_remote_training_run_index_v2,
};

fn main() -> Result<(), Box<dyn Error>> {
    let root = workspace_root()?;
    let fixtures_dir = root.join("fixtures/training_visualization");
    let parameter_golf_reports_dir = root.join("fixtures/parameter_golf/reports");
    fs::create_dir_all(&fixtures_dir)?;
    fs::create_dir_all(&parameter_golf_reports_dir)?;

    let google_summary_only = sample_google_summary_only_visualization_bundle()?;
    let google_live = sample_google_live_visualization_bundle()?;
    let parameter_golf_live = sample_parameter_golf_live_visualization_bundle()?;
    let parameter_golf_distributed = sample_parameter_golf_distributed_live_visualization_bundle()?;
    let run_index = sample_remote_training_run_index()?;
    let google_summary_only_v2 = sample_google_summary_only_visualization_bundle_v2()?;
    let google_live_v2 = sample_google_live_visualization_bundle_v2()?;
    let parameter_golf_live_v2 = sample_parameter_golf_live_visualization_bundle_v2()?;
    let parameter_golf_distributed_v2 =
        sample_parameter_golf_distributed_live_visualization_bundle_v2()?;
    let parameter_golf_homegolf_v2 = build_parameter_golf_homegolf_visualization_bundle_v2()?;
    let parameter_golf_xtrain_report = build_parameter_golf_xtrain_quick_eval_report()?;
    let parameter_golf_xtrain_v2 = build_parameter_golf_xtrain_visualization_bundle_v2()?;
    let run_index_v2 = sample_remote_training_run_index_v2()?;

    fs::write(
        fixtures_dir.join("psion_google_summary_only_remote_training_visualization_bundle_v1.json"),
        format!("{}\n", serde_json::to_string_pretty(&google_summary_only)?),
    )?;
    fs::write(
        fixtures_dir.join("psion_google_live_remote_training_visualization_bundle_v1.json"),
        format!("{}\n", serde_json::to_string_pretty(&google_live)?),
    )?;
    fs::write(
        fixtures_dir.join("parameter_golf_live_remote_training_visualization_bundle_v1.json"),
        format!("{}\n", serde_json::to_string_pretty(&parameter_golf_live)?),
    )?;
    fs::write(
        fixtures_dir
            .join("parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v1.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&parameter_golf_distributed)?
        ),
    )?;
    fs::write(
        fixtures_dir.join("remote_training_run_index_v1.json"),
        format!("{}\n", serde_json::to_string_pretty(&run_index)?),
    )?;
    fs::write(
        fixtures_dir.join("psion_google_summary_only_remote_training_visualization_bundle_v2.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&google_summary_only_v2)?
        ),
    )?;
    fs::write(
        fixtures_dir.join("psion_google_live_remote_training_visualization_bundle_v2.json"),
        format!("{}\n", serde_json::to_string_pretty(&google_live_v2)?),
    )?;
    fs::write(
        fixtures_dir.join("parameter_golf_live_remote_training_visualization_bundle_v2.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&parameter_golf_live_v2)?
        ),
    )?;
    fs::write(
        fixtures_dir
            .join("parameter_golf_distributed_8xh100_remote_training_visualization_bundle_v2.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&parameter_golf_distributed_v2)?
        ),
    )?;
    fs::write(
        fixtures_dir.join("parameter_golf_homegolf_remote_training_visualization_bundle_v2.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&parameter_golf_homegolf_v2)?
        ),
    )?;
    fs::write(
        parameter_golf_reports_dir.join("parameter_golf_xtrain_quick_eval_report.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&parameter_golf_xtrain_report)?
        ),
    )?;
    fs::write(
        fixtures_dir.join("parameter_golf_xtrain_remote_training_visualization_bundle_v2.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&parameter_golf_xtrain_v2)?
        ),
    )?;
    fs::write(
        fixtures_dir.join("remote_training_run_index_v2.json"),
        format!("{}\n", serde_json::to_string_pretty(&run_index_v2)?),
    )?;
    Ok(())
}

fn workspace_root() -> Result<PathBuf, Box<dyn Error>> {
    let current = std::env::current_dir()?;
    for candidate in [
        current.as_path(),
        current.parent().unwrap_or(current.as_path()),
    ] {
        if candidate.join("Cargo.toml").is_file() && candidate.join("fixtures").is_dir() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err("failed to find workspace root".into())
}
