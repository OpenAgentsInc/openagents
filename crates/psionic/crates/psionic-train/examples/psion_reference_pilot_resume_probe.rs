use std::{env, error::Error, fs, path::PathBuf};

use psionic_train::probe_psion_reference_pilot_resume;

fn main() -> Result<(), Box<dyn Error>> {
    let repo_root = workspace_root()?;
    let checkpoint_dir = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("target/psion_reference_pilot_bundle"));
    let output_dir = env::args()
        .nth(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| repo_root.join("target/psion_reference_pilot_resume_probe"));
    fs::create_dir_all(&output_dir)?;

    let probe = probe_psion_reference_pilot_resume(repo_root.as_path(), checkpoint_dir.as_path())?;
    let output_file = output_dir.join("psion_reference_pilot_resume_probe.json");
    fs::write(&output_file, serde_json::to_vec_pretty(&probe)?)?;

    println!(
        "psion reference pilot resume probe completed: run={} checkpoint={} output={}",
        probe.run_id,
        probe.checkpoint_ref,
        output_file.display()
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
