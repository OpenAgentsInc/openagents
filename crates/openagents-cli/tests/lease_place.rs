//! `openagents lease run --class` end to end (#10767), under a temporary
//! HOME and lease root. A stand-in `ssh` first on `PATH` runs the "remote"
//! command in a local shell with its own temporary home, and refuses the
//! computer `downbox`; no real computer is reached.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use serde_json::Value;

struct Fixture {
    _dir: tempfile::TempDir,
    home: PathBuf,
    bin: PathBuf,
    remote_home: PathBuf,
    work: PathBuf,
}

fn git(home: &Path, dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args(["-c", "user.name=test", "-c", "user.email=test@example.test"])
        .args(args)
        .current_dir(dir)
        .env("HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn fixture(computers: &[&str]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let home = root.join("home");
    let remote_home = root.join("remote-home");
    let bin = root.join("bin");
    for path in [&home, &remote_home, &bin] {
        std::fs::create_dir_all(path).unwrap();
    }
    let ssh = bin.join("ssh");
    std::fs::write(
        &ssh,
        format!(
            r#"#!/bin/sh
while [ "$#" -gt 0 ]; do
  case "$1" in
    -o) shift 2 ;;
    --) shift; break ;;
    -*) shift ;;
    *) break ;;
  esac
done
dest=$1; shift
printf '%s\n' "$dest" >> '{calls}'
if [ "$dest" = downbox ]; then echo 'ssh: connect to host downbox: Connection refused' >&2; exit 255; fi
exec env HOME='{remote}' sh -c "$*"
"#,
            calls = root.join("ssh-calls").display(),
            remote = remote_home.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&ssh, std::fs::Permissions::from_mode(0o755)).unwrap();

    git(&home, &root, &["init", "--quiet", "--bare", "origin.git"]);
    git(&home, &root, &["init", "--quiet", "work"]);
    let work = root.join("work");
    std::fs::write(
        work.join("job.sh"),
        "#!/bin/sh\necho \"ran $1 under ${OPENAGENTS_LEASES:-no lease} placed ${OPENAGENTS_PLACEMENT:-here}\"\nmkdir -p results\nprintf 'score=%s\\n' \"$1\" > results/score.txt\nexit 3\n",
    )
    .unwrap();
    git(&home, &work, &["add", "job.sh"]);
    git(&home, &work, &["commit", "--quiet", "-m", "job"]);
    git(
        &home,
        &work,
        &[
            "remote",
            "add",
            "origin",
            root.join("origin.git").to_str().unwrap(),
        ],
    );
    git(
        &home,
        &work,
        &["push", "--quiet", "-u", "origin", "HEAD:main"],
    );

    let settings = serde_json::json!({
        "schema": "openagents.settings.v1",
        "coder": { "placement": { "computers": computers } },
    });
    std::fs::create_dir_all(home.join(".openagents")).unwrap();
    std::fs::write(
        home.join(".openagents/settings.json"),
        serde_json::to_vec(&settings).unwrap(),
    )
    .unwrap();
    Fixture {
        _dir: dir,
        home,
        bin,
        remote_home,
        work,
    }
}

impl Fixture {
    fn openagents(&self, args: &[&str]) -> Output {
        let path = format!(
            "{}:{}",
            self.bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(args)
            .current_dir(&self.work)
            .env_clear()
            .env("PATH", path)
            .env("HOME", &self.home)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("OPENAGENTS_LEASE_ROOT", self.home.join("leases"))
            .env("OPENAGENTS_SESSION", "place-test")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    fn calls(&self) -> String {
        std::fs::read_to_string(self.home.parent().unwrap().join("ssh-calls")).unwrap_or_default()
    }
}

/// The receipt `--json` prints after the command's own output.
fn receipt(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    serde_json::from_str(stdout.lines().last().unwrap()).unwrap()
}

#[test]
fn a_benchmark_runs_on_the_computer_that_answers_and_brings_its_results_back() {
    let f = fixture(&["fakebox"]);
    let copy = f.home.join("placed.json");
    let output = f.openagents(&[
        "--json",
        "lease",
        "run",
        "--class",
        "bench",
        "--fetch",
        "results/score.txt",
        "--receipt",
        copy.to_str().unwrap(),
        "--",
        "sh",
        "job.sh",
        "7",
    ]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("ran 7 under no lease placed remote\n"),
        "{stdout}"
    );
    let placed = receipt(&output);
    assert_eq!(placed["schema"], "openagents.lease.placement-receipt.v1");
    assert_eq!(placed["class"], "bench");
    assert_eq!(placed["asked"], "auto");
    assert_eq!(placed["place"], "remote");
    assert_eq!(placed["computer"], "fakebox");
    assert_eq!(placed["exit"], 3);
    assert_eq!(placed["leases"], Value::Array(Vec::new()));
    assert_eq!(placed["commit"].as_str().unwrap().len(), 40);
    let fetched = f.work.join("results/score.txt");
    assert_eq!(std::fs::read_to_string(&fetched).unwrap(), "score=7\n");
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(&copy).unwrap()).unwrap(),
        placed
    );
    assert!(
        f.remote_home
            .join(".openagents/remote-runs/origin")
            .join(placed["commit"].as_str().unwrap())
            .join("results/score.txt")
            .exists()
    );
    let kept = std::fs::read_dir(f.home.join("leases/placements"))
        .unwrap()
        .count();
    assert_eq!(kept, 1);
}

#[test]
fn auto_runs_here_under_quiet_when_no_computer_answers() {
    let f = fixture(&["downbox"]);
    let output = f.openagents(&[
        "--json",
        "lease",
        "run",
        "--class",
        "release-gate",
        "--",
        "sh",
        "job.sh",
        "1",
    ]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("ran 1 under quiet placed here\n"),
        "{stdout}"
    );
    let placed = receipt(&output);
    assert_eq!(placed["place"], "local");
    assert_eq!(placed["leases"][0]["resource"], "quiet");
    assert!(
        placed["reason"]
            .as_str()
            .unwrap()
            .contains("no configured computer answers"),
        "{placed}"
    );
    assert_eq!(f.calls(), "downbox\n");
}

#[test]
fn a_soak_stays_here_without_asking_any_computer_unless_told_otherwise() {
    let f = fixture(&["fakebox"]);
    let output = f.openagents(&[
        "--json", "lease", "quiet", "--class", "soak", "--", "sh", "job.sh", "2",
    ]);
    assert_eq!(output.status.code(), Some(3), "{output:?}");
    assert_eq!(receipt(&output)["place"], "local");
    assert_eq!(f.calls(), "");

    // An explicit remote that doesn't answer is refused, never run here.
    let output = f.openagents(&[
        "lease",
        "quiet",
        "--class",
        "soak",
        "--place",
        "remote:downbox",
        "--",
        "sh",
        "job.sh",
        "3",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--place local"),
        "{output:?}"
    );
}

#[test]
fn a_remote_run_refuses_an_unpushed_commit() {
    let f = fixture(&["fakebox"]);
    git(
        &f.home,
        &f.work,
        &["commit", "--quiet", "--allow-empty", "-m", "local only"],
    );
    let output = f.openagents(&[
        "lease", "run", "--class", "bench", "--", "sh", "job.sh", "4",
    ]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("is not pushed"),
        "{output:?}"
    );
}
