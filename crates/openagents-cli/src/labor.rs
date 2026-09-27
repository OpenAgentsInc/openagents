//! NIP-MKT and NIP-LAB free labor orders through `coder-labor`. A *book* is
//! one operator-admitted setup (market, offering, encrypted terms, and the
//! pinned closure) with a private journal under `~/.openagents/labor/NAME/`.
//! Every command hands signed declarations to the crate and reports what it
//! decided; nothing here authors a record, widens a grant, or retries an
//! execution. Refusals exit 1 and carry a typed `reason` under `--json`.

use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

use coder_labor::Blobs;
use coder_labor::book::Setup;
use coder_labor::store::{Dispatch, Store};
use nostr::domain::Event;
use serde_json::{Value, json};

use crate::{Args, EXIT_FAILURE, Output};

const USAGE: &str = "usage: openagents labor COMMAND [OPTIONS]
  offer NAME SETUP_FILE [--relay URL] [--timeout SECONDS] [--as PROFILE]
        Admit an operator setup (market, offering, encrypted terms, and the
        pinned closure) as book NAME and open its private journal. With
        --relay, also publish the signed offering.
  order NAME EVENT... [--attach FILE]... [--relay URL] [--timeout SECONDS]
        Apply NIP-MKT negotiation records: rfq, quote, order, order_ack.
  deliver NAME EVENT... [--attach FILE]... [--relay URL] [--timeout SECONDS]
        Apply NIP-LAB records: linkage, submission, delivery, verification,
        review, dispute.
  accept NAME EVENT [--relay URL] [--timeout SECONDS]
        Apply the buyer's acceptance record.
  execute NAME EXECUTE_EVENT --grant FILE [--tasks DIR]
        Dispatch the bound CJ request under the operator's local execution
        grant, or reconcile the dispatch that already exists.
  check NAME [--reconcile]
        Report the order, retained records, observations, and dispatch state.
        --reconcile recovers the dispatched task's state first.
  list  List every book and its state.
EVENT is a file, inline JSON, - for stdin, or a 64-hex event ID fetched from
--relay (default wss://relay.openagents.com, --timeout default 10). --attach
adds one exact JSON artifact the record references. --as names the profile key
that opens the encrypted records (default: default). Books live in
~/.openagents/labor/ (LABOR_HOME overrides).";

const DEFAULT_TIMEOUT: u64 = 10;
const MKT_SCHEMAS: &[&str] = &[nostr::market_contracts::RECORD_SCHEMA];
const LAB_SCHEMAS: &[&str] = &[
    coder_labor::records::LINK,
    coder_labor::records::SUBMISSION,
    coder_labor::records::DELIVERY,
    coder_labor::records::VERIFICATION,
    coder_labor::records::REVIEW,
    coder_labor::records::DISPUTE,
];
const ACCEPT_SCHEMAS: &[&str] = &[coder_labor::records::ACCEPTANCE];

/// Why a command exited 1. The `reason` field under `--json`.
#[derive(Clone, Copy, Debug)]
enum Reason {
    /// The setup's closure or terms differ from what this host admits.
    Admission,
    /// A record was refused or the evidence conflicts.
    Transition,
    /// The journal could not be opened or written.
    Store,
    /// The relay did not deliver or acknowledge in time.
    Relay,
    /// Dispatch refused or the task did not finish.
    Execution,
}

impl Reason {
    fn name(self) -> &'static str {
        match self {
            Self::Admission => "admission",
            Self::Transition => "transition",
            Self::Store => "store",
            Self::Relay => "relay",
            Self::Execution => "execution",
        }
    }
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("labor", "a command is required", USAGE);
    };
    if command == "--help" || command == "-h" || command == "help" {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(rest, &["reconcile"]) {
        Ok(args) => args,
        Err(message) => return output.usage("labor", &message, USAGE),
    };
    let identity = match crate::relay::identity_for(args.option("as")) {
        Ok(identity) => identity,
        Err(message) => return output.fail("labor", &message),
    };
    let timeout = match args.number::<u64>("timeout", DEFAULT_TIMEOUT) {
        Ok(seconds) => Duration::from_secs(seconds),
        Err(message) => return output.usage("labor", &message, USAGE),
    };
    let relay = crate::relay::relay_url(args.option("relay"));
    match command.as_str() {
        "offer" => {
            let [name, file, ..] = args.positional() else {
                return output.usage("labor", "NAME and SETUP_FILE are required", USAGE);
            };
            offer(
                output,
                &identity,
                name,
                file,
                args.option("relay").map(|_| relay.as_str()),
                timeout,
            )
        }
        "order" | "deliver" | "accept" => {
            let Some((name, sources)) = args.positional().split_first() else {
                return output.usage("labor", "NAME is required", USAGE);
            };
            let schemas = match command.as_str() {
                "order" => MKT_SCHEMAS,
                "deliver" => LAB_SCHEMAS,
                _ => ACCEPT_SCHEMAS,
            };
            if sources.is_empty() || (command == "accept" && sources.len() != 1) {
                return output.usage("labor", "one or more EVENTs are required", USAGE);
            }
            let events = match load_events(sources, &relay, &identity.secret, timeout) {
                Ok(events) => events,
                Err(message) => return refuse(output, command, Reason::Relay, &message),
            };
            let attachments = match load_attachments(&args.options("attach")) {
                Ok(blobs) => blobs,
                Err(message) => return output.fail("labor", &message),
            };
            receive(
                output,
                &identity.secret,
                name,
                command,
                schemas,
                events,
                attachments,
            )
        }
        "execute" => {
            let [name, source, ..] = args.positional() else {
                return output.usage("labor", "NAME and EXECUTE_EVENT are required", USAGE);
            };
            let Some(grant) = args.option("grant") else {
                return output.usage("labor", "--grant FILE is required", USAGE);
            };
            execute(
                output,
                &identity.secret,
                name,
                source,
                grant,
                args.option("tasks"),
            )
        }
        "check" => {
            let Some(name) = args.positional().first() else {
                return output.usage("labor", "NAME is required", USAGE);
            };
            check(output, &identity.secret, name, args.switch("reconcile"))
        }
        "list" => list(output, &identity.secret),
        other => output.usage("labor", &format!("unknown command `{other}`"), USAGE),
    }
}

