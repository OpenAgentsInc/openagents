//! `openagents agent`: the workshop agent (`docs/verse/workshop-agent.md`).
//!
//! Requests, answers, stops, pauses, memory, and the journal go to the
//! running host as NIP-HOST `studio.agent.*` operations over its same-user
//! control socket, so the host plans, checks, and journals everything; a
//! read falls back to the agent's files when no host answers. Making an
//! agent, attesting its key, retiring it, and adding or turning on a
//! standing job are the owner's own actions at the host, so they write the
//! host root's agent records directly.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use coder::argv::Args;
use coder::task::agent::{self, Record, State, Store};
use coder::task::agent_jobs::{self, Edit, Jobs};
use coder_access::Operation;
use coder_access::agent::{self as wire, Mode};
use coder_access::protocol::Outcome;
use serde_json::{Value, json};

use crate::Output;
use crate::studio_host::{Host, Refusal, socket};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub const USAGE: &str = "usage: openagents agent COMMAND [--root DIR] [--control-socket PATH]
  new NAME [--workspace DIR] [--owner-key FILE] [--days N] [--route ROUTE]
               Make an agent with her own key, attested by the owner key in
               FILE (64 hex or nsec1) for N days, at most 365 (default 365).
  attest NAME --owner-key FILE [--days N]
               Attest her key again.
  list         Every agent: state, activity, last report, service record.
  show NAME    One agent in full: key, attestation, transcript, jobs.
  ask NAME TEXT... [--mode MODE] [--workspace LABEL] [--from DIR] [--wait]
               Hand her a request; MODE is auto, task, or terminal, DIR is
               where you asked from, and --wait follows it to her report.
  answer NAME confirm
               CONFIRM the command she proposed.
  answer NAME reject
               REJECT the command she proposed.
  stop NAME [--reason TEXT]
               The kill switch: her standing jobs go off, her panes are
               released with Ctrl+C, her work is cancelled, and she starts
               nothing until resumed. Each step is journaled.
  pause NAME   She keeps everything and starts nothing new.
  resume NAME  She takes work again.
  retire NAME  Stop her, delete her key, and keep her journal.
  log NAME [--after N]
               Her journal, newest last.
  memory NAME list
               Her memory: projects, preferences, outcomes, notes, and
               insights.
  memory NAME note TEXT...
               Tell her something to remember.
  memory NAME forget ID
               Forget an entry; the journal keeps only that it was forgotten.
  memory NAME accept ID
               Accept a preference she proposed, so her briefings carry it.
  memory NAME reject ID
               Reject a preference she proposed.
  memory NAME engrams [--owner-key FILE]
               Her engram heads: slug, time, and event ID. With the owner
               key in FILE, decrypt each one and print it too.
  jobs NAME list
               Her standing jobs, all off until you turn one on.
  jobs NAME add TEMPLATE [--repository OWNER/REPO] [--label L]
               Add nightly-check, watch-issues, keep-green, or reflect,
               off.
  jobs NAME on JOB
               Turn a job on.
  jobs NAME off JOB
               Turn a job off.
  jobs NAME delete JOB
               Delete a job.
  jobs NAME renew JOB [--days N]
               Renew a job for N days, at most 90 (default 30).

The workshop agent, Alice by default, has her own key, a desk in the
Everglade workshop, a journal, memory, and standing jobs. The running host
plans and checks every request; this command is one of its clients. The
smart terminal's `@alice TEXT` sends the same request as `ask`. Defaults:
--root ~/.openagents/host, and the control socket the OpenAgents app and
`openagents host serve --control` use.";

/// What each command does and where the phone runs it.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("new", Effect::LocalWrite),
    Declared::computer("attest", Effect::LocalWrite),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("ask", Effect::Publishes),
    Declared::computer("answer confirm", Effect::Publishes),
    Declared::computer("answer reject", Effect::Publishes),
    Declared::computer("stop", Effect::Publishes),
    Declared::computer("pause", Effect::Publishes),
    Declared::computer("resume", Effect::Publishes),
    Declared::computer("retire", Effect::LocalWrite),
    Declared::computer("log", Effect::ReadOnly),
    Declared::computer("memory list", Effect::ReadOnly),
    Declared::computer("memory note", Effect::Publishes),
    Declared::computer("memory forget", Effect::Publishes),
    Declared::computer("memory accept", Effect::Publishes),
    Declared::computer("memory reject", Effect::Publishes),
    Declared::computer("memory engrams", Effect::ReadOnly),
    Declared::computer("jobs list", Effect::ReadOnly),
    Declared::computer("jobs add", Effect::LocalWrite),
    Declared::computer("jobs on", Effect::LocalWrite),
    Declared::computer("jobs off", Effect::LocalWrite),
    Declared::computer("jobs delete", Effect::LocalWrite),
    Declared::computer("jobs renew", Effect::LocalWrite),
];

