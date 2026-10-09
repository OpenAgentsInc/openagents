//! `coder-worker`: the other end of the relay door.
//!
//! [`coder::relay::RelayDoor`] publishes NIP-CJ job requests and waits for
//! an answer. Something has to be there to answer them, and until this
//! binary existed nothing was — the transport had a client, a relay, and a
//! specification, and no fulfillment side, so a job request went out and
//! the only thing a run could learn was that nobody came back.
//!
//! The worker subscribes to `{ "kinds": [25900, 25920], "#p": [<its pubkey>] }`,
//! decrypts each request, answers it through an Open Responses door, and
//! publishes the reply as `27000` partial feedback and one `26900` result.
//! The relay carries ciphertext and holds nothing: every kind is
//! ephemeral, so a worker that is not connected when a request is
//! published never sees it, and there is no queue to drain. For the same
//! reason a relay that drops the socket or restarts does not end the
//! worker: it reconnects with backoff, from one second up to a minute,
//! and subscribes again, so the outage costs the jobs published while it
//! lasted and nothing after. A relay that goes silent without closing the
//! socket is a fault too: the worker probes its subscription every 30
//! seconds and reconnects when a probe goes unanswered, and it renews the
//! subscription on an overlapping connection before the relay's front end
//! would end it (#9946).
//!
//! ```sh
//! export CODER_WORKER_SECRET=<64 hex or nsec>
//! export CODER_RELAY=wss://relay.openagents.com
//! export CODER_DOOR_KEY=…            # the door the worker answers through
//! export CODER_WORKER_MODEL=gemini   # the gateway lane this worker runs
//! export OPENROUTER_API_KEY=…         # the primary every turn asks first
//! coder-worker --once
//! ```
//!
//! That is the shipped chat worker's configuration
//! (`deploy/coder-worker-chat.env.example`): every turn asks the OpenRouter
//! primary (Space Bunny Alpha) first, and the gateway's `gemini` lane
//! answers any turn the primary does not.
//!
//! The lane is the worker's own. [`coder::generate::WORKER_MODEL_VAR`]
//! outranks `CODER_MODEL` because the model a service pays for is not
//! automatically the model a local user would pick, and one constant
//! cannot be both.
//!
//! A gateway door gets a primary in front of it
//! ([`coder::generate::WORKER_PRIMARY_VAR`], #10109): every turn asks
//! OpenRouter's primary first — Space Bunny Alpha at low reasoning unless
//! the variable names another, or `off` — and a turn the primary has not
//! started answering goes to the gateway door. Each result names the model
//! that wrote it.
//!
//! | Flag | Effect |
//! | --- | --- |
//! | `--once` | Answer one job, then exit. |
//! | `--decline <CODE>` | Refuse every job with this NIP-CJ error code. |
//! | `-h`, `--help` | Print the usage text. |
//!
//! `--decline` is there because a worker that refuses is one of the three
//! states a caller has to be able to tell apart, and it cannot be
//! exercised by turning something off: an absent worker and a refusing
//! worker differ precisely in that the refusing one answers.
//!
//! `CODER_WORKER_ALLOW` names the customers this worker answers, as a
//! comma-separated list of `npub` or hex public keys. A request names a
//! worker with a `p` tag and public keys are not secret, so a worker that
//! spends a door key on whoever finds it has no spend control at all. A
//! request from anyone else is refused with the typed code `not_admitted`
//! rather than dropped, because a silent refusal looks like an outage to
//! the terminal. Unset, the worker answers every request, which is right
//! for a local relay and wrong for a public one.
//!
//! Jobs run concurrently, `CODER_WORKER_JOBS` at once; unset, an executor
//! door runs as many as its manifest's `concurrent_max` and a model door
//! runs four. A job that arrives with every slot taken is refused with the
//! typed code `busy` at once, so the terminal can route it elsewhere
//! rather than wait behind work it cannot see.
//!
//! Two request shapes go beyond an ordinary turn. A payload with
//! `"type": "probe"` is answered without generating, with the door's name
//! and model, so a terminal can learn the worker is there. A payload with a
//! `delegation` object (`writes`, `minutes`) is one bounded task from a
//! fan-out on the terminal's side; an executor door runs it under its own
//! approval and boundary with those bounds, and a model door refuses a
//! writing one rather than answering an edit with prose. The worker holds
//! the bound itself too: a run still unanswered thirty seconds past its
//! stated minutes is refused `timed_out`, so no request is left without
//! an answer. Read [`docs/coder/guides/worker-executor.md`] for that path.
//!
//! `CODER_WORKER_OPEN=1` opens a worker to callers it has never met, as
//! the OpenAgents app's chat needs, with no usage limit (#10120): an open
//! caller gets conversation jobs only (no delegation and no execution),
//! and a request past the byte bound is refused `limit_exceeded`. Keys on
//! `CODER_WORKER_ALLOW` get everything. `CODER_WORKER_QUOTA` is an abuse
//! brake for emergencies only, off in the shipped configuration
//! ([`coder::relay::quota`]): unset is unlimited, and each of its counts
//! is unlimited unless named. Setting it also opens the worker.
//!
//! Every job is recorded: one JSON line per job in the usage log
//! ([`coder::relay::usage`]), `CODER_WORKER_USAGE_DIR` or, under systemd,
//! `usage/` in the unit's state directory. `coder-worker usage` reads it.
//! Day files older than `CODER_WORKER_USAGE_DAYS` (30 unless set;
//! `forever` keeps them all) are deleted at start and as each new day
//! begins (#11042).
//!
//! One request is answered once. An event whose ID the worker has already
//! seen is set aside, and a request whose `created_at` is more than ten
//! minutes old is refused `stale`: nothing on this path is stored, so an
//! old request arriving now is a replay, not a job.
//!
//! Read [`docs/coder/measurements/relay-transport.md`] for the proof this binary was
//! written to make possible.

use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::{Duration, Instant};

use coder::first;
use coder::generate::{
    Door, FallbackDoor, Generate, GenerateError, Lane, Message, Meta, OPENROUTER_KEY_VAR,
    ResponsesDoor, Role, Usage, WORKER_MODEL_VAR, WORKER_PRIMARY_VAR, model_from_env, model_named,
};
use coder::relay::liveness::{
    self, DRAIN, Liveness, PROBE_PREFIX, Renewal, Successor, next_draining, notify_watchdog,
    poll_some,
};
use coder::relay::quota::{Ledger, Policy};
use coder::relay::{
    DEFAULT_RELAY_URL, FEEDBACK_KIND, Identity, PAYLOAD_VERSION, REQUEST_KIND, RESULT_KIND, Socket,
    parse_pubkey, partial_payload, payload_version, send,
};
use coder::router::seams::{
    Ask, AuthorAsk, AuthorStep, CliAnswer, CliAsk, Continuation, Grounding, GymLookup, Lookup,
    SeamError,
};
use coder::router::wire::{Served, Shadow};
use coder::router::{self, Bank, Mode, Seams, Tier};
use futures_util::StreamExt;
use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::XOnlyPublicKey;
use serde_json::{Value, json};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio::sync::{mpsc, watch};
use tokio::task::JoinSet;
use tokio_tungstenite::tungstenite;

/// How much text collects before a partial goes out.
///
/// One relay event per token delta would put the round trip in the middle
/// of every word and tell the operator more about the relay's event rate
/// than about the answer. The result carries the whole text anyway, so
/// partials are a progress signal, not the payload.
const PARTIAL_BYTES: usize = 160;

/// The first wait after the relay goes away; each failure doubles it.
const RECONNECT_FLOOR: Duration = Duration::from_secs(1);

/// The longest wait between reconnect attempts.
const RECONNECT_CEILING: Duration = Duration::from_secs(60);

/// Overrides the probe period (`coder::relay::liveness::PROBE_EVERY`), in
/// milliseconds.
const PROBE_VAR: &str = "CODER_WORKER_PROBE_MS";

/// Overrides the renewal period (`coder::relay::liveness::RENEW_EVERY`),
/// in milliseconds.
const RENEW_VAR: &str = "CODER_WORKER_RENEW_MS";

/// The subscription that carries the worker's jobs.
const JOBS_SUBSCRIPTION: &str = "jobs";

/// How long past a delegation's stated minutes the worker keeps waiting.
///
/// The executor ends the run at the bound and reports that with a code;
/// this is the margin for it to do so. A run still silent after it is
/// refused `timed_out` by the worker itself, so the customer hears from
/// someone either way.
const DELEGATION_GRACE: Duration = Duration::from_secs(30);

/// How long a job with no stated minutes may run before it is refused
/// `timed_out`.
///
/// A model door answers in seconds and its own transport has bounds;
/// this is the bound behind those, so a door that hangs cannot hold a
/// slot for the rest of the worker's life and leave every later job
/// refused `busy`.
const UNDELEGATED_BOUND: Duration = Duration::from_secs(10 * 60);

/// How long the worker waits for a job before answering for it.
#[derive(Clone, Copy)]
struct Waits {
    /// Past a delegation's stated minutes.
    grace: Duration,
    /// For a job that states no minutes.
    undelegated: Duration,
}

const WAITS: Waits = Waits {
    grace: DELEGATION_GRACE,
    undelegated: UNDELEGATED_BOUND,
};

/// How far in the past a request's `created_at` may be before it is
/// refused `stale`.
///
/// Every NIP-CJ kind is ephemeral, so a live request is at most a clock
/// skew old. One older than this was published a while ago and is
/// arriving again: a relay replaying its log, or someone replaying a
/// captured event to spend this worker. It is signed by the customer,
/// so the customer is told, and nothing is generated.
const REQUEST_WINDOW: Duration = Duration::from_secs(10 * 60);

/// How many request IDs the worker remembers to drop a second delivery.
///
/// One request is answered once. The relay may deliver the same event
/// again after a reconnect or through a second subscription, and a
/// replayed event inside [`REQUEST_WINDOW`] carries the same ID as the
/// original; both are set aside. The memory is bounded so a long-running
/// worker does not grow with every job it ever saw.
const SEEN_REQUESTS: usize = 4096;

const USAGE: &str = "\
coder-worker — answer NIP-CJ job requests from a relay.

Usage: coder-worker [--once] [--decline <CODE>] [--check]
       coder-worker usage [--since YYYY-MM-DD] [--by key|surface|route|model|day|kind|outcome|payer]
                          [--json] [--dir DIR]

  --once             Answer one job, then exit.
  --decline <CODE>   Refuse every job with this NIP-CJ error code.
  --check            Read the configuration, print what the worker would
                     run as, and exit: 0 when it is safe to deploy, 78
                     when it is not. A worker that names no customers
                     (CODER_WORKER_ALLOW unset) and is not open on a relay
                     that is not loopback is not.
  -h, --help         Print this text.

usage prints totals from the usage log, grouped by --by (day unless
named), from --since on; --json prints the rows as JSON. Its directory is
--dir, else CODER_WORKER_USAGE_DIR, else usage/ in STATE_DIRECTORY, else
/var/lib/coder-worker-chat/usage.

CODER_WORKER_SECRET names the worker identity, 64 hex or an nsec.
CODER_RELAY picks the relay. CODER_WORKER_ALLOW, when set, lists the
customer pubkeys (npub or hex, comma-separated) this worker answers; any
other request is refused with code not_admitted. CODER_WORKER_OPEN=1
answers every other caller too, with no usage limit: conversation jobs
only, each request at most 96 KiB. CODER_WORKER_QUOTA
([day=N][,minute=N][,total=N][,bytes=N]) is an emergency brake, off unless
set; each count left out is unlimited, and setting it also opens the
worker. CODER_WORKER_QUOTA_FILE keeps its day's counts across a restart.
Every job is appended to the usage log (CODER_WORKER_USAGE_DIR, else
usage/ in systemd's STATE_DIRECTORY; off says off), and day files older
than CODER_WORKER_USAGE_DAYS (30 unless set; forever keeps them) are
deleted. CODER_WORKER_JOBS bounds
how many jobs run at once; the rest are refused busy. The first-response
judge answers a turn that asks for it (opener or judge in the request)
through the decision profile the agent resolves (TYPESAFE_API_KEY or
~/.openagents/jev.json); when TypeSafe cannot answer for its own reasons
(402, 429, 5xx, a timeout, no connection) it falls back to the Vercel AI
Gateway (AI_GATEWAY_API_KEY) and then OpenRouter (OPENROUTER_API_KEY), each
off without its key. CODER_WORKER_JUDGE=off turns it off. A turn that
names the chat router (\"router\": \"chat-router-v2\", or v1) gets every tier;
CODER_WORKER_ROUTER=shadow logs the router's decision but serves what the
first response alone would, and =off ignores the router. CODER_PERSONALIZE
(openrouter[:MODEL], gateway[:LANE], or off) picks the model that writes
the rest of a router stem. The door the worker
answers through comes from the environment exactly as it does for the
agent, except for the lane: CODER_WORKER_MODEL names the model or lane
this worker runs, and outranks CODER_MODEL. CODER_WORKER_PRIMARY names a
model every turn asks OpenRouter (OPENROUTER_API_KEY) for first, at low
reasoning, in front of that door, which takes any turn the primary has not
started in four seconds, or, while thinking, answered in eight; unset, it
is Space Bunny Alpha whenever
OPENROUTER_API_KEY is set, and off answers on that door alone. The worker proves its relay
subscription live with a probe every 30 seconds and renews it on an
overlapping connection every 45 minutes; CODER_WORKER_PROBE_MS and
CODER_WORKER_RENEW_MS change those periods. Under a systemd unit with
WatchdogSec, every answered probe pets the watchdog.";

/// What the command line asked for.
struct Options {
    once: bool,
    check: bool,
    decline: Option<String>,
    /// Customers this worker answers; `None` admits everyone.
    allow: Option<Vec<String>>,
    /// Whether callers off the allowlist are answered (`CODER_WORKER_OPEN`).
    open: bool,
    /// The emergency brake every caller off the allowlist is admitted
    /// under, when an operator set one. Setting it also opens the worker.
    quota: Option<Policy>,
}

impl Options {
    /// Whether this worker answers callers off its allowlist.
    fn opens(&self) -> bool {
        self.open || self.quota.is_some()
    }
}

/// The environment variable that lists admitted customers.
const ALLOW_VAR: &str = "CODER_WORKER_ALLOW";

/// The environment variable that opens the worker to every caller.
const OPEN_VAR: &str = "CODER_WORKER_OPEN";

/// The environment variable that sets the emergency brake.
const QUOTA_VAR: &str = "CODER_WORKER_QUOTA";

/// The environment variable naming the usage log's directory, or `off`.
const USAGE_DIR_VAR: &str = "CODER_WORKER_USAGE_DIR";

/// The environment variable naming how many days of usage files the
/// worker keeps (#11042): a whole number of days, or `forever`. Unset is
/// [`coder::relay::usage::DEFAULT_KEEP_DAYS`].
const USAGE_DAYS_VAR: &str = "CODER_WORKER_USAGE_DAYS";

/// Reads `CODER_WORKER_USAGE_DAYS`: `Some(days)`, or `None` for `forever`.
fn usage_days_from(value: Option<&str>) -> Result<Option<u32>, String> {
    match value.map(str::trim) {
        None | Some("") => Ok(Some(coder::relay::usage::DEFAULT_KEEP_DAYS)),
        Some("forever") => Ok(None),
        Some(text) => match text.parse::<u32>() {
            Ok(days) if days > 0 => Ok(Some(days)),
            _ => Err(format!(
                "{USAGE_DAYS_VAR} is a number of days or `forever`, not `{text}`"
            )),
        },
    }
}

/// Where `coder-worker usage` reads when nothing else names a directory.
const DEFAULT_USAGE_DIR: &str = "/var/lib/coder-worker-chat/usage";

/// Reads `CODER_WORKER_OPEN`: `1`, `true`, `on`, or `yes` opens.
fn open_from_env() -> Result<bool, String> {
    match env::var(OPEN_VAR).as_deref().map(str::trim) {
        Err(_) | Ok("" | "0" | "false" | "off" | "no") => Ok(false),
        Ok("1" | "true" | "on" | "yes") => Ok(true),
        Ok(other) => Err(format!("{OPEN_VAR} is 1 or 0, not `{other}`")),
    }
}

/// The usage log's directory: `CODER_WORKER_USAGE_DIR`, else `usage/` in
/// the systemd unit's state directory; `None` when it is `off` or neither
/// is set.
fn usage_dir_from_env() -> Option<PathBuf> {
    match env::var(USAGE_DIR_VAR).as_deref().map(str::trim) {
        Ok("off") => None,
        Ok(dir) if !dir.is_empty() => Some(PathBuf::from(dir)),
        _ => env::var("STATE_DIRECTORY")
            .ok()
            .and_then(|dirs| dirs.split(':').next().map(str::to_string))
            .filter(|dir| !dir.is_empty())
            .map(|dir| PathBuf::from(dir).join("usage")),
    }
}

/// `coder-worker usage ...`: totals from the usage log.
fn usage_command(arguments: impl Iterator<Item = String>) -> Result<(), String> {
    use coder::relay::usage;
    let mut since = None;
    let mut by = usage::By::Day;
    let mut as_json = false;
    let mut dir = None;
    let mut arguments = arguments;
    while let Some(argument) = arguments.next() {
        let mut value = |name: &str| {
            arguments
                .next()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match argument.as_str() {
            "--since" => since = Some(value("--since")?),
            "--by" => by = usage::By::parse(&value("--by")?)?,
            "--dir" => dir = Some(PathBuf::from(value("--dir")?)),
            "--json" => as_json = true,
            other => return Err(format!("unknown argument {other}")),
        }
    }
    let dir = dir
        .or_else(usage_dir_from_env)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_USAGE_DIR));
    let read = usage::read(&dir, since.as_deref())?;
    let rows = usage::stats(&read.records, by);
    if as_json {
        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "dir": dir.display().to_string(),
                "since": since,
                "by": by.word(),
                "rows": rows,
                "unreadable_lines": read.unreadable,
            }))
            .map_err(|error| error.to_string())?
        );
    } else {
        print!("{}", usage::table(&rows, by));
        if read.unreadable > 0 {
            eprintln!("{} line(s) did not read as records", read.unreadable);
        }
    }
    Ok(())
}

/// The environment variable naming the file that keeps the day's counts.
const QUOTA_FILE_VAR: &str = "CODER_WORKER_QUOTA_FILE";

/// Reads `CODER_WORKER_QUOTA`, or `None` when unset.
fn quota_from_env() -> Result<Option<Policy>, String> {
    match env::var(QUOTA_VAR) {
        Ok(text) => Policy::parse(&text)
            .map(Some)
            .map_err(|why| format!("{QUOTA_VAR}: {why}")),
        Err(_) => Ok(None),
    }
}

/// The environment variable that turns the first-response judge off.
const JUDGE_VAR: &str = "CODER_WORKER_JUDGE";

/// The environment variable that bounds how many jobs run at once.
const JOBS_VAR: &str = "CODER_WORKER_JOBS";

/// Jobs at once for a model door when `CODER_WORKER_JOBS` is unset.
const DEFAULT_JOBS: usize = 4;

/// Reads `CODER_WORKER_ALLOW` into hex pubkeys, or `None` when unset.
///
/// An entry that does not parse is an error, not an admitted nobody: a
/// typo in the one list that controls spend must stop the worker.
fn allowed_from_env() -> Result<Option<Vec<String>>, String> {
    let Ok(text) = env::var(ALLOW_VAR) else {
        return Ok(None);
    };
    allowed(&text).map(Some)
}

fn allowed(text: &str) -> Result<Vec<String>, String> {
    let mut keys = Vec::new();
    for entry in text
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
    {
        let key = parse_pubkey(entry)
            .ok_or_else(|| format!("{ALLOW_VAR}: {entry} is not an npub or 64 hex"))?;
        keys.push(hex(&key.serialize()));
    }
    if keys.is_empty() {
        return Err(format!("{ALLOW_VAR} is set but names no pubkey"));
    }
    Ok(keys)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn options() -> Result<Options, String> {
    let mut once = false;
    let mut check = false;
    let mut decline = None;
    let mut arguments = env::args().skip(1).peekable();
    if arguments.peek().map(String::as_str) == Some("usage") {
        arguments.next();
        if let Err(why) = usage_command(arguments) {
            eprintln!("coder-worker usage: {why}");
            std::process::exit(64);
        }
        std::process::exit(0);
    }
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--once" => once = true,
            "--check" => check = true,
            "--decline" => {
                decline = Some(
                    arguments
                        .next()
                        .ok_or_else(|| "--decline needs a code".to_string())?,
                );
            }
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument {other}")),
        }
    }
    Ok(Options {
        once,
        check,
        decline,
        allow: allowed_from_env()?,
        open: open_from_env()?,
        quota: quota_from_env()?,
    })
}

/// The exit status for a configuration that must not be deployed.
const EX_CONFIG: u8 = 78;

/// Whether `url` names a relay on this machine.
///
/// A worker with no allowlist on a loopback relay answers only what this
/// machine publishes; the same worker on any other relay answers whoever
/// finds its key. `deploy/README.md` says never to run one, and this is
/// what `--check` reads to refuse it.
fn is_loopback(url: &str) -> bool {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let authority = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host)| host);
    let host = if let Some(bracketed) = authority.strip_prefix('[') {
        bracketed
            .split_once(']')
            .map_or(bracketed, |(host, _)| host)
    } else {
        authority
            .rsplit_once(':')
            .map_or(authority, |(host, _)| host)
    };
    host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

/// Whether this configuration may be deployed. A worker off loopback
/// either names its customers or is opened on purpose: an accidental
/// open worker would hand delegation and execution to whoever finds its
/// key, where an opened one gives strangers conversation jobs only.
fn deployable(allow: Option<&[String]>, opens: bool, url: &str) -> Result<(), String> {
    if allow.is_none() && !opens && !is_loopback(url) {
        return Err(format!(
            "{ALLOW_VAR} is unset and {url} is not a loopback relay: a worker on a shared \
             relay that names no customers answers whoever finds its key with everything. \
             Set {ALLOW_VAR} to the customer pubkeys this worker serves, or {OPEN_VAR}=1 to \
             answer every caller with conversation jobs only."
        ));
    }
    Ok(())
}

#[tokio::main]
async fn main() -> ExitCode {
    let options = match options() {
        Ok(options) => options,
        Err(why) => {
            eprintln!("coder-worker: {why}\n\n{USAGE}");
            return ExitCode::from(64);
        }
    };
    match serve(&options).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(why) if options.check => {
            eprintln!("coder-worker: {why}");
            ExitCode::from(EX_CONFIG)
        }
        Err(why) => {
            eprintln!("coder-worker: {why}");
            ExitCode::FAILURE
        }
    }
}

/// Connects, subscribes, and answers jobs until `--once` is satisfied.
///
/// A relay that drops the socket, restarts, or closes the subscription is
/// a transport fault, not a reason to stop serving: the worker waits with
/// backoff, connects again, and subscribes again, and a job that was
/// running through the fault publishes on whichever socket is open when
/// it finishes. Only configuration stops the worker, before the first
/// connection.
///
/// Jobs run concurrently, up to the worker's bound; the socket stays with
/// this loop, and every job publishes through one channel it drains, so
/// two jobs' frames never interleave on the wire.
async fn serve(options: &Options) -> Result<(), String> {
    let secret = env::var("CODER_WORKER_SECRET")
        .map_err(|_| "CODER_WORKER_SECRET is not set".to_string())?;
    let identity = Arc::new(Identity::from_text(&secret, "CODER_WORKER_SECRET")?);
    let url = env::var("CODER_RELAY").unwrap_or_else(|_| DEFAULT_RELAY_URL.to_string());
    // The worker's lane is its own. The model a service pays for is not
    // automatically the model someone would pick at their own terminal, so
    // `CODER_WORKER_MODEL` outranks the `CODER_MODEL` the door would
    // otherwise read, and a lane named for a door that cannot run it is
    // refused rather than quietly dropped.
    // What every door asks its model provider about keeping and training
    // on the conversation (#11040): strict unless the operator lowers it,
    // and a value that names no level stops the worker.
    if let Ok(text) = env::var(coder::generate::PROVIDER_PRIVACY_VAR) {
        coder::generate::ProviderPrivacy::parse(&text)?;
    }
    // The inference gateway (#11064): with its service key here, every
    // model call goes through it and its router picks the model and
    // upstream, so no provider key is needed. `CODER_WORKER_INFERENCE=direct`
    // keeps the provider doors below.
    let gateway = coder::generate::inference_door_from_env()?;
    let door = match gateway {
        Some(gateway) => {
            eprintln!(
                "inference gateway {} ({}; {}=direct for the provider doors)",
                gateway.url,
                gateway.model,
                coder::generate::INFERENCE_MODE_VAR
            );
            Arc::new(Door::Live(gateway))
        }
        None => {
            let mut door = Door::from_env()?;
            if let Some(model) = model_from_env(WORKER_MODEL_VAR) {
                door = door
                    .serving(&model)
                    .map_err(|why| format!("{WORKER_MODEL_VAR}: {why}"))?;
            }
            // The primary goes in front of that door (#10109): every turn
            // asks OpenRouter's primary first, and one it does not start
            // answering goes to the door above. Unset, the primary is Space
            // Bunny Alpha whenever OpenRouter's key is here.
            Arc::new(ordered(
                door,
                env::var(WORKER_PRIMARY_VAR).ok().as_deref(),
                env::var(OPENROUTER_KEY_VAR).ok().as_deref(),
            )?)
        }
    };
    let jobs = jobs_bound(&door)?;
    // The judge that answers first: one System One call per admitted
    // conversation turn, run beside the model call (see `coder::first`).
    // The same decision profile the agent resolves; none configured means
    // no judgment, and a configuration that does not resolve stops the
    // worker rather than quietly running without one.
    // With their keys in the environment, a judgment asks the Vercel AI
    // Gateway first, then OpenRouter, then TypeSafe direct last
    // (`jev::doors`); the first door asked keeps three fifths of the
    // first budget, so a hung door leaves the others the rest.
    let (judge, fallbacks) = match env::var(JUDGE_VAR).as_deref() {
        Ok("off") => (None, Vec::new()),
        Ok("") | Err(_) => {
            match coder::decision::from_env_with_fallbacks(Some(first::BUDGET * 3 / 5))? {
                Some((judge, fallbacks)) => (Some(Arc::new(judge)), fallbacks),
                None => (None, Vec::new()),
            }
        }
        Ok(other) => return Err(format!("{JUDGE_VAR} is `off` or unset, not `{other}`")),
    };

    eprintln!("worker  {}", identity.pubkey());
    eprintln!("relay   {url}");
    // The lane is named beside the model, and the model is always there:
    // a run whose evidence cannot say which model answered cannot be
    // compared against one that used another.
    match &*door {
        Door::Fallback(ordered) => eprintln!(
            "door    {} ({} at {} with reasoning {}, then {} at {} for any turn it has not \
             started in {} ms or, thinking, answered by {} ms)",
            door.name(),
            ordered.primary.model,
            ordered.primary.url,
            coder::generate::PRIMARY_EFFORT,
            ordered.fallback.model,
            ordered.fallback.url,
            coder::generate::PRIMARY_FIRST_WORD.as_millis(),
            coder::generate::PRIMARY_THINKING.as_millis()
        ),
        _ => match Lane::read(door.model()) {
            Some(lane) => eprintln!(
                "door    {} ({}, lane {})",
                door.name(),
                door.model(),
                lane.name()
            ),
            None => eprintln!("door    {} ({})", door.name(), door.model()),
        },
    }
    eprintln!(
        "privacy {} (asked of the chat model's providers; {})",
        coder::generate::ProviderPrivacy::from_env().word(),
        coder::generate::PROVIDER_PRIVACY_VAR
    );
    match &judge {
        Some(judge) => eprintln!(
            "judge   {} ({}): first response and suggestions",
            judge
                .doors()
                .unwrap_or_else(|| judge.base_url().to_string()),
            judge.default_model()
        ),
        None => eprintln!("judge   none: no first response before the model's"),
    }
    for fallback in &fallbacks {
        eprintln!("judge   {fallback}");
    }
    // The chat router's bank ships inside the binary; a bank that breaks
    // its own rules stops the worker here, and `--check` with it.
    let bank = Bank::builtin();
    let problems = router::bank::lint(bank, None);
    if !problems.is_empty() {
        return Err(format!(
            "the answer bank {} breaks its rules:\n{}",
            bank.id(),
            problems.join("\n")
        ));
    }
    // Each seam comes from its own module's configuration; one that is
    // not configured stays the no-op, and the router falls back past it.
    let mut seams = Seams {
        personalize: router::personalize::seam_from_env()?,
        codebase: coder::codebase::seam_from_env(judge.clone())?,
        cli: cli_seam(judge.as_ref(), &door)?,
        ..Seams::default()
    };
    // The product knowledge base answers `product.kb` turns when its corpus,
    // an embeddings key, and the judge are all here; otherwise the route is
    // answered by the model alone, as with no knowledge base.
    match judge.clone() {
        Some(judge) => match coder::product_kb::ProductKnowledge::from_env(judge) {
            Ok(kb) => {
                eprintln!(
                    "product kb {} ({} entries), embeddings through {}",
                    kb.corpus().tag(),
                    kb.corpus().base.entries.len(),
                    kb.recipient()
                );
                let kb = Arc::new(kb);
                let warming = kb.clone();
                tokio::spawn(async move {
                    if let Err(why) = warming.warm().await {
                        eprintln!("product kb not warmed: {why}");
                    }
                });
                seams.product = kb;
            }
            Err(why) => eprintln!("product kb off: {why}"),
        },
        None => eprintln!("product kb off: no judge"),
    }
    // The Gym's records answer the Gym and eval routes when the product
    // corpus (its tool catalog and Gym notes), an embeddings key, and the
    // judge are here. Published results are read through the ext-eval
    // profile parser, and the starter test sets from the hosted runner's
    // releases and bucket (`CODER_EVAL_BLOBS`), so `eval.run` offers a test
    // before anyone has published a result; adoptions are not read yet.
    match judge.clone() {
        Some(judge) => match coder::gym_kb::GymKnowledge::from_env(judge) {
            Ok(gym) => {
                let records = gym.records();
                eprintln!(
                    "gym records: {} tools, {} builds, {} notes; published results read from \
                     the relay every {} minutes",
                    records.tools.len(),
                    records.releases.len(),
                    records.notes.len(),
                    coder::gym_kb::REFRESH.as_secs() / 60
                );
                let gym = Arc::new(gym);
                let warming = gym.clone();
                let (relay, reader) = (url.clone(), identity.clone());
                tokio::spawn(async move {
                    if let Err(why) = warming.warm().await {
                        eprintln!("gym records not warmed: {why}");
                    }
                    // Published results: read, verified, and counted, then
                    // read again; a failed read keeps the last one.
                    loop {
                        match warming.refresh(&relay, &reader).await {
                            Ok(admitted) => eprintln!(
                                "gym records: {} verified results, {} test sets, {} refused",
                                admitted.results.len(),
                                admitted.suites.len(),
                                admitted.refused.len()
                            ),
                            Err(why) => eprintln!("gym records not read: {why}"),
                        }
                        tokio::time::sleep(coder::gym_kb::REFRESH).await;
                    }
                });
                seams.gym = gym;
            }
            Err(why) => eprintln!("gym records off: {why}"),
        },
        None => eprintln!("gym records off: no judge"),
    }
    // The authoring interview's chat driver (#9937): the worker's door and
    // judge; without both, `eval.author` answers with the bank's
    // `eval.author.soon`.
    seams.author = coder::eval_author::seam(
        &door,
        judge
            .clone()
            .map(|j| j as Arc<dyn coder::product_kb::Judge>),
    );
    // Through the inference gateway the news lane asks for the `fast`
    // class unless a model is named: the gateway serves no model the
    // direct lane's default names.
    let news = match env::var(GYM_NEWS_MODEL_VAR) {
        Err(_) if door.gateway().is_some_and(ResponsesDoor::is_gateway) => {
            Some("openagents/fast".to_string())
        }
        _ => gym_news_model_from_env(),
    };
    let jev_fallbacks: Vec<&str> = fallbacks
        .iter()
        .filter(|fallback| fallback.on)
        .map(|fallback| fallback.name)
        .collect();
    let routing = Arc::new(
        RouterConfig::with_news_and_jev(
            router_from_env()?,
            seams,
            &door,
            news.as_deref(),
            &jev_fallbacks,
        )
        .calibrated(router::calibration::Calibration::from_env(&bank.id())?),
    );
    eprintln!(
        "router  {} ({:?}), bank {} with {} answers; seams {:?}",
        router::set_id(),
        routing.setting,
        bank.id(),
        bank.answers.len(),
        routing.seams
    );
    match &routing.calibration {
        Some(map) if map.fitted_with(&bank.id()) => {
            eprintln!("calibration {} on, fitted {}", map.id(), map.created);
        }
        Some(map) => eprintln!(
            "calibration {} on, fitted {} with {} (this bank is {})",
            map.id(),
            map.created,
            map.bank,
            bank.id()
        ),
        None => eprintln!("calibration off: raw probabilities"),
    }
    match routing.news.as_deref() {
        Some(Door::Fallback(news)) => eprintln!(
            "gym news {} first, then {} with its reasoning off",
            news.primary.model, news.fallback.model
        ),
        Some(news) => eprintln!("gym news {} with its reasoning off", news.model()),
        None => eprintln!("gym news on the chat door"),
    }
    eprintln!("jobs    {jobs} at once; more are refused as busy");
    if let Some(code) = &options.decline {
        eprintln!("declining every job with {code}");
    }
    let count =
        |count: Option<u32>| count.map_or_else(|| "unlimited".to_string(), |n| n.to_string());
    match (&options.allow, options.opens(), &options.quota) {
        (Some(keys), false, _) => eprintln!("admits  {} customer(s)", keys.len()),
        (allow, true, None) => eprintln!(
            "admits  every caller with no usage limit: conversation jobs, {} bytes a request; \
             {} customer key(s) get everything",
            Policy::UNLIMITED.max_request_bytes,
            allow.as_ref().map_or(0, Vec::len)
        ),
        (allow, true, Some(quota)) => eprintln!(
            "admits  every caller under the emergency brake {QUOTA_VAR}: {} a minute and {} a \
             day per key, {} a day in all, {} bytes a request; {} customer key(s) unbraked",
            count(quota.per_key_minute),
            count(quota.per_key_day),
            count(quota.total_day),
            quota.max_request_bytes,
            allow.as_ref().map_or(0, Vec::len)
        ),
        (None, false, _) => eprintln!("admits  every customer ({ALLOW_VAR} unset)"),
    }
    let ledger = if options.opens() {
        let policy = options.quota.unwrap_or(Policy::UNLIMITED);
        let path = env::var(QUOTA_FILE_VAR).ok().map(PathBuf::from);
        let ledger = Ledger::open(policy, path, unix_now())?;
        if policy.counts() {
            eprintln!("quota   {} job(s) admitted today", ledger.total());
        }
        Some(Arc::new(std::sync::Mutex::new(ledger)))
    } else {
        None
    };
    let keep_days = usage_days_from(env::var(USAGE_DAYS_VAR).ok().as_deref())?;
    let usage = usage_dir_from_env().map(|dir| {
        let log = coder::relay::usage::Log::new(&dir).keeping(keep_days);
        match keep_days {
            Some(days) => eprintln!(
                "usage   one line per job in {}, kept {days} day(s)",
                dir.display()
            ),
            None => eprintln!(
                "usage   one line per job in {}, kept forever",
                dir.display()
            ),
        }
        match log.prune(unix_now()) {
            Ok(0) => {}
            Ok(deleted) => eprintln!("usage   deleted {deleted} day file(s) past the window"),
            Err(why) => eprintln!("usage   could not delete old usage files: {why}"),
        }
        Arc::new(log)
    });
    if usage.is_none() {
        eprintln!("usage   log off ({USAGE_DIR_VAR} is off, or unset outside systemd)");
    }
    let liveness = Liveness::from_env(PROBE_VAR, RENEW_VAR)?;
    eprintln!(
        "liveness a probe every {} s; the subscription is renewed every {} s",
        liveness.probe.as_secs_f64(),
        liveness.renew.as_secs_f64()
    );
    if options.check {
        deployable(options.allow.as_deref(), options.opens(), &url)?;
        eprintln!("the configuration is safe to deploy");
        return Ok(());
    }

    // Keep the door's, the judge's, and the codebase embedder's HTTPS
    // connections open, so a turn does not start with a handshake. The
    // door and judge calls are unbilled reads; the embedder's is one fixed
    // word (a fraction of a millionth of a dollar), and carries no message.
    tokio::spawn(warm(
        door.clone(),
        judge.clone(),
        routing.seams.codebase.clone(),
    ));

    let (outgoing, frames) = mpsc::unbounded_channel::<Value>();
    let mut worker = Worker {
        options,
        url: url.clone(),
        liveness,
        identity,
        door,
        judge,
        running: Arc::new(Semaphore::new(jobs)),
        outgoing,
        frames,
        tasks: JoinSet::new(),
        answered: 0,
        seen: VecDeque::with_capacity(SEEN_REQUESTS),
        ledger,
        usage,
        routing,
    };
    let mut backoff = RECONNECT_FLOOR;
    loop {
        let session = async {
            let socket = subscribe(&url, &worker.identity).await?;
            eprintln!("waiting for jobs");
            Ok::<Socket, String>(socket)
        };
        let why = match session.await {
            Ok(socket) => match worker.session(socket).await {
                Ok(()) => return Ok(()),
                Err(Fault::Subscribed(why)) => {
                    backoff = RECONNECT_FLOOR;
                    why
                }
                Err(Fault::Early(why)) => why,
            },
            Err(why) => why,
        };
        eprintln!("relay: {why}; reconnecting in {} s", backoff.as_secs());
        // Jobs keep running while the socket is down; their frames wait in
        // the channel for the next one. Nothing is read from the relay
        // until then, and nothing can be: the kinds are ephemeral, so a
        // request published now is lost whatever the worker does.
        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(RECONNECT_CEILING);
    }
}

