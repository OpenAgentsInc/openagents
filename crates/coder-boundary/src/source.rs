//! Keeping a command out of the person's own checkout, even with full
//! access.
//!
//! Coder works in a worktree of its own, never in the checkout it was
//! made from: the checkout changes only by Git's record of the worktree.
//! Under the write boundary that holds by construction, since every write
//! outside the worktree and a scratch is denied. A full-access run has no
//! write boundary, and a whole coding agent approves its own tools, so
//! there the worktree used to be advisory: in the shadow-baseline study
//! (#10209) every routed `fix-git` run `cd`'d into the person's checkout
//! and merged into its `master` (#10247).
//!
//! A [`Guard`] names what must stay unwritten for one worktree: the
//! source checkout's working tree and its Git directory. It allows back
//! the only places in them that work in the worktree needs:
//!
//! - the worktree itself, when it lies inside the checkout;
//! - the worktree's own administrative directory
//!   (`.git/worktrees/<name>`: its `HEAD`, index, and reflog);
//! - the object store (`.git/objects`), so a commit in the worktree can
//!   store its objects. Objects are named by their content; adding one
//!   changes no file, branch, tag, or index of the checkout.
//!
//! Remote-tracking refs and their reflogs are writable so a push can record
//! where it saved the task commit. They do not move the checkout's branch.
//!
//! Everything else there stays unwritten: the checkout's files, its
//! index and `HEAD`, local refs (branches, tags, and the stash),
//! `packed-refs`, the configuration, and the hooks. Everything
//! outside the checkout is as full access always was: writable, readable,
//! and online.
//!
//! The guard is enforced, not asked for:
//!
//! - On macOS the command runs under `sandbox-exec` with the
//!   [`crate::privacy`] profile, whose `(allow default)` is followed by
//!   the guard's write denies and then its allows. As with the privacy
//!   profile, `sandbox-exec` itself and the system's set-user-ID programs
//!   start outside it (a profile can't apply inside another, and the
//!   kernel won't raise a sandboxed process's privileges), and a process
//!   that already runs in a sandbox is left to that sandbox.
//! - On Linux the command runs under `bwrap` with the whole host bound
//!   as it is (`--dev-bind / /`: devices, `/proc`, and the network as
//!   they were), then the checkout and its Git directory bound read-only
//!   over it, then the allowed paths bound writable over those. A write
//!   there fails with "Read-only file system". The command runs in a
//!   user namespace, so a set-user-ID program such as `sudo` does not
//!   raise its privileges there.
//!
//! On other platforms there is no guard ([`APPLIES`] is false).

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::boundary::{Error, SANDBOX_EXEC, allow, backend_path, deny, existing};

/// Whether this platform enforces a [`Guard`].
pub const APPLIES: bool = cfg!(any(target_os = "macos", target_os = "linux"));

/// The variable that stops Git's repository discovery at the worktree's
/// parent, so a command in a folder beside the worktree never finds a
/// repository above it.
pub const CEILING: &str = "GIT_CEILING_DIRECTORIES";

/// What one worktree's commands may not write: its source checkout and
/// that checkout's Git directory, with the few places in them the
/// worktree needs allowed back. See the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guard {
    worktree: PathBuf,
    /// Every rule, `true` for an allow, in the order it applies:
    /// shallower paths first, and at the same depth a deny before an
    /// allow, so each later rule is the exception to the earlier ones.
    rules: Vec<(PathBuf, bool)>,
}