const SWITCHES: &[&str] = &["wait"];

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_none_or(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("agent", &message, USAGE),
    };
    let root = args.option("root").map_or_else(default_root, PathBuf::from);
    let words: Vec<&str> = args.positional().iter().map(String::as_str).collect();
    let now = coder::task::autostart::unix_now();
    let result = match words.as_slice() {
        ["new", name] => new(output, &root, name, &args, now),
        ["attest", name] => attest(output, &root, name, &args, now),
        ["list"] => list(output, &root, &args),
        ["show", name] => show(output, &root, name, &args, now),
        ["ask", name, text @ ..] if !text.is_empty() => ask(output, name, &text.join(" "), &args),
        ["answer", name, word @ ("confirm" | "reject")] => {
            answer(output, name, *word == "confirm", &args)
        }
        ["stop", name] => send(
            output,
            &args,
            &Operation::StopAgent {
                agent: (*name).into(),
                reason: args
                    .option("reason")
                    .unwrap_or("stopped from the command line")
                    .into(),
            },
            &format!("Stopped {name}. Her journal records each step: openagents agent log {name}"),
        ),
        ["pause", name] => send(
            output,
            &args,
            &Operation::PauseSeat {
                seat: (*name).into(),
            },
            &format!("Paused {name}: she keeps everything and starts nothing new."),
        ),
        ["resume", name] => send(
            output,
            &args,
            &Operation::ResumeSeat {
                seat: (*name).into(),
            },
            &format!("Resumed {name}."),
        ),
        ["retire", name] => retire(output, &root, name, &args, now),
        ["log", name] => log(output, &root, name, &args),
        ["memory", name, rest @ ..] => memory(output, &root, name, rest, &args),
        ["jobs", name, rest @ ..] => jobs(output, &root, name, rest, &args, now),
        _ => return output.usage("agent", "unknown or incomplete command", USAGE),
    };
    match result {
        Ok(()) => 0,
        Err(Fail::Refused(refusal)) => refusal.report(output),
        Err(Fail::Failed(message)) => {
            eprintln!("openagents agent: {message}");
            if output.json() {
                println!("{}", json!({"error": message}));
            }
            crate::EXIT_FAILURE
        }
    }
}

enum Fail {
    Refused(Refusal),
    Failed(String),
}

impl From<Refusal> for Fail {
    fn from(refusal: Refusal) -> Self {
        Self::Refused(refusal)
    }
}

impl From<String> for Fail {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}

fn default_root() -> PathBuf {
    agent::host_root().unwrap_or_else(|| PathBuf::from(".openagents/host"))
}

fn host(args: &Args) -> Result<Host, Fail> {
    let (path, _) = socket(args);
    let path = path.ok_or_else(|| Fail::Failed("no control socket path on this system".into()))?;
    Host::new(path).map_err(Fail::Failed)
}

/// Sends `operation` and returns its answer's JSON.
fn call(args: &Args, operation: &Operation) -> Result<Value, Fail> {
    let mut host = host(args)?;
    match host.call(operation)? {
        Outcome::Agent { agent } => Ok(*agent),
        Outcome::Dispatched { receipt } => Ok(json!({"dispatched": receipt.reference})),
        _ => Err(Fail::Failed("the host answered another operation".into())),
    }
}

fn send(output: &Output, args: &Args, operation: &Operation, said: &str) -> Result<(), Fail> {
    let value = call(args, operation)?;
    output.emit(&value, |_| said.to_string());
    Ok(())
}

