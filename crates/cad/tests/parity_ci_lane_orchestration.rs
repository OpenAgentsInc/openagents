#![allow(
    clippy::all,
    clippy::expect_used,
    clippy::panic,
    clippy::pedantic,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used
)]

use std::path::{Path, PathBuf};
use std::process::Command;

fn repo_root() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .and_then(Path::parent)
        .expect("crate should be in <repo>/crates/cad")
        .to_path_buf()
}

#[test]
fn parity_ci_lane_script_list_mode_includes_required_steps() {
    let script = repo_root().join("scripts/cad/parity-ci-lane.sh");
    let output = Command::new("bash")
        .arg(script.as_os_str())
        .arg("--list")
        .output()
        .expect("parity-ci-lane --list should run");
    assert!(
        output.status.success(),
        "parity-ci-lane --list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.contains(&"parity-check"));
    assert!(lines.contains(&"ci-artifact-manifest-check"));
    assert!(lines.contains(&"artifact-copy"));
    assert!(lines.contains(&"artifact-bundle"));
    assert!(lines.contains(&"artifact-checksum"));
}
