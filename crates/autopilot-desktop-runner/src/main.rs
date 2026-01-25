use std::path::PathBuf;
use std::process::{Command, Stdio};

fn main() -> anyhow::Result<()> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = manifest_dir
        .parent()
        .and_then(|path| path.parent())
        .ok_or_else(|| anyhow::anyhow!("Failed to resolve OpenAgents repo root"))?;
    let app_dir = repo_root.join("apps").join("autopilot-desktop");

    if !app_dir.is_dir() {
        return Err(anyhow::anyhow!(
            "Autopilot desktop app not found at {}",
            app_dir.display()
        ));
    }

    let status = Command::new("bun")
        .arg("run")
        .arg("tauri")
        .arg("dev")
        .current_dir(&app_dir)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    match status.code() {
        Some(code) => std::process::exit(code),
        None => std::process::exit(1),
    }
}
