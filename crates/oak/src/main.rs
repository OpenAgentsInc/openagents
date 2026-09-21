//! `oak` — the caller's CLI for the decision API.
//!
//! `oak ask` sends one state, or a bounded batch of them, through
//! `POST /v1/systemone` and writes one JSON object per row to standard
//! output. `oak models` lists the doors the credential can reach.
//! `oak classify` sends one `openagents.classify.v1` envelope through
//! `POST /v1/classify` and writes the report it answers.
//!
//! ```text
//! oak ask --questions PATH [--input state|lines|ndjson] [STATE] [FLAGS]
//! oak models [FLAGS]
//! oak classify --envelope PATH [FLAGS]
//! ```
//!
//! Credentials come from `OPENAGENTS_API_KEY` or the `api_key` field of a
//! protected config file — never a flag, so a key never lands in a process
//! list. The endpoint comes from `--url`, `OPENAGENTS_BASE_URL`, or the
//! file's `base_url`; the workspace from `--workspace`,
//! `OPENAGENTS_WORKSPACE`, or the file's `workspace`. Read
//! `docs/decision-models/caller.md` and
//! `docs/decision-models/classification-callers.md`.

use std::io::{BufWriter, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use jev::{
    Entry, Question, Questions, ResponseBody, SystemOneRequest,
};
use oak::{
    CALLER_FAULTS, CLASSIFY_ENVELOPE_FAULTS, CLASSIFY_SCHEMA, CallOpts, MAX_ENVELOPE_BYTES,
    MAX_RETRY_AFTER, Reply, Settings, read_bounded,
};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Map, Value, json};

/// Every row answered.
const EXIT_ANSWERED: i32 = oak::EXIT_ANSWERED;
/// The run itself failed: bad credentials, an unbound door, a malformed
/// envelope — the fix is the caller's, not a retry's.
const EXIT_FAILURE: i32 = oak::EXIT_FAILURE;
/// The command line or its inputs did not parse.
const EXIT_USAGE: i32 = oak::EXIT_USAGE;
/// No row answered and at least one was refused.
const EXIT_REFUSED: i32 = oak::EXIT_REFUSED;
/// No row answered or refused and at least one was unavailable or invalid.
const EXIT_UNAVAILABLE: i32 = oak::EXIT_UNAVAILABLE;
/// Some mix of the above.
const EXIT_MIXED: i32 = oak::EXIT_MIXED;

fn usage() -> ! {
    eprintln!(
        "usage:\n  \
         oak ask --questions PATH [--input state|lines|ndjson] [STATE] [FLAGS]\n  \
         oak models [FLAGS]\n  \
         oak classify --envelope PATH [FLAGS]\n\n  \
         flags:\n    \
         --url URL              service root (OPENAGENTS_BASE_URL, or `base_url` in the file)\n    \
         --model DOOR           door to ask (OPENAGENTS_MODEL, or `model` in the file)\n    \
         --workspace ID         workspace for X-Workspace-Id (OPENAGENTS_WORKSPACE,\n    \
         \x20                       or `workspace` in the file)\n    \
         --config PATH          credential file (OPENAGENTS_CONFIG,\n    \
         \x20                       default ~/.config/openagents/oak.json)\n    \
         --envelope PATH        the openagents.classify.v1 envelope; `-` reads stdin\n    \
         --input MODE           state | lines | ndjson (default: state)\n    \
         --concurrency N        in-flight calls for batch input (default: 4)\n    \
         --retries N            retries per call, honoring Retry-After (default: 3)\n    \
         --timeout SECS         per-attempt timeout (default: 60)\n    \
         --request-id KEY       idempotency key; batch rows derive KEY/<id>\n    \
         --select ID            emit only these question ids (repeatable)\n    \
         --uncertain-below P    tag rows whose winning probability falls under P\n    \
         --quiet                no progress on standard error"
    );
    std::process::exit(EXIT_USAGE);
}

