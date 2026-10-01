//! `episode run` with `CODER_ONE_CODEX_LOGIN=take` removes the run's own
//! copy of the Codex login before anything else it does, even when the
//! episode then fails, and refuses to run when the copy can't be removed
//! (issue #9599). It never removes the person's own login: a link is
//! refused with the file it names untouched, and without `CODEX_HOME`, or
//! with it at `~/.codex`, nothing is taken (issue #10083). Temporary
//! folders only; no real home is touched.

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
/// after the take; its standard error. `home` is `CODEX_HOME`, if any.
fn run(root: &Path, home: Option<&Path>) -> (bool, String) {
    let instruction = root.join("instruction.txt");
    std::fs::write(&instruction, "say hello").unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_coder-one"));
    let output = command
        .args(["episode", "run", "--contract", CONTRACT])
        .arg("--instruction-file")
        .arg(&instruction)
        .arg("--output-dir")
        .arg(root.join("episode"))
        .current_dir(root)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", root)
        .env("CODER_ONE_CODEX_LOGIN", "take");
    let output = match home {
        Some(home) => output.env("CODEX_HOME", home),
        None => output,
    }
    .output()
    .unwrap();
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stderr).into_owned(),
    )
}

#[test]
fn the_run_removes_its_copy_of_the_login_before_it_fails() {
    let root = tempfile::tempdir().unwrap();
    let home = root.path().join("codex-home");
    std::fs::create_dir_all(&home).unwrap();
    let copy = home.join("auth.json");
    std::fs::write(&copy, login()).unwrap();

    let (ok, stderr) = run(root.path(), Some(&home));
    assert!(!ok);
    assert!(stderr.contains("OPENAGENTS_API_KEY"), "{stderr}");
    assert!(std::fs::symlink_metadata(&copy).is_err());
    assert!(!stderr.contains("acct-test"));
}

/// A dotfiles-style `auth.json` link under `CODEX_HOME`: the run stops
/// before the model runs anything, and the link and the person's file it
/// names are both left exactly as they were (#10083).
#[test]
fn a_linked_login_stops_the_run_and_the_persons_file_stays() {
    let root = tempfile::tempdir().unwrap();
    let dotfiles = root.path().join("dotfiles");
    let home = root.path().join("codex-home");
    std::fs::create_dir_all(&dotfiles).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    let real = dotfiles.join("auth.json");
    std::fs::write(&real, login()).unwrap();
    let link = home.join("auth.json");
    std::os::unix::fs::symlink(&real, &link).unwrap();

    let (ok, stderr) = run(root.path(), Some(&home));
    assert!(!ok);
    assert!(stderr.contains("is a link"), "{stderr}");
    assert!(
        !stderr.contains("OPENAGENTS_API_KEY"),
        "stopped first: {stderr}"
    );
    assert_eq!(std::fs::read_to_string(&real).unwrap(), login());
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!stderr.contains("acct-test"));
}

/// Without `CODEX_HOME`, or with it at `~/.codex`, the login is the
/// person's own: the run refuses to take it and leaves it (#10083).
#[test]
fn the_persons_own_login_is_never_taken() {
    let root = tempfile::tempdir().unwrap();
    let person = root.path().join(".codex");
    std::fs::create_dir_all(&person).unwrap();
    let file = person.join("auth.json");
    std::fs::write(&file, login()).unwrap();

    for home in [None, Some(person.as_path())] {
        let (ok, stderr) = run(root.path(), home);
        assert!(!ok);
        assert!(stderr.contains("CODEX_HOME"), "{stderr}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), login());
    }
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

    let (ok, stderr) = run(root.path(), Some(&home));
    std::fs::set_permissions(&home, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(!ok);
    if !removable {
        assert!(stderr.contains("can't remove"), "{stderr}");
    }
}