fn store(root: &Path, name: &str) -> Result<(Store, Record), Fail> {
    let store = Store::new(root, name).map_err(Fail::Failed)?;
    let _ = store.migrate(coder::task::autostart::unix_now());
    let record = store.load().map_err(Fail::Failed)?.ok_or_else(|| {
        Fail::Failed(format!(
            "there is no agent named {name} in {}",
            root.display()
        ))
    })?;
    Ok((store, record))
}

fn owner_key(args: &Args) -> Result<Option<secp256k1::SecretKey>, Fail> {
    let Some(path) = args.option("owner-key") else {
        return Ok(None);
    };
    let text = std::fs::read_to_string(path)
        .map_err(|e| Fail::Failed(format!("cannot read {path}: {e}")))?;
    agent::parse_secret(&text)
        .map(Some)
        .map_err(|e| Fail::Failed(format!("{path}: {e}")))
}

fn expiry(args: &Args, now: u64) -> Result<u64, Fail> {
    let days: u64 = args.number("days", 365).map_err(Fail::Failed)?;
    if days == 0 || days > 365 {
        return Err(Fail::Failed("an attestation lasts 1 to 365 days".into()));
    }
    Ok(now + days * 86_400)
}

fn new(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let store = Store::new(root, name).map_err(Fail::Failed)?;
    let workspace = args
        .option("workspace")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"));
    let existed = store.load().map_err(Fail::Failed)?.is_some()
        || store.migrate(now).map_err(Fail::Failed)?;
    let mut record = store.open(&workspace, now).map_err(Fail::Failed)?;
    if let Some(route) = args.option("route") {
        coder::task::studio::parse_route(route).map_err(|e| Fail::Failed(e.to_string()))?;
        record.route = route.into();
        store.save(&record).map_err(Fail::Failed)?;
    }
    let mut record = store.ensure_key(record, now).map_err(Fail::Failed)?;
    if let Some(owner) = owner_key(args)? {
        record = store
            .attest(record, &owner, expiry(args, now)?, now)
            .map_err(Fail::Failed)?;
    }
    let value = record_json(&record, now);
    output.emit(&value, |_| {
        let mut text = format!(
            "{} {name}. Her key is {}.",
            if existed { "Opened" } else { "Made" },
            record.pubkey.as_deref().unwrap_or("missing")
        );
        match &record.attestation {
            Some(_) => text.push_str(&format!(
                " The owner attested it until {}.",
                value["attested_until"]
            )),
            None => text.push_str(&format!(
                " Attest it with `openagents agent attest {name} --owner-key FILE`."
            )),
        }
        text.push_str(&format!(
            "\nShe works in {}. Her record and journal: {}",
            record.workspace,
            store.dir().display()
        ));
        text
    });
    Ok(())
}

fn attest(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let (store, record) = store(root, name)?;
    let owner =
        owner_key(args)?.ok_or_else(|| Fail::Failed("attest needs --owner-key FILE".into()))?;
    let record = store.ensure_key(record, now).map_err(Fail::Failed)?;
    let record = store
        .attest(record, &owner, expiry(args, now)?, now)
        .map_err(Fail::Failed)?;
    let value = record_json(&record, now);
    output.emit(&value, |v| {
        format!("Attested {name}'s key until {}.", v["attested_until"])
    });
    Ok(())
}

fn record_json(record: &Record, now: u64) -> Value {
    let attested_until = match (&record.pubkey, &record.attestation) {
        (Some(pubkey), Some(attestation)) => {
            agent::verify_attestation(pubkey, attestation, now).ok()
        }
        _ => None,
    };
    json!({
        "name": record.name,
        "state": record.state.word(),
        "workspace": record.workspace,
        "look": record.look,
        "route": record.route,
        "desk": record.desk,
        "pubkey": record.pubkey,
        "attestation": record.attestation,
        "attested_until": attested_until,
        "charter": record.charter,
    })
}

/// The agents as the host sees them, or from disk when no host answers.
fn agents(root: &Path, args: &Args) -> Result<(wire::Agents, bool), Fail> {
    match call(args, &Operation::ListAgents {}) {
        Ok(value) => serde_json::from_value(value)
            .map(|agents| (agents, true))
            .map_err(|e| Fail::Failed(format!("the host's answer does not read: {e}"))),
        Err(Fail::Refused(refusal)) if refusal.code == "unavailable" => {
            let agents = coder::task::agent_host::Agents::new(
                root,
                root.join("no-tasks"),
                Default::default(),
            );
            Ok((agents.list(), false))
        }
        Err(other) => Err(other),
    }
}

