//! Read-only Git that runs no program a repository's own config names.
//!
//! A repository's `.git/config` can name programs that Git starts on its
//! own while it only reads: `core.fsmonitor` on `git status`,
//! `diff.external` and `diff.<driver>.command` on `git diff`,
//! `diff.<driver>.textconv` on `git log -p`, `git show`, and `git blame`,
//! and `filter.<driver>.clean`, `smudge`, and `process` whenever Git
//! compares a work-tree file with the index. Pagers, hooks, `askpass`
//! programs, and signature verifiers are the same kind of risk. A hostile
//! checkout could run a program on this computer the moment Coder asks
//! for its status, before any permission check.
//!
//! [`ReadOnlyGit`] builds the one invocation every Coder read goes
//! through:
//!
//! - [`READ_ONLY_OPTIONS`] turn off the fixed keys on the command line,
//!   which outranks every config file.
//! - The repository's own filter and diff drivers are listed with
//!   `git config` (which reads config as data and runs nothing) and each
//!   program key is set empty. A driver whose name can't be written as a
//!   `-c` option is refused rather than left on.
//! - `diff`, `log`, `show`, and `whatchanged` get `--no-ext-diff` and
//!   `--no-textconv`; `blame`, `annotate`, and `grep` get `--no-textconv`.
//! - [`harden_environment`] sets `GIT_PAGER=cat` and
//!   `GIT_TERMINAL_PROMPT=0`, removes `GIT_ASKPASS`, `SSH_ASKPASS`, and
//!   `GIT_EXTERNAL_DIFF`, and drops the variables that point Git at
//!   another repository.
//!
//! `uploadpack.packObjectsHook` isn't set here: Git reads it only from
//! system, global, and command-line config, which a repository can't
//! write, and no read here runs `upload-pack`.
//!
//! The checklist follows fx's `safe_git.zig` (Apache-2.0); this is a
//! reimplementation, not a copy.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Options placed before the subcommand on every read-only call. Git
/// takes `-c` over every config file, so a repository can't turn these
/// back on.
pub const READ_ONLY_OPTIONS: &[&str] = &[
    "--no-pager",
    "--no-optional-locks",
    "-c",
    "core.fsmonitor=false",
    "-c",
    "core.hooksPath=/dev/null",
    "-c",
    "core.attributesFile=/dev/null",
    "-c",
    "core.pager=cat",
    "-c",
    "core.askPass=",
    "-c",
    "credential.helper=",
    // An empty external diff makes a diff that would have used one fail
    // instead of running anything, as a backstop to `--no-ext-diff`.
    "-c",
    "diff.external=",
    "-c",
    "diff.autoRefreshIndex=false",
    "-c",
    "log.showSignature=false",
    "-c",
    "gc.auto=0",
    "-c",
    "protocol.ext.allow=never",
];

/// The `(prefix, suffix)` config patterns whose middle part a repository
/// names, the program keys under them, and the value that turns each off.
const DRIVER_KEYS: &[(&str, &str, &str)] = &[
    ("filter.", ".clean", ""),
    ("filter.", ".smudge", ""),
    ("filter.", ".process", ""),
    ("filter.", ".required", "false"),
    ("diff.", ".textconv", ""),
    ("diff.", ".command", ""),
];

/// Subcommands that can run an external diff or a text conversion, and
/// the options that stop each.
fn subcommand_options(subcommand: &str) -> &'static [&'static str] {
    match subcommand {
        "diff" | "log" | "show" | "whatchanged" => &["--no-ext-diff", "--no-textconv"],
        "blame" | "annotate" | "grep" => &["--no-textconv"],
        _ => &[],
    }
}

/// Why a read-only Git call wasn't built.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused(pub String);

impl std::fmt::Display for Refused {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Refused {}

/// One read-only Git invocation in a directory.
#[derive(Debug, Clone)]
pub struct ReadOnlyGit {
    program: String,
    cwd: PathBuf,
    location: Vec<String>,
}

impl ReadOnlyGit {
    /// Git, from `PATH`, run in `cwd`.
    #[must_use]
    pub fn new(cwd: &Path) -> Self {
        Self {
            program: "git".into(),
            cwd: cwd.to_path_buf(),
            location: Vec::new(),
        }
    }

    /// Run this Git program instead of the one on `PATH`.
    #[must_use]
    pub fn program(mut self, program: &str) -> Self {
        self.program = program.into();
        self
    }

    /// Options that choose the repository, such as `-C DIR` or
    /// `--git-dir=X --work-tree=Y`. They go before the subcommand, and the
    /// driver listing uses them too.
    #[must_use]
    pub fn location<S: AsRef<str>>(mut self, options: &[S]) -> Self {
        self.location = options.iter().map(|o| o.as_ref().to_string()).collect();
        self
    }

    /// The program this call runs.
    #[must_use]
    pub fn program_name(&self) -> &str {
        &self.program
    }

