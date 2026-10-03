//! One way to fetch for every Coder process sharing a repository.
//!
//! Git refuses a fetch when another fetch holds the same ref's lock
//! (`cannot lock ref 'refs/remotes/origin/main'`). Coder's own fetches take
//! one file lock in the common Git directory, so linked worktrees and
//! separate processes wait their turn instead of failing. A person's
//! `git pull` or another tool doesn't take that lock, so a fetch that still
//! loses a ref lock waits briefly and tries again. The measurements behind
//! this are in `docs/worktrees/2026-10-03-git-lock-contention.md`.

use std::path::Path;
use std::time::Duration;

/// Tries after the first, for a fetch that loses a ref lock to a process
/// outside Coder.
const RETRIES: u32 = 4;

/// Fetches `refspecs` from `origin` in `dir` under the repository's fetch
/// lock. `git` runs one Git command in `dir` and returns its standard
/// output, or its standard error on failure; callers pass their own runner
/// so their paths and environment stay as they were.
pub fn fetch(
    dir: &Path,
    refspecs: &[&str],
    git: impl Fn(&Path, &[&str]) -> Result<String, String>,
) -> Result<(), String> {
    let lock = lock_file(dir, &git)?;
    lock.lock()
        .map_err(|error| format!("cannot lock the repository for fetch: {error}"))?;
    let mut args = vec!["fetch", "-q", "origin"];
    args.extend_from_slice(refspecs);
    let mut result = git(dir, &args).map(|_| ());
    let mut retry = 0;
    while let Err(why) = &result {
        if retry == RETRIES || !lost_a_ref_lock(why) {
            break;
        }
        std::thread::sleep(backoff(retry));
        retry += 1;
        result = git(dir, &args).map(|_| ());
    }
    // Another thread may fork while the lock is held. Unlock explicitly
    // so its child cannot retain the lock until it closes inherited files.
    let unlocked = lock
        .unlock()
        .map_err(|error| format!("cannot release the repository's fetch lock: {error}"));
    result.and(unlocked)
}

/// The repository's fetch lock: one file in the common Git directory, so
/// every linked worktree of a clone locks the same file and other clones
/// don't.
pub fn lock_file(
    dir: &Path,
    git: impl Fn(&Path, &[&str]) -> Result<String, String>,
) -> Result<std::fs::File, String> {
    let common = git(
        dir,
        &["rev-parse", "--path-format=absolute", "--git-common-dir"],
    )?;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(Path::new(common.trim()).join("openagents-fetch.lock"))
        .map_err(|error| format!("cannot open the repository's fetch lock: {error}"))
}

/// Whether Git refused because another process held a ref's lock file.
pub fn lost_a_ref_lock(stderr: &str) -> bool {
    stderr.contains("cannot lock ref")
        || (stderr.contains("couldn't write") && stderr.contains(".lock"))
        || (stderr.contains("Unable to create") && stderr.contains(".lock': File exists"))
}

