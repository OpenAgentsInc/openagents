use super::*;

use std::process::{Command, Output};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use crate::{Broker, Holder, Limits, Request, Resource, Wait};

/// A machine in a temporary folder: a fake `cargo`, a fake `openagents`
/// that prints what the lease would have run, and the installed shim.
struct Machine {
    dir: tempfile::TempDir,
    gate: Gate,
}

fn script(path: &Path, text: &str) {
    use std::os::unix::fs::PermissionsExt as _;
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

impl Machine {
    fn new() -> Machine {
        let dir = tempfile::tempdir().unwrap();
        let root = std::fs::canonicalize(dir.path()).unwrap();
        script(
            &root.join("real/cargo"),
            "#!/bin/sh\necho \"real $* target=${CARGO_TARGET_DIR:-}\"\n",
        );
        script(
            &root.join("bin/openagents"),
            "#!/bin/sh\necho \"lease $* builds=$OPENAGENTS_BUILD_LEASES floor=$OPENAGENTS_SLOT_FREE_GB target=${CARGO_TARGET_DIR:-}\"\n",
        );
        let gate = Gate {
            shims: root.join("shims"),
            targets: root.join("targets"),
            bin: Some(root.join("bin/openagents")),
        };
        gate.install().unwrap();
        Machine { dir, gate }
    }

    fn root(&self) -> PathBuf {
        std::fs::canonicalize(self.dir.path()).unwrap()
    }

    fn cargo(&self, cwd: &Path, args: &[&str], extra: &[(&str, &str)]) -> Output {
        let path = OsString::from(format!(
            "{}:/usr/bin:/bin",
            self.root().join("real").display()
        ));
        let mut command = Command::new(self.gate.shims.join("cargo"));
        command
            .args(args)
            .current_dir(cwd)
            .env_clear()
            .env("HOME", self.root())
            .env("OPENAGENTS_LEASE_ROOT", self.root().join("leases"))
            .env(FLOOR_VAR, "0");
        for (name, value) in self.gate.vars(Some(&path)) {
            command.env(name, value);
        }
        for (name, value) in extra {
            command.env(name, value);
        }
        command.output().unwrap()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

fn git(cwd: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "init.defaultBranch=main",
        ])
        .args(args)
        .current_dir(cwd)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(status.status.success(), "git {args:?}: {status:?}");
}

#[test]
fn light_subcommands_run_cargo_directly() {
    let machine = Machine::new();
    let output = machine.cargo(&machine.root(), &["fmt", "--all"], &[]);
    assert_eq!(stdout(&output), "real fmt --all target=");
    // Inside a build lease, even a build passes straight through.
    let output = machine.cargo(
        &machine.root(),
        &["build"],
        &[("OPENAGENTS_LEASES", "build")],
    );
    assert_eq!(stdout(&output), "real build target=");
}

#[test]
fn a_build_waits_for_a_slot_under_the_coordinators_limit() {
    let machine = Machine::new();
    let ws = machine.root().join("ws");
    std::fs::create_dir_all(&ws).unwrap();
    std::fs::write(ws.join("Cargo.lock"), "").unwrap();
    let output = machine.cargo(&ws, &["test", "-p", "x"], &[]);
    let line = stdout(&output);
    assert!(
        line.starts_with(&format!(
            "lease lease build --keep-target-dir -- {} test -p x builds=5 floor=0 target={}/ws-",
            machine.root().join("real/cargo").display(),
            machine.gate.targets.display()
        )),
        "{line}"
    );
    set_limit(&machine.root().join("leases"), 2).unwrap();
    assert_eq!(limit(&machine.root().join("leases")), 2);
    let line = stdout(&machine.cargo(&ws, &["build"], &[]));
    assert!(line.contains(" builds=2 "), "{line}");
    assert!(set_limit(&machine.root().join("leases"), 0).is_err());
}

#[test]
fn every_worktree_shares_one_target_folder_per_workspace() {
    let machine = Machine::new();
    let repo = machine.root().join("repo");
    std::fs::create_dir_all(repo.join("apps/app")).unwrap();
    std::fs::write(repo.join("Cargo.lock"), "").unwrap();
    std::fs::write(repo.join("apps/app/Cargo.lock"), "").unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-qm", "init"]);
    git(&repo, &["worktree", "add", "-q", "../wt"]);
    let target = |cwd: &Path, extra: &[(&str, &str)]| {
        let output = machine.cargo(cwd, &["check"], extra);
        let line = stdout(&output);
        let target = line.rsplit("target=").next().unwrap().to_owned();
        (target, String::from_utf8_lossy(&output.stderr).into_owned())
    };
    let (main_root, _) = target(&repo, &[]);
    let (wt_root, _) = target(&machine.root().join("wt"), &[]);
    let (main_app, _) = target(&repo.join("apps/app"), &[]);
    let (wt_app, _) = target(&machine.root().join("wt/apps/app"), &[]);
    assert_eq!(main_root, wt_root);
    assert_eq!(main_app, wt_app);
    assert_ne!(main_root, main_app);
    let targets = machine.gate.targets.display().to_string();
    assert!(
        main_root.starts_with(&format!("{targets}/repo-")),
        "{main_root}"
    );
    assert!(
        main_app.starts_with(&format!("{targets}/repo-apps-app-")),
        "{main_app}"
    );
    assert!(Path::new(&main_root).is_dir());
    // A private copy is replaced by the shared folder, and the agent is told.
    let (private, note) = target(&repo, &[("CARGO_TARGET_DIR", "/elsewhere/copy")]);
    assert_eq!(private, main_root);
    assert!(note.contains("building in the shared folder"), "{note}");
    // Unless the caller asks to keep its own.
    let (kept, _) = target(
        &repo,
        &[
            ("CARGO_TARGET_DIR", "/elsewhere/copy"),
            ("OPENAGENTS_KEEP_TARGET_DIR", "1"),
        ],
    );
    assert_eq!(kept, "/elsewhere/copy");
}

#[test]
fn a_build_below_the_disk_floor_does_not_start() {
    let machine = Machine::new();
    let output = machine.cargo(&machine.root(), &["build"], &[(FLOOR_VAR, "999999999")]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty());
    let message = String::from_utf8_lossy(&output.stderr);
    assert!(
        message
            .contains("agent builds need at least 999999999 GiB. Free up space, then try again."),
        "{message}"
    );
}

#[test]
fn without_openagents_the_build_runs_and_says_so() {
    let mut machine = Machine::new();
    std::fs::remove_file(machine.root().join("bin/openagents")).unwrap();
    machine.gate.bin = None;
    let output = machine.cargo(&machine.root(), &["build"], &[]);
    assert_eq!(stdout(&output), "real build target=");
    assert!(String::from_utf8_lossy(&output.stderr).contains("without waiting for a build slot"));
}

#[test]
fn the_gate_comes_from_the_environment() {
    let env = |name: &str| (name == "HOME").then(|| OsString::from("/h"));
    let gate = Gate::from(&env).unwrap();
    assert_eq!(
        gate.shims,
        PathBuf::from("/h/.openagents/bin/agent-build-shims")
    );
    assert_eq!(gate.targets, PathBuf::from("/h/work/agent-targets"));
    assert_eq!(gate.bin, None);
    let off = |name: &str| match name {
        "HOME" => Some(OsString::from("/h")),
        GATE_VAR => Some(OsString::from("0")),
        _ => None,
    };
    assert_eq!(Gate::from(&off), None);
    let vars = gate.vars(Some(OsStr::new("/bin")));
    assert_eq!(
        vars[0],
        (
            OsString::from("PATH"),
            OsString::from("/h/.openagents/bin/agent-build-shims:/bin")
        )
    );
}

/// Eight chats ask to build under a limit of three: three run, five wait,
/// and the coordinator sees both counts and the free disk.
#[test]
fn status_counts_running_and_waiting_builds() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("leases");
    set_limit(&root, 3).unwrap();
    let limits = Limits {
        build: limit(&root),
        memory_gib: 8,
        disk_floor_gb: 0,
        build_disk_gb: 0,
    };
    let broker = Arc::new(Broker::new(root.clone(), limits).with_poll(Duration::from_millis(10)));
    let done = Arc::new(AtomicBool::new(false));
    let chats: Vec<_> = (0..8)
        .map(|chat| {
            let broker = broker.clone();
            let done = done.clone();
            std::thread::spawn(move || {
                let holder = Holder {
                    session: format!("chat:{chat}"),
                    agent: "none".into(),
                    pid: std::process::id(),
                    command: "cargo".into(),
                };
                let request =
                    Request::new(Resource::parse("build").unwrap(), holder).wait(Wait::Forever);
                let lease = broker.acquire(request).unwrap();
                while !done.load(Ordering::SeqCst) {
                    std::thread::sleep(Duration::from_millis(5));
                }
                drop(lease);
            })
        })
        .collect();
    let free = |_: &Path| Ok(40 * GIB);
    let started = Instant::now();
    let seen = loop {
        let seen = status_with(&root, dir.path(), FLOOR_GIB * GIB, &free).unwrap();
        if seen.running == 3 && seen.waiting == 5 {
            break seen;
        }
        assert!(started.elapsed() < Duration::from_secs(20), "{seen:?}");
        std::thread::sleep(Duration::from_millis(10));
    };
    assert_eq!(seen.limit, 3);
    assert_eq!(seen.holders.len(), 3);
    assert_eq!(seen.free_bytes, Some(40 * GIB));
    assert!(!seen.below_floor);
    done.store(true, Ordering::SeqCst);
    for chat in chats {
        chat.join().unwrap();
    }
    let low = |_: &Path| Ok(GIB);
    let seen = status_with(&root, dir.path(), FLOOR_GIB * GIB, &low).unwrap();
    assert_eq!((seen.running, seen.waiting), (0, 0));
    assert!(seen.below_floor);
    // A machine that never built has no lease root yet.
    let empty = status_with(&dir.path().join("absent"), dir.path(), 0, &low).unwrap();
    assert_eq!((empty.running, empty.limit), (0, DEFAULT_LIMIT));
}
