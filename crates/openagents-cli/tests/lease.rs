//! `openagents lease` end to end, under a temporary lease root and HOME.
#![cfg(unix)]

use std::path::Path;
use std::process::{Command, Output, Stdio};

use serde_json::Value;

fn openagents(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("OPENAGENTS_LEASE_ROOT", home.join("leases"))
        .env("OPENAGENTS_SESSION", "lease-test")
        .env("OPENAGENTS_BUILD_LEASES", "1")
        // Builds take a target slot; the test's volume may be short of
        // the default floor.
        .env("OPENAGENTS_SLOT_FREE_GB", "0")
        .stdin(Stdio::null())
        .output()
        .unwrap()
}

#[test]
fn lease_runs_the_command_under_the_lease_and_leaves_a_receipt() {
    let dir = tempfile::tempdir().unwrap();
    let receipt = dir.path().join("receipt.json");
    let output = openagents(
        dir.path(),
        &[
            "lease",
            "quiet",
            "--receipt",
            receipt.to_str().unwrap(),
            "--json",
            "--",
            "sh",
            "-c",
            "echo \"$OPENAGENTS_LEASES $OPENAGENTS_SESSION\"; echo --json; exit 7",
        ],
    );
    assert_eq!(output.status.code(), Some(7), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let mut lines = stdout.lines();
    assert_eq!(lines.next(), Some("quiet lease-test"));
    // `--json` after `--` belongs to the command.
    assert_eq!(lines.next(), Some("--json"));
    let printed: Value = serde_json::from_str(lines.next().unwrap()).unwrap();
    assert_eq!(printed["resource"], "quiet");
    assert_eq!(printed["exit"], 7);
    assert_eq!(printed["held_whole_run"], true);
    assert_eq!(printed["holder"]["command"], "sh");
    let written: Value = serde_json::from_slice(&std::fs::read(&receipt).unwrap()).unwrap();
    assert_eq!(written, printed);
    let id = written["id"].as_str().unwrap();
    assert!(
        dir.path()
            .join(format!("leases/receipts/{id}.json"))
            .exists()
    );
}

#[test]
fn nesting_passes_through_and_no_wait_refuses_a_busy_resource() {
    let dir = tempfile::tempdir().unwrap();
    let me = env!("CARGO_BIN_EXE_openagents");
    // The only build slot is held by the outer command; the inner one runs
    // inside it without waiting, and a plain --no-wait request is refused.
    let script = format!(
        "{me} lease build --no-wait -- true && \
         env -u OPENAGENTS_LEASES {me} lease build --no-wait -- true; echo inner=$?"
    );
    let output = openagents(dir.path(), &["lease", "build", "--", "sh", "-c", &script]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("inner=1"), "{stdout}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("build is not free"), "{stderr}");

    let listed = openagents(dir.path(), &["lease", "list", "--json"]);
    assert!(listed.status.success(), "{listed:?}");
    let value: Value = serde_json::from_slice(&listed.stdout).unwrap();
    assert_eq!(value["leases"], Value::Array(Vec::new()));
    assert_eq!(value["limits"]["build"], 1);
}

#[test]
fn the_screen_is_refused_without_a_grant_and_a_grant_needs_a_terminal() {
    let dir = tempfile::tempdir().unwrap();
    let output = openagents(dir.path(), &["lease", "screen", "--", "true"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("grant"),
        "{output:?}"
    );
    let output = openagents(dir.path(), &["lease", "grant", "screen", "--for", "10m"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not a terminal"),
        "{output:?}"
    );
    let output = openagents(dir.path(), &["lease", "revoke", "screen", "--json"]);
    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["revoked"], false);
    let output = openagents(dir.path(), &["lease", "grant", "gpu"]);
    assert_eq!(output.status.code(), Some(64));
}

#[test]
fn lease_build_runs_in_a_target_slot_and_lists_its_holder() {
    let dir = tempfile::tempdir().unwrap();
    let me = env!("CARGO_BIN_EXE_openagents");
    let script = format!("echo \"target=$CARGO_TARGET_DIR\"; {me} lease list --json");
    let output = openagents(dir.path(), &["lease", "build", "--", "sh", "-c", &script]);
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let (first, listed) = stdout.split_once('\n').unwrap();
    let target = first.strip_prefix("target=").unwrap();
    let slots = dir.path().join(".openagents/targets");
    assert!(
        Path::new(target).starts_with(&slots) && target.contains("-slot-0"),
        "{target}"
    );
    assert!(Path::new(target).is_dir());
    let value: Value = serde_json::from_str(listed).unwrap();
    let held = &value["leases"][0];
    assert_eq!(held["resource"], "build");
    assert_eq!(held["state"], "held");
    assert_eq!(held["holder"]["session"], "lease-test");
    assert_eq!(held["holder"]["command"], "sh");

    // --keep-target-dir keeps a target directory already set.
    let output = Command::new(me)
        .args([
            "lease",
            "build",
            "--keep-target-dir",
            "--",
            "sh",
            "-c",
            "echo \"$CARGO_TARGET_DIR\"",
        ])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env("OPENAGENTS_LEASE_ROOT", dir.path().join("leases"))
        .env("CARGO_TARGET_DIR", "/kept/target")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "/kept/target"
    );
}

#[test]
fn a_second_build_waits_for_the_first_under_a_count_of_one() {
    let dir = tempfile::tempdir().unwrap();
    let me = env!("CARGO_BIN_EXE_openagents");
    let log = dir.path().join("log");
    let mut first = Command::new(me)
        .args([
            "lease",
            "build",
            "--",
            "sh",
            "-c",
            &format!(
                "echo first-start >> {0}; sleep 2; echo first-end >> {0}",
                log.display()
            ),
        ])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env("OPENAGENTS_LEASE_ROOT", dir.path().join("leases"))
        .env("OPENAGENTS_BUILD_LEASES", "1")
        .env("OPENAGENTS_SLOT_FREE_GB", "0")
        .stdin(Stdio::null())
        .spawn()
        .unwrap();
    let started = std::time::Instant::now();
    while !std::fs::read_to_string(&log).is_ok_and(|text| text.contains("first-start")) {
        assert!(
            started.elapsed().as_secs() < 30,
            "the first build never started"
        );
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let script = format!("echo second >> {}", log.display());
    let second = openagents(dir.path(), &["lease", "build", "--", "sh", "-c", &script]);
    assert!(second.status.success(), "{second:?}");
    let stderr = String::from_utf8_lossy(&second.stderr);
    assert!(stderr.contains("waiting for build"), "{stderr}");
    assert!(first.wait().unwrap().success());
    assert_eq!(
        std::fs::read_to_string(&log).unwrap(),
        "first-start\nfirst-end\nsecond\n"
    );
}

/// Waiters line up by priority: `--priority` and the session's
/// `OPENAGENTS_LEASE_PRIORITY`, and `lease list` shows each one's place,
/// priority, and wait.
#[test]
fn waiters_line_up_by_priority_and_list_shows_their_places() {
    let dir = tempfile::tempdir().unwrap();
    let me = env!("CARGO_BIN_EXE_openagents");
    let gate = dir.path().join("gate");
    let spawn = |priority: Option<&str>, variable: Option<&str>, script: String| {
        let mut command = Command::new(me);
        command.args(["lease", "gpu"]);
        if let Some(priority) = priority {
            command.args(["--priority", priority]);
        }
        command
            .args(["--", "sh", "-c", &script])
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", dir.path())
            .env("OPENAGENTS_LEASE_ROOT", dir.path().join("leases"))
            .env(
                "OPENAGENTS_SESSION",
                priority.or(variable).unwrap_or("holder"),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        if let Some(variable) = variable {
            command.env("OPENAGENTS_LEASE_PRIORITY", variable);
        }
        command.spawn().unwrap()
    };
    let list = || {
        let listed = openagents(dir.path(), &["lease", "list", "--json"]);
        assert!(listed.status.success(), "{listed:?}");
        serde_json::from_slice::<Value>(&listed.stdout).unwrap()
    };
    let waiting = |count: usize| {
        let started = std::time::Instant::now();
        loop {
            let value = list();
            let leases = value["leases"].as_array().unwrap().clone();
            if leases
                .iter()
                .filter(|lease| lease["state"] == "waiting")
                .count()
                >= count
            {
                return value;
            }
            assert!(started.elapsed().as_secs() < 30, "{value}");
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    };
    // The holder runs until the gate file appears.
    let mut holder = spawn(
        None,
        None,
        format!("while [ ! -e {} ]; do sleep 0.05; done", gate.display()),
    );
    let started = std::time::Instant::now();
    while list()["leases"].as_array().unwrap().is_empty() {
        assert!(started.elapsed().as_secs() < 30);
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    let mut background = spawn(Some("background"), None, "true".to_owned());
    waiting(1);
    let mut push = spawn(None, Some("push"), "true".to_owned());
    let value = waiting(2);
    assert_eq!(value["aging_minutes"], 20);
    let rows: Vec<(String, String, Value)> = value["leases"]
        .as_array()
        .unwrap()
        .iter()
        .map(|lease| {
            (
                lease["holder"]["session"].as_str().unwrap().to_owned(),
                lease["priority"].as_str().unwrap().to_owned(),
                lease["position"].clone(),
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("holder".to_owned(), "normal".to_owned(), Value::Null),
            ("push".to_owned(), "push".to_owned(), Value::from(1)),
            (
                "background".to_owned(),
                "background".to_owned(),
                Value::from(2)
            ),
        ]
    );
    assert!(value["leases"][2]["wait_ms"].as_u64().is_some());
    // The table shows the same.
    let text = openagents(dir.path(), &["lease", "list"]);
    let text = String::from_utf8_lossy(&text.stdout);
    assert!(text.contains("waiting #1"), "{text}");
    assert!(text.contains("PRIORITY"), "{text}");
    std::fs::write(&gate, b"").unwrap();
    for child in [&mut holder, &mut background, &mut push] {
        assert!(child.wait().unwrap().success());
    }
    let refused = openagents(
        dir.path(),
        &["lease", "gpu", "--priority", "urgent", "--", "true"],
    );
    assert_eq!(refused.status.code(), Some(64), "{refused:?}");
}

#[test]
fn lease_build_refuses_below_the_floor_and_names_the_reclaim_command() {
    let dir = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(["lease", "build", "--", "true"])
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", dir.path())
        .env("OPENAGENTS_LEASE_ROOT", dir.path().join("leases"))
        .env("OPENAGENTS_SLOT_FREE_GB", "1000000000")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("GB free"), "{stderr}");
    assert!(stderr.contains("the floor is 1000000000 GB"), "{stderr}");
    assert!(
        stderr.contains("openagents background run disk"),
        "{stderr}"
    );
}

/// The `cargo` shim end to end: a stand-in `cargo` reports the lease and
/// target directory it ran under.
#[test]
fn the_cargo_shim_leases_cargo_test_and_passes_cargo_fmt_through() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = tempfile::tempdir().unwrap();
    let shims = dir.path().join("lease-shims");
    coder_lease::shim::install(&shims).unwrap();
    let tools = dir.path().join("tools");
    std::fs::create_dir(&tools).unwrap();
    let cargo = tools.join("cargo");
    std::fs::write(
        &cargo,
        "#!/bin/sh\necho \"$1 leases=${OPENAGENTS_LEASES:-none} target=${CARGO_TARGET_DIR:-none}\"\n",
    )
    .unwrap();
    std::fs::set_permissions(&cargo, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = coder_lease::shim::path_with(
        &shims,
        Some(std::ffi::OsStr::new(&format!(
            "{}:/usr/bin:/bin",
            tools.display()
        ))),
    );
    let run = |sub: &str| {
        let output = Command::new(shims.join("cargo"))
            .arg(sub)
            .env_clear()
            .env("PATH", &path)
            .env("HOME", dir.path())
            .env("OPENAGENTS_LEASE_ROOT", dir.path().join("leases"))
            .env("OPENAGENTS_SLOT_FREE_GB", "0")
            .env(coder_lease::shim::BIN_VAR, env!("CARGO_BIN_EXE_openagents"))
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        String::from_utf8(output.stdout).unwrap()
    };
    let test = run("test");
    assert!(test.starts_with("test leases=build target="), "{test}");
    assert!(test.contains(".openagents/targets/"), "{test}");
    assert_eq!(run("fmt").trim(), "fmt leases=none target=none");
    let receipts = std::fs::read_dir(dir.path().join("leases/receipts"))
        .unwrap()
        .count();
    assert_eq!(receipts, 1);
}