    /// The arguments after the program: the fixed options, the location,
    /// this repository's driver overrides, the subcommand with the options
    /// that stop external diffs and text conversion, and the rest.
    ///
    /// # Errors
    ///
    /// When the repository's drivers can't be listed, or one has a name a
    /// `-c` option can't carry.
    pub fn args(&self, args: &[&str]) -> Result<Vec<String>, Refused> {
        let mut argv: Vec<String> = READ_ONLY_OPTIONS.iter().map(|s| (*s).into()).collect();
        argv.extend(self.location.iter().cloned());
        for (key, value) in self.driver_overrides()? {
            argv.push("-c".into());
            argv.push(format!("{key}={value}"));
        }
        if let Some((subcommand, rest)) = args.split_first() {
            argv.push((*subcommand).into());
            argv.extend(
                subcommand_options(subcommand)
                    .iter()
                    .map(|s| (*s).to_string()),
            );
            argv.extend(rest.iter().map(|s| (*s).to_string()));
        }
        Ok(argv)
    }

    /// A ready command: the program, [`Self::args`], the directory, a
    /// hardened environment, and no standard input.
    ///
    /// # Errors
    ///
    /// As [`Self::args`].
    pub fn command(&self, args: &[&str]) -> Result<Command, Refused> {
        let mut command = Command::new(&self.program);
        command.args(self.args(args)?).current_dir(&self.cwd);
        harden_environment(&mut command);
        command.stdin(Stdio::null());
        Ok(command)
    }