/// How often the worker touches its door and judge to keep their pooled
/// connections open: under the HTTP client's 90-second idle close.
const WARM_EVERY: Duration = Duration::from_secs(45);

/// Warm the door and the judge now and every [`WARM_EVERY`].
async fn warm(
    door: Arc<Door>,
    judge: Option<Arc<jev::Client>>,
    codebase: Arc<dyn coder::router::seams::CodebaseKb>,
) {
    loop {
        let judging = async {
            if let Some(judge) = &judge {
                let _ = judge.models().list(jev::ListOptions::default()).await;
            }
        };
        tokio::join!(door.warm(), judging, codebase.warm());
        tokio::time::sleep(WARM_EVERY).await;
    }
}

/// Why one connection to the relay ended.
enum Fault {
    /// The relay confirmed the subscription before the fault, so the relay
    /// was healthy and the next attempt starts from the shortest wait.
    Subscribed(String),
    /// The socket or the subscription failed before `EOSE`, which reads as
    /// a relay still starting or still broken; the wait keeps growing.
    Early(String),
}

/// The worker's state across relay connections.
struct Worker<'a> {
    options: &'a Options,
    /// The relay, for a successor connection.
    url: String,
    liveness: Liveness,
    identity: Arc<Identity>,
    door: Arc<Door>,
    /// The System One client for the first response, when configured.
    judge: Option<Arc<jev::Client>>,
    running: Arc<Semaphore>,
    outgoing: mpsc::UnboundedSender<Value>,
    frames: mpsc::UnboundedReceiver<Value>,
    tasks: JoinSet<Result<(), String>>,
    answered: usize,
    /// IDs of the last [`SEEN_REQUESTS`] requests, oldest first.
    seen: VecDeque<String>,
    /// The open lane's admission (size bound, and the emergency brake
    /// when one is set), when the worker is open.
    ledger: Option<Arc<std::sync::Mutex<Ledger>>>,
    /// The usage log, when it is on.
    usage: Option<Arc<coder::relay::usage::Log>>,
    /// The chat router's configuration.
    routing: Arc<RouterConfig>,
}

impl Worker<'_> {
    /// Serves one connection until it fails or `--once` is satisfied.
    ///
    /// Besides jobs, the loop proves the connection live every
    /// [`Liveness::probe`] and renews it every [`Liveness::renew`]: a
    /// successor connection subscribes first, then the old one is closed
    /// and read for [`DRAIN`] longer, so the two subscriptions overlap and
    /// a job is never published to neither. A job delivered on both is
    /// answered once.
    async fn session(&mut self, mut socket: Socket) -> Result<(), Fault> {
        let mut subscribed = false;
        let fault = |subscribed: bool, why: String| {
            if subscribed {
                Fault::Subscribed(why)
            } else {
                Fault::Early(why)
            }
        };
        let probe_every = self.liveness.probe;
        let mut probes =
            tokio::time::interval_at(tokio::time::Instant::now() + probe_every, probe_every);
        probes.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut probe_count: u64 = 0;
        // The probe the relay has not answered yet.
        let mut outstanding: Option<String> = None;
        let mut renew_at = tokio::time::Instant::now() + self.liveness.renew;
        let mut renewing: Option<Renewal> = None;
        // The replaced connection and when reading it stops.
        let mut draining: Option<(Socket, tokio::time::Instant)> = None;
        loop {
            tokio::select! {
                frame = self.frames.recv() => {
                    let Some(frame) = frame else { continue };
                    send(&mut socket, frame)
                        .await
                        .map_err(|error| fault(subscribed, error.to_string()))?;
                }
                Some(ended) = self.tasks.join_next(), if !self.tasks.is_empty() => {
                    match ended {
                        Ok(Ok(())) => {}
                        Ok(Err(why)) => eprintln!("job: {why}"),
                        Err(join) => eprintln!("job: {join}"),
                    }
                    self.answered += 1;
                    if self.options.once && self.answered >= 1 {
                        // The job's frames were queued before its task
                        // ended; drain them before the socket goes.
                        while let Ok(frame) = self.frames.try_recv() {
                            send(&mut socket, frame)
                                .await
                                .map_err(|error| fault(subscribed, error.to_string()))?;
                        }
                        return Ok(());
                    }
                }
                _ = probes.tick() => {
                    if outstanding.is_some() {
                        return Err(fault(subscribed, format!(
                            "the relay stopped answering: a liveness probe had no reply in {} s",
                            probe_every.as_secs_f64()
                        )));
                    }
                    probe_count += 1;
                    let id = format!("{PROBE_PREFIX}{probe_count}");
                    send(&mut socket, probe_request(&id, self.identity.pubkey()))
                        .await
                        .map_err(|error| fault(subscribed, error.to_string()))?;
                    outstanding = Some(id);
                }
                () = tokio::time::sleep_until(renew_at), if subscribed && renewing.is_none() => {
                    renewing = Some(Box::pin(successor(self.url.clone(), self.identity.clone())));
                }
                made = poll_some(&mut renewing) => {
                    renewing = None;
                    match made {
                        Ok(Successor { socket: next, early }) => {
                            let mut old = std::mem::replace(&mut socket, next);
                            // The old subscription stops after the new one
                            // is live; whatever it delivered meanwhile is
                            // still read for a moment.
                            let _ = send(&mut old, json!(["CLOSE", JOBS_SUBSCRIPTION])).await;
                            draining = Some((old, tokio::time::Instant::now() + DRAIN));
                            outstanding = None;
                            probes.reset();
                            renew_at = tokio::time::Instant::now() + self.liveness.renew;
                            eprintln!("renewed the jobs subscription on a new connection");
                            notify_watchdog();
                            for value in early {
                                self.admit(&value);
                            }
                        }
                        Err(why) => {
                            // The current connection still proves itself
                            // with probes; try again after the next one.
                            eprintln!("relay: renewing the subscription failed: {why}; keeping the current connection");
                            renew_at = tokio::time::Instant::now() + probe_every;
                        }
                    }
                }
                frame = next_draining(&mut draining) => {
                    match frame {
                        Some(Ok(tungstenite::Message::Text(text))) => {
                            let Ok(value) = serde_json::from_str::<Value>(&text) else { continue };
                            if value[1].as_str() != Some(JOBS_SUBSCRIPTION) {
                                continue;
                            }
                            match value[0].as_str() {
                                Some("EVENT") => self.admit(&value),
                                Some("CLOSED") => draining = None,
                                _ => {}
                            }
                        }
                        Some(Ok(_)) => {}
                        Some(Err(_)) | None => draining = None,
                    }
                }
                frame = socket.next() => {
                    let Some(frame) = frame else {
                        return Err(fault(subscribed, "the relay closed the socket".to_string()));
                    };
                    let frame = frame.map_err(|error| fault(subscribed, format!("socket: {error}")))?;
                    let tungstenite::Message::Text(text) = frame else {
                        continue;
                    };
                    let Ok(value) = serde_json::from_str::<Value>(&text) else {
                        continue;
                    };
                    // The answer to a liveness probe: the relay is there
                    // and serving this connection.
                    if outstanding.as_deref().is_some_and(|id| value[1].as_str() == Some(id)) {
                        match value[0].as_str() {
                            Some("EOSE") => {
                                let id = outstanding.take().unwrap_or_default();
                                send(&mut socket, json!(["CLOSE", id]))
                                    .await
                                    .map_err(|error| fault(subscribed, error.to_string()))?;
                                notify_watchdog();
                            }
                            // A refused probe still came from the relay.
                            Some("CLOSED") => {
                                outstanding = None;
                                notify_watchdog();
                            }
                            _ => {}
                        }
                        continue;
                    }
                    if value[1].as_str() != Some(JOBS_SUBSCRIPTION) {
                        continue;
                    }
                    // The relay buffers live events until the history query
                    // behind a REQ finishes, and a CLOSED subscription
                    // receives nothing at all. Both are the worker's
                    // business to act on, because from the terminal each
                    // looks like a worker that is not there.
                    match value[0].as_str() {
                        Some("EOSE") => {
                            eprintln!("subscribed; jobs arrive live from here");
                            subscribed = true;
                            notify_watchdog();
                        }
                        Some("CLOSED") => {
                            return Err(fault(subscribed, format!(
                                "the relay closed the jobs subscription: {}",
                                value[2].as_str().unwrap_or_default()
                            )));
                        }
                        Some("EVENT") => self.admit(&value),
                        _ => {}
                    }
                }
            }
        }
    }

    /// Checks one `EVENT` frame on the jobs subscription and starts its
    /// job, from whichever connection delivered it.
    fn admit(&mut self, value: &Value) {
        // What the relay delivered is checked before it is
        // trusted for anything, its label included. Something
        // that is not a signed request to this worker is set
        // aside with one line saying why; there is no one to
        // answer, because nothing proved who sent it.
        let request = match serde_json::from_value::<Event>(value[2].clone()) {
            Ok(request) => request,
            Err(error) => {
                eprintln!("ignored an event that does not parse: {error}");
                return;
            }
        };
        if request.kind == nostr::execution::REQUEST_KIND {
            // A metered caller gets conversation jobs only:
            // execution is for the operator's own keys.
            if !execution_admitted(self.options, &request.pubkey) {
                eprintln!(
                    "ignored execution {}: not from an allowlisted key",
                    &request.id[..request.id.len().min(16)]
                );
                return;
            }
            // Execution admission is the shared store. A closed
            // socket does not cancel the claim, and a later
            // relay OK is not acceptance: `Store::answer` is.
            if let Err(why) = answer_execution(&self.identity, &request, &self.outgoing) {
                eprintln!(
                    "ignored execution {}: {why}",
                    &request.id[..request.id.len().min(16)]
                );
            }
            return;
        }
        if let Err(why) = addressed(&request, self.identity.pubkey()) {
            eprintln!("ignored {}: {why}", &request.id[..request.id.len().min(16)]);
            return;
        }
        if self.seen.contains(&request.id) {
            eprintln!("ignored {}: already delivered", &request.id[..16]);
            return;
        }
        if self.seen.len() == SEEN_REQUESTS {
            self.seen.pop_front();
        }
        self.seen.push_back(request.id.clone());
        // Admission is decided here, before anything is spawned:
        // a job over the bound is refused `busy` at once rather
        // than queued behind work the terminal cannot see.
        let permit = self.running.clone().try_acquire_owned().ok();
        let job = Job {
            identity: self.identity.clone(),
            answered: Default::default(),
            upstream: Default::default(),
            door: self.door.clone(),
            judge: self.judge.clone(),
            decline: self.options.decline.clone(),
            allow: self.options.allow.clone(),
            ledger: self.ledger.clone(),
            usage: self.usage.clone(),
            publish: self.outgoing.clone(),
            permit,
            waits: WAITS,
            routing: self.routing.clone(),
            payer: None,
            payer_last: None,
            payer_refusal: None,
        };
        self.tasks.spawn(async move { job.answer(&request).await });
    }
}

/// The chat door on a caller's own keys (BYOK): our current primary and
/// fallback models, on their OpenRouter key and then their Vercel AI
/// Gateway key ([`model_access::Access::chat`]). A gateway-only key gets
/// the fallback model, since the gateway does not serve every primary.
/// Returns the door, who pays when its first door answers, and the
/// provider of its last door.
///
/// # Errors
///
/// No key of theirs serves the chat, or this worker's door is not a model
/// door (an executor), in one plain line.
fn their_door(
    ours: &Door,
    access: &model_access::Access,
) -> Result<(Door, model_access::Payer, model_access::Provider), String> {
    let (primary, fallback) = match ours {
        Door::Fallback(ordered) => (
            ordered.primary.model.clone(),
            ordered.fallback.model.clone(),
        ),
        Door::Live(live) if live.is_gateway() => {
            let model = model_named(coder::generate::DEFAULT_LANE.name()).to_string();
            (model.clone(), model)
        }
        Door::Live(live) => (live.model.clone(), live.model.clone()),
        _ => return Err("this worker can't answer on your keys".to_string()),
    };
    let doors = match access.chat(model_access::Use::Chat {
        primary: &primary,
        fallback: &fallback,
    }) {
        Ok(model_access::Doors::Theirs(doors)) => doors,
        Ok(model_access::Doors::Ours) => return Err("no payer keys".to_string()),
        Err(no_door) => return Err(no_door.to_string()),
    };
    let to_door = |door: &model_access::ChatDoor| {
        let responses =
            ResponsesDoor::new(door.responses_base(), door.model.clone(), door.key.expose());
        if door.provider == model_access::Provider::OpenRouter
            && door.model == primary
            && primary != fallback
        {
            responses.with_options(serde_json::Map::from_iter([(
                "reasoning".to_string(),
                json!({ "effort": coder::generate::PRIMARY_EFFORT }),
            )]))
        } else {
            responses
        }
    };
    let first = doors.first().ok_or("no door")?;
    let last = doors.last().ok_or("no door")?;
    let door = if doors.len() > 1 {
        Door::Fallback(Box::new(FallbackDoor::new(to_door(first), to_door(last))))
    } else {
        Door::Live(to_door(first))
    };
    Ok((door, first.payer(), last.provider))
}

/// The worker's jobs subscription request.
fn jobs_request(worker: &str) -> Value {
    json!(["REQ", JOBS_SUBSCRIPTION, {
        "kinds": [REQUEST_KIND, nostr::execution::REQUEST_KIND],
        "#p": [worker],
    }])
}

/// A liveness probe: a `REQ` the relay answers with `EOSE` at once, since
/// it asks for no stored events (the jobs kinds are ephemeral, and the
/// limit is zero). Its events, if any arrived before the `CLOSE`, are set
/// aside by subscription ID; jobs are taken only from the jobs
/// subscription.
fn probe_request(id: &str, worker: &str) -> Value {
    liveness::probe_request(id, json!({ "kinds": [REQUEST_KIND], "#p": [worker] }))
}

/// Connects, authenticates, and sends the jobs subscription.
async fn subscribe(url: &str, identity: &Identity) -> Result<Socket, String> {
    liveness::subscribe(url, identity, jobs_request(identity.pubkey())).await
}

/// Opens the connection that replaces the current one, and returns it once
/// its jobs subscription is live.
async fn successor(url: String, identity: Arc<Identity>) -> Result<Successor, String> {
    let request = jobs_request(identity.pubkey());
    liveness::successor(url, identity, JOBS_SUBSCRIPTION, request).await
}

/// Whether `request` is a signed job request naming this worker.
///
/// The relay's filter asked for exactly this, and the relay is transport,
/// not authority: a relay that is wrong or lying delivers something else,
/// and the worker checks rather than assumes.
/// Admit one execution request and publish the store's events.
///
/// Program dispatch stays [`coder::execution::Store::dispatch_program`],
/// which the terminal and the headless caller share. This loop does not
/// treat the relay's `OK`, or the socket closing afterwards, as acceptance
/// or cancellation.
fn answer_execution(
    identity: &Identity,
    request: &Event,
    publish: &mpsc::UnboundedSender<Value>,
) -> Result<(), String> {
    let dir = execution_dir()?;
    let mut store = coder::execution::Store::open(
        &dir,
        identity.pubkey(),
        4,
        unix_now().saturating_add(7 * 24 * 60 * 60),
    )?;
    let bytes = BTreeMap::new();
    let mailbox = execution_mailbox(&request.id);
    let intake = coder::execution::Intake {
        event: request,
        secret: identity.secret(),
        signer: identity.signer(),
        now: unix_now(),
        window: nostr::execution::Window::DEFAULT,
        bytes: &bytes,
        quoted_spend: None,
        remaining: None,
        nonce: secp256k1::rand::random(),
        mailbox: &mailbox,
    };
    for event in store.answer(&intake)? {
        publish
            .send(json!(["EVENT", event]))
            .map_err(|_| "the serving loop is gone".to_string())?;
    }
    Ok(())
}

/// Whether this worker takes execution requests from `caller`: an open
/// worker's callers off its allowlist get conversation jobs only.
fn execution_admitted(options: &Options, caller: &str) -> bool {
    !options.opens()
        || options
            .allow
            .as_ref()
            .is_some_and(|keys| keys.iter().any(|key| key == caller))
}

fn execution_dir() -> Result<PathBuf, String> {
    let home = env::var("HOME").map_err(|_| "HOME is unset".to_string())?;
    Ok(PathBuf::from(home).join(".openagents").join("execution"))
}

fn execution_mailbox(event_id: &str) -> String {
    let digest = nostr::contracts::digest_bytes(format!("mailbox:{event_id}").as_bytes());
    digest.trim_start_matches("sha256:").to_string()
}

fn addressed(request: &Event, worker: &str) -> Result<(), String> {
    if request.kind != REQUEST_KIND {
        return Err(format!("kind {} is not a job request", request.kind));
    }
    request
        .validate_crypto()
        .map_err(|error| format!("the signature does not verify: {error}"))?;
    if !request.tag_values("p").any(|key| key == worker) {
        return Err("the request is not addressed to this worker".to_string());
    }
    Ok(())
}

/// `door` with the primary [`WORKER_PRIMARY_VAR`] asks for (`asked`) in
/// front of it, reached with OpenRouter's `key` (#10109): the door itself
/// when no primary is asked for or there is no key to reach one.
///
/// # Errors
///
/// When a primary is named with no key to reach it, or named in front of
/// a door that is not a gateway door: a relay or an executor picks its own
/// model, and a fallback in front of it would be a second way to answer.
/// Unnamed, the default primary goes in front of a gateway door only.
fn ordered(door: Door, asked: Option<&str>, key: Option<&str>) -> Result<Door, String> {
    let Some((model, key)) = coder::generate::worker_primary(asked, key)? else {
        return Ok(door);
    };
    match door {
        Door::Live(fallback) => Ok(Door::Fallback(Box::new(FallbackDoor::openrouter(
            &model, &key, fallback,
        )))),
        // Unasked, a door that picks its own model (a relay, an executor)
        // or the stub keeps answering alone.
        door if asked.is_none_or(|asked| asked.trim().is_empty()) => Ok(door),
        other => Err(format!(
            "{WORKER_PRIMARY_VAR} puts {model} in front of the {} door, which picks its own \
             model; set {WORKER_PRIMARY_VAR}=off",
            other.name()
        )),
    }
}

/// How many jobs run at once.
///
/// `CODER_WORKER_JOBS` states it. Unset, an executor door runs as many as
/// its manifest claims are safe at once, and a model door runs
/// [`DEFAULT_JOBS`].
fn jobs_bound(door: &Door) -> Result<usize, String> {
    match env::var(JOBS_VAR) {
        Ok(text) if !text.is_empty() => text
            .parse::<usize>()
            .ok()
            .filter(|jobs| *jobs > 0)
            .ok_or_else(|| format!("{JOBS_VAR} must be a positive whole number")),
        _ => Ok(match door {
            Door::Executor(executor) => executor.concurrent_max().max(1),
            _ => DEFAULT_JOBS,
        }),
    }
}

/// One job request and everything answering it needs.
struct Job {
    identity: Arc<Identity>,
    door: Arc<Door>,
    /// The model the door named as having written this job's reply, when
    /// it named one ([`start_model`]).
    answered: std::sync::Mutex<Option<String>>,
    /// The upstream the inference gateway named for this job's reply, when
    /// the door is the gateway ([`Meta::Upstream`]).
    upstream: std::sync::Mutex<Option<String>>,
    /// The System One client for the first response and rankings.
    judge: Option<Arc<jev::Client>>,
    decline: Option<String>,
    allow: Option<Vec<String>>,
    /// The admission a caller off the allowlist goes through, when the
    /// worker is open: the size bound, and the emergency brake when set.
    ledger: Option<Arc<std::sync::Mutex<Ledger>>>,
    /// The usage log every job is recorded in, when it is on.
    usage: Option<Arc<coder::relay::usage::Log>>,
    /// Frames for the serving loop to write to the socket, in order.
    publish: mpsc::UnboundedSender<Value>,
    /// A slot under the concurrency bound, or `None` when every slot was
    /// taken at admission and the job is refused `busy`.
    permit: Option<OwnedSemaphorePermit>,
    /// How long to wait for the job before refusing it `timed_out`.
    waits: Waits,
    /// Everything a routed turn needs beyond the judge.
    routing: Arc<RouterConfig>,
    /// Who pays for this job's model calls when the caller sent its own
    /// provider keys (`payer.keys`, BYOK): `None` is ours.
    payer: Option<model_access::Payer>,
    /// The provider of the caller's last door, whose failure is the one
    /// the caller hears.
    payer_last: Option<model_access::Provider>,
    /// Why the caller's payer envelope could not be used; the job is then
    /// refused, never answered on our keys.
    payer_refusal: Option<String>,
}

/// The chat router's configuration on this worker.
struct RouterConfig {
    /// How a request that asks for the router is served.
    setting: RouterSetting,
    /// The integration seams (no-ops until their modules are wired).
    seams: Seams,
    /// The bank's slot values, from this worker's configuration.
    facts: router::Facts,
    /// The same values naming the fallback as our model, for the turns
    /// after the primary's last one failed before its first words
    /// ([`Job::facts`]); `None` for a door with no fallback.
    fell_back: Option<router::Facts>,
    /// The door a grounded `gym.news` reply runs on: the chat door's
    /// gateway and key with [`router::gym::NEWS_MODEL`] and its reasoning
    /// off (#9950), or `None` for the chat door itself.
    news: Option<Arc<Door>>,
    /// The calibration map each reading's probabilities go through before
    /// the policy decides, when `CODER_WORKER_ROUTER_CALIBRATION=on`
    /// (#9959); `None` serves the raw probabilities.
    calibration: Option<router::calibration::Calibration>,
}

impl RouterConfig {
    /// The configuration with `calibration` applied to every reading.
    fn calibrated(mut self, calibration: Option<router::calibration::Calibration>) -> Self {
        self.calibration = calibration;
        self
    }

    #[cfg(test)]
    fn new(setting: RouterSetting, seams: Seams, door: &Door) -> Self {
        Self::with_news(setting, seams, door, Some(router::gym::NEWS_MODEL))
    }

    /// The configuration with grounded `gym.news` replies on `news` (a
    /// model id), when the door is a live gateway door and the Gym's
    /// records are here; `None` keeps them on the chat door.
    #[cfg(test)]
    fn with_news(setting: RouterSetting, seams: Seams, door: &Door, news: Option<&str>) -> Self {
        Self::with_news_and_jev(setting, seams, door, news, &[])
    }

    /// [`RouterConfig::with_news`] for a judge that falls back to
    /// `jev_fallbacks` (doors named for a person) when TypeSafe cannot
    /// answer: the privacy answer names them.
    /// This configuration for one job on the caller's own keys (BYOK):
    /// the bank's model and privacy answers name the caller's own key as
    /// the door, every recipient as reached on their keys, and Jev through
    /// `jev`, their providers that serve it.
    fn on_their_keys(mut self, door: &Door, jev: &[&str]) -> Self {
        let primary = match door {
            Door::Fallback(ordered) => {
                Some((ordered.primary.model.as_str(), ordered.primary.url.as_str()))
            }
            _ => None,
        };
        let (model, url) = match door {
            Door::Fallback(ordered) => (
                ordered.fallback.model.as_str(),
                Some(ordered.fallback.url.as_str()),
            ),
            Door::Live(live) => (live.model.as_str(), Some(live.url.as_str())),
            door => (door.model(), None),
        };
        self.facts = router::worker_facts_theirs(primary, model, url, &self.seams, jev);
        self.fell_back = match door {
            Door::Fallback(ordered) => {
                let named = first::Facts::of(&ordered.fallback.model, Some(&ordered.fallback.url));
                match (named.chat_model, named.chat_model_host) {
                    (Some(model), Some(host)) => Some(
                        self.facts
                            .clone()
                            .set("worker.lane.display", model)
                            .set("worker.door.display", format!("{host} on your own key")),
                    ),
                    _ => None,
                }
            }
            _ => None,
        };
        self
    }

