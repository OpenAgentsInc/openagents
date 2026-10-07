//! `openagents browser run` end to end, under a temporary HOME, with a
//! stand-in browser: a script that writes `DevToolsActivePort` into its
//! `--user-data-dir` the way Chrome does, records its process, and sleeps.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

const STAND_IN: &str = r#"#!/bin/sh
for arg in "$@"; do
  case "$arg" in --user-data-dir=*) dir="${arg#--user-data-dir=}" ;; esac
done
echo "$$" >> "$STAND_IN_PIDS"
echo "$*" > "$STAND_IN_PIDS.args"
port=$(( $$ % 30000 + 20000 ))
printf '%s\n/devtools/browser/stand-in-%s\n' "$port" "$$" > "$dir/DevToolsActivePort"
exec sleep 600
"#;

struct Fixture {
    _dir: tempfile::TempDir,
    home: PathBuf,
    browser: PathBuf,
    pids: PathBuf,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let home = dir.path().canonicalize().unwrap();
    let browser = home.join("stand-in-chrome");
    std::fs::write(&browser, STAND_IN).unwrap();
    std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o755)).unwrap();
    let pids = home.join("pids");
    Fixture {
        _dir: dir,
        home,
        browser,
        pids,
    }
}

fn command(fixture: &Fixture, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_openagents"));
    command
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", &fixture.home)
        .env("OPENAGENTS_SESSION", "test:browser")
        .env("STAND_IN_PIDS", &fixture.pids)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    command
}

fn browser_run(fixture: &Fixture, script: &str) -> Command {
    command(
        fixture,
        &[
            "--json",
            "browser",
            "run",
            "--browser",
            fixture.browser.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            script,
        ],
    )
}

/// The summary `--json` prints after the command's own output.
fn summary(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().last().unwrap_or_default();
    serde_json::from_str(line).unwrap_or_else(|_| panic!("no summary in {output:?}"))
}

fn alive(pid: i32) -> bool {
    // SAFETY: signal 0 only asks whether the process exists.
    unsafe { libc::kill(pid, 0) == 0 }
}

fn pids(fixture: &Fixture) -> Vec<i32> {
    std::fs::read_to_string(&fixture.pids)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().parse().ok())
        .collect()
}

fn scratch(fixture: &Fixture) -> PathBuf {
    fixture.home.join(".openagents/scratch/test-browser")
}