struct Ask {
    questions: PathBuf,
    input: Input,
    state_arg: Option<String>,
    concurrency: usize,
    retries: u32,
    timeout: Duration,
    request_id: Option<String>,
    select: Vec<String>,
    uncertain_below: Option<f64>,
    url: Option<String>,
    model: Option<String>,
    workspace: Option<String>,
    config: Option<PathBuf>,
    quiet: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Input {
    State,
    Lines,
    Ndjson,
}

/// One batch row: the id it reports under, the state to ask about — or the
/// parse failure it reports instead — and its idempotency key.
struct Item {
    id: String,
    state: Option<Value>,
    invalid: Option<String>,
    request_id: Option<String>,
}

/// How one row ended, ordered to index the counts array.
#[derive(Clone, Copy, PartialEq)]
enum Outcome {
    Answered = 0,
    Refused = 1,
    Unavailable = 2,
    Invalid = 3,
}

impl Outcome {
    fn name(self) -> &'static str {
        match self {
            Self::Answered => "answered",
            Self::Refused => "refused",
            Self::Unavailable => "unavailable",
            Self::Invalid => "invalid",
        }
    }
}

/// The shared flags every verb reads: endpoint, workspace, credential
/// file, timeout, and retries.
#[derive(Default)]
struct Common {
    url: Option<String>,
    workspace: Option<String>,
    config: Option<PathBuf>,
    timeout: Option<Duration>,
    retries: Option<u32>,
    request_id: Option<String>,
    quiet: bool,
}

impl Common {
    /// Read one flag into the shared set; `true` when it took it.
    fn flag(&mut self, name: &str, args: &mut impl Iterator<Item = String>) -> bool {
        let value = |args: &mut dyn Iterator<Item = String>| args.next().unwrap_or_else(|| usage());
        match name {
            "--url" => self.url = Some(value(args)),
            "--workspace" => self.workspace = Some(value(args)),
            "--config" => self.config = Some(PathBuf::from(value(args))),
            "--timeout" => self.timeout = Some(seconds(Some(value(args)))),
            "--retries" => {
                self.retries = Some(
                    value(args)
                        .parse::<u32>()
                        .unwrap_or_else(|_| usage()),
                );
            }
            "--request-id" => self.request_id = Some(value(args)),
            "--quiet" => self.quiet = true,
            _ => return false,
        }
        true
    }

    fn opts(&self) -> CallOpts {
        CallOpts {
            timeout: self.timeout.unwrap_or(Duration::from_secs(60)),
            retries: self.retries.unwrap_or(3),
            request_id: self.request_id.clone(),
        }
    }

    fn settings(&self) -> Result<Settings, String> {
        Settings::resolve(
            self.url.clone(),
            None,
            self.workspace.clone(),
            self.config.clone(),
        )
    }
}

