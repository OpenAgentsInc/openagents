//! A command run on another computer in a checkout at one pushed commit
//! (#10767).
//!
//! [`Job::run`] sends a fixed `sh` script that keeps a bare clone of the
//! repository under `~/.openagents/remote-runs/NAME/repo.git`, fetches the
//! commit when the clone lacks it, adds a detached worktree for it at
//! `~/.openagents/remote-runs/NAME/COMMIT`, and runs the command there with
//! `CARGO_TARGET_DIR` defaulting to `~/.openagents/remote-runs/NAME/target`,
//! one target directory every commit's checkout shares so builds stay warm.
//! The command's output streams to wherever the caller points it.
//! [`Job::fetch`] copies one result file back. [`reachable`] asks whether a
//! computer answers at all.

use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use supervise::Ending;
use supervise::blocking;

use crate::error::Error;
use crate::ssh::{Destination, Forwarding, Ssh, Stdin, quote};

/// How long [`reachable`] waits for a computer to answer.
const PROBE_WALL: Duration = Duration::from_secs(15);

/// How long a job may run: it has no deadline of its own.
const RUN_WALL: Duration = Duration::from_secs(10 * 365 * 86_400);

/// How long one result file may take to copy back.
const FETCH_WALL: Duration = Duration::from_secs(30 * 60);

/// The exit code the script uses when the checkout can't be prepared, so
/// the command never ran.
pub const SETUP_FAILED: i32 = 125;

/// The checkout script. Its arguments are the repository, the commit, the
/// checkout name, and then the command.
const RUN: &str = r#"set -u
fail() { printf 'openagents remote run: %s\n' "$1" >&2; exit 125; }
url=$1; commit=$2; name=$3; shift 3
command -v git >/dev/null 2>&1 || fail "git is not installed on this computer"
base="$HOME/.openagents/remote-runs/$name"
mkdir -p "$base" || fail "cannot create $base"
if [ ! -d "$base/repo.git" ]; then
  rm -rf "$base/repo.git.partial"
  git clone --quiet --bare "$url" "$base/repo.git.partial" || fail "cannot clone $url"
  mv "$base/repo.git.partial" "$base/repo.git" || fail "cannot keep the clone"
fi
repo="$base/repo.git"
if ! git -C "$repo" cat-file -e "$commit^{commit}" 2>/dev/null; then
  git -C "$repo" fetch --quiet "$url" '+refs/heads/*:refs/heads/*' >&2 || true
fi
if ! git -C "$repo" cat-file -e "$commit^{commit}" 2>/dev/null; then
  git -C "$repo" fetch --quiet "$url" "$commit" >&2 || fail "$url has no commit $commit; push it first"
fi
dir="$base/$commit"
if [ ! -e "$dir/.git" ]; then
  git -C "$repo" worktree prune >/dev/null 2>&1 || true
  git -C "$repo" worktree add --quiet --detach "$dir" "$commit" >&2 || fail "cannot check out $commit"
fi
cd "$dir" || fail "cannot enter $dir"
CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$base/target}
OPENAGENTS_PLACEMENT=remote
export CARGO_TARGET_DIR OPENAGENTS_PLACEMENT
exec "$@"
"#;

/// The fetch script. Its arguments are the checkout name, the commit, and
/// the file's path in the checkout.
const FETCH: &str = r#"set -u
cd "$HOME/.openagents/remote-runs/$1/$2" || exit 2
exec cat -- "$3"
"#;

/// Whether `destination` answers a no-op command over `ssh` (`program`, or
/// `ssh` from `PATH`) in batch mode within 15 seconds.
#[must_use]
pub fn reachable(destination: &str, program: Option<&Path>) -> bool {
    let Ok(destination) = Destination::parse(destination) else {
        return false;
    };
    let ssh = Ssh {
        program: program.map_or_else(|| PathBuf::from("ssh"), Path::to_path_buf),
        destination,
        prompter: None,
    };
    ssh.run("true", Stdin::Bytes(Vec::new()), PROBE_WALL, "the probe")
        .is_ok_and(|output| output.ending == Ending::Exited(Some(0)))
}