/// The directory books live in: `LABOR_HOME`, or `~/.openagents/labor`.
pub fn home() -> PathBuf {
    if let Some(dir) = std::env::var_os("LABOR_HOME") {
        return PathBuf::from(dir);
    }
    verse::identity::home()
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
        .join("labor")
}

fn book_dir(name: &str) -> Result<PathBuf, String> {
    if name.is_empty()
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_' || b == b'.')
        || name.starts_with('.')
    {
        return Err(format!(
            "`{name}` is not a book name (letters, digits, `-`, `_`, `.`)"
        ));
    }
    Ok(home().join(name))
}

fn refuse(output: &Output, command: &str, reason: Reason, message: &str) -> u8 {
    eprintln!("openagents labor {command}: {message}");
    if output.json() {
        println!("{}", json!({ "error": message, "reason": reason.name() }));
    }
    EXIT_FAILURE
}

fn now() -> u64 {
    crate::relay::unix_now()
}

fn read_setup(dir: &Path) -> Result<Setup, String> {
    let path = dir.join("setup.json");
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("{} is not a labor setup: {error}", path.display()))
}

fn open_store(dir: &Path, setup: Setup, secret: &secp256k1::SecretKey) -> Result<Store, String> {
    Store::open(&dir.join("journal"), setup, *secret)
}

