//! Exercise #12104 with a real Cargo build and its actual profile lock.
#![cfg(unix)]

use std::fs::{self, File, FileTimes};
use std::os::unix::io::AsRawFd as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime};

use coder_lease::agent_builds::sweep::{self, Host, Layout, Policy, Proc};

struct Machine {
    target: PathBuf,
    pid: u32,
}

impl Host for Machine {
    fn processes(&self) -> Result<Vec<Proc>, String> {
        Ok(vec![Proc {
            pid: self.pid,
            name: "cargo".into(),
            target_dir: Some(self.target.clone()),
            ..Proc::default()
        }])
    }
    fn free(&self, _: &Path) -> std::io::Result<u64> {
        Ok(u64::MAX / 2)
    }
    fn now(&self) -> SystemTime {
        SystemTime::now()
    }
    fn stamp(&self, _: SystemTime) -> String {
        "test".into()
    }
}

struct Build(Child);
impl Drop for Build {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn write(path: &Path, text: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn command(repo: &Path, home: &Path, target: &Path) -> Command {
    let cargo = Path::new(env!("CARGO"));
    let mut command = Command::new(cargo);
    command
        .current_dir(repo)
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap())
        .env("HOME", home)
        .env("CARGO_HOME", home.join("cargo-home"))
        .env("RUSTC", cargo.parent().unwrap().join("rustc"))
        .env("CARGO_TARGET_DIR", target)
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_HTTP_TIMEOUT", "20")
        .args(["build", "--offline", "-v"]);
    command
}

fn age_tree(path: &Path) {
    let meta = fs::symlink_metadata(path).unwrap();
    if meta.is_dir() {
        for entry in fs::read_dir(path).unwrap() {
            age_tree(&entry.unwrap().path());
        }
    } else if meta.is_file() {
        let at = SystemTime::now() - Duration::from_secs(3600);
        File::open(path)
            .unwrap()
            .set_times(FileTimes::new().set_accessed(at).set_modified(at))
            .unwrap();
    }
}

fn executable(profile: &Path) -> PathBuf {
    fs::read_dir(profile.join("deps"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| {
            p.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("sweep_probe-")
                && !p.file_name().unwrap().to_string_lossy().contains('.')
        })
        .unwrap()
}

#[test]
#[ignore = "requires real Cargo and OPENAGENTS_SCRATCH; runs offline"]
fn a_long_build_succeeds_while_idle_artifacts_are_trimmed() {
    let scratch = std::env::var_os("OPENAGENTS_SCRATCH").expect("set OPENAGENTS_SCRATCH");
    let scratch = tempfile::Builder::new()
        .prefix("sweep-build-")
        .tempdir_in(scratch)
        .unwrap();
    let root = fs::canonicalize(scratch.path()).unwrap();
    let home = root.join("home");
    let repo = root.join("repo");
    let target = home.join("work/openagents-target-1");
    write(
        &repo.join("Cargo.toml"),
        "[workspace]\n[package]\nname=\"sweep-probe\"\nversion=\"0.1.0\"\nedition=\"2024\"\n[dependencies]\nsweep-dependency={path=\"dependency\"}\n",
    );
    write(
        &repo.join("dependency/Cargo.toml"),
        "[package]\nname=\"sweep-dependency\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
    );
    write(
        &repo.join("dependency/src/lib.rs"),
        "pub fn value() -> u32 { 12104 }\n",
    );
    write(
        &repo.join("src/main.rs"),
        "fn main() { println!(\"{}\", sweep_dependency::value()); }\n",
    );
    write(
        &repo.join("build.rs"),
        r#"fn main() {
        if let Ok(barrier) = std::env::var("SWEEP_BARRIER") {
            let barrier = std::path::Path::new(&barrier);
            std::fs::write(barrier.join("ready"), "ready").unwrap();
            let until = std::time::Instant::now() + std::time::Duration::from_secs(45);
            while !barrier.join("continue").exists() {
                assert!(std::time::Instant::now() < until, "sweep test timed out");
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        }
    }"#,
    );
    let log = File::create(root.join("cargo.log")).unwrap();
    for release in [false, true] {
        let mut cmd = command(&repo, &home, &target);
        cmd.env("RUSTFLAGS", "-Copt-level=1")
            .stdout(Stdio::null())
            .stderr(log.try_clone().unwrap());
        if release {
            cmd.arg("--release");
        }
        assert!(
            cmd.status().unwrap().success(),
            "{}",
            fs::read_to_string(root.join("cargo.log")).unwrap()
        );
    }
    let old_debug = executable(&target.join("debug"));
    let old_release = executable(&target.join("release"));
    age_tree(&target);
    write(
        &home.join(".openagents/build-sweep.state.json"),
        "{\"since\":0,\"used\":{}}",
    );
    let mut build = Build(
        command(&repo, &home, &target)
            .env("RUSTFLAGS", "-Copt-level=0")
            .env("SWEEP_BARRIER", &root)
            .stdout(Stdio::null())
            .stderr(log.try_clone().unwrap())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(30);
    while !root.join("ready").exists() {
        assert!(
            Instant::now() < deadline && build.0.try_wait().unwrap().is_none(),
            "{}",
            fs::read_to_string(root.join("cargo.log")).unwrap()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    let cargo_lock = File::open(target.join("debug/.cargo-lock")).unwrap();
    // SAFETY: the descriptor belongs to this test; failure proves Cargo holds it.
    assert_ne!(
        unsafe { libc::flock(cargo_lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    let machine = Machine {
        target: target.clone(),
        pid: build.0.id(),
    };
    let layout = Layout::at(&home, &home.join(".openagents/leases"));
    let policy = Policy {
        cap: 1,
        floor: 0,
        critical: 0,
        idle: Duration::from_secs(1800),
        lock_wait: Duration::ZERO,
        ..Policy::default()
    };
    let (send, receive) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        sweep::sweep_waiting(&layout, &policy, &machine, false, |report| {
            send.send(report.clone()).unwrap()
        })
    });
    let first = receive.recv_timeout(Duration::from_secs(15)).unwrap();
    assert!(
        !old_release.exists(),
        "idle release artifacts must go during the debug build: {first:?}"
    );
    assert!(
        old_debug.exists(),
        "the active profile must retain pending units"
    );
    assert!(
        build.0.try_wait().unwrap().is_none(),
        "the long build still holds Cargo's lock"
    );
    fs::write(root.join("continue"), "continue").unwrap();
    assert!(
        build.0.wait().unwrap().success(),
        "{}",
        fs::read_to_string(root.join("cargo.log")).unwrap()
    );
    let next = receive.recv_timeout(Duration::from_secs(15)).unwrap();
    worker.join().unwrap();
    assert!(
        !old_debug.exists(),
        "queued cleanup must take the next handoff: {next:?}"
    );
    let output = Command::new(target.join("debug/sweep-probe"))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "12104");
    assert!(
        next.actions
            .iter()
            .any(|action| action.verb == "lock handoff")
    );
}