/// One command at one commit on one computer.
#[derive(Clone)]
pub struct Job {
    ssh: Ssh,
    repository: String,
    commit: String,
    name: String,
    command: Vec<String>,
}

impl std::fmt::Debug for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Job")
            .field("destination", &self.ssh.destination)
            .field("repository", &self.repository)
            .field("commit", &self.commit)
            .field("command", &self.command)
            .finish_non_exhaustive()
    }
}

impl Job {
    /// A job that runs `command` on `destination` in a checkout of
    /// `repository` (a URL `git clone` takes there) at `commit`.
    ///
    /// # Errors
    /// The destination could be read as an option, the repository is empty
    /// or starts with `-`, the commit isn't a full hexadecimal object name,
    /// or the command is empty.
    pub fn new(
        destination: &str,
        repository: &str,
        commit: &str,
        command: Vec<String>,
    ) -> Result<Self, Error> {
        let destination = Destination::parse(destination)?;
        if repository.is_empty()
            || repository.starts_with('-')
            || repository.chars().any(char::is_control)
        {
            return Err(Error::InvalidRunner(format!(
                "`{repository}` is not a repository URL"
            )));
        }
        if !matches!(commit.len(), 40 | 64) || !commit.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(Error::InvalidRunner(format!(
                "`{commit}` is not a full commit name"
            )));
        }
        if command.is_empty() {
            return Err(Error::InvalidRunner("the command is empty".into()));
        }
        Ok(Job {
            ssh: Ssh {
                program: PathBuf::from("ssh"),
                destination,
                prompter: None,
            },
            name: checkout_name(repository),
            repository: repository.to_owned(),
            commit: commit.to_ascii_lowercase(),
            command,
        })
    }

    /// Runs this `ssh` program instead of the one on `PATH`.
    #[must_use]
    pub fn program(mut self, program: impl Into<PathBuf>) -> Self {
        self.ssh.program = program.into();
        self
    }

    /// The checkout on the remote computer, relative to its home.
    #[must_use]
    pub fn remote_dir(&self) -> String {
        format!("~/.openagents/remote-runs/{}/{}", self.name, self.commit)
    }

    /// Runs the command, with its output going to `stdout` and `stderr`,
    /// and returns its exit code: the remote command's, [`SETUP_FAILED`]
    /// when the checkout couldn't be prepared, or 255 when `ssh` itself
    /// failed.
    ///
    /// # Errors
    /// `ssh` could not be started or ended without an exit code.
    pub fn run(&self, stdout: Stdio, stderr: Stdio) -> Result<i32, Error> {
        let mut remote = format!(
            "sh -c {} oa-remote-run {} {} {}",
            quote(RUN),
            quote(&self.repository),
            quote(&self.commit),
            quote(&self.name)
        );
        for word in &self.command {
            remote.push(' ');
            remote.push_str(&quote(word));
        }
        let (mut command, askpass) = self.ssh.command(&Forwarding::None, Some(&remote))?;
        // This machine's target directory means nothing there, and an SSH
        // configuration that sends the environment must not carry it.
        command
            .env_remove("CARGO_TARGET_DIR")
            .stdin(Stdio::null())
            .stdout(stdout)
            .stderr(stderr);
        blocking::own_group(&mut command);
        let mut child = command
            .spawn()
            .map_err(|error| Error::Spawn(format!("{}: {error}", self.ssh.program.display())))?;
        let ending = blocking::wait(&mut child, RUN_WALL);
        drop(askpass);
        match ending {
            Ending::Exited(Some(code)) => Ok(code),
            Ending::Exited(None) => Err(Error::Ssh {
                code: None,
                detail: "ssh ended on a signal".into(),
            }),
            Ending::TimedOut => Err(Error::TimedOut("the remote run")),
            Ending::Failed(why) => Err(Error::Spawn(why)),
        }
    }

    /// Copies `path`, relative to the checkout, back to `to`, replacing it
    /// only once the whole file arrived.
    ///
    /// # Errors
    /// `path` leaves the checkout, or the file couldn't be read there or
    /// written here.
    pub fn fetch(&self, path: &str, to: &Path) -> Result<(), Error> {
        let relative = Path::new(path);
        if path.is_empty()
            || !relative
                .components()
                .all(|part| matches!(part, Component::Normal(_)))
        {
            return Err(Error::InvalidRunner(format!(
                "`{path}` is not a path inside the checkout"
            )));
        }
        if let Some(parent) = to.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(Error::Io)?;
        }
        let partial = to.with_extension(match to.extension() {
            Some(extension) => format!("{}.partial", extension.to_string_lossy()),
            None => "partial".to_owned(),
        });
        let file = std::fs::File::create(&partial).map_err(Error::Io)?;
        let remote = format!(
            "sh -c {} oa-remote-fetch {} {} {}",
            quote(FETCH),
            quote(&self.name),
            quote(&self.commit),
            quote(path)
        );
        let (mut command, askpass) = self.ssh.command(&Forwarding::None, Some(&remote))?;
        command
            .stdin(Stdio::null())
            .stdout(Stdio::from(file))
            .stderr(Stdio::piped());
        blocking::own_group(&mut command);
        let result = fetch_into(&mut command, &self.ssh.program);
        drop(askpass);
        match result {
            Ok(()) => std::fs::rename(&partial, to).map_err(Error::Io),
            Err(error) => {
                let _ = std::fs::remove_file(&partial);
                Err(error)
            }
        }
    }
}

