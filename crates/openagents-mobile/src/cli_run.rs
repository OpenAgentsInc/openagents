//! Running a read-only command a chat offer proposed on the person's
//! connected computer.
//!
//! The chat router may offer a read-only `openagents` command that runs on
//! a computer (`verse who`, `kb search …`). The person's **Run** tap is the
//! confirmation. The phone then checks, from typed state and never from
//! the words on screen, that the command is on its own read-only list
//! ([`crate::router::READ_ONLY`]), that a computer is ready, and that this
//! phone holds the computer's "Open terminals" right; then it runs
//! `openagents --json ARGV` there the way `openagents computer exec` does
//! (NIP-HOST `terminal.open` and NIP-TERM, through
//! `coder_computers::terminal::exec`), off the UI thread, for at most
//! [`TIMEOUT`]. The host checks the right again on every request.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use coder_computers::live::Terminals;
use coder_computers::terminal::exec::{self, Limits};
use coder_computers::{Computers, HostRecord};
use coder_host::access::Right;

use crate::coder_tab::{Availability, CliOutcome, availability};

/// How long the command may run on the computer.
pub(crate) const TIMEOUT: Duration = Duration::from_secs(20);
/// How long to wait for the computer to open a shell.
const WAIT: Duration = Duration::from_secs(10);
/// The most output lines a card shows: the last ones.
pub(crate) const MAX_LINES: usize = 40;
/// The most output bytes a card shows: the last ones.
pub(crate) const MAX_BYTES: usize = 4 * 1024;

/// What a command wrote on the computer and how it ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RemoteRun {
    pub output: String,
    pub exit: i32,
    pub timed_out: bool,
}

/// Runs a command on a computer. Blocks; the tab calls it off the UI
/// thread. The error is a plain sentence for the card.
pub(crate) trait RemoteCli: Send + Sync {
    fn run(&self, host: &str, command: &[String]) -> Result<RemoteRun, String>;
}

/// The live runner: the Computers service's current link to each host.
pub(crate) struct Live {
    terminals: Terminals,
    handle: tokio::runtime::Handle,
}

impl Live {
    pub(crate) fn new(terminals: Terminals, handle: tokio::runtime::Handle) -> Self {
        Self { terminals, handle }
    }
}

impl RemoteCli for Live {
    fn run(&self, host: &str, command: &[String]) -> Result<RemoteRun, String> {
        let limits = Limits {
            wait: WAIT,
            timeout: TIMEOUT,
            ..Limits::default()
        };
        exec::run(
            &self.handle,
            self.terminals.links(host),
            host,
            command,
            limits,
            1,
            &mut |_| {},
        )
        .map(|run| RemoteRun {
            output: run.output,
            exit: run.exit,
            timed_out: run.timed_out,
        })
        .map_err(|phase| phase.describe())
    }
}

/// The computer a proposed command may run on now, with its key and
/// label, or why it may not.
pub(crate) fn target(
    argv: &[String],
    computers: Option<&Computers>,
    selected: Option<&str>,
) -> Result<(String, String), CliOutcome> {
    if !crate::router::read_only(argv) {
        return Err(CliOutcome::Refused(
            "We only run read-only commands from the chat.".into(),
        ));
    }
    let Some(computers) = computers else {
        return Err(no_computer());
    };
    let snapshot = computers.snapshot();
    let host = match availability(snapshot, selected) {
        Availability::Ready(host) => host,
        Availability::Connecting(host) => {
            return Err(CliOutcome::Refused(format!(
                "{} is still connecting. Try again in a moment.",
                host.label
            )));
        }
        Availability::Offline(host) => {
            return Err(CliOutcome::Refused(format!(
                "{} is offline. Try again when it's back online.",
                host.label
            )));
        }
        Availability::NotConfigured => return Err(no_computer()),
    };
    if !terminal_allowed(host, snapshot.now) {
        return Err(CliOutcome::Refused(format!(
            "{} hasn't allowed terminal access from this phone, so we can't run this there.",
            host.label
        )));
    }
    Ok((host.key.clone(), host.label.clone()))
}

fn no_computer() -> CliOutcome {
    CliOutcome::Refused("Connect a computer to run this.".into())
}

/// Whether this phone holds `host`'s "Open terminals" right now.
fn terminal_allowed(host: &HostRecord, now: u64) -> bool {
    host.enrollment
        .rights(now)
        .is_some_and(|rights| rights.contains(Right::Terminal))
}

