//! Alice's task mode on a connected computer (#10930).
//!
//! When her policy names computers (`agents/NAME/policy.json` `computers`),
//! a task-mode request may run its coding on one of them instead of this
//! host. The remote lane talks to the computer through the owner's device
//! grant and the installed `openagents` CLI (`openagents computer`), which
//! already carries `task.create`, `task.review`, `task.cancel`, and
//! `host.exec` over NIP-HOST; this module never holds a credential and
//! never asks the computer for a Git one.
//!
//! - **Placement.** `auto` picks the first computer with a free slot under
//!   its policy cap, else this host; `local` and a computer's name pin the
//!   placement. A computer that is off or cannot take a task is reported
//!   once and the work falls back to this host or stays queued.
//! - **The task.** The remote task runs on the computer's own auto-start
//!   policy with the typed `devin` engine preference, in a worktree of its
//!   registered checkout at the exact commit this host's checkout had when
//!   the request was taken. A commit that is not pushed is refused before
//!   anything is created.
//! - **The change.** When the remote task completes, its diff comes back
//!   (`git diff --binary BASE` over `host.exec`), applies to the local
//!   studio worktree `submit_remote` prepared, commits on the task's
//!   branch, and the studio entry moves to `Stage::Merge` — the Mac's
//!   Merge station reads and merges it like a local task.
//! - **Cancel.** `agent stop` and F7 cancel the remote task through the
//!   same grant, which stops its Devin session too.

use std::process::Stdio;
use std::time::{Duration, Instant};

/// How long one remote call may take before the computer counts as away.
/// Longer than the `--wait` the calls pass plus a call's own work, so the
/// CLI's refusal reaches the lane instead of a silent kill.
const CALL_LIMIT: Duration = Duration::from_secs(90);

/// The phase a remote task is in, as the lane last read it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    /// The lane could not establish the phase: the computer is away, or the
    /// task is not known there.
    Unknown,
}

impl Phase {
    /// Whether the task ended: no further polling changes it.
    #[must_use]
    pub fn ended(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }
}

/// A remote task's dispatch.
#[derive(Clone, Debug)]
pub struct Brief {
    /// The task's title, at most a line.
    pub title: String,
    /// The whole prompt: the brief plus where it must leave its change.
    pub prompt: String,
    /// The host-scoped workspace label the remote host resolves.
    pub workspace: String,
    /// The commit the remote worktree starts from.
    pub base: String,
}

/// What a landed remote task's change is carried back as.
#[derive(Clone, Debug)]
pub struct Review {
    /// The commit the remote worktree started from.
    pub base: String,
    /// The unified diff against `base`, binary-safe (`--binary`).
    pub diff: String,
}

/// The computer side of her remote work: one device grant, no credentials
/// of its own. The live lane is [`Cli`]; tests drive a scripted one.
pub trait Remote: Send {
    /// Whether `name` is enrolled, switched on, and reachable enough to
    /// take work; or the one plain sentence why not.
    fn ready(&mut self, name: &str) -> Result<(), String>;
    /// Ensure `name`'s checkout of the workspace holds `base`: clone the
    /// checkout from `origin` when the computer has none, fetch `base`
    /// otherwise. Refuses when the commit is not pushed or Git cannot be
    /// reached.
    fn ensure(
        &mut self,
        name: &str,
        checkout: &str,
        origin: &str,
        base: &str,
    ) -> Result<(), String>;
    /// Create the task `brief` describes; returns its remote task id.
    fn create(&mut self, name: &str, brief: &Brief) -> Result<String, String>;
    /// `name`'s phase for `task`, `Unknown` when unreadable.
    fn phase(&mut self, name: &str, task: &str) -> Phase;
    /// `task`'s change as `base` plus the whole diff, once it completed.
    /// `checkout` is the computer's checkout the task ran in.
    fn review(&mut self, name: &str, task: &str, checkout: &str) -> Result<Review, String>;
    /// Cancel `task`; a missing or ended task answers `Ok`.
    fn cancel(&mut self, name: &str, task: &str) -> Result<(), String>;
}