fn main() {
    let mut args = std::env::args().skip(1).peekable();
    let Some(verb) = args.next() else {
        usage();
    };
    match verb.as_str() {
        "version" | "--version" => {
            println!("oak {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(EXIT_ANSWERED);
        }
        "models" => {
            let mut common = Common::default();
            while let Some(flag) = args.next() {
                if !common.flag(&flag, &mut args) {
                    usage();
                }
            }
            std::process::exit(models(&common));
        }
        "classify" => {
            let mut common = Common::default();
            let mut envelope = None;
            while let Some(flag) = args.next() {
                if common.flag(&flag, &mut args) {
                    continue;
                }
                match flag.as_str() {
                    "--envelope" => {
                        envelope = Some(args.next().unwrap_or_else(|| usage()));
                    }
                    _ => usage(),
                }
            }
            let Some(envelope) = envelope else { usage() };
            std::process::exit(classify(&envelope, &common));
        }
        "ask" => {}
        _ => usage(),
    }

    let mut ask = Ask {
        questions: PathBuf::new(),
        input: Input::State,
        state_arg: None,
        concurrency: 4,
        retries: 3,
        timeout: Duration::from_secs(60),
        request_id: None,
        select: Vec::new(),
        uncertain_below: None,
        url: None,
        model: None,
        workspace: None,
        config: None,
        quiet: false,
    };
    let mut positional: Vec<String> = Vec::new();
    let mut questions_seen = false;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--questions" => {
                ask.questions = PathBuf::from(args.next().unwrap_or_else(|| usage()));
                questions_seen = true;
            }
            "--input" => {
                ask.input = match args.next().as_deref() {
                    Some("state") => Input::State,
                    Some("lines") => Input::Lines,
                    Some("ndjson") => Input::Ndjson,
                    _ => usage(),
                };
            }
            "--concurrency" => {
                ask.concurrency = args
                    .next()
                    .and_then(|value| value.parse::<usize>().ok())
                    .filter(|n| *n > 0)
                    .unwrap_or_else(|| usage());
            }
            "--retries" => {
                ask.retries = args
                    .next()
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or_else(|| usage());
            }
            "--timeout" => ask.timeout = seconds(args.next()),
            "--request-id" => ask.request_id = args.next(),
            "--select" => ask.select.push(args.next().unwrap_or_else(|| usage())),
            "--uncertain-below" => {
                ask.uncertain_below = Some(
                    args.next()
                        .and_then(|value| value.parse::<f64>().ok())
                        .filter(|p| (0.0..=1.0).contains(p))
                        .unwrap_or_else(|| usage()),
                );
            }
            "--url" => ask.url = args.next(),
            "--model" => ask.model = args.next(),
            "--workspace" => ask.workspace = args.next(),
            "--config" => ask.config = args.next().map(PathBuf::from),
            "--quiet" => ask.quiet = true,
            _ if flag.starts_with('-') => usage(),
            _ => positional.push(flag),
        }
    }
    if !questions_seen {
        usage();
    }
    match (positional.len(), ask.input) {
        (0, _) => {}
        (1, Input::State) => ask.state_arg = positional.into_iter().next(),
        _ => usage(),
    }
    std::process::exit(run(ask));
}

fn seconds(value: Option<String>) -> Duration {
    value
        .and_then(|value| value.parse::<u64>().ok())
        .map(Duration::from_secs)
        .unwrap_or_else(|| usage())
}

/// The `models` verb: the doors this credential is authorized for, one card
/// per line, as the service wrote them.
fn models(common: &Common) -> i32 {
    let settings = match common.settings() {
        Ok(settings) => settings,
        Err(message) => return fatal(&message),
    };
    let transport = match settings.transport() {
        Ok(transport) => transport,
        Err(message) => return fatal(&message),
    };
    match transport.get_models(&common.opts()) {
        Ok(Reply::Document { body, .. }) => {
            let mut out = BufWriter::new(std::io::stdout().lock());
            let Some(cards) = body.get("models").and_then(Value::as_array) else {
                return fatal("the listing did not carry a `models` array");
            };
            for card in cards {
                if writeln!(out, "{}", serde_json::to_string(card).unwrap_or_default()).is_err() {
                    return EXIT_ANSWERED;
                }
            }
            if !common.quiet {
                eprintln!("oak: {} doors", cards.len());
            }
            EXIT_ANSWERED
        }
        Ok(Reply::Refused {
            code, message, request_id, ..
        }) => {
            if let Some(request_id) = request_id {
                eprintln!("oak: {code}: {message} (request_id={request_id})");
            } else {
                eprintln!("oak: {code}: {message}");
            }
            EXIT_FAILURE
        }
        Err(error) => fatal(&error.to_string()),
    }
}

