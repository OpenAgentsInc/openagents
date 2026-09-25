//! What a lean-loop candidate is, on disk: the files the host snapshots,
//! identifies, bounds, and restores.
//!
//! In a task container without Git, a candidate is the whole workspace,
//! as it always was: [`crate::handoff::copy_tree`] takes the snapshot,
//! [`super::parallel::copyable`] bounds it, and
//! [`crate::compose::replace_contents`] restores it. [`Scope::Plain`] keeps
//! that behavior byte for byte.
//!
//! In a Git work tree, the whole workspace includes the `.git` directory
//! and build output such as `target/`, which put a real checkout over the
//! snapshot bound although neither is part of the candidate. There a
//! candidate is the files `git ls-files --cached --others
//! --exclude-standard` lists: tracked files that still exist, and untracked
//! files Git doesn't ignore. [`Scope::Git`] snapshots, identifies, and
//! bounds only those, and a restore leaves ignored files and the Git
//! directory where they are.
//!
//! Both scopes leave out what [`super::lean::evidence_tree`] leaves out:
//! Git metadata, Python bytecode, and the named cache directories. So the
//! identity of a workspace in either scope equals `evidence_tree` of its
//! snapshot.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::parallel;

/// The most files a candidate snapshot holds.
pub const MAX_FILES: usize = 20_000;

/// How [`Scope::Git`] identities are described in the record.
pub const GIT_SCOPE: &str = "file contents and link targets of the files Git lists as tracked or \
untracked and not ignored, excluding Git metadata, Python bytecode, and named caches";

/// How [`Scope::Plain`] identities are described in the record.
pub const PLAIN_SCOPE: &str =
    "file contents and link targets, excluding Git metadata, Python bytecode, and named caches";

/// Which files make up a candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Every file in the workspace.
    Plain,
    /// The files Git lists as tracked or untracked and not ignored, by the
    /// rules of this Git directory. A copy of the workspace, such as a
    /// lane's, is read by the same rules.
    Git(PathBuf),
}

/// A `git` command with the caller's repository variables cleared, so the
/// command reads only the directories it names.
fn git() -> Command {
    let mut command = Command::new("git");
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
    ] {
        command.env_remove(name);
    }
    command
}

/// Whether a relative path is outside every candidate identity: in a
/// named cache or Git directory, or Python bytecode.
fn excluded(relative: &str) -> bool {
    let mut parts = relative.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return part.ends_with(".pyc");
        }
        if parallel::UNMERGED.contains(&part) {
            return true;
        }
    }
    false
}

impl Scope {
    /// The scope of `workdir`: [`Scope::Git`] when it is the top of a Git
    /// work tree and Git can read it, else [`Scope::Plain`].
    #[must_use]
    pub fn of(workdir: &Path) -> Scope {
        if workdir.join(".git").symlink_metadata().is_err() {
            return Scope::Plain;
        }
        let Ok(output) = git()
            .arg("-C")
            .arg(workdir)
            .args(["rev-parse", "--absolute-git-dir"])
            .output()
        else {
            return Scope::Plain;
        };
        let dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !output.status.success() || dir.is_empty() {
            return Scope::Plain;
        }
        Scope::Git(PathBuf::from(dir))
    }

