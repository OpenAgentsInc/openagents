//! `openagents lease`: run a command under a lease from the host resource
//! broker (`crates/coder-lease`), list holders and waiters, and grant or
//! revoke the real screen. `docs/coder/runtime/leases.md` is the guide.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder_lease::{
    Blocked, Broker, Error, Grant, Holder, Priority, Queued, Request, Resource, State, Wait,
    grant_refusal,
};
use serde_json::{Value, json};

use crate::Output;

pub(crate) const USAGE: &str =
    "usage: openagents lease RESOURCE [--amount N] [--priority P] [--no-wait] [--receipt PATH] -- CMD [ARGS...]
       openagents lease build [--keep-target-dir] [--priority P] [--no-wait] [--receipt PATH] -- CMD [ARGS...]
  list           Every lease held and waiting: resource, each waiter's place in
                 the queue, priority, holder session, agent, process, command
                 name, and how long it has waited or been held, with the build
                 count, memory budget, disk floor, and aging step.
  grant screen [--for DURATION] [--to SESSION]
                 Let agents take the real screen for DURATION (default 1h),
                 or only the session SESSION. Asks you to confirm on this
                 terminal, and refuses when standard input is not a terminal
                 or an agent runs the command.
  revoke screen  End the screen grant; new screen leases are refused.
Run CMD while holding a lease on RESOURCE, and exit with CMD's status.
RESOURCE is build, memory, disk, quiet, screen, browser, gpu, unreal,
blender, artifact/NAME, or issue/N. memory (GiB) and disk (GB) need
--amount. A request that can't be admitted waits its turn in a priority
queue: owner, push, normal (the default), then background, first in, first
out within a priority. --priority P sets it; OPENAGENTS_LEASE_PRIORITY sets
the default for a session. A waiter rises one level for each 20 minutes it
waits (OPENAGENTS_LEASE_AGING_MINUTES; 0 turns aging off). --no-wait fails at
once instead. quiet waits for running builds to finish, and new builds wait
at every priority while quiet is held or queued. CMD gets
OPENAGENTS_LEASE_ID, OPENAGENTS_LEASES, and OPENAGENTS_SESSION; a command
already under a lease on RESOURCE runs without taking another. On release,
a receipt lands in ~/.openagents/leases/receipts/ (OPENAGENTS_LEASE_ROOT
moves the root), and --receipt PATH writes a copy; with --json it is
printed after CMD's output.
build also takes one of Coder's target slots for the current repository
(~/.openagents/targets, shared with Coder tasks and capped at 25 GB) and
sets CARGO_TARGET_DIR to it; --keep-target-dir keeps a CARGO_TARGET_DIR
already set. Below the free-space floor (10 GB, OPENAGENTS_SLOT_FREE_GB) it
refuses and names the reclaim command.";

/// What each command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("", Effect::LongRunning),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("grant screen", Effect::Grants),
    Declared::computer("revoke screen", Effect::Grants),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some(first) = words.first() else {
        return output.usage("lease", "a resource or command is required", USAGE);
    };
    match first.as_str() {
        "help" | "-h" | "--help" => {
            println!("{USAGE}");
            0
        }
        "list" => list(output, &words[1..]),
        "grant" => grant(output, &words[1..]),
        "revoke" => revoke(output, &words[1..]),
        _ => hold(output, words),
    }
}

fn broker(output: &Output) -> Result<Broker, u8> {
    Broker::from_env().map_err(|error| output.fail("lease", &error.to_string()))
}

fn list(output: &Output, words: &[String]) -> u8 {
    if let Some(word) = words.first() {
        return output.usage("lease", &format!("unexpected argument `{word}`"), USAGE);
    }
    let broker = match broker(output) {
        Ok(broker) => broker,
        Err(code) => return code,
    };
    let leases = match broker.queue() {
        Ok(leases) => leases,
        Err(error) => return output.fail("lease", &error.to_string()),
    };
    let grant = match broker.grant_of(&Resource::Screen) {
        Ok(grant) => grant,
        Err(error) => return output.fail("lease", &error.to_string()),
    };
    let now = coder_lease::now_ms();
    let value = json!({
        "root": broker.root().display().to_string(),
        "limits": broker.limits(),
        "aging_minutes": broker.aging().map_or(0, |aging| aging.as_secs() / 60),
        "leases": leases,
        "screen_grant": grant.map(|grant| {
            let active = now < grant.expires_at_ms;
            json!({ "grant": grant, "active": active })
        }),
    });
    output.emit(&value, |_| render_list(&broker, &leases, &value, now));
    0
}