/// The `classify` verb: one envelope in, one report out — the gateway's
/// document verbatim, ordered results, aggregates, selections, and usage
/// included.
fn classify(envelope: &str, common: &Common) -> i32 {
    let bytes = if envelope == "-" {
        if std::io::stdin().is_terminal() {
            eprintln!("oak: --envelope - reads the envelope from standard input");
            return EXIT_USAGE;
        }
        match read_bounded(std::io::stdin().lock(), MAX_ENVELOPE_BYTES, "envelope") {
            Ok(bytes) => bytes,
            Err(message) => {
                eprintln!("oak: {message}");
                return EXIT_USAGE;
            }
        }
    } else {
        let file = match std::fs::File::open(envelope) {
            Ok(file) => file,
            Err(error) => {
                eprintln!("oak: cannot read {envelope}: {error}");
                return EXIT_USAGE;
            }
        };
        match read_bounded(file, MAX_ENVELOPE_BYTES, "envelope") {
            Ok(bytes) => bytes,
            Err(message) => {
                eprintln!("oak: {message}");
                return EXIT_USAGE;
            }
        }
    };
    let parsed: Value = match serde_json::from_slice(&bytes) {
        Ok(parsed) => parsed,
        Err(error) => {
            eprintln!("oak: the envelope does not parse as JSON: {error}");
            return EXIT_USAGE;
        }
    };
    if !parsed.is_object() || parsed.get("v").and_then(Value::as_str) != Some(CLASSIFY_SCHEMA) {
        eprintln!("oak: the envelope is not a `{CLASSIFY_SCHEMA}` document");
        return EXIT_USAGE;
    }
    let settings = match common.settings() {
        Ok(settings) => settings,
        Err(message) => return fatal(&message),
    };
    let transport = match settings.transport() {
        Ok(transport) => transport,
        Err(message) => return fatal(&message),
    };
    let started = Instant::now();
    match transport.post_classify(&bytes, &common.opts()) {
        Ok(Reply::Document {
            status,
            body,
            request_id,
        }) => {
            let mut out = BufWriter::new(std::io::stdout().lock());
            let _ = writeln!(out, "{body}");
            let _ = out.flush();
            if !common.quiet {
                let outcomes = &body["outcomes"];
                eprintln!(
                    "oak: {} ({} answered, {} refused, {} unavailable, {} unattempted) in {:.1}s",
                    body["outcome"].as_str().unwrap_or("unknown"),
                    outcomes["answered"].as_u64().unwrap_or(0),
                    outcomes["refused"].as_u64().unwrap_or(0),
                    outcomes["unavailable"].as_u64().unwrap_or(0),
                    outcomes["unattempted"].as_u64().unwrap_or(0),
                    started.elapsed().as_secs_f64(),
                );
                if let Some(request_id) = request_id {
                    eprintln!("oak: request_id={request_id}");
                }
            }
            match body["outcome"].as_str() {
                Some("answered") => EXIT_ANSWERED,
                Some("mixed") => EXIT_MIXED,
                Some("refused") => EXIT_REFUSED,
                Some("unavailable") | Some("unattempted") => EXIT_UNAVAILABLE,
                _ if (200..300).contains(&status) => EXIT_ANSWERED,
                _ => EXIT_FAILURE,
            }
        }
        Ok(Reply::Refused {
            status,
            code,
            message,
            request_id,
            body,
        }) => {
            // The typed refusal is the run's answer — print it so a pipe
            // still reads one JSON document.
            let mut out = BufWriter::new(std::io::stdout().lock());
            let _ = writeln!(out, "{body}");
            let _ = out.flush();
            if let Some(request_id) = request_id {
                eprintln!("oak: {code}: {message} (request_id={request_id})");
            } else {
                eprintln!("oak: {code}: {message}");
            }
            if CALLER_FAULTS.contains(&code.as_str())
                || CLASSIFY_ENVELOPE_FAULTS.contains(&code.as_str())
            {
                EXIT_FAILURE
            } else if status >= 500 || status == 429 || status == 408 {
                EXIT_UNAVAILABLE
            } else {
                match code.as_str() {
                    "rate_limited" | "busy" | "overloaded" | "unavailable" | "door_unavailable" => {
                        EXIT_UNAVAILABLE
                    }
                    _ => EXIT_REFUSED,
                }
            }
        }
        Err(error) => {
            eprintln!("oak: {error}");
            match error.code {
                "unavailable" => EXIT_UNAVAILABLE,
                _ => EXIT_FAILURE,
            }
        }
    }
}

