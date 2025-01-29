use anyhow::Result;
use std::path::Path;
use std::process::Command;
use tracing::debug;

pub fn run_cargo_tests(repo_path: &Path) -> Result<bool> {
    debug!("Running cargo tests in {:?}", repo_path);

    let output = Command::new("cargo")
        .arg("test")
        .current_dir(repo_path)
        .output()?;

    Ok(output.status.success())
}