fn render_list(broker: &Broker, leases: &[Queued], value: &Value, now: u64) -> String {
    let limits = broker.limits();
    let aging = broker.aging().map_or_else(
        || "waiters don't age".to_owned(),
        |aging| {
            format!(
                "a waiter rises one priority level every {}",
                span(u64::try_from(aging.as_millis()).unwrap_or(u64::MAX))
            )
        },
    );
    let mut lines = vec![format!(
        "Limits: {} build slots, {} GiB of memory, a {} GB disk floor; {aging}.",
        limits.build, limits.memory_gib, limits.disk_floor_gb
    )];
    let grant = &value["screen_grant"];
    if grant["active"].as_bool() == Some(true) {
        let to = grant["grant"]["to"].as_str().unwrap_or("every session");
        let left = grant["grant"]["expires_at_ms"]
            .as_u64()
            .unwrap_or(now)
            .saturating_sub(now);
        lines.push(format!(
            "The screen is granted to {to} for {} more.",
            span(left)
        ));
    }
    if leases.is_empty() {
        lines.push("No leases are held or waiting.".to_owned());
        return lines.join("\n");
    }
    let mut rows = vec![
        [
            "RESOURCE", "STATE", "PRIORITY", "AMOUNT", "SESSION", "AGENT", "PID", "COMMAND",
            "WAITED", "HELD",
        ]
        .map(str::to_owned)
        .to_vec(),
    ];
    for queued in leases {
        let entry = &queued.entry;
        let priority = if queued.effective_priority == entry.priority {
            entry.priority.to_string()
        } else {
            format!("{} (aged to {})", entry.priority, queued.effective_priority)
        };
        rows.push(vec![
            entry.resource.clone(),
            match (entry.state, queued.position) {
                (State::Held, _) => "held".to_owned(),
                (State::Waiting, Some(position)) => format!("waiting #{position}"),
                (State::Waiting, None) => "waiting".to_owned(),
            },
            priority,
            entry.amount.to_string(),
            entry.holder.session.clone(),
            entry.holder.agent.clone(),
            entry.holder.pid.to_string(),
            entry.holder.command.clone(),
            span(queued.wait_ms),
            entry
                .acquired_at_ms
                .map_or_else(|| "-".to_owned(), |at| span(now.saturating_sub(at))),
        ]);
    }
    lines.push(crate::out::table(&rows));
    lines.join("\n")
}

/// `1h05m`, `3m12s`, or `40s`.
fn span(ms: u64) -> String {
    let seconds = ms / 1000;
    match seconds {
        0..60 => format!("{seconds}s"),
        60..3600 => format!("{}m{:02}s", seconds / 60, seconds % 60),
        _ => format!("{}h{:02}m", seconds / 3600, seconds / 60 % 60),
    }
}

fn screen_word(output: &Output, command: &str, words: &[String]) -> Result<(), u8> {
    match words.first().map(String::as_str) {
        Some("screen") => Ok(()),
        Some(other) => Err(output.usage(
            "lease",
            &format!("only the screen takes a grant, not `{other}`; use `openagents lease {command} screen`"),
            USAGE,
        )),
        None => Err(output.usage(
            "lease",
            &format!("`openagents lease {command}` needs `screen`"),
            USAGE,
        )),
    }
}

