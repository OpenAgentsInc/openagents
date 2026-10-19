//! A relative local remote reaches the same repository from a task
//! worktree elsewhere (#10333).

use super::*;

fn run(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
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

#[test]
fn a_relative_remote_resolves_against_the_main_checkout() {
    let root = tempfile::tempdir().unwrap();
    let root = std::fs::canonicalize(root.path()).unwrap();
    let origin = root.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    run(&origin, &["init", "-q", "--bare", "-b", "main"]);
    let checkout = root.join("project");
    std::fs::create_dir_all(&checkout).unwrap();
    run(&checkout, &["init", "-q", "-b", "main"]);
    run(&checkout, &["config", "user.name", "Remote"]);
    run(&checkout, &["config", "user.email", "remote@example.com"]);
    run(&checkout, &["config", "commit.gpgsign", "false"]);
    std::fs::write(checkout.join("a.txt"), "a\n").unwrap();
    run(&checkout, &["add", "a.txt"]);
    run(&checkout, &["commit", "-q", "-m", "a"]);
    run(&checkout, &["remote", "add", "origin", "../origin.git"]);
    run(&checkout, &["push", "-q", "origin", "main"]);

    // A worktree two levels away, as Coder's are.
    let worktree = root.join("store").join("worktrees").join("project-1");
    run(
        &checkout,
        &[
            "worktree",
            "add",
            "--detach",
            "-q",
            worktree.to_str().unwrap(),
        ],
    );
    std::fs::write(worktree.join("b.txt"), "b\n").unwrap();
    run(&worktree, &["add", "b.txt"]);
    run(&worktree, &["commit", "-q", "-m", "b"]);

    assert_eq!(
        remote_overrides(&worktree),
        vec![(
            format!("url.{}.insteadOf", origin.display()),
            "../origin.git".to_owned()
        )]
    );
    git_out(&worktree, &["push", "-q", "origin", "HEAD:main"]).unwrap();
    assert_eq!(
        run(&origin, &["log", "-1", "--format=%s", "main"]),
        "b",
        "the push reached the checkout's origin"
    );
    let variables = remote_override_environment(&worktree, 1);
    assert_eq!(variables[0].1, "2");
    assert_eq!(variables[1].0, "GIT_CONFIG_KEY_1");
}

#[test]
fn urls_and_absolute_paths_are_left_alone() {
    assert!(relative_local("../origin.git"));
    assert!(relative_local("origin.git"));
    assert!(!relative_local("https://github.com/o/n.git"));
    assert!(!relative_local("git@github.com:o/n.git"));
    assert!(!relative_local("/srv/origin.git"));
    assert!(!relative_local(""));
}
