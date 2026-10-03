use super::*;
use std::path::PathBuf;

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

fn configure(dir: &Path) {
    run(dir, &["config", "user.name", "Freshener"]);
    run(dir, &["config", "user.email", "freshener@example.com"]);
    run(dir, &["config", "commit.gpgsign", "false"]);
}

fn commit(dir: &Path, path: &str, text: &str) -> String {
    std::fs::write(dir.join(path), text).unwrap();
    run(dir, &["add", path]);
    run(dir, &["commit", "-q", "-m", path]);
    run(dir, &["rev-parse", "HEAD"])
}

/// A bare `origin` on `main` with one commit, a clone `mine` tracking it,
/// and another clone `theirs` that pushes ahead of `mine`.
fn repos(root: &Path) -> (PathBuf, PathBuf, PathBuf) {
    let origin = root.join("origin.git");
    std::fs::create_dir_all(&origin).unwrap();
    run(&origin, &["init", "-q", "--bare", "-b", "main"]);
    let seed = root.join("seed");
    std::fs::create_dir_all(&seed).unwrap();
    run(&seed, &["init", "-q", "-b", "main"]);
    configure(&seed);
    commit(&seed, "one.txt", "one\n");
    run(
        &seed,
        &["remote", "add", "origin", &origin.display().to_string()],
    );
    run(&seed, &["push", "-q", "origin", "main"]);
    let clone = |name: &str| {
        let dir = root.join(name);
        run(
            root,
            &[
                "clone",
                "-q",
                &origin.display().to_string(),
                &dir.display().to_string(),
            ],
        );
        configure(&dir);
        dir
    };
    let mine = clone("mine");
    let theirs = clone("theirs");
    (origin, mine, theirs)
}

fn head(dir: &Path) -> String {
    run(dir, &["rev-parse", "HEAD"])
}

fn fetch_specs(dir: &Path) -> String {
    run(dir, &["config", "--get-all", "remote.origin.fetch"])
}

#[test]
fn a_branch_behind_its_remote_starts_at_the_fetched_tip() {
    let root = tempfile::tempdir().unwrap();
    let (_, mine, theirs) = repos(root.path());
    let before = fetch_specs(&mine);
    let pushed = commit(&theirs, "two.txt", "two\n");
    run(&theirs, &["push", "-q", "origin", "main"]);
    let fresh = base(&mine, &head(&mine), TIMEOUT);
    assert_eq!(
        fresh,
        Fresh::Tracked {
            upstream: "refs/remotes/origin/main".into(),
            fetched: true,
            base: pushed.clone(),
        }
    );
    assert_eq!(
        run(&mine, &["rev-parse", "refs/remotes/origin/main"]),
        pushed
    );
    // The local branch itself is untouched, and no refspec was added.
    assert_ne!(head(&mine), pushed);
    assert_eq!(fetch_specs(&mine), before);
}

#[test]
fn local_commits_keep_the_checkouts_head() {
    let root = tempfile::tempdir().unwrap();
    let (_, mine, theirs) = repos(root.path());
    commit(&theirs, "two.txt", "two\n");
    run(&theirs, &["push", "-q", "origin", "main"]);
    let own = commit(&mine, "mine.txt", "mine\n");
    let fresh = base(&mine, &own, TIMEOUT);
    assert!(matches!(
        fresh,
        Fresh::Tracked { fetched: true, ref base, .. } if *base == own
    ));
}

#[test]
fn an_unreachable_remote_falls_back_to_the_cached_ref() {
    let root = tempfile::tempdir().unwrap();
    let (origin, mine, _) = repos(root.path());
    std::fs::remove_dir_all(&origin).unwrap();
    let at = head(&mine);
    let fresh = base(&mine, &at, TIMEOUT);
    assert_eq!(
        fresh,
        Fresh::Tracked {
            upstream: "refs/remotes/origin/main".into(),
            fetched: false,
            base: at,
        }
    );
}

#[test]
fn a_held_fetch_lock_gives_up_within_the_timeout() {
    let root = tempfile::tempdir().unwrap();
    let (_, mine, theirs) = repos(root.path());
    commit(&theirs, "two.txt", "two\n");
    run(&theirs, &["push", "-q", "origin", "main"]);
    let lock = landing::fetch_lock(&mine).unwrap();
    lock.lock().unwrap();
    let at = head(&mine);
    let started = Instant::now();
    let fresh = base(&mine, &at, Duration::from_millis(300));
    assert!(started.elapsed() < Duration::from_secs(5));
    assert!(matches!(
        fresh,
        Fresh::Tracked { fetched: false, ref base, .. } if *base == at
    ));
    lock.unlock().unwrap();
}

#[test]
fn a_detached_head_or_untracked_branch_fetches_nothing() {
    let root = tempfile::tempdir().unwrap();
    let (_, mine, _) = repos(root.path());
    let at = head(&mine);
    run(&mine, &["checkout", "-q", "--detach"]);
    assert_eq!(base(&mine, &at, TIMEOUT), Fresh::Untracked);
    run(&mine, &["checkout", "-q", "-b", "local-only"]);
    assert_eq!(base(&mine, &at, TIMEOUT), Fresh::Untracked);
}