fn line(view: &wire::AgentView) -> String {
    let activity = serde_json::to_value(view.activity)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default();
    format!(
        "{:<12} {:<8} {:<9} {:<28} requests {} finished {} merged {}",
        view.name,
        view.state,
        activity,
        if view.headline.is_empty() {
            "no report yet"
        } else {
            &view.headline
        },
        view.service.requests,
        view.service.finished,
        view.service.merged
    )
}

fn list(output: &Output, root: &Path, args: &Args) -> Result<(), Fail> {
    let (agents, live) = agents(root, args)?;
    let value = json!({"agents": agents.agents, "host": live});
    output.emit(&value, |_| {
        if agents.agents.is_empty() {
            return "No agents. Make one with `openagents agent new alice`.".into();
        }
        let mut text: Vec<String> = agents.agents.iter().map(line).collect();
        if !live {
            text.push("(no host answered; read from her records)".into());
        }
        text.join("\n")
    });
    Ok(())
}

fn show(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let (store, record) = store(root, name)?;
    let (agents, live) = agents(root, args)?;
    let view = agents.agents.into_iter().find(|a| a.name == name);
    let jobs = Jobs::new(store.clone()).rows().unwrap_or_default();
    let value = json!({
        "record": record_json(&record, now),
        "view": view,
        "jobs": jobs,
        "host": live,
    });
    output.emit(&value, |v| {
        let mut text = vec![
            format!("{name} ({}) at desk {}", record.state.word(), record.desk),
            format!("key: {}", record.pubkey.as_deref().unwrap_or("none")),
            format!(
                "attested until: {}",
                v["record"]["attested_until"]
                    .as_u64()
                    .map_or("not attested".to_string(), |at| at.to_string())
            ),
            format!("works in: {}", record.workspace),
            format!("charter: {}", record.charter),
        ];
        if let Some(view) = &view {
            text.push(line(view));
            if let Some(pending) = &view.pending {
                text.push(format!(
                    "PROPOSED: {} ({}) -- openagents agent answer {name} confirm|reject",
                    pending.command, pending.why
                ));
            }
            if let Some(change) = &view.change {
                text.push(format!("change: task {} ({})", change.task, change.stage));
            }
            text.push(String::new());
            text.extend(view.lines.iter().cloned());
        }
        for job in &jobs {
            text.push(format!(
                "job {} [{}] {} {}/{} {}",
                job.job,
                if job.enabled { "on" } else { "off" },
                job.trigger,
                job.occurrences,
                job.max_occurrences,
                job.last.as_deref().unwrap_or("")
            ));
        }
        text.join("\n")
    });
    Ok(())
}

fn ask(output: &Output, name: &str, text: &str, args: &Args) -> Result<(), Fail> {
    let mode = match args.option("mode").unwrap_or("auto") {
        "auto" => Mode::Auto,
        "task" => Mode::Task,
        "terminal" => Mode::Terminal,
        other => {
            return Err(Fail::Failed(format!(
                "--mode is auto, task, or terminal, not {other}"
            )));
        }
    };
    let context = args
        .option("from")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .map(|dir| format!("The owner asked from {}.", dir.display()))
        .unwrap_or_default();
    let before = view_of(args, name).ok().map_or(0, |v| v.service.requests);
    call(
        args,
        &Operation::AskAgent {
            agent: name.into(),
            text: text.into(),
            workspace: args.option("workspace").map(str::to_owned),
            context,
            mode,
            typist: false,
        },
    )?;
    if !args.switch("wait") {
        output.emit(&json!({"asked": name}), |_| {
            format!("Asked {name}. Follow her with `openagents agent show {name}`.")
        });
        return Ok(());
    }
    let start = Instant::now();
    let mut shown = 0;
    let mut proposed = None;
    loop {
        let view = view_of(args, name)?;
        if !output.json() {
            let total = view.lines.len();
            for line in view.lines.iter().skip(shown.min(total)) {
                println!("{line}");
            }
            shown = total;
            if let Some(pending) = &view.pending
                && proposed != Some(pending.step)
            {
                proposed = Some(pending.step);
                println!(
                    "PROPOSED: {} ({}) -- openagents agent answer {name} confirm|reject",
                    pending.command, pending.why
                );
            }
        }
        if !view.busy && view.service.requests > before {
            output.emit(&json!({"agent": view}), |_| String::new());
            return Ok(());
        }
        if start.elapsed() > Duration::from_secs(60 * 60) {
            return Err(Fail::Failed("she has not reported in an hour".into()));
        }
        std::thread::sleep(Duration::from_millis(500));
    }
}

