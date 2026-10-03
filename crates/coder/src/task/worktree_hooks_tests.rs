use super::*;

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args([
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@t",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// A checkout at `<dir>/src` with `hooks` committed (when given) and an
/// ignored `.env`, and a linked worktree of its `HEAD` at `<dir>/wt`.
fn setup_repo(dir: &Path, hooks: Option<&str>) -> (PathBuf, PathBuf) {
    let top = dir.join("src");
    std::fs::create_dir_all(top.join(".openagents")).unwrap();
    git(&top, &["init", "-q", "-b", "main"]);
    std::fs::write(top.join("README"), "one\n").unwrap();
    std::fs::write(top.join(".gitignore"), ".env\n").unwrap();
    std::fs::write(top.join(".env"), "SECRET=1\n").unwrap();
    if let Some(hooks) = hooks {
        std::fs::write(top.join(FILE), hooks).unwrap();
    }
    git(&top, &["add", "-A"]);
    git(&top, &["commit", "-q", "-m", "one"]);
    let worktree = dir.join("wt");
    git(
        &top,
        &[
            "worktree",
            "add",
            "--detach",
            "-q",
            worktree.to_str().unwrap(),
            "HEAD",
        ],
    );
    (
        top.canonicalize().unwrap(),
        worktree.canonicalize().unwrap(),
    )
}

fn head(dir: &Path) -> String {
    git(dir, &["rev-parse", "HEAD"])
}

/// Whether this computer enforces the source guard the hooks need.
fn guarded(worktree: &Path) -> bool {
    coder_boundary::source::Guard::for_worktree(worktree)
        .ok()
        .flatten()
        .is_some_and(|guard| guard.enforceable().is_ok())
}

#[test]
fn a_repository_without_hooks_does_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (top, worktree) = setup_repo(dir.path(), None);
    assert_eq!(trusted(&top, &head(&top)), Trusted::None);
    let ran = setup(&top, &worktree, &head(&top), &dir.path().join("log")).unwrap();
    assert_eq!(ran, Ran::default());
    assert!(!dir.path().join("log").exists());
}

#[test]
fn setup_makes_the_worktree_ready_from_the_source_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let hooks = r#"{"setup": [
        "cp \"$OPENAGENTS_SOURCE_CHECKOUT/.env\" .env",
        "printf '%s %s' \"$OPENAGENTS_WORKTREE\" \"$OPENAGENTS_WORKTREE_PORT\" > ready"
    ]}"#;
    let (top, worktree) = setup_repo(dir.path(), Some(hooks));
    if !guarded(&worktree) {
        return;
    }
    let log = dir.path().join("hooks").join("t.setup.log");
    let ran = setup(&top, &worktree, &head(&top), &log).unwrap();
    assert_eq!(ran.commands, 2);
    let port = ran.port.unwrap();
    assert_eq!(
        std::fs::read_to_string(worktree.join(".env")).unwrap(),
        "SECRET=1\n"
    );
    assert_eq!(
        std::fs::read_to_string(worktree.join("ready")).unwrap(),
        format!("{} {port}", worktree.display())
    );
    // The port is the worktree's: asked again, it is the same.
    assert_eq!(super::port(&worktree).unwrap(), port);
    assert!(std::fs::read_to_string(&log).unwrap().contains("$ cp"));
}

#[test]
fn setup_never_writes_the_source_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let hooks = r#"{"setup": "echo changed > \"$OPENAGENTS_SOURCE_CHECKOUT/README\""}"#;
    let (top, worktree) = setup_repo(dir.path(), Some(hooks));
    if !guarded(&worktree) {
        return;
    }
    let error = setup(&top, &worktree, &head(&top), &dir.path().join("log")).unwrap_err();
    assert!(error.contains("worktree setup"), "{error}");
    assert_eq!(
        std::fs::read_to_string(top.join("README")).unwrap(),
        "one\n"
    );
}

#[test]
fn a_failing_command_stops_setup_and_says_which() {
    let dir = tempfile::tempdir().unwrap();
    let hooks = r#"{"setup": ["echo before; exit 3", "touch after"]}"#;
    let (top, worktree) = setup_repo(dir.path(), Some(hooks));
    if !guarded(&worktree) {
        return;
    }
    let error = setup(&top, &worktree, &head(&top), &dir.path().join("log")).unwrap_err();
    assert!(error.contains("`echo before; exit 3` exited 3"), "{error}");
    assert!(error.contains("before"), "{error}");
    assert!(!worktree.join("after").exists());
}

