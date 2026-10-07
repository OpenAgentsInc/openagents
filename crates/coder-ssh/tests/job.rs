//! A remote job against the fake `ssh`: the "remote" is a temporary home on
//! this machine, the repository a bare repository in a temporary directory.
//! The real `~/.ssh`, `~/.openagents`, and Git configuration are never read
//! or written.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use coder_ssh::{Job, SETUP_FAILED, reachable};

#[path = "support/fake_ssh.rs"]
mod fake_ssh;

/// Runs `git` with a scratch home and no system configuration.
fn git(home: &Path, dir: &Path, args: &[&str]) -> String {
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
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    ssh: PathBuf,
    remote_home: PathBuf,
    origin: PathBuf,
    commit: String,
}

fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    let local_home = root.join("local-home");
    let remote_home = root.join("remote-home");
    let shim_dir = root.join("shim");
    for path in [&local_home, &remote_home, &shim_dir, &shim_dir.join("bin")] {
        std::fs::create_dir_all(path).unwrap();
    }
    let origin = root.join("origin.git");
    git(
        &local_home,
        &root,
        &["init", "--quiet", "--bare", "origin.git"],
    );
    let work = root.join("work");
    git(&local_home, &root, &["init", "--quiet", "work"]);
    std::fs::write(
        work.join("job.sh"),
        "#!/bin/sh\necho \"out $1 in $(basename \"$PWD\")\"\necho \"err $CARGO_TARGET_DIR\" >&2\nmkdir -p results\nprintf 'score=%s\\n' \"$1\" > results/score.txt\nexit 3\n",
    )
    .unwrap();
    git(&local_home, &work, &["add", "job.sh"]);
    git(&local_home, &work, &["commit", "--quiet", "-m", "job"]);
    let commit = git(&local_home, &work, &["rev-parse", "HEAD"]);
    git(
        &local_home,
        &work,
        &[
            "push",
            "--quiet",
            origin.to_str().unwrap(),
            "HEAD:refs/heads/main",
        ],
    );
    let ssh = fake_ssh::shim(&shim_dir, &remote_home, "/bin/sh");
    Fixture {
        _dir: dir,
        root,
        ssh,
        remote_home,
        origin,
        commit,
    }
}

#[test]
fn a_job_runs_at_the_commit_streams_its_output_and_fetches_results() {
    let f = fixture();
    let job = Job::new(
        "fake",
        f.origin.to_str().unwrap(),
        &f.commit,
        vec!["sh".into(), "job.sh".into(), "it's 42".into()],
    )
    .unwrap()
    .program(&f.ssh);
    let out = f.root.join("out.txt");
    let err = f.root.join("err.txt");
    let code = job
        .run(
            Stdio::from(std::fs::File::create(&out).unwrap()),
            Stdio::from(std::fs::File::create(&err).unwrap()),
        )
        .unwrap();
    let stdout = std::fs::read_to_string(&out).unwrap();
    let stderr = std::fs::read_to_string(&err).unwrap();
    assert_eq!(code, 3, "{stdout}\n{stderr}");
    assert_eq!(stdout, format!("out it's 42 in {}\n", f.commit));
    let target = f.remote_home.join(".openagents/remote-runs/origin/target");
    assert!(
        stderr.contains(&format!("err {}", target.display())),
        "{stderr}"
    );
    assert_eq!(
        job.remote_dir(),
        format!("~/.openagents/remote-runs/origin/{}", f.commit)
    );

    let local = f.root.join("fetched/score.txt");
    job.fetch("results/score.txt", &local).unwrap();
    assert_eq!(std::fs::read_to_string(&local).unwrap(), "score=it's 42\n");
    assert!(
        job.fetch("results/missing.txt", &f.root.join("m.txt"))
            .is_err()
    );
    assert!(!f.root.join("m.txt").exists());

    // A second run reuses the clone and the checkout.
    let again = job.run(Stdio::null(), Stdio::null()).unwrap();
    assert_eq!(again, 3);
}

#[test]
fn an_unpushed_commit_fails_before_the_command_runs() {
    let f = fixture();
    let job = Job::new(
        "fake",
        f.origin.to_str().unwrap(),
        &"b".repeat(40),
        vec!["sh".into(), "-c".into(), "echo ran".into()],
    )
    .unwrap()
    .program(&f.ssh);
    let out = f.root.join("out.txt");
    let err = f.root.join("err.txt");
    let code = job
        .run(
            Stdio::from(std::fs::File::create(&out).unwrap()),
            Stdio::from(std::fs::File::create(&err).unwrap()),
        )
        .unwrap();
    assert_eq!(code, SETUP_FAILED);
    assert_eq!(std::fs::read_to_string(&out).unwrap(), "");
    assert!(
        std::fs::read_to_string(&err)
            .unwrap()
            .contains("push it first"),
        "{}",
        std::fs::read_to_string(&err).unwrap()
    );
}

#[test]
fn reachability_follows_whether_ssh_answers() {
    let f = fixture();
    assert!(reachable("fake", Some(&f.ssh)));
    assert!(!reachable("fake", Some(&f.root.join("no-such-ssh"))));
    assert!(!reachable("-oProxyCommand=x", Some(&f.ssh)));
}