impl Guard {
    /// The guard for the Git worktree at `worktree`, found with Git: the
    /// main working tree (`git worktree list`'s first entry, unless the
    /// repository is bare) and the common Git directory are denied; the
    /// worktree, its administrative directory, the object store, and remote-
    /// tracking refs and their reflogs are allowed back. `None` when `worktree` is not a linked worktree (it
    /// is the checkout itself, or a repository of its own): there is no
    /// other checkout to keep it out of.
    ///
    /// # Errors
    /// Git can't describe the worktree, or a path has no safe spelling.
    pub fn for_worktree(worktree: &Path) -> Result<Option<Guard>, Error> {
        let worktree = existing(worktree)?;
        let directories = git(
            &worktree,
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-dir",
                "--git-common-dir",
            ],
        )?;
        let mut lines = directories.lines();
        let (Some(own), Some(common)) = (lines.next(), lines.next()) else {
            return Err(git_error(&worktree, "no Git directory"));
        };
        let own = existing(Path::new(own))?;
        let common = existing(Path::new(common))?;
        if own == common {
            return Ok(None);
        }
        let list = git(&worktree, &["worktree", "list", "--porcelain"])?;
        let main = main_worktree(&list)
            .map(|main| existing(&main))
            .transpose()?;
        let mut protected = vec![common.clone()];
        protected.extend(main.filter(|main| *main != worktree));
        let objects = common.join("objects");
        let mut allowed = vec![worktree.clone(), own];
        if objects.is_dir() {
            allowed.push(existing(&objects)?);
        }
        // Git creates loose tracking refs even when the old ref is packed.
        // Prepare these directories outside the guard: their parents remain
        // protected, including refs/heads and packed-refs.
        for relative in ["refs/remotes", "logs/refs/remotes"] {
            let path = common.join(relative);
            std::fs::create_dir_all(&path).map_err(Error::Io)?;
            allowed.push(existing(&path)?);
        }
        Guard::new(&worktree, &protected, &allowed).map(Some)
    }

    /// A guard that denies writes beneath `protected` and allows them
    /// back beneath `allowed`, for commands working in `worktree`. An
    /// allowed path outside every protected one is left out: it was never
    /// denied. Every path must exist and is resolved.
    ///
    /// # Errors
    /// A path is relative, missing, or has no safe Seatbelt spelling.
    pub fn new(
        worktree: &Path,
        protected: &[PathBuf],
        allowed: &[PathBuf],
    ) -> Result<Guard, Error> {
        let worktree = existing(worktree)?;
        let mut rules = Vec::new();
        for path in protected {
            let path = existing(path)?;
            // The spelling is checked here, so the profile is never
            // refused later.
            deny(&path)?;
            if !rules.contains(&(path.clone(), false)) {
                rules.push((path, false));
            }
        }
        for path in allowed {
            let path = existing(path)?;
            allow(&path)?;
            let inside = rules
                .iter()
                .any(|(denied, writable)| !writable && path.starts_with(denied));
            if inside && !rules.contains(&(path.clone(), true)) {
                rules.push((path, true));
            }
        }
        rules.sort_by_key(|(path, writable)| (path.components().count(), *writable));
        Ok(Guard { worktree, rules })
    }

    /// The worktree the guard is for, resolved.
    #[must_use]
    pub fn worktree(&self) -> &Path {
        &self.worktree
    }

    /// The paths whose writes are denied, resolved.
    pub fn protected(&self) -> impl Iterator<Item = &Path> {
        self.rules
            .iter()
            .filter(|(_, writable)| !writable)
            .map(|(path, _)| path.as_path())
    }

    /// The paths inside a protected one whose writes are allowed back.
    pub fn allowed(&self) -> impl Iterator<Item = &Path> {
        self.rules
            .iter()
            .filter(|(_, writable)| *writable)
            .map(|(path, _)| path.as_path())
    }

    /// The Seatbelt rules, for a profile that allows writes by default.
    #[must_use]
    pub fn seatbelt(&self) -> String {
        let mut rules = String::from(
            ";; the source checkout stays unwritten (crates/coder-boundary/src/source.rs)\n",
        );
        for (path, writable) in &self.rules {
            // Spellings were checked when the guard was made.
            let rule = if *writable { allow(path) } else { deny(path) };
            rules.push_str(&rule.unwrap_or_default());
        }
        rules
    }

    /// The `bwrap` arguments before the program, ending in `--`: the
    /// host as it is, then each protected path read-only and each allowed
    /// one writable, in order.
    #[must_use]
    pub fn bubblewrap(&self) -> Vec<OsString> {
        let mut args: Vec<OsString> = ["--die-with-parent", "--dev-bind", "/", "/"]
            .into_iter()
            .map(Into::into)
            .collect();
        for (path, writable) in &self.rules {
            args.push(if *writable { "--bind" } else { "--ro-bind" }.into());
            args.push(path.into());
            args.push(path.into());
        }
        args.push("--".into());
        args
    }

    /// The environment a guarded command adds: Git's discovery stops at
    /// the worktree's parent ([`CEILING`]).
    #[must_use]
    pub fn environment(&self) -> Vec<(OsString, OsString)> {
        self.worktree
            .parent()
            .map(|parent| vec![(CEILING.into(), parent.into())])
            .unwrap_or_default()
    }

    /// Whether this host can enforce the guard: on Linux, `bwrap` is
    /// installed and can make a namespace here; on macOS, always (a
    /// process with no `sandbox-exec` or already in a sandbox is left as
    /// [`crate::privacy::command`] leaves it).
    ///
    /// # Errors
    /// The platform has no guard, or `bwrap` is missing or inoperable.
    pub fn enforceable(&self) -> Result<(), Error> {
        if !APPLIES {
            return Err(Error::Unsupported(std::env::consts::OS));
        }
        if cfg!(target_os = "linux") {
            let backend = Path::new(backend_path());
            if !backend.is_file() {
                return Err(Error::Unavailable(backend.to_path_buf()));
            }
            crate::boundary::operable(backend)?;
        }
        Ok(())
    }

    /// `program` under the guard, with the [`crate::privacy`] rules on
    /// macOS (`private` allowed back there): the caller adds the
    /// arguments, working directory, and environment as for `program`
    /// itself, then [`Guard::environment`].
    #[must_use]
    pub fn command(&self, program: impl Into<PathBuf>, private: &[&Path]) -> Command {
        let (program, arguments) = self.wrap(program.into(), private);
        let mut command = Command::new(program);
        command.args(arguments);
        command
    }

    /// [`Guard::command`] as a program and its arguments, for a caller
    /// that takes them apart, such as an agent spawned over its standard
    /// streams.
    #[must_use]
    pub fn argv(
        &self,
        program: PathBuf,
        arguments: Vec<String>,
        private: &[&Path],
    ) -> (PathBuf, Vec<String>) {
        let (backend, mut prefix) = self.wrap(program, private);
        if prefix.is_empty() {
            return (backend, arguments);
        }
        let mut wrapped: Vec<String> = prefix
            .drain(..)
            .map(|argument| argument.to_string_lossy().into_owned())
            .collect();
        wrapped.extend(arguments);
        (backend, wrapped)
    }

    /// The backend and its arguments up to and including `program`, or
    /// `program` and nothing where nothing wraps.
    fn wrap(&self, program: PathBuf, private: &[&Path]) -> (PathBuf, Vec<OsString>) {
        if cfg!(target_os = "linux") {
            let mut args = self.bubblewrap();
            args.push(program.into());
            return (PathBuf::from(backend_path()), args);
        }
        if cfg!(target_os = "macos")
            && program != Path::new(SANDBOX_EXEC)
            && !crate::privacy::sandboxed()
            && Path::new(SANDBOX_EXEC).is_file()
        {
            let home = crate::privacy::home();
            let profile = crate::privacy::profile_with(home.as_deref(), private, &self.seatbelt());
            return (
                PathBuf::from(SANDBOX_EXEC),
                vec!["-p".into(), profile.into(), program.into()],
            );
        }
        (program, Vec::new())
    }
}

