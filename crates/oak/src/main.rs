//! `oak` — the caller's CLI for the decision API.
//!
//! `oak ask` sends one state, or a bounded batch of them, through
//! `POST /v1/systemone` and writes one JSON object per row to standard
//! output. `oak models` lists the doors the credential can reach.
//!
//! ```text
//! oak ask --questions PATH [--input state|lines|ndjson] [STATE] [FLAGS]
//! oak models [FLAGS]
//! ```
//!
//! Credentials come from `OPENAGENTS_API_KEY` or the `api_key` field of a
//! protected config file — never a flag, so a key never lands in a process
//! list. The endpoint comes from `--url`, `OPENAGENTS_BASE_URL`, or the
//! file's `base_url`. Read `docs/decision-models/caller.md`.

use std::io::{BufWriter, IsTerminal, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use jev::{
    BlockingClient, Config, Entry, Question, Questions, ResponseBody, RetryPolicy, SystemOneRequest,
};
use reqwest::header::{HeaderMap, HeaderValue};
use serde_json::{Map, Value, json};

/// Every row answered.
const EXIT_ANSWERED: i32 = 0;
/// The run itself failed: bad credentials, an unbound door, a malformed
/// envelope — the fix is the caller's, not a retry's.
const EXIT_FAILURE: i32 = 1;
/// The command line or its inputs did not parse.
const EXIT_USAGE: i32 = 2;
/// No row answered and at least one was refused.
const EXIT_REFUSED: i32 = 3;
/// No row answered or refused and at least one was unavailable or invalid.
const EXIT_UNAVAILABLE: i32 = 4;
/// Some mix of the above.
const EXIT_MIXED: i32 = 5;

/// A response error the caller must fix rather than retry: credentials,
/// bindings, and envelope shape apply to every row identically, so the run
/// stops on the first.
const CALLER_FAULTS: &[&str] = &[
    "unauthenticated",
    "door_not_bound",
    "malformed",
    "invalid_request",
    "too_many_questions",
    "too_many_options",
];

/// The longest a `Retry-After` is honored before the attempt goes anyway.
const MAX_RETRY_AFTER: Duration = Duration::from_secs(60);

fn usage() -> ! {
    eprintln!(
        "usage:\n  \
         oak ask --questions PATH [--input state|lines|ndjson] [STATE] [FLAGS]\n  \
         oak models [FLAGS]\n\n  \
         flags:\n    \
         --url URL              service root (OPENAGENTS_BASE_URL, or `base_url` in the file)\n    \
         --model DOOR           door to ask (OPENAGENTS_MODEL, or `model` in the file)\n    \
         --config PATH          credential file (OPENAGENTS_CONFIG,\n    \
         \x20                       default ~/.config/openagents/oak.json)\n    \
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

/// Settings from the flag, the environment, or the config file, in that
/// order. `api_key` is never a flag.
#[derive(Debug, Default, serde::Deserialize)]
struct FileConfig {
    api_key: Option<String>,
    base_url: Option<String>,
    model: Option<String>,
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

fn main() {
    let mut args = std::env::args().skip(1);
    let Some(verb) = args.next() else {
        usage();
    };
    let mut url = None;
    let mut config = None;
    let mut quiet = false;
    let mut timeout = Duration::from_secs(60);
    match verb.as_str() {
        "models" => {
            while let Some(flag) = args.next() {
                match flag.as_str() {
                    "--url" => url = args.next(),
                    "--config" => config = args.next().map(PathBuf::from),
                    "--timeout" => timeout = seconds(args.next()),
                    "--quiet" => quiet = true,
                    _ => usage(),
                }
            }
            std::process::exit(models(url, config, timeout, quiet));
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
        timeout,
        request_id: None,
        select: Vec::new(),
        uncertain_below: None,
        url: None,
        model: None,
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

/// Endpoint and credential, resolved.
struct Settings {
    api_key: String,
    base_url: String,
    model: Option<String>,
}

impl Settings {
    fn resolve(
        url: Option<String>,
        model: Option<String>,
        config: Option<PathBuf>,
    ) -> Result<Self, String> {
        let file = load_config(config)?;
        let base_url = url
            .or_else(|| env("OPENAGENTS_BASE_URL"))
            .or(file.base_url)
            .ok_or_else(|| {
                "no service root: pass --url, set OPENAGENTS_BASE_URL, or name \
                 `base_url` in the config file"
                    .to_string()
            })?;
        let api_key = env("OPENAGENTS_API_KEY").or(file.api_key).ok_or_else(|| {
            "no credential: set OPENAGENTS_API_KEY or name `api_key` in the config \
             file — a key never goes on the command line"
                .to_string()
        })?;
        let model = model.or_else(|| env("OPENAGENTS_MODEL")).or(file.model);
        Ok(Self {
            api_key,
            base_url,
            model,
        })
    }

    fn client(&self, timeout: Duration) -> Result<BlockingClient, String> {
        // Retries are oak's own loop: the service settles quota by
        // (request, attempt), and only oak can bump `x-attempt`.
        BlockingClient::new(
            Config::new()
                .api_key(self.api_key.clone())
                .base_url(self.base_url.clone())
                .timeout(timeout)
                .retry(RetryPolicy {
                    max_retries: 0,
                    ..RetryPolicy::default()
                }),
        )
        .map_err(|error| error.to_string())
    }
}

/// One environment value, trimmed; an empty one reads as unset.
fn env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

/// Read the config file. A file that does not exist is no file; a file a
/// group or world permission can read is refused rather than trusted.
fn load_config(path: Option<PathBuf>) -> Result<FileConfig, String> {
    let path = path
        .or_else(|| env("OPENAGENTS_CONFIG").map(PathBuf::from))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join(".config/openagents/oak.json"))
        });
    let Some(path) = path else {
        return Ok(FileConfig::default());
    };
    if !path.exists() {
        return Ok(FileConfig::default());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path)
            .map_err(|error| format!("cannot stat {}: {error}", path.display()))?
            .permissions()
            .mode();
        if mode & 0o077 != 0 {
            return Err(format!(
                "{} is readable by group or others; run `chmod 600 {}`",
                path.display(),
                path.display()
            ));
        }
    }
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|error| format!("{} does not parse: {error}", path.display()))
}

/// The `models` verb: the doors this credential is authorized for, one card
/// per line.
fn models(url: Option<String>, config: Option<PathBuf>, timeout: Duration, quiet: bool) -> i32 {
    let settings = match Settings::resolve(url, None, config) {
        Ok(settings) => settings,
        Err(message) => return fatal(&message),
    };
    let client = match settings.client(timeout) {
        Ok(client) => client,
        Err(message) => return fatal(&message),
    };
    match client.list_models(jev::ListOptions::new()) {
        Ok(cards) => {
            let mut out = BufWriter::new(std::io::stdout().lock());
            for card in &cards {
                if writeln!(out, "{}", serde_json::to_string(card).unwrap_or_default()).is_err() {
                    return EXIT_ANSWERED;
                }
            }
            if !quiet {
                eprintln!("oak: {} doors", cards.len());
            }
            EXIT_ANSWERED
        }
        Err(error) => fatal(&error.to_string()),
    }
}

fn fatal(message: &str) -> i32 {
    eprintln!("oak: {message}");
    EXIT_FAILURE
}

fn run(ask: Ask) -> i32 {
    let started = Instant::now();
    let settings = match Settings::resolve(ask.url.clone(), ask.model.clone(), ask.config.clone()) {
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
    client: &BlockingClient,
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