    /// How the record describes this scope's identities.
    #[must_use]
    pub fn describe(&self) -> &'static str {
        match self {
            Scope::Plain => PLAIN_SCOPE,
            Scope::Git(_) => GIT_SCOPE,
        }
    }

    /// The name the record gives this scope.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Scope::Plain => "plain",
            Scope::Git(_) => "git",
        }
    }

    /// The candidate's files under `dir`, relative and sorted: files and
    /// symbolic links, never directories, and nothing [`excluded`].
    ///
    /// # Errors
    ///
    /// Returns a message when a directory can't be read, Git can't list
    /// the files, or a path isn't UTF-8.
    pub fn files(&self, dir: &Path) -> Result<Vec<String>, String> {
        let mut out = BTreeSet::new();
        match self {
            Scope::Plain => {
                let mut stack = vec![dir.to_path_buf()];
                while let Some(at) = stack.pop() {
                    for entry in
                        std::fs::read_dir(&at).map_err(|e| format!("{}: {e}", at.display()))?
                    {
                        let entry = entry.map_err(|e| e.to_string())?;
                        let kind = entry.file_type().map_err(|e| e.to_string())?;
                        let path = entry.path();
                        let relative = path.strip_prefix(dir).map_err(|e| e.to_string())?;
                        let relative = relative.to_str().ok_or("file path is not UTF-8")?;
                        if kind.is_dir() {
                            if !excluded(&format!("{relative}/x")) {
                                stack.push(path);
                            }
                        } else if !excluded(relative) {
                            out.insert(relative.to_string());
                        }
                    }
                }
            }
            Scope::Git(git_dir) => {
                let output = git()
                    .arg(format!("--git-dir={}", git_dir.display()))
                    .arg(format!("--work-tree={}", dir.display()))
                    .arg("-C")
                    .arg(dir)
                    .args([
                        "ls-files",
                        "-z",
                        "--cached",
                        "--others",
                        "--exclude-standard",
                    ])
                    .output()
                    .map_err(|e| format!("git could not list the candidate files: {e}"))?;
                if !output.status.success() {
                    return Err(format!(
                        "git could not list the candidate files: {}",
                        String::from_utf8_lossy(&output.stderr).trim()
                    ));
                }
                for raw in output.stdout.split(|b| *b == 0) {
                    if raw.is_empty() {
                        continue;
                    }
                    let relative =
                        std::str::from_utf8(raw).map_err(|_| "file path is not UTF-8")?;
                    let relative = relative.trim_end_matches('/');
                    if excluded(relative) {
                        continue;
                    }
                    // A tracked file the workspace deleted is still in the
                    // index; a nested repository or submodule is a directory.
                    match dir.join(relative).symlink_metadata() {
                        Ok(meta) if !meta.is_dir() => {
                            out.insert(relative.to_string());
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(out.into_iter().collect())
    }

    /// The candidate identity of `dir`: each file's SHA-256, a symbolic
    /// link by its target. It equals [`super::lean::evidence_tree`] of a
    /// snapshot [`Scope::snapshot`] took of `dir`.
    ///
    /// # Errors
    ///
    /// Returns a message when the inventory is incomplete.
    pub fn identity(&self, dir: &Path) -> Result<BTreeMap<String, String>, String> {
        match self {
            Scope::Plain => super::lean::evidence_tree(dir),
            Scope::Git(_) => {
                let mut tree = BTreeMap::new();
                for relative in self.files(dir)? {
                    let path = dir.join(&relative);
                    let meta = path.symlink_metadata().map_err(|e| e.to_string())?;
                    let bytes = if meta.file_type().is_symlink() {
                        let target = std::fs::read_link(&path).map_err(|e| e.to_string())?;
                        format!(
                            "link:{}",
                            target.to_str().ok_or("link target is not UTF-8")?
                        )
                        .into_bytes()
                    } else if meta.is_file() {
                        std::fs::read(&path).map_err(|e| e.to_string())?
                    } else {
                        return Err(format!("unsupported candidate entry: {}", path.display()));
                    };
                    tree.insert(relative, crate::accept::sha256(&bytes));
                }
                Ok(tree)
            }
        }
    }

    /// Whether `dir`'s candidate fits the snapshot bound: at most
    /// [`MAX_FILES`] files and [`parallel::MAX_COPY_BYTES`] bytes.
    ///
    /// # Errors
    ///
    /// Returns a plain statement of what is over the bound, or why the
    /// files couldn't be counted.
    pub fn bound(&self, dir: &Path) -> Result<(), String> {
        let limit = format!(
            "{MAX_FILES} files or {} MiB",
            parallel::MAX_COPY_BYTES / (1024 * 1024)
        );
        match self {
            Scope::Plain => {
                if parallel::copyable(dir) {
                    Ok(())
                } else {
                    Err(format!(
                        "the workspace is over the snapshot bound of {limit}, or a directory in \
                         it can't be read"
                    ))
                }
            }
            Scope::Git(_) => {
                let files = self.files(dir)?;
                let mut bytes = 0u64;
                for relative in &files {
                    bytes += dir
                        .join(relative)
                        .symlink_metadata()
                        .map(|m| m.len())
                        .unwrap_or(0);
                }
                if files.len() > MAX_FILES || bytes > parallel::MAX_COPY_BYTES {
                    Err(format!(
                        "the workspace's tracked and unignored files are {} files and {} MiB, \
                         over the snapshot bound of {limit}",
                        files.len(),
                        bytes.div_ceil(1024 * 1024)
                    ))
                } else {
                    Ok(())
                }
            }
        }
    }

    /// Copies `dir`'s candidate to `to`, replacing whatever `to` held.
    ///
    /// # Errors
    ///
    /// Returns a message when a file can't be listed, read, or written.
    pub fn snapshot(&self, dir: &Path, to: &Path) -> Result<(), String> {
        match self {
            Scope::Plain => crate::handoff::copy_tree(dir, to),
            Scope::Git(_) => {
                let files = self.files(dir)?;
                if to.symlink_metadata().is_ok() {
                    std::fs::remove_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
                }
                std::fs::create_dir_all(to).map_err(|e| format!("{}: {e}", to.display()))?;
                for relative in files {
                    place(&dir.join(&relative), &to.join(&relative))?;
                }
                Ok(())
            }
        }
    }

    /// Makes `dir`'s candidate equal `from`'s: removes candidate files
    /// `from` doesn't have and copies in every file it has. In
    /// [`Scope::Git`] it leaves ignored files and the Git directory alone.
    ///
    /// # Errors
    ///
    /// Returns a message when `dir` isn't a workspace the host may
    /// replace, or a file can't be listed, removed, or written.
    pub fn restore(&self, dir: &Path, from: &Path) -> Result<(), String> {
        match self {
            Scope::Plain => crate::compose::replace_contents(dir, from),
            Scope::Git(_) => {
                if !crate::compose::safe_to_replace(dir) {
                    return Err(format!(
                        "{} is not a workspace the host may replace",
                        dir.display()
                    ));
                }
                let want = self.files(from)?;
                let keep: BTreeSet<&String> = want.iter().collect();
                for relative in self.files(dir)? {
                    if keep.contains(&relative) {
                        continue;
                    }
                    let path = dir.join(&relative);
                    std::fs::remove_file(&path)
                        .map_err(|e| format!("cannot remove {}: {e}", path.display()))?;
                    // Remove directories the removal emptied, up to `dir`.
                    let mut parent = path.parent();
                    while let Some(at) = parent {
                        if at == dir || std::fs::remove_dir(at).is_err() {
                            break;
                        }
                        parent = at.parent();
                    }
                }
                for relative in &want {
                    place(&from.join(relative), &dir.join(relative))?;
                }
                Ok(())
            }
        }
    }
}

/// Copies one file or symbolic link, making its parent directories and
/// replacing a file already at `to`.
fn place(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    if let Ok(meta) = to.symlink_metadata() {
        if meta.is_dir() {
            return Err(format!(
                "{} is a directory where the candidate has a file",
                to.display()
            ));
        }
        std::fs::remove_file(to).map_err(|e| format!("{}: {e}", to.display()))?;
    }
    let meta = from
        .symlink_metadata()
        .map_err(|e| format!("{}: {e}", from.display()))?;
    if meta.file_type().is_symlink() {
        let target = std::fs::read_link(from).map_err(|e| e.to_string())?;
        std::os::unix::fs::symlink(target, to).map_err(|e| format!("{}: {e}", to.display()))
    } else {
        std::fs::copy(from, to)
            .map(|_| ())
            .map_err(|e| format!("{}: {e}", from.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, path: &str, text: &str) {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    fn run_git(dir: &Path, args: &[&str]) {
        let status = git()
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=test", "-c", "user.email=test@example.com"])
            .args(args)
            .output()
            .unwrap();
        assert!(status.status.success(), "{args:?}: {status:?}");
    }

    /// A Git work tree with a tracked source file, an untracked one, a
    /// `.gitignore`, and an ignored `target/` too big to snapshot.
    fn repository() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path();
        run_git(work, &["init", "-q"]);
        write(work, ".gitignore", "target/\n");
        write(work, "src/lib.rs", "pub fn one() -> u32 { 1 }\n");
        run_git(work, &["add", "-A"]);
        run_git(work, &["commit", "-q", "-m", "start"]);
        write(work, "notes.txt", "untracked\n");
        // Past the bytes a plain copy counts, as a sparse file.
        write(work, "target/debug/deps/lib.o", "");
        std::fs::File::create(work.join("target/big.bin"))
            .unwrap()
            .set_len(parallel::MAX_COPY_BYTES + 1)
            .unwrap();
        dir
    }

    #[test]
    fn a_repository_with_a_big_ignored_directory_snapshots_and_restores() {
        let repo = repository();
        let work = repo.path();
        assert!(!parallel::copyable(work), "the plain bound counts target/");
        let scope = Scope::of(work);
        assert!(matches!(scope, Scope::Git(_)), "{scope:?}");
        assert_eq!(
            scope.files(work).unwrap(),
            [".gitignore", "notes.txt", "src/lib.rs"]
        );
        scope.bound(work).unwrap();

        let snapshots = tempfile::tempdir().unwrap();
        let snap = snapshots.path().join("session-1");
        let before = scope.identity(work).unwrap();
        scope.snapshot(work, &snap).unwrap();
        assert_eq!(super::super::lean::evidence_tree(&snap).unwrap(), before);
        assert!(!snap.join("target").exists());
        assert!(!snap.join(".git").exists());

        // Later edits: change a tracked file, add and delete files, build.
        write(work, "src/lib.rs", "pub fn one() -> u32 { 2 }\n");
        write(work, "src/extra/new.rs", "// new\n");
        std::fs::remove_file(work.join("notes.txt")).unwrap();
        write(work, "target/debug/app", "binary");
        assert_ne!(scope.identity(work).unwrap(), before);

        scope.restore(work, &snap).unwrap();
        assert_eq!(scope.identity(work).unwrap(), before);
        assert!(
            !work.join("src/extra").exists(),
            "an emptied directory goes"
        );
        assert_eq!(
            std::fs::read_to_string(work.join("target/debug/app")).unwrap(),
            "binary",
            "ignored build output stays"
        );
        assert!(work.join("target/big.bin").is_file());
        assert!(work.join(".git/HEAD").is_file(), "the Git directory stays");
        run_git(work, &["status", "--short"]);
    }

    #[test]
    fn a_copy_of_the_repository_is_read_by_its_ignore_rules() {
        let repo = repository();
        let work = repo.path();
        let scope = Scope::of(work);
        let lanes = tempfile::tempdir().unwrap();
        let lane = lanes.path().join("lane");
        scope.snapshot(work, &lane).unwrap();
        write(&lane, "target/lane-build.o", "");
        write(&lane, "src/lane.rs", "// lane\n");
        let files = scope.files(&lane).unwrap();
        assert!(files.contains(&"src/lane.rs".to_string()), "{files:?}");
        assert!(!files.iter().any(|f| f.starts_with("target/")), "{files:?}");
        scope.restore(work, &lane).unwrap();
        assert!(work.join("src/lane.rs").is_file());
        assert!(!work.join("target/lane-build.o").exists());
    }

    #[test]
    fn a_plain_workspace_keeps_the_existing_identity_and_copy() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work");
        write(&work, "solve.py", "print(1)\n");
        write(&work, "__pycache__/solve.cpython-312.pyc", "bytecode");
        write(&work, "data/a.txt", "a\n");
        std::os::unix::fs::symlink("data/a.txt", work.join("link")).unwrap();
        let scope = Scope::of(&work);
        assert_eq!(scope, Scope::Plain);
        assert_eq!(
            scope.identity(&work).unwrap(),
            super::super::lean::evidence_tree(&work).unwrap()
        );
        assert_eq!(
            scope.files(&work).unwrap(),
            ["data/a.txt", "link", "solve.py"]
        );
        let snap = dir.path().join("snap");
        scope.snapshot(&work, &snap).unwrap();
        assert!(
            snap.join("__pycache__/solve.cpython-312.pyc").is_file(),
            "a plain snapshot copies everything, as before"
        );
        write(&work, "solve.py", "print(2)\n");
        scope.restore(&work, &snap).unwrap();
        assert_eq!(
            std::fs::read_to_string(work.join("solve.py")).unwrap(),
            "print(1)\n"
        );
    }
}