fn fatal(message: &str) -> i32 {
    eprintln!("oak: {message}");
    EXIT_FAILURE
}

fn run(ask: Ask) -> i32 {
    let started = Instant::now();
    let settings = match Settings::resolve(
        ask.url.clone(),
        ask.model.clone(),
        ask.workspace.clone(),
        ask.config.clone(),
    ) {
        Ok(settings) => settings,
        Err(message) => return fatal(&message),
    };
    let Some(model) = settings.model.clone() else {
        eprintln!(
            "oak: no door named — pass --model, set OPENAGENTS_MODEL, or name \
             `model` in the config file"
        );
        return EXIT_USAGE;
    };
    let text = match std::fs::read_to_string(&ask.questions) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("oak: cannot read {}: {error}", ask.questions.display());
            return EXIT_USAGE;
        }
    };
    let body: Map<String, Value> = match serde_json::from_str(&text) {
        Ok(body) => body,
        Err(error) => {
            eprintln!(
                "oak: {} does not parse as a questions object: {error}",
                ask.questions.display()
            );
            return EXIT_USAGE;
        }
    };
    let mut questions = Questions::new();
    for (id, question) in &body {
        questions.insert(id.clone(), Question::Raw(question.clone()));
    }
    if let Err(error) = questions.validate() {
        eprintln!("oak: {error}");
        return EXIT_USAGE;
    }

    let items = match read_items(&ask) {
        Ok(items) => items,
        Err(code) => return code,
    };
    if items.is_empty() {
        eprintln!("oak: nothing to ask — the input carried no rows");
        return EXIT_USAGE;
    }

    let (done, finished) = mpsc::channel::<(usize, Value, Outcome)>();
    let counter = AtomicUsize::new(0);
    let stop = AtomicBool::new(false);
    let workers = ask.concurrency.min(items.len());
    let total = items.len();

    std::thread::scope(|scope| {
        for _ in 0..workers {
            let done = done.clone();
            let counter = &counter;
            let stop = &stop;
            let items = &items;
            let settings = &settings;
            let questions = &questions;
            let model = &model;
            let ask = &ask;
            scope.spawn(move || {
                let Ok(client) = settings.client(ask.timeout) else {
                    stop.store(true, Ordering::SeqCst);
                    let _ = done.send((
                        usize::MAX,
                        json!({"message": "the client did not build"}),
                        Outcome::Unavailable,
                    ));
                    return;
                };
                loop {
                    if stop.load(Ordering::SeqCst) {
                        return;
                    }
                    let index = counter.fetch_add(1, Ordering::SeqCst);
                    let Some(item) = items.get(index) else { return };
                    let (row, outcome) = call(&client, model, questions, ask, item);
                    if done.send((index, row, outcome)).is_err() {
                        return;
                    }
                }
            });
        }
        drop(done);

        // Emit in input order as the contiguous prefix completes.
        let mut pending = std::collections::BTreeMap::new();
        let mut next = 0_usize;
        let mut seen = 0_usize;
        let mut counts = [0_usize; 4];
        let mut out = BufWriter::new(std::io::stdout().lock());
        let mut fatal_error: Option<String> = None;
        'rows: while seen < total {
            let Ok((index, row, outcome)) = finished.recv() else {
                break;
            };
            if index == usize::MAX {
                fatal_error = row["message"].as_str().map(str::to_string);
                break;
            }
            seen += 1;
            pending.insert(index, (row, outcome));
            while let Some((row, outcome)) = pending.remove(&next) {
                if let Some(code) = row.pointer("/error/code").and_then(Value::as_str)
                    && CALLER_FAULTS.contains(&code)
                {
                    fatal_error = row
                        .pointer("/error/message")
                        .and_then(Value::as_str)
                        .map(|message| format!("{code}: {message}"));
                }
                counts[outcome as usize] += 1;
                if !ask.quiet {
                    eprintln!(
                        "oak: {}/{} {} {}",
                        next + 1,
                        total,
                        row["id"].as_str().unwrap_or("?"),
                        outcome.name()
                    );
                }
                if writeln!(out, "{row}").is_err() {
                    // A closed pipe (`| head`) exits quietly.
                    stop.store(true, Ordering::SeqCst);
                    break 'rows;
                }
                if fatal_error.is_some() {
                    stop.store(true, Ordering::SeqCst);
                    break 'rows;
                }
                next += 1;
            }
        }
        let _ = out.flush();
        if !ask.quiet {
            eprintln!(
                "oak: {} answered, {} refused, {} unavailable, {} invalid in {:.1}s",
                counts[0],
                counts[1],
                counts[2],
                counts[3],
                started.elapsed().as_secs_f64()
            );
        }
        if let Some(message) = fatal_error {
            eprintln!("oak: {message}");
            return EXIT_FAILURE;
        }
        let (answered, refused, failed) = (counts[0], counts[1], counts[2] + counts[3]);
        match () {
            _ if seen == 0 => EXIT_FAILURE,
            _ if refused == 0 && failed == 0 => EXIT_ANSWERED,
            _ if answered == 0 && refused > 0 && failed == 0 => EXIT_REFUSED,
            _ if answered == 0 && refused == 0 && failed > 0 => EXIT_UNAVAILABLE,
            _ => EXIT_MIXED,
        }
    })
}