    fn with_news_and_jev(
        setting: RouterSetting,
        seams: Seams,
        door: &Door,
        news: Option<&str>,
        jev_fallbacks: &[&str],
    ) -> Self {
        // The news lane is the gateway door's, with its reasoning off; a
        // worker with a primary asks the primary first there too (#10109).
        let news = match (door.gateway(), news) {
            (Some(live), Some(model)) if seams.gym.available() && model != live.model => {
                let gateway = live
                    .clone()
                    .serving(model)
                    .with_options(router::gym::news_options());
                Some(match door {
                    Door::Fallback(ordered) => Door::Fallback(Box::new(ordered.before(gateway))),
                    _ => Door::Live(gateway),
                })
            }
            _ => None,
        };
        let news_name = news.as_ref().and_then(Door::gateway).map(|news| {
            if news.model == router::gym::NEWS_MODEL {
                router::gym::NEWS_MODEL_NAME
            } else {
                news.model.as_str()
            }
        });
        let facts = match door {
            Door::Fallback(ordered) => router::worker_facts_ordered(
                Some((&ordered.primary.model, &ordered.primary.url)),
                &ordered.fallback.model,
                Some(&ordered.fallback.url),
                &seams,
                news_name,
                jev_fallbacks,
            ),
            Door::Live(live) => router::worker_facts_with_jev(
                &live.model,
                Some(&live.url),
                &seams,
                news_name,
                jev_fallbacks,
            ),
            door => router::worker_facts(door.model(), None, &seams),
        };
        // While the primary is missing its turns, our model is the
        // fallback, and a reply that names our model names it.
        let fell_back = match door {
            Door::Fallback(ordered) => {
                let named = first::Facts::of(&ordered.fallback.model, Some(&ordered.fallback.url));
                match (named.chat_model, named.chat_model_host) {
                    (Some(model), Some(host)) => Some(
                        facts
                            .clone()
                            .set("worker.lane.display", model)
                            .set("worker.door.display", host),
                    ),
                    _ => None,
                }
            }
            _ => None,
        };
        Self {
            setting,
            seams,
            facts,
            fell_back,
            news: news.map(Arc::new),
            calibration: None,
        }
    }
}

/// The environment variable naming the model grounded `gym.news` replies
/// run on: a model id or lane name, or `off` for the chat door's model.
/// Unset is [`router::gym::NEWS_MODEL`].
const GYM_NEWS_MODEL_VAR: &str = "CODER_GYM_NEWS_MODEL";

/// The news lane's model from [`GYM_NEWS_MODEL_VAR`].
fn gym_news_model_from_env() -> Option<String> {
    match env::var(GYM_NEWS_MODEL_VAR).as_deref().map(str::trim) {
        Ok("off") => None,
        Ok(model) if !model.is_empty() => Some(model_named(model).to_string()),
        _ => Some(router::gym::NEWS_MODEL.to_string()),
    }
}

/// How the worker serves a request that asks for the router.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RouterSetting {
    /// Every tier.
    Live,
    /// Phase 0: route and log, but serve what `opener` alone would.
    Shadow,
    /// Ignore `router`; serve what `opener` alone would, without the log.
    Off,
}

/// The environment variable that sets [`RouterSetting`].
const ROUTER_VAR: &str = "CODER_WORKER_ROUTER";

/// The environment variable that turns the chat router's CLI route off
/// (`off`); unset or `on` wires it whenever a judge is configured.
const CLI_VAR: &str = "CODER_WORKER_CLI";

/// Whether the CLI route is wired: `on` (the default) or `off`.
fn cli_from_env() -> Result<bool, String> {
    match env::var(CLI_VAR).as_deref() {
        Ok("on") | Ok("") | Err(_) => Ok(true),
        Ok("off") => Ok(false),
        Ok(other) => Err(format!("{CLI_VAR} is on or off, not `{other}`")),
    }
}

/// The CLI route's free-text fill through the worker's own door. It names
/// no recipient: the door is already named as the chat model.
struct DoorFill(Arc<Door>);

impl coder::cli_route::Fill for DoorFill {
    fn fill<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
    ) -> futures_util::future::BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let mut sink = |_: &str| {};
            let mut meta = |_: coder::generate::Meta| {};
            self.0
                .generate(instructions, input, &mut sink, &mut meta)
                .await
                .map(|(text, _)| text)
                .map_err(|error| error.cause().to_string())
        })
    }

    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }
}

/// The CLI route when [`CLI_VAR`] allows it and a judge descends the tree,
/// else the no-op.
fn cli_seam(
    judge: Option<&Arc<jev::Client>>,
    door: &Arc<Door>,
) -> Result<Arc<dyn coder::router::seams::CliRoute>, String> {
    Ok(match (cli_from_env()?, judge) {
        (true, Some(judge)) => Arc::new(coder::cli_route::CommandRoute::new(
            (**judge).clone(),
            Arc::new(DoorFill(door.clone())),
        )),
        _ => Arc::new(coder::router::seams::NoCli),
    })
}

/// A provider of the caller's, named for a person as the privacy answer
/// names a door.
fn door_name(provider: model_access::Provider) -> &'static str {
    match provider {
        model_access::Provider::OpenRouter => "OpenRouter",
        model_access::Provider::Vercel => "the Vercel AI Gateway",
        model_access::Provider::TypeSafe => "TypeSafe",
    }
}

/// The seams for one job on the caller's own keys (BYOK): each seam the
/// worker holds, lent the caller's embedder and Jev on their keys
/// ([`router::seams::TheirKeys`]), so product, codebase, Gym, CLI, and
/// authoring turns stay grounded on our records while every embedding,
/// judgment, and model call is theirs. A seam their keys cannot run (no Jev
/// on them, no key that embeds) is off for the job, never on ours.
fn their_seams(
    ours: &Seams,
    access: &model_access::Access,
    judge: Option<&Arc<jev::Client>>,
    door: &Arc<Door>,
) -> Seams {
    let personalize: Arc<dyn router::seams::Personalize> =
        match router::personalize::Personalizer::theirs(access) {
            Some(personalizer) => Arc::new(personalizer),
            None => Arc::new(router::seams::NoPersonalize),
        };
    let Some(judge) = judge else {
        return Seams {
            personalize,
            ..Seams::default()
        };
    };
    let theirs = router::seams::TheirKeys {
        access: access.clone(),
        judge: judge.clone(),
    };
    Seams {
        personalize,
        product: ours
            .product
            .on_their_keys(&theirs)
            .unwrap_or_else(|| Arc::new(router::seams::NoKb)),
        codebase: ours
            .codebase
            .on_their_keys(&theirs)
            .unwrap_or_else(|| Arc::new(router::seams::NoKb)),
        gym: ours
            .gym
            .on_their_keys(&theirs)
            .unwrap_or_else(|| Arc::new(router::seams::NoGym)),
        cli: if ours.cli.groups().is_empty() {
            Arc::new(router::seams::NoCli)
        } else {
            Arc::new(coder::cli_route::CommandRoute::new(
                (**judge).clone(),
                Arc::new(DoorFill(door.clone())),
            ))
        },
        author: if ours.author.available() {
            coder::eval_author::seam(
                &door,
                Some(judge.clone() as Arc<dyn coder::product_kb::Judge>),
            )
        } else {
            Arc::new(router::seams::NoAuthor)
        },
    }
}

fn router_from_env() -> Result<RouterSetting, String> {
    match env::var(ROUTER_VAR).as_deref() {
        Ok("live") | Ok("") | Err(_) => Ok(RouterSetting::Live),
        Ok("shadow") => Ok(RouterSetting::Shadow),
        Ok("off") => Ok(RouterSetting::Off),
        Ok(other) => Err(format!(
            "{ROUTER_VAR} is live, shadow, or off, not `{other}`"
        )),
    }
}

/// What one judged turn asked for, and what it needs to act on the
/// judgment.
struct Turn {
    mode: Mode,
    /// Route and log, but serve the legacy tier.
    shadow: bool,
    /// Whether the caller asked to be shown a first response at all
    /// (`opener` or `router`), rather than the judgment alone.
    show: bool,
    /// The user's latest message; it reaches a seam only through
    /// `router::redact`.
    message: String,
    context: router::Context,
    /// The authoring interview's draft the request carried, when it passed
    /// `router::card::draft`: data for the author seam, never an
    /// instruction.
    draft: Option<Value>,
    /// A try's or a full run's result for that draft, when it passed
    /// `router::card::tried`.
    tried: Option<ext_eval::author::runner::Tried>,
    /// Results a check must not be offered (`router::card::skip`): the
    /// phone's trainer's own and the ones it already checked.
    skip: Vec<String>,
}

impl Job {
    /// The model to name on this job's result: the one the door named as
    /// the writer, or the door's own.
    fn answered_model(&self) -> String {
        self.answered
            .lock()
            .ok()
            .and_then(|answered| answered.clone())
            .unwrap_or_else(|| self.door.model().to_string())
    }

    /// The bank's slot values now: the fallback's model while the primary
    /// is missing its turns, the configured one otherwise.
    fn facts(&self) -> &router::Facts {
        match (&*self.door, &self.routing.fell_back) {
            (Door::Fallback(ordered), Some(fell_back)) if ordered.primary_down() => fell_back,
            _ => &self.routing.facts,
        }
    }

    /// Answers one job request: decrypt, generate, publish.
    ///
    /// The request arrives signed by the customer and addressed to this
    /// worker; [`addressed`] saw to that. What is inside may still be
    /// unreadable, and that is answered, not dropped: the customer proved
    /// who they are, so a typed `malformed` tells them what to fix,
    /// where a silence would tell them the worker is down.
    ///
    /// Whatever happens, the job is recorded in the usage log afterwards
    /// (#10120): who asked, from where, how it was answered, and how long
    /// it took, never its text.
    async fn answer(mut self, request: &Event) -> Result<(), String> {
        let arrived = Instant::now();
        let observed = std::sync::Mutex::new(coder::relay::usage::Observed::new(
            &request.pubkey,
            request.content.len(),
            unix_now_ms(),
        ));
        // A job that carries the caller's own provider keys runs every
        // model call on them, or is refused: never on ours (BYOK).
        match self.bind_payer(request) {
            Ok(Some(payer)) => {
                if let Ok(mut observed) = observed.lock() {
                    observed.paid_by(&payer);
                }
                self.payer = Some(payer);
            }
            Ok(None) => {}
            Err(why) => self.payer_refusal = Some(why),
        }
        let served = self.serve(request, &observed, arrived).await;
        if let Some(log) = &self.usage {
            let observed = observed
                .into_inner()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            let upstream = self.upstream.lock().ok().and_then(|named| named.clone());
            let door = observed
                .record
                .model
                .as_deref()
                .map(|model| match &upstream {
                    // The gateway's choice: the upstream account that
                    // answered, and the gateway it went through.
                    Some(upstream) if !model.starts_with("bank:") => {
                        format!("{upstream} via {}", answering_door(&self.door, model))
                    }
                    _ => answering_door(&self.door, model),
                });
            let elapsed = u64::try_from(arrived.elapsed().as_millis()).unwrap_or(u64::MAX);
            if let Err(why) = log.append(&observed.finish(elapsed, door)) {
                eprintln!("usage: not recorded: {why}");
            }
        }
        served
    }