/// `openagents --json ARGV`.
pub(crate) fn command(argv: &[String]) -> Vec<String> {
    let mut command = vec!["openagents".to_owned(), "--json".to_owned()];
    command.extend(argv.iter().cloned());
    command
}

/// Starts `command` on `host` on its own thread and returns the slot its
/// outcome lands in; `ring` is called once it does.
pub(crate) fn spawn(
    runner: Arc<dyn RemoteCli>,
    host: String,
    label: String,
    command: Vec<String>,
    ring: fn(),
) -> Arc<Mutex<Option<CliOutcome>>> {
    let slot = Arc::new(Mutex::new(None));
    let landed = slot.clone();
    std::thread::spawn(move || {
        let outcome = outcome(&label, runner.run(&host, &command));
        *landed.lock().unwrap_or_else(|p| p.into_inner()) = Some(outcome);
        ring();
    });
    slot
}

/// What the card shows for a finished run.
pub(crate) fn outcome(label: &str, run: Result<RemoteRun, String>) -> CliOutcome {
    let run = match run {
        Ok(run) => run,
        Err(why) => return CliOutcome::Refused(why),
    };
    if run.timed_out {
        return CliOutcome::Refused(format!(
            "{label} didn't finish within {} seconds.",
            TIMEOUT.as_secs()
        ));
    }
    let mut lines = readable(&run.output);
    if run.exit != 0 {
        lines.push(format!("The command exited with code {}.", run.exit));
    }
    if lines.is_empty() {
        lines.push("The command printed nothing.".into());
    }
    CliOutcome::Output(lines)
}

/// The output as lines: JSON pretty-printed when it parses, else as
/// written; the last [`MAX_LINES`] lines within [`MAX_BYTES`].
pub(crate) fn readable(output: &str) -> Vec<String> {
    let trimmed = output.trim();
    let text = serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| trimmed.to_owned());
    let mut kept: Vec<String> = Vec::new();
    let mut bytes = 0;
    for line in text.lines().rev() {
        if kept.len() == MAX_LINES || bytes + line.len() > MAX_BYTES {
            break;
        }
        bytes += line.len();
        kept.push(line.to_owned());
    }
    kept.reverse();
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> Vec<String> {
        line.split(' ').map(str::to_owned).collect()
    }

    #[test]
    fn only_listed_read_only_commands_run() {
        for argv in ["wallet send 10", "computer approve host", "verse say hi"] {
            assert_eq!(
                target(&words(argv), None, None),
                Err(CliOutcome::Refused(
                    "We only run read-only commands from the chat.".into()
                )),
                "{argv}"
            );
        }
        assert_eq!(
            target(&words("verse who"), None, None),
            Err(CliOutcome::Refused("Connect a computer to run this.".into()))
        );
    }

    #[test]
    fn the_command_is_openagents_json() {
        assert_eq!(
            command(&words("kb search relays")),
            words("openagents --json kb search relays")
        );
    }

    #[test]
    fn json_output_is_pretty_printed_and_bounded() {
        assert_eq!(
            readable("{\"who\":[\"ada\"]}\n"),
            ["{", "  \"who\": [", "    \"ada\"", "  ]", "}"]
        );
        let long: String = (0..100).map(|n| format!("line {n}\n")).collect();
        let lines = readable(&long);
        assert_eq!(lines.len(), MAX_LINES);
        assert_eq!(lines.last().map(String::as_str), Some("line 99"));
        let wide = "x".repeat(3000);
        assert_eq!(readable(&format!("{wide}\n{wide}")).len(), 1);
    }

    #[test]
    fn a_finished_run_reads_plainly() {
        let ran = |output: &str, exit, timed_out| {
            Ok(RemoteRun {
                output: output.into(),
                exit,
                timed_out,
            })
        };
        assert_eq!(
            outcome("Studio", ran("ok", 0, false)),
            CliOutcome::Output(vec!["ok".into()])
        );
        assert_eq!(
            outcome("Studio", ran("", 2, false)),
            CliOutcome::Output(vec!["The command exited with code 2.".into()])
        );
        assert_eq!(
            outcome("Studio", ran("", 124, true)),
            CliOutcome::Refused("Studio didn't finish within 20 seconds.".into())
        );
        assert_eq!(
            outcome("Studio", Err("The computer refused.".into())),
            CliOutcome::Refused("The computer refused.".into())
        );
    }
}