/// Admit `file` as the book `name` and open its journal.
fn offer(
    output: &Output,
    identity: &verse::identity::Identity,
    name: &str,
    file: &str,
    relay: Option<&str>,
    timeout: Duration,
) -> u8 {
    let dir = match book_dir(name) {
        Ok(dir) => dir,
        Err(message) => return output.usage("labor", &message, USAGE),
    };
    let text = match std::fs::read_to_string(file) {
        Ok(text) => text,
        Err(error) => return output.fail("labor", &format!("cannot read {file}: {error}")),
    };
    let setup: Setup = match serde_json::from_str(&text) {
        Ok(setup) => setup,
        Err(error) => {
            return refuse(
                output,
                "offer",
                Reason::Admission,
                &format!("{file} is not a labor setup: {error}"),
            );
        }
    };
    let proposed = serde_json::to_value(&setup).unwrap_or(Value::Null);
    if dir.join("setup.json").exists() {
        match read_setup(&dir) {
            Ok(existing) if serde_json::to_value(&existing).unwrap_or(Value::Null) == proposed => {}
            Ok(_) => {
                return refuse(
                    output,
                    "offer",
                    Reason::Store,
                    &format!("book `{name}` has another frozen setup"),
                );
            }
            Err(message) => return refuse(output, "offer", Reason::Store, &message),
        }
    } else {
        if let Err(message) = coder_labor::book::Book::new(setup.clone(), identity.secret) {
            return refuse(output, "offer", Reason::Admission, &message);
        }
        if let Err(message) = write_setup(&dir, &proposed) {
            return refuse(output, "offer", Reason::Store, &message);
        }
    }
    let store = match open_store(&dir, setup.clone(), &identity.secret) {
        Ok(store) => store,
        Err(message) => return refuse(output, "offer", Reason::Admission, &message),
    };
    let published = match relay {
        Some(url) => {
            let mut client = crate::relay::Client::connect(url, identity.signer.clone());
            match client.publish(setup.offering.clone(), timeout) {
                Ok(published) if published.accepted => json!({ "relay": url, "accepted": true }),
                Ok(published) => {
                    return refuse(
                        output,
                        "offer",
                        Reason::Relay,
                        &format!("{url} refused the offering: {}", published.message),
                    );
                }
                Err(message) => return refuse(output, "offer", Reason::Relay, &message),
            }
        }
        None => Value::Null,
    };
    let mut report = status(name, &setup.market, &store, None);
    report["offering"] = json!(setup.offering.id);
    report["published"] = published;
    output.emit(&report, |value| {
        format!(
            "{} admitted  market {}  offering {}{}",
            value["name"].as_str().unwrap_or(""),
            value["market"].as_str().unwrap_or(""),
            value["offering"].as_str().unwrap_or(""),
            if value["published"].is_null() {
                String::new()
            } else {
                format!(
                    "  published to {}",
                    value["published"]["relay"].as_str().unwrap_or("")
                )
            }
        )
    });
    0
}

fn write_setup(dir: &Path, setup: &Value) -> Result<(), String> {
    let parent = dir.parent().ok_or("labor home has no parent")?;
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(parent)
        .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
    match std::fs::DirBuilder::new().mode(0o700).create(dir) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(format!("cannot create {}: {error}", dir.display())),
    }
    let path = dir.join("setup.json");
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .map_err(|error| format!("cannot create {}: {error}", path.display()))?;
    std::io::Write::write_all(&mut file, setup.to_string().as_bytes())
        .and_then(|()| file.sync_all())
        .map_err(|error| format!("cannot write {}: {error}", path.display()))
}

/// Read each EVENT source: stdin, inline JSON, a relay event ID, or a file.
fn load_events(
    sources: &[String],
    relay: &str,
    secret: &secp256k1::SecretKey,
    timeout: Duration,
) -> Result<Vec<Event>, String> {
    let mut events = Vec::with_capacity(sources.len());
    for source in sources {
        let event = if source == "-" {
            let mut text = String::new();
            std::io::Read::read_to_string(&mut std::io::stdin(), &mut text)
                .map_err(|error| format!("cannot read stdin: {error}"))?;
            parse_event(&text)?
        } else if source.trim_start().starts_with('{') {
            parse_event(source)?
        } else if source.len() == 64 && source.bytes().all(|b| b.is_ascii_hexdigit()) {
            crate::runtime()
                .block_on(tokio::time::timeout(
                    timeout,
                    coder_labor::transport::fetch(relay, secret, source),
                ))
                .map_err(|_| format!("{relay} did not deliver {source} within {timeout:?}"))??
        } else {
            let text = std::fs::read_to_string(source)
                .map_err(|error| format!("cannot read {source}: {error}"))?;
            parse_event(&text)?
        };
        events.push(event);
    }
    Ok(events)
}

fn parse_event(text: &str) -> Result<Event, String> {
    serde_json::from_str(text).map_err(|error| format!("invalid event: {error}"))
}

fn load_attachments(files: &[&str]) -> Result<Blobs, String> {
    let mut blobs = Blobs::default();
    for file in files {
        let text = std::fs::read_to_string(file)
            .map_err(|error| format!("cannot read {file}: {error}"))?;
        let value: Value =
            serde_json::from_str(&text).map_err(|error| format!("{file} is not JSON: {error}"))?;
        let bytes = nostr::contracts::jcs(&value).map_err(|error| error.to_string())?;
        blobs
            .0
            .insert(nostr::contracts::digest_bytes(&bytes), value);
    }
    Ok(blobs)
}

