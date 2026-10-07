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
use coder::task::agent_lifecycle as lifecycle;
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
      [--preset PRESET] [--role ROLE]
               Make an agent with her own key, attested by the owner key in
               FILE (64 hex or nsec1) for N days, at most 365 (default 365).
               A crew member's name (alice, bob, paul, erin, frank, pat, arthur, vanna) starts from its preset:
               charter, look, and definition. --preset starts another name
               from one.
  charter NAME --role ROLE --expected N --drafting on|off --purpose TEXT
               Owner-only host edit of a sales job charter. All model tools,
               task execution, sending, payments, and publication stay absent.
  verdict NAME record FILE
               Owner-only host recording of a signed private recommendation.
               FILE is a bounded VerdictInput JSON document with exact pins.
               A question-set digest is a reference, not proof of a decision.
  verdict NAME list
               Read signed retained recommendations. They grant no approval.
  crew status  Owner-only current cohort digest and retained cleanup results.
  crew stop|pause --cohort NAME --all [--reason TEXT]
               Stop or pause all native sales members and revoke pending subjects.
               Other agents stay as they are.
  crew stop|pause --cohort NAME --members NAMES [--reason TEXT]
               Stop or pause a comma-separated exact subset of native sales members.
               Other agents stay as they are.
  crew resume --cohort NAME --all --expected DIGEST
               Explicitly resume the same full selection at its current digest.
               Disabled jobs stay off; stale approvals cannot resume.
  crew resume --cohort NAME --members NAMES --expected DIGEST
               Explicitly resume the same exact subset at its current digest.
               Disabled jobs stay off; stale approvals cannot resume.
  attest NAME --owner-key FILE [--days N]
               Attest her key again.
  renew NAME --owner-key FILE [--days N]
               Renew the owner's attestation of her key before it expires,
               and sign her profile again.
  list        Every agent: state, activity, last report, service record.
  show NAME [--owner-key FILE]
               One agent in full: key, attestation, transcript, jobs, and
               spend today and in all against her budget, from her NIP-AM
               records, decrypted with the owner key in FILE when given.
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
  retire NAME [--owner-key FILE]
               Stop her, delete her key from the host's key store, and keep
               her journal and engrams, which the owner key still reads.
               With relay sync on and the owner key, ask her relays to
               archive her key (NIP-IA).
  rotate NAME [--owner-key FILE] [--reason TEXT] [--days N]
               Give her a new key: every engram is encrypted again under
               it, the owner signs a lineage record and a new attestation,
               and with relay sync on the owner asks her relays to archive
               the old key (NIP-IA). The running host rotates her with the
               owner key it holds; else her key must be in a file here.
               Grants to her old key don't carry over.
  move NAME --to HOSTKEY
               Mark her moved to the owner's computer whose host key is
               HOSTKEY, after you copied her directory there. This computer
               runs nothing of hers again.
  export NAME --out FILE [--memory none|core|all] [--owner-key FILE]
               A snapshot without her key: her definition, and with core
               her core profile, or with all every memory as plaintext.
               The owner key reads her core when her key isn't here.
  import FILE --name NAME [--workspace DIR] [--owner-key FILE] [--days N]
               Make a new agent from a snapshot, with a new key of her own;
               with the owner key, her memory is encrypted under it.
  signing NAME on|off
               Sign the tip of her merged worktree changes with her key
               (NIP-GS), with the owner's attestation embedded. Off by
               default.
  xp-link NAME --owner-key FILE
               Sign her side of an XP key link (NIP-XP 13195) to the owner
               key in FILE, and print the key the owner's trainer profile
               (13193) must list. Nothing is published; the key counts
               toward the owner only once both sides are.
  engine NAME coder|codex
               What does her coding: coder, Coder's own model (the
               default), or codex, where Coder delegates the coding to the
               Codex agent on your ChatGPT login and checks it, and works on
               its own model while Codex is out of capacity.
  log NAME [--after N]
               Her journal, newest last.
  memory NAME list
               Her memory: projects, preferences, outcomes, notes, and
               insights, then the knowledge entries she drafted, each with
               the command that publishes it. Only you publish a draft.
  memory NAME note TEXT...
               Tell her something to remember.
  memory NAME forget ID
               Forget an entry; the journal keeps only that it was forgotten.
  memory NAME accept ID
               Accept a preference she proposed, so her briefings carry it.
               Entry 0 is a core profile she proposed: accepting it writes
               her core, unless her core changed since she proposed it.
  memory NAME reject ID
               Reject a preference she proposed, or, as entry 0, her
               proposed core profile.
  memory NAME engrams [--owner-key FILE] [--from-relay] [--relay URL]...
               Her engram heads: slug, time, and event ID. With the owner
               key in FILE, decrypt each one and print it too. With
               --from-relay, read them from relays with the owner key
               alone: her write relays from her relay list on each URL,
               else the URLs, else the relays she syncs with.
  memory NAME engrams --orphans [--owner-key FILE]
               The memories her core does not reach through [[slug]]
               links, and the links that name a missing memory. Nothing
               deletes an orphan.
  memory NAME sync on [--relay URL]...
               Sync her engrams with these relays, by default the owner's
               relay. Sync is off until you turn it on; a relay sees her
               key, her owner's, and the sizes and times of her engrams.
  memory NAME sync off
               Stop syncing her engrams.
  memory NAME sync status
               Her relays and the last pass: heads taken and published,
               conflicts, refusals, and relays that may hold more.
  memory NAME sync now
               Run a pass now when her key is in a file here, else ask the
               host to run one at its next sweep.
  jobs NAME list
               Her standing jobs, all off until you turn one on.
  jobs NAME add TEMPLATE [--repository OWNER/REPO] [--label L]
               Add nightly-check, watch-issues, keep-green, reflect, or
               plan (her morning day plan), off.
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
    Declared::computer("charter", Effect::Publishes),
    Declared::computer("verdict record", Effect::Publishes),
    Declared::computer("verdict list", Effect::ReadOnly),
    Declared::computer("crew status", Effect::ReadOnly),
    Declared::computer("crew stop", Effect::Publishes),
    Declared::computer("crew pause", Effect::Publishes),
    Declared::computer("crew resume", Effect::Publishes),
    Declared::computer("attest", Effect::LocalWrite),
    Declared::computer("renew", Effect::LocalWrite),
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("ask", Effect::Publishes),
    Declared::computer("answer confirm", Effect::Publishes),
    Declared::computer("answer reject", Effect::Publishes),
    Declared::computer("stop", Effect::Publishes),
    Declared::computer("pause", Effect::Publishes),
    Declared::computer("resume", Effect::Publishes),
    Declared::computer("retire", Effect::Publishes),
    Declared::computer("rotate", Effect::Publishes),
    Declared::computer("move", Effect::LocalWrite),
    Declared::computer("export", Effect::LocalWrite),
    Declared::computer("import", Effect::LocalWrite),
    Declared::computer("signing on", Effect::LocalWrite),
    Declared::computer("signing off", Effect::LocalWrite),
    Declared::computer("xp-link", Effect::ReadOnly),
    Declared::computer("engine coder", Effect::LocalWrite),
    Declared::computer("engine codex", Effect::LocalWrite),
    Declared::computer("log", Effect::ReadOnly),
    Declared::computer("memory list", Effect::ReadOnly),
    Declared::computer("memory note", Effect::Publishes),
    Declared::computer("memory forget", Effect::Publishes),
    Declared::computer("memory accept", Effect::Publishes),
    Declared::computer("memory reject", Effect::Publishes),
    Declared::computer("memory engrams", Effect::ReadOnly),
    Declared::computer("memory sync on", Effect::LocalWrite),
    Declared::computer("memory sync off", Effect::LocalWrite),
    Declared::computer("memory sync status", Effect::ReadOnly),
    Declared::computer("memory sync now", Effect::Publishes),
    Declared::computer("jobs list", Effect::ReadOnly),
    Declared::computer("jobs add", Effect::LocalWrite),
    Declared::computer("jobs on", Effect::LocalWrite),
    Declared::computer("jobs off", Effect::LocalWrite),
    Declared::computer("jobs delete", Effect::LocalWrite),
    Declared::computer("jobs renew", Effect::LocalWrite),
];

