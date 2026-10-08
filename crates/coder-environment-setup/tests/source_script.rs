//! The source materialization script, run with the local `sh` and `git`
//! against a scratch repository.

use coder_environment::SourcePin;
use coder_environment_setup::source::{Mode, Report, command, remote_url};
use std::path::{Path, PathBuf};
use std::process::Command;

const GH_SECRET: &str = "ghp_source_secret_value_0123456789";

struct Fixture {
    _dir: tempfile::TempDir,
    home: PathBuf,
    origin: PathBuf,
    base: PathBuf,
    commits: Vec<String>,
}

fn git(home: &Path, cwd: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .output()
        .unwrap();
    assert!(out.status.success(), "{args:?}: {out:?}");
    String::from_utf8(out.stdout).unwrap().trim().to_owned()
}

fn fixture() -> Option<Fixture> {
    if Command::new("git").arg("--version").output().is_err() {
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_path_buf();
    let home = base.join("home");
    let origin = base.join("origin");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&origin).unwrap();
    git(&home, &origin, &["init", "-q", "."]);
    let mut commits = vec![];
    for (i, text) in ["one", "two"].iter().enumerate() {
        std::fs::write(origin.join("file.txt"), text).unwrap();
        std::fs::write(origin.join(".gitignore"), "target/\n").unwrap();
        git(&home, &origin, &["add", "."]);
        git(&home, &origin, &["commit", "-q", "-m", &format!("c{i}")]);
        commits.push(git(&home, &origin, &["rev-parse", "HEAD"]));
    }
    Some(Fixture {
        _dir: dir,
        home,
        origin,
        base,
        commits,
    })
}

fn pin(f: &Fixture, revision: &str) -> SourcePin {
    SourcePin {
        repository: Some(format!("file://{}", f.origin.display())),
        revision: revision.into(),
        digest: "b".repeat(64),
    }
}

/// Run the command as a provider would: `sh -c` in the checkout with its
/// extra environment and named credential values.
fn run(f: &Fixture, pin: &SourcePin, mode: Mode, cwd: &Path) -> (i32, String) {
    let (text, names, env) = command(pin, mode, Some("GH_TOKEN")).unwrap();
    let mut c = Command::new("sh");
    c.arg("-c")
        .arg(&text)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", &f.home)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .envs(env);
    for n in names {
        c.env(n, GH_SECRET);
    }
    let out = c.output().unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
    )
}

fn tree_holds(dir: &Path, needle: &str) -> bool {
    let mut stack = vec![dir.to_path_buf()];
    while let Some(p) = stack.pop() {
        for e in std::fs::read_dir(&p).unwrap() {
            let path = e.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else if std::fs::read(&path)
                .unwrap()
                .windows(needle.len())
                .any(|w| w == needle.as_bytes())
            {
                return true;
            }
        }
    }
    false
}

#[test]
fn materializes_the_exact_commit_with_no_credential_on_disk() {
    let Some(f) = fixture() else { return };
    // An older (non-tip) commit, fetched by its exact identity.
    let p = pin(&f, &f.commits[0]);
    let work = f.base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let (code, out) = run(&f, &p, Mode::Materialize, &work);
    assert_eq!(code, 0, "{out}");
    let report = Report::parse(&out).unwrap();
    assert!(report.verified(&p) && report.fetched, "{report:?}");
    assert_eq!(
        std::fs::read_to_string(work.join("file.txt")).unwrap(),
        "one"
    );
    assert_eq!(git(&f.home, &work, &["rev-parse", "HEAD"]), f.commits[0]);
    // Fetched by URL: no remote, helper, or token in any Git file.
    let config = std::fs::read_to_string(work.join(".git/config")).unwrap();
    assert!(!config.contains("remote") && !config.contains("credential"));
    assert!(!tree_holds(&work.join(".git"), GH_SECRET));

    // Rerunning is a proven no-op.
    let (code, out) = run(&f, &p, Mode::Materialize, &work);
    assert_eq!(code, 0, "{out}");
    let again = Report::parse(&out).unwrap();
    assert!(again.verified(&p) && !again.fetched);

    // Verify accepts ignored build output, refuses a tracked change.
    std::fs::create_dir_all(work.join("target")).unwrap();
    std::fs::write(work.join("target/app"), "bin").unwrap();
    let (code, out) = run(&f, &p, Mode::Verify, &work);
    assert_eq!(code, 0, "{out}");
    std::fs::write(work.join("file.txt"), "changed").unwrap();
    let (code, out) = run(&f, &p, Mode::Verify, &work);
    assert_eq!(code, 3);
    let bad = Report::parse(&out).unwrap();
    assert_eq!(bad.error.as_deref(), Some("dirty"));
    assert!(!bad.verified(&p));
}

#[test]
fn refuses_a_different_head_a_foreign_directory_and_a_missing_checkout() {
    let Some(f) = fixture() else { return };
    let p = pin(&f, &f.commits[1]);
    // A checkout at another commit fails verification without fetching.
    let work = f.base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    assert_eq!(
        run(&f, &pin(&f, &f.commits[0]), Mode::Materialize, &work).0,
        0
    );
    let (code, out) = run(&f, &p, Mode::Verify, &work);
    assert_eq!(code, 3);
    assert_eq!(Report::parse(&out).unwrap().error.as_deref(), Some("head"));
    // Materialize moves it to the pin.
    assert_eq!(run(&f, &p, Mode::Materialize, &work).0, 0);

    let foreign = f.base.join("foreign");
    std::fs::create_dir_all(&foreign).unwrap();
    std::fs::write(foreign.join("stray"), "x").unwrap();
    let (code, out) = run(&f, &p, Mode::Materialize, &foreign);
    assert_eq!(code, 3);
    assert_eq!(
        Report::parse(&out).unwrap().error.as_deref(),
        Some("not-empty")
    );

    let empty = f.base.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let (code, out) = run(&f, &p, Mode::Verify, &empty);
    assert_eq!(code, 3);
    assert_eq!(
        Report::parse(&out).unwrap().error.as_deref(),
        Some("no-checkout")
    );

    // An unknown commit cannot be fetched.
    let missing = f.base.join("missing");
    std::fs::create_dir_all(&missing).unwrap();
    let (code, out) = run(&f, &pin(&f, &"0".repeat(40)), Mode::Materialize, &missing);
    assert_eq!(code, 3);
    assert_eq!(Report::parse(&out).unwrap().error.as_deref(), Some("fetch"));
}

#[test]
fn repository_labels_resolve_without_credentials() {
    let mut p = SourcePin {
        repository: Some("OpenAgentsInc/openagents".into()),
        revision: "a".repeat(40),
        digest: "b".repeat(64),
    };
    assert_eq!(
        remote_url(&p).unwrap(),
        "https://github.com/OpenAgentsInc/openagents.git"
    );
    for bad in [
        "https://x-access-token:ghp_x@github.com/o/r.git",
        "git@github.com:o/r.git",
        "o/r; rm -rf /",
        "../o",
    ] {
        p.repository = Some(bad.into());
        assert!(remote_url(&p).is_err(), "{bad}");
    }
    p.repository = None;
    assert!(remote_url(&p).is_err());
    assert!(command(&p, Mode::Verify, None).is_ok());
}