/// `words` as one subprocess call, bounded by `CALL_LIMIT`.
fn output(command: &mut std::process::Command) -> Result<std::process::Output, String> {
    let mut child = command.spawn().map_err(|error| error.to_string())?;
    let began = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                return child.wait_with_output().map_err(|error| error.to_string());
            }
            Ok(None) if began.elapsed() < CALL_LIMIT => {
                std::thread::sleep(Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the call timed out".into());
            }
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// Where a lane keeps what it reads between calls.
fn home() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(std::path::PathBuf::from)
}

/// A lane that shells out to `openagents computer`, the owner's own
/// enrolled device, for every remote effect. The binary is `OPENAGENTS_BIN`,
/// `~/.openagents/bin/openagents`, or `openagents` on `PATH`, whichever
/// resolves first.
pub struct Cli {
    binary: std::path::PathBuf,
}

impl Cli {
    /// A lane over `binary`; `None` finds the installed `openagents`.
    #[must_use]
    pub fn new(binary: Option<std::path::PathBuf>) -> Self {
        let binary = binary.unwrap_or_else(|| {
            std::env::var_os("OPENAGENTS_BIN")
                .map(std::path::PathBuf::from)
                .or_else(|| home().map(|home| home.join(".openagents/bin/openagents")))
                .filter(|path| path.exists())
                .unwrap_or_else(|| std::path::PathBuf::from("openagents"))
        });
        Self { binary }
    }

    /// `openagents computer ... --wait 60 --json`; the answer's stdout as
    /// JSON. The wider wait rides out a linked computer's connecting
    /// windows; the default 15 seconds refuses inside them.
    fn json(&self, words: &[&str]) -> Result<serde_json::Value, String> {
        let mut command = std::process::Command::new(&self.binary);
        command
            .arg("computer")
            .args(words)
            .arg("--wait")
            .arg("60")
            .arg("--json")
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped());
        output(&mut command).and_then(|ran| {
            if ran.status.success() {
                serde_json::from_slice(&ran.stdout)
                    .map_err(|why| format!("`computer {}` answered oddly: {why}", words[0]))
            } else {
                let error = String::from_utf8_lossy(&ran.stderr);
                Err(format!(
                    "`openagents computer {}` refused: {}",
                    words[0],
                    error.trim()
                ))
            }
        })
    }

    /// `openagents computer exec NAME -- WORDS`, as plain stdout.
    fn exec(&self, name: &str, words: &[&str]) -> Result<String, String> {
        let mut command = std::process::Command::new(&self.binary);
        // The relay handshake takes most of the default 15-second attach
        // wait, so every script on a linked computer needs a wider one.
        command
            .arg("computer")
            .arg("exec")
            .arg(name)
            .arg("--wait")
            .arg("60")
            .arg("--")
            .args(words)
            .stdin(Stdio::null())
            .stderr(Stdio::piped())
            .stdout(Stdio::piped());
        output(&mut command).and_then(|ran| {
            if ran.status.success() {
                Ok(String::from_utf8_lossy(&ran.stdout).into_owned())
            } else {
                // `exec`'s exit is the remote command's; the remote's own
                // output lands on stdout, stderr holds the CLI's words —
                // report whichever names the refusal.
                let error = String::from_utf8_lossy(&ran.stderr);
                let out = String::from_utf8_lossy(&ran.stdout);
                let detail = if error.trim().is_empty() {
                    let tail = out.trim().lines().rev().take(4).collect::<Vec<_>>();
                    tail.into_iter().rev().collect::<Vec<_>>()
                } else {
                    error.trim().lines().collect::<Vec<_>>()
                }
                .join(" ");
                Err(format!(
                    "{name}'s `{}` exited {}: {}",
                    words[0],
                    ran.status.code().unwrap_or(-1),
                    detail
                ))
            }
        })
    }
}

impl Remote for Cli {
    fn ready(&mut self, name: &str) -> Result<(), String> {
        // `computer workspaces` answers only while the host is connected
        // and the grant's `workspaces.read` holds.
        self.json(&["workspaces", name]).map(|_| ()).map_err(|why| {
            if why.contains("not connected") || why.contains("unreachable") {
                format!("{name} is not connected")
            } else {
                why
            }
        })
    }