/// The schema a sealed record carries, as this identity opens it. A record
/// this identity cannot open has no schema; the store retains the refusal.
fn schema_of(event: &Event, secret: &secp256k1::SecretKey) -> Option<String> {
    let opened = nostr::private_artifact::open(event, secret).ok()?;
    let body = nostr::contracts::parse_strict(opened.inline_bytes()?).ok()?;
    body["v"].as_str().map(str::to_owned)
}

fn receive(
    output: &Output,
    secret: &secp256k1::SecretKey,
    name: &str,
    command: &str,
    schemas: &[&str],
    events: Vec<Event>,
    attachments: Blobs,
) -> u8 {
    let dir = match book_dir(name) {
        Ok(dir) => dir,
        Err(message) => return output.usage("labor", &message, USAGE),
    };
    let setup = match read_setup(&dir) {
        Ok(setup) => setup,
        Err(message) => return refuse(output, command, Reason::Store, &message),
    };
    let setup_market = setup.market.clone();
    let mut store = match open_store(&dir, setup, secret) {
        Ok(store) => store,
        Err(message) => return refuse(output, command, Reason::Store, &message),
    };
    for event in &events {
        if let Some(schema) = schema_of(event, secret)
            && !schemas.contains(&schema.as_str())
        {
            return output.usage(
                "labor",
                &format!(
                    "{} is a `{schema}` record; `{command}` does not apply it",
                    event.id
                ),
                USAGE,
            );
        }
    }
    let mut results = Vec::with_capacity(events.len());
    let mut refused = false;
    for event in events {
        let id = event.id.clone();
        let schema = schema_of(&event, secret);
        match store.receive(event, now(), attachments.clone()) {
            Ok(outcome) => {
                if outcome != "applied" && outcome != "duplicate" {
                    refused = true;
                }
                results.push(json!({ "id": id, "schema": schema, "outcome": outcome }));
            }
            Err(message) => return refuse(output, command, Reason::Store, &message),
        }
    }
    let mut report = status(name, &setup_market, &store, None);
    report["events"] = Value::Array(results);
    if refused {
        report["error"] = json!("a record was refused or the evidence conflicts");
        report["reason"] = json!(Reason::Transition.name());
    }
    output.emit(&report, |value| {
        let mut lines: Vec<String> = value["events"]
            .as_array()
            .map(|events| {
                events
                    .iter()
                    .map(|event| {
                        format!(
                            "{}  {}  {}",
                            &event["id"].as_str().unwrap_or("")
                                [..12.min(event["id"].as_str().map_or(0, str::len))],
                            event["schema"].as_str().unwrap_or("?"),
                            event["outcome"].as_str().unwrap_or("")
                        )
                    })
                    .collect()
            })
            .unwrap_or_default();
        lines.push(status_line(value));
        lines.join("\n")
    });
    if refused {
        eprintln!("openagents labor {command}: a record was refused or the evidence conflicts");
        EXIT_FAILURE
    } else {
        0
    }
}

fn execute(
    output: &Output,
    secret: &secp256k1::SecretKey,
    name: &str,
    source: &str,
    grant: &str,
    tasks: Option<&str>,
) -> u8 {
    let dir = match book_dir(name) {
        Ok(dir) => dir,
        Err(message) => return output.usage("labor", &message, USAGE),
    };
    let text = if source.trim_start().starts_with('{') {
        source.to_owned()
    } else {
        match std::fs::read_to_string(source) {
            Ok(text) => text,
            Err(error) => return output.fail("labor", &format!("cannot read {source}: {error}")),
        }
    };
    let event = match parse_event(&text) {
        Ok(event) => event,
        Err(message) => return output.fail("labor", &message),
    };
    let grant_bytes = match std::fs::read(grant) {
        Ok(bytes) => bytes,
        Err(error) => return output.fail("labor", &format!("cannot read {grant}: {error}")),
    };
    let tasks = tasks.map_or_else(|| dir.join("tasks"), PathBuf::from);
    let setup = match read_setup(&dir) {
        Ok(setup) => setup,
        Err(message) => return refuse(output, "execute", Reason::Store, &message),
    };
    let market = setup.market.clone();
    let mut store = match open_store(&dir, setup, secret) {
        Ok(store) => store,
        Err(message) => return refuse(output, "execute", Reason::Store, &message),
    };
    let dispatch =
        match crate::runtime().block_on(store.dispatch(event, &grant_bytes, &tasks, now())) {
            Ok(dispatch) => dispatch,
            Err(message) => return refuse(output, "execute", Reason::Execution, &message),
        };
    let finished = dispatch.state == "finished";
    let mut report = status(name, &market, &store, Some(&dispatch));
    if !finished {
        report["error"] = json!(format!("the dispatched task is {}", dispatch.state));
        report["reason"] = json!(Reason::Execution.name());
    }
    output.emit(&report, status_line);
    if finished {
        0
    } else {
        eprintln!(
            "openagents labor execute: the dispatched task is {}",
            dispatch.state
        );
        EXIT_FAILURE
    }
}