fn grant(output: &Output, words: &[String]) -> u8 {
    if let Err(code) = screen_word(output, "grant", words) {
        return code;
    }
    let args = match crate::argv::parse_command(
        &words[1..],
        "lease grant screen",
        &["for", "to"],
        &[],
        0,
        0,
    ) {
        Ok(args) => args,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let duration = match args.option("for").map(coder_lease::parse_duration) {
        None => coder_lease::DEFAULT_GRANT,
        Some(Ok(duration)) => duration,
        Some(Err(message)) => return output.usage("lease", &message, USAGE),
    };
    let to = args.option("to").map(str::to_owned);
    if let Some(reason) = grant_refusal(
        std::io::stdin().is_terminal(),
        &|name| std::env::var(name).ok(),
        &coder_lease::ancestors,
    ) {
        return output.fail("lease", &reason);
    }
    let broker = match broker(output) {
        Ok(broker) => broker,
        Err(code) => return code,
    };
    let whom = to.as_deref().map_or_else(
        || "any agent session".to_owned(),
        |to| format!("session {to}"),
    );
    eprint!(
        "Let {whom} take your real screen for {}? Agents can then open windows and capture it. Type yes to confirm: ",
        span(u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
    );
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err()
        || !matches!(answer.trim().to_ascii_lowercase().as_str(), "yes" | "y")
    {
        return output.fail(
            "lease",
            "not granted; the screen stays off limits to agents",
        );
    }
    let grant = Grant::new(&Resource::Screen, duration, to);
    if let Err(error) = broker.grant(&grant) {
        return output.fail("lease", &error.to_string());
    }
    output.emit(&json!({ "granted": grant }), |_| {
        format!(
            "Granted the screen to {whom} for {}.",
            span(grant.expires_at_ms - grant.granted_at_ms)
        )
    });
    0
}

fn revoke(output: &Output, words: &[String]) -> u8 {
    if let Err(code) = screen_word(output, "revoke", words) {
        return code;
    }
    if let Some(word) = words.get(1) {
        return output.usage("lease", &format!("unexpected argument `{word}`"), USAGE);
    }
    let broker = match broker(output) {
        Ok(broker) => broker,
        Err(code) => return code,
    };
    match broker.revoke(&Resource::Screen) {
        Ok(revoked) => {
            output.emit(&json!({ "revoked": revoked }), |_| {
                if revoked {
                    "Revoked the screen grant; new screen leases are refused.".to_owned()
                } else {
                    "The screen was not granted.".to_owned()
                }
            });
            0
        }
        Err(error) => output.fail("lease", &error.to_string()),
    }
}

/// `openagents lease RESOURCE [OPTIONS] -- CMD [ARGS...]`, parsed.
#[derive(Debug, PartialEq)]
struct Hold {
    resource: Resource,
    amount: Option<u64>,
    priority: Option<Priority>,
    no_wait: bool,
    receipt: Option<PathBuf>,
    keep_target_dir: bool,
    command: Vec<String>,
}

fn parse_hold(words: &[String]) -> Result<Hold, String> {
    let split = words
        .iter()
        .position(|word| word == "--")
        .ok_or("put the command after `--`: openagents lease RESOURCE -- CMD [ARGS...]")?;
    let (options, command) = (&words[..split], &words[split + 1..]);
    if command.is_empty() {
        return Err("a command is required after `--`".to_owned());
    }
    let args = crate::argv::parse_command(
        options,
        "lease",
        &["amount", "receipt", "priority"],
        &["no-wait", "keep-target-dir"],
        1,
        1,
    )?;
    let resource = Resource::parse(&args.positional()[0])?;
    let keep_target_dir = args.switch("keep-target-dir");
    if keep_target_dir && resource != Resource::Build {
        return Err("--keep-target-dir goes with build only".to_owned());
    }
    let amount = match args.option("amount") {
        None => None,
        Some(text) => Some(
            text.parse::<u64>()
                .ok()
                .filter(|amount| *amount > 0)
                .ok_or_else(|| format!("--amount is `{text}`, not a whole number above 0"))?,
        ),
    };
    let priority = args.option("priority").map(Priority::parse).transpose()?;
    Ok(Hold {
        resource,
        amount,
        priority,
        no_wait: args.switch("no-wait"),
        receipt: args.option("receipt").map(PathBuf::from),
        keep_target_dir,
        command: command.to_vec(),
    })
}

fn describe(blocked: &Blocked) -> String {
    let holders: Vec<String> = blocked
        .by
        .iter()
        .take(4)
        .map(|entry| {
            format!(
                "{} {} by {} ({} pid {})",
                entry.resource,
                match entry.state {
                    State::Held => "held",
                    State::Waiting => "queued",
                },
                entry.holder.session,
                entry.holder.command,
                entry.holder.pid
            )
        })
        .collect();
    if holders.is_empty() {
        blocked.reason.clone()
    } else {
        format!("{}: {}", blocked.reason, holders.join("; "))
    }
}

fn hold(output: &Output, words: &[String]) -> u8 {
    let hold = match parse_hold(words) {
        Ok(hold) => hold,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let broker = match broker(output) {
        Ok(broker) => broker,
        Err(code) => return code,
    };
    // --priority, else the session's OPENAGENTS_LEASE_PRIORITY, else normal.
    let priority = match hold.priority {
        Some(priority) => priority,
        None => match Priority::from_env() {
            Ok(priority) => priority.unwrap_or_default(),
            Err(message) => return output.usage("lease", &message, USAGE),
        },
    };
    let holder = Holder::detect(&hold.command[0]);
    let mut request = Request::new(hold.resource.clone(), holder)
        .priority(priority)
        .wait(if hold.no_wait {
            Wait::No
        } else {
            Wait::Forever
        })
        .inherit_env();
    if let Some(amount) = hold.amount {
        request = request.amount(amount);
    }
    let shim = std::env::var_os(SHIM_VAR).is_some_and(|value| value == "1");
    let lease = match broker.acquire_notify(request, &mut |blocked| {
        eprintln!(
            "openagents lease: waiting for {}: {}",
            hold.resource,
            describe(blocked)
        );
    }) {
        Ok(lease) => lease,
        Err(Error::Busy(blocked)) => {
            return output.fail(
                "lease",
                &format!(
                    "{} is not free (--no-wait): {}",
                    hold.resource,
                    describe(&blocked)
                ),
            );
        }
        // The cargo shim runs cargo anyway when the lease table can't be
        // written, such as inside a sandbox that keeps the home unwritten.
        Err(Error::Io(error)) if shim => {
            eprintln!(
                "openagents lease: running {} without a build lease: {error}",
                hold.command[0]
            );
            let (exit, failure) = run_command(&hold.command, &[]);
            return finish_unleased(exit, failure);
        }
        Err(error) => return output.fail("lease", &error.to_string()),
    };
    let mut env = lease.env();
    // A build takes a target slot, unless it runs inside another build
    // lease or keeps a target directory already set.
    let slot = if hold.resource == Resource::Build && !lease.nested() {
        let set = std::env::var_os("CARGO_TARGET_DIR").filter(|dir| !dir.is_empty());
        if hold.keep_target_dir && set.is_some() {
            None
        } else {
            match target_slot(shim) {
                Ok(slot) => slot,
                Err(message) => {
                    drop(lease);
                    return output.fail("lease", &message);
                }
            }
        }
    } else {
        None
    };
    if let Some(slot) = &slot {
        env.push((
            "CARGO_TARGET_DIR".to_owned(),
            slot.path.display().to_string(),
        ));
    }
    let (exit, failure) = run_command(&hold.command, &env);
    drop(slot);
    let receipt = match lease.release(exit) {
        Ok(receipt) => receipt,
        Err(error) => return output.fail("lease", &error.to_string()),
    };
    if let Some(path) = &hold.receipt
        && let Err(error) = receipt.write(path)
    {
        return output.fail("lease", &error.to_string());
    }
    if output.json() {
        println!("{}", serde_json::to_value(&receipt).unwrap_or(Value::Null));
    }
    if let Some(message) = failure {
        eprintln!("openagents lease: {message}");
    }
    match exit {
        Some(code) => u8::try_from(code & 0xff).unwrap_or(crate::EXIT_FAILURE),
        None => crate::EXIT_FAILURE,
    }
}

/// Set by the `cargo` lease shim (`coder_lease::shim`) when it runs
/// `openagents lease build`.
const SHIM_VAR: &str = coder_lease::shim::SHIM_VAR;

/// The exit code of a command run without a lease.
fn finish_unleased(exit: Option<i32>, failure: Option<String>) -> u8 {
    if let Some(message) = failure {
        eprintln!("openagents lease: {message}");
    }
    match exit {
        Some(code) => u8::try_from(code & 0xff).unwrap_or(crate::EXIT_FAILURE),
        None => crate::EXIT_FAILURE,
    }
}

/// One of Coder's target slots for the repository the working directory
/// is in, from the task store's pool (`coder::task::targets`), so Coder
/// tasks and conversation agents share one pool and one cap. `None`
/// outside a Git repository, or, under the shim, when the pool can't be
/// written. Waits while every slot of the repository is taken.
fn target_slot(shim: bool) -> Result<Option<coder::task::targets::Lease>, String> {
    let common = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output();
    let common = match common {
        Ok(found) if found.status.success() => {
            PathBuf::from(String::from_utf8_lossy(&found.stdout).trim())
        }
        _ => {
            eprintln!(
                "openagents lease: not in a Git repository, so the build keeps its own target directory"
            );
            return Ok(None);
        }
    };
    let store = coder::task::local::default_store();
    match coder::task::targets::Lease::acquire_waiting(&store, &common, &mut || {
        eprintln!(
            "openagents lease: waiting for a target slot; every slot of this repository is in use"
        );
    }) {
        Ok(slot) => Ok(Some(slot)),
        Err(coder::task::Error::BuildDiskLow { free, floor }) => Err(disk_low(free, floor)),
        Err(coder::task::Error::Io(error)) if shim => {
            eprintln!("openagents lease: building without a target slot: {error}");
            Ok(None)
        }
        Err(error) => Err(format!("no target slot: {error}")),
    }
}

/// The refusal below the free-space floor, in decimal gigabytes.
fn disk_low(free: u64, floor: u64) -> String {
    #[allow(clippy::cast_precision_loss)]
    let gb = |bytes: u64| bytes as f64 / 1e9;
    format!(
        "builds are refused below the free-space floor: {:.1} GB free, and the floor is {:.0} GB. \
         Reclaim space with `openagents background run disk`, or lower the floor with \
         OPENAGENTS_SLOT_FREE_GB.",
        gb(free),
        gb(floor)
    )
}

/// Runs the command under `supervise`, in a process group of its own that
/// holds the terminal while it runs, and returns its exit code and, when
/// it had none, why.
fn run_command(command: &[String], env: &[(String, String)]) -> (Option<i32>, Option<String>) {
    let mut child = Command::new(&command[0]);
    child.args(&command[1..]).env_remove(SHIM_VAR);
    for (name, value) in env {
        child.env(name, value);
    }
    supervise::blocking::own_group(&mut child);
    let mut child = match child.spawn() {
        Ok(child) => child,
        Err(error) => {
            return (
                None,
                Some(format!("`{}` did not start: {error}", command[0])),
            );
        }
    };
    #[cfg(unix)]
    let terminal = signals::hand_over(child.id());
    // A lease has no deadline: it ends when the command does.
    let ending = supervise::blocking::wait(&mut child, Duration::from_secs(10 * 365 * 86_400));
    #[cfg(unix)]
    signals::take_back(terminal);
    match ending {
        supervise::Ending::Exited(Some(code)) => (Some(code), None),
        supervise::Ending::Exited(None) => {
            (None, Some(format!("`{}` ended on a signal", command[0])))
        }
        supervise::Ending::TimedOut => (None, Some(format!("`{}` was stopped", command[0]))),
        supervise::Ending::Failed(message) => (None, Some(message)),
    }
}

/// The command runs in a process group of its own, so the terminal's
/// signals reach it as the foreground group, and termination signals sent
/// to this process are passed on to it.
#[cfg(unix)]
mod signals {
    use std::sync::atomic::{AtomicI32, Ordering};

    static GROUP: AtomicI32 = AtomicI32::new(0);

    extern "C" fn pass_on(signal: libc::c_int) {
        let group = GROUP.load(Ordering::Relaxed);
        if group > 0 {
            // SAFETY: `kill` is async-signal-safe.
            unsafe { libc::kill(-group, signal) };
        }
    }

    /// Passes termination signals to the group and, when this process is
    /// the terminal's foreground, makes the group the foreground. Returns
    /// whether it took the terminal.
    pub(super) fn hand_over(child: u32) -> bool {
        let Ok(group) = i32::try_from(child) else {
            return false;
        };
        GROUP.store(group, Ordering::Relaxed);
        // SAFETY: installing a handler that only calls `kill`, and reading
        // and setting the terminal's foreground group on standard input.
        unsafe {
            for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
                libc::signal(signal, pass_on as *const () as libc::sighandler_t);
            }
            if libc::isatty(0) == 1 && libc::tcgetpgrp(0) == libc::getpgrp() {
                libc::signal(libc::SIGTTOU, libc::SIG_IGN);
                if libc::tcsetpgrp(0, group) == 0 {
                    // It may have stopped reading the terminal before it
                    // owned it.
                    libc::kill(-group, libc::SIGCONT);
                    return true;
                }
            }
        }
        false
    }

    /// Takes the terminal back after the command ends.
    pub(super) fn take_back(terminal: bool) {
        GROUP.store(0, Ordering::Relaxed);
        if terminal {
            // SAFETY: giving the terminal back to this process's own group,
            // with SIGTTOU still ignored so the call can't stop it.
            unsafe {
                libc::tcsetpgrp(0, libc::getpgrp());
                libc::signal(libc::SIGTTOU, libc::SIG_DFL);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn lease_parses_the_resource_options_and_command() {
        let hold = parse_hold(&words(&[
            "disk",
            "--amount",
            "40",
            "--no-wait",
            "--receipt",
            "out/r.json",
            "--",
            "cargo",
            "test",
            "--json",
        ]))
        .unwrap();
        assert_eq!(
            hold,
            Hold {
                resource: Resource::Disk,
                amount: Some(40),
                priority: None,
                no_wait: true,
                receipt: Some(PathBuf::from("out/r.json")),
                keep_target_dir: false,
                command: words(&["cargo", "test", "--json"]),
            }
        );
        assert!(parse_hold(&words(&["build", "cargo", "test"])).is_err());
        assert!(parse_hold(&words(&["build", "--"])).is_err());
        assert!(parse_hold(&words(&["cpu", "--", "true"])).is_err());
        assert!(parse_hold(&words(&["build", "--amount", "0", "--", "true"])).is_err());
        assert!(parse_hold(&words(&["--", "true"])).is_err());
        assert_eq!(
            parse_hold(&words(&["issue/10755", "--", "true"]))
                .unwrap()
                .resource,
            Resource::Issue(10755)
        );
    }

    #[test]
    fn lease_takes_a_priority_by_name() {
        let hold = parse_hold(&words(&["build", "--priority", "push", "--", "cargo"])).unwrap();
        assert_eq!(hold.priority, Some(Priority::Push));
        let hold = parse_hold(&words(&["gpu", "--", "true"])).unwrap();
        assert_eq!(hold.priority, None);
        assert!(parse_hold(&words(&["build", "--priority", "urgent", "--", "cargo"])).is_err());
    }

    #[test]
    fn keep_target_dir_goes_with_build_only() {
        let hold = parse_hold(&words(&["build", "--keep-target-dir", "--", "cargo"])).unwrap();
        assert!(hold.keep_target_dir);
        assert!(parse_hold(&words(&["gpu", "--keep-target-dir", "--", "true"])).is_err());
    }

    #[test]
    fn the_disk_refusal_names_free_space_the_floor_and_the_reclaim_command() {
        let message = disk_low(6_200_000_000, 10_000_000_000);
        assert!(message.contains("6.2 GB free"), "{message}");
        assert!(message.contains("the floor is 10 GB"), "{message}");
        assert!(
            message.contains("openagents background run disk"),
            "{message}"
        );
    }

    #[test]
    fn spans_read_as_hours_minutes_and_seconds() {
        assert_eq!(span(40_000), "40s");
        assert_eq!(span(192_000), "3m12s");
        assert_eq!(span(3_900_000), "1h05m");
    }
}