    fn ensure(
        &mut self,
        name: &str,
        checkout: &str,
        origin: &str,
        base: &str,
    ) -> Result<(), String> {
        // Clone the checkout when the computer has none, fetch `base`
        // otherwise. A commit that is not pushed fails both fetches, so the
        // cat-file check below refuses it in words.
        // `${checkout#'~/'}` quotes the pattern: an unquoted `~` tilde-
        // expands inside `${x#p}` under bash and the strip misses.
        let script = format!(
            "checkout='{checkout}'; \
             case \"$checkout\" in '~/'*) checkout=\"$HOME/${{checkout#'~/'}}\";; esac; \
             if ! git -C \"$checkout\" rev-parse --git-dir >/dev/null 2>&1; then \
               mkdir -p \"$(dirname \"$checkout\")\" && \
               git clone -q '{origin}' \"$checkout\" || exit 2; \
             fi; \
             git -C \"$checkout\" fetch -q origin '{base}' 2>/dev/null || \
             git -C \"$checkout\" fetch -q origin || exit 3; \
             git -C \"$checkout\" cat-file -e '{base}^{{commit}}' && \
             git -C \"$checkout\" reset --hard -q '{base}' && \
             git -C \"$checkout\" clean -fdq"
        );
        self.exec(name, &["sh", "-c", &script])
            .map(|_| ())
            .map_err(|why| {
                // The script's remote exits: 3 the fetch, 128 the absent
                // base — both mean the commit is not on the remote's
                // `origin`; 2 is the clone refusing. Every other failure
                // is the exec itself: the computer's own words say why.
                if why.contains("exited 3") || why.contains("exited 128") {
                    format!("the starting commit is not pushed ({why}), so {name} cannot fetch it")
                } else {
                    why
                }
            })
    }

    fn create(&mut self, name: &str, brief: &Brief) -> Result<String, String> {
        let answer = self.json(&[
            "task",
            name,
            &brief.prompt,
            "--workspace",
            &brief.workspace,
            "--title",
            &brief.title,
            "--engine",
            "devin",
        ])?;
        answer["task"]
            .as_str()
            .map(str::to_owned)
            .ok_or_else(|| "the computer answered no task identity".to_string())
    }

    fn phase(&mut self, name: &str, task: &str) -> Phase {
        // The host's task file says `status` and `execution`; the compact
        // record's words map one to one.
        // The task file is one compact line; `intent_digest` and `intent`
        // anchor the task's own fields past the ones its prompt text may
        // quote.
        let script = format!(
            "record=\"$HOME/.openagents/tasks/task/{task}.json\"; \
             test -f \"$record\" || {{ echo 'unknown'; exit 0; }}; \
             sed -n 's/.*\"intent_digest\":\"[^\"]*\",\"status\":\"\\([^\"]*\\)\",\"execution\":\"\\([^\"]*\\)\".*/\\1:\\2/p' \"$record\""
        );
        match self
            .exec(name, &["sh", "-c", &script])
            .map(|out| out.trim().to_owned())
        {
            Ok(words) => match words.as_str() {
                "queued:not_started" | "queued:unknown" | "queued:" => Phase::Queued,
                words if words.starts_with("cancelled") => Phase::Cancelled,
                words if words.starts_with("running") || words.starts_with("cancel_requested") => {
                    Phase::Running
                }
                "finished:finished" => Phase::Completed,
                words if words.starts_with("finished") => Phase::Failed,
                _ => Phase::Unknown,
            },
            Err(_) => Phase::Unknown,
        }
    }

    fn review(&mut self, name: &str, task: &str, checkout: &str) -> Result<Review, String> {
        let _ = task;
        // A remote task writes in the checkout `ensure` pinned to `base`;
        // `git diff --binary` against the pin carries the whole change back.
        let script = format!(
            "checkout='{checkout}'; \
             case \"$checkout\" in '~/'*) checkout=\"$HOME/${{checkout#'~/'}}\";; esac; \
             test -d \"$checkout\" || {{ echo 'no such checkout' >&2; exit 4; }}; \
             base=\"$(git -C \"$checkout\" rev-parse HEAD)\" || exit 5; \
             git -C \"$checkout\" add -A && \
             echo \"BASE=$base\"; git -C \"$checkout\" diff --cached --binary HEAD"
        );
        let out = self.exec(name, &["sh", "-c", &script])?;
        let Some(base) = out
            .lines()
            .next()
            .and_then(|line| line.strip_prefix("BASE="))
            .map(str::to_owned)
        else {
            return Err("the computer's diff had no base".into());
        };
        let diff = out
            .strip_prefix(&format!("BASE={base}\n"))
            .unwrap_or(&out)
            .to_owned();
        if diff.trim().is_empty() {
            return Err("the task changed nothing".into());
        }
        Ok(Review { base, diff })
    }