fn check(output: &Output, secret: &secp256k1::SecretKey, name: &str, reconcile: bool) -> u8 {
    let dir = match book_dir(name) {
        Ok(dir) => dir,
        Err(message) => return output.usage("labor", &message, USAGE),
    };
    let setup = match read_setup(&dir) {
        Ok(setup) => setup,
        Err(message) => return refuse(output, "check", Reason::Store, &message),
    };
    let market = setup.market.clone();
    let mut store = match open_store(&dir, setup, secret) {
        Ok(store) => store,
        Err(message) => return refuse(output, "check", Reason::Store, &message),
    };
    let dispatch = if reconcile {
        match store.reconcile() {
            Ok(dispatch) => Some(dispatch),
            Err(message) => return refuse(output, "check", Reason::Execution, &message),
        }
    } else {
        None
    };
    let mut report = status(name, &market, &store, dispatch.as_ref());
    report["observations"] = store
        .observations()
        .iter()
        .map(|observation| {
            json!({
                "id": observation.event.id,
                "received_at": observation.received_at,
                "outcome": observation.outcome,
            })
        })
        .collect();
    output.emit(&report, |value| {
        let mut lines = vec![status_line(value)];
        let records = &value["records"];
        for field in [
            "linkage",
            "submission",
            "delivery",
            "verification",
            "review",
            "acceptance",
        ] {
            lines.push(format!(
                "{field:<13} {}",
                if records[field].is_null() {
                    "absent"
                } else {
                    "retained"
                }
            ));
        }
        lines.push(format!("disputes      {}", records["disputes"]));
        lines.push(format!("conflict      {}", records["conflict"]));
        if let Some(observations) = value["observations"].as_array() {
            for observation in observations {
                lines.push(format!(
                    "{}  {}  {}",
                    &observation["id"].as_str().unwrap_or("")
                        [..12.min(observation["id"].as_str().map_or(0, str::len))],
                    observation["received_at"],
                    observation["outcome"].as_str().unwrap_or("")
                ));
            }
        }
        lines.join("\n")
    });
    0
}

fn list(output: &Output, secret: &secp256k1::SecretKey) -> u8 {
    let home = home();
    let mut names: Vec<String> = match std::fs::read_dir(&home) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().join("setup.json").is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return output.fail("labor", &format!("cannot read {}: {error}", home.display()));
        }
    };
    names.sort();
    let books: Vec<Value> = names
        .iter()
        .map(|name| {
            let dir = home.join(name);
            let opened = read_setup(&dir).and_then(|setup| {
                let market = setup.market.clone();
                open_store(&dir, setup, secret).map(|store| (market, store))
            });
            match opened {
                Ok((market, store)) => status(name, &market, &store, None),
                Err(message) => json!({ "name": name, "error": message }),
            }
        })
        .collect();
    output.emit(
        &json!({ "home": home.display().to_string(), "books": books }),
        |value| {
            let mut rows = vec![vec![
                "NAME".to_owned(),
                "MARKET".to_owned(),
                "ORDER".to_owned(),
                "DISPATCH".to_owned(),
            ]];
            for book in value["books"].as_array().into_iter().flatten() {
                if let Some(error) = book["error"].as_str() {
                    rows.push(vec![
                        book["name"].as_str().unwrap_or("").to_owned(),
                        format!("unavailable: {error}"),
                    ]);
                    continue;
                }
                rows.push(vec![
                    book["name"].as_str().unwrap_or("").to_owned(),
                    short(book["market"].as_str().unwrap_or("")),
                    if book["order"].is_null() {
                        "negotiating".to_owned()
                    } else {
                        "confirmed".to_owned()
                    },
                    book["dispatch"]["state"]
                        .as_str()
                        .unwrap_or("none")
                        .to_owned(),
                ]);
            }
            crate::out::table(&rows)
        },
    );
    0
}