fn profiles(dir: &Path) -> Vec<PathBuf> {
    std::fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .map(|entry| entry.path())
                .filter(|path| {
                    path.file_name()
                        .is_some_and(|name| name.to_string_lossy().starts_with("chrome-"))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn wait_output(child: Child) -> Output {
    child.wait_with_output().unwrap()
}

#[test]
fn two_concurrent_runs_get_their_own_port_and_profile_and_both_are_removed() {
    let fixture = fixture();
    // Each command reports what it was given, then outlives the other's
    // start so the two browsers run at once.
    let script = r#"echo "$OPENAGENTS_CHROME_PORT $OPENAGENTS_CHROME_WS"; sleep 1"#;
    let first = browser_run(&fixture, script).spawn().unwrap();
    let second = browser_run(&fixture, script).spawn().unwrap();
    let outputs = [wait_output(first), wait_output(second)];
    let mut ports = Vec::new();
    let mut found = Vec::new();
    for output in &outputs {
        assert!(output.status.success(), "{output:?}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let given: Vec<&str> = stdout.lines().next().unwrap().split(' ').collect();
        let value = summary(output);
        assert_eq!(given[0], value["port"].to_string());
        assert_eq!(given[1], value["ws"].as_str().unwrap());
        assert!(given[1].starts_with(&format!("ws://127.0.0.1:{}/devtools/browser/", given[0])));
        let profile = PathBuf::from(value["profile"].as_str().unwrap());
        assert_eq!(profile.parent().unwrap(), scratch(&fixture));
        assert_eq!(value["profile_removed"], true);
        assert!(!profile.exists(), "{} was left behind", profile.display());
        ports.push(given[0].to_owned());
        found.push(profile);
    }
    assert_ne!(ports[0], ports[1]);
    assert_ne!(found[0], found[1]);
    assert!(profiles(&scratch(&fixture)).is_empty());

    let started = pids(&fixture);
    assert_eq!(started.len(), 2, "{started:?}");
    for pid in started {
        assert!(!alive(pid), "the browser {pid} outlived its run");
    }
    // Headless by default, on a port Chrome picks.
    let args = std::fs::read_to_string(fixture.pids.with_extension("args")).unwrap();
    assert!(args.contains("--headless=new"), "{args}");
    assert!(args.contains("--remote-debugging-port=0"), "{args}");
}

#[test]
fn a_failing_command_keeps_its_status_and_still_removes_the_profile() {
    let fixture = fixture();
    let output = browser_run(&fixture, "exit 3").output().unwrap();
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let value = summary(&output);
    assert_eq!(value["exit"], 3);
    assert_eq!(value["profile_removed"], true);
    assert!(profiles(&scratch(&fixture)).is_empty());
    for pid in pids(&fixture) {
        assert!(!alive(pid), "the browser {pid} outlived its run");
    }
}

#[test]
fn an_interrupted_run_ends_the_browser_and_removes_the_profile() {
    let fixture = fixture();
    let child = browser_run(&fixture, "touch \"$HOME/started\"; sleep 30")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(20);
    while !fixture.home.join("started").exists() {
        assert!(Instant::now() < deadline, "the command never started");
        std::thread::sleep(Duration::from_millis(20));
    }
    let pid = i32::try_from(child.id()).unwrap();
    // SAFETY: signalling the child this test started.
    unsafe { libc::kill(pid, libc::SIGTERM) };
    let output = wait_output(child);
    assert!(!output.status.success(), "{output:?}");
    assert!(profiles(&scratch(&fixture)).is_empty());
    let started = pids(&fixture);
    assert_eq!(started.len(), 1, "{started:?}");
    assert!(!alive(started[0]));
}

#[test]
fn a_missing_browser_fails_before_the_command_and_leaves_no_profile() {
    let fixture = fixture();
    let output = command(
        &fixture,
        &[
            "browser",
            "run",
            "--browser",
            "/nonexistent/chrome",
            "--",
            "sh",
            "-c",
            "touch \"$HOME/ran\"",
        ],
    )
    .output()
    .unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("did not start"));
    assert!(!fixture.home.join("ran").exists());
    assert!(profiles(&scratch(&fixture)).is_empty());
}

#[test]
fn a_headed_run_needs_the_owners_screen_grant() {
    let fixture = fixture();
    let output = command(
        &fixture,
        &[
            "browser",
            "run",
            "--headed",
            "--browser",
            fixture.browser.to_str().unwrap(),
            "--",
            "true",
        ],
    )
    .output()
    .unwrap();
    assert!(!output.status.success(), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("openagents lease grant screen"), "{stderr}");
    assert!(
        pids(&fixture).is_empty(),
        "the browser started without a grant"
    );
}

/// The real Chrome, headless, when it is installed here: the command
/// reaches its DevTools endpoint on the port it was given.
#[test]
fn real_headless_chrome_answers_on_the_given_port_when_installed() {
    let chrome = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    if !chrome.is_file() || Command::new("curl").arg("--version").output().is_err() {
        eprintln!("skipped: no Chrome or curl here");
        return;
    }
    let fixture = fixture();
    let output = command(
        &fixture,
        &[
            "--json",
            "browser",
            "run",
            "--browser",
            chrome.to_str().unwrap(),
            "--",
            "sh",
            "-c",
            "curl -sf \"http://127.0.0.1:$OPENAGENTS_CHROME_PORT/json/version\"",
        ],
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("webSocketDebuggerUrl"), "{stdout}");
    let value = summary(&output);
    assert_eq!(value["profile_removed"], true);
    assert!(profiles(&scratch(&fixture)).is_empty());
}