/// The main working tree from `git worktree list --porcelain`: the first
/// record's `worktree` line, unless that record is `bare`.
fn main_worktree(porcelain: &str) -> Option<PathBuf> {
    let first = porcelain.split("\n\n").next()?;
    let mut path = None;
    for line in first.lines() {
        if line == "bare" {
            return None;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            path = Some(PathBuf::from(rest));
        }
    }
    path
}

/// Where [`git`] looks for Git before the bare name: a task owner starts
/// with a cleared environment, and NixOS keeps Git in none of the usual
/// folders, so a bare `git` alone isn't found there (#10244).
const GIT_PATHS: [&str; 4] = [
    "/usr/bin/git",
    "/opt/homebrew/bin/git",
    "/usr/local/bin/git",
    "/run/current-system/sw/bin/git",
];

fn git(directory: &Path, arguments: &[&str]) -> Result<String, Error> {
    let program = GIT_PATHS
        .into_iter()
        .find(|path| Path::new(path).is_file())
        .unwrap_or("git");
    let output = Command::new(program)
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_COMMON_DIR")
        .stdin(std::process::Stdio::null())
        .output()
        .map_err(Error::Io)?;
    if !output.status.success() {
        return Err(git_error(
            directory,
            String::from_utf8_lossy(&output.stderr).trim(),
        ));
    }
    String::from_utf8(output.stdout).map_err(|_| Error::Unsafe(directory.to_path_buf()))
}