/// Read the input into rows. Each row already carries the id it reports
/// under and the idempotency key it reserves quota with.
fn read_items(ask: &Ask) -> Result<Vec<Item>, i32> {
    let keyed = |id: &str, line_key: Option<String>| {
        line_key.or_else(|| ask.request_id.as_ref().map(|key| format!("{key}/{id}")))
    };
    match ask.input {
        Input::State => {
            let raw = match &ask.state_arg {
                Some(state) => state.clone(),
                None => {
                    if std::io::stdin().is_terminal() {
                        eprintln!("oak: no state — pass one as an argument or on standard input");
                        return Err(EXIT_USAGE);
                    }
                    let mut raw = String::new();
                    if std::io::stdin().read_to_string(&mut raw).is_err() {
                        eprintln!("oak: cannot read standard input");
                        return Err(EXIT_FAILURE);
                    }
                    raw
                }
            };
            let state = serde_json::from_str::<Value>(&raw).unwrap_or(Value::String(raw));
            Ok(vec![Item {
                id: "state".to_string(),
                state: Some(state),
                invalid: None,
                request_id: ask.request_id.clone(),
            }])
        }
        Input::Lines | Input::Ndjson => {
            if std::io::stdin().is_terminal() {
                eprintln!("oak: --input reads rows from standard input");
                return Err(EXIT_USAGE);
            }
            let mut raw = String::new();
            if std::io::stdin().read_to_string(&mut raw).is_err() {
                eprintln!("oak: cannot read standard input");
                return Err(EXIT_FAILURE);
            }
            let mut items = Vec::new();
            for (index, line) in raw.lines().enumerate() {
                if line.trim().is_empty() {
                    continue;
                }
                let id = format!("line-{}", index + 1);
                let item = match ask.input {
                    Input::Lines => Item {
                        id: id.clone(),
                        state: Some(Value::String(line.to_string())),
                        invalid: None,
                        request_id: keyed(&id, None),
                    },
                    // An object with a `state` key is an envelope —
                    // `{id?, state, request_id?}` — anything else is the
                    // state itself.
                    Input::Ndjson => match serde_json::from_str::<Value>(line) {
                        Ok(Value::Object(mut object)) if object.contains_key("state") => {
                            let id = object
                                .remove("id")
                                .and_then(|value| value.as_str().map(str::to_string))
                                .unwrap_or(id);
                            let line_key = object
                                .remove("request_id")
                                .and_then(|value| value.as_str().map(str::to_string));
                            Item {
                                request_id: keyed(&id, line_key),
                                id,
                                state: Some(object.remove("state").unwrap_or(Value::Null)),
                                invalid: None,
                            }
                        }
                        Ok(state) => Item {
                            id: id.clone(),
                            state: Some(state),
                            invalid: None,
                            request_id: keyed(&id, None),
                        },
                        Err(error) => Item {
                            id: id.clone(),
                            state: None,
                            invalid: Some(format!("not JSON: {error}")),
                            request_id: keyed(&id, None),
                        },
                    },
                    Input::State => unreachable!(),
                };
                items.push(item);
            }
            Ok(items)
        }
    }
}