    fn cancel(&mut self, name: &str, task: &str) -> Result<(), String> {
        // The wire's revision is the task's current one; a stale number is
        // refused, so read it first. A missing record is already gone.
        let script = format!(
            "record=\"$HOME/.openagents/tasks/task/{task}.json\"; \
             test -f \"$record\" || {{ echo 'gone'; exit 0; }}; \
             sed -n 's/.*\"revision\":\\([0-9]*\\),\"intent\".*/\\1/p' \"$record\""
        );
        let revision = self.exec(name, &["sh", "-c", &script])?;
        let revision = revision.trim();
        if revision == "gone" {
            return Ok(());
        }
        if revision.is_empty() || !revision.bytes().all(|b| b.is_ascii_digit()) {
            return Err(format!("{name}'s record of {task} is unreadable"));
        }
        self.json(&["cancel", name, task, "--revision", revision])
            .map(|_| ())
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::collections::BTreeMap;
    use std::sync::{Arc, Mutex};

    /// A remote lane a test scripts: each computer's readiness, each
    /// created task's phases in order, and the review a completed task
    /// returns. Its fields are shared, so the test keeps a handle beside
    /// the boxed lane.
    #[derive(Default)]
    pub(crate) struct Fake {
        /// Computers that answer `ready` badly.
        pub(crate) offline: Arc<Mutex<Vec<String>>>,
        /// Remote task ids a test gave its phases, in poll order.
        pub(crate) phases: Arc<Mutex<BTreeMap<String, Vec<Phase>>>>,
        /// What `review` answers, by remote task id.
        pub(crate) reviews: Arc<Mutex<BTreeMap<String, Review>>>,
        /// Every call, in order.
        pub(crate) calls: Arc<Mutex<Vec<String>>>,
        /// Tasks created: (computer, brief).
        pub(crate) created: Arc<Mutex<Vec<(String, Brief)>>>,
        /// The remote task ids the lane hands out, in order.
        pub(crate) issued: Arc<Mutex<Vec<String>>>,
    }

    impl Fake {
        pub(crate) fn new() -> Self {
            Self::default()
        }

        /// The id the next `create` answers.
        pub(crate) fn issue(&mut self, id: &str) {
            self.issued.lock().unwrap().push(id.to_string());
        }
    }

    impl Remote for Fake {
        fn ready(&mut self, name: &str) -> Result<(), String> {
            self.calls.lock().unwrap().push(format!("ready {name}"));
            let offline = self.offline.lock().unwrap();
            if offline.iter().any(|off| off == name) {
                Err(format!("{name} is not connected"))
            } else {
                Ok(())
            }
        }

        fn ensure(
            &mut self,
            name: &str,
            checkout: &str,
            _origin: &str,
            base: &str,
        ) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("ensure {name} {checkout} {base}"));
            Ok(())
        }

        fn create(&mut self, name: &str, brief: &Brief) -> Result<String, String> {
            self.created
                .lock()
                .unwrap()
                .push((name.to_string(), brief.clone()));
            Ok(self
                .issued
                .lock()
                .unwrap()
                .get(self.created.lock().unwrap().len() - 1)
                .cloned()
                .unwrap_or_else(|| {
                    format!("remote-{}-{}", name, self.created.lock().unwrap().len())
                }))
        }

        fn phase(&mut self, name: &str, task: &str) -> Phase {
            self.calls
                .lock()
                .unwrap()
                .push(format!("phase {name} {task}"));
            let mut phases = self.phases.lock().unwrap();
            match phases.get_mut(task) {
                Some(list) if list.len() > 1 => list.remove(0),
                Some(list) => list[0],
                None => Phase::Running,
            }
        }

        fn review(&mut self, name: &str, task: &str, checkout: &str) -> Result<Review, String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("review {name} {task}"));
            self.reviews
                .lock()
                .unwrap()
                .remove(task)
                .ok_or_else(|| "no scripted review".into())
        }

        fn cancel(&mut self, name: &str, task: &str) -> Result<(), String> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("cancel {name} {task}"));
            Ok(())
        }
    }
}
