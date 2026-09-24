//! `episode run` with `CODER_ONE_CODEX_LOGIN=take` removes the Codex login
//! before anything else it does, even when the episode then fails, and
//! refuses to run when the login can't be removed (issue #9599).

#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

const CONTRACT: &str = "openagents.coder.episode.v1";

fn login() -> String {
    r#"{"auth_mode":"chatgpt","tokens":{"access_token":"a.b.c","account_id":"acct-test"}}"#
        .to_string()
}

/// `episode run` with the login taken and no door key, so it stops right
/// after the take; its standard error.
fn run(root: &Path, home: &Path) -> (bool, String) {
    let instruction = root.join("instruction.txt");
    std::fs::write(&instruction, "say hello").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_coder-one"))
        .args(["episode", "run", "--contract", CONTRACT])
        .arg("--instruction-file")
        .arg(&instruction)
        .arg("--output-dir")
        .arg(root.join("episode"))
        .current_dir(root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", root)
        .env("CODEX_HOME", home)
        .env("CODER_ONE_CODEX_LOGIN", "take")
        .output()
        .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_run_removes_the_login_and_its_link_before_it_fails() {
    let root = tempfile::tempdir().unwrap();
    let secrets = root.path().join("secrets");
    let home = root.path().join("codex-home");
    std::fs::create_dir_all(&secrets).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let file = secrets.join("auth.json");
    std::fs::write(&file, login()).unwrap();
    std::os::unix::fs::symlink(&file, home.join("auth.json")).unwrap();

    let (ok, stderr) = run(root.path(), &home);
    assert!(!ok);
    assert!(stderr.contains("OPENAGENTS_API_KEY"), "{stderr}");
    assert!(!file.exists());
    assert!(std::fs::symlink_metadata(home.join("auth.json")).is_err());
    assert!(!stderr.contains("acct-test"));
}

#[test]
fn a_login_that_cant_be_removed_stops_the_run() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("codex-home");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(home.join("auth.json"), login()).unwrap();
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o500)).unwrap();
    // Root can remove it anyway, so the refusal can't be observed.
    let removable = std::fs::File::create(home.join("probe")).is_ok();

    let (ok, stderr) = run(root.path(), &home);
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!ok);
    if !removable {
        assert!(stderr.contains("can't remove"), "{stderr}");
    }
}
