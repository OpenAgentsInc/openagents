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
fn parity_blocker_workflow_list_includes_expected_profiles() {
    let script = repo_root().join("scripts/cad/parity-blocker-workflow.sh");
    let output = Command::new("bash")
        .arg(script.as_os_str())
        .arg("--list")
        .output()
        .expect("parity-blocker-workflow --list should run");
    assert!(
        output.status.success(),
        "parity-blocker-workflow --list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("stdout should be utf8");
    let lines: Vec<&str> = stdout.lines().collect();
    assert!(lines.contains(&"phase_a_baseline_v1"));
    assert!(lines.contains(&"parity_complete_v1"));
}