    /// Bind this job to the caller's own provider keys when its body names
    /// `payer.keys` (BYOK, NIP-CJ "Caller-paid model calls"): the model,
    /// the personalizer, Jev, and every routed seam's embedding and
    /// judgment run on the caller's keys for this job only ([`their_seams`]);
    /// a seam their keys cannot run is off, never on ours. The keys live in this job's doors and are dropped with it;
    /// they are never logged, recorded, or published. `None` for a job
    /// that names no payer.
    ///
    /// # Errors
    ///
    /// A sentence that never carries a key: the envelope is missing or does
    /// not open, or no key of the caller's serves the chat.
    fn bind_payer(&mut self, request: &Event) -> Result<Option<model_access::Payer>, String> {
        let Some(customer) = parse_hex(&request.pubkey)
            .and_then(|bytes| XOnlyPublicKey::from_byte_array(bytes).ok())
        else {
            return Ok(None);
        };
        let conversation = nip44::conversation_key(self.identity.secret(), &customer);
        let Some(payload) = nip44::decrypt(&request.content, &conversation)
            .ok()
            .and_then(|plaintext| serde_json::from_str::<Value>(&plaintext).ok())
        else {
            // `serve` refuses a body that does not open.
            return Ok(None);
        };
        let named = payload["requires"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item == model_access::PAYER_FEATURE));
        if !named {
            return Ok(None);
        }
        let sealed = payload["payer"]["keys"]
            .as_str()
            .ok_or("the job names payer.keys and carries no payer envelope")?;
        let mut opened = nip44::decrypt(sealed, &conversation)
            .map_err(|_| "the payer envelope does not decrypt".to_string())?;
        let keys = model_access::Keys::from_envelope_plaintext(&opened);
        // SAFETY: zero bytes keep the string valid UTF-8.
        unsafe { opened.as_bytes_mut().fill(0) };
        let access = model_access::Access::theirs(keys?);
        let (door, first, last) = their_door(&self.door, &access)?;
        let mut jev_names: Vec<&'static str> = Vec::new();
        let judge = match access.decisions() {
            Ok(model_access::Decisions::Theirs { config, order, .. }) => {
                jev_names = order.iter().map(|provider| door_name(*provider)).collect();
                let model = self.judge.as_ref().map_or_else(
                    || jev::defaults::MODEL.to_string(),
                    |judge| judge.default_model().to_string(),
                );
                jev::Client::new(config.default_model(model))
                    .ok()
                    .map(Arc::new)
            }
            _ => None,
        };
        let door = Arc::new(door);
        let seams = their_seams(&self.routing.seams, &access, judge.as_ref(), &door);
        let routing =
            RouterConfig::with_news_and_jev(self.routing.setting, seams, &door, None, &[])
                .calibrated(self.routing.calibration.clone())
                .on_their_keys(&door, if judge.is_some() { &jev_names[..] } else { &[] });
        self.door = door;
        self.judge = judge;
        self.routing = Arc::new(routing);
        self.payer_last = Some(last);
        Ok(Some(first))
    }

    /// [`Job::answer`]'s work, noting each published body in `observed`.
    async fn serve(
        &self,
        request: &Event,
        observed: &std::sync::Mutex<coder::relay::usage::Observed>,
        arrived: Instant,
    ) -> Result<(), String> {
        let customer = parse_hex(&request.pubkey)
            .and_then(|bytes| XOnlyPublicKey::from_byte_array(bytes).ok())
            .ok_or("the request's pubkey does not parse")?;
        let conversation = nip44::conversation_key(self.identity.secret(), &customer);
        let label = &request.id[..16];

        let publish = |kind: u16, content: Value| -> Result<(), String> {
            let ciphertext = nip44::encrypt(
                &content.to_string(),
                &conversation,
                secp256k1::rand::random::<[u8; 32]>(),
            )
            .map_err(|error| format!("encrypt: {error}"))?;
            let event = self.identity.signer().sign(
                unix_now(),
                kind,
                vec![
                    Tag::new(vec!["e".into(), request.id.clone()]),
                    Tag::new(vec!["p".into(), request.pubkey.clone()]),
                ],
                ciphertext,
            );
            self.publish
                .send(json!(["EVENT", event]))
                .map_err(|_| "the serving loop is gone".to_string())?;
            if let Ok(mut observed) = observed.lock() {
                let elapsed = u64::try_from(arrived.elapsed().as_millis()).unwrap_or(u64::MAX);
                observed.saw(&content, elapsed);
            }
            Ok(())
        };
        let refuse_after = |version: u64,
                            code: &str,
                            message: String,
                            retry_after_ms: Option<u64>|
         -> Result<(), String> {
            let mut status = json!({
                "v": version,
                "type": "status",
                "status": "error",
                "code": code,
                "message": message,
            });
            if let Some(wait) = retry_after_ms {
                status["retry_after_ms"] = json!(wait);
            }
            publish(FEEDBACK_KIND, status)?;
            eprintln!("job {label} declined: {code}");
            Ok(())
        };
        let refuse = |version: u64, code: &str, message: String| -> Result<(), String> {
            refuse_after(version, code, message, None)
        };

        let payload = match nip44::decrypt(&request.content, &conversation)
            .map_err(|error| format!("the content does not decrypt under NIP-44: {error}"))
            .and_then(|plaintext| {
                serde_json::from_str::<Value>(&plaintext)
                    .map_err(|error| format!("the payload is not JSON: {error}"))
            }) {
            Ok(payload) if payload.is_object() => {
                if let Ok(mut observed) = observed.lock() {
                    observed.request(&payload);
                }
                payload
            }
            Ok(_) => {
                return refuse(
                    PAYLOAD_VERSION,
                    "malformed",
                    "the payload is not a JSON object".to_string(),
                );
            }
            Err(why) => return refuse(PAYLOAD_VERSION, "malformed", why),
        };

        // The worker answers at the version the request named, so a
        // version-1 terminal gets version-1 feedback — partials with no
        // `seq`, since it cannot check one — and a request that names
        // anything else is declined rather than generated against a schema
        // this worker cannot read.
        let Some(version) = payload_version(&payload) else {
            return refuse(
                PAYLOAD_VERSION,
                "unsupported_version",
                "the job request names a payload version this worker does not serve".to_string(),
            );
        };

        // NIP-CJ: a body naming a feature this worker does not serve is
        // refused. The one feature served is `payer.keys`, and a job that
        // names it runs only on the caller's keys.
        let requires: Vec<&str> = match payload.get("requires") {
            None | Some(Value::Null) => Vec::new(),
            Some(Value::Array(items)) => {
                let words: Vec<&str> = items.iter().filter_map(Value::as_str).collect();
                if words.len() != items.len() {
                    return refuse(
                        version,
                        "malformed",
                        "requires is not a list of words".into(),
                    );
                }
                words
            }
            Some(_) => return refuse(version, "malformed", "requires is not a list".into()),
        };
        if let Some(unknown) = requires
            .iter()
            .find(|feature| **feature != model_access::PAYER_FEATURE)
        {
            return refuse(
                version,
                "unsupported_feature",
                format!("this worker does not serve the feature {unknown}"),
            );
        }
        if requires.contains(&model_access::PAYER_FEATURE) {
            if let Some(why) = &self.payer_refusal {
                // A fixed line (their keys can't serve the chat) travels as
                // the payer's own failure; anything else is malformed.
                let code = if model_access::Failure::parse_line(why).is_some() {
                    model_access::PAYER_FAILED
                } else {
                    "malformed"
                };
                return refuse(version, code, why.clone());
            }
            if self.payer.is_none() {
                return refuse(
                    version,
                    "malformed",
                    "the job names payer.keys and carries no usable payer envelope".into(),
                );
            }
        }

        let listed = self
            .allow
            .as_ref()
            .is_some_and(|keys| keys.contains(&request.pubkey));
        // An open worker admits callers off the list, below.
        let open_caller = !listed && self.ledger.is_some();
        if self.allow.is_some() && !listed && !open_caller {
            return refuse(
                version,
                "not_admitted",
                "this worker does not answer requests from your pubkey".to_string(),
            );
        }

        let age = unix_now().saturating_sub(request.created_at);
        if age > REQUEST_WINDOW.as_secs() {
            return refuse(
                version,
                "stale",
                format!(
                    "the request was created {age} s ago, past the {} s this worker answers",
                    REQUEST_WINDOW.as_secs()
                ),
            );
        }

        if let Some(code) = &self.decline {
            return refuse(
                version,
                code,
                format!("this worker is configured to decline every job ({code})"),
            );
        }

        // A probe asks whether this worker is here and what answers
        // through it. It costs nothing at the door: the answer is the
        // door's name and model, which is what a capability probe records
        // as the version.
        if payload["type"].as_str() == Some("probe") {
            publish(
                RESULT_KIND,
                json!({
                    "v": version,
                    "type": "result",
                    "text": format!("{} {}", self.door.name(), self.door.model()),
                    "model": self.door.model(),
                    "probe": {
                        "door": self.door.name(),
                        "model": self.door.model(),
                        "delegates": matches!(*self.door, Door::Executor(_)),
                    },
                }),
            )?;
            eprintln!("job {label} probed");
            return Ok(());
        }

        if self.permit.is_none() {
            return refuse(
                version,
                "busy",
                "this worker is running as many jobs as it admits at once".to_string(),
            );
        }

        if open_caller {
            // An open caller gets a conversation: a delegation is a
            // bounded task for the operator's own terminal.
            if !payload["delegation"].is_null() {
                return refuse(
                    version,
                    "not_admitted",
                    "this worker answers conversation jobs only from your pubkey".to_string(),
                );
            }
            let admitted = self.ledger.as_ref().map(|ledger| {
                ledger
                    .lock()
                    .map_err(|_| "the admission ledger is poisoned".to_string())
                    .map(|mut ledger| {
                        ledger.admit(&request.pubkey, request.content.len(), unix_now())
                    })
            });
            match admitted {
                Some(Ok(Err(refusal))) => {
                    return refuse_after(
                        version,
                        refusal.code(),
                        refusal.message(),
                        refusal.retry_after_ms(),
                    );
                }
                Some(Err(why)) => return refuse(version, "internal", why),
                Some(Ok(Ok(()))) | None => {}
            }
        }

        // A ranking of the caller's suggestions: one System One call, no
        // generation. Admitted exactly as a turn is, above.
        if payload["type"].as_str() == Some("rank") {
            return self.rank(version, &payload, &publish, &refuse).await;
        }

        let started = Instant::now();
        // A request that carries a `delegation` is one bounded task from a
        // fan-out on the terminal's side: the terminal holds no executor,
        // so the bounds it states are applied here, under this worker's
        // approval. A worker with no executor door cannot hold them and
        // says so rather than answering a writing task with prose.
        let minutes = payload["delegation"]["minutes"].as_u64();
        let answering = async {
            match (&payload["delegation"], &*self.door) {
                (Value::Object(delegation), Door::Executor(executor)) => {
                    let writes = delegation["writes"].as_bool().unwrap_or(false);
                    let minutes = minutes.unwrap_or(0);
                    let prompt = payload["task"].as_str().unwrap_or_default();
                    eprintln!(
                        "job {label} delegated: {} task, {minutes} min",
                        if writes { "writing" } else { "reading" }
                    );
                    // An executor answers in one piece, so without this the
                    // terminal hears nothing until the result. Its contact
                    // wait is shorter than many executor turns, and a
                    // silent turn past it reads as an absent worker.
                    publish(
                        FEEDBACK_KIND,
                        json!({
                            "v": version,
                            "type": "status",
                            "status": "processing",
                        }),
                    )
                    .map_err(GenerateError::Stream)?;
                    executor
                        .delegate(prompt, writes, minutes)
                        .await
                        .map(|text| (text, None, None))
                }
                (Value::Object(delegation), _) if delegation["writes"].as_bool() == Some(true) => {
                    Err(GenerateError::Refused {
                        code: "internal".to_string(),
                        message: format!(
                            "this worker answers through {}, which cannot run a writing task; \
                             it needs an executor door",
                            self.door.name()
                        ),
                    })
                }
                _ => {
                    // The turn is admitted: say so at once, before any
                    // model or judge has answered, so the caller hears the
                    // worker within one relay round trip.
                    publish(
                        FEEDBACK_KIND,
                        json!({
                            "v": version,
                            "type": "status",
                            "status": "processing",
                        }),
                    )
                    .map_err(GenerateError::Stream)?;
                    let input = transcript(&payload);
                    // The judge runs beside the model, never in front of
                    // it, and only for a caller that asks: `opener: true`
                    // for the judgment and an opener, `judge: true` for the
                    // judgment alone. A caller that asks for neither, such
                    // as Microcoder's cloud steps whose replies must be one
                    // JSON object, gets the model's reply untouched and
                    // spends no judgment.
                    // A request that names the router (`"router":
                    // "chat-router-v2"`, or v1) gets every tier; one that asks
                    // only for `opener` gets what the first response
                    // always showed: a prepared answer with no offer, an
                    // opener, or nothing.
                    // `chat-router-v1` (build 20) and `chat-router-v2` both
                    // ask for routing; both are routed with the v2 set.
                    let routed = router::asks_router(&payload["router"])
                        && self.routing.setting != RouterSetting::Off;
                    let opener = payload["opener"].as_bool() == Some(true) || routed;
                    let judged = opener || payload["judge"].as_bool() == Some(true);
                    let turn = Turn {
                        mode: if routed && self.routing.setting == RouterSetting::Live {
                            Mode::Router
                        } else {
                            Mode::Legacy
                        },
                        shadow: routed && self.routing.setting == RouterSetting::Shadow,
                        show: opener,
                        message: latest(&payload, &input),
                        context: router::Context::of(&payload["context"]),
                        draft: router::card::draft(&payload["draft"]).ok(),
                        tried: router::card::tried(&payload["tried"]).ok(),
                        skip: router::card::skip(&payload["skip"]),
                    };
                    // A plan's results (#10183): the model writes their
                    // combined summary, with no routing and no offer, from
                    // the transcript up to the request that started them.
                    let summary = !turn.context.runs.is_empty();
                    let mut input = input;
                    if summary {
                        while input
                            .last()
                            .is_some_and(|message| message.role != Role::User)
                        {
                            input.pop();
                        }
                    }
                    let triage = (judged && !summary)
                        .then(|| self.triage(&turn, &input))
                        .flatten();
                    let mut instructions = payload["instructions"]
                        .as_str()
                        .unwrap_or_default()
                        .to_string();
                    // Where the chat runs, from its typed context: on a
                    // computer, Coder runs here and the project folder is
                    // the working directory (#10077).
                    if let Some(note) = turn.context.note() {
                        if !instructions.is_empty() {
                            instructions.push_str("\n\n");
                        }
                        instructions.push_str(&note);
                    }
                    if let Some(note) = turn.context.runs_note() {
                        if !instructions.is_empty() {
                            instructions.push_str("\n\n");
                        }
                        instructions.push_str(&note);
                    }
                    if opener && triage.is_some() {
                        if !instructions.is_empty() {
                            instructions.push_str("\n\n");
                        }
                        instructions.push_str(first::MODEL_NOTE);
                    }
                    // A slow model is still working: past the primary's
                    // first-word wait, tell the caller once, so the chat
                    // shows that rather than a bare spinner.
                    let generating =
                        self.generate(version, &instructions, &input, &publish, triage, &turn);
                    tokio::pin!(generating);
                    let slow = tokio::time::sleep(coder::generate::PRIMARY_THINKING);
                    tokio::pin!(slow);
                    let mut told = false;
                    loop {
                        tokio::select! {
                            done = &mut generating => break done,
                            () = &mut slow, if !told => {
                                told = true;
                                // A failed notice never fails the turn.
                                let _ = publish(
                                    FEEDBACK_KIND,
                                    json!({
                                        "v": version,
                                        "type": "status",
                                        "status": "still_working",
                                    }),
                                );
                            }
                        }
                    }
                }
            }
        };
        // The stated minutes bound the executor; the worker waits that long
        // plus a grace for the executor's own report, then stops waiting
        // and says so. Dropping the run ends the executor's process group,
        // and the customer holds a typed answer rather than a silence it
        // cannot tell from a worker that went away. A job that states no
        // minutes is held to the worker's own bound, so the slot it holds
        // comes back whatever the door does.
        let (bound, stated) = match minutes {
            Some(minutes) => (
                Duration::from_secs(minutes.saturating_mul(60)) + self.waits.grace,
                format!("its {minutes}-minute limit"),
            ),
            None => (
                self.waits.undelegated,
                format!(
                    "this worker's {}-minute limit",
                    self.waits.undelegated.as_secs() / 60
                ),
            ),
        };
        let Ok(answered) = tokio::time::timeout(bound, answering).await else {
            eprintln!(
                "job {label} timed out after {} ms",
                started.elapsed().as_millis()
            );
            return refuse(
                version,
                "timed_out",
                format!("the job ran longer than {stated}, so this worker stopped waiting for it"),
            );
        };

        match answered {
            Ok((text, usage, served)) => {
                let mut result = json!({
                    "v": version,
                    "type": "result",
                    "text": text,
                    "usage": usage.map(|usage| json!({
                        "input": usage.input_tokens,
                        "output": usage.output_tokens,
                    })),
                    "model": self.answered_model(),
                });
                // A routed turn names its tier, route, and bank; one whose
                // text no model wrote names the bank as its `model`.
                if let Some(served) = &served {
                    router::wire::annotate(&mut result, served, Bank::builtin());
                }
                let by = result["model"].as_str().unwrap_or("?").to_string();
                publish(RESULT_KIND, result)?;
                eprintln!(
                    "job {label} answered in {} ms, {} chars, by {by}",
                    started.elapsed().as_millis(),
                    text.len()
                );
            }
            // The door said no with a code: the code travels as it is, so
            // the terminal can tell an executor's refusal from a failure.
            Err(GenerateError::Refused { code, message }) => {
                refuse(version, &code, message)?;
            }
            Err(error) => {
                // On the caller's own keys, their provider's refusal is
                // theirs: one plain line from its status, never the
                // provider's words (BYOK).
                if let (Some(provider), GenerateError::Status(status, _)) =
                    (self.payer_last, &error)
                    && let Some(failure) = model_access::Failure::of_status(provider, *status)
                {
                    refuse(version, model_access::PAYER_FAILED, failure.line())?;
                    eprintln!("job {label} failed on the caller's key: HTTP {status}");
                    return Ok(());
                }
                // The worker's own door failed. That is the worker's
                // problem and the caller should hear it as one, with a
                // code, rather than as silence it cannot tell from an
                // absent worker.
                refuse(version, "internal", error.to_string())?;
                eprintln!("job {label} failed: {error}");
            }
        }
        Ok(())
    }

    /// The router's judgment for this turn, as a future the generation
    /// races, or `None` when no judge is configured.
    ///
    /// It never fails the turn: a judge that errs or runs past
    /// [`first::LATE`] answers `None`, and the turn is the model's alone.
    /// Its log line is a [`Shadow`] record: ids, probabilities, tiers, and
    /// the judge's time, never message text.
    fn triage(&self, turn: &Turn, input: &[Message]) -> Option<Judging> {
        let judge = self.judge.clone()?;
        let routing = self.routing.clone();
        let bank = Bank::builtin();
        let groups = routing.seams.cli.groups();
        let tools = routing.seams.gym.tools();
        // The admitted-capability set for the turn: the built-ins, the
        // catalog, and Coder's adoptions (#9960).
        let admitted = router::Admitted::of(&tools, &routing.seams.gym.adoptions());
        // A desktop turn also asks which of the app's decks the message
        // means, for `presentation.open` (#10058); elsewhere there is no
        // slide viewer to open one in.
        let decks: &[openagents_deck::DeckEntry] =
            if turn.context.surface() == router::Surface::Desktop {
                router::decks()
            } else {
                &[]
            };
        // The bank's facts for this turn: placed on a computer when the
        // turn is, with the computer's name and project folder (#10077).
        let facts = turn.context.facts(self.facts());
        // Jev reads only that the chat's Coder run ended, never what it
        // reported (#10094).
        let judged = turn.context.judged(input);
        let request = router::request(
            &turn.message,
            &judged,
            bank,
            &facts,
            &groups,
            &tools,
            &admitted,
            decks,
        );
        let (mode, shadow, context) = (turn.mode, turn.shadow, turn.context.clone());
        let draft = turn.draft.is_some();
        // Whether the latest message has earlier ones to refer to: a
        // count, never text (#10138).
        let earlier = judged.len() > 1;
        // A plugin being made on this computer (#10177): our last message
        // ended at an open step's fixed line, an exact comparison.
        let plugin =
            makes_plugins(&turn.context) && coder::eval_author::plugin::open(input).is_some();
        Some(Box::pin(async move {
            let started = Instant::now();
            let answered = tokio::time::timeout(first::LATE, judge.system_one(request)).await;
            let milliseconds = started.elapsed().as_millis();
            match answered {
                Ok(Ok(response)) => {
                    // The door that answered: a fallback door names itself
                    // in `service.door` (`jev::doors`); otherwise the
                    // judge's own door.
                    let door = response
                        .service()
                        .and_then(|service| service["door"].as_str().map(str::to_string))
                        .unwrap_or_else(|| judge.base_url().to_string());
                    if door != judge.base_url() {
                        eprintln!("judge answered by door {door} in {milliseconds} ms");
                    }
                    let answered_by = (door, response.model.clone());
                    let mut reading = router::reading(&response, bank, &facts, &admitted);
                    if let Some(map) = &routing.calibration {
                        map.apply(&mut reading);
                    }
                    let personalize = routing.seams.personalize.available();
                    let decide = |mode| {
                        router::decide(
                            &reading,
                            bank,
                            &facts,
                            &router::Situation {
                                mode,
                                context: &context,
                                personalize,
                                draft,
                                earlier,
                                plugin,
                            },
                        )
                    };
                    let (decided, mut decisions) = router::decisions::capture(
                        &response,
                        routing.calibration.is_some(),
                        || decide(if shadow { Mode::Router } else { mode }),
                    );
                    let served = if shadow {
                        let (tier, served_decisions) = router::decisions::capture(
                            &response,
                            routing.calibration.is_some(),
                            || decide(Mode::Legacy),
                        );
                        decisions = served_decisions;
                        tier
                    } else {
                        decided.clone()
                    };
                    for reading in &mut decisions {
                        reading.action = Some(served.word().into());
                    }
                    let record = Shadow::of(
                        &reading,
                        bank,
                        mode,
                        shadow,
                        &decided,
                        &served,
                        u64::try_from(milliseconds).unwrap_or(u64::MAX),
                    )
                    .calibrated(routing.calibration.as_ref());
                    eprintln!("{}", record.line());
                    Some(Judged {
                        routing: reading,
                        decided,
                        served,
                        shadow,
                        answered_by,
                        decisions,
                    })
                }
                Ok(Err(error)) => {
                    eprintln!("judge failed in {milliseconds} ms: {error}");
                    None
                }
                Err(_) => {
                    eprintln!(
                        "judge ran past {} ms; the model answers unrouted",
                        first::LATE.as_millis()
                    );
                    None
                }
            }
        }))
    }

    /// Answers a `rank` job: the caller's candidates, most likely first.
    async fn rank(
        &self,
        version: u64,
        payload: &Value,
        publish: &(dyn Fn(u16, Value) -> Result<(), String> + Sync),
        refuse: &(dyn Fn(u64, &str, String) -> Result<(), String> + Sync),
    ) -> Result<(), String> {
        let Some(judge) = &self.judge else {
            return refuse(
                version,
                "unavailable",
                "this worker has no judge configured to rank suggestions".to_string(),
            );
        };
        let candidates = match first::candidates_of(&payload["candidates"]) {
            Ok(candidates) => candidates,
            Err(why) => return refuse(version, "malformed", why),
        };
        let draft = payload["draft"].as_str().unwrap_or_default();
        let input = transcript_only(payload);
        let started = Instant::now();
        let answered = tokio::time::timeout(
            first::BUDGET,
            judge.system_one(first::rank_request(draft, &input, &candidates)),
        )
        .await;
        match answered {
            Ok(Ok(response)) => {
                let ranked = first::ranking(&response, &candidates);
                eprintln!(
                    "ranked {} candidate(s) in {} ms",
                    ranked.len(),
                    started.elapsed().as_millis()
                );
                publish(
                    RESULT_KIND,
                    json!({
                        "v": version,
                        "type": "result",
                        "text": ranked.first().map_or("", |(id, _)| id.as_str()),
                        "model": response.model,
                        "ranked": ranked
                            .iter()
                            .map(|(id, p)| json!({ "id": id, "p": p }))
                            .collect::<Vec<_>>(),
                        "set": first::SET,
                    }),
                )
            }
            Ok(Err(error)) => refuse(version, "unavailable", format!("the judge failed: {error}")),
            Err(_) => refuse(
                version,
                "unavailable",
                format!(
                    "the judge did not answer in {} ms",
                    first::BUDGET.as_millis()
                ),
            ),
        }
    }

    /// Generates through the door, publishing partials as text collects.
    ///
    /// `triage` races the generation. When it answers before the model's
    /// first words and the caller asked to be shown a first response, its
    /// tier decides what the caller sees (see `coder::router::policy`):
    /// a whole bank answer or refusal, and the model call is dropped; a
    /// bank stem closed by a validated continuation or its generic end,
    /// and the model call is dropped; a retrieval or a CLI proposal, while
    /// the model keeps running as the fallback; or a lead line above the
    /// model's reply. Either way its typed judgment goes out as `judgment`
    /// feedback, and an offer as `offer` feedback. A judgment that arrives
    /// after the model has started adds the feedback only. The third value
    /// is what a routed turn served, for its result.
    ///
    /// A turn that asks to be shown a first response holds the model's
    /// words while the judgment is out, up to [`first::LATE`], so a late
    /// judgment still routes it (#10110); past [`first::BUDGET`] the bank's
    /// [`first::PROGRESS_OPENER`] line shows meanwhile, as partial `seq` 0,
    /// and is left out of the result (#10139). A judgment that never comes
    /// leaves the model's reply, written under [`first::UNROUTED_NOTE`].
    ///
    /// An opener the judgment chose is never written into the reply: a
    /// reply starts with its answer (#10139). Only a bank lead that says
    /// something, such as a possible secret's warning or the Gym news
    /// line, goes above the model's words.
    async fn generate(
        &self,
        version: u64,
        instructions: &str,
        input: &[Message],
        publish: &(dyn Fn(u16, Value) -> Result<(), String> + Sync),
        triage: Option<Judging>,
        turn: &Turn,
    ) -> Result<(String, Option<Usage>, Option<Served>), GenerateError> {
        // The progress line shows only while the reply is pending: the
        // result, which replaces the partials, starts with the reply
        // (#10139).
        let mut opening = String::new();
        self.routed(
            version,
            instructions,
            input,
            publish,
            triage,
            turn,
            &mut opening,
        )
        .await
    }

    /// [`Job::generate`]'s turn, with the progress line it showed, if any,
    /// in `opening`; the reply's text comes back without it.
    #[allow(clippy::too_many_arguments)]
    async fn routed(
        &self,
        version: u64,
        instructions: &str,
        input: &[Message],
        publish: &(dyn Fn(u16, Value) -> Result<(), String> + Sync),
        triage: Option<Judging>,
        turn: &Turn,
        opening: &mut String,
    ) -> Result<(String, Option<Usage>, Option<Served>), GenerateError> {
        let bank = Bank::builtin();
        let placed = turn.context.facts(self.facts());
        let facts = &placed;
        let seams = &self.routing.seams;
        let mut judging = triage.is_some();
        // The model's words wait for a judgment the caller asked to be
        // shown, until it comes or runs past `first::LATE` (the triage
        // future's own bound); a model that finishes meanwhile waits at
        // the gate.
        let hold = turn.show && judging;
        let (gate, gate_open) = watch::channel(!hold);
        // The call started with the turn answers when no judgment routes
        // it, so it carries the fixed unrouted note.
        let unrouted = if turn.show {
            if instructions.is_empty() {
                first::UNROUTED_NOTE.to_string()
            } else {
                format!("{instructions}\n\n{}", first::UNROUTED_NOTE)
            }
        } else {
            instructions.to_string()
        };
        let (generating, mut incoming, mut said) =
            start_model(self.door.clone(), unrouted, input.to_vec());
        let mut generating = gated(generating, gate_open);
        // Whether `buffer` holds words written while they were held.
        let mut held = false;
        // A late judgment whose seam races a model that already has words:
        // the words wait for the seam too.
        let mut seam_holds = false;
        let mut progress = Box::pin(tokio::time::sleep(first::BUDGET));
        let mut progressing = hold;
        // Partials after a progress line move up one `seq`.
        let shift = std::sync::atomic::AtomicU64::new(0);
        let mut triage: Judging =
            triage.unwrap_or_else(|| Box::pin(std::future::pending::<Option<Judged>>()));
        let mut seam: SeamCall = Box::pin(std::future::pending());
        let mut seam_waiting = false;
        // What the judgment decided, kept for the seam's answer.
        let mut pending: Option<(router::Routing, Tier)> = None;
        // The lead line shown above the model's reply, which leads the
        // result too.
        let mut lead = String::new();
        let mut buffer = String::new();
        let mut partial_seq = 0u64;
        let mut model_started = false;
        let mut draining = true;
        let mut served: Option<Served> = None;
        // A grounded Gym reply's citations are for us: they are taken out
        // as the reply streams, with the items the model was given.
        // A grounded product reply's `[openagents.…]` citations likewise.
        let mut tidy: Option<(router::gym::Tidy, Cites)> = None;
        // When the turn began, the Gym seam answered, and the model's first
        // words went out: durations for the `router gym reply` line.
        let begun = Instant::now();
        let mut seam_ms: Option<u128> = None;
        let mut words_ms: Option<u128> = None;
        let send = |seq: u64, text: &str| {
            let seq = seq + shift.load(std::sync::atomic::Ordering::Relaxed);
            publish(FEEDBACK_KIND, partial_payload(version, seq, text))
                .map_err(GenerateError::Stream)
        };
        // An offer or card NIP-CJ's own writer refuses is not sent; the
        // reply stands without it.
        let offer = |offer: &router::Offer| match offer.feedback(version) {
            Ok(body) => publish(FEEDBACK_KIND, body).map_err(GenerateError::Stream),
            Err(why) => {
                eprintln!("router offer {} not sent: {why}", offer.word());
                Ok(())
            }
        };
        let card = |card: &router::card::Card| match card.feedback(version) {
            Ok(body) => publish(FEEDBACK_KIND, body).map_err(GenerateError::Stream),
            Err(why) => {
                eprintln!("router card {} not sent: {why}", card.word());
                Ok(())
            }
        };
        loop {
            let holding = hold && (judging || (seam_holds && seam_waiting));
            gate.send_replace(!holding);
            if !holding && held {
                held = false;
                if !buffer.is_empty() {
                    if !model_started {
                        words_ms = Some(begun.elapsed().as_millis());
                    }
                    model_started = true;
                    send(partial_seq, &buffer)?;
                    partial_seq += 1;
                    buffer.clear();
                }
            }
            tokio::select! {
                () = &mut progress, if progressing && judging => {
                    progressing = false;
                    eprintln!(
                        "judge ran past {} ms; holding the model up to {} ms",
                        first::BUDGET.as_millis(),
                        first::LATE.as_millis()
                    );
                    if partial_seq == 0
                        && let Some(line) = bank.opener(first::PROGRESS_OPENER)
                    {
                        *opening = format!("{}\n\n", line.text);
                        send(0, opening)?;
                        shift.store(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                judged = &mut triage, if judging => {
                    judging = false;
                    let Some(judged) = judged else { continue };
                    let mut judgment = router::wire::judgment(
                        version,
                        &judged.routing,
                        &judged.served,
                        bank,
                        judged.shadow.then_some(&judged.decided),
                    );
                    // Which Jev door answered and the model it served, so
                    // the thread's decision record names them (additive).
                    judgment["door"] = Value::String(judged.answered_by.0.clone());
                    judgment["model"] = Value::String(judged.answered_by.1.clone());
                    judgment["decisions"] = json!(judged.decisions);
                    publish(FEEDBACK_KIND, judgment).map_err(GenerateError::Stream)?;
                    let routing = judged.routing;
                    let tier = judged.served;
                    let mut record = served_of(&routing, &tier, bank, facts);
                    if !turn.show || partial_seq != 0 {
                        continue;
                    }
                    // Words the model wrote while held are the reply's
                    // start only on a tier that keeps the model; a tier
                    // with a seam then holds them until the seam answers.
                    let keeps = matches!(
                        tier,
                        Tier::Model { note: None, .. } | Tier::Grounded { .. } | Tier::Cli { .. }
                    );
                    // A seam's tier holds the model's words until the seam
                    // answers, whether or not any came while the judgment
                    // was pending: a model faster than the retrieval (the
                    // primary's first words come in about a second) would
                    // otherwise answer a knowledge question before its
                    // knowledge arrived (#10109).
                    seam_holds = matches!(tier, Tier::Grounded { .. } | Tier::Cli { .. });
                    if !keeps {
                        buffer.clear();
                        held = false;
                    }
                    match &tier {
                        // The whole reply: returning drops the model call.
                        Tier::CannedFinal { text, .. } | Tier::Refuse { text, .. } => {
                            send(0, text)?;
                            if let Tier::CannedFinal { offer: Some(shown), .. } = &tier {
                                offer(shown)?;
                            }
                            return Ok((text.clone(), None, Some(record)));
                        }
                        // A missing capability (#9960): the bank's line,
                        // the card that names the closest admitted one and
                        // how to add one (the interview when it is wired,
                        // else the Gym), and the Gym offer; the model call
                        // is dropped.
                        Tier::Capability { text, closest, .. } => {
                            send(0, text)?;
                            let add = if seams.author.available() {
                                nostr::cj_conversation::Add::Author
                            } else {
                                nostr::cj_conversation::Add::Gym
                            };
                            card(&router::card::Card::Capability {
                                closest: closest.clone(),
                                add,
                            })?;
                            if add == nostr::cj_conversation::Add::Gym {
                                offer(&router::Offer::OpenScreen {
                                    screen: router::Screen::VerseGym,
                                    label: "See the Gym".to_string(),
                                })?;
                            }
                            return Ok((text.clone(), None, Some(record)));
                        }
                        Tier::CannedStem { stem, generic_end, offer: shown, personalize, answer } => {
                            send(0, stem)?;
                            partial_seq = 1;
                            // The stem is the reply's start, so the model's
                            // words can no longer follow it: drop the call.
                            generating = Box::pin(std::future::pending());
                            draining = false;
                            if *personalize && seams.personalize.available() {
                                let ask = Ask {
                                    route: routing.route,
                                    answer: answer.id.clone(),
                                    stem: stem.clone(),
                                    message: router::redact(&turn.message),
                                };
                                seam = continuation(seams, ask);
                                seam_waiting = true;
                                pending = Some((routing, tier.clone()));
                                served = Some(record);
                                continue;
                            }
                            send(1, generic_end)?;
                            if let Some(shown) = shown {
                                offer(shown)?;
                            }
                            return Ok((format!("{stem}{generic_end}"), None, Some(record)));
                        }
                        Tier::Grounded { corpus, .. } => {
                            let lookup = Lookup {
                                message: router::redact(&turn.message),
                                transcript: input.to_vec(),
                            };
                            seam = grounding(seams, *corpus, lookup);
                            seam_waiting = true;
                            pending = Some((routing, tier.clone()));
                        }
                        Tier::Gym { route, .. } => {
                            // Every Gym reply is the bank's or a model
                            // restarted with the records (or told there are
                            // none): the call started with the turn is never
                            // shown, so it is dropped (#9950).
                            generating = Box::pin(std::future::pending());
                            draining = false;
                            let lookup = GymLookup {
                                route: *route,
                                message: router::redact(&turn.message),
                                transcript: input.to_vec(),
                            };
                            seam = gym_records(seams, lookup);
                            seam_waiting = true;
                            pending = Some((routing, tier.clone()));
                        }
                        // The interview's words come from the seam, or the
                        // bank: drop the model call.
                        Tier::Author => {
                            generating = Box::pin(std::future::pending());
                            draining = false;
                            let ask = AuthorAsk {
                                message: router::redact(&turn.message),
                                transcript: input.to_vec(),
                                draft: turn.draft.clone(),
                                tried: turn.tried.clone(),
                                surface: turn.context.surface(),
                                here: makes_plugins(&turn.context),
                                coder_run: turn.context.coder_run.clone(),
                            };
                            seam = author_step(seams, ask);
                            seam_waiting = true;
                            pending = Some((routing, tier.clone()));
                            served = Some(record);
                            continue;
                        }
                        Tier::Cli { group, also, .. } => {
                            let ask = CliAsk {
                                group: group.clone(),
                                also: also.clone(),
                                message: router::redact(&turn.message),
                                transcript: input.to_vec(),
                                surface: turn.context.surface(),
                            };
                            seam = proposal(seams, ask);
                            seam_waiting = true;
                            pending = Some((routing, tier.clone()));
                        }
                        Tier::Model { lead: shown, note } => {
                            if let Some(note) = note {
                                (generating, incoming, said) = start_model(
                                    self.door.clone(),
                                    format!("{instructions}\n\n{note}"),
                                    input.to_vec(),
                                );
                                draining = true;
                            }
                            if let Some(shown) = shown
                                && bank.opener(&shown.id).is_none()
                                && opening.is_empty()
                            {
                                lead = format!("{}\n\n", shown.text);
                                send(partial_seq, &lead)?;
                                partial_seq += 1;
                            }
                        }
                    }
                    record.model = None;
                    served = Some(record);
                }
                outcome = &mut seam, if seam_waiting => {
                    seam_waiting = false;
                    let Some((routing, tier)) = pending.take() else { continue };
                    // A sure wallet request in a terminal whose descent
                    // found no command still checks the built-in wallet:
                    // its read-only overview (#10170).
                    let outcome = match outcome {
                        SeamOutcome::Cli(Ok(CliAnswer::NoCommand))
                            if matches!(tier, Tier::Cli { .. })
                                && routing.route == router::RouteId::Wallet
                                && turn.context.surface() == router::Surface::Terminal =>
                        {
                            SeamOutcome::Cli(Ok(CliAnswer::Proposal(
                                router::policy::wallet_overview(),
                            )))
                        }
                        outcome => outcome,
                    };
                    match (outcome, &tier) {
                        (
                            SeamOutcome::Continued(continued),
                            Tier::CannedStem { stem, generic_end, offer: shown, .. },
                        ) => {
                            let (end, model) =
                                router::close_stem(generic_end, continued.as_ref(), &turn.message);
                            send(1, &end)?;
                            if let Some(shown) = shown {
                                offer(shown)?;
                            }
                            let mut record = served.take().unwrap_or_default();
                            if let Some(model) = model {
                                record.model = Some(model);
                            }
                            return Ok((format!("{stem}{end}"), None, Some(record)));
                        }
                        (SeamOutcome::Author(stepped), Tier::Author) => {
                            let checked = stepped
                                .ok()
                                .and_then(|step| router::gym::check_step(&step).ok());
                            if let Some(step) = checked {
                                send(0, &step.text)?;
                                if let Some(draft) = &step.draft {
                                    card(&router::card::Card::Draft { draft: draft.clone() })?;
                                }
                                if let Some(shown) = &step.offer {
                                    offer(shown)?;
                                }
                                let record = Served {
                                    tier: "author",
                                    route: routing.route.word(),
                                    model: Some(step.model.clone()),
                                    plugin: step.plugin.as_ref().map(|flow| flow.wire()),
                                    ..Served::default()
                                };
                                return Ok((step.text, None, Some(record)));
                            }
                            // No interview wired, or its step failed its
                            // checks: the bank says so.
                            if let Some(entry) = bank.entry("eval.author.soon")
                                && let Some(text) = entry.render(facts)
                            {
                                send(0, &text)?;
                                let record = Served {
                                    tier: "author",
                                    route: routing.route.word(),
                                    answer: Some(entry.tag()),
                                    model: Some(format!("bank:{}", bank.name)),
                                    followups: bank.followups(entry, facts),
                                    ..Served::default()
                                };
                                return Ok((text, None, Some(record)));
                            }
                            return Err(GenerateError::Stream(
                                "the bank has no eval.author.soon".to_string(),
                            ));
                        }
                        // The model has spoken meanwhile: its reply stands.
                        _ if partial_seq != 0 => {}
                        (SeamOutcome::Gym(found), Tier::Gym { route, tool, lead: shown }) => {
                            seam_ms = Some(begun.elapsed().as_millis());
                            let reply = match &found {
                                // A check skips what the phone names as its
                                // trainer's own or already checked.
                                Ok(found) if *route == router::RouteId::EvalCheck && !turn.skip.is_empty() => {
                                    router::gym::reply(*route, tool.as_deref(), &found.skipping(&turn.skip), bank, facts)
                                }
                                Ok(found) => router::gym::reply(*route, tool.as_deref(), found, bank, facts),
                                Err(SeamError::Failed(why)) => {
                                    eprintln!("router gym seam failed: {why}");
                                    router::gym::Reply::Model
                                }
                                Err(SeamError::Unavailable) => {
                                    router::gym::reply(*route, tool.as_deref(), &Default::default(), bank, facts)
                                }
                            };
                            match reply {
                                router::gym::Reply::Bank { answer, text, card: shown_card, offer: shown_offer } => {
                                    send(0, &text)?;
                                    if let Some(shown) = &shown_card {
                                        card(shown)?;
                                    }
                                    if let Some(shown) = &shown_offer {
                                        offer(shown)?;
                                    }
                                    let record = Served {
                                        tier: "gym",
                                        route: routing.route.word(),
                                        answer: Some(answer.tag()),
                                        model: Some(format!("bank:{}", bank.name)),
                                        followups: bank.followups(&answer, facts),
                                        ..Served::default()
                                    };
                                    return Ok((text, None, Some(record)));
                                }
                                router::gym::Reply::Grounded { items, card: shown_card } => {
                                    // The first words go with the card, from
                                    // the bank, before the model's (#9950).
                                    let opening = bank
                                        .entry(router::gym::NEWS_LEAD)
                                        .and_then(|entry| entry.render(facts));
                                    if let Some(text) = &opening {
                                        lead = format!("{text}\n\n");
                                        send(partial_seq, &lead)?;
                                        partial_seq += 1;
                                    }
                                    card(&shown_card)?;
                                    let door = self.routing.news.clone().unwrap_or_else(|| self.door.clone());
                                    (generating, incoming, said) = start_model(
                                        door.clone(),
                                        format!(
                                            "{instructions}\n\n{}",
                                            router::gym::instructions(&items, opening.as_deref())
                                        ),
                                        input.to_vec(),
                                    );
                                    draining = true;
                                    buffer.clear();
                                    if let Some(record) = &mut served {
                                        record.citations = items.iter().map(Into::into).collect();
                                        record.model = Some(door.model().to_string());
                                    }
                                    tidy = Some((router::gym::Tidy::default(), Cites::Gym(items)));
                                }
                                router::gym::Reply::Model => {
                                    (generating, incoming, said) = start_model(
                                        self.door.clone(),
                                        format!("{instructions}\n\n{}", router::gym::NO_RECORDS_NOTE),
                                        input.to_vec(),
                                    );
                                    draining = true;
                                    buffer.clear();
                                    if let Some(record) = &mut served {
                                        record.tier = "model";
                                    }
                                }
                            }
                            if let Some(shown) = shown
                                && bank.opener(&shown.id).is_none()
                                && lead.is_empty()
                                && opening.is_empty()
                            {
                                lead = format!("{}\n\n", shown.text);
                                send(partial_seq, &lead)?;
                                partial_seq += 1;
                            }
                        }
                        (SeamOutcome::Grounded(Ok(found)), Tier::Grounded { corpus, lead: shown }) => {
                            match router::grounded(
                                &found,
                                routing.needs_specifics,
                                turn.context.here(),
                                turn.context.surface() == router::Surface::Web,
                            ) {
                                router::Grounded::Answer(passage) => {
                                    let text = passage.answer.clone().unwrap_or_default();
                                    send(0, &text)?;
                                    let record = Served {
                                        tier: "canned",
                                        route: routing.route.word(),
                                        answer: Some(passage.id.clone()),
                                        model: Some(format!("kb:{}", corpus.word())),
                                        citations: vec![(&passage).into()],
                                        commit: found.commit.clone(),
                                        ..Served::default()
                                    };
                                    return Ok((text, None, Some(record)));
                                }
                                router::Grounded::Dispatch => {
                                    if let Some(tier) = explore(bank, facts, turn) {
                                        return self
                                            .finish_stem(&tier, &routing, bank, turn, &send, &offer)
                                            .await;
                                    }
                                }
                                grounded => {
                                    let (note, passages) = match &grounded {
                                        router::Grounded::Passages(passages) => (
                                            router::grounded_note(*corpus, passages, found.commit.as_deref()),
                                            passages.clone(),
                                        ),
                                        _ => (router::NO_DOCS_NOTE.to_string(), Vec::new()),
                                    };
                                    (generating, incoming, said) = start_model(
                                        self.door.clone(),
                                        format!("{instructions}\n\n{note}"),
                                        input.to_vec(),
                                    );
                                    draining = true;
                                    buffer.clear();
                                    if let Some(record) = &mut served {
                                        record.citations = passages.iter().map(Into::into).collect();
                                        record.commit = found.commit.clone();
                                    }
                                    if *corpus == router::Corpus::Product {
                                        tidy = Some((
                                            coder::product_kb::tidier(),
                                            Cites::Product(Box::new(found.clone())),
                                        ));
                                    }
                                    if let Some(shown) = shown
                                        && bank.opener(&shown.id).is_none()
                                        && opening.is_empty()
                                    {
                                        lead = format!("{}\n\n", shown.text);
                                        send(partial_seq, &lead)?;
                                        partial_seq += 1;
                                    }
                                }
                            }
                        }
                        (SeamOutcome::Cli(Ok(CliAnswer::Proposal(proposal))), Tier::Cli { lead: shown, .. }) => {
                            match router::gate(proposal.effect, turn.context.surface()) {
                                router::CliGate::Offer => {
                                    // A terminal runs a read-only command at
                                    // once and shows what it printed
                                    // (#10170); anything else waits for a
                                    // confirm.
                                    let id = if turn.context.surface() == router::Surface::Terminal
                                        && proposal.effect == router::Effect::ReadOnly
                                    {
                                        "cli.run"
                                    } else {
                                        "cli.offer"
                                    };
                                    if let Some(entry) = bank.entry(id)
                                        && let Some(text) = entry.render(facts)
                                    {
                                        send(0, &text)?;
                                        offer(&router::Offer::Cli {
                                            argv: proposal.argv.clone(),
                                            effect: proposal.effect,
                                            runs_on: proposal.runs_on,
                                        })?;
                                        let record = Served {
                                            tier: "cli",
                                            route: routing.route.word(),
                                            answer: Some(entry.tag()),
                                            model: Some(format!("bank:{}", bank.name)),
                                            ..Served::default()
                                        };
                                        return Ok((text, None, Some(record)));
                                    }
                                }
                                router::CliGate::Screen(screen) => {
                                    let id = match screen {
                                        router::Screen::Wallet => "wallet.send",
                                        _ => "account.computers",
                                    };
                                    if let Some(entry) = bank.placed(id, facts)
                                        && let Some(text) = entry.render(facts)
                                    {
                                        send(0, &text)?;
                                        if let Some(shown) = entry.offer() {
                                            offer(&shown)?;
                                        }
                                        let record = Served {
                                            tier: "canned",
                                            route: routing.route.word(),
                                            answer: Some(entry.tag()),
                                            model: Some(format!("bank:{}", bank.name)),
                                            ..Served::default()
                                        };
                                        return Ok((text, None, Some(record)));
                                    }
                                }
                                router::CliGate::Withhold => {}
                            }
                            if let Some(shown) = shown
                                && bank.opener(&shown.id).is_none()
                                && opening.is_empty()
                            {
                                lead = format!("{}\n\n", shown.text);
                                send(partial_seq, &lead)?;
                                partial_seq += 1;
                            }
                        }
                        (SeamOutcome::Cli(Ok(CliAnswer::Missing(what))), _) => {
                            if let Some(entry) = bank.entry("clarify.generic")
                                && let Some((stem, generic_end)) = entry.stem(facts)
                            {
                                let asked = Continuation {
                                    text: format!(" {}?", what.trim().trim_end_matches('?')),
                                    model: format!("bank:{}", bank.name),
                                };
                                let (end, _) =
                                    router::close_stem(&generic_end, Some(&asked), &turn.message);
                                send(0, &format!("{stem}{end}"))?;
                                let record = Served {
                                    tier: "stem",
                                    route: routing.route.word(),
                                    answer: Some(entry.tag()),
                                    model: Some(format!("bank:{}", bank.name)),
                                    ..Served::default()
                                };
                                return Ok((format!("{stem}{end}"), None, Some(record)));
                            }
                        }
                        // Nothing found, nothing configured, or a failure:
                        // the model already running is the reply, under
                        // its lead line.
                        (outcome, tier) => {
                            if let SeamOutcome::Grounded(Err(SeamError::Failed(why)))
                            | SeamOutcome::Cli(Err(SeamError::Failed(why))) = &outcome
                            {
                                eprintln!("router seam failed: {why}");
                            }
                            let mut tier_word = "model";
                            // A wallet request the wallet's commands did
                            // not serve: the model answers about the
                            // built-in wallet, never asking which (#10170).
                            if matches!(tier, Tier::Cli { .. }) && routing.route == router::RouteId::Wallet {
                                buffer.clear();
                                held = false;
                                (generating, incoming, said) = start_model(
                                    self.door.clone(),
                                    format!("{instructions}\n\n{}", router::policy::WALLET_NOTE),
                                    input.to_vec(),
                                );
                                draining = true;
                            }
                            if let Tier::Grounded { lead: Some(shown), .. } | Tier::Cli { lead: Some(shown), .. } = tier
                                && bank.opener(&shown.id).is_none()
                                && opening.is_empty()
                            {
                                lead = format!("{}\n\n", shown.text);
                                send(partial_seq, &lead)?;
                                partial_seq += 1;
                                tier_word = "opener";
                            }
                            // The result says what was served: the model.
                            if let Some(record) = &mut served {
                                record.tier = tier_word;
                            }
                        }
                    }
                }
                delta = incoming.recv(), if draining => match delta {
                    Some(delta) => {
                        match &mut tidy {
                            Some((tidying, _)) => buffer.push_str(&tidying.push(&delta)),
                            None => buffer.push_str(&delta),
                        }
                        // Held words wait for the judgment. Otherwise the
                        // model's first delta goes at once, so a reader
                        // sees the answer begin; later ones collect.
                        if holding {
                            held = true;
                        } else if buffer.len() >= PARTIAL_BYTES || !model_started {
                            if !model_started {
                                words_ms = Some(begun.elapsed().as_millis());
                            }
                            model_started = true;
                            // `seq` is the signed ordering the terminal
                            // checks deltas against; arrival order proves
                            // nothing. A version-1 answer makes no such
                            // promise and carries none.
                            send(partial_seq, &buffer)?;
                            partial_seq += 1;
                            buffer.clear();
                        }
                    }
                    // The generation dropped the sender, so nothing more is
                    // coming and the branch would otherwise spin.
                    None => draining = false,
                },
                answered = &mut generating => {
                    // The model that wrote the reply, when the door named
                    // one: the result names it rather than the door's
                    // first choice (#10109).
                    let (named_model, named_upstream) = said
                        .lock()
                        .ok()
                        .map(|mut named| (named.model.take(), named.upstream.take()))
                        .unwrap_or_default();
                    if let Some(upstream) = named_upstream
                        && let Ok(mut slot) = self.upstream.lock()
                    {
                        *slot = Some(upstream);
                    }
                    if let Some(model) = named_model {
                        if let Some(record) = &mut served
                            && record.model.is_some()
                        {
                            record.model = Some(model.clone());
                        }
                        if let Ok(mut answered) = self.answered.lock() {
                            *answered = Some(model);
                        }
                    }
                    return answered.map(|(text, usage)| {
                        let text = match &tidy {
                            Some((_, Cites::Product(grounding))) => {
                                let cited = coder::product_kb::cited(&text, grounding);
                                // Ids only, never the reply.
                                eprintln!(
                                    "router product reply: cited {:?}, unknown {:?}",
                                    cited.known, cited.unknown
                                );
                                coder::product_kb::tidy(&text)
                            }
                            Some((_, Cites::Gym(items))) => {
                                let cited = router::gym::check_reply(&text, items);
                                let shown = router::gym::tidy(&text);
                                let check = router::gym::post_check(&shown);
                                // Ids, words, and durations only, never the
                                // reply.
                                eprintln!(
                                    "router gym reply: {} cited, {} invented, banned {:?}, {} raw ids; \
                                     records at {} ms, model's first words at {} ms, done at {} ms",
                                    cited.known.len(),
                                    cited.invented.len(),
                                    check.banned,
                                    check.raw.len(),
                                    seam_ms.unwrap_or_default(),
                                    words_ms.unwrap_or_default(),
                                    begun.elapsed().as_millis()
                                );
                                shown
                            }
                            None => text,
                        };
                        (format!("{lead}{text}"), usage, served)
                    });
                }
            }
        }
    }

    /// Serves a dispatch stem decided after retrieval: the stem, a
    /// continuation or its generic end, and the offer.
    async fn finish_stem(
        &self,
        tier: &Tier,
        routing: &router::Routing,
        bank: &Bank,
        turn: &Turn,
        send: &(dyn Fn(u64, &str) -> Result<(), GenerateError> + Sync),
        offer: &(dyn Fn(&router::Offer) -> Result<(), GenerateError> + Sync),
    ) -> Result<(String, Option<Usage>, Option<Served>), GenerateError> {
        let Tier::CannedStem {
            answer,
            stem,
            generic_end,
            offer: shown,
            personalize,
        } = tier
        else {
            return Err(GenerateError::Stream("not a stem".to_string()));
        };
        send(0, stem)?;
        let seams = &self.routing.seams;
        let continued = if *personalize && seams.personalize.available() {
            let ask = Ask {
                route: routing.route,
                answer: answer.id.clone(),
                stem: stem.clone(),
                message: router::redact(&turn.message),
            };
            match continuation(seams, ask).await {
                SeamOutcome::Continued(continued) => continued,
                _ => None,
            }
        } else {
            None
        };
        let (end, model) = router::close_stem(generic_end, continued.as_ref(), &turn.message);
        send(1, &end)?;
        if let Some(shown) = shown {
            offer(shown)?;
        }
        let mut record = served_of(routing, tier, bank, &turn.context.facts(self.facts()));
        if let Some(model) = model {
            record.model = Some(model);
        }
        Ok((format!("{stem}{end}"), None, Some(record)))
    }
}

/// The router's judgment, decided.
struct Judged {
    routing: router::Routing,
    /// What the router decided.
    decided: Tier,
    /// What the turn serves: `decided`, or in shadow mode the legacy tier.
    served: Tier,
    shadow: bool,
    /// The Jev door that answered and the model it served.
    answered_by: (String, String),
    decisions: Vec<route_contract::decision::DecisionReading>,
}

/// The router's judgment, running.
type Judging = Pin<Box<dyn Future<Output = Option<Judged>> + Send>>;

/// A model call, running.
type Generation =
    Pin<Box<dyn Future<Output = Result<(String, Option<Usage>), GenerateError>> + Send>>;

/// Starts a model call whose deltas arrive on the returned receiver.
/// Dropping the future cancels the call.
///
/// The third value is the model the door names as having written the
/// answer, once it does: a door with a fallback names the one that
/// answered ([`Meta::Model`]), and every other door names none.
fn start_model(
    door: Arc<Door>,
    instructions: String,
    input: Vec<Message>,
) -> (Generation, mpsc::UnboundedReceiver<String>, Said) {
    let (deltas, incoming) = mpsc::unbounded_channel::<String>();
    let said = Said::default();
    let naming = said.clone();
    let generating = async move {
        // The sink owns the sender, so the channel closes when the call
        // ends and the drain stops.
        let mut sink = move |delta: &str| {
            let _ = deltas.send(delta.to_string());
        };
        door.generate(&instructions, &input, &mut sink, &mut |meta| {
            if let Ok(mut named) = naming.lock() {
                match meta {
                    Meta::Model(model) => named.model = Some(model),
                    Meta::Upstream(upstream) => named.upstream = Some(upstream),
                    _ => {}
                }
            }
        })
        .await
    };
    (Box::pin(generating), incoming, said)
}

/// The model a running generation's door named as its writer, and the
/// upstream when the door is the inference gateway.
type Said = Arc<std::sync::Mutex<Named>>;

/// What a door named about the answer it wrote.
#[derive(Default)]
struct Named {
    model: Option<String>,
    upstream: Option<String>,
}

/// `generation`, answering only once `gate` is open: a model call that
/// finishes while its words are held keeps its reply until they are not.
fn gated(generation: Generation, mut gate: watch::Receiver<bool>) -> Generation {
    Box::pin(async move {
        let answered = generation.await;
        let _ = gate.wait_for(|open| *open).await;
        answered
    })
}

/// What a tidied grounded reply's citations are checked against.
enum Cites {
    /// A `gym.news` reply's items.
    Gym(Vec<router::gym::Item>),
    /// A product reply's passages.
    Product(Box<coder::router::seams::Grounding>),
}

/// What a seam answered, bounded by its budget.
enum SeamOutcome {
    /// A continuation, or `None` when the seam failed or ran late.
    Continued(Option<Continuation>),
    Grounded(Result<Grounding, SeamError>),
    Cli(Result<CliAnswer, SeamError>),
    Gym(Result<router::gym::Grounding, SeamError>),
    Author(Result<AuthorStep, SeamError>),
}

/// A seam call, running.
type SeamCall = Pin<Box<dyn Future<Output = SeamOutcome> + Send>>;

fn continuation(seams: &Seams, ask: Ask) -> SeamCall {
    let personalize = seams.personalize.clone();
    Box::pin(async move {
        let answered = tokio::time::timeout(
            router::seams::PERSONALIZE_BUDGET,
            personalize.continuation(&ask),
        )
        .await;
        SeamOutcome::Continued(match answered {
            Ok(Ok(continued)) => Some(continued),
            Ok(Err(error)) => {
                eprintln!("router personalize: {error}");
                None
            }
            Err(_) => {
                eprintln!("router personalize ran past its budget");
                None
            }
        })
    })
}

fn grounding(seams: &Seams, corpus: router::Corpus, lookup: Lookup) -> SeamCall {
    let product = seams.product.clone();
    let codebase = seams.codebase.clone();
    Box::pin(async move {
        let found = match corpus {
            router::Corpus::Product => {
                tokio::time::timeout(router::seams::KB_BUDGET, product.ground(&lookup)).await
            }
            router::Corpus::Codebase => {
                tokio::time::timeout(router::seams::KB_BUDGET, codebase.ground(&lookup)).await
            }
            // The Gym's records are read through `gym_records`, never as a
            // grounded corpus.
            router::Corpus::Gym => Ok(Err(SeamError::Unavailable)),
        };
        SeamOutcome::Grounded(
            found.unwrap_or_else(|_| Err(SeamError::Failed("ran past its budget".to_string()))),
        )
    })
}

fn gym_records(seams: &Seams, lookup: GymLookup) -> SeamCall {
    let gym = seams.gym.clone();
    Box::pin(async move {
        SeamOutcome::Gym(
            tokio::time::timeout(router::seams::GYM_BUDGET, gym.ground(&lookup))
                .await
                .unwrap_or_else(|_| Err(SeamError::Failed("ran past its budget".to_string()))),
        )
    })
}

fn author_step(seams: &Seams, ask: AuthorAsk) -> SeamCall {
    let author = seams.author.clone();
    Box::pin(async move {
        let stepped = tokio::time::timeout(router::seams::AUTHOR_BUDGET, author.step(&ask))
            .await
            .unwrap_or_else(|_| Err(SeamError::Failed("ran past its budget".to_string())));
        if let Err(SeamError::Failed(why)) = &stepped {
            eprintln!("router author seam failed: {why}");
        }
        SeamOutcome::Author(stepped)
    })
}

fn proposal(seams: &Seams, ask: CliAsk) -> SeamCall {
    let cli = seams.cli.clone();
    Box::pin(async move {
        SeamOutcome::Cli(
            tokio::time::timeout(router::seams::CLI_BUDGET, cli.propose(&ask))
                .await
                .unwrap_or_else(|_| Err(SeamError::Failed("ran past its budget".to_string()))),
        )
    })
}

/// Whether the turn's chat makes a plugin in steps (#10177): a terminal on
/// the computer Coder runs on, whose client runs the steps that happen
/// there (showing the drafted tests, running them, installing and turning
/// the plugin on). Elsewhere `eval.author` is the authoring interview.
fn makes_plugins(context: &router::Context) -> bool {
    context.here() && context.surface() == router::Surface::Terminal
}

/// The exploration dispatch a codebase question escalates to: the
/// no-computer answer when the device has none, else the explore stem.
fn explore(bank: &Bank, facts: &router::Facts, turn: &Turn) -> Option<Tier> {
    // The website offers no Coder run (#10106).
    if turn.context.computer_ready == Some(false) || turn.context.surface() == router::Surface::Web
    {
        return None;
    }
    let entry = bank.entry("dispatch.explore_stem")?;
    let (stem, generic_end) = entry.stem(facts)?;
    Some(Tier::CannedStem {
        answer: entry.clone(),
        stem,
        generic_end,
        offer: entry.offer(),
        personalize: true,
    })
}

/// The result fields for `tier`: its word, route, the bank entry that
/// supplied text, the bank as the `model` when no model wrote any, and the
/// entry's followup chips.
fn served_of(routing: &router::Routing, tier: &Tier, bank: &Bank, facts: &router::Facts) -> Served {
    let answer = tier.answer();
    Served {
        tier: tier.word(),
        route: routing.route.word(),
        answer: answer.map(router::Entry::tag),
        // The admitted capability that answered, when the reading named
        // one at the policy's confidence: a typed id, never text.
        capability: routing
            .capability
            .as_ref()
            .filter(|(_, p)| *p >= router::policy::CAPABILITY_CONFIDENCE)
            .map(|(entry, _)| entry.id.clone()),
        model: answer.map(|_| format!("bank:{}", bank.name)),
        followups: match tier {
            Tier::CannedFinal { answer, .. } | Tier::Capability { answer, .. } => {
                bank.followups(answer, facts)
            }
            _ => Vec::new(),
        },
        // An entry that comes with the plugin cards carries the compiled-in
        // catalog's slugs (docs/web/plugin-card.md): typed ids, never text.
        plugins: answer
            .filter(|entry| entry.plugins)
            .map(|_| {
                coder::gym_kb::catalog_plugins()
                    .into_iter()
                    .map(|plugin| plugin.slug)
                    .collect()
            })
            .unwrap_or_default(),
        ..Served::default()
    }
}

/// The user's latest message: the request's `task`, else the last user
/// turn of the transcript.
fn latest(payload: &Value, input: &[Message]) -> String {
    payload["task"]
        .as_str()
        .map(str::to_string)
        .or_else(|| {
            input
                .iter()
                .rev()
                .find(|message| message.role == Role::User)
                .map(|message| message.text.clone())
        })
        .unwrap_or_default()
}

/// The conversation a request carries, without falling back to its task:
/// a ranking's draft is its own field.
fn transcript_only(payload: &Value) -> Vec<Message> {
    let mut only = payload.clone();
    only["task"] = Value::Null;
    transcript(&only)
}

/// The conversation the request carries, oldest first.
fn transcript(payload: &Value) -> Vec<Message> {
    let mut input = Vec::new();
    if let Some(turns) = payload["transcript"].as_array() {
        for turn in turns {
            let role = match turn["role"].as_str() {
                Some("assistant") => Role::Assistant,
                _ => Role::User,
            };
            input.push(Message {
                role,
                text: turn["content"].as_str().unwrap_or_default().to_string(),
            });
        }
    }
    // A request with no transcript still carries the task.
    if input.is_empty()
        && let Some(task) = payload["task"].as_str()
    {
        input.push(Message {
            role: Role::User,
            text: task.to_string(),
        });
    }
    input
}

fn parse_hex(text: &str) -> Option<[u8; 32]> {
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes[index] = ((high << 4) | low) as u8;
    }
    Some(bytes)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn unix_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// The door that wrote a reply `model` names, for the usage log: `bank`
/// for a prepared answer, the host of the gateway that served the model,
/// or the door's own name.
fn answering_door(door: &Door, model: &str) -> String {
    if model.starts_with("bank:") {
        return "bank".to_string();
    }
    let host = |url: &str| {
        let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
        rest.split(['/', '?', '#'])
            .next()
            .unwrap_or(rest)
            .to_string()
    };
    match door {
        Door::Fallback(ordered) if model == ordered.primary.model => host(&ordered.primary.url),
        Door::Fallback(ordered) => host(&ordered.fallback.url),
        Door::Live(live) => host(&live.url),
        other => other.name().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// #11042: the usage window is 30 days unless set, `forever` keeps
    /// every file, and anything else stops the worker.
    #[test]
    fn the_usage_window_defaults_to_thirty_days() {
        assert_eq!(usage_days_from(None), Ok(Some(30)));
        assert_eq!(usage_days_from(Some(" ")), Ok(Some(30)));
        assert_eq!(usage_days_from(Some("90")), Ok(Some(90)));
        assert_eq!(usage_days_from(Some("forever")), Ok(None));
        assert!(usage_days_from(Some("0")).is_err());
        assert!(usage_days_from(Some("a month")).is_err());
    }
    use coder::generate::StubGenerate;
    use secp256k1::SecretKey;

    /// The model the recorded Gemini stream answers as.
    const GEMINI: &str = coder::generate::Lane::Gemini.model();

    // Exercise the worker's response path in process. The stub door needs
    // no credentials and never makes a model request.
    async fn response(payload: Value, decline: Option<&str>) -> Value {
        response_admitting(payload, decline, None, true).await
    }

    fn identities() -> (Identity, Identity, [u8; 32]) {
        let worker = Identity::from_secret(SecretKey::from_byte_array([41; 32]).unwrap()).unwrap();
        let client = Identity::from_secret(SecretKey::from_byte_array([42; 32]).unwrap()).unwrap();
        let public = XOnlyPublicKey::from_byte_array(parse_hex(client.pubkey()).unwrap()).unwrap();
        let conversation = nip44::conversation_key(worker.secret(), &public);
        (worker, client, conversation)
    }

    async fn response_admitting(
        payload: Value,
        decline: Option<&str>,
        allow: Option<Vec<String>>,
        admitted: bool,
    ) -> Value {
        response_through(
            Door::Stub(StubGenerate::default()),
            payload,
            decline,
            allow,
            admitted,
        )
        .await
    }

    async fn response_through(
        door: Door,
        payload: Value,
        decline: Option<&str>,
        allow: Option<Vec<String>>,
        admitted: bool,
    ) -> Value {
        let (_, _, conversation) = identities();
        let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
        response_to_content(door, content, unix_now(), decline, allow, admitted).await
    }

    async fn response_to_content(
        door: Door,
        content: String,
        created_at: u64,
        decline: Option<&str>,
        allow: Option<Vec<String>>,
        admitted: bool,
    ) -> Value {
        response_metered(door, content, created_at, decline, allow, admitted, None).await
    }

    async fn response_metered(
        door: Door,
        content: String,
        created_at: u64,
        decline: Option<&str>,
        allow: Option<Vec<String>>,
        admitted: bool,
        ledger: Option<Arc<std::sync::Mutex<Ledger>>>,
    ) -> Value {
        response_logged(
            door, content, created_at, decline, allow, admitted, ledger, None,
        )
        .await
    }

    /// [`response_metered`] for a worker that records its jobs in `usage`.
    #[allow(clippy::too_many_arguments)]
    async fn response_logged(
        door: Door,
        content: String,
        created_at: u64,
        decline: Option<&str>,
        allow: Option<Vec<String>>,
        admitted: bool,
        ledger: Option<Arc<std::sync::Mutex<Ledger>>>,
        usage: Option<Arc<coder::relay::usage::Log>>,
    ) -> Value {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            let (worker, client, conversation) = identities();
            let request = client.signer().sign(
                created_at,
                REQUEST_KIND,
                vec![Tag::new(vec!["p".into(), worker.pubkey().to_string()])],
                content,
            );
            let (publish, mut frames) = mpsc::unbounded_channel();
            let slots = Arc::new(Semaphore::new(1));
            let permit = admitted.then(|| slots.clone().try_acquire_owned().unwrap());
            let routing = Arc::new(RouterConfig::new(
                RouterSetting::Live,
                Seams::default(),
                &door,
            ));
            let job = Job {
                routing,
                answered: Default::default(),
                upstream: Default::default(),
                identity: Arc::new(worker),
                door: Arc::new(door),
                judge: None,
                decline: decline.map(str::to_owned),
                allow,
                ledger,
                usage,
                publish,
                permit,
                waits: Waits {
                    grace: Duration::from_millis(200),
                    undelegated: Duration::from_millis(200),
                },
                payer: None,
                payer_last: None,
                payer_refusal: None,
            };
            job.answer(&request).await.unwrap();
            assert_eq!(
                slots.available_permits(),
                1,
                "the job's slot is free once it has answered"
            );
            // The first answer that is not the admission's `processing`
            // acknowledgement, which every admitted turn opens with.
            loop {
                let value = frames.recv().await.unwrap();
                let event: Event = serde_json::from_value(value[1].clone()).unwrap();
                event.validate_crypto().unwrap();
                assert!(event.tag_values("e").any(|id| id == request.id));
                assert!(event.tag_values("p").any(|key| key == client.pubkey()));
                let body: Value =
                    serde_json::from_str(&nip44::decrypt(&event.content, &conversation).unwrap())
                        .unwrap();
                if body["status"] != "processing" {
                    break body;
                }
            }
        })
        .await
        .expect("the local worker response must finish")
    }

    /// A door that never answers: a relay door pointed at a listener that
    /// accepts the connection and then says nothing, with the connect
    /// bound set far past the test.
    fn silent_door() -> (Door, std::net::TcpListener) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let (worker, client, _) = identities();
        let public = XOnlyPublicKey::from_byte_array(parse_hex(worker.pubkey()).unwrap()).unwrap();
        let door =
            coder::relay::RelayDoor::new(url, public, client).connecting(Duration::from_secs(3600));
        (Door::Relay(Box::new(door)), listener)
    }

    #[test]
    fn only_an_open_worker_off_loopback_is_refused_deployment() {
        for url in [
            "ws://127.0.0.1:7777",
            "ws://localhost:7777/",
            "ws://LOCALHOST",
            "ws://[::1]:7777",
            "wss://user@127.0.0.1/path?x=1",
            "127.0.0.1:7777",
        ] {
            assert!(is_loopback(url), "{url}");
            assert!(deployable(None, false, url).is_ok(), "{url}");
        }
        for url in [
            "wss://relay.openagents.com",
            "ws://10.0.0.5:7777",
            "ws://[2001:db8::1]:7777",
            "ws://127.0.0.1.example.com",
            "wss://relay.example/127.0.0.1",
        ] {
            assert!(!is_loopback(url), "{url}");
            let why = deployable(None, false, url).unwrap_err();
            assert!(why.contains(ALLOW_VAR) && why.contains(url), "{why}");
            assert!(why.contains(OPEN_VAR), "{why}");
            assert!(
                deployable(Some(&["ab".to_string()]), false, url).is_ok(),
                "{url}"
            );
            // Opened on purpose, with no quota at all (#10120).
            assert!(deployable(None, true, url).is_ok(), "{url}");
        }
    }

    /// Content the worker cannot read is answered with a typed
    /// `malformed`: the request was signed and addressed to this worker,
    /// so the customer is known and hears what went wrong.
    #[tokio::test]
    async fn unreadable_content_is_refused_malformed() {
        let (_, _, conversation) = identities();
        let stub = || Door::Stub(StubGenerate::default());
        let not_nip44 = "not ciphertext at all".to_string();
        let not_json = nip44::encrypt("{this is not json", &conversation, [43; 32]).unwrap();
        let not_object = nip44::encrypt("[1, 2, 3]", &conversation, [43; 32]).unwrap();
        for (content, why) in [
            (not_nip44, "NIP-44"),
            (not_json, "not JSON"),
            (not_object, "not a JSON object"),
        ] {
            let refused = response_to_content(stub(), content, unix_now(), None, None, true).await;
            assert_eq!(refused["v"], PAYLOAD_VERSION, "{refused}");
            assert_eq!(refused["type"], "status");
            assert_eq!(refused["status"], "error");
            assert_eq!(refused["code"], "malformed", "{refused}");
            assert!(
                refused["message"].as_str().unwrap().contains(why),
                "{refused}"
            );
        }
    }

    /// A request created long before it arrived is a replay, not a job:
    /// refused `stale`, so the customer whose key signed it hears about
    /// it and nothing is generated.
    #[tokio::test]
    async fn a_replayed_old_request_is_refused_stale() {
        let (_, _, conversation) = identities();
        let content = nip44::encrypt(
            &json!({"v": 2, "task": "hello"}).to_string(),
            &conversation,
            [43; 32],
        )
        .unwrap();
        let stub = || Door::Stub(StubGenerate::default());
        let old = unix_now() - REQUEST_WINDOW.as_secs() - 60;
        let refused = response_to_content(stub(), content.clone(), old, None, None, true).await;
        assert_eq!(refused["v"], 2);
        assert_eq!(refused["type"], "status");
        assert_eq!(refused["code"], "stale", "{refused}");
        let fresh = unix_now() - REQUEST_WINDOW.as_secs() + 60;
        let answered = response_to_content(stub(), content, fresh, None, None, true).await;
        assert_eq!(answered["type"], "result", "{answered}");
    }

    /// An event that is not a signed job request naming this worker is
    /// set aside before decryption: nothing proved who sent it, so there
    /// is no one to answer.
    #[test]
    fn events_that_are_not_requests_to_this_worker_are_set_aside() {
        let (worker, client, _) = identities();
        let to_us = Tag::new(vec!["p".into(), worker.pubkey().to_string()]);
        let to_them = Tag::new(vec!["p".into(), client.pubkey().to_string()]);
        let sign = |kind: u16, tags: Vec<Tag>| {
            client
                .signer()
                .sign(unix_now(), kind, tags, "ciphertext".to_string())
        };
        assert!(addressed(&sign(REQUEST_KIND, vec![to_us.clone()]), worker.pubkey()).is_ok());
        assert!(
            addressed(&sign(RESULT_KIND, vec![to_us.clone()]), worker.pubkey())
                .unwrap_err()
                .contains("not a job request")
        );
        assert!(
            addressed(&sign(REQUEST_KIND, vec![to_them]), worker.pubkey())
                .unwrap_err()
                .contains("not addressed")
        );
        let mut forged = sign(REQUEST_KIND, vec![to_us]);
        forged.content = "other ciphertext".to_string();
        assert!(
            addressed(&forged, worker.pubkey())
                .unwrap_err()
                .contains("does not verify")
        );
    }

    /// A delegation whose run outlasts its stated minutes plus the grace
    /// is refused `timed_out` rather than left unanswered, and so is a
    /// job with no minutes that outlasts the worker's own bound.
    #[tokio::test]
    async fn a_job_that_outruns_its_bound_is_refused_timed_out() {
        for (payload, stated) in [
            (
                json!({"v":2,"task":"hello","delegation":{"writes":false,"minutes":0}}),
                "its 0-minute limit",
            ),
            (
                json!({"v":2,"task":"hello"}),
                "this worker's 0-minute limit",
            ),
        ] {
            let (door, _listener) = silent_door();
            let refused = response_through(door, payload, None, None, true).await;
            assert_eq!(refused["type"], "status");
            assert_eq!(refused["status"], "error");
            assert_eq!(refused["code"], "timed_out");
            assert!(
                refused["message"].as_str().unwrap().contains(stated),
                "{refused}"
            );
        }
    }

    #[tokio::test]
    async fn the_worker_preserves_supported_request_versions() {
        for version in [1, 2] {
            let result = response(json!({"v":version,"task":"hello"}), None).await;
            assert_eq!(result["v"], version);
            assert_eq!(result["type"], "result");
            assert!(!result["text"].as_str().unwrap().is_empty());
            let refused =
                response(json!({"v":version,"task":"hello"}), Some("quota_exhausted")).await;
            assert_eq!(refused["v"], version);
            assert_eq!(refused["code"], "quota_exhausted");
        }
    }

    #[tokio::test]
    async fn a_customer_off_the_allowlist_is_refused_with_a_typed_code() {
        let client = Identity::from_secret(SecretKey::from_byte_array([42; 32]).unwrap()).unwrap();
        let stranger =
            Identity::from_secret(SecretKey::from_byte_array([44; 32]).unwrap()).unwrap();
        let payload = json!({"v":2,"task":"hello"});
        let admitted = response_admitting(
            payload.clone(),
            None,
            Some(vec![client.pubkey().to_string()]),
            true,
        )
        .await;
        assert_eq!(admitted["type"], "result");
        let refused = response_admitting(
            payload,
            None,
            Some(vec![stranger.pubkey().to_string()]),
            true,
        )
        .await;
        assert_eq!(refused["type"], "status");
        assert_eq!(refused["code"], "not_admitted");
        assert!(refused.get("text").is_none());
    }

    /// A job that finds every slot taken is refused `busy` before any
    /// generation, and a probe is answered regardless: it is how a
    /// terminal learns the worker is here at all.
    #[tokio::test]
    async fn a_full_worker_refuses_busy_and_still_answers_probes() {
        let busy = response_admitting(json!({"v":2,"task":"hello"}), None, None, false).await;
        assert_eq!(busy["type"], "status");
        assert_eq!(busy["code"], "busy");

        let probed = response_admitting(json!({"v":2,"type":"probe"}), None, None, false).await;
        assert_eq!(probed["type"], "result");
        assert_eq!(probed["probe"]["door"], "stub");
        assert_eq!(probed["probe"]["delegates"], false);
        assert_eq!(probed["model"], Door::Stub(StubGenerate::default()).model());
    }

    /// A writing delegation needs an executor door. A model door answers
    /// a reading one as a prompt and refuses a writing one with a reason,
    /// rather than describing an edit it cannot make.
    #[tokio::test]
    async fn a_model_door_refuses_a_writing_delegation() {
        let reading = response(
            json!({"v":2,"task":"hello","delegation":{"writes":false,"minutes":1}}),
            None,
        )
        .await;
        assert_eq!(reading["type"], "result");

        let writing = response(
            json!({"v":2,"task":"edit a file","delegation":{"writes":true,"minutes":1}}),
            None,
        )
        .await;
        assert_eq!(writing["type"], "status");
        assert_eq!(writing["code"], "internal");
        assert!(
            writing["message"]
                .as_str()
                .unwrap()
                .contains("executor door"),
            "{writing}"
        );
    }

    #[test]
    fn the_allowlist_reads_npub_and_hex_and_refuses_typos() {
        let client = Identity::from_secret(SecretKey::from_byte_array([42; 32]).unwrap()).unwrap();
        let public = XOnlyPublicKey::from_byte_array(parse_hex(client.pubkey()).unwrap()).unwrap();
        let npub = nostr::nip19::encode_npub(&public.serialize());
        let keys = allowed(&format!(" {npub}, {}", client.pubkey())).unwrap();
        assert_eq!(keys, vec![client.pubkey().to_string(); 2]);
        assert!(allowed("").is_err());
        assert!(allowed("not-a-key").is_err());
    }

    #[tokio::test]
    async fn unsupported_requests_refuse_before_generation_or_configured_decline() {
        for payload in [json!({"task":"hello"}), json!({"v":99,"task":"hello"})] {
            for decline in [None, Some("quota_exhausted")] {
                let result = response(payload.clone(), decline).await;
                assert_eq!(result["type"], "status");
                assert_eq!(result["code"], "unsupported_version");
                assert!(result.get("text").is_none());
            }
        }
    }

    /// An open worker answers a caller it has never met with no usage
    /// limit (#10120): many turns in the same minute are all answered, a
    /// delegation is refused, and a key on the allowlist gets everything.
    #[tokio::test]
    async fn an_open_worker_answers_every_caller_with_no_limit() {
        let (_, client, conversation) = identities();
        let ledger = Arc::new(std::sync::Mutex::new(
            Ledger::open(Policy::UNLIMITED, None, unix_now()).unwrap(),
        ));
        let ask = |payload: Value, allow: Option<Vec<String>>| {
            let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
            response_metered(
                Door::Stub(StubGenerate::default()),
                content,
                unix_now(),
                None,
                allow,
                true,
                Some(ledger.clone()),
            )
        };
        let turn = json!({"v":2,"task":"hello"});
        for _ in 0..12 {
            assert_eq!(ask(turn.clone(), None).await["type"], "result");
        }
        // The allowlist names someone else; the open worker still answers.
        let other = vec!["ab".repeat(32)];
        assert_eq!(ask(turn.clone(), Some(other)).await["type"], "result");
        let listed = Some(vec![client.pubkey().to_string()]);
        assert_eq!(ask(turn.clone(), listed).await["type"], "result");
        let delegation = json!({"v":2,"task":"x","delegation":{"writes":false,"minutes":1}});
        assert_eq!(ask(delegation, None).await["code"], "not_admitted");
    }

    /// The emergency brake still works when an operator sets it: a key
    /// past its minute hears `rate_limited` with the wait, and a key on
    /// the allowlist is not counted.
    #[tokio::test]
    async fn an_emergency_brake_meters_callers_off_the_allowlist() {
        let (_, client, conversation) = identities();
        let policy = Policy::parse("minute=2").unwrap();
        let ledger = Arc::new(std::sync::Mutex::new(
            Ledger::open(policy, None, unix_now()).unwrap(),
        ));
        let ask = |payload: Value, allow: Option<Vec<String>>| {
            let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
            response_metered(
                Door::Stub(StubGenerate::default()),
                content,
                unix_now(),
                None,
                allow,
                true,
                Some(ledger.clone()),
            )
        };
        let turn = json!({"v":2,"task":"hello"});
        assert_eq!(ask(turn.clone(), None).await["type"], "result");
        assert_eq!(ask(turn.clone(), None).await["type"], "result");
        let limited = ask(turn.clone(), None).await;
        assert_eq!(limited["code"], "rate_limited");
        assert!(limited["retry_after_ms"].as_u64().unwrap() > 0, "{limited}");
        let listed = Some(vec![client.pubkey().to_string()]);
        assert_eq!(ask(turn.clone(), listed).await["type"], "result");
        assert_eq!(ledger.lock().unwrap().total(), 2);
    }

    /// Every job lands in the usage log as one line, answered or refused,
    /// with the caller's surface and client, and never the message text.
    #[tokio::test]
    async fn every_job_is_recorded_in_the_usage_log_without_its_text() {
        let (_, client, conversation) = identities();
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(coder::relay::usage::Log::new(dir.path().join("usage")));
        let ask = |payload: Value, admitted: bool| {
            let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
            response_logged(
                Door::Stub(StubGenerate::default()),
                content,
                unix_now(),
                None,
                None,
                admitted,
                None,
                Some(log.clone()),
            )
        };
        let turn = json!({"v":2,"task":"a private question","client":"openagents-mobile",
            "context":{"surface":"phone"}});
        assert_eq!(ask(turn.clone(), true).await["type"], "result");
        assert_eq!(ask(turn, false).await["code"], "busy");
        let read = coder::relay::usage::read(log.dir(), None).unwrap();
        assert_eq!(read.records.len(), 2);
        let answered = &read.records[0];
        assert_eq!(answered.key, client.pubkey());
        assert_eq!(answered.surface.as_deref(), Some("phone"));
        assert_eq!(answered.client.as_deref(), Some("openagents-mobile"));
        assert_eq!(answered.kind, "turn");
        assert_eq!(answered.outcome, coder::relay::usage::Outcome::Answered);
        assert!(answered.model.is_some() && answered.door.is_some());
        assert!(answered.bytes_in > 0 && answered.bytes_out > 0);
        assert_eq!(
            read.records[1].outcome,
            coder::relay::usage::Outcome::Refused
        );
        assert_eq!(read.records[1].code.as_deref(), Some("busy"));
        let rows = coder::relay::usage::stats(&read.records, coder::relay::usage::By::Surface);
        assert_eq!((rows[0].group.as_str(), rows[0].jobs), ("phone", 2));
        for entry in std::fs::read_dir(log.dir()).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            assert!(!text.contains("private question"), "{text}");
        }
    }

    /// BYOK (NIP-CJ "Caller-paid model calls"): a job that names a feature
    /// this worker does not serve is refused; a `payer.keys` job whose
    /// envelope does not open, or that this worker cannot run on the
    /// caller's keys, is refused and never answered on ours; and the usage
    /// log and every published body name the payer and never a key.
    #[tokio::test]
    async fn a_payer_job_runs_on_the_callers_keys_or_is_refused() {
        let (_, _, conversation) = identities();
        let dir = tempfile::tempdir().unwrap();
        let log = Arc::new(coder::relay::usage::Log::new(dir.path().join("usage")));
        let key = "sk-or-v1-callers-own-key-0123456789";
        let mut keys = model_access::Keys::none();
        keys.insert(
            model_access::Provider::OpenRouter,
            model_access::ApiKey::new(key),
        );
        let ask = |payload: Value| {
            let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
            response_logged(
                Door::Stub(StubGenerate::default()),
                content,
                unix_now(),
                None,
                None,
                true,
                None,
                Some(log.clone()),
            )
        };
        let unknown = ask(json!({"v":2,"requires":["teleport"],"task":"hi"})).await;
        assert_eq!(unknown["code"], "unsupported_feature");

        let mut sealed = json!({"v":2,"requires":[],"task":"hi"});
        openagents_chat::basic_coder::seal_payer(&mut sealed, &keys, &conversation).unwrap();
        assert_eq!(sealed["requires"], json!(["payer.keys"]));
        assert!(
            !sealed.to_string().contains(key),
            "the body never holds the key"
        );
        // A stub door is not a model door: the job is refused, not answered
        // on this worker's own door.
        let stubbed = ask(sealed).await;
        assert_eq!(stubbed["status"], "error");
        assert_ne!(stubbed["type"], "result");
        assert!(!stubbed.to_string().contains(key));

        let broken =
            ask(json!({"v":2,"requires":["payer.keys"],"task":"hi","payer":{"keys":"nope"}})).await;
        assert_eq!(broken["code"], "malformed");
        let missing = ask(json!({"v":2,"requires":["payer.keys"],"task":"hi"})).await;
        assert_eq!(missing["code"], "malformed");

        for entry in std::fs::read_dir(log.dir()).unwrap() {
            let text = std::fs::read_to_string(entry.unwrap().path()).unwrap();
            assert!(!text.contains(key), "{text}");
        }
    }

    /// The chat door on a caller's keys asks our primary on their
    /// OpenRouter key, then our fallback on their gateway key, at the
    /// providers' own URLs and never at our door.
    #[test]
    fn the_callers_door_is_our_models_on_their_providers() {
        let ours = Door::Fallback(Box::new(FallbackDoor::new(
            coder::generate::ResponsesDoor::new(
                "https://our.door",
                Lane::SpaceBunny.model(),
                "our-key",
            ),
            coder::generate::ResponsesDoor::new("https://our.gateway", GEMINI, "our-key"),
        )));
        let mut keys = model_access::Keys::none();
        keys.insert(
            model_access::Provider::OpenRouter,
            model_access::ApiKey::new("their-or"),
        );
        keys.insert(
            model_access::Provider::Vercel,
            model_access::ApiKey::new("their-gw"),
        );
        let (door, payer, last) = their_door(&ours, &model_access::Access::theirs(keys)).unwrap();
        let Door::Fallback(ordered) = door else {
            panic!("not two doors");
        };
        assert_eq!(ordered.primary.url, "https://openrouter.ai/api");
        assert_eq!(ordered.primary.model, Lane::SpaceBunny.model());
        assert_eq!(ordered.fallback.url, "https://ai-gateway.vercel.sh");
        assert_eq!(ordered.fallback.model, GEMINI);
        assert_eq!(payer.word(), "theirs");
        assert_eq!(last, model_access::Provider::Vercel);
        let mut gateway = model_access::Keys::none();
        gateway.insert(
            model_access::Provider::Vercel,
            model_access::ApiKey::new("their-gw"),
        );
        let (door, _, _) = their_door(&ours, &model_access::Access::theirs(gateway)).unwrap();
        let Door::Live(live) = door else {
            panic!("a gateway key alone is one door");
        };
        assert_eq!(
            live.model, GEMINI,
            "a gateway-only key gets the fallback model"
        );
        assert!(
            their_door(
                &Door::Stub(StubGenerate::default()),
                &model_access::Access::theirs(model_access::Keys::none())
            )
            .is_err()
        );
    }

    /// A product knowledge base that says whose keys it runs on.
    struct Whose(&'static str);

    impl router::seams::ProductKb for Whose {
        fn available(&self) -> bool {
            true
        }
        fn recipients(&self) -> Vec<String> {
            vec![format!("{} (embeddings)", self.0)]
        }
        fn ground<'a>(
            &'a self,
            _: &'a router::seams::Lookup,
        ) -> futures_util::future::BoxFuture<
            'a,
            Result<router::seams::Grounding, router::seams::SeamError>,
        > {
            Box::pin(async { Err(router::seams::SeamError::Unavailable) })
        }
        fn on_their_keys(
            &self,
            theirs: &router::seams::TheirKeys,
        ) -> Option<Arc<dyn router::seams::ProductKb>> {
            theirs
                .embedder()
                .map(|_| Arc::new(Whose("Their door")) as Arc<dyn router::seams::ProductKb>)
        }
    }

    /// A job on the caller's keys keeps its grounded seams: each is lent
    /// their embedder and Jev, never left on ours; without Jev on their
    /// keys the grounded seams are off. The model and privacy answers
    /// name their own key as the door and every recipient as theirs.
    #[test]
    fn a_payer_job_grounds_on_their_keys_and_says_so() {
        let ours = Door::Fallback(Box::new(FallbackDoor::new(
            coder::generate::ResponsesDoor::new(
                coder::generate::OPENROUTER_DOOR_URL,
                Lane::SpaceBunny.model(),
                "our-key",
            ),
            coder::generate::ResponsesDoor::new(
                coder::generate::DEFAULT_DOOR_URL,
                GEMINI,
                "our-key",
            ),
        )));
        let mut keys = model_access::Keys::none();
        keys.insert(
            model_access::Provider::OpenRouter,
            model_access::ApiKey::new("their-or"),
        );
        let access = model_access::Access::theirs(keys);
        let (door, _, _) = their_door(&ours, &access).unwrap();
        let door = Arc::new(door);
        let our_seams = Seams {
            product: Arc::new(Whose("Our door")),
            ..Seams::default()
        };
        let judge = Arc::new(
            jev::Client::new(jev::Config::local("http://127.0.0.1:9", "jev-test")).unwrap(),
        );
        let lent = their_seams(&our_seams, &access, Some(&judge), &door);
        assert_eq!(
            lent.product.recipients(),
            vec!["Their door (embeddings)".to_string()]
        );
        assert!(
            !lent.codebase.available(),
            "a seam we do not hold stays off"
        );
        let without_jev = their_seams(&our_seams, &access, None, &door);
        assert!(
            !without_jev.product.available(),
            "no Jev on their keys: off, never ours"
        );

        let config = RouterConfig::with_news_and_jev(RouterSetting::Live, lent, &door, None, &[])
            .on_their_keys(&door, &["OpenRouter"]);
        let render = |facts: &router::Facts, id: &str| {
            Bank::builtin()
                .entry(id)
                .and_then(|entry| entry.render(facts))
                .unwrap_or_else(|| panic!("{id} renders"))
        };
        let model = render(&config.facts, "meta.model");
        assert!(
            model.starts_with(
                "Our chat runs on Space Bunny Alpha (an anonymous preview model) through OpenRouter \
                 on your own key."
            ),
            "{model}"
        );
        let privacy = render(&config.facts, "meta.privacy");
        assert!(privacy.contains("Their door (embeddings)"), "{privacy}");
        assert!(!privacy.contains("Our door"), "{privacy}");
        assert!(
            privacy.contains(
                "Jev, which chooses how we reply, through OpenRouter, all on your own keys"
            ),
            "{privacy}"
        );
        assert!(!privacy.contains("TypeSafe for Jev"), "{privacy}");
        let fell_back = config.fell_back.as_ref().expect("a fallback's facts");
        assert!(
            render(fell_back, "meta.model").contains("on your own key"),
            "{}",
            render(fell_back, "meta.model")
        );
    }

    /// A request past the quota's byte bound is refused `limit_exceeded`
    /// before anything is generated or counted.
    #[tokio::test]
    async fn a_metered_request_past_its_bytes_is_refused() {
        let (_, _, conversation) = identities();
        let policy = Policy::parse("day=5,minute=2,total=10,bytes=200").unwrap();
        let ledger = Arc::new(std::sync::Mutex::new(
            Ledger::open(policy, None, unix_now()).unwrap(),
        ));
        let payload = json!({"v":2,"task":"x".repeat(400)});
        let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
        let refused = response_metered(
            Door::Stub(StubGenerate::default()),
            content,
            unix_now(),
            None,
            None,
            true,
            Some(ledger.clone()),
        )
        .await;
        assert_eq!(refused["code"], "limit_exceeded");
        assert!(refused.get("retry_after_ms").is_none());
        assert_eq!(ledger.lock().unwrap().total(), 0);
    }

    #[test]
    fn an_open_caller_gets_no_execution() {
        let options = |allow: Option<Vec<String>>, open: bool, quota: Option<Policy>| Options {
            once: false,
            check: false,
            decline: None,
            allow,
            open,
            quota,
        };
        let owner = "ab".repeat(32);
        assert!(execution_admitted(&options(None, false, None), &owner));
        assert!(!execution_admitted(&options(None, true, None), &owner));
        // A brake set alone opens the worker too.
        let brake = Policy::parse("minute=2").ok();
        assert!(!execution_admitted(&options(None, false, brake), &owner));
        assert!(execution_admitted(
            &options(Some(vec![owner.clone()]), true, None),
            &owner
        ));
        assert!(!execution_admitted(
            &options(Some(vec![owner.clone()]), true, None),
            &"cd".repeat(32)
        ));
    }

    /// A one-request HTTP server: it reads one request, waits `delay`, and
    /// answers `body` as `content_type`. Its base URL.
    fn serve_once(delay: Duration, content_type: &'static str, body: String) -> String {
        serve_times(1, delay, content_type, body)
    }

    /// Answers `times` requests, each on its own thread, each after `delay`.
    fn serve_times(
        times: usize,
        delay: Duration,
        content_type: &'static str,
        body: String,
    ) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for _ in 0..times {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let body = body.clone();
                std::thread::spawn(move || answer_once(stream, delay, content_type, &body));
            }
        });
        url
    }

    /// [`serve_times`], keeping each request's JSON body.
    fn serve_recorded(
        times: usize,
        delay: Duration,
        content_type: &'static str,
        body: String,
    ) -> (String, Arc<std::sync::Mutex<Vec<Value>>>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
        let keep = seen.clone();
        std::thread::spawn(move || {
            for _ in 0..times {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                let (body, keep) = (body.clone(), keep.clone());
                std::thread::spawn(move || {
                    let request = answer_once(stream, delay, content_type, &body);
                    if let Ok(value) = serde_json::from_slice::<Value>(&request) {
                        keep.lock().unwrap().push(value);
                    }
                });
            }
        });
        (url, seen)
    }

    fn answer_once(
        stream: std::net::TcpStream,
        delay: Duration,
        content_type: &str,
        body: &str,
    ) -> Vec<u8> {
        use std::io::{BufRead, BufReader, Read, Write};
        let mut reader = BufReader::new(stream);
        let mut length = 0usize;
        loop {
            let mut line = String::new();
            if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            let lower = line.to_ascii_lowercase();
            if let Some(value) = lower.strip_prefix("content-length:") {
                length = value.trim().parse().unwrap_or(0);
            }
        }
        let mut request = vec![0; length];
        let _ = reader.read_exact(&mut request);
        std::thread::sleep(delay);
        let mut stream = reader.into_inner();
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\ncontent-type: {content_type}\r\ncontent-length: {}\r\n\
             connection: close\r\n\r\n{body}",
            body.len()
        );
        request
    }

    /// A model door that starts answering after `delay`, from the recorded
    /// Gemini stream.
    fn slow_door(delay: Duration) -> Door {
        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
        let url = serve_once(delay, "text/event-stream", stream.to_string());
        Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"))
    }

    /// A judge on loopback that answers `answers` after `delay`.
    fn judge(delay: Duration, answers: Value) -> Arc<jev::Client> {
        let body = json!({ "model": "jev-test", "answers": answers }).to_string();
        let url = serve_once(delay, "application/json", body);
        Arc::new(jev::Client::new(jev::Config::local(url, "jev-test")).unwrap())
    }

    /// A choice answer that is sure of `choice`.
    fn sure(choice: &str, options: &[&str]) -> Value {
        let probabilities: serde_json::Map<String, Value> = options
            .iter()
            .map(|option| ((*option).to_string(), json!(f64::from(*option == choice))))
            .collect();
        json!({ "type": "choice", "choice": choice, "confidence": 1.0, "probabilities": probabilities })
    }

    /// The first-response answers: respond, a computer task, no prepared
    /// answer, `opener`.
    fn triaged(opener: &str) -> Value {
        judged("none", 0.9, opener)
    }

    /// The router's answers: respond, the prepared `answer` and its route
    /// (the answer's first route, or `general` for `none`), the specifics
    /// probability, and `opener`; the lane is computer when no answer fits.
    fn judged(answer: &str, specifics: f64, opener: &str) -> Value {
        let bank = Bank::builtin();
        let route = bank
            .entry(answer)
            .map_or("general", |entry| entry.routes[0].as_str());
        routed(route, answer, specifics, opener)
    }

    /// The router's answers with the route named.
    fn routed(route: &str, answer: &str, specifics: f64, opener: &str) -> Value {
        routed_in(&Value::Null, route, answer, specifics, opener)
    }

    /// [`routed`] for a turn with `context`, whose place and slots decide
    /// which answers the question offers.
    fn routed_in(
        context: &Value,
        route: &str,
        answer: &str,
        specifics: f64,
        opener: &str,
    ) -> Value {
        let bank = Bank::builtin();
        let openers: Vec<&str> = bank
            .openers
            .iter()
            .map(|opener| opener.id.as_str())
            .chain(["none"])
            .collect();
        // The loopback door is no gateway, so the answers that need one
        // are not offered, and the judge answers only the rest.
        let facts = router::Context::of(context).facts(&router::worker_facts(
            GEMINI,
            None,
            &Seams::default(),
        ));
        let answers: Vec<&str> = bank
            .answers
            .iter()
            .filter(|entry| entry.selectable(&facts))
            .map(|entry| entry.id.as_str())
            .chain(["none"])
            .collect();
        let routes: Vec<&str> = router::RouteId::ALL
            .iter()
            .map(|route| route.word())
            .chain(["none"])
            .collect();
        let mut answers = json!({
            "action": sure("respond", &["respond", "clarify", "end_conversation", "none"]),
            "route": sure(route, &routes),
            "lane": sure(if answer == "none" { "computer" } else { "chat" }, &["chat", "computer", "none"]),
            "answer": sure(answer, &answers),
            "needs_specifics": { "type": "noul", "noul": specifics },
            "opener": sure(opener, &openers),
            "capability": capability("not-a-capability-request", &[]),
            "risk": sure("ok", &["ok", "secret_shared", "asks_for_secret", "harmful", "money_movement", "none"]),
            "engine": engine("none"),
            "fanout": fanout("one"),
            "read_only": { "type": "noul", "noul": 0.0 },
            "summarize": { "type": "noul", "noul": 0.0 },
        });
        // A desktop turn also asks which deck (#10058).
        if router::Context::of(context).surface() == router::Surface::Desktop {
            let decks: Vec<&str> = router::decks()
                .iter()
                .map(|deck| deck.id)
                .chain(["none"])
                .collect();
            answers["deck"] = sure("none", &decks);
        }
        answers
    }

    /// The `engine` answer over the closed engine list and `none`, sure
    /// of `choice` (#10076).
    fn engine(choice: &str) -> Value {
        let options: Vec<&str> = router::CodingEngine::ALL
            .iter()
            .map(|engine| engine.word())
            .chain(["none"])
            .collect();
        sure(choice, &options)
    }

    /// The `fanout` answer over its closed options, sure of `choice`
    /// (#10183).
    fn fanout(choice: &str) -> Value {
        let options: Vec<&str> = ["one"]
            .into_iter()
            .chain(router::Fanout::ALL.iter().map(|fanout| fanout.word()))
            .collect();
        sure(choice, &options)
    }

    /// The `capability` answer over the built-in admitted set, `extra`
    /// catalog ids, `none`, and `not-a-capability-request`, sure of
    /// `choice`.
    fn capability(choice: &str, extra: &[&str]) -> Value {
        let admitted = router::Admitted::builtin();
        let options: Vec<&str> = admitted
            .entries
            .iter()
            .map(|entry| entry.id.as_str())
            .chain(extra.iter().copied())
            .chain(["none", "not-a-capability-request"])
            .collect();
        sure(choice, &options)
    }

    /// Every answer to one turn, in publication order, up to the result
    /// or a refusal, and how long each took from the request.
    async fn frames_through(
        door: Door,
        judge: Option<Arc<jev::Client>>,
        payload: Value,
    ) -> Vec<(Duration, Value)> {
        frames_routed(door, judge, payload, Seams::default(), RouterSetting::Live).await
    }

    /// [`frames_through`] with the router's seams and setting.
    async fn frames_routed(
        door: Door,
        judge: Option<Arc<jev::Client>>,
        payload: Value,
        seams: Seams,
        setting: RouterSetting,
    ) -> Vec<(Duration, Value)> {
        let (worker, client, conversation) = identities();
        let content = nip44::encrypt(&payload.to_string(), &conversation, [43; 32]).unwrap();
        let request = client.signer().sign(
            unix_now(),
            REQUEST_KIND,
            vec![Tag::new(vec!["p".into(), worker.pubkey().to_string()])],
            content,
        );
        let (publish, mut frames) = mpsc::unbounded_channel();
        let slots = Arc::new(Semaphore::new(1));
        let routing = Arc::new(RouterConfig::new(setting, seams, &door));
        let job = Job {
            identity: Arc::new(worker),
            door: Arc::new(door),
            judge,
            answered: Default::default(),
            upstream: Default::default(),
            decline: None,
            allow: None,
            ledger: None,
            usage: None,
            publish,
            permit: slots.clone().try_acquire_owned().ok(),
            waits: WAITS,
            routing,
            payer: None,
            payer_last: None,
            payer_refusal: None,
        };
        let started = Instant::now();
        tokio::spawn(async move { job.answer(&request).await });
        let mut out = Vec::new();
        tokio::time::timeout(Duration::from_secs(10), async {
            while let Some(value) = frames.recv().await {
                let event: Event = serde_json::from_value(value[1].clone()).unwrap();
                let body: Value =
                    serde_json::from_str(&nip44::decrypt(&event.content, &conversation).unwrap())
                        .unwrap();
                let end = body["type"] == "result" || body["status"] == "error";
                out.push((started.elapsed(), body));
                if end {
                    break;
                }
            }
        })
        .await
        .expect("the turn ends");
        out
    }

    /// A turn that asks for the first response, as the app's does.
    fn turn(task: &str) -> Value {
        json!({
            "v": 2, "requires": [], "task": task,
            "transcript": [{ "role": "user", "content": task }],
            "opener": true,
        })
    }

    /// A caller that does not ask gets no judgment and no opener, even
    /// from a worker with a judge: Microcoder's cloud steps must read back
    /// the model's JSON exactly as it wrote it.
    #[tokio::test]
    async fn a_turn_that_does_not_ask_is_the_models_alone() {
        let mut payload = turn("{\"next\": \"answer\"}");
        payload.as_object_mut().unwrap().remove("opener");
        let frames = frames_through(
            slow_door(Duration::from_millis(300)),
            Some(judge(Duration::ZERO, triaged("explain"))),
            payload,
        )
        .await;
        assert!(frames.iter().all(|(_, body)| body["type"] != "judgment"));
        assert!(
            frames
                .iter()
                .all(|(_, body)| body["delta"] != "We'll look that up for you.\n\n")
        );
        let result = &frames.last().unwrap().1;
        assert_eq!(result["type"], "result");
        assert!(!result["text"].as_str().unwrap().starts_with("Here's how"));
    }

    /// The judge answers in its own time, before the model: the caller
    /// hears `processing`, then the typed judgment naming the opener it
    /// chose, then the model's words. The opener is never written into the
    /// reply: the partials and the result start with the answer (#10139).
    #[tokio::test]
    async fn the_judge_answers_first_and_the_reply_starts_with_its_answer() {
        let frames = frames_through(
            slow_door(Duration::from_millis(600)),
            Some(judge(Duration::ZERO, triaged("explain"))),
            turn("How do Nostr relays work?"),
        )
        .await;
        let bodies: Vec<&Value> = frames.iter().map(|(_, body)| body).collect();
        assert_eq!(bodies[0]["status"], "processing");
        assert_eq!(bodies[1]["type"], "judgment");
        assert_eq!(bodies[1]["verdict"], "respond");
        assert_eq!(bodies[1]["lane"], "computer");
        assert_eq!(bodies[1]["opener"], "explain");
        assert_eq!(bodies[1]["tier"], "opener");
        let opener = "We'll look that up for you.";
        assert!(
            bodies
                .iter()
                .filter(|body| body["type"] == "partial")
                .all(|body| !body["delta"].as_str().unwrap().contains(opener))
        );
        if let Some(model) = bodies.iter().find(|body| body["type"] == "partial") {
            assert_eq!(model["seq"], 0);
        }
        let result = bodies.last().unwrap();
        assert_eq!(result["type"], "result");
        let text = result["text"].as_str().unwrap();
        assert!(!text.is_empty() && !text.contains(opener), "{text}");
        assert_eq!(result["model"], GEMINI);
    }

    /// A sure prepared answer is the whole reply in the judge's time: one
    /// partial with all of it, then the result with the same text, named
    /// as the bank's rather than the model's, and no model words at all.
    #[tokio::test]
    async fn a_sure_prepared_answer_is_the_whole_reply() {
        let frames = frames_through(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, judged("meta.who", 0.05, "none"))),
            turn("Who are you?"),
        )
        .await;
        let bodies: Vec<&Value> = frames.iter().map(|(_, body)| body).collect();
        assert_eq!(bodies[0]["status"], "processing");
        assert_eq!(bodies[1]["type"], "judgment");
        assert_eq!(bodies[1]["tier"], "canned");
        assert_eq!(bodies[1]["answer"], "meta.who@2");
        assert!(bodies[1]["opener"].is_null());
        assert_eq!(bodies[2]["type"], "partial");
        assert_eq!(bodies[2]["seq"], 0);
        let text = bodies[2]["delta"].as_str().unwrap();
        assert!(text.starts_with("We are OpenAgents."), "{text}");
        assert_eq!(bodies.len(), 4, "{bodies:?}");
        let (at, result) = frames.last().unwrap();
        assert_eq!(result["type"], "result");
        assert_eq!(result["text"], text);
        assert_eq!(result["model"], "bank:chat-answers-v1");
        assert_eq!(result["answer"], "meta.who@2");
        assert_eq!(result["tier"], "canned");
        // The reply is done long before the model would have begun.
        assert!(*at < Duration::from_millis(1_000), "{at:?}");
    }

    /// Below every threshold nothing is shown before the model: the
    /// judgment goes out, and the reply is the model's, unprefixed.
    #[tokio::test]
    async fn an_unsure_judgment_shows_nothing_before_the_model() {
        let mut answers = judged("meta.capabilities", 0.8, "explain");
        answers["opener"]["confidence"] = json!(0.4);
        let frames = frames_through(
            slow_door(Duration::from_millis(300)),
            Some(judge(Duration::ZERO, answers)),
            turn("Can you work on my Rails app?"),
        )
        .await;
        let bodies: Vec<&Value> = frames.iter().map(|(_, body)| body).collect();
        assert_eq!(bodies[1]["type"], "judgment");
        assert_eq!(bodies[1]["tier"], "model");
        assert!(bodies[1]["opener"].is_null());
        let result = bodies.last().unwrap();
        assert_eq!(result["model"], GEMINI);
        assert!(result["answer"].is_null());
        // Any partial is the model's own start of the result.
        let text = result["text"].as_str().unwrap();
        if let Some(partial) = bodies.iter().find(|body| body["type"] == "partial") {
            assert!(text.starts_with(partial["delta"].as_str().unwrap()));
        }
        assert!(!text.starts_with("Here's how") && !text.starts_with("In this chat"));
    }

    /// A caller that asks for the judgment alone gets it, and its reply is
    /// the model's alone.
    #[tokio::test]
    async fn a_caller_can_decline_the_opener_and_keep_the_judgment() {
        let mut payload = turn("thanks!");
        payload["opener"] = json!(false);
        payload["judge"] = json!(true);
        let frames = frames_through(
            slow_door(Duration::from_millis(300)),
            Some(judge(
                Duration::ZERO,
                judged("smalltalk.thanks", 0.05, "none"),
            )),
            payload,
        )
        .await;
        let bodies: Vec<&Value> = frames.iter().map(|(_, body)| body).collect();
        assert!(bodies.iter().any(|body| body["type"] == "judgment"));
        assert!(bodies.iter().all(|body| {
            body["type"] != "partial"
                || !body["delta"]
                    .as_str()
                    .unwrap_or_default()
                    .starts_with("You're welcome!")
        }));
        assert!(
            !bodies.last().unwrap()["text"]
                .as_str()
                .unwrap()
                .starts_with("You're welcome!")
        );
    }

    /// A judge slower than the model still routes the turn: the model's
    /// words wait for the judgment, which serves the bank's whole answer
    /// (#10110).
    #[tokio::test]
    async fn a_judge_slower_than_the_model_still_routes() {
        let frames = frames_through(
            slow_door(Duration::ZERO),
            Some(judge(
                Duration::from_millis(1_200),
                judged("meta.who", 0.05, "none"),
            )),
            turn("Who are you?"),
        )
        .await;
        let bodies: Vec<&Value> = frames.iter().map(|(_, body)| body).collect();
        assert_eq!(bodies[1]["type"], "judgment");
        assert_eq!(bodies[1]["tier"], "canned");
        // No word of the model's went out before the judgment.
        assert_eq!(bodies[2]["type"], "partial");
        assert_eq!(bodies[2]["seq"], 0);
        let (at, result) = frames.last().unwrap();
        assert_eq!(result["model"], "bank:chat-answers-v1");
        assert!(
            result["text"]
                .as_str()
                .unwrap()
                .starts_with("We are OpenAgents."),
            "{result}"
        );
        assert!(*at < first::BUDGET, "{at:?}");
    }

    /// A judgment past the first budget still routes the turn: the bank's
    /// progress line shows at the budget as partial `seq` 0, the routed
    /// reply follows it (#10110), and the result, which replaces the
    /// partials, is the reply alone (#10139).
    #[tokio::test]
    async fn a_judge_past_the_first_budget_still_routes_under_a_progress_line() {
        let frames = frames_through(
            slow_door(Duration::ZERO),
            Some(judge(
                first::BUDGET + Duration::from_millis(800),
                judged("meta.who", 0.05, "none"),
            )),
            turn("Who are you?"),
        )
        .await;
        let bodies: Vec<&Value> = frames.iter().map(|(_, body)| body).collect();
        let line = format!(
            "{}\n\n",
            Bank::builtin().opener(first::PROGRESS_OPENER).unwrap().text
        );
        assert_eq!(bodies[1]["type"], "partial");
        assert_eq!(bodies[1]["seq"], 0);
        assert_eq!(bodies[1]["delta"], line.as_str());
        assert!(frames[1].0 >= first::BUDGET, "{:?}", frames[1].0);
        assert_eq!(bodies[2]["type"], "judgment");
        assert_eq!(bodies[2]["tier"], "canned");
        assert_eq!(bodies[3]["type"], "partial");
        assert_eq!(bodies[3]["seq"], 1);
        let answer = bodies[3]["delta"].as_str().unwrap();
        assert!(answer.starts_with("We are OpenAgents."), "{answer}");
        let result = bodies.last().unwrap();
        assert_eq!(result["model"], "bank:chat-answers-v1");
        assert_eq!(result["text"], answer);
    }

    /// A judge past the second bound leaves the reply the model's, shown
    /// under the progress line while pending, and the model was told what OpenAgents' own
    /// products are (#10110).
    #[tokio::test]
    async fn a_judge_past_the_late_bound_leaves_the_model_under_the_unrouted_note() {
        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
        let (url, seen) = serve_recorded(1, Duration::ZERO, "text/event-stream", stream.into());
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let frames = frames_through(
            door,
            Some(judge(
                first::LATE + Duration::from_millis(2_000),
                judged("meta.who", 0.05, "none"),
            )),
            turn("What's new in the Gym?"),
        )
        .await;
        let line = format!(
            "{}\n\n",
            Bank::builtin().opener(first::PROGRESS_OPENER).unwrap().text
        );
        let (at, result) = frames.last().unwrap();
        assert!(*at >= first::LATE, "{at:?}");
        assert_eq!(result["model"], GEMINI);
        let text = result["text"].as_str().unwrap();
        assert!(!text.is_empty() && !text.starts_with(&line), "{text}");
        assert!(
            frames
                .iter()
                .any(|(_, body)| body["type"] == "partial" && body["delta"] == line.as_str())
        );
        assert!(frames.iter().all(|(_, body)| body["type"] != "judgment"));
        let instructions = seen.lock().unwrap()[0]["instructions"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(
            instructions.ends_with(first::UNROUTED_NOTE),
            "{instructions}"
        );
    }

    /// A turn that does not ask for a first response gets no unrouted
    /// note: its instructions reach the model as they came.
    #[tokio::test]
    async fn only_a_turn_that_asks_to_be_shown_gets_the_unrouted_note() {
        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
        let (url, seen) = serve_recorded(1, Duration::ZERO, "text/event-stream", stream.into());
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let mut payload = turn("{\"next\": \"answer\"}");
        payload.as_object_mut().unwrap().remove("opener");
        payload["instructions"] = json!("Reply with JSON.");
        let frames = frames_through(door, None, payload).await;
        assert_eq!(frames.last().unwrap().1["type"], "result");
        assert_eq!(seen.lock().unwrap()[0]["instructions"], "Reply with JSON.");
    }

    /// #10183: a fan-out read in a terminal is one `run_coder` offer with
    /// the plan, and the reply says what starts, with no continuation.
    #[tokio::test]
    async fn a_fan_out_offer_carries_the_plan_and_says_what_starts() {
        let context = json!({
            "surface": "terminal",
            "computer_ready": true,
            "computer": {"place": "here", "engines": [
                {"engine": "codex", "state": "ready"},
                {"engine": "claude", "state": "ready"},
                {"engine": "grok", "state": "ready"},
            ]},
        });
        let mut answers = routed_in(&context, "work.dispatch", "dispatch.stem", 0.9, "none");
        answers["fanout"] = fanout("each_engine");
        answers["read_only"] = json!({"type": "noul", "noul": 0.95});
        answers["summarize"] = json!({"type": "noul", "noul": 0.9});
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn(
                "do 3 readonly delegations, 1 per agent, explore repo and summarize briefly",
                context,
            ),
            personalized(" look through the repo."),
            RouterSetting::Live,
        )
        .await;
        let offers = of_type(&frames, "offer");
        assert_eq!(offers.len(), 1, "{offers:?}");
        assert_eq!(offers[0]["offer"], "run_coder");
        assert_eq!(
            offers[0]["runs"],
            json!(["codex", "claude_code", "grok_build"])
        );
        assert_eq!(offers[0]["read_only"], true);
        assert_eq!(offers[0]["summarize"], true);
        let result = &frames.last().unwrap().1;
        assert_eq!(
            result["text"],
            "Exploring the repo with Codex, Claude Code, and Grok Build."
        );
        assert!(
            result["answer"]
                .as_str()
                .unwrap()
                .starts_with("dispatch.fan_out@")
        );
    }

    /// #10183: a request carrying a plan's ended runs is the model's
    /// combined summary of them: no judgment or offer, the reports in its
    /// instructions, and a transcript that ends with the person's request.
    #[tokio::test]
    async fn a_plans_results_get_the_models_combined_summary() {
        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
        let (url, seen) = serve_recorded(1, Duration::ZERO, "text/event-stream", stream.into());
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let ask = "do 3 readonly delegations, 1 per agent, explore repo and summarize briefly";
        let mut payload = turn(ask);
        payload["transcript"] = json!([
            {"role": "user", "content": ask},
            {"role": "assistant", "content": "Exploring the repo with Codex, Claude Code, and Grok Build."},
        ]);
        payload["router"] = json!("chat-router-v2");
        payload["context"] = json!({
            "surface": "terminal",
            "runs": [
                {"ending": "finished", "turn": 1, "engine": "codex", "summary": "A Rust workspace of 80 crates.", "files": [], "commands": []},
                {"ending": "finished", "turn": 1, "engine": "claude", "summary": "Rust monorepo; apps under bins/.", "files": [], "commands": []},
                {"ending": "failed", "turn": 1, "engine": "grok", "summary": "Not signed in.", "files": [], "commands": []},
            ],
        });
        let judging = judge(
            Duration::ZERO,
            routed("work.dispatch", "dispatch.stem", 0.9, "none"),
        );
        let frames = frames_through(door, Some(judging), payload).await;
        assert!(frames.iter().all(|(_, body)| body["type"] != "judgment"));
        assert!(frames.iter().all(|(_, body)| body["type"] != "offer"));
        assert_eq!(frames.last().unwrap().1["type"], "result");
        let request = seen.lock().unwrap()[0].clone();
        let instructions = request["instructions"].as_str().unwrap();
        assert!(instructions.contains("combined summary"), "{instructions}");
        assert!(
            instructions.contains("A Rust workspace of 80 crates."),
            "{instructions}"
        );
        assert!(instructions.contains("Grok Build failed"), "{instructions}");
        let input = request["input"].as_array().unwrap();
        assert_eq!(input.last().unwrap()["role"], "user", "{input:?}");
    }

    /// A `rank` job answers the caller's candidates, most likely first,
    /// without generating; with no judge it is refused `unavailable`.
    #[tokio::test]
    async fn a_rank_job_orders_the_candidates() {
        let rank = json!({
            "v": 2, "requires": [], "type": "rank", "draft": "",
            "transcript": [{ "role": "user", "content": "the relay drops sockets" }],
            "candidates": [
                { "id": "openagents", "label": "OpenAgentsInc/openagents" },
                { "id": "psionic", "label": "OpenAgentsInc/psionic" },
            ],
        });
        let answers = json!({ "next": {
            "type": "choice", "choice": "openagents", "confidence": 0.7,
            "probabilities": { "openagents": 0.7, "psionic": 0.2, "none": 0.1 }
        }});
        let frames = frames_through(
            Door::Stub(StubGenerate::default()),
            Some(judge(Duration::ZERO, answers)),
            rank.clone(),
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["type"], "result");
        assert_eq!(result["text"], "openagents");
        assert_eq!(result["ranked"][0]["id"], "openagents");
        assert_eq!(result["ranked"][1]["id"], "psionic");
        assert_eq!(result["set"], first::SET);
        assert!(frames.iter().all(|(_, body)| body["type"] != "partial"));

        let refused = frames_through(Door::Stub(StubGenerate::default()), None, rank).await;
        assert_eq!(refused.last().unwrap().1["code"], "unavailable");

        let mut bad = turn("x");
        bad["type"] = json!("rank");
        bad["candidates"] = json!([{ "id": "none" }]);
        let refused = frames_through(
            Door::Stub(StubGenerate::default()),
            Some(judge(Duration::ZERO, json!({}))),
            bad,
        )
        .await;
        assert_eq!(refused.last().unwrap().1["code"], "malformed");
    }

    // ---------------------------------------------------------------------
    // The chat router (`"router": "chat-router-v1"`)
    // ---------------------------------------------------------------------

    /// A turn that asks for the router, as build 19 of the app does.
    fn routed_turn(task: &str, context: Value) -> Value {
        json!({
            "v": 2, "requires": [], "task": task,
            "transcript": [{ "role": "user", "content": task }],
            "opener": true, "router": "chat-router-v1", "context": context,
        })
    }

    fn of_type<'a>(frames: &'a [(Duration, Value)], kind: &str) -> Vec<&'a Value> {
        frames
            .iter()
            .map(|(_, body)| body)
            .filter(|body| body["type"] == kind)
            .collect()
    }

    /// A personalization seam that answers `text` as `model`.
    struct Writes(&'static str);

    impl router::seams::Personalize for Writes {
        fn available(&self) -> bool {
            true
        }
        fn recipients(&self) -> Vec<String> {
            vec!["a test provider".into()]
        }
        fn continuation<'a>(
            &'a self,
            ask: &'a Ask,
        ) -> futures_util::future::BoxFuture<'a, Result<Continuation, SeamError>> {
            // The seam sees the stem and a redacted message, nothing more.
            assert!(!ask.message.contains("nsec1"), "{}", ask.message);
            Box::pin(async move {
                Ok(Continuation {
                    text: self.0.to_string(),
                    model: "test/cheap".into(),
                })
            })
        }
    }

    fn personalized(text: &'static str) -> Seams {
        Seams {
            personalize: Arc::new(Writes(text)),
            ..Seams::default()
        }
    }

    /// A request for code work gets the dispatch sentence at once, closed
    /// by the validated continuation, and a Run Coder offer; the model call
    /// is dropped, so the result arrives long before the model's words.
    #[tokio::test]
    async fn a_routed_work_request_is_offered_to_coder_and_the_model_is_dropped() {
        let answers = routed("work.dispatch", "dispatch.stem", 0.9, "none");
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn(
                "fix the flaky relay test, my key is nsec1qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqq",
                json!({ "surface": "phone", "computer_ready": true }),
            ),
            personalized(" fixing the flaky relay test."),
            RouterSetting::Live,
        )
        .await;
        let judgment = of_type(&frames, "judgment")[0];
        // Build 20 names `chat-router-v1`; it is routed with the v3 set,
        // named with its digest.
        assert_eq!(judgment["set"], router::set_id());
        assert_eq!(judgment["route"], "work.dispatch");
        assert_eq!(judgment["tier"], "offer");
        let partials = of_type(&frames, "partial");
        assert_eq!(partials[0]["seq"], 0);
        assert_eq!(partials[0]["delta"], "Working on");
        assert_eq!(partials[1]["seq"], 1);
        assert_eq!(partials[1]["delta"], " fixing the flaky relay test.");
        assert_eq!(partials.len(), 2);
        let offers = of_type(&frames, "offer");
        assert_eq!(offers[0]["offer"], "run_coder");
        assert_eq!(offers[0]["target"], "connected_computer");
        let (at, result) = frames.last().unwrap();
        assert_eq!(result["text"], "Working on fixing the flaky relay test.");
        assert_eq!(result["tier"], "offer");
        assert_eq!(result["answer"], "dispatch.stem@3");
        assert_eq!(result["model"], "test/cheap");
        assert_eq!(result["route"], "work.dispatch");
        assert!(
            result["bank"]
                .as_str()
                .unwrap()
                .starts_with("chat-answers-v1@")
        );
        assert!(*at < Duration::from_millis(1_000), "{at:?}");
    }

    /// The owner's "Do a test delegation to claude" (#10076): the typed
    /// `engine` reading names Claude Code, so the offer carries it as
    /// NIP-CJ's `engine` and the stem says it was asked for; with no
    /// engine read, the offer carries none.
    #[tokio::test]
    async fn a_dispatch_offer_names_the_engine_the_person_asked_for() {
        let mut answers = routed("work.dispatch", "dispatch.stem", 0.9, "none");
        answers["engine"] = engine("claude_code");
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn(
                "Do a test delegation to claude",
                json!({ "surface": "phone", "computer_ready": true }),
            ),
            personalized(" take this on."),
            RouterSetting::Live,
        )
        .await;
        let offers = of_type(&frames, "offer");
        assert_eq!(offers[0]["offer"], "run_coder");
        assert_eq!(offers[0]["engine"], "claude_code");
        let partials = of_type(&frames, "partial");
        assert_eq!(partials[0]["delta"], "Starting Claude Code on");
        let result = &frames.last().unwrap().1;
        assert!(
            result["answer"]
                .as_str()
                .unwrap()
                .starts_with("dispatch.engine_stem@")
        );
        assert_eq!(result["text"], "Starting Claude Code on this.");
        let plain = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed("work.dispatch", "dispatch.stem", 0.9, "none"),
            )),
            routed_turn("delegate this", json!({ "computer_ready": true })),
            personalized(" take this on."),
            RouterSetting::Live,
        )
        .await;
        let offers = of_type(&plain, "offer");
        assert_eq!(offers[0]["offer"], "run_coder");
        assert!(offers[0].get("engine").is_none(), "{}", offers[0]);
    }

    /// A continuation that breaks the rules is never shown: the stem's
    /// generic end closes the sentence, and the bank is the `model`.
    #[tokio::test]
    async fn an_invalid_continuation_falls_back_to_the_generic_end() {
        let answers = routed("work.dispatch", "dispatch.stem", 0.9, "none");
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn("fix it", json!({})),
            personalized(" fix it. I already fixed it at https://example.com"),
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["text"], "Working on this.");
        assert_eq!(result["model"], "bank:chat-answers-v1");
    }

    /// With no computer ready, the answer says so and offers the computers
    /// screen; nothing is dispatched.
    #[tokio::test]
    async fn with_no_computer_the_offer_is_to_connect_one() {
        let answers = routed("work.dispatch", "dispatch.stem", 0.9, "none");
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn("look through my repo", json!({ "computer_ready": false })),
            Seams::default(),
            RouterSetting::Live,
        )
        .await;
        let offers = of_type(&frames, "offer");
        assert_eq!(offers[0]["offer"], "open_screen");
        assert_eq!(offers[0]["screen"], "account.computers");
        let result = &frames.last().unwrap().1;
        assert_eq!(result["answer"], "dispatch.no_computer@2");
        assert!(
            result["text"]
                .as_str()
                .unwrap()
                .starts_with("That needs a computer.")
        );
    }

    /// TypeSafe unreachable, the Vercel AI Gateway answers: the judgment
    /// is served as any other and names the door that answered and the
    /// model it served, so the thread's decision record can (#10064).
    #[tokio::test]
    async fn a_fallback_doors_judgment_names_that_door() {
        let answers = routed("work.dispatch", "dispatch.stem", 0.9, "none");
        let gateway = serve_once(
            Duration::ZERO,
            "application/json",
            json!({ "model": "typesafe-ai/jev", "answers": answers }).to_string(),
        );
        // A port nothing listens on: TypeSafe does not answer.
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let typesafe = format!("http://{}", closed.local_addr().unwrap());
        drop(closed);
        let failover = jev::doors::Failover::new(
            jev::doors::Door::new(
                jev::doors::TYPESAFE_DOOR,
                typesafe,
                jev::doors::Naming::Canonical,
                jev::ApiKey::new("ts-test"),
            ),
            vec![jev::doors::Door::new(
                jev::doors::GATEWAY_DOOR,
                format!("{gateway}/typesafe/v1/systemone"),
                jev::doors::Naming::Gateway,
                jev::ApiKey::new("vck-test"),
            )],
        );
        let judge = Arc::new(
            jev::Client::new(
                jev::Config::new()
                    .exchange(jev::doors::exchange(failover))
                    .base_url(jev::doors::TYPESAFE_DOOR)
                    .default_model("jev-1.13.0"),
            )
            .unwrap(),
        );
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge),
            routed_turn(
                "fix the flaky relay test",
                json!({ "surface": "phone", "computer_ready": true }),
            ),
            personalized(" fixing the flaky relay test."),
            RouterSetting::Live,
        )
        .await;
        let judgment = of_type(&frames, "judgment")[0];
        assert_eq!(judgment["route"], "work.dispatch");
        assert_eq!(judgment["door"], jev::doors::GATEWAY_DOOR);
        assert_eq!(judgment["model"], "typesafe-ai/jev");
    }

    /// A request that calls for a capability none of the admitted ones
    /// covers (#9960): the bank's line naming the closest admitted
    /// capability from the typed set, the `capability` card with how to
    /// add one (the Gym, since no interview is wired), the Gym offer, and
    /// the model call dropped. Nothing on the wire is the message.
    #[tokio::test]
    async fn a_missing_capability_gets_the_bank_line_the_card_and_the_gym_offer() {
        let mut answers = routed("capability.missing", "none", 0.9, "none");
        let mut none = capability("none", &[]);
        none["confidence"] = json!(0.7);
        none["probabilities"]["none"] = json!(0.7);
        none["probabilities"]["chat.coder"] = json!(0.25);
        none["probabilities"]["not-a-capability-request"] = json!(0.05);
        answers["capability"] = none;
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            v2_turn("Book me a flight to Denver next Friday"),
            Seams::default(),
            RouterSetting::Live,
        )
        .await;
        let judgment = of_type(&frames, "judgment")[0];
        assert_eq!(judgment["set"], router::set_id());
        assert_eq!(judgment["route"], "capability.missing");
        assert_eq!(judgment["capability"], Value::Null);
        assert_eq!(judgment["capability_missing_p"], 0.7);
        let cards = of_type(&frames, "card");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["card"], "capability");
        assert_eq!(cards[0]["status"], "missing");
        assert_eq!(cards[0]["closest"]["name"], "Coder");
        assert_eq!(cards[0]["closest"]["reach"], "chat");
        assert_eq!(cards[0]["add"], "gym");
        nostr::cj_conversation::parse_card(cards[0]).expect("a card NIP-CJ reads");
        let offers = of_type(&frames, "offer");
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0]["offer"], "open_screen");
        assert_eq!(offers[0]["screen"], "verse.gym");
        let result = &frames.last().unwrap().1;
        let text = result["text"].as_str().unwrap();
        assert!(
            text.starts_with("There's no plugin for that yet. The closest thing we have is Coder:"),
            "{text}"
        );
        assert_eq!(result["tier"], "canned");
        assert_eq!(result["answer"], "capability.missing_near@2");
        assert!(result["model"].as_str().unwrap().starts_with("bank:"));
        assert_eq!(result["capability"], Value::Null);
        for (_, body) in &frames {
            assert!(!body.to_string().contains("Denver"), "{body}");
        }
        assert!(
            frames.last().unwrap().0 < Duration::from_millis(1_400),
            "the model call was dropped"
        );

        // A named Coder-run capability on a work request is the stem that
        // names it, and the result names the capability.
        let tools = gym_records().records.tools;
        let mut work = routed("work.dispatch", "dispatch.stem", 0.9, "none");
        work["tool"] = sure(
            "openagents.tool-project-map",
            &[
                "openagents.tool-project-map",
                "openagents.tool-code-finder",
                "none",
            ],
        );
        work["capability"] = capability(
            "openagents.tool-project-map",
            &["openagents.tool-project-map", "openagents.tool-code-finder"],
        );
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, work)),
            v2_turn("use Project map on my repo before you refactor the auth module"),
            Seams {
                gym: Arc::new(Gym(router::gym::Grounding {
                    records: router::gym::Records {
                        tools,
                        ..router::gym::Records::default()
                    },
                    news: Vec::new(),
                })),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        // No computer is connected in `v2_turn`: the no-computer answer.
        assert_eq!(result["answer"], "dispatch.no_computer@2");
        assert_eq!(result["capability"], "openagents.tool-project-map");
    }

    /// A canned answer carries its followup chips, only for entries the
    /// worker can fill.
    #[tokio::test]
    async fn a_routed_canned_answer_carries_followup_chips() {
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, judged("meta.who", 0.05, "none"))),
            routed_turn("who are you", json!({})),
            Seams::default(),
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["tier"], "canned");
        // meta.model needs the gateway, which this test worker lacks;
        // meta.pricing needs nothing since it names no quota (#10120).
        assert_eq!(
            result["followups"],
            json!([
                { "id": "meta.capabilities", "label": "What can you do?" },
                { "id": "meta.pricing", "label": "What does it cost?" }
            ])
        );
    }

    /// A message holding a secret is answered with the bank's refusal,
    /// never model text.
    #[tokio::test]
    async fn a_shared_secret_gets_the_bank_refusal() {
        let mut answers = judged("none", 0.2, "none");
        answers["risk"] = sure(
            "secret_shared",
            &[
                "ok",
                "secret_shared",
                "asks_for_secret",
                "harmful",
                "money_movement",
                "none",
            ],
        );
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn("here are my words: abandon abandon ...", json!({})),
            personalized(" never shown."),
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["tier"], "refuse");
        assert_eq!(result["answer"], "refuse.secret_shared@1");
        assert!(of_type(&frames, "offer").is_empty());
    }

    /// In shadow mode the router's decision is logged and named, but the
    /// turn is served as the first response always served it, less the
    /// opener line (#10139).
    #[tokio::test]
    async fn shadow_mode_serves_the_legacy_tier_and_names_the_routed_one() {
        let answers = routed("work.dispatch", "dispatch.stem", 0.9, "plan");
        let frames = frames_routed(
            slow_door(Duration::from_millis(300)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn("fix it", json!({})),
            Seams::default(),
            RouterSetting::Shadow,
        )
        .await;
        let judgment = of_type(&frames, "judgment")[0];
        assert_eq!(judgment["tier"], "opener");
        assert_eq!(judgment["shadow"], "offer");
        assert!(of_type(&frames, "offer").is_empty());
        let result = &frames.last().unwrap().1;
        assert_eq!(result["model"], GEMINI);
        // The opener is never written into the reply (#10139).
        let text = result["text"].as_str().unwrap();
        assert!(
            !text.is_empty() && !text.contains("Here's a plan."),
            "{text}"
        );
    }

    /// A phone from before the router, asking only for `opener`, never
    /// gets an offer or a stem, even for a wallet or dispatch reading.
    #[tokio::test]
    async fn an_opener_only_request_gets_no_offers() {
        let frames = frames_through(
            slow_door(Duration::from_millis(300)),
            Some(judge(
                Duration::ZERO,
                routed("work.dispatch", "dispatch.stem", 0.9, "none"),
            )),
            turn("fix it"),
        )
        .await;
        assert!(of_type(&frames, "offer").is_empty());
        assert_eq!(of_type(&frames, "judgment")[0]["tier"], "model");
        assert_eq!(frames.last().unwrap().1["model"], GEMINI);
    }

    /// A product knowledge seam for tests.
    struct Knows(Grounding);

    impl router::seams::ProductKb for Knows {
        fn available(&self) -> bool {
            true
        }
        fn recipients(&self) -> Vec<String> {
            vec!["an embedding provider".into()]
        }
        fn ground<'a>(
            &'a self,
            _: &'a Lookup,
        ) -> futures_util::future::BoxFuture<'a, Result<Grounding, SeamError>> {
            Box::pin(async move { Ok(self.0.clone()) })
        }
    }

    fn passage(relevance: f64, answer: Option<&str>) -> router::seams::Passage {
        router::seams::Passage {
            id: "product.connect@1".into(),
            title: "Connect a computer".into(),
            text: "Install the Coder host and link it from the app.".into(),
            source: "docs/coder/runtime/host-service.md".into(),
            relevance,
            answer: answer.map(str::to_string),
            off_computer: false,
            in_app: false,
        }
    }

    /// A product entry's own reviewed answer is served whole; otherwise the
    /// model is restarted with the passages and the result cites them.
    #[tokio::test]
    async fn product_questions_are_grounded_in_the_knowledge_base() {
        let answers = routed("product.kb", "none", 0.1, "none");
        let whole = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers.clone())),
            routed_turn("how do I connect my Mac", json!({})),
            Seams {
                product: Arc::new(Knows(Grounding {
                    passages: vec![passage(
                        0.9,
                        Some("We connect computers from your account."),
                    )],
                    ..Grounding::default()
                })),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let result = &whole.last().unwrap().1;
        assert_eq!(result["text"], "We connect computers from your account.");
        assert_eq!(result["model"], "kb:product");
        assert_eq!(result["citations"][0]["id"], "product.connect@1");

        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
        let url = serve_times(
            2,
            Duration::from_millis(200),
            "text/event-stream",
            stream.to_string(),
        );
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let grounded = frames_routed(
            door,
            Some(judge(Duration::ZERO, answers)),
            routed_turn("how do I connect my Mac", json!({})),
            Seams {
                product: Arc::new(Knows(Grounding {
                    passages: vec![passage(0.7, None), passage(0.2, None)],
                    ..Grounding::default()
                })),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let result = &grounded.last().unwrap().1;
        assert_eq!(result["type"], "result", "{result}");
        assert_eq!(result["tier"], "grounded");
        assert_eq!(result["model"], GEMINI);
        assert_eq!(result["citations"].as_array().unwrap().len(), 1);
    }

    /// A product knowledge seam that answers after a wait.
    struct KnowsLate(Grounding, Duration);

    impl router::seams::ProductKb for KnowsLate {
        fn available(&self) -> bool {
            true
        }
        fn recipients(&self) -> Vec<String> {
            vec!["an embedding provider".into()]
        }
        fn ground<'a>(
            &'a self,
            _: &'a Lookup,
        ) -> futures_util::future::BoxFuture<'a, Result<Grounding, SeamError>> {
            Box::pin(async move {
                tokio::time::sleep(self.1).await;
                Ok(self.0.clone())
            })
        }
    }

    /// A model faster than the retrieval does not answer a knowledge
    /// question before its knowledge arrives (#10109): the words of the
    /// call started with the turn wait for the seam, and the reply is the
    /// one restarted with the passages. The primary's first words come in
    /// about a second, before a retrieval does.
    #[tokio::test]
    async fn a_fast_model_waits_for_the_knowledge_it_is_grounded_on() {
        let answers = routed("product.kb", "none", 0.1, "none");
        // The call started with the turn answers after the judgment and
        // before the retrieval, lowercase; the grounded restart answers
        // capitalized.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            let bodies = [
                (
                    Duration::from_millis(150),
                    include_str!("../../fixtures/gateway/stealth-space-bunny-alpha.sse"),
                ),
                (
                    Duration::ZERO,
                    include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse"),
                ),
            ];
            for (delay, body) in bodies {
                let Ok((stream, _)) = listener.accept() else {
                    return;
                };
                std::thread::spawn(move || answer_once(stream, delay, "text/event-stream", body));
            }
        });
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let frames = frames_routed(
            door,
            Some(judge(Duration::ZERO, answers)),
            routed_turn("how do I connect my Mac", json!({})),
            Seams {
                product: Arc::new(KnowsLate(
                    Grounding {
                        passages: vec![passage(0.7, None)],
                        ..Grounding::default()
                    },
                    Duration::from_millis(600),
                )),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["type"], "result", "{result}");
        assert_eq!(result["tier"], "grounded");
        assert_eq!(result["text"], "One\nTwo\nThree\nFour\nFive");
        assert!(
            frames
                .iter()
                .all(|(_, body)| !body["delta"].as_str().unwrap_or("").contains("one")),
            "the ungrounded words never reached the caller: {frames:?}"
        );
    }

    /// A grounded product reply's `[openagents.…]` citations are read for
    /// the log and taken out of every streamed piece and the final text,
    /// even when the stream splits a citation in two.
    #[tokio::test]
    async fn product_citations_never_reach_the_phone() {
        let answers = routed("product.kb", "none", 0.1, "none");
        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse")
            .replace(
                r"One\nTwo\nThree\nFour\nFive",
                r"Scan the code [openagents.connect-computer@1].\n[openagents.connect-computer@1]\nNo Tailscale.",
            )
            .replace(r#""delta":"One\n""#, r#""delta":"Scan the code [openagents.conn""#)
            .replace(
                r#""delta":"Two\nThree\nFour\nFive""#,
                r#""delta":"ect-computer@1].\n[openagents.connect-computer@1]\nNo Tailscale.""#,
            );
        let url = serve_times(2, Duration::from_millis(200), "text/event-stream", stream);
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let mut cited = passage(0.7, None);
        cited.id = "openagents.connect-computer@1".into();
        let frames = frames_routed(
            door,
            Some(judge(Duration::ZERO, answers)),
            routed_turn("how do I connect a phone", json!({})),
            Seams {
                product: Arc::new(Knows(Grounding {
                    passages: vec![cited],
                    ..Grounding::default()
                })),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["tier"], "grounded", "{result}");
        assert_eq!(result["text"], "Scan the code.\nNo Tailscale.");
        let streamed: String = of_type(&frames, "partial")
            .iter()
            .map(|partial| partial["delta"].as_str().unwrap_or_default().to_string())
            .collect();
        // Whatever streamed before the reply ended (the split itself is
        // `product_kb`'s streaming test) carries no citation.
        assert!(!streamed.contains("[openagents."), "{streamed:?}");
    }

    /// A CLI seam for tests: proposes `argv` with `effect`.
    struct Proposes(Vec<&'static str>, router::Effect);

    impl router::seams::CliRoute for Proposes {
        fn groups(&self) -> Vec<router::seams::CliGroup> {
            vec![router::seams::CliGroup {
                id: "computer".into(),
                summary: "Your computers".into(),
                tree: None,
            }]
        }
        fn recipients(&self) -> Vec<String> {
            Vec::new()
        }
        fn propose<'a>(
            &'a self,
            _: &'a CliAsk,
        ) -> futures_util::future::BoxFuture<'a, Result<CliAnswer, SeamError>> {
            Box::pin(async move {
                Ok(CliAnswer::Proposal(router::seams::CliProposal {
                    argv: self.0.iter().map(|arg| (*arg).to_string()).collect(),
                    effect: self.1,
                    runs_on: router::RunsOn::ThisDevice,
                }))
            })
        }
    }

    /// A read-only command is offered with a confirm; a spending one never
    /// is, and the wallet screen is offered instead.
    #[tokio::test]
    async fn cli_proposals_pass_the_gate_or_are_not_offered() {
        let mut answers = routed("cli", "none", 0.5, "none");
        answers["cli_group"] = sure("computer", &["computer", "none"]);
        let listed = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers.clone())),
            routed_turn(
                "which of my computers are online",
                json!({ "surface": "phone" }),
            ),
            Seams {
                cli: Arc::new(Proposes(vec!["computer", "list"], router::Effect::ReadOnly)),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let offer = of_type(&listed, "offer")[0];
        assert_eq!(offer["offer"], "cli");
        assert_eq!(offer["argv"], json!(["computer", "list"]));
        assert_eq!(offer["confirm"], true);
        assert_eq!(listed.last().unwrap().1["answer"], "cli.offer@1");

        let paying = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn("pay that invoice", json!({ "surface": "phone" })),
            Seams {
                cli: Arc::new(Proposes(vec!["wallet", "pay"], router::Effect::Spends)),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let offers = of_type(&paying, "offer");
        assert_eq!(offers.len(), 1);
        assert_eq!(offers[0]["offer"], "open_screen");
        assert_eq!(offers[0]["screen"], "wallet");
        assert_eq!(paying.last().unwrap().1["answer"], "wallet.send@1");
    }

    /// A Gym seam for tests: fixed records, and fixed news.
    struct Gym(router::gym::Grounding);

    impl router::seams::GymKb for Gym {
        fn available(&self) -> bool {
            true
        }
        fn recipients(&self) -> Vec<String> {
            vec!["an embedding provider".into()]
        }
        fn tools(&self) -> Vec<router::gym::Tool> {
            self.0.records.tools.clone()
        }
        fn ground<'a>(
            &'a self,
            _: &'a GymLookup,
        ) -> futures_util::future::BoxFuture<'a, Result<router::gym::Grounding, SeamError>>
        {
            Box::pin(async move { Ok(self.0.clone()) })
        }
    }

    /// The Gym's test records: Project map with a published test set and a
    /// Better result, Code finder with neither, and one build.
    fn gym_records() -> router::gym::Grounding {
        use router::gym::*;
        let event = |n: u8, kind: u16| EventPointer {
            id: format!("{n:02x}").repeat(32),
            pubkey: format!("{:02x}", n + 100).repeat(32),
            kind,
        };
        let artifact = |n: u8, schema: Option<&str>| ArtifactRef {
            digest: format!("sha256:{}", format!("{n:02x}").repeat(32)),
            size: 4096,
            media_type: "application/json".into(),
            schema: schema.map(str::to_string),
            event: None,
            sources: Vec::new(),
        };
        let tool = |id: &str, name: &str| Tool {
            id: format!("openagents.tool-{id}"),
            name: name.into(),
            line: format!("What {name} does."),
            source: format!("knowledge/openagents/openagents.tool-{id}.md"),
            slugs: vec![id.into()],
        };
        let subject = DefinitionRef {
            id: format!("{}:project-map/map", "ab".repeat(32)),
            artifact: artifact(1, None),
            event: None,
        };
        router::gym::Grounding {
            records: Records {
                tools: vec![
                    tool("project-map", "Project map"),
                    tool("code-finder", "Code finder"),
                ],
                results: vec![ResultRecord {
                    publication: event(10, 3189),
                    tool: Some("openagents.tool-project-map".into()),
                    tool_name: "Project map".into(),
                    trainer: "3e".repeat(32),
                    suite: event(20, 3184),
                    cases: 8,
                    subject: subject.clone(),
                    headline: Headline {
                        subject_passed: 7,
                        baseline_passed: Some(5),
                        total: 8,
                    },
                    verdict: Verdict::Pass,
                    report: artifact(2, Some(nostr::kb::REPORT_SCHEMA)),
                    checks: None,
                    checked: Checks {
                        confirmed: 1,
                        disputed: 0,
                    },
                    current: true,
                    at: 1_790_000_010,
                }],
                suites: vec![SuiteRecord {
                    release: event(20, 3184),
                    tool: Some("openagents.tool-project-map".into()),
                    tool_name: "Project map".into(),
                    author: "50".repeat(32),
                    subject,
                    cases: 8,
                    at: 1_790_000_000,
                    source: event(20, 3184),
                }],
                adoptions: Vec::new(),
                releases: vec![Release {
                    version: "1.0.0".into(),
                    build: "20".into(),
                    title: "Smarter chat and a simpler Wallet".into(),
                    items: vec!["Instant answers".into()],
                    source: "crates/openagents-mobile/src/account.rs".into(),
                }],
                notes: Vec::new(),
            },
            news: Vec::new(),
        }
    }

    /// The router's answers with the route and the `tool` reading.
    fn routed_tool(route: &str, tool: &str) -> Value {
        let mut answers = routed(route, "none", 0.5, "none");
        answers["tool"] = sure(
            tool,
            &[
                "openagents.tool-project-map",
                "openagents.tool-code-finder",
                "none",
            ],
        );
        answers["capability"] = capability(
            "not-a-capability-request",
            &["openagents.tool-project-map", "openagents.tool-code-finder"],
        );
        answers
    }

    fn v2_turn(task: &str) -> Value {
        json!({
            "v": 2, "requires": [], "task": task,
            "transcript": [{ "role": "user", "content": task }],
            "opener": true, "router": "chat-router-v2",
            "context": { "surface": "phone", "computer_ready": false },
        })
    }

    /// `eval.run`: the bank's sentence names the tool and its tests from
    /// the records, the tool card and the `start_eval` offer ride as
    /// feedback, and the model call is dropped.
    #[tokio::test]
    async fn a_tool_test_is_offered_from_the_records_with_its_card() {
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed_tool("eval.run", "openagents.tool-project-map"),
            )),
            v2_turn("Test Project map on Coder"),
            Seams {
                gym: Arc::new(Gym(gym_records())),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let judgment = of_type(&frames, "judgment")[0];
        assert_eq!(judgment["set"], router::set_id());
        assert_eq!(judgment["route"], "eval.run");
        assert_eq!(judgment["tool"], "openagents.tool-project-map");
        let cards = of_type(&frames, "card");
        assert_eq!(cards.len(), 1);
        assert_eq!(cards[0]["card"], "tool");
        assert_eq!(
            cards[0]["latest"]["headline"],
            json!({ "subject_passed": 7, "baseline_passed": 5, "total": 8 })
        );
        let offers = of_type(&frames, "offer");
        assert_eq!(offers[0]["offer"], "start_eval");
        assert_eq!(
            offers[0]["size"],
            json!({ "cases": 8, "runs": 3, "arms": 2 })
        );
        assert_eq!(offers[0]["where"], "hosted");
        let result = &frames.last().unwrap().1;
        assert!(
            result["text"]
                .as_str()
                .unwrap()
                .starts_with("We'd test Project map with its published test set: 8 tests"),
            "{result}"
        );
        assert_eq!(result["tier"], "gym");
        assert_eq!(result["answer"], "eval.run.offer@1");
        assert!(result["model"].as_str().unwrap().starts_with("bank:"));
        assert!(
            frames.last().unwrap().0 < Duration::from_millis(1_400),
            "the model call was dropped"
        );
    }

    /// `gym.news`: the bank's lead line and the news card from the kept
    /// items go out before the model's first words (#9950); the model is
    /// restarted on the news lane (its reasoning off) told to answer from
    /// the items under that line; the result cites them and names the news
    /// model.
    #[tokio::test]
    async fn news_is_grounded_in_the_gyms_records() {
        let mut grounding = gym_records();
        grounding.news = grounding
            .records
            .items()
            .into_iter()
            .map(|item| (item, 0.9))
            .collect();
        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
        let (url, seen) = serve_recorded(
            2,
            Duration::from_millis(200),
            "text/event-stream",
            stream.to_string(),
        );
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let frames = frames_routed(
            door,
            Some(judge(Duration::ZERO, routed_tool("gym.news", "none"))),
            v2_turn("What's new in the Gym?"),
            Seams {
                gym: Arc::new(Gym(grounding)),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let lead = Bank::builtin()
            .entry(router::gym::NEWS_LEAD)
            .and_then(|entry| entry.render(&router::Facts::default()))
            .expect("the bank has the news lead");
        let partials = of_type(&frames, "partial");
        assert_eq!(partials[0]["delta"], format!("{lead}\n\n"));
        let cards = of_type(&frames, "card");
        assert_eq!(cards[0]["card"], "news");
        assert_eq!(cards[0]["items"].as_array().unwrap().len(), 3);
        nostr::cj_conversation::parse_card(cards[0]).expect("a card NIP-CJ reads");
        // The lead and the card go out before any of the model's words.
        let position = |wanted: &Value| frames.iter().position(|(_, body)| body == wanted);
        assert!(position(partials[0]) < position(cards[0]));
        if let Some(words) = partials.get(1) {
            assert!(position(cards[0]) < position(words));
        }
        let result = &frames.last().unwrap().1;
        assert_eq!(result["tier"], "gym");
        assert_eq!(result["route"], "gym.news");
        assert_eq!(result["citations"].as_array().unwrap().len(), 3);
        assert_eq!(result["model"], router::gym::NEWS_MODEL);
        let text = result["text"].as_str().unwrap();
        assert!(
            text.starts_with(&lead) && text.len() > lead.len() + 2,
            "{text}"
        );
        // The reply is the news lane's call, with its reasoning off, told
        // the lead is already shown.
        let seen = seen.lock().unwrap();
        let news: Vec<&Value> = seen
            .iter()
            .filter(|body| body["model"] == router::gym::NEWS_MODEL)
            .collect();
        assert_eq!(news.len(), 1, "{seen:?}");
        assert_eq!(news[0]["reasoning"]["effort"], "none");
        assert_eq!(news[0]["max_output_tokens"], router::gym::NEWS_MAX_TOKENS);
        assert!(news[0]["instructions"].as_str().unwrap().contains(&lead));
    }

    /// A Space Bunny Alpha primary on loopback in front of `fallback`:
    /// answering from the recorded OpenRouter stream when `answers`, and
    /// refusing every connection otherwise, as a model that is gone does.
    fn primary_before(answers: bool, fallback: Door) -> Door {
        let url = if answers {
            let stream = include_str!("../../fixtures/gateway/stealth-space-bunny-alpha.sse");
            serve_once(Duration::ZERO, "text/event-stream", stream.to_string())
        } else {
            // A port that was bound and let go: nothing listens there.
            let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}", closed.local_addr().unwrap());
            drop(closed);
            url
        };
        let Door::Live(fallback) = fallback else {
            panic!("a gateway door");
        };
        Door::Fallback(Box::new(
            coder::generate::FallbackDoor::new(
                coder::generate::ResponsesDoor::new(url, Lane::SpaceBunny.model(), "test"),
                fallback,
            )
            .first_word(Duration::from_secs(2), Duration::from_secs(4)),
        ))
    }

    /// The result names the model that wrote the reply: the primary when
    /// it answered, the fallback when the primary missed the turn
    /// (#10109).
    #[tokio::test]
    async fn the_result_names_the_door_that_answered() {
        let mut payload = turn("How do Nostr relays work?");
        payload.as_object_mut().unwrap().remove("opener");
        for (answers, model, text) in [
            (
                true,
                Lane::SpaceBunny.model(),
                "one\ntwo\nthree\nfour\nfive",
            ),
            (false, GEMINI, "One\nTwo\nThree\nFour\nFive"),
        ] {
            let door = primary_before(answers, slow_door(Duration::ZERO));
            let frames = frames_through(door, None, payload.clone()).await;
            let result = &frames.last().unwrap().1;
            assert_eq!(result["type"], "result", "{frames:?}");
            assert_eq!(result["model"], model);
            assert_eq!(result["text"], text);
        }
    }

    /// The worker puts the primary in front of a gateway door by default
    /// when OpenRouter's key is here, never in front of a door that picks
    /// its own model, and refuses a primary named with no way to reach it.
    #[test]
    fn the_primary_goes_in_front_of_a_gateway_door() {
        let gateway = || {
            Door::Live(coder::generate::ResponsesDoor::new(
                coder::generate::DEFAULT_DOOR_URL,
                GEMINI,
                "test",
            ))
        };
        let ordered_door = ordered(gateway(), None, Some("key")).unwrap();
        let Door::Fallback(both) = &ordered_door else {
            panic!("the primary is in front");
        };
        assert_eq!(both.primary.model, Lane::SpaceBunny.model());
        assert_eq!(both.primary.url, coder::generate::OPENROUTER_DOOR_URL);
        assert_eq!(both.fallback.model, GEMINI);
        assert!(matches!(
            ordered(gateway(), Some("off"), Some("key")).unwrap(),
            Door::Live(_)
        ));
        assert!(matches!(
            ordered(gateway(), None, None).unwrap(),
            Door::Live(_)
        ));
        let stub = || Door::Stub(StubGenerate::default());
        assert!(matches!(
            ordered(stub(), None, Some("key")).unwrap(),
            Door::Stub(_)
        ));
        assert!(ordered(stub(), Some("space-bunny"), Some("key")).is_err());
        assert!(ordered(gateway(), Some("space-bunny"), None).is_err());
    }

    /// "What models does this use?" names no single model: a router picks
    /// the best fit per message across many (2026-10-09). The privacy
    /// answer names both doors and what the
    /// primary's provider may keep, whichever is answering (#10109).
    #[test]
    fn the_model_answer_names_no_single_model() {
        let door = Door::Fallback(Box::new(coder::generate::FallbackDoor::openrouter(
            Lane::SpaceBunny.model(),
            "test",
            coder::generate::ResponsesDoor::new(coder::generate::DEFAULT_DOOR_URL, GEMINI, "test"),
        )));
        let config = RouterConfig::new(RouterSetting::Live, Seams::default(), &door);
        let render = |facts: &router::Facts, id: &str| {
            Bank::builtin()
                .entry(id)
                .and_then(|entry| entry.render(facts))
                .unwrap_or_else(|| panic!("{id} renders"))
        };
        let fell_back = config.fell_back.as_ref().expect("a fallback's facts");
        for facts in [&config.facts, fell_back] {
            let model = render(facts, "meta.model");
            assert!(model.starts_with("There isn't one model."), "{model}");
            assert!(!model.contains("Gemini") && !model.contains("OpenRouter"), "{model}");
        }
        for facts in [&config.facts, fell_back] {
            for id in ["meta.privacy", "meta.data_retention"] {
                let said = render(facts, id);
                assert!(
                    said.contains(
                        "OpenRouter for Space Bunny Alpha (an anonymous preview model), the Vercel AI \
                         Gateway for Google's Gemini 3.8 Flash when Space Bunny Alpha can't answer"
                    ),
                    "{said}"
                );
                assert!(
                    said.contains(&format!(
                        "{}.",
                        coder::first::keeps_sentence(coder::generate::ProviderPrivacy::from_env())
                    )),
                    "{said}"
                );
            }
        }
    }

    /// Jev's fallback doors are named in the privacy answer only when
    /// they are on (#10064).
    #[test]
    fn the_privacy_answer_names_jevs_fallback_doors_when_they_are_on() {
        let door = Door::Live(coder::generate::ResponsesDoor::new(
            coder::generate::DEFAULT_DOOR_URL,
            GEMINI,
            "test",
        ));
        let privacy = |fallbacks: &[&str]| {
            let config = RouterConfig::with_news_and_jev(
                RouterSetting::Live,
                Seams::default(),
                &door,
                None,
                fallbacks,
            );
            Bank::builtin()
                .entry("meta.privacy")
                .and_then(|entry| entry.render(&config.facts))
                .unwrap()
        };
        let off = privacy(&[]);
        assert!(off.contains("TypeSafe for Jev"), "{off}");
        assert!(!off.contains("OpenRouter"), "{off}");
        let on = privacy(&["the Vercel AI Gateway", "OpenRouter"]);
        assert!(
            on.contains("through the Vercel AI Gateway or OpenRouter"),
            "{on}"
        );
    }

    /// `CODER_GYM_NEWS_MODEL=off` keeps Gym news on the chat door; the
    /// privacy answer names the news model only when it is used.
    #[test]
    fn the_news_lane_is_named_where_it_is_used() {
        let seams = || Seams {
            gym: Arc::new(Gym(gym_records())),
            ..Seams::default()
        };
        let door = Door::Live(coder::generate::ResponsesDoor::new(
            coder::generate::DEFAULT_DOOR_URL,
            GEMINI,
            "test",
        ));
        let on = RouterConfig::new(RouterSetting::Live, seams(), &door);
        assert_eq!(
            on.news.as_ref().map(|news| news.model()),
            Some(router::gym::NEWS_MODEL)
        );
        let recipients = |config: &RouterConfig| {
            Bank::builtin()
                .entry("meta.privacy")
                .and_then(|entry| entry.render(&config.facts))
                .unwrap()
        };
        assert!(
            recipients(&on).contains(router::gym::NEWS_MODEL_NAME),
            "{}",
            recipients(&on)
        );
        let off = RouterConfig::with_news(RouterSetting::Live, seams(), &door, None);
        assert!(off.news.is_none());
        assert!(!recipients(&off).contains(router::gym::NEWS_MODEL_NAME));
        let no_gym = RouterConfig::new(RouterSetting::Live, Seams::default(), &door);
        assert!(no_gym.news.is_none());
    }

    /// With no Gym records configured, an `eval.check` turn says no result
    /// is waiting, from the bank, and states no number.
    #[tokio::test]
    async fn without_records_a_check_turn_says_nothing_is_waiting() {
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed("eval.check", "none", 0.5, "none"),
            )),
            v2_turn("Is there a result I can check?"),
            Seams::default(),
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["answer"], "eval.check.none@1");
        assert!(of_type(&frames, "card").is_empty());
        assert!(of_type(&frames, "offer").is_empty());
    }

    /// An interview step for tests: fixed text, a draft, and an offer.
    struct Interviews;

    impl router::seams::EvalAuthor for Interviews {
        fn available(&self) -> bool {
            true
        }
        fn recipients(&self) -> Vec<String> {
            Vec::new()
        }
        fn step<'a>(
            &'a self,
            ask: &'a AuthorAsk,
        ) -> futures_util::future::BoxFuture<'a, Result<AuthorStep, SeamError>> {
            let seen = ask.draft.clone();
            Box::pin(async move {
                use nostr::cj_conversation::{Size, SubjectSource, SuiteSource, Where};
                Ok(AuthorStep {
                    text: if seen.is_some() {
                        "Good. Should we try it once?".into()
                    } else {
                        "What should a good run look like?".into()
                    },
                    draft: seen,
                    offer: Some(router::Offer::StartEval {
                        suite: SuiteSource::Draft,
                        subject: SubjectSource::Draft,
                        size: Size {
                            cases: 1,
                            runs: 1,
                            arms: 2,
                        },
                        at: Where::Hosted,
                        label: "Try it once".into(),
                    }),
                    model: "interview-test".into(),
                    plugin: None,
                })
            })
        }
    }

    /// A plugin step for tests (#10177): the step the transcript's last
    /// fixed line opened, or the draft.
    struct PluginSeam;

    impl router::seams::EvalAuthor for PluginSeam {
        fn available(&self) -> bool {
            true
        }
        fn recipients(&self) -> Vec<String> {
            Vec::new()
        }
        fn step<'a>(
            &'a self,
            ask: &'a AuthorAsk,
        ) -> futures_util::future::BoxFuture<'a, Result<AuthorStep, SeamError>> {
            use openagents_chat::plugin_flow::{Flow, Step};
            let open = coder::eval_author::plugin::open(&ask.transcript);
            let here = ask.here;
            Box::pin(async move {
                assert!(here, "the turn came from the computer Coder runs on");
                let (step, offer) = match open {
                    Some(Step::Tests) => (Step::Run, None),
                    _ => (
                        Step::Draft,
                        Some(router::Offer::RunCoder {
                            label: "Run Coder".into(),
                            engine: None,
                            plan: Default::default(),
                        }),
                    ),
                };
                Ok(AuthorStep {
                    text: step.line().into(),
                    draft: None,
                    offer,
                    model: coder::eval_author::plugin::MODEL.into(),
                    plugin: Some(Flow::at(step, Some("greeter".into()))),
                })
            })
        }
    }

    /// #10177: on a computer, `eval.author` serves the plugin flow's step
    /// as the result's typed `plugin` field, with its Run Coder offer, and
    /// a short reply while a plugin is being made continues it rather than
    /// answering as small talk.
    #[tokio::test]
    async fn a_plugin_step_rides_the_result_and_an_open_flow_continues() {
        let context = json!({
            "surface": "terminal", "computer_ready": true,
            "computer": { "place": "here", "engines": [] },
        });
        let terminal = |task: &str| {
            let mut turn = v2_turn(task);
            turn["context"] = context.clone();
            turn
        };
        let seams = || Seams {
            author: Arc::new(PluginSeam),
            ..Seams::default()
        };
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed_in(&context, "eval.author", "none", 0.1, "none"),
            )),
            terminal("Help me make a plugin that greets people by name"),
            seams(),
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["plugin"]["step"], "draft");
        assert_eq!(result["tier"], "author");
        assert_eq!(of_type(&frames, "offer")[0]["offer"], "run_coder");

        let mut turn = terminal("yes");
        turn["transcript"] = json!([
            { "role": "user", "content": "Help me make a plugin that greets people by name" },
            { "role": "assistant", "content": openagents_chat::plugin_flow::Step::Tests.line() },
            { "role": "user", "content": "yes" },
        ]);
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed_in(&context, "smalltalk", "smalltalk.thanks", 0.1, "none"),
            )),
            turn,
            seams(),
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["plugin"]["step"], "run", "{result}");
        assert_eq!(result["plugin"]["slug"], "greeter");
    }

    /// `eval.author` without the interview wired says so, from
    /// the bank; with a draft open, a short reply continues the interview
    /// through the seam, and its draft card and offer ride as feedback.
    #[tokio::test]
    async fn the_interview_is_the_seams_or_the_banks_never_the_models() {
        let soon = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed("eval.author", "none", 0.9, "none"),
            )),
            v2_turn("Help me make a tool"),
            Seams::default(),
            RouterSetting::Live,
        )
        .await;
        let result = &soon.last().unwrap().1;
        assert_eq!(result["answer"], "eval.author.soon@3");
        assert_eq!(result["tier"], "author");
        assert!(soon.last().unwrap().0 < Duration::from_millis(1_400));

        let mut turn = v2_turn("looks good");
        turn["draft"] = serde_json::from_str(include_str!(
            "../../../nostr/fixtures/eval-ext/eval-draft/valid/chat-made-tool.json"
        ))
        .unwrap();
        let frames = frames_routed(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed("smalltalk", "smalltalk.thanks", 0.1, "none"),
            )),
            turn,
            Seams {
                author: Arc::new(Interviews),
                ..Seams::default()
            },
            RouterSetting::Live,
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["text"], "Good. Should we try it once?");
        assert_eq!(result["model"], "interview-test");
        assert_eq!(of_type(&frames, "card")[0]["card"], "draft");
        assert_eq!(of_type(&frames, "offer")[0]["suite"], "draft");
    }

    // ---------------------------------------------------------------------
    // Where the chat runs (#10077)
    // ---------------------------------------------------------------------

    /// The desktop's context: this computer is where Coder runs, with the
    /// chat's project folder, as `router-request-computer.json` carries it.
    fn on_computer(ready: bool) -> Value {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../fixtures/nip-cj/router-request-computer.json"
        ))
        .unwrap();
        let mut context = fixture["context"].clone();
        context["computer_ready"] = json!(ready);
        context
    }

    /// On a computer, "what's your working dir" is answered from the turn's
    /// context: the project folder by name and path, with no offer and no
    /// word about connecting a computer.
    #[tokio::test]
    async fn on_a_computer_the_working_directory_is_the_project_folder() {
        let answers = routed_in(
            &on_computer(true),
            "meta",
            "meta.limits_chat.here",
            0.1,
            "none",
        );
        let frames = frames_through(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn("whats your working dir", on_computer(true)),
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["answer"], "meta.limits_chat.here@1");
        let text = result["text"].as_str().unwrap();
        assert!(
            text.contains("openagents at /Users/someone/work/openagents"),
            "{text}"
        );
        assert!(!text.to_lowercase().contains("connect"), "{text}");
        assert!(of_type(&frames, "offer").is_empty());
        // The phone's version of the same answer is not shown there, and
        // the desktop's is not shown on a phone.
        let phone = frames_through(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed("meta", "meta.limits_chat.here", 0.1, "none"),
            )),
            routed_turn("whats your working dir", json!({ "surface": "phone" })),
        )
        .await;
        assert_ne!(phone.last().unwrap().1["answer"], "meta.limits_chat.here@1");
    }

    /// Work asked for on a computer whose coding agents can't take it is
    /// told so, with no offer to connect a computer; a phone with none
    /// still gets that offer.
    #[tokio::test]
    async fn on_a_computer_dispatch_never_asks_to_connect_one() {
        let answers = routed_in(
            &on_computer(false),
            "work.dispatch",
            "dispatch.stem",
            0.9,
            "none",
        );
        let frames = frames_through(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(Duration::ZERO, answers)),
            routed_turn("look through my repo", on_computer(false)),
        )
        .await;
        let result = &frames.last().unwrap().1;
        assert_eq!(result["answer"], "dispatch.no_computer.here@3");
        assert!(
            !result["text"].as_str().unwrap().contains("Connect one"),
            "{result}"
        );
        assert!(of_type(&frames, "offer").is_empty());
        // A ready computer gets the dispatch offer, as on a paired phone.
        let ready = frames_through(
            slow_door(Duration::from_millis(1_500)),
            Some(judge(
                Duration::ZERO,
                routed_in(
                    &on_computer(true),
                    "work.dispatch",
                    "dispatch.stem",
                    0.9,
                    "none",
                ),
            )),
            routed_turn("look through my repo", on_computer(true)),
        )
        .await;
        assert_eq!(of_type(&ready, "offer")[0]["offer"], "run_coder");
    }

    /// The chat model is told where the chat runs: on a computer, that
    /// Coder runs here, never to connect a computer, the agents'
    /// readiness, and the project folder as the working directory; on a
    /// paired phone, the computer's name; on a phone with none, nothing.
    #[tokio::test]
    async fn the_model_is_told_where_the_chat_runs() {
        let instructions = |context: Value| async move {
            let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
            let (url, seen) = serve_recorded(1, Duration::ZERO, "text/event-stream", stream.into());
            let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
            let mut payload = routed_turn("whats your working dir", context);
            payload["instructions"] = json!("We are OpenAgents.");
            let frames = frames_through(door, None, payload).await;
            assert_eq!(frames.last().unwrap().1["type"], "result");
            let seen = seen.lock().unwrap();
            seen[0]["instructions"].as_str().unwrap().to_string()
        };
        let here = instructions(on_computer(true)).await;
        assert!(
            here.starts_with("We are OpenAgents.\n\nAbout this chat:"),
            "{here}"
        );
        for words in [
            "on the user's own computer (named \"Studio Mac\")",
            "Never tell the user to connect a computer",
            "Codex is at its usage limit; Claude Code is ready",
            "\"openagents\", at \"/Users/someone/work/openagents\"",
            "working directory",
        ] {
            assert!(here.contains(words), "{words} in {here}");
        }
        let paired = instructions(json!({
            "surface": "phone", "computer_ready": true,
            "computer": { "place": "paired", "name": "Studio Mac" },
            // A path from a phone is never read.
            "project": { "name": "openagents", "path": "/Users/someone/work/openagents" },
        }))
        .await;
        assert!(
            paired.contains("paired with their computer \"Studio Mac\""),
            "{paired}"
        );
        assert!(!paired.contains("/Users/someone"), "{paired}");
        let phone = instructions(json!({ "surface": "phone", "computer_ready": false })).await;
        // With no judge, the reply is unrouted: the fixed note follows.
        assert_eq!(
            phone,
            format!("We are OpenAgents.\n\n{}", first::UNROUTED_NOTE)
        );
    }

    /// A follow-up after Coder's run ended tells the chat model what the
    /// run reported, so the chat answers questions about it (#10094).
    #[tokio::test]
    async fn the_model_is_told_what_the_finished_run_did() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../fixtures/nip-cj/router-request-coder-run.json"
        ))
        .unwrap();
        let stream = include_str!("../../fixtures/gateway/google-gemini-3.8-flash.sse");
        let (url, seen) = serve_recorded(1, Duration::ZERO, "text/event-stream", stream.into());
        let door = Door::Live(coder::generate::ResponsesDoor::new(url, GEMINI, "test"));
        let mut payload = fixture.clone();
        payload["instructions"] = json!("We are OpenAgents.");
        let frames = frames_through(door, None, payload).await;
        assert_eq!(frames.last().unwrap().1["type"], "result");
        let seen = seen.lock().unwrap();
        let instructions = seen[0]["instructions"].as_str().unwrap();
        for words in [
            "Coder, our coding agent, already ran in this chat: its turn 1 finished on Codex \
             (gpt-6-luna).",
            "It holds a Rust workspace with 40 crates.",
            "Files it changed: NOTE.md (added).",
            "Commands it ran: `ls`, `cargo metadata --no-deps`.",
            "Answer questions about that run",
        ] {
            assert!(instructions.contains(words), "{words} in {instructions}");
        }
        // The model reads the conversation as sent, without the line Jev
        // reads.
        assert!(!seen[0].to_string().contains(router::MARKER_FINISHED));
    }
}
