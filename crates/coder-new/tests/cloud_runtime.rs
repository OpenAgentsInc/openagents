use serde_json::Value;
use std::process::Command;
#[test]
fn headless_binary_reports_revision_and_uses_the_same_settings_commands() {
    let binary = env!("CARGO_BIN_EXE_coder-cloud-runtime");
    let manifest = Command::new(binary)
        .arg("--runtime-manifest")
        .output()
        .unwrap();
    assert!(manifest.status.success());
    let manifest: Value = serde_json::from_slice(&manifest.stdout).unwrap();
    assert_eq!(manifest["schema"], "openagents.coder.cloud-runtime.v1");
    assert_eq!(manifest["revision"].as_str().unwrap().len(), 40);
    let root = tempfile::tempdir().unwrap();
    let args = [
        "--json",
        "coder",
        "--state",
        root.path().to_str().unwrap(),
        "plugins",
        "enable",
        "gce-cloud",
    ];
    let saved = Command::new(binary)
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .current_dir(root.path())
        .output()
        .unwrap();
    assert!(
        saved.status.success(),
        "{}",
        String::from_utf8_lossy(&saved.stderr)
    );
    let stored: Value =
        serde_json::from_slice(&std::fs::read(root.path().join("bundled-plugins.json")).unwrap())
            .unwrap();
    assert!(stored.to_string().contains("gce"));
    assert!(
        Command::new(binary)
            .args(["coder", "--help"])
            .output()
            .unwrap()
            .status
            .success()
    );
}