    /// Every program key of a filter or diff driver the repository
    /// configures (anything but the person's own global and system
    /// config), with the value that turns it off.
    fn driver_overrides(&self) -> Result<Vec<(String, &'static str)>, Refused> {
        let mut command = Command::new(&self.program);
        command
            .args(READ_ONLY_OPTIONS)
            .args(&self.location)
            .args([
                "config",
                "--show-scope",
                "-z",
                "--name-only",
                "--get-regexp",
                r"^(filter|diff)\.",
            ])
            .current_dir(&self.cwd)
            .stdin(Stdio::null())
            .stderr(Stdio::null());
        harden_environment(&mut command);
        let output = command
            .output()
            .map_err(|e| Refused(format!("cannot run git: {e}")))?;
        match output.status.code() {
            Some(0) => {}
            // No key matched.
            Some(1) => return Ok(Vec::new()),
            _ => {
                return Err(Refused(
                    "git could not list this repository's diff and filter drivers".into(),
                ));
            }
        }
        let mut fields = output.stdout.split(|&b| b == 0);
        let mut overrides = Vec::new();
        while let (Some(scope), Some(name)) = (fields.next(), fields.next()) {
            if matches!(scope, b"global" | b"system") {
                continue;
            }
            let name = String::from_utf8_lossy(name);
            for (prefix, suffix, value) in DRIVER_KEYS {
                let Some(driver) = name
                    .strip_prefix(prefix)
                    .and_then(|rest| rest.strip_suffix(suffix))
                else {
                    continue;
                };
                if driver.is_empty() {
                    continue;
                }
                if driver.contains(['=', '\n', '\0']) {
                    return Err(Refused(format!(
                        "this repository names a Git driver ({name}) that can't be turned off"
                    )));
                }
                let key = format!("{prefix}{driver}{suffix}");
                if !overrides.iter().any(|(k, _)| *k == key) {
                    overrides.push((key, *value));
                }
            }
        }
        Ok(overrides)
    }
}

/// Environment for a Git read: no pager, no prompt, no `askpass` program,
/// no external diff, and no optional locks. It also drops the variables
/// that point Git at another repository, so the driver listing and the
/// read itself see the same one: the caller chooses the repository with
/// the directory and [`ReadOnlyGit::location`].
pub fn harden_environment(command: &mut Command) {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_COMMON_DIR",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(name);
    }
    command
        .env("GIT_PAGER", "cat")
        .env("PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env_remove("GIT_ASKPASS")
        .env_remove("SSH_ASKPASS")
        .env_remove("GIT_EXTERNAL_DIFF");
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    /// A repository whose config names a program for every key a read
    /// could start: fsmonitor, an external diff, a diff driver's command
    /// and text conversion, a filter's clean, smudge, and process, a
    /// pager, a signature verifier, an `askpass`, and every hook. Each
    /// program appends its name to `marks` before it does its job.
    struct Hostile {
        _held: tempfile::TempDir,
        repo: PathBuf,
        marks: PathBuf,
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(["-c", "core.hooksPath=/dev/null"])
            .args(args)
            .current_dir(repo)
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn hostile() -> Hostile {
        let held = tempfile::tempdir().unwrap();
        let root = held.path().canonicalize().unwrap();
        let marks = root.join("marks");
        let bin = root.join("bin");
        std::fs::create_dir(&bin).unwrap();
        let program = |name: &str| -> String {
            let path = bin.join(name);
            std::fs::write(
                &path,
                format!(
                    "#!/bin/sh\necho {name} >> '{}'\n\
                     if [ $# -gt 0 ] && [ -f \"$1\" ]; then cat \"$1\"; else cat; fi\n",
                    marks.display()
                ),
            )
            .unwrap();
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
            path.display().to_string()
        };
        let repo = root.join("repo");
        std::fs::create_dir(&repo).unwrap();
        git(&repo, &["init", "-q"]);
        std::fs::write(repo.join("a.txt"), "a\n").unwrap();
        std::fs::write(repo.join("b.bin"), "x\n").unwrap();
        std::fs::write(repo.join("c.md"), "c\n").unwrap();
        std::fs::write(
            repo.join(".gitattributes"),
            "a.txt filter=evil\nb.bin diff=evil\n",
        )
        .unwrap();
        git(&repo, &["add", "-A"]);
        git(
            &repo,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "-qm",
                "base",
            ],
        );
        // An included file counts as the repository's own config too.
        let included = root.join("included.cfg");
        std::fs::write(
            &included,
            format!("[filter \"inc\"]\n\tclean = {}\n", program("included")),
        )
        .unwrap();
        for (key, value) in [
            ("core.fsmonitor", program("fsmonitor")),
            ("diff.external", program("external")),
            ("diff.evil.command", program("command")),
            ("diff.evil.textconv", program("textconv")),
            ("filter.evil.clean", program("clean")),
            ("filter.evil.smudge", program("smudge")),
            ("filter.evil.process", program("process")),
            ("filter.evil.required", "true".into()),
            ("core.pager", program("pager")),
            ("pager.status", program("pager")),
            ("core.askPass", program("askpass")),
            ("log.showSignature", "true".into()),
            ("gpg.program", program("gpg")),
            ("include.path", included.display().to_string()),
        ] {
            git(&repo, &["config", key, &value]);
        }
        std::fs::write(repo.join(".git/info/attributes"), "c.md filter=inc\n").unwrap();
        let hooks = repo.join(".git/hooks");
        std::fs::create_dir_all(&hooks).unwrap();
        for hook in ["post-checkout", "post-index-change", "fsmonitor-watchman"] {
            let target = program(hook);
            std::fs::copy(&target, hooks.join(hook)).unwrap();
        }
        // Dirty both files so status and diff have work to compare.
        std::fs::write(repo.join("a.txt"), "a\nb\n").unwrap();
        std::fs::write(repo.join("b.bin"), "x\ny\n").unwrap();
        std::fs::write(repo.join("c.md"), "c\nd\n").unwrap();
        Hostile {
            _held: held,
            repo,
            marks,
        }
    }

    fn ran(marks: &Path) -> String {
        std::fs::read_to_string(marks).unwrap_or_default()
    }

    /// The fixture really is hostile: plain Git runs its programs.
    #[test]
    fn plain_git_runs_the_programs_a_hostile_repository_names() {
        let fixture = hostile();
        for args in [&["status"][..], &["diff"], &["log", "-p", "-1"], &["show"]] {
            let _ = Command::new("git")
                .args(args)
                .current_dir(&fixture.repo)
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .stdin(Stdio::null())
                .output()
                .unwrap();
        }
        let marks = ran(&fixture.marks);
        // The filter fails its protocol and stops `git diff` before the
        // external diff, so this checks the programs that run first.
        for program in ["fsmonitor", "textconv", "process", "post-index-change"] {
            assert!(marks.contains(program), "{program} did not run: {marks}");
        }
    }

    #[test]
    fn read_only_git_runs_nothing_a_hostile_repository_names() {
        let fixture = hostile();
        let git = ReadOnlyGit::new(&fixture.repo);
        for args in [
            &["status"][..],
            &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
            &["diff"],
            &["diff", "HEAD"],
            &["diff", "--cached"],
            &["log", "-p", "-1"],
            &["log", "--oneline", "--graph", "--all", "-n", "5"],
            &["show"],
            &["show", "HEAD"],
            &["blame", "b.bin"],
            &["ls-files"],
            &["rev-parse", "HEAD"],
        ] {
            let output = git
                .command(args)
                .unwrap()
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .env("GIT_CONFIG_GLOBAL", "/dev/null")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "git {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                ran(&fixture.marks),
                "",
                "git {} ran a program",
                args.join(" ")
            );
        }
        // The reads still see the change.
        let diff = git.command(&["diff"]).unwrap().output().unwrap();
        assert!(String::from_utf8_lossy(&diff.stdout).contains("+b"));
    }

    #[test]
    fn a_driver_name_a_c_option_cant_carry_is_refused() {
        let fixture = hostile();
        git(
            &fixture.repo,
            &["config", "filter.a=b.smudge", "/bin/false"],
        );
        let refused = ReadOnlyGit::new(&fixture.repo).args(&["status"]);
        assert!(refused.is_err(), "{refused:?}");
    }

    #[test]
    fn diff_and_history_reads_get_the_options_that_stop_external_programs() {
        let dir = tempfile::tempdir().unwrap();
        let args = ReadOnlyGit::new(dir.path())
            .location(&["-C", "."])
            .args(&["diff", "--stat"])
            .unwrap();
        let at = args.iter().position(|a| a == "diff").unwrap();
        assert_eq!(
            &args[at..],
            ["diff", "--no-ext-diff", "--no-textconv", "--stat"]
        );
        assert!(args[..at].contains(&"core.fsmonitor=false".to_string()));
        assert_eq!(&args[at - 2..at], ["-C", "."]);
    }
}