/// 100, 200, 400, 800 ms, each with up to half again added so contenders
/// don't retry in step.
fn backoff(retry: u32) -> Duration {
    let base = 100u64 << retry;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|now| now.subsec_nanos() as u64)
        .unwrap_or(0);
    Duration::from_millis(base + nanos % (base / 2 + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::Barrier;

    fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .map_err(|error| error.to_string())?;
        if output.status.success() {
            Ok(String::from_utf8_lossy(&output.stdout).into_owned())
        } else {
            Err(String::from_utf8_lossy(&output.stderr).into_owned())
        }
    }

    fn must(dir: &Path, args: &[&str]) -> String {
        git(dir, args).unwrap_or_else(|why| panic!("git {args:?}: {why}"))
    }

    /// A bare origin, a seed that advances it, and a clone with `linked`
    /// extra worktrees.
    fn repository(root: &Path, linked: usize) -> (PathBuf, PathBuf, Vec<PathBuf>) {
        let origin = root.join("origin.git");
        must(
            root,
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                origin.to_str().unwrap(),
            ],
        );
        let seed = root.join("seed");
        must(root, &["init", "-q", "-b", "main", seed.to_str().unwrap()]);
        for (key, value) in [
            ("user.name", "Seed"),
            ("user.email", "seed@example.com"),
            ("commit.gpgsign", "false"),
        ] {
            must(&seed, &["config", key, value]);
        }
        std::fs::write(seed.join("f"), "0").unwrap();
        must(&seed, &["add", "f"]);
        must(&seed, &["commit", "-qm", "0"]);
        must(
            &seed,
            &[
                "push",
                "-q",
                origin.to_str().unwrap(),
                "HEAD:refs/heads/main",
            ],
        );
        let clone = root.join("clone");
        must(
            root,
            &[
                "clone",
                "-q",
                origin.to_str().unwrap(),
                clone.to_str().unwrap(),
            ],
        );
        let mut trees = vec![clone.clone()];
        for i in 0..linked {
            let tree = root.join(format!("linked-{i}"));
            must(
                &clone,
                &["worktree", "add", "-q", "--detach", tree.to_str().unwrap()],
            );
            trees.push(tree);
        }
        (origin, seed, trees)
    }

    fn advance(seed: &Path, origin: &Path, round: usize) -> String {
        std::fs::write(seed.join("f"), round.to_string()).unwrap();
        must(seed, &["commit", "-qam", &round.to_string()]);
        must(
            seed,
            &[
                "push",
                "-q",
                origin.to_str().unwrap(),
                "HEAD:refs/heads/main",
            ],
        );
        must(seed, &["rev-parse", "HEAD"]).trim().to_owned()
    }

    #[test]
    fn ref_lock_refusals_are_told_apart_from_other_failures() {
        assert!(lost_a_ref_lock(
            "error: cannot lock ref 'refs/remotes/origin/main': is at 130a8bc but expected a45a5a9"
        ));
        assert!(lost_a_ref_lock(
            "error: cannot update ref 'refs/remotes/origin/main': couldn't write \
             '/repo/.git/refs/remotes/origin/main.lock'"
        ));
        assert!(!lost_a_ref_lock(
            "fatal: unable to access 'https://github.com/x/y/': Could not resolve host"
        ));
        assert!(!lost_a_ref_lock("fatal: couldn't find remote ref main"));
    }

    /// Five Coder fetches and one outside `git fetch` start together each
    /// round, as on coderos-4080 when someone pulls during a batch. Without
    /// the retry one of them failed every round
    /// (`bench/worktrees/git_contention.py`, scenario `fetch-mixed`).
    #[test]
    fn coder_fetches_survive_an_outside_fetch_of_the_same_ref() {
        const CODER: usize = 5;
        let dir = tempfile::tempdir().unwrap();
        let (origin, seed, trees) = repository(dir.path(), CODER);
        for round in 1..=8 {
            let tip = advance(&seed, &origin, round);
            let barrier = Barrier::new(CODER + 1);
            let results: Vec<Result<(), String>> = std::thread::scope(|scope| {
                let outside = scope.spawn(|| {
                    barrier.wait();
                    let _ = git(&trees[0], &["fetch", "-q", "origin", "main"]);
                });
                let coder: Vec<_> = trees[1..]
                    .iter()
                    .map(|tree| {
                        let barrier = &barrier;
                        scope.spawn(move || {
                            barrier.wait();
                            fetch(tree, &["main"], git)
                        })
                    })
                    .collect();
                outside.join().unwrap();
                coder.into_iter().map(|h| h.join().unwrap()).collect()
            });
            for result in results {
                result.unwrap_or_else(|why| panic!("round {round}: {why}"));
            }
            assert_eq!(must(&trees[0], &["rev-parse", "origin/main"]).trim(), tip);
        }
    }

    #[test]
    fn a_failed_fetch_releases_the_lock_and_does_not_retry_other_failures() {
        let dir = tempfile::tempdir().unwrap();
        let (_, _, trees) = repository(dir.path(), 1);
        let calls = std::sync::atomic::AtomicUsize::new(0);
        let failing = |dir: &Path, args: &[&str]| {
            if args.first() == Some(&"fetch") {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                return Err("fatal: couldn't find remote ref nope".to_owned());
            }
            git(dir, args)
        };
        assert!(fetch(&trees[1], &["nope"], failing).is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        fetch(&trees[0], &["main"], git).unwrap();
    }
}