/// Run one row's call with oak's own retry loop. The service settles quota
/// by `(request, attempt)`, so every retry keeps the idempotency key and
/// bumps `x-attempt` — the client-level retry cannot express that and
/// stays off.
fn call(
    client: &jev::BlockingClient,
    model: &str,
    questions: &Questions,
    ask: &Ask,
    item: &Item,
) -> (Value, Outcome) {
    // A row that never parsed is visible without costing a call.
    if let Some(message) = &item.invalid {
        return (
            row(
                item,
                "invalid",
                None,
                None,
                None,
                Some(("invalid_row", message)),
            ),
            Outcome::Invalid,
        );
    }
    let state = item.state.clone().unwrap_or(Value::Null);
    if let Some(key) = &item.request_id
        && HeaderValue::from_str(key).is_err()
    {
        return (
            row(
                item,
                "invalid",
                None,
                None,
                None,
                Some(("invalid_row", "the idempotency key is not a header value")),
            ),
            Outcome::Invalid,
        );
    }
    let mut attempt = 0_u32;
    let mut delay = Duration::from_millis(250);
    loop {
        attempt += 1;
        let mut request = SystemOneRequest::new(Entry::from(state.clone()), questions.clone())
            .model(model)
            .timeout(ask.timeout);
        if let Some(key) = &item.request_id {
            let mut headers = HeaderMap::new();
            if let Ok(value) = HeaderValue::from_str(key) {
                headers.insert("idempotency-key", value);
            }
            headers.insert(
                "x-attempt",
                HeaderValue::from_str(&attempt.to_string())
                    .unwrap_or_else(|_| HeaderValue::from_static("1")),
            );
            request = request.headers(headers);
        }
        match client.system_one(request) {
            Ok(response) => {
                let mut answers = Map::new();
                if let Some(ResponseBody::Json(body)) = response.raw().body()
                    && let Some(object) = body.get("answers").and_then(Value::as_object)
                {
                    for (id, answer) in object {
                        if ask.select.is_empty() || ask.select.iter().any(|s| s == id) {
                            answers.insert(id.clone(), answer.clone());
                        }
                    }
                }
                let uncertain = ask.uncertain_below.is_some_and(|below| {
                    answers
                        .values()
                        .any(|answer| winning_probability(answer) < below)
                });
                // The gateway names its request id `x-request-id`; the SDK's
                // own `x-typesafe-request-id` check stays as the fallback.
                let request_id = response
                    .raw()
                    .headers
                    .get("x-request-id")
                    .and_then(|value| value.to_str().ok())
                    .or_else(|| response.request_id());
                let mut value = row(
                    item,
                    "answered",
                    Some(&response.model),
                    request_id,
                    Some(json!({
                        "input_tokens": response.usage.input_tokens,
                        "output_tokens": response.usage.output_tokens,
                    })),
                    None,
                );
                value["answers"] = Value::Object(answers);
                if uncertain {
                    value["uncertain"] = Value::Bool(true);
                }
                return (value, Outcome::Answered);
            }
            Err(jev::Error::Api(error)) => {
                let (code, message) = error_code(&error);
                if CALLER_FAULTS.contains(&code.as_str()) {
                    return (
                        row(
                            item,
                            "refused",
                            None,
                            error_request_id(&error),
                            None,
                            Some((&code, &message)),
                        ),
                        Outcome::Refused,
                    );
                }
                let outcome = match code.as_str() {
                    "idempotency_conflict" | "quota_exhausted" => Outcome::Refused,
                    _ if error.status >= 500 => Outcome::Unavailable,
                    "rate_limited" | "busy" | "overloaded" | "unavailable" | "door_unavailable" => {
                        Outcome::Unavailable
                    }
                    _ => Outcome::Refused,
                };
                let retryable = matches!(outcome, Outcome::Unavailable)
                    && !matches!(
                        code.as_str(),
                        "identity_mismatch" | "registry_unavailable" | "ledger_unavailable"
                    );
                if retryable && attempt <= ask.retries {
                    std::thread::sleep(error.retry_after().unwrap_or(delay).min(MAX_RETRY_AFTER));
                    delay = (delay * 2).min(Duration::from_secs(5));
                    continue;
                }
                return (
                    row(
                        item,
                        outcome.name(),
                        None,
                        error_request_id(&error),
                        None,
                        Some((&code, &message)),
                    ),
                    outcome,
                );
            }
            Err(error @ (jev::Error::Connection { .. } | jev::Error::Timeout { .. })) => {
                if attempt <= ask.retries {
                    std::thread::sleep(delay.min(MAX_RETRY_AFTER));
                    delay = (delay * 2).min(Duration::from_secs(5));
                    continue;
                }
                return (
                    row(
                        item,
                        "unavailable",
                        None,
                        None,
                        None,
                        Some(("unavailable", &error.to_string())),
                    ),
                    Outcome::Unavailable,
                );
            }
            Err(error) => {
                return (
                    row(
                        item,
                        "refused",
                        None,
                        None,
                        None,
                        Some(("invalid_request", &error.to_string())),
                    ),
                    Outcome::Refused,
                );
            }
        }
    }
}

