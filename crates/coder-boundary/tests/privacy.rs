//! Nothing a privacy-sandboxed command does reaches a place macOS guards
//! with a privacy prompt: it gets an ordinary "Operation not permitted",
//! which a sandbox denial returns before the privacy check runs, so no
//! prompt is shown. The tests use a made-up home with a `Music` folder of
//! its own and never touch the real one.
#![cfg(target_os = "macos")]

use std::path::Path;
use std::process::Command;

use coder_boundary::{Boundary, SANDBOX_EXEC, privacy};
use tempfile::TempDir;

fn fake_home() -> (TempDir, std::path::PathBuf) {
    let dir = TempDir::new().unwrap();
    let home = dir.path().canonicalize().unwrap();
    std::fs::create_dir_all(home.join("Music/Music")).unwrap();
    std::fs::write(home.join("Music/Music/library"), "songs").unwrap();
    std::fs::create_dir_all(home.join("work/project")).unwrap();
    std::fs::write(home.join("work/project/file"), "code").unwrap();
    (dir, home)
}

fn sandboxed(profile: &str, program: &str, args: &[&Path]) -> std::process::Output {
    Command::new(SANDBOX_EXEC)
        .arg("-p")
        .arg(profile)
        .arg(program)
        .args(args)
        .output()
        .unwrap()
}

#[test]
fn a_privacy_sandboxed_command_gets_eperm_in_music_and_works_elsewhere() {
    if privacy::sandboxed() || !Path::new(SANDBOX_EXEC).is_file() {
        return;
    }
    let (_dir, home) = fake_home();
    let profile = privacy::profile(&home, &[]);
    let read = sandboxed(&profile, "/bin/cat", &[&home.join("Music/Music/library")]);
    assert!(!read.status.success());
    let error = String::from_utf8_lossy(&read.stderr);
    assert!(error.contains("Operation not permitted"), "{error}");
    let list = sandboxed(&profile, "/bin/ls", &[&home.join("Music")]);
    assert!(!list.status.success());
    let write = sandboxed(&profile, "/usr/bin/touch", &[&home.join("Music/new")]);
    assert!(!write.status.success());
    assert!(!home.join("Music/new").exists());
    // The home itself still lists, Music included, and the rest reads.
    let home_listing = sandboxed(&profile, "/bin/ls", &[&home]);
    assert!(home_listing.status.success());
    assert!(String::from_utf8_lossy(&home_listing.stdout).contains("Music"));
    let code = sandboxed(&profile, "/bin/cat", &[&home.join("work/project/file")]);
    assert_eq!(String::from_utf8_lossy(&code.stdout), "code");
    // The owner's set-user-ID tools still run, as they do unsandboxed.
    let ps = Command::new(SANDBOX_EXEC)
        .args(["-p", &profile, "/bin/sh", "-c", "ps -p $$ -o pid="])
        .output()
        .unwrap();
    assert!(
        ps.status.success(),
        "{}",
        String::from_utf8_lossy(&ps.stderr)
    );
}

#[test]
fn a_checkout_inside_a_protected_folder_is_allowed_back() {
    if privacy::sandboxed() || !Path::new(SANDBOX_EXEC).is_file() {
        return;
    }
    let (_dir, home) = fake_home();
    let checkout = home.join("Documents/project");
    std::fs::create_dir_all(&checkout).unwrap();
    std::fs::write(checkout.join("file"), "mine").unwrap();
    std::fs::write(home.join("Documents/other"), "private").unwrap();
    let profile = privacy::profile(&home, &[checkout.as_path()]);
    let ok = sandboxed(&profile, "/bin/cat", &[&checkout.join("file")]);
    assert_eq!(String::from_utf8_lossy(&ok.stdout), "mine");
    let denied = sandboxed(&profile, "/bin/cat", &[&home.join("Documents/other")]);
    assert!(!denied.status.success());
}

#[test]
fn a_boundary_still_starts_inside_the_privacy_sandbox() {
    if privacy::sandboxed() || !Path::new(SANDBOX_EXEC).is_file() {
        return;
    }
    let (_dir, home) = fake_home();
    let Ok(boundary) = Boundary::readonly().build() else {
        return;
    };
    let mut inner = vec![
        SANDBOX_EXEC.to_owned(),
        "-f".to_owned(),
        boundary.profile_file().display().to_string(),
        "/usr/bin/true".to_owned(),
    ];
    let profile = privacy::profile(&home, &[]);
    let program = inner.remove(0);
    let output = Command::new(SANDBOX_EXEC)
        .arg("-p")
        .arg(&profile)
        .arg(program)
        .args(&inner)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn every_boundary_denies_the_real_protected_places() {
    let Some(home) = privacy::home() else {
        return;
    };
    let Ok(boundary) = Boundary::readonly().build() else {
        return;
    };
    let music = home.join("Music");
    assert!(
        boundary.profile().contains(&format!(
            "(deny file-read* file-write* (subpath \"{}\"))",
            music.display()
        )),
        "{}",
        boundary.profile()
    );
    assert!(boundary.profile().contains("(deny appleevent-send)"));
    // A boundary never lets its command leave it the way the privacy-only
    // sandbox lets a boundary start.
    assert!(!boundary.profile().contains("no-sandbox"));
}

#[test]
fn the_command_helper_wraps_with_the_account_profile() {
    let command = privacy::command("/bin/echo", &[]);
    if privacy::sandboxed() || !Path::new(SANDBOX_EXEC).is_file() {
        assert_eq!(command.get_program(), "/bin/echo");
        return;
    }
    assert_eq!(command.get_program(), SANDBOX_EXEC);
    let args: Vec<_> = command.get_args().collect();
    assert_eq!(args[0], "-p");
    assert_eq!(args[2], "/bin/echo");
}