#[test]
fn standard_input_is_closed_so_a_prompt_cannot_wait() {
    let dir = tempfile::tempdir().unwrap();
    let hooks = r#"{"setup": "read answer || touch closed"}"#;
    let (top, worktree) = setup_repo(dir.path(), Some(hooks));
    if !guarded(&worktree) {
        return;
    }
    setup(&top, &worktree, &head(&top), &dir.path().join("log")).unwrap();
    assert!(worktree.join("closed").exists());
}

#[test]
fn a_repository_time_limit_ends_a_command_that_runs_on() {
    let dir = tempfile::tempdir().unwrap();
    let hooks = r#"{"setup": "sleep 30", "timeout_seconds": 1}"#;
    let (top, worktree) = setup_repo(dir.path(), Some(hooks));
    if !guarded(&worktree) {
        return;
    }
    let began = Instant::now();
    let error = setup(&top, &worktree, &head(&top), &dir.path().join("log")).unwrap_err();
    assert!(
        error.contains("ran past the repository's 1 s limit"),
        "{error}"
    );
    assert!(began.elapsed() < Duration::from_secs(20));
}

#[test]
fn only_the_committed_file_runs() {
    let dir = tempfile::tempdir().unwrap();
    let (top, worktree) = setup_repo(dir.path(), Some(r#"{"setup": "touch committed"}"#));
    if !guarded(&worktree) {
        return;
    }
    // An edit nobody committed never runs.
    std::fs::write(top.join(FILE), r#"{"setup": "touch edited"}"#).unwrap();
    setup(&top, &worktree, &head(&top), &dir.path().join("log")).unwrap();
    assert!(worktree.join("committed").exists());
    assert!(!worktree.join("edited").exists());
}

#[test]
fn a_commit_that_changes_the_hooks_is_not_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let (top, worktree) = setup_repo(dir.path(), Some(r#"{"setup": "touch ours"}"#));
    // The task's commit, like a pull request from a fork, changes them.
    std::fs::write(worktree.join(FILE), r#"{"setup": "touch theirs"}"#).unwrap();
    git(&worktree, &["commit", "-q", "-am", "theirs"]);
    let theirs = head(&worktree);
    let Trusted::Skipped(why) = trusted(&top, &theirs) else {
        panic!("trusted a changed file");
    };
    assert!(why.contains("different"), "{why}");
    let ran = setup(&top, &worktree, &theirs, &dir.path().join("log")).unwrap();
    assert_eq!(ran.commands, 0);
    assert!(ran.skipped.is_some());
    assert!(!worktree.join("ours").exists());
    assert!(!worktree.join("theirs").exists());
}

#[test]
fn a_commit_that_adds_hooks_the_branch_lacks_is_not_trusted() {
    let dir = tempfile::tempdir().unwrap();
    let (top, worktree) = setup_repo(dir.path(), None);
    std::fs::create_dir_all(worktree.join(".openagents")).unwrap();
    std::fs::write(worktree.join(FILE), r#"{"setup": "touch theirs"}"#).unwrap();
    git(&worktree, &["add", "-A"]);
    git(&worktree, &["commit", "-q", "-m", "theirs"]);
    assert!(matches!(
        trusted(&top, &head(&worktree)),
        Trusted::Skipped(_)
    ));
}

#[test]
fn a_file_that_is_not_hooks_is_skipped_not_run() {
    let dir = tempfile::tempdir().unwrap();
    let (top, _) = setup_repo(dir.path(), Some(r#"{"setpu": "touch x"}"#));
    let Trusted::Skipped(why) = trusted(&top, &head(&top)) else {
        panic!("ran an unknown key");
    };
    assert!(why.contains("is not worktree hooks"), "{why}");
}

#[test]
fn teardown_runs_with_the_worktrees_port_and_a_failure_keeps_it() {
    let dir = tempfile::tempdir().unwrap();
    let hooks = r#"{"setup": "true",
        "teardown": ["echo \"$OPENAGENTS_WORKTREE_PORT\" > \"$OPENAGENTS_SOURCE_CHECKOUT/../down\"", "exit 4"]}"#;
    let (top, worktree) = setup_repo(dir.path(), Some(hooks));
    if !guarded(&worktree) {
        return;
    }
    let port = setup(&top, &worktree, &head(&top), &dir.path().join("log"))
        .unwrap()
        .port
        .unwrap();
    let error = teardown(&top, &worktree, &dir.path().join("down.log")).unwrap_err();
    assert!(
        error.contains("worktree teardown `exit 4` exited 4"),
        "{error}"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("down"))
            .unwrap()
            .trim(),
        port.to_string()
    );
}

#[test]
fn a_repository_without_teardown_tears_down_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let (top, worktree) = setup_repo(dir.path(), Some(r#"{"setup": "true"}"#));
    teardown(&top, &worktree, &dir.path().join("log")).unwrap();
    assert!(!dir.path().join("log").exists());
}