const SWITCHES: &[&str] = &["wait", "from-relay", "orphans", "all"];

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
        ["crew", "status"] => crew_status(output, &args),
        ["crew", action @ ("stop" | "pause" | "resume")] => crew_control(output, &args, action),
        ["new", name] => new(output, &root, name, &args, now),
        ["charter", name] => charter(output, name, &args),
        ["verdict", name, "record", file] => verdict(output, name, file, &args),
        ["verdict", name, "list"] => verdicts(output, &root, name, &args),
        ["attest", name] => attest(output, &root, name, &args, now, false),
        ["renew", name] => attest(output, &root, name, &args, now, true),
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
            &format!(
                "Stopped {name}. {} journal records each step: openagents agent log {name}",
                refer(&root, name).their_cap()
            ),
        ),
        ["pause", name] => send(
            output,
            &args,
            &Operation::PauseSeat {
                seat: (*name).into(),
            },
            &format!(
                "Paused {name}: {} keeps everything and starts nothing new.",
                refer(&root, name).they()
            ),
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
        ["rotate", name] => rotate(output, &root, name, &args, now),
        ["move", name] => move_to(output, &root, name, &args, now),
        ["export", name] => export(output, &root, name, &args, now),
        ["import", file] => import(output, &root, file, &args, now),
        ["signing", name, word @ ("on" | "off")] => {
            signing(output, &root, name, *word == "on", now)
        }
        ["xp-link", name] => xp_link(output, &root, name, &args, now),
        ["engine", name, word @ ("coder" | "codex")] => engine(output, &root, name, word, now),
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

fn crew_status(output: &Output, args: &Args) -> Result<(), Fail> {
    let value = call(args, &Operation::CrewStatus {})?;
    output.emit(&value, |v| {
        serde_json::to_string_pretty(v).unwrap_or_default()
    });
    Ok(())
}

fn crew_control(output: &Output, args: &Args, action: &str) -> Result<(), Fail> {
    use coder_access::crew::{Control, ControlAction, Selection};
    let selection = match (args.switch("all"), args.option("members")) {
        (true, None) => Selection::AllSales,
        (false, Some(names)) => Selection::Members(names.split(',').map(str::to_owned).collect()),
        _ => {
            return Err(Fail::Failed(
                "Select exactly one of --all or --members NAMES.".into(),
            ));
        }
    };
    let control = Control {
        cohort: args
            .option("cohort")
            .ok_or_else(|| Fail::Failed("Name the owner cohort with --cohort NAME.".into()))?
            .into(),
        selection,
        action: match action {
            "stop" => ControlAction::Stop,
            "pause" => ControlAction::Pause,
            _ => ControlAction::Resume,
        },
        expected: args.option("expected").map(str::to_owned),
        reason: args
            .option("reason")
            .unwrap_or("Owner crew control from the command line.")
            .into(),
    };
    control
        .validate()
        .map_err(|e| Fail::Failed(e.to_string()))?;
    let value = call(args, &Operation::ControlCrew { control })?;
    output.emit(&value, |v| {
        serde_json::to_string_pretty(v).unwrap_or_default()
    });
    Ok(())
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

/// How sentences about agent `name` refer to it.
fn refer(root: &Path, name: &str) -> agent::Refer {
    Store::new(root, name).map_or_else(|_| agent::Refer::for_name(name), |s| s.refer())
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

fn charter(output: &Output, name: &str, args: &Args) -> Result<(), Fail> {
    let role = args
        .option("role")
        .ok_or_else(|| Fail::Failed("Choose --role for the crew charter.".into()))?;
    let job_role = coder_access::crew::JobRole::parse(role).map_err(|e| Fail::Failed(e.message))?;
    let expected = args.number("expected", 0_u64).map_err(Fail::Failed)?;
    let drafting = match args.option("drafting") {
        Some("on") => true,
        Some("off") => false,
        _ => return Err(Fail::Failed("Choose --drafting on or off.".into())),
    };
    let purpose = args
        .option("purpose")
        .ok_or_else(|| Fail::Failed("Supply --purpose text.".into()))?
        .into();
    send(
        output,
        args,
        &Operation::SetAgentCharter {
            agent: name.into(),
            job_role,
            expected,
            drafting,
            purpose,
        },
        "The owner changed the crew charter; it grants no tools or action approval.",
    )
}

fn verdict(output: &Output, name: &str, file: &str, args: &Args) -> Result<(), Fail> {
    use std::io::Read;
    let metadata = std::fs::symlink_metadata(file)
        .map_err(|_| Fail::Failed("Cannot read the crew verdict input.".into()))?;
    if !metadata.is_file() || metadata.len() > 32 * 1024 {
        return Err(Fail::Failed(
            "A crew verdict input must be a regular file of at most 32 KiB.".into(),
        ));
    }
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let opened = options
        .open(file)
        .map_err(|_| Fail::Failed("Cannot read the crew verdict input.".into()))?;
    let current = opened
        .metadata()
        .map_err(|_| Fail::Failed("Cannot read the crew verdict input.".into()))?;
    if !current.is_file() || current.len() > 32 * 1024 {
        return Err(Fail::Failed(
            "The crew verdict input is unavailable or exceeds its bound.".into(),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if current.dev() != metadata.dev() || current.ino() != metadata.ino() {
            return Err(Fail::Failed(
                "The crew verdict input changed while it was opened.".into(),
            ));
        }
    }
    let mut bytes = Vec::new();
    opened
        .take(32 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Fail::Failed("Cannot read the crew verdict input.".into()))?;
    if bytes.len() > 32 * 1024 {
        return Err(Fail::Failed(
            "The crew verdict input exceeds 32 KiB.".into(),
        ));
    }
    let verdict: coder_access::crew::VerdictInput = serde_json::from_slice(&bytes)
        .map_err(|_| Fail::Failed("The crew verdict input is malformed.".into()))?;
    verdict.validate().map_err(|e| Fail::Failed(e.message))?;
    send(
        output,
        args,
        &Operation::RecordAgentVerdict {
            agent: name.into(),
            verdict,
        },
        "The host retained a signed private recommendation, without approving an action.",
    )
}

fn verdicts(output: &Output, root: &Path, name: &str, args: &Args) -> Result<(), Fail> {
    let value = match call(args, &Operation::ListAgentVerdicts { agent: name.into() }) {
        Ok(value) => value,
        Err(Fail::Refused(refusal)) if refusal.code == "unavailable" => {
            let (store, _) = store(root, name)?;
            json!({"verdicts": store.crew_verdicts().map_err(Fail::Failed)?})
        }
        Err(error) => return Err(error),
    };
    output.emit(&value, |value| {
        serde_json::to_string_pretty(value).unwrap_or_default()
    });
    Ok(())
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
    let preset = match args.option("preset") {
        Some(named) => Some(agent::preset(named).ok_or_else(|| {
            let known: Vec<&str> = agent::PRESETS.iter().map(|p| p.name).collect();
            Fail::Failed(format!(
                "there is no preset named {named}; the presets are {}",
                known.join(", ")
            ))
        })?),
        None => agent::preset(name),
    };
    let job_role = args
        .option("role")
        .map(coder_access::crew::JobRole::parse)
        .transpose()
        .map_err(|e| Fail::Failed(e.message))?
        .or_else(|| preset.and_then(|p| p.job_role));
    if let Some(job_role) = job_role {
        if args.option("owner-key").is_some() || args.option("route").is_some() {
            return Err(Fail::Failed(
                "Sales creation uses the running host's owner admission; omit owner-key and route."
                    .into(),
            ));
        }
        return send(
            output,
            args,
            &Operation::NewCrewAgent {
                agent: name.into(),
                workspace: workspace.display().to_string(),
                job_role,
            },
            "The host created the sales member with its private drafting charter.",
        );
    }
    let mut record = store
        .open_as(&workspace, now, preset)
        .map_err(Fail::Failed)?;
    if let Some(route) = args.option("route") {
        coder::task::studio::parse_route(route).map_err(|e| Fail::Failed(e.to_string()))?;
        record.route = route.into();
        store.save(&record).map_err(Fail::Failed)?;
    }
    // An agent that has a key keeps it; the host may hold it in its
    // keychain, out of this command's reach.
    if record.pubkey.is_none() || record.state == State::Retired {
        record = store.ensure_key(record, now).map_err(Fail::Failed)?;
    }
    if let Some(owner) = owner_key(args)? {
        record = store
            .attest(record, &owner, expiry(args, now)?, now)
            .map_err(Fail::Failed)?;
    }
    let value = record_json(&record, now);
    output.emit(&value, |_| {
        let p = record.refer();
        let mut text = format!(
            "{} {name}. {} key is {}.",
            if existed { "Opened" } else { "Made" },
            p.their_cap(),
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
            "\n{} works in {}. {} record and journal: {}",
            p.they_cap(),
            record.workspace,
            p.their_cap(),
            store.dir().display()
        ));
        text
    });
    Ok(())
}

/// `attest`, or `renew` when `renewing`: the owner signs her key's
/// attestation again, and her profile is signed with it when her key is
/// here. Only an agent with no key yet gets one; a key she had is never
/// replaced.
fn attest(
    output: &Output,
    root: &Path,
    name: &str,
    args: &Args,
    now: u64,
    renewing: bool,
) -> Result<(), Fail> {
    let (store, record) = store(root, name)?;
    let verb = if renewing { "renew" } else { "attest" };
    let owner =
        owner_key(args)?.ok_or_else(|| Fail::Failed(format!("{verb} needs --owner-key FILE")))?;
    let record = if record.pubkey.is_none() && !renewing {
        store.ensure_key(record, now).map_err(Fail::Failed)?
    } else {
        record
    };
    if record.pubkey.is_none() {
        return Err(Fail::Failed(format!(
            "{name} has no key to renew; attest it with `openagents agent attest {name}`"
        )));
    }
    let record = store
        .attest(record, &owner, expiry(args, now)?, now)
        .map_err(Fail::Failed)?;
    let profile = coder::task::agent_profile::load(&store)
        .ok()
        .flatten()
        .is_some_and(|event| coder::task::agent_profile::current(&record, &event));
    let mut value = record_json(&record, now);
    value["profile_signed"] = json!(profile);
    output.emit(&value, |v| {
        let mut text = format!(
            "{} {name}'s key until {}.",
            if renewing { "Renewed" } else { "Attested" },
            v["attested_until"]
        );
        let p = record.refer();
        text.push_str(&if profile {
            format!(
                " {} profile is signed with the new attestation.",
                p.their_cap()
            )
        } else {
            format!(
                " The host signs {} profile with it when it next opens {}.",
                p.their(),
                p.them()
            )
        });
        text
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
    let authorized = attested_until
        .zip(record.attestation.as_ref())
        .map(|(until, a)| wire::authorized_line(&a.owner, until, now));
    json!({
        "authorized": authorized,
        "authorized_by": attested_until.and(record.attestation.as_ref()).map(|a| a.owner.clone()),
        "renew": attested_until.and_then(|until| wire::renewal_warning(until, now)),
        "definition": record.definition(),
        "roles": record.roles,
        "job_role": record.job_role,
        "crew_charter": record.crew_charter,
        "name": record.name,
        "state": record.state.word(),
        "workspace": record.workspace,
        "look": record.look,
        "route": record.route,
        "engine": if record.engine.is_empty() { "coder" } else { record.engine.as_str() },
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
            text.push("(no host answered; read from the agents' records)".into());
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
    let spend = spend_json(&store, &record, args, now)?;
    let value = json!({
        "record": record_json(&record, now),
        "view": view,
        "jobs": jobs,
        "spend": spend,
        "host": live,
    });
    output.emit(&value, |v| {
        let mut text = vec![
            format!("{name} ({}) at desk {}", record.state.word(), record.desk),
            format!(
                "key: {} (in the {})",
                record.pubkey.as_deref().unwrap_or("none"),
                store.custody_kind()
            ),
            format!(
                "attested until: {}",
                v["record"]["attested_until"]
                    .as_u64()
                    .map_or("not attested".to_string(), |at| at.to_string())
            ),
        ];
        if let Some(authorized) = v["record"]["authorized"].as_str() {
            text.push(authorized.to_string());
        }
        if let Some(role) = record.job_role {
            text.push(format!("job role: {} (no authority grant)", role.name()));
            if let Some(charter) = &record.crew_charter {
                text.push(format!(
                    "machine charter {}: drafting {}; model tools disabled",
                    charter.revision,
                    if charter.drafting { "on" } else { "off" }
                ));
            }
        }
        text.extend([
            format!("works in: {}", record.workspace),
            format!("charter: {}", record.charter),
        ]);
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
        text.extend(spend_lines(&v["spend"]));
        text.join("\n")
    });
    Ok(())
}

/// Her spend from her NIP-AM records: decrypted with the owner key when
/// `--owner-key` names one, else with her own key here.
fn spend_json(store: &Store, record: &Record, args: &Args, now: u64) -> Result<Value, Fail> {
    use coder::task::agent_spend::{self, Tally};
    let tally = |t: Tally| {
        json!({"records": t.records, "usd": t.usd, "unpriced": t.unpriced, "tokens": t.tokens,
               "words": t.words()})
    };
    let budget = match agent_spend::Budget::load(store) {
        Ok(budget) => json!(budget),
        Err(why) => json!({"error": why}),
    };
    let (read, by) = match owner_key(args)? {
        Some(owner) => (agent_spend::owner_read(store, &owner), "owner key"),
        None => (agent_spend::agent_read(store, record), "her key"),
    };
    Ok(match read {
        Ok(view) => json!({
            "read_with": by,
            "today": tally(view.tally(agent_spend::day_of(now))),
            "total": tally(view.tally(0)),
            "problems": view.problems,
            "budget": budget,
        }),
        Err(why) => json!({"error": why, "budget": budget}),
    })
}

fn spend_lines(spend: &Value) -> Vec<String> {
    let budget = &spend["budget"];
    let mut lines = vec![match budget["error"].as_str() {
        Some(why) => format!("budget: unreadable, so she starts nothing ({why})"),
        None => format!(
            "budget: ${:.2} and {} tokens a day, ${:.2} and {} tokens a request",
            budget["daily_usd"].as_f64().unwrap_or_default(),
            budget["daily_tokens"],
            budget["request_usd"].as_f64().unwrap_or_default(),
            budget["request_tokens"],
        ),
    }];
    if let Some(why) = spend["error"].as_str() {
        lines.push(format!("spend: not read ({why})"));
        return lines;
    }
    for (label, key) in [("today", "today"), ("in all", "total")] {
        lines.push(format!(
            "spend {label}: {}",
            spend[key]["words"].as_str().unwrap_or_default()
        ));
    }
    for problem in spend["problems"].as_array().into_iter().flatten() {
        lines.push(format!(
            "spend record not read: {}",
            problem.as_str().unwrap_or_default()
        ));
    }
    lines
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
            format!(
                "Asked {name}. Follow {} with `openagents agent show {name}`.",
                agent::Refer::for_name(name).them()
            )
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
            return Err(Fail::Failed(format!(
                "{} has not reported in an hour",
                agent::Refer::for_name(name).they()
            )));
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

/// The running host, when one answers on its control socket.
fn live_host(args: &Args) -> Option<Host> {
    host(args).ok().filter(Host::answers)
}

/// Sends the owner's NIP-IA archive request to her relays, as the owner.
fn send_archive(
    store: &Store,
    owner: &secp256k1::SecretKey,
    request: &nostr::domain::Event,
    relays: &[String],
    now: u64,
) -> Vec<lifecycle::Sent> {
    lifecycle::send_archive(
        store,
        owner,
        request,
        relays,
        &coder::task::agent_sync::Live,
        now,
    )
}

fn archive_line(sent: &[lifecycle::Sent]) -> String {
    if sent.is_empty() {
        return String::new();
    }
    let took = sent.iter().filter(|s| s.accepted).count();
    format!(
        " {took} of {} relays took the owner's request to archive the key.",
        sent.len()
    )
}

/// `retire`: through the running host, which deletes her key from its key
/// store (the keychain when it runs with one); else from her files here.
fn retire(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let owner = owner_key(args)?;
    let (store, _) = store(root, name)?;
    if let Some(mut host) = live_host(args) {
        match host.call(&Operation::RetireAgent { agent: name.into() }) {
            Ok(Outcome::Agent { agent: value }) => {
                // The host asks her relays itself when it holds the owner
                // key; otherwise this command does, with the owner key.
                let mut sent = Vec::new();
                if value["archive_request"].is_null()
                    && let Some(owner) = &owner
                    && let Some((request, relays)) =
                        lifecycle::retired_archive(&store, owner, now).map_err(Fail::Failed)?
                {
                    sent = send_archive(&store, owner, &request, &relays, now);
                }
                let deleted = value["key_deleted"] == true;
                let value =
                    json!({"retired": name, "key_deleted": deleted, "host": true, "archive": sent});
                output.emit(&value, |_| {
                    format!(
                        "Retired {name} at the host: {their} key is deleted, and {their} \
                         journal and engrams stay for the owner key.{}",
                        archive_line(&sent),
                        their = store.refer().their()
                    )
                });
                return Ok(());
            }
            Ok(_) => return Err(Fail::Failed("the host answered another operation".into())),
            // A host from before `studio.agent.retire`: stop her there and
            // retire her from her files.
            Err(refusal) if refusal.code == "unsupported" || refusal.code == "malformed" => {
                let _ = call(
                    args,
                    &Operation::StopAgent {
                        agent: name.into(),
                        reason: "retired".into(),
                    },
                );
            }
            Err(refusal) => return Err(Fail::Refused(refusal)),
        }
    }
    let p = store.refer();
    let retired = lifecycle::retire(&store, owner.as_ref(), now).map_err(Fail::Failed)?;
    let sent = match (&retired.archive, &owner) {
        (Some(request), Some(owner)) => send_archive(&store, owner, request, &retired.relays, now),
        _ => Vec::new(),
    };
    let value = json!({"retired": name, "key_deleted": retired.key_deleted, "host": false, "archive": sent});
    output.emit(&value, |_| {
        format!(
            "Retired {name}: {}, and {} journal and engrams stay for the owner key.{}",
            if retired.key_deleted {
                format!("{} key is deleted", p.their())
            } else {
                format!("{} had no key here", p.they())
            },
            p.their(),
            archive_line(&sent)
        )
    });
    Ok(())
}

/// `rotate`: through the running host, with the owner key it holds; else
/// here, with her key in a file and the owner key in FILE.
fn rotate(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let reason = args.option("reason").unwrap_or_default().to_string();
    let owner = owner_key(args)?;
    let (store, record) = store(root, name)?;
    let p = record.refer();
    let (they, _, their) = p.words();
    let here = store.key().ok().flatten().is_some();
    if let Some(mut host) = live_host(args) {
        match host.call(&Operation::RotateAgent {
            agent: name.into(),
            reason: reason.clone(),
        }) {
            Ok(Outcome::Agent { agent: value }) => {
                output.emit(&value, |v| {
                    format!(
                        "Rotated {name} at the host: {their} key is {}, and {} engrams are \
                         encrypted under it. Grants to {their} old key don't carry over; \
                         delegate again any {they} needs.",
                        v["new"].as_str().unwrap_or("new"),
                        v["engrams"]
                    )
                });
                return Ok(());
            }
            Ok(_) => return Err(Fail::Failed("the host answered another operation".into())),
            Err(_) if owner.is_some() && here => {}
            Err(refusal) => return Err(Fail::Refused(refusal)),
        }
    }
    let owner = owner.ok_or_else(|| {
        Fail::Failed("no host answered, so rotate needs the owner key: --owner-key FILE".into())
    })?;
    let screen = secret_screen_shapes();
    let rotated = lifecycle::rotate(&store, &screen, &owner, &reason, expiry(args, now)?, now)
        .map_err(Fail::Failed)?;
    let sent = match &rotated.archive {
        Some(request) => send_archive(&store, &owner, request, &rotated.relays, now),
        None => Vec::new(),
    };
    // Her heads, profile, and relay list go out under the new key now.
    let status = (!rotated.relays.is_empty()).then(|| {
        coder::task::agent_sync::sync(&store, &screen, &coder::task::agent_sync::Live, now)
    });
    let value = json!({
        "agent": name,
        "old": rotated.old,
        "new": rotated.new,
        "engrams": rotated.engrams,
        "lineage": rotated.lineage,
        "archive": sent,
        "sync": status,
    });
    output.emit(&value, |_| {
        format!(
            "Rotated {name}: {their} key {} is now {}, and {} engrams are encrypted under it. \
             The owner signed the lineage record. Grants to {their} old key don't carry over; \
             delegate again any {they} needs.{}",
            rotated.old,
            rotated.new,
            rotated.engrams,
            archive_line(&sent)
        )
    });
    Ok(())
}

/// `move`: marks her moved to the owner's other computer.
fn move_to(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let to = args.option("to").ok_or_else(|| {
        Fail::Failed("move needs --to HOSTKEY, the other computer's host key".into())
    })?;
    let (store, record) = store(root, name)?;
    let _ = call(
        args,
        &Operation::StopAgent {
            agent: name.into(),
            reason: "moved to another computer".into(),
        },
    );
    lifecycle::mark_moved(&store, to, now).map_err(Fail::Failed)?;
    let p = record.refer();
    let (they, them, their) = p.words();
    output.emit(&json!({"moved": name, "to": to}), |_| {
        format!(
            "Moved {name}: the computer {to} runs {them} now, and this one runs nothing of {}. \
             That computer's host must hold {their} key and grant it what {they} needs there.",
            p.theirs()
        )
    });
    Ok(())
}

/// `export`: a snapshot without her key, mode 0600, never over a file.
fn export(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    use std::io::Write;
    let out = args
        .option("out")
        .ok_or_else(|| Fail::Failed("export needs --out FILE".into()))?;
    let choice = lifecycle::MemoryChoice::parse(args.option("memory").unwrap_or("none"))
        .map_err(Fail::Failed)?;
    let (store, record) = store(root, name)?;
    let their = record.refer().their().to_string();
    let snapshot = lifecycle::export(
        &store,
        &secret_screen_shapes(),
        choice,
        owner_key(args)?.as_ref(),
        now,
    )
    .map_err(Fail::Failed)?;
    let body = serde_json::to_vec_pretty(&snapshot).map_err(|e| Fail::Failed(e.to_string()))?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options
        .open(out)
        .and_then(|mut file| file.write_all(&body).and_then(|()| file.sync_all()))
        .map_err(|e| Fail::Failed(format!("cannot write {out}: {e}")))?;
    let value = json!({
        "exported": name,
        "out": out,
        "memory": choice,
        "core": snapshot.core.is_some(),
        "entries": snapshot.entries.len(),
    });
    output.emit(&value, |_| {
        format!(
            "Exported {name} to {out} without {their} key: {their} definition{}{}.",
            if snapshot.core.is_some() {
                format!(", {their} core")
            } else {
                String::new()
            },
            if snapshot.entries.is_empty() {
                String::new()
            } else {
                format!(
                    ", and {} memory entries as plaintext",
                    snapshot.entries.len()
                )
            }
        )
    });
    Ok(())
}

/// `signing NAME on|off`: NIP-GS signatures on her merged changes.
fn signing(output: &Output, root: &Path, name: &str, on: bool, now: u64) -> Result<(), Fail> {
    let (store, _) = store(root, name)?;
    let settings = coder::task::agent_git_sign::set(&store, on, now).map_err(Fail::Failed)?;
    output.emit(&json!({"agent": name, "signing": settings}), |_| {
        if on {
            format!(
                "The tip of each change of {name}'s you merge is now signed with {} key, with \
                 your attestation embedded (NIP-GS).",
                store.refer().their()
            )
        } else {
            format!("{name}'s merged changes are no longer signed.")
        }
    });
    Ok(())
}

/// `xp-link`: the agent's side of an XP key link to the owner, signed
/// here and printed, and the entry the owner's trainer profile adds.
fn xp_link(output: &Output, root: &Path, name: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let owner = owner_key(args)?
        .ok_or_else(|| Fail::Failed("xp-link needs the owner key: --owner-key FILE".into()))?;
    let (store, record) = store(root, name)?;
    let trainer = agent::public_hex(&owner);
    let linked =
        coder::task::agent_profile::xp_link(&store, &trainer, now).map_err(Fail::Failed)?;
    let value = json!({
        "agent": name,
        "link": linked.link,
        "trainer": linked.trainer,
        "profile_key": linked.key,
    });
    output.emit(&value, |_| {
        format!(
            "Signed {name}'s XP key link to the trainer {trainer}. Nothing was published.\n\
             Add {} key to the keys of your trainer profile (13193) so the link holds both \
             ways:\n  {}\nThe signed link (13195):\n  {}",
            record.refer().their(),
            linked.key,
            serde_json::to_string(&linked.link).unwrap_or_default()
        )
    });
    Ok(())
}

/// `engine NAME coder|codex`: what does her coding. The host reads her
/// record at each request, so the next one uses it.
fn engine(output: &Output, root: &Path, name: &str, word: &str, now: u64) -> Result<(), Fail> {
    let (store, mut record) = store(root, name)?;
    record.engine = agent::parse_engine(word).map_err(Fail::Failed)?;
    store.save(&record).map_err(Fail::Failed)?;
    let line = if record.codes_on_codex() {
        "the owner set her engine to codex: Coder delegates her coding to Codex"
    } else {
        "the owner set her engine to coder: Coder codes on its own model"
    };
    let _ = store.append(&agent::Entry::new(now, agent::Kind::Control, line));
    output.emit(
        &json!({"agent": name, "engine": if record.engine.is_empty() { "coder" } else { record.engine.as_str() }}),
        |_| {
            if record.codes_on_codex() {
                format!(
                    "{name} now codes on Codex: Coder hands her coding to the Codex agent on \
                     your ChatGPT login and checks it. When Codex is out of capacity, Coder \
                     works on its own model."
                )
            } else {
                format!("{name} now codes on Coder's own model.")
            }
        },
    );
    Ok(())
}

/// `import`: a new agent from a snapshot, with a new key.
fn import(output: &Output, root: &Path, file: &str, args: &Args, now: u64) -> Result<(), Fail> {
    let name = args
        .option("name")
        .ok_or_else(|| Fail::Failed("import needs --name NAME for the new agent".into()))?;
    let text = std::fs::read_to_string(file)
        .map_err(|e| Fail::Failed(format!("cannot read {file}: {e}")))?;
    let snapshot: lifecycle::Snapshot = serde_json::from_str(&text)
        .map_err(|e| Fail::Failed(format!("{file} is not an agent snapshot: {e}")))?;
    let store = Store::new(root, name).map_err(Fail::Failed)?;
    let workspace = args
        .option("workspace")
        .map(PathBuf::from)
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"));
    let owner = owner_key(args)?;
    let record = lifecycle::import(
        &store,
        &secret_screen_shapes(),
        &snapshot,
        &workspace,
        owner.as_ref(),
        expiry(args, now)?,
        now,
    )
    .map_err(Fail::Failed)?;
    let value = record_json(&record, now);
    output.emit(&value, |_| {
        format!(
            "Made {name} from a snapshot of {} with a new key, {}.{}",
            snapshot.name,
            record.pubkey.as_deref().unwrap_or("missing"),
            if record.attestation.is_some() {
                ""
            } else {
                " Attest it to encrypt the agent's memory: openagents agent attest NAME \
                 --owner-key FILE"
            }
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
        ["engrams"] if args.switch("from-relay") => {
            return engrams_from_relay(output, root, name, args);
        }
        ["engrams"] if args.switch("orphans") => return orphans(output, root, name, args),
        ["engrams"] => return engrams(output, root, name, args),
        ["sync", verb] => return sync(output, root, name, verb, args),
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
            let mut memory = coder::task::agent_consolidate::rows(&store, &secret_screen_shapes());
            memory.extend(
                coder::task::agent_memory::Memory::new(store.clone(), secret_screen_shapes())
                    .rows(None)
                    .map_err(Fail::Failed)?,
            );
            wire::Memory {
                drafts: coder::task::agent_share::draft_rows(&store).map_err(Fail::Failed)?,
                memory,
            }
        }
        Err(other) => return Err(other),
    };
    output.emit(&json!(memory), |_| {
        let mut lines: Vec<String> = memory
            .memory
            .iter()
            .map(|m| format!("{:>4} {:<10} {:<9} {}", m.id, m.kind, m.state, m.text))
            .collect();
        if lines.is_empty() {
            lines.push(format!("{name} remembers nothing yet."));
        }
        if !memory.drafts.is_empty() {
            lines.push(format!(
                "\nKnowledge drafts waiting for you ({}); nothing is published until you run:",
                memory.drafts.len()
            ));
            for draft in &memory.drafts {
                lines.push(format!(
                    "  {} ({}): {}\n    {}",
                    draft.id, draft.kind, draft.title, draft.publish
                ));
            }
        }
        lines.join("\n")
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

/// The memories her `core` does not reach, and the links that name a
/// missing memory, read with her key here or with the owner key.
fn orphans(output: &Output, root: &Path, name: &str, args: &Args) -> Result<(), Fail> {
    use coder::task::agent_engrams::{self, EngramStore, Opened};
    let (store, _) = store(root, name)?;
    let reach = if let Some(owner) = owner_key(args)? {
        let view = agent_engrams::owner_read(&store, &owner).map_err(Fail::Failed)?;
        if !view.problems.is_empty() {
            return Err(Fail::Failed(format!(
                "{name}'s engrams do not all read, so no orphan list is complete: {}",
                view.problems.join("; ")
            )));
        }
        agent_engrams::reach(&view.heads)
    } else {
        match EngramStore::read(&store, &secret_screen_shapes()) {
            Opened::Ready(engrams) => engrams.reach(),
            Opened::Skipped(why) => {
                let said = format!("{name} keeps no engrams: {why}.");
                output.emit(&json!({"orphans": [], "skipped": why}), |_| said.clone());
                return Ok(());
            }
            Opened::Unreadable(why) => {
                return Err(Fail::Failed(format!(
                    "{name}'s engram store cannot be read, so nothing from it is carried: {why}"
                )));
            }
        }
    };
    output.emit(&json!(reach), |_| {
        let mut lines = Vec::new();
        if !reach.core {
            lines.push(format!(
                "{name} has no core yet, so every memory is an orphan."
            ));
        }
        lines.push(format!(
            "{} memories reachable from her core, {} orphans; nothing deletes an orphan.",
            reach.reachable.len(),
            reach.orphans.len()
        ));
        for orphan in &reach.orphans {
            lines.push(format!("  orphan  {orphan}"));
        }
        for dangling in &reach.dangling {
            lines.push(format!(
                "  missing {} (linked from {})",
                dangling.to, dangling.from
            ));
        }
        lines.join("\n")
    });
    Ok(())
}

/// Her heads from relays, decrypted with the owner key alone.
fn engrams_from_relay(output: &Output, root: &Path, name: &str, args: &Args) -> Result<(), Fail> {
    use coder::task::agent_sync;
    let owner = owner_key(args)?.ok_or_else(|| {
        Fail::Failed("--from-relay reads with the owner key: --owner-key FILE".into())
    })?;
    let store = Store::new(root, name).map_err(Fail::Failed)?;
    let record = store.load().map_err(Fail::Failed)?;
    let agent_hex = match (
        args.option("agent"),
        record.as_ref().and_then(|r| r.pubkey.clone()),
    ) {
        (Some(hex), _) => hex.to_string(),
        (None, Some(hex)) => hex,
        (None, None) => {
            return Err(Fail::Failed(format!(
                "{name} isn't on this computer; name {} key with --agent HEX",
                agent::Refer::for_name(name).their()
            )));
        }
    };
    coder::task::sales::privacy::check_relay_read(&store, &agent_hex).map_err(Fail::Failed)?;
    let agent = agent_hex
        .parse::<secp256k1::XOnlyPublicKey>()
        .map_err(|_| Fail::Failed(format!("{agent_hex} isn't a public key")))?;
    let mut relays: Vec<String> = args
        .options("relay")
        .into_iter()
        .map(str::to_string)
        .collect();
    if relays.is_empty() && record.is_some() {
        relays = agent_sync::Settings::load(&store)
            .map_err(Fail::Failed)?
            .memory_relays;
    }
    let view =
        agent_sync::owner_read(&agent, &owner, &relays, &agent_sync::Live).map_err(Fail::Failed)?;
    coder::task::sales::privacy::check_relay_read(&store, &agent_hex).map_err(Fail::Failed)?;
    for head in &view.heads {
        coder::task::sales::privacy::check_memory_projection(&store, &head.body.to_json())
            .map_err(Fail::Failed)?;
    }
    let heads: Vec<Value> = view.heads.iter().map(|h| head_json(h, true)).collect();
    let value = json!({
        "heads": heads,
        "relays": view.relays,
        "forgotten": view.forgotten,
        "problems": view.problems,
        "truncated": view.truncated,
        "decrypted": true,
    });
    output.emit(&value, |_| {
        let mut lines: Vec<String> = vec![format!("From {}:", view.relays.join(", "))];
        for h in &heads {
            let text = h["body"]
                .get("profile")
                .or_else(|| h["body"].get("value"))
                .and_then(Value::as_str)
                .unwrap_or("");
            lines.push(format!(
                "{:<28} {:>10} {}\n    {}",
                h["slug"].as_str().unwrap_or(""),
                h["created_at"],
                h["id"].as_str().unwrap_or(""),
                agent::ascii(text).replace('\n', "\n    ")
            ));
        }
        if heads.is_empty() {
            lines.push(format!("These relays hold no heads of {name}'s."));
        }
        if view.forgotten > 0 {
            lines.push(format!("{} entries forgotten.", view.forgotten));
        }
        for url in &view.truncated {
            lines.push(format!("{url} answered with its limit and may hold more."));
        }
        for problem in &view.problems {
            lines.push(format!("not read: {problem}"));
        }
        lines.join("\n")
    });
    Ok(())
}

/// Her relay sync settings and last pass, as the command prints them.
fn sync_lines(
    name: &str,
    settings: &coder::task::agent_sync::Settings,
    status: Option<&coder::task::agent_sync::Status>,
) -> String {
    if !settings.on() {
        return format!("Relay sync is off for {name}.");
    }
    let mut lines = vec![format!(
        "{name} syncs with {}.",
        settings.memory_relays.join(", ")
    )];
    let Some(status) = status else {
        lines.push("No pass has run yet.".into());
        return lines.join("\n");
    };
    lines.push(format!(
        "Last pass at {}: {} heads taken, {} published.",
        status.at, status.pulled, status.pushed
    ));
    if let Some(error) = &status.error {
        lines.push(format!("It didn't finish: {error}"));
    }
    for relay in &status.relays {
        let mut line = format!("  {} holds {} heads", relay.url, relay.heads);
        if relay.truncated {
            line.push_str(" and may hold more (limit reached)");
        }
        if let Some(error) = &relay.error {
            line.push_str(&format!("; error: {error}"));
        }
        lines.push(line);
        for refused in &relay.refused {
            lines.push(format!("    refused {refused}"));
        }
    }
    for conflict in &status.conflicts {
        lines.push(format!("Conflict: {conflict}"));
    }
    lines.join("\n")
}

/// `memory NAME sync on|off|status|now`: the owner's own settings at the
/// host, and a pass on demand.
fn sync(output: &Output, root: &Path, name: &str, verb: &str, args: &Args) -> Result<(), Fail> {
    use coder::task::agent_sync::{self, Settings, Status};
    let (store, record) = store(root, name)?;
    let now = coder::task::autostart::unix_now();
    match verb {
        "on" | "off" => {
            let relays: Vec<String> = if verb == "off" {
                Vec::new()
            } else {
                let named: Vec<String> = args
                    .options("relay")
                    .into_iter()
                    .map(str::to_string)
                    .collect();
                if named.is_empty() {
                    vec![agent_sync::DEFAULT_RELAY.to_string()]
                } else {
                    named
                }
            };
            if verb == "on" && record.attestation.is_none() {
                return Err(Fail::Failed(format!(
                    "{name} has no owner attestation to present to a relay; attest {} key first",
                    record.refer().their()
                )));
            }
            let settings = agent_sync::set_relays(&store, &relays, now).map_err(Fail::Failed)?;
            output.emit(&json!({"settings": settings}), |_| {
                sync_lines(name, &settings, None)
            });
        }
        "status" => {
            let settings = Settings::load(&store).map_err(Fail::Failed)?;
            let status = Status::load(&store).map_err(Fail::Failed)?;
            output.emit(&json!({"settings": settings, "status": status}), |_| {
                sync_lines(name, &settings, status.as_ref())
            });
        }
        "now" => {
            let settings = Settings::load(&store).map_err(Fail::Failed)?;
            if !settings.on() {
                return Err(Fail::Failed(format!(
                    "relay sync is off for {name}; turn it on with: openagents agent memory {name} sync on"
                )));
            }
            if store.custody(&record).is_err() || store.key().ok().flatten().is_none() {
                agent_sync::request(&store, now).map_err(Fail::Failed)?;
                let said = format!(
                    "{} key isn't in a file here, so the host runs a pass at its next sweep. \
                     Check it with: openagents agent memory {name} sync status",
                    record.refer().their_cap()
                );
                output.emit(&json!({"requested_at": now}), |_| said.clone());
                return Ok(());
            }
            let status = agent_sync::sync(&store, &secret_screen_shapes(), &agent_sync::Live, now);
            output.emit(&json!({"settings": settings, "status": status}), |_| {
                sync_lines(name, &settings, Some(&status))
            });
            if let Some(error) = &status.error {
                return Err(Fail::Failed(format!("the pass didn't finish: {error}")));
            }
        }
        _ => return Err(Fail::Failed(format!("unknown sync command; {USAGE}"))),
    }
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
        assert!(attest(&output, &root, "alice", &long, now, false).is_err());
        // Renewal: a new expiry, the same key, and her profile signed again.
        let renew = args(&[
            "renew",
            "alice",
            "--owner-key",
            owner.to_str().unwrap(),
            "--days",
            "200",
        ]);
        assert!(attest(&output, &root, "alice", &renew, now + 10, true).is_ok());
        let (store, renewed) = super::store(&root, "alice").ok().unwrap();
        assert_eq!(renewed.pubkey.as_deref(), Some(pubkey.as_str()));
        let value = record_json(&renewed, now + 10);
        assert_eq!(value["attested_until"], now + 10 + 200 * 86_400);
        assert!(
            value["authorized"]
                .as_str()
                .unwrap()
                .starts_with("authorized by npub1")
        );
        assert!(value["renew"].is_null());
        let near = record_json(&renewed, now + 10 + 190 * 86_400);
        assert!(near["renew"].as_str().unwrap().contains("10 days"));
        let profile = coder::task::agent_profile::load(&store).unwrap().unwrap();
        assert!(coder::task::agent_profile::current(&renewed, &profile));
        assert!(
            nostr::domain::verify_owner_attestation(&profile)
                .unwrap()
                .is_some()
        );
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
    fn rotate_export_import_and_move_without_a_host() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let owner = dir.path().join("owner.key");
        std::fs::write(&owner, "07".repeat(32)).unwrap();
        let owner = owner.to_str().unwrap();
        let socket = dir.path().join("none.sock");
        let socket = socket.to_str().unwrap();
        let output = Output::new(true);
        let now = 1_791_158_400;
        let workspace = dir.path().to_str().unwrap();
        let made = args(&[
            "new",
            "alice",
            "--owner-key",
            owner,
            "--workspace",
            workspace,
        ]);
        assert!(new(&output, &root, "alice", &made, now).is_ok());
        let (_, before) = store(&root, "alice").ok().unwrap();

        // Rotation needs the owner key, then gives her a new attested key.
        let bare = args(&["rotate", "alice", "--control-socket", socket]);
        assert!(rotate(&output, &root, "alice", &bare, now + 1).is_err());
        let rotated = args(&[
            "rotate",
            "alice",
            "--owner-key",
            owner,
            "--reason",
            "routine",
            "--control-socket",
            socket,
        ]);
        assert!(rotate(&output, &root, "alice", &rotated, now + 2).is_ok());
        let (store_after, after) = store(&root, "alice").ok().unwrap();
        assert_ne!(after.pubkey, before.pubkey);
        assert!(after.attestation.is_some());
        assert_eq!(lifecycle::lineage(&store_after).unwrap().len(), 1);

        // Export never writes over a file, and import makes a new key.
        let out = dir.path().join("alice.json");
        let exported = args(&[
            "export",
            "alice",
            "--out",
            out.to_str().unwrap(),
            "--memory",
            "core",
        ]);
        assert!(export(&output, &root, "alice", &exported, now + 3).is_ok());
        assert!(export(&output, &root, "alice", &exported, now + 3).is_err());
        let imported = args(&[
            "import",
            out.to_str().unwrap(),
            "--name",
            "bob",
            "--owner-key",
            owner,
            "--workspace",
            workspace,
        ]);
        assert!(import(&output, &root, out.to_str().unwrap(), &imported, now + 4).is_ok());
        let (_, bob) = store(&root, "bob").ok().unwrap();
        assert!(bob.pubkey.is_some() && bob.pubkey != after.pubkey);

        // Moving marks her moved and names the other computer.
        let other = "ab".repeat(32);
        let moved = args(&["move", "alice", "--to", &other, "--control-socket", socket]);
        assert!(move_to(&output, &root, "alice", &moved, now + 5).is_ok());
        let (_, record) = store(&root, "alice").ok().unwrap();
        assert_eq!(record.state, State::Moved);
        assert_eq!(record.roles.unwrap().controller, other);
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
        // Her seeded core links nothing, so the note is an orphan, read
        // the same with her key and with the owner key.
        assert_eq!(
            coder::task::agent_engrams::reach(&view.heads).orphans.len(),
            1
        );
        assert!(orphans(&output, &root, "alice", &args(&["--orphans"])).is_ok());
        let both = args(&["--orphans", "--owner-key", owner.to_str().unwrap()]);
        assert!(orphans(&output, &root, "alice", &both).is_ok());
        assert!(memory_list_routes_orphans(&output, &root));
    }

    fn memory_list_routes_orphans(output: &Output, root: &Path) -> bool {
        memory(output, root, "alice", &["engrams"], &args(&["--orphans"])).is_ok()
    }

    #[test]
    fn show_reads_her_spend_with_her_key_or_the_owner_key() {
        use coder::task::agent_spend::{self, Call, Counters, Meter};
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
        let (store, record) = store(&root, "alice").ok().unwrap();
        let mut meter = Meter::open(&store, &record, now).unwrap();
        let call = Call {
            harness: agent_spend::HARNESS,
            turn_id: "plan".into(),
            model: None,
            usage: Counters {
                input_tokens: Some(90),
                output_tokens: Some(10),
                total_tokens: None,
                cost_usd: Some(0.25),
            },
            stop: "end_turn",
        };
        meter.record(&call, now).unwrap();
        let with_owner = args(&["--owner-key", owner.to_str().unwrap()]);
        for given in [args(&[]), with_owner] {
            let spend = spend_json(&store, &record, &given, now).ok().unwrap();
            assert_eq!(spend["today"]["records"], 1, "{spend}");
            assert_eq!(spend["total"]["tokens"], 100);
            let lines = spend_lines(&spend).join("\n");
            assert!(lines.contains("spend today: $0.2500 and 100 tokens over 1 records"));
            assert!(lines.contains("budget: $5.00 and 10000000 tokens a day"));
        }
        let spend = spend_json(&store, &record, &args(&[]), now + 86_400)
            .ok()
            .unwrap();
        assert_eq!(spend["today"]["records"], 0);
    }

    #[test]
    fn relay_sync_is_off_until_the_owner_turns_it_on() {
        use coder::task::agent_sync::Settings;
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let owner = dir.path().join("owner.key");
        std::fs::write(&owner, "07".repeat(32)).unwrap();
        let output = Output::new(true);
        let made = args(&[
            "new",
            "alice",
            "--owner-key",
            owner.to_str().unwrap(),
            "--workspace",
            dir.path().to_str().unwrap(),
        ]);
        assert!(new(&output, &root, "alice", &made, 1_791_158_400).is_ok());
        let (store, _) = store(&root, "alice").ok().unwrap();
        assert!(!Settings::load(&store).unwrap().on());
        // A pass on demand while off is refused before any connection.
        assert!(sync(&output, &root, "alice", "now", &args(&[])).is_err());
        assert!(
            sync(
                &output,
                &root,
                "alice",
                "on",
                &args(&["--relay", "https://x"])
            )
            .is_err()
        );
        let on = args(&[
            "--relay",
            "ws://127.0.0.1:7777/",
            "--relay",
            "WS://127.0.0.1:7777",
        ]);
        assert!(sync(&output, &root, "alice", "on", &on).is_ok());
        assert_eq!(
            Settings::load(&store).unwrap().memory_relays,
            vec!["ws://127.0.0.1:7777/"]
        );
        assert!(sync(&output, &root, "alice", "status", &args(&[])).is_ok());
        assert!(sync(&output, &root, "alice", "off", &args(&[])).is_ok());
        assert!(!Settings::load(&store).unwrap().on());
        assert!(sync(&output, &root, "alice", "bogus", &args(&[])).is_err());
        // Reading from relays needs the owner key.
        let from = args(&["--from-relay", "--relay", "ws://127.0.0.1:9"]);
        assert!(engrams_from_relay(&output, &root, "alice", &from).is_err());
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
