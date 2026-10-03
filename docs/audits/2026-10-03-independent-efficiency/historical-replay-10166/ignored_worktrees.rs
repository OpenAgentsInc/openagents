//! Independent acceptance checks for the public behavior requested by issue 10166.
//! Uses only APIs present before the fix and disposable local Git repositories.

use background::git::{removable, remove};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

struct Fixture {
    _temp: TempDir,
    source: PathBuf,
    tree: PathBuf,
}

fn git(directory: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(directory)
        .args([
            "-c",
            "user.name=Replay fixture",
            "-c",
            "user.email=replay@example.invalid",
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("source");
        let origin = temp.path().join("origin.git");
        let tree = temp.path().join("task-tree");
        git(
            temp.path(),
            &["init", "--quiet", "--bare", origin.to_str().unwrap()],
        );
        git(
            temp.path(),
            &[
                "init",
                "--quiet",
                "--initial-branch=main",
                source.to_str().unwrap(),
            ],
        );
        fs::write(source.join("tracked.txt"), "committed source\n").unwrap();
        fs::write(
            source.join(".gitignore"),
            "/.env\n/private/\n/target/\n/web/node_modules/\n/.cargo-target-replay/\n",
        )
        .unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "--quiet", "-m", "Fixture"]);
        git(
            &source,
            &["remote", "add", "origin", origin.to_str().unwrap()],
        );
        git(&source, &["push", "--quiet", "origin", "main"]);
        git(
            &source,
            &[
                "worktree",
                "add",
                "--quiet",
                "--detach",
                tree.to_str().unwrap(),
                "HEAD",
            ],
        );
        Self {
            _temp: temp,
            source,
            tree,
        }
    }

    fn write(&self, path: &str, bytes: &[u8]) {
        let path = self.tree.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn assert_ignored_data_kept(&self, path: &str, bytes: &[u8], reason_fragment: &str) {
        // Establish the original failure condition: ordinary status is clean.
        assert_eq!(
            git(
                &self.tree,
                &["status", "--porcelain", "--untracked-files=all"]
            ),
            ""
        );
        let result = removable(&self.tree);
        assert!(
            result.is_err(),
            "A worktree containing ignored user data must remain: {path}"
        );
        let reason = result.unwrap_err();
        assert!(
            reason.contains(reason_fragment),
            "The reason must identify the retained path: {reason}"
        );
        assert_eq!(fs::read(self.tree.join(path)).unwrap(), bytes);
        assert!(self.tree.join(".git").is_file());
    }
}

#[test]
fn ignored_dotenv_is_kept_with_a_useful_reason() {
    let fixture = Fixture::new();
    let bytes = b"fixture-only local settings\n";
    fixture.write(".env", bytes);
    fixture.assert_ignored_data_kept(".env", bytes, ".env");
}

#[test]
fn ignored_private_directory_is_kept_with_its_contents() {
    let fixture = Fixture::new();
    let bytes = b"fixture-only private data\0with binary bytes";
    fixture.write("private/data.bin", bytes);
    fixture.assert_ignored_data_kept("private/data.bin", bytes, "private");
}

#[test]
fn disposable_caches_do_not_hide_ignored_user_data() {
    let fixture = Fixture::new();
    fixture.write("target/debug/cache", b"rebuildable");
    fixture.write("web/node_modules/dependency/index.js", b"reinstallable");
    let bytes = b"settings retained beside caches\n";
    fixture.write(".env", bytes);
    fixture.assert_ignored_data_kept(".env", bytes, ".env");
}

#[test]
fn recognized_cache_only_worktree_can_still_be_removed() {
    let fixture = Fixture::new();
    fixture.write("target/debug/cache", b"rebuildable");
    fixture.write("web/node_modules/dependency/index.js", b"reinstallable");
    fixture.write(".cargo-target-replay/debug/cache", b"rebuildable");
    let undo = removable(&fixture.tree).expect("Recognized ignored caches must not block cleanup");
    remove(&undo).expect("The approved cache-only worktree must be removable");
    assert!(!fixture.tree.exists());
    assert_eq!(
        fs::read_to_string(fixture.source.join("tracked.txt")).unwrap(),
        "committed source\n"
    );
}

#[test]
fn clean_pushed_worktree_remains_eligible() {
    let fixture = Fixture::new();
    let undo = removable(&fixture.tree).expect("A clean pushed worktree remains eligible");
    remove(&undo).unwrap();
    assert!(!fixture.tree.exists());
}

#[test]
fn ordinary_unsaved_work_is_still_protected() {
    let fixture = Fixture::new();
    fixture.write("untracked.txt", b"unsaved");
    assert!(removable(&fixture.tree).is_err());
    assert_eq!(
        fs::read(fixture.tree.join("untracked.txt")).unwrap(),
        b"unsaved"
    );
    fs::remove_file(fixture.tree.join("untracked.txt")).unwrap();
    fixture.write("tracked.txt", b"edited");
    assert!(removable(&fixture.tree).is_err());
    assert_eq!(
        fs::read(fixture.tree.join("tracked.txt")).unwrap(),
        b"edited"
    );
}

#[test]
fn unpushed_commits_are_still_protected() {
    let fixture = Fixture::new();
    fixture.write("tracked.txt", b"committed only in this task\n");
    git(&fixture.tree, &["add", "tracked.txt"]);
    git(
        &fixture.tree,
        &["commit", "--quiet", "-m", "Unpushed task change"],
    );
    assert!(removable(&fixture.tree).is_err());
    assert!(fixture.tree.exists());
}