fn view_of(args: &Args, name: &str) -> Result<wire::AgentView, Fail> {
    let value = call(args, &Operation::ListAgents {})?;
    let agents: wire::Agents = serde_json::from_value(value)
        .map_err(|e| Fail::Failed(format!("the host's answer does not read: {e}")))?;
    agents
        .agents
        .into_iter()
        .find(|a| a.name == name)
        .ok_or_else(|| Fail::Failed(format!("the host has no agent named {name}")))
}

fn answer(output: &Output, name: &str, confirm: bool, args: &Args) -> Result<(), Fail> {
    let view = view_of(args, name)?;
    let pending = view
        .pending
        .ok_or_else(|| Fail::Failed(format!("{name} is not waiting on you")))?;
    call(
        args,
        &Operation::AnswerAgent {
            agent: name.into(),
            step: pending.step,
            confirm,
        },
    )?;
    let word = if confirm { "Confirmed" } else { "Rejected" };
    output.emit(&json!({"step": pending.step, "confirm": confirm}), |_| {
        format!("{word}: {}", pending.command)
    });
    Ok(())
}

fn retire(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let (store, mut record) = store(root, name)?;
    // Stop her through the host first, when one answers.
    let stopped = call(
        args,
        &Operation::StopAgent {
            agent: name.into(),
            reason: "retired".into(),
        },
    )
    .is_ok();
    if !stopped {
        let _ = Jobs::new(store.clone()).disable_all();
    }
    let deleted = store.delete_key().map_err(Fail::Failed)?;
    record.state = State::Retired;
    record.attestation = None;
    store.save(&record).map_err(Fail::Failed)?;
    store
        .append(&agent::Entry::new(
            now,
            agent::Kind::Control,
            &format!(
                "retired by the owner; {}; her journal stays",
                if deleted {
                    "her key is deleted"
                } else {
                    "she had no key"
                }
            ),
        ))
        .map_err(Fail::Failed)?;
    output.emit(&json!({"retired": name, "key_deleted": deleted}), |_| {
        format!(
            "Retired {name}: her key is deleted and her journal stays. She never published, so \
             there is no key to archive with NIP-IA."
        )
    });
    Ok(())
}