fn git_error(directory: &Path, why: &str) -> Error {
    Error::Resolve {
        path: directory.to_path_buf(),
        error: format!("git: {why}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_main_worktree_is_the_first_record_unless_bare() {
        let list = "worktree /a/main\nHEAD 0123\nbranch refs/heads/master\n\n\
                    worktree /b/wt\nHEAD 4567\ndetached\n\n";
        assert_eq!(main_worktree(list), Some(PathBuf::from("/a/main")));
        assert_eq!(
            main_worktree("worktree /a/repo.git\nbare\n\nworktree /b/wt\n"),
            None
        );
        assert_eq!(main_worktree(""), None);
    }

    #[test]
    fn rules_apply_shallow_first_and_allows_only_inside_a_deny() {
        let root = tempfile::tempdir().unwrap();
        let checkout = root.path().join("checkout");
        let git = checkout.join(".git");
        let admin = git.join("worktrees/wt");
        let worktree = checkout.join("nested/wt");
        let elsewhere = root.path().join("elsewhere");
        for path in [&admin, &worktree, &elsewhere] {
            std::fs::create_dir_all(path).unwrap();
        }
        let guard = Guard::new(
            &worktree,
            &[git.clone(), checkout.clone()],
            &[admin.clone(), worktree.clone(), elsewhere],
        )
        .unwrap();
        let real = |path: &Path| path.canonicalize().unwrap();
        assert_eq!(
            guard.protected().collect::<Vec<_>>(),
            [real(&checkout), real(&git)]
        );
        assert_eq!(
            guard.allowed().collect::<Vec<_>>(),
            [real(&worktree), real(&admin)]
        );
        let rules = guard.seatbelt();
        let at = |needle: String| rules.find(&needle).unwrap();
        assert!(
            at(format!(
                "(deny file-write* (subpath \"{}\")",
                real(&checkout).display()
            )) < at(format!(
                "(allow file-write* (subpath \"{}\")",
                real(&admin).display()
            )),
            "{rules}"
        );
        let args: Vec<String> = guard
            .bubblewrap()
            .into_iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect();
        assert_eq!(&args[..4], ["--die-with-parent", "--dev-bind", "/", "/"]);
        assert_eq!(args[4], "--ro-bind");
        assert_eq!(args.last().map(String::as_str), Some("--"));
        assert_eq!(
            guard.environment(),
            [(CEILING.into(), real(&worktree).parent().unwrap().into())]
        );
    }
}