fn fetch_into(command: &mut Command, program: &Path) -> Result<(), Error> {
    let mut child = command
        .spawn()
        .map_err(|error| Error::Spawn(format!("{}: {error}", program.display())))?;
    let mut stderr = child.stderr.take();
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        if let Some(stream) = stderr.as_mut() {
            let _ = std::io::Read::read_to_string(stream, &mut text);
        }
        text
    });
    let ending = blocking::wait(&mut child, FETCH_WALL);
    let detail = reader.join().unwrap_or_default();
    match ending {
        Ending::Exited(Some(0)) => Ok(()),
        Ending::Exited(code) => Err(Error::Ssh {
            code,
            detail: detail.trim().chars().take(600).collect(),
        }),
        Ending::TimedOut => Err(Error::TimedOut("copying a result file")),
        Ending::Failed(why) => Err(Error::Spawn(why)),
    }
}

/// A directory name for the repository's checkouts: the last part of its
/// URL without `.git`, kept to letters, digits, `.`, `_`, and `-`.
fn checkout_name(repository: &str) -> String {
    let last = repository
        .trim_end_matches('/')
        .rsplit(['/', ':'])
        .next()
        .unwrap_or_default()
        .trim_end_matches(".git");
    let name: String = last
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
        .collect();
    if name.is_empty() || name.starts_with('.') {
        "repo".to_owned()
    } else {
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checkouts_are_named_after_the_repository() {
        assert_eq!(
            checkout_name("https://github.com/OpenAgentsInc/openagents.git"),
            "openagents"
        );
        assert_eq!(checkout_name("git@github.com:me/thing"), "thing");
        assert_eq!(checkout_name("/srv/git/origin.git/"), "origin");
        assert_eq!(checkout_name("/srv/.."), "repo");
    }

    #[test]
    fn a_job_refuses_unsafe_inputs() {
        let commit = "a".repeat(40);
        let command = vec!["true".to_owned()];
        assert!(Job::new("box", "https://x/r.git", &commit, command.clone()).is_ok());
        assert!(Job::new("-oProxyCommand=x", "https://x/r", &commit, command.clone()).is_err());
        assert!(Job::new("box", "--upload-pack=x", &commit, command.clone()).is_err());
        assert!(Job::new("box", "https://x/r", "HEAD", command.clone()).is_err());
        assert!(Job::new("box", "https://x/r", &commit, Vec::new()).is_err());
        let job = Job::new("box", "https://x/r", &commit, command).unwrap();
        assert!(job.fetch("../secret", Path::new("x")).is_err());
        assert!(job.fetch("/etc/passwd", Path::new("x")).is_err());
        assert!(job.fetch("", Path::new("x")).is_err());
    }
}