/// One output row.
fn row(
    item: &Item,
    outcome: &str,
    model: Option<&str>,
    request_id: Option<&str>,
    usage: Option<Value>,
    error: Option<(&str, &str)>,
) -> Value {
    let mut value = json!({"id": item.id, "outcome": outcome});
    if let Some(model) = model {
        value["model"] = json!(model);
    }
    if let Some(request_id) = request_id {
        value["request_id"] = json!(request_id);
    }
    if let Some(usage) = usage {
        value["usage"] = usage;
    }
    if let Some((code, message)) = error {
        value["error"] = json!({"code": code, "message": message});
    }
    value
}

/// The request id an error carries — the gateway's `x-request-id`, then the
/// SDK's `x-typesafe-request-id`.
fn error_request_id(error: &jev::ApiError) -> Option<&str> {
    error
        .headers
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .or(error.request_id.as_deref())
}

/// The typed code and message an error body carries, or the status's own.
fn error_code(error: &jev::ApiError) -> (String, String) {
    match &error.body {
        Some(ResponseBody::Json(body)) => {
            let code = body
                .pointer("/error/code")
                .and_then(Value::as_str)
                .map(str::to_string)
                .unwrap_or_else(|| format!("http_{}", error.status));
            (code, error.message())
        }
        _ => (format!("http_{}", error.status), error.message()),
    }
}

/// The probability of whichever outcome won: a Noul's winning side, a
/// Choice or Score's top level. `--uncertain-below` compares against it.
fn winning_probability(answer: &Value) -> f64 {
    if let Some(noul) = answer.get("noul").and_then(Value::as_f64) {
        return noul.max(1.0 - noul);
    }
    answer
        .get("probabilities")
        .and_then(Value::as_object)
        .and_then(|probabilities| {
            probabilities
                .values()
                .filter_map(Value::as_f64)
                .reduce(f64::max)
        })
        .unwrap_or(1.0)
}