fn log(output: &Output, root: &Path, name: &str, args: &Args) -> Result<(), Fail> {
    let after = args.option("after").and_then(|a| a.parse().ok());
    let journal: wire::Journal = match call(
        args,
        &Operation::AgentLog {
            agent: name.into(),
            after,
        },
    ) {
        Ok(value) => serde_json::from_value(value)
            .map_err(|e| Fail::Failed(format!("the host's answer does not read: {e}")))?,
        Err(Fail::Refused(refusal)) if refusal.code == "unavailable" => {
            let (store, _) = store(root, name)?;
            let entries = store.journal(usize::MAX).map_err(Fail::Failed)?;
            let rows = entries
                .into_iter()
                .enumerate()
                .map(|(i, e)| wire::JournalRow {
                    seq: i as u64 + 1,
                    at: e.at,
                    kind: serde_json::to_value(e.kind)
                        .ok()
                        .and_then(|v| v.as_str().map(str::to_owned))
                        .unwrap_or_default(),
                    text: e.text,
                    status: e.status,
                })
                .filter(|row| after.is_none_or(|a| row.seq > a))
                .collect();
            wire::Journal { journal: rows }
        }
        Err(other) => return Err(other),
    };
    output.emit(&json!(journal), |_| {
        journal
            .journal
            .iter()
            .map(|row| {
                format!(
                    "{:>4} {} {:<9} {}{}",
                    row.seq,
                    row.at,
                    row.kind,
                    row.text,
                    row.status
                        .map(|s| format!(" (exit {s})"))
                        .unwrap_or_default()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

fn memory(
    output: &Output,
    root: &Path,
    name: &str,
    rest: &[&str],
    args: &Args,
) -> Result<(), Fail> {
    let edit = match rest {
        [] | ["list"] => None,
        ["engrams"] => return engrams(output, root, name, args),
        ["note", text @ ..] if !text.is_empty() => Some(wire::MemoryEdit::Note {
            text: text.join(" "),
        }),
        [verb @ ("forget" | "accept" | "reject"), id] => {
            let id: u64 = id
                .parse()
                .map_err(|_| Fail::Failed(format!("{id} is not a memory entry")))?;
            Some(match *verb {
                "forget" => wire::MemoryEdit::Forget { id },
                "accept" => wire::MemoryEdit::Accept { id },
                _ => wire::MemoryEdit::Reject { id },
            })
        }
        _ => return Err(Fail::Failed(format!("unknown memory command; {USAGE}"))),
    };
    if let Some(edit) = edit {
        let value = call(
            args,
            &Operation::EditAgentMemory {
                agent: name.into(),
                edit,
            },
        )?;
        output.emit(&value, |v| {
            format!("Done: entry {}.", v["dispatched"].as_str().unwrap_or(""))
        });
        return Ok(());
    }
    let memory: wire::Memory = match call(
        args,
        &Operation::ListAgentMemory {
            agent: name.into(),
            after: None,
        },
    ) {
        Ok(value) => serde_json::from_value(value)
            .map_err(|e| Fail::Failed(format!("the host's answer does not read: {e}")))?,
        Err(Fail::Refused(refusal)) if refusal.code == "unavailable" => {
            let (store, _) = store(root, name)?;
            wire::Memory {
                memory: coder::task::agent_memory::Memory::new(store, secret_screen_shapes())
                    .rows(None)
                    .map_err(Fail::Failed)?,
            }
        }
        Err(other) => return Err(other),
    };
    output.emit(&json!(memory), |_| {
        if memory.memory.is_empty() {
            return format!("{name} remembers nothing yet.");
        }
        memory
            .memory
            .iter()
            .map(|m| format!("{:>4} {:<10} {:<9} {}", m.id, m.kind, m.state, m.text))
            .collect::<Vec<_>>()
            .join("\n")
    });
    Ok(())
}

/// One engram head as the command prints it.
fn head_json(engram: &nostr::engram::Engram, with_body: bool) -> Value {
    let mut head = json!({
        "slug": engram.slug().as_str(),
        "created_at": engram.created_at,
        "id": engram.id,
    });
    if with_body {
        head["body"] = serde_json::from_str::<Value>(&engram.body.to_json()).unwrap_or(Value::Null);
    }
    head
}

/// Her engram heads, read with her key on this computer, or decrypted
/// with the owner key when `--owner-key FILE` names it.
fn engrams(output: &Output, root: &Path, name: &str, args: &Args) -> Result<(), Fail> {
    use coder::task::agent_engrams::{self, EngramStore, Opened};
    let (store, _) = store(root, name)?;
    let (heads, problems, decrypted) = if let Some(owner) = owner_key(args)? {
        let view = agent_engrams::owner_read(&store, &owner).map_err(Fail::Failed)?;
        let heads: Vec<Value> = view.heads.iter().map(|h| head_json(h, true)).collect();
        (heads, view.problems, true)
    } else {
        match EngramStore::read(&store, &secret_screen_shapes()) {
            Opened::Ready(engrams) => (
                engrams
                    .heads()
                    .into_iter()
                    .filter(|h| !h.is_tombstone())
                    .map(|h| head_json(h, false))
                    .collect(),
                Vec::new(),
                false,
            ),
            Opened::Skipped(why) => {
                let said = format!("{name} keeps no engrams: {why}.");
                output.emit(&json!({"heads": [], "skipped": why}), |_| said.clone());
                return Ok(());
            }
            Opened::Unreadable(why) => {
                return Err(Fail::Failed(format!(
                    "{name}'s engram store cannot be read, so nothing from it is carried: {why}"
                )));
            }
        }
    };
    let value = json!({"heads": heads, "problems": problems, "decrypted": decrypted});
    output.emit(&value, |_| {
        let mut lines: Vec<String> = heads
            .iter()
            .map(|h| {
                let mut line = format!(
                    "{:<28} {:>10} {}",
                    h["slug"].as_str().unwrap_or(""),
                    h["created_at"],
                    h["id"].as_str().unwrap_or("")
                );
                if let Some(body) = h.get("body") {
                    let text = body
                        .get("profile")
                        .or_else(|| body.get("value"))
                        .and_then(Value::as_str)
                        .unwrap_or("(forgotten)");
                    line.push_str(&format!(
                        "\n    {}",
                        agent::ascii(text).replace('\n', "\n    ")
                    ));
                }
                line
            })
            .collect();
        if lines.is_empty() {
            lines.push(format!("{name} has no engrams yet."));
        }
        for problem in &problems {
            lines.push(format!("not read: {problem}"));
        }
        lines.join("\n")
    });
    Ok(())
}

fn secret_screen_shapes() -> secret_screen::Screen {
    secret_screen::Screen::shapes()
}

fn jobs(
    output: &Output,
    root: &Path,
    name: &str,
    rest: &[&str],
    args: &Args,
    now: u64,
) -> Result<(), Fail> {
    let (store, _) = store(root, name)?;
    let jobs = Jobs::new(store);
    let said = match rest {
        [] | ["list"] => {
            let rows = jobs.rows().map_err(Fail::Failed)?;
            output.emit(&json!({"jobs": rows}), |_| {
                if rows.is_empty() {
                    return format!(
                        "{name} has no standing jobs. Add one: openagents agent jobs {name} add nightly-check"
                    );
                }
                rows.iter()
                    .map(|job| {
                        format!(
                            "{:<14} [{}] {:<8} {}/{} expires {} {}",
                            job.job,
                            if job.enabled { "on" } else { "off" },
                            job.trigger,
                            job.occurrences,
                            job.max_occurrences,
                            job.expires_at,
                            job.last.as_deref().unwrap_or("")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            });
            return Ok(());
        }
        ["add", template] => {
            let job = agent_jobs::template(
                template,
                args.option("workspace").unwrap_or(""),
                args.option("repository"),
                args.option("label"),
                utc_offset(),
                now,
            )
            .map_err(Fail::Failed)?;
            jobs.add(job, now).map_err(Fail::Failed)?;
            format!(
                "Added {template}, off. Turn it on with `openagents agent jobs {name} on {template}`."
            )
        }
        ["on", job] => {
            jobs.edit(job, Edit::On, now).map_err(Fail::Failed)?;
            format!("Turned {job} on.")
        }
        ["off", job] => {
            jobs.edit(job, Edit::Off, now).map_err(Fail::Failed)?;
            format!("Turned {job} off.")
        }
        ["delete", job] => {
            jobs.edit(job, Edit::Delete, now).map_err(Fail::Failed)?;
            format!("Deleted {job}.")
        }
        ["renew", job] => {
            let days: u64 = args.number("days", 30).map_err(Fail::Failed)?;
            jobs.edit(job, Edit::Renew(now + days * 86_400), now)
                .map_err(Fail::Failed)?;
            format!("Renewed {job} for {days} days.")
        }
        _ => {
            return Err(Fail::Failed(
                "unknown jobs command; see `openagents agent --help`".into(),
            ));
        }
    };
    output.emit(&json!({"done": said}), |_| said.clone());
    Ok(())
}

/// This computer's offset from UTC in minutes, from `date +%z`.
fn utc_offset() -> i32 {
    std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .and_then(|o| {
            let text = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let sign = if text.starts_with('-') { -1 } else { 1 };
            let digits = text.trim_start_matches(['+', '-']);
            let hours: i32 = digits.get(..2)?.parse().ok()?;
            let minutes: i32 = digits.get(2..4)?.parse().ok()?;
            Some(sign * (hours * 60 + minutes))
        })
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(words: &[&str]) -> Args {
        let words: Vec<String> = words.iter().map(|w| (*w).to_string()).collect();
        Args::parse(&words, SWITCHES).unwrap()
    }

    #[test]
    fn new_makes_a_keyed_attested_agent_and_retire_deletes_the_key() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let owner = dir.path().join("owner.key");
        std::fs::write(&owner, "07".repeat(32)).unwrap();
        let output = Output::new(true);
        let now = 1_791_158_400;
        let made = args(&[
            "new",
            "alice",
            "--owner-key",
            owner.to_str().unwrap(),
            "--days",
            "30",
            "--workspace",
            dir.path().to_str().unwrap(),
        ]);
        assert!(new(&output, &root, "alice", &made, now).is_ok());
        let (store, record) = store(&root, "alice").ok().unwrap();
        let pubkey = record.pubkey.clone().unwrap();
        let until =
            agent::verify_attestation(&pubkey, record.attestation.as_ref().unwrap(), now).unwrap();
        assert_eq!(until, now + 30 * 86_400);
        assert!(store.key().unwrap().is_some());
        // Too long an attestation is refused.
        let long = args(&[
            "attest",
            "alice",
            "--owner-key",
            owner.to_str().unwrap(),
            "--days",
            "400",
        ]);
        assert!(attest(&output, &root, "alice", &long, now).is_err());
        // A socket nobody answers: retire still deletes the key.
        let retired = args(&[
            "retire",
            "alice",
            "--control-socket",
            dir.path().join("none.sock").to_str().unwrap(),
        ]);
        assert!(retire(&output, &root, "alice", &retired, now + 5).is_ok());
        let (kept, record) = super::store(&root, "alice").ok().unwrap();
        assert_eq!(record.state, State::Retired);
        assert!(kept.key().unwrap().is_none());
        assert!(
            kept.journal(50)
                .unwrap()
                .iter()
                .any(|e| e.text.starts_with("retired"))
        );
    }

    #[test]
    fn engrams_list_with_her_key_and_decrypt_with_the_owner_key() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let owner = dir.path().join("owner.key");
        std::fs::write(&owner, "07".repeat(32)).unwrap();
        let output = Output::new(true);
        let now = 1_791_158_400;
        let made = args(&[
            "new",
            "alice",
            "--owner-key",
            owner.to_str().unwrap(),
            "--workspace",
            dir.path().to_str().unwrap(),
        ]);
        assert!(new(&output, &root, "alice", &made, now).is_ok());
        let (store, _) = store(&root, "alice").ok().unwrap();
        let memory = coder::task::agent_memory::Memory::new(store.clone(), secret_screen_shapes());
        memory
            .add(
                coder::task::agent_memory::MemoryKind::Note,
                coder::task::agent_memory::Author::Owner,
                "the owner reads this",
                vec![],
                now,
            )
            .unwrap();
        assert!(engrams(&output, &root, "alice", &args(&[])).is_ok());
        let view = coder::task::agent_engrams::owner_read(
            &store,
            &agent::parse_secret(&"07".repeat(32)).unwrap(),
        )
        .unwrap();
        assert_eq!(view.heads.len(), 3, "core, persona, and the note");
        let with_owner = args(&["--owner-key", owner.to_str().unwrap()]);
        assert!(engrams(&output, &root, "alice", &with_owner).is_ok());
        let missing = args(&["--owner-key", dir.path().join("none").to_str().unwrap()]);
        assert!(engrams(&output, &root, "alice", &missing).is_err());
    }

    #[test]
    fn jobs_are_added_off_and_turned_on_at_the_host() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let output = Output::new(true);
        let now = 1_791_158_400;
        new(&output, &root, "alice", &args(&["new", "alice"]), now)
            .ok()
            .unwrap();
        jobs(
            &output,
            &root,
            "alice",
            &["add", "nightly-check"],
            &args(&["jobs"]),
            now,
        )
        .ok()
        .unwrap();
        let (store, _) = store(&root, "alice").ok().unwrap();
        assert!(!Jobs::new(store.clone()).load().unwrap()[0].enabled);
        jobs(
            &output,
            &root,
            "alice",
            &["on", "nightly-check"],
            &args(&["jobs"]),
            now,
        )
        .ok()
        .unwrap();
        assert!(Jobs::new(store).load().unwrap()[0].enabled);
        assert!(
            jobs(
                &output,
                &root,
                "alice",
                &["add", "watch-issues"],
                &args(&["jobs"]),
                now
            )
            .is_err(),
            "watch issues names a repository"
        );
    }
}
