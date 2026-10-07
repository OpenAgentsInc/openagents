//! #10748: the Boat owner program, run for real with `sh` against a local
//! Git repository and a stub engine in a temporary directory. No network,
//! Boat account, or model is used.
#![cfg(all(feature = "boat", unix))]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::{Duration, Instant};

use retail_cloud::boat::{OWNER_SCRIPT, parse_status, parse_stop};
use retail_cloud::dispatch::{ExecutorEnd, TaskStatus};
use retail_cloud::sha256_hex;

const KEY: &str = "sk-fake-owner-script-0000";

fn owner(script: &Path, args: &[&str], env: &[(&str, &Path)]) -> String {
    let mut command = Command::new("sh");
    command.arg(script).args(args);
    for (k, v) in env {
        command.env(k, v);
    }
    let out = command.output().expect("sh runs");
    assert!(out.status.success(), "{args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.com")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.com")
        .output()
        .unwrap();
    assert!(out.status.success(), "git {args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

#[test]
fn the_owner_program_clones_runs_checks_scrubs_and_stops() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let script = root.join("owner-v1.sh");
    std::fs::write(&script, OWNER_SCRIPT).unwrap();

    // The admitted source.
    let upstream = root.join("upstream");
    std::fs::create_dir(&upstream).unwrap();
    git(&upstream, &["init", "-q"]);
    std::fs::write(upstream.join("parse.txt"), "a,b\n").unwrap();
    git(&upstream, &["add", "-A"]);
    git(&upstream, &["commit", "-q", "-m", "seed"]);
    let commit = git(&upstream, &["rev-parse", "HEAD"]);

    // A stub engine: login echoes the key it reads (so the log must be
    // scrubbed), and exec edits the workspace.
    let engine = root.join("engine");
    std::fs::write(
        &engine,
        "#!/bin/sh\ncase $1 in\n login) cat; echo; echo logged in ;;\n exec) echo 'a,b,' > parse.txt; echo edited ;;\nesac\n",
    )
    .unwrap();
    std::fs::set_permissions(&engine, std::fs::Permissions::from_mode(0o755)).unwrap();

    let base = root.join("oa-retail");
    let ws = base.join("src");
    let ws_s = ws.to_str().unwrap();
    let out = owner(
        &script,
        &["clone", upstream.to_str().unwrap(), &commit, ws_s],
        &[],
    );
    assert_eq!(out, format!("{commit} clean"));
    let source = std::fs::read_to_string(base.join("source")).unwrap();
    assert_eq!(source.trim(), format!("{} {commit}", upstream.display()));
    // Cloning again keeps the workspace.
    assert_eq!(
        owner(
            &script,
            &["clone", upstream.to_str().unwrap(), &commit, ws_s],
            &[]
        ),
        format!("{commit} clean")
    );

    let key = base.join("exec-1/provider.key");
    let key_s = key.to_str().unwrap();
    assert_eq!(owner(&script, &["prepare", key_s], &[]), "prepared");
    std::fs::write(&key, KEY).unwrap();
    assert_eq!(owner(&script, &["private", key_s], &[]), "600");
    assert_eq!(owner(&script, &["exists", key_s], &[]), "yes");

    let dir = base.join("task/task_exec-1");
    let dir_s = dir.to_str().unwrap();
    owner(&script, &["prepare", &format!("{dir_s}/checks/count")], &[]);
    for (name, text) in [
        ("prompt", "Make the parser accept trailing commas."),
        ("key", key_s),
        ("workspace", ws_s),
        ("max_seconds", "60"),
        ("checks/count", "1"),
        ("checks/0", "grep -q 'a,b,' parse.txt"),
    ] {
        std::fs::write(dir.join(name), text).unwrap();
    }
    let env = [("OA_RETAIL_ENGINE", engine.as_path())];
    assert_eq!(owner(&script, &["submit", dir_s], &env), "new");
    assert_eq!(owner(&script, &["submit", dir_s], &env), "existing");

    let checks = vec!["grep -q 'a,b,' parse.txt".to_owned()];
    let deadline = Instant::now() + Duration::from_secs(30);
    let status = loop {
        let text = std::fs::read_to_string(dir.join("status")).unwrap_or_default();
        if let Some(status @ TaskStatus::Ended { .. }) = parse_status(&text, &checks) {
            break status;
        }
        assert!(Instant::now() < deadline, "the runner never ended: {text}");
        std::thread::sleep(Duration::from_millis(50));
    };
    let patch_bytes = std::fs::read(dir.join("artifacts/patch")).unwrap();
    let patch = sha256_hex(&patch_bytes);
    let TaskStatus::Ended {
        end,
        patch: got,
        checks: runs,
    } = status
    else {
        unreachable!()
    };
    assert_eq!(end, ExecutorEnd::Completed);
    assert_eq!(got.as_deref(), Some(patch.as_str()));
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].exit_status, 0);
    assert_eq!(runs[0].candidate, patch);

    let log = std::fs::read_to_string(dir.join("artifacts/log")).unwrap();
    assert!(!log.contains(KEY), "{log}");
    assert!(log.contains("[redacted]"), "{log}");
    let manifest = std::fs::read_to_string(dir.join("manifest")).unwrap();
    assert_eq!(manifest.lines().count(), 3, "{manifest}");
    assert!(manifest.contains(&format!("patch patch {patch} ")));
    let events = std::fs::read_to_string(dir.join("events")).unwrap();
    assert!(events.lines().count() >= 3, "{events}");

    assert_eq!(owner(&script, &["stop", dir_s, "stop-1"], &[]), "stopped");
    assert_eq!(owner(&script, &["stop", dir_s, "stop-1"], &[]), "existing");
    let receipt = std::fs::read_to_string(dir.join("stop/stop-1")).unwrap();
    let evidence = parse_stop(&receipt, &checks).unwrap();
    assert!(evidence.started);
    assert_eq!(evidence.effects, vec![patch]);
    assert!(matches!(evidence.status, TaskStatus::Ended { .. }));

    assert_eq!(owner(&script, &["remove", key_s], &[]), "removed");
    assert_eq!(owner(&script, &["exists", key_s], &[]), "no");
}

#[test]
fn a_stop_before_the_runner_starts_cancels_the_task() {
    let tmp = tempfile::tempdir().unwrap();
    let script = tmp.path().join("owner-v1.sh");
    std::fs::write(&script, OWNER_SCRIPT).unwrap();
    let dir = tmp.path().join("task/t");
    std::fs::create_dir_all(&dir).unwrap();
    let dir_s = dir.to_str().unwrap();
    assert_eq!(owner(&script, &["stop", dir_s, "r"], &[]), "stopped");
    let receipt = std::fs::read_to_string(dir.join("stop/r")).unwrap();
    let evidence = parse_stop(&receipt, &[]).unwrap();
    assert!(!evidence.started);
    assert!(evidence.effects.is_empty());
    assert_eq!(evidence.status, TaskStatus::Cancelled);
}
