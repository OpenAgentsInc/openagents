//! Ephemeral Git auth against the real `git`. A separate test binary, so
//! its child processes never share a store lease descriptor with the
//! session tests.

use coder_environment_setup::git_auth_env;

const GH_SECRET: &str = "ghp_setup_secret_value_0123456789";

/// Real Git: the per-process helper authenticates, and neither the clone's
/// `.git/config` nor any file under `.git` holds the token.
#[test]
fn ephemeral_git_auth_never_writes_the_token() {
    use std::process::{Command, Stdio};
    if Command::new("git").arg("--version").output().is_err() {
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    let git = |args: &[&str], auth: bool| {
        let mut c = Command::new("git");
        c.args(args)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", &home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if auth {
            c.envs(git_auth_env("GH_TOKEN")).env("GH_TOKEN", GH_SECRET);
        }
        c
    };
    let source = dir.path().join("source.git");
    let source = source.to_str().unwrap();
    assert!(
        git(&["init", "--bare", "-q", source], false)
            .status()
            .unwrap()
            .success()
    );

    // The helper answers from the variable at run time.
    let mut fill = git(&["credential", "fill"], true).spawn().unwrap();
    use std::io::Write;
    fill.stdin
        .take()
        .unwrap()
        .write_all(b"protocol=https\nhost=github.com\n\n")
        .unwrap();
    let out = fill.wait_with_output().unwrap();
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains(&format!("password={GH_SECRET}")), "{text}");
    assert!(text.contains("username=x-access-token"));

    // Any other host, or plain http, gets nothing: a setup command's Git
    // dependency or submodule elsewhere is never handed the token.
    for asked in [
        &b"protocol=https\nhost=evil.example\n\n"[..],
        b"protocol=https\nhost=github.com.evil.example\n\n",
        b"protocol=http\nhost=github.com\n\n",
    ] {
        let mut fill = git(&["credential", "fill"], true).spawn().unwrap();
        fill.stdin.take().unwrap().write_all(asked).unwrap();
        let out = fill.wait_with_output().unwrap();
        let text = String::from_utf8_lossy(&out.stdout);
        assert!(!text.contains(GH_SECRET), "{text}");
    }

    let work = dir.path().join("work");
    let work_s = work.to_str().unwrap();
    assert!(
        git(&["clone", "-q", source, work_s], true)
            .status()
            .unwrap()
            .success()
    );
    let config = std::fs::read_to_string(work.join(".git/config")).unwrap();
    assert!(!config.contains(GH_SECRET));
    assert!(!config.contains("credential"));
    let mut stack = vec![work.join(".git")];
    while let Some(p) = stack.pop() {
        for entry in std::fs::read_dir(&p).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                let bytes = std::fs::read(&path).unwrap();
                assert!(
                    !bytes
                        .windows(GH_SECRET.len())
                        .any(|w| w == GH_SECRET.as_bytes()),
                    "{path:?} holds the token"
                );
            }
        }
    }
    // Only the process environment carries the helper.
    let origin = git(
        &[
            "-C",
            work_s,
            "config",
            "--show-origin",
            "--get-all",
            "credential.helper",
        ],
        true,
    )
    .output()
    .unwrap();
    let origin = String::from_utf8_lossy(&origin.stdout);
    assert!(
        origin.lines().all(|l| l.starts_with("command line:")),
        "{origin}"
    );
    let none = git(
        &["-C", work_s, "config", "--get-all", "credential.helper"],
        false,
    )
    .output()
    .unwrap();
    assert!(!none.status.success());
}