fn short(text: &str) -> String {
    text.chars().take(12).collect()
}

/// One book's state: parties, the confirmed order, and the retained records.
fn status(name: &str, market_id: &str, store: &Store, dispatch: Option<&Dispatch>) -> Value {
    let book = &store.book;
    let market = book.market();
    let records = &book.records;
    json!({
        "name": name,
        "market": market_id,
        "buyer": market.buyer,
        "provider": market.provider,
        "worker": market.worker,
        "delivery_due_at": market.delivery_due_at,
        "retain_until": market.retain_until,
        "order": book.order().map(|order| json!({
            "order_id": order.order_id,
            "market": order.market,
            "buyer": order.buyer,
            "provider": order.provider,
            "order": coder_labor::artifact_value(&order.order),
            "confirmation": coder_labor::artifact_value(&order.confirmation),
        })),
        "records": {
            "linkage": records.link,
            "submission": records.submission,
            "delivery": records.delivery,
            "verification": records.verification,
            "review": records.review,
            "acceptance": records.acceptance,
            "disputes": records.disputes.len(),
            "conflict": records.conflict,
        },
        "observation_count": store.observations().len(),
        "dispatch": dispatch.map(|dispatch| json!({
            "execute": dispatch.execute.id,
            "task_id": dispatch.task_id,
            "task_directory": dispatch.task_directory.display().to_string(),
            "state": dispatch.state,
        })),
    })
}

fn status_line(value: &Value) -> String {
    let stage = if value["records"]["conflict"].as_bool().unwrap_or(false) {
        "conflict"
    } else if !value["records"]["acceptance"].is_null() {
        "accepted"
    } else if value["records"]["disputes"].as_u64().unwrap_or(0) > 0 {
        "disputed"
    } else if !value["records"]["review"].is_null() {
        "reviewed"
    } else if !value["records"]["verification"].is_null() {
        "verified"
    } else if !value["records"]["delivery"].is_null() {
        "delivered"
    } else if !value["records"]["submission"].is_null() {
        "submitted"
    } else if !value["records"]["linkage"].is_null() {
        "linked"
    } else if !value["order"].is_null() {
        "confirmed"
    } else {
        "negotiating"
    };
    format!(
        "{}  {stage}  buyer {}  provider {}  observations {}{}",
        value["name"].as_str().unwrap_or(""),
        short(value["buyer"].as_str().unwrap_or("")),
        short(value["provider"].as_str().unwrap_or("")),
        value["observation_count"],
        value["dispatch"]["state"]
            .as_str()
            .map(|state| format!("  dispatch {state}"))
            .unwrap_or_default()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_names_are_plain_path_segments() {
        assert!(book_dir("order-42.v1").is_ok());
        for bad in ["", ".hidden", "../up", "a/b", "sp ace"] {
            assert!(book_dir(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn stage_follows_the_retained_records() {
        let mut value = json!({
            "name": "b", "buyer": "", "provider": "", "observation_count": 0,
            "order": null,
            "records": {
                "linkage": null, "submission": null, "delivery": null,
                "verification": null, "review": null, "acceptance": null,
                "disputes": 0, "conflict": false,
            },
        });
        assert!(status_line(&value).contains("  negotiating  "));
        value["order"] = json!({});
        assert!(status_line(&value).contains("  confirmed  "));
        value["records"]["delivery"] = json!({});
        assert!(status_line(&value).contains("  delivered  "));
        value["records"]["disputes"] = json!(1);
        assert!(status_line(&value).contains("  disputed  "));
        value["records"]["acceptance"] = json!({});
        assert!(status_line(&value).contains("  accepted  "));
        value["records"]["conflict"] = json!(true);
        assert!(status_line(&value).contains("  conflict  "));
    }

    #[test]
    fn reasons_have_stable_names() {
        assert_eq!(Reason::Admission.name(), "admission");
        assert_eq!(Reason::Transition.name(), "transition");
        assert_eq!(Reason::Store.name(), "store");
        assert_eq!(Reason::Relay.name(), "relay");
        assert_eq!(Reason::Execution.name(), "execution");
    }
}
