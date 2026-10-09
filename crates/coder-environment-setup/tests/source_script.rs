//! The source materialization script, run with the local `sh` and `git`
//! against a scratch repository.

use coder_environment::SourcePin;
use coder_environment_setup::source::{
    Bounds, Lfs, Mode, Report, Submodules, TIMED_OUT, command, command_with, remote_url,
};
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

fn real_git() -> PathBuf {
    let out = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    PathBuf::from(String::from_utf8(out.stdout).unwrap().trim())
}

/// A `PATH` holding only `git` (or a stand-in for it) and the system
/// tools, so no `git-lfs` from the developer's machine is visible.
fn bare_path(dir: &Path, git_script: Option<&str>) -> String {
    std::fs::create_dir_all(dir).unwrap();
    let git = dir.join("git");
    match git_script {
        Some(text) => {
            std::fs::write(&git, text).unwrap();
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&git, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        None => std::os::unix::fs::symlink(real_git(), &git).unwrap(),
    }
    format!("{}:/usr/bin:/bin", dir.display())
}

fn run_with(f: &Fixture, pin: &SourcePin, cwd: &Path, path: &str, bounds: Bounds) -> (i32, String) {
    let (text, names, env) =
        command_with(pin, Mode::Materialize, Some("GH_TOKEN"), bounds).unwrap();
    let mut c = Command::new("/bin/sh");
    c.arg("-c")
        .arg(&text)
        .current_dir(cwd)
        .env_clear()
        .env("PATH", path)
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

#[test]
fn submodules_are_initialized_at_their_recorded_commits() {
    let Some(f) = fixture() else { return };
    // Local submodule URLs are file transport, which Git allows only when
    // asked; a GitHub submodule needs no such setting.
    std::fs::write(
        f.home.join(".gitconfig"),
        "[protocol \"file\"]\n\tallow = always\n",
    )
    .unwrap();
    let sub = f.base.join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    git(&f.home, &sub, &["init", "-q", "."]);
    std::fs::write(sub.join("lib.txt"), "library").unwrap();
    git(&f.home, &sub, &["add", "."]);
    git(&f.home, &sub, &["commit", "-q", "-m", "lib"]);
    let url = format!("file://{}", sub.display());
    git(
        &f.home,
        &f.origin,
        &["submodule", "add", "-q", &url, "vendor/lib"],
    );
    git(
        &f.home,
        &f.origin,
        &["commit", "-q", "-m", "with submodule"],
    );
    let head = git(&f.home, &f.origin, &["rev-parse", "HEAD"]);
    let p = pin(&f, &head);

    let work = f.base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let path = bare_path(&f.base.join("bin"), None);
    let (code, out) = run_with(&f, &p, &work, &path, Bounds::default());
    assert_eq!(code, 0, "{out}");
    let report = Report::parse(&out).unwrap();
    assert_eq!(report.submodules, Submodules::Yes, "{out}");
    assert!(report.verified(&p), "{report:?}");
    assert_eq!(
        std::fs::read_to_string(work.join("vendor/lib/lib.txt")).unwrap(),
        "library"
    );
    // Verify proves the same checkout, and refuses an uninitialized one.
    let (code, out) = run(&f, &p, Mode::Verify, &work);
    assert_eq!(code, 0, "{out}");
    assert_eq!(Report::parse(&out).unwrap().submodules, Submodules::Yes);
    git(
        &f.home,
        &work,
        &["submodule", "deinit", "-q", "--all", "-f"],
    );
    let (code, out) = run(&f, &p, Mode::Verify, &work);
    assert_eq!(code, 3, "{out}");
    let bad = Report::parse(&out).unwrap();
    assert_eq!(bad.error.as_deref(), Some("submodules"));
    assert_eq!(bad.submodules, Submodules::Failed);
}

#[test]
fn lfs_files_without_git_lfs_are_reported_as_pointers() {
    let Some(f) = fixture() else { return };
    std::fs::write(
        f.origin.join(".gitattributes"),
        "*.bin filter=lfs diff=lfs merge=lfs -text\n",
    )
    .unwrap();
    let pointer = "version https://git-lfs.github.com/spec/v1\n\
                   oid sha256:4d7a214614ab2935c943f9e0ff69d22eadbb8f32b1258daaa5e2ca24d17e2393\n\
                   size 12345\n";
    std::fs::write(f.origin.join("model.bin"), pointer).unwrap();
    git(&f.home, &f.origin, &["add", "."]);
    git(&f.home, &f.origin, &["commit", "-q", "-m", "lfs"]);
    let head = git(&f.home, &f.origin, &["rev-parse", "HEAD"]);
    let p = pin(&f, &head);
    let work = f.base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    let path = bare_path(&f.base.join("bin"), None);
    let (code, out) = run_with(&f, &p, &work, &path, Bounds::default());
    assert_eq!(code, 0, "{out}");
    let report = Report::parse(&out).unwrap();
    assert_eq!(report.lfs, Lfs::Pointers, "{out}");
    assert!(report.verified(&p));
    assert_eq!(
        std::fs::read_to_string(work.join("model.bin")).unwrap(),
        pointer
    );
    // A repository without LFS says so.
    let plain = f.base.join("plain");
    std::fs::create_dir_all(&plain).unwrap();
    let (code, out) = run_with(
        &f,
        &pin(&f, &f.commits[1]),
        &plain,
        &path,
        Bounds::default(),
    );
    assert_eq!(code, 0, "{out}");
    assert_eq!(Report::parse(&out).unwrap().lfs, Lfs::None);
}

#[test]
fn a_hung_fetch_is_reported_as_timed_out_within_its_bound() {
    let Some(f) = fixture() else { return };
    let p = pin(&f, &f.commits[1]);
    let work = f.base.join("work");
    std::fs::create_dir_all(&work).unwrap();
    // A stand-in `git` whose fetch never answers.
    let script = format!(
        "#!/bin/sh\nif [ \"$1\" = fetch ]; then exec sleep 60; fi\nexec '{}' \"$@\"\n",
        real_git().display()
    );
    let path = bare_path(&f.base.join("bin"), Some(&script));
    let started = std::time::Instant::now();
    let bounds = Bounds {
        clone_seconds: 1,
        fetch_seconds: 1,
    };
    let (code, out) = run_with(&f, &p, &work, &path, bounds);
    let took = started.elapsed();
    assert_eq!(code, 3, "{out}");
    let report = Report::parse(&out).unwrap();
    assert!(report.timed_out(), "{out}");
    assert_eq!(report.error.as_deref(), Some(TIMED_OUT));
    // Two bounded attempts and the pause between them, never the hang.
    assert!(took < std::time::Duration::from_secs(20), "{took:?}");
    // A fetch that fails outright is still a failed fetch.
    let missing = f.base.join("missing");
    std::fs::create_dir_all(&missing).unwrap();
    let real = bare_path(&f.base.join("realbin"), None);
    let (code, out) = run_with(&f, &pin(&f, &"0".repeat(40)), &missing, &real, bounds);
    assert_eq!(code, 3);
    assert_eq!(Report::parse(&out).unwrap().error.as_deref(), Some("fetch"));
}
