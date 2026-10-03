//! The `fix-git` case from #10247: a full-access command in Coder's
//! worktree `cd`s into the person's checkout and commits and merges into
//! its `master`. Under a [`source::Guard`] every write there fails and
//! the checkout's branch, `HEAD`, index, and files are as they were,
//! while the same command commits in its own worktree and writes
//! anywhere else as full access always could.
#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use coder_boundary::{privacy, source};
use tempfile::TempDir;

fn git(directory: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
        ])
        .args(args)
        .current_dir(directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A checkout on `master` with one commit, and a detached worktree of it
/// beside the checkout, as Coder makes one.
fn checkout_and_worktree() -> (TempDir, PathBuf, PathBuf) {
    let root = TempDir::new().unwrap();
    let base = root.path().canonicalize().unwrap();
    let checkout = base.join("checkout");
    let worktree = base.join("worktrees/run");
    std::fs::create_dir_all(&checkout).unwrap();
    std::fs::create_dir_all(worktree.parent().unwrap()).unwrap();
    git(&checkout, &["init", "-q", "-b", "master"]);
    std::fs::write(checkout.join("site.md"), "old\n").unwrap();
    git(&checkout, &["add", "site.md"]);
    git(&checkout, &["commit", "-qm", "first"]);
    git(
        &checkout,
        &[
            "worktree",
            "add",
            "--detach",
            "-q",
            worktree.to_str().unwrap(),
        ],
    );
    (root, checkout, worktree)
}

fn guarded(guard: &source::Guard, cwd: &Path, script: &str) -> Output {
    let mut command = guard.command("/bin/sh", &[guard.worktree()]);
    command
        .args(["-c", script])
        .current_dir(cwd)
        .envs(guard.environment())
        .env("GIT_AUTHOR_NAME", "Engine")
        .env("GIT_AUTHOR_EMAIL", "engine@example.invalid")
        .env("GIT_COMMITTER_NAME", "Engine")
        .env("GIT_COMMITTER_EMAIL", "engine@example.invalid");
    command.output().unwrap()
}

/// Whether this process can apply a guard at all: on macOS, not when it
/// already runs in a sandbox (a profile can't apply inside one).
fn applies(guard: &source::Guard) -> bool {
    if cfg!(target_os = "macos")
        && (privacy::sandboxed() || !Path::new("/usr/bin/sandbox-exec").is_file())
    {
        return false;
    }
    guard.enforceable().unwrap();
    true
}

#[test]
fn an_engine_that_cds_into_the_checkout_cannot_commit_or_merge_there() {
    let (_root, checkout, worktree) = checkout_and_worktree();
    let guard = source::Guard::for_worktree(&worktree).unwrap().unwrap();
    if !applies(&guard) {
        return;
    }
    let master = git(&checkout, &["rev-parse", "master"]);
    let index = std::fs::read(checkout.join(".git/index")).unwrap();

    // The fix-git run: work in the worktree, commit there, then go to the
    // checkout and merge the commit into master.
    let fixed = guarded(
        &guard,
        &worktree,
        "printf 'fixed\\n' > site.md && git commit -qam fix && git rev-parse HEAD",
    );
    assert!(
        fixed.status.success(),
        "a commit in the worktree works: {}",
        String::from_utf8_lossy(&fixed.stderr)
    );
    let commit = String::from_utf8_lossy(&fixed.stdout).trim().to_string();
    assert_eq!(git(&worktree, &["rev-parse", "HEAD"]), commit);

    for script in [
        format!(
            "cd '{}' && git merge -q --ff-only {commit}",
            checkout.display()
        ),
        format!(
            "cd '{}' && git commit -q --allow-empty -m escape",
            checkout.display()
        ),
        format!("git -C '{}' branch -f master {commit}", checkout.display()),
        format!(
            "git -C '{}' checkout -q --detach {commit}",
            checkout.display()
        ),
        format!("printf escape > '{}'", checkout.join("site.md").display()),
        format!("touch '{}'", checkout.join("new-file").display()),
        format!("touch '{}'", checkout.join(".git/refs/heads/new").display()),
        format!("printf x >> '{}'", checkout.join(".git/config").display()),
    ] {
        let escaped = guarded(&guard, &worktree, &script);
        assert!(
            !escaped.status.success(),
            "{script} must fail: {}",
            String::from_utf8_lossy(&escaped.stdout)
        );
    }
    assert_eq!(git(&checkout, &["rev-parse", "master"]), master);
    assert_eq!(
        git(&checkout, &["symbolic-ref", "HEAD"]),
        "refs/heads/master"
    );
    assert_eq!(std::fs::read(checkout.join(".git/index")).unwrap(), index);
    assert_eq!(
        std::fs::read_to_string(checkout.join("site.md")).unwrap(),
        "old\n"
    );
    assert!(!checkout.join("new-file").exists());
    assert!(!checkout.join(".git/refs/heads/new").exists());
    assert_eq!(git(&checkout, &["status", "--porcelain"]), "");

    // Everything outside the checkout is as writable as full access
    // always made it.
    let elsewhere = TempDir::new().unwrap();
    let outside = elsewhere.path().join("outside");
    let wrote = guarded(
        &guard,
        &worktree,
        &format!("printf written > '{}'", outside.display()),
    );
    assert!(wrote.status.success());
    assert_eq!(std::fs::read(&outside).unwrap(), b"written");
}

#[test]
fn a_checkout_of_its_own_has_nothing_to_guard() {
    let (_root, checkout, worktree) = checkout_and_worktree();
    assert_eq!(source::Guard::for_worktree(&checkout).unwrap(), None);
    let guard = source::Guard::for_worktree(&worktree).unwrap().unwrap();
    let protected: Vec<&Path> = guard.protected().collect();
    assert_eq!(protected, [checkout.as_path(), &checkout.join(".git")]);
    let allowed: Vec<&Path> = guard.allowed().collect();
    assert_eq!(
        allowed,
        [
            &checkout.join(".git/objects"),
            &checkout.join(".git/worktrees/run")
        ]
    );
}
