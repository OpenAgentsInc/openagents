//! `openagents computer cloud ...`: cloud computers through the public
//! Computers API (`openagents_sdk::computers`, #11823), signed in as this
//! computer's OpenAgents account.
//!
//! - **One request, one effect.** `create` and `exec` save their request
//!   (`~/.openagents/computers/cloud-requests/`, mode `0600`) before they
//!   send it, and print its key. After a lost answer or a restart, the
//!   same `--key` sends the saved request again, so the service answers
//!   with the computer or command it already made instead of starting a
//!   second one. A key used for different settings is refused before
//!   anything is sent.
//! - **Waiting is not cancelling.** `--wait` bounds how long the command
//!   waits for the computer or command; when it ends, the work keeps
//!   running and the result says so. Only `cancel`, `stop`, and `delete`
//!   change remote work.
//! - **Only what the account can do.** Every call carries the signed-in
//!   session, and the service checks the account on each one. Nothing
//!   here names a Boat sandbox, a GCE instance, or a provider: placement
//!   is the public `auto`, `our-cloud`, or `fast-sandbox`, and an explicit
//!   placement the service can't meet refuses instead of moving the work.
//! - **No secrets.** Command text, its working directory, and its
//!   environment are saved request settings on our servers, so they must
//!   not hold credentials; `exec` takes no `--env` of secret names.
//!
//! The MCP server runs only the read-only verbs (`list`, `show`,
//! `capabilities`, `output`): the command tree declares the others as
//! spending or publishing, and `openagents mcp serve` refuses them.

use std::path::{Path, PathBuf};
use std::time::Duration;

use openagents_sdk::CancellationToken;
use openagents_sdk::computers::{
    CommandOptions, CommandOutcome, CommandRequest, Computer, Computers, CreateOptions,
    CreateRequest, Error, Refusal, WaitFor,
};
use serde_json::{Value, json};

use crate::{Args, Output};

/// The longest `--wait` for a computer to wake, stop, or be deleted.
pub(crate) const MAX_LIFECYCLE_WAIT: u64 = 900;
/// The longest `--wait` for a command; the API's own command limit.
pub(crate) const MAX_COMMAND_WAIT: u64 = 3600;
/// How long `exec` waits by default before it leaves the command running.
const DEFAULT_COMMAND_WAIT: u64 = 60;
/// The most output `exec` collects per stream; the service keeps 256 KiB.
const MAX_OUTPUT: usize = 256 * 1024;

/// Switches the cloud verbs take (no value).
pub(crate) const SWITCHES: &[&str] = &["overwrite"];

/// A failure with a stable kind a caller can branch on, and the request
/// key to retry with when the result is unknown.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Failure {
    pub kind: &'static str,
    pub message: String,
    pub key: Option<String>,
}

impl Failure {
    fn new(kind: &'static str, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            key: None,
        }
    }
    fn document(&self) -> Value {
        let mut value = json!({"error": self.message, "kind": self.kind});
        if let Some(key) = &self.key {
            value["key"] = json!(key);
        }
        value
    }
}

/// What a verb needs: the account's Computers client, the folder its
/// requests are saved in, and the local cancellation (which never cancels
/// remote work).
pub(crate) struct Context<'a> {
    pub api: Computers<'a>,
    pub requests: PathBuf,
    pub cancel: CancellationToken,
}

/// Where requests are saved before they are sent.
fn requests_dir() -> Result<PathBuf, String> {
    let home = user_home::home_os().ok_or("Your home folder couldn't be found.")?;
    Ok(PathBuf::from(home).join(".openagents/computers/cloud-requests"))
}

/// `openagents computer cloud VERB ...`.
pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some(verb) = words.first() else {
        return output.usage(
            "computer cloud",
            "a command is required",
            crate::computer::USAGE,
        );
    };
    let args = match Args::parse(&words[1..], SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("computer cloud", &message, crate::computer::USAGE),
    };
    let requests = match requests_dir() {
        Ok(dir) => dir,
        Err(message) => return output.fail("computer cloud", &message),
    };
    let (client, session) = match crate::agents::connect() {
        Ok(pair) => pair,
        Err(message) => {
            return report(output, &Failure::new("signed_out", message));
        }
    };
    let api = match client.computers(&session) {
        Ok(api) => api,
        Err(error) => return report(output, &failure(&error, None)),
    };
    let context = Context {
        api,
        requests,
        cancel: CancellationToken::new(),
    };
    match crate::runtime().block_on(dispatch(&context, verb, &args)) {
        Ok(value) => {
            output.emit(&value, render);
            0
        }
        Err(failure) => report(output, &failure),
    }
}

fn report(output: &Output, failure: &Failure) -> u8 {
    let mut line = failure.message.clone();
    if let Some(key) = &failure.key {
        line.push_str(&format!(" Request key: {key}."));
    }
    eprintln!("openagents computer cloud: {line}");
    if output.json() {
        println!("{}", failure.document());
    }
    crate::EXIT_FAILURE
}

fn render(value: &Value) -> String {
    serde_json::to_string_pretty(value).unwrap_or_default()
}

/// Run one verb against the account.
///
/// # Errors
/// The typed [`Failure`] the caller reports.
pub(crate) async fn dispatch(
    context: &Context<'_>,
    verb: &str,
    args: &Args,
) -> Result<Value, Failure> {
    let positional = args.positional();
    let id = |index: usize| -> Result<&str, Failure> {
        positional
            .get(index)
            .map(String::as_str)
            .ok_or_else(|| Failure::new("usage", format!("`cloud {verb}` needs a computer ID.")))
    };
    match verb {
        "capabilities" => {
            let capabilities = context
                .api
                .capabilities(&context.cancel)
                .await
                .map_err(|e| failure(&e, None))?;
            Ok(serde_json::to_value(capabilities).unwrap_or(Value::Null))
        }
        "list" => {
            let limit: u16 = args
                .number("limit", 50)
                .map_err(|m| Failure::new("usage", m))?;
            let page = context
                .api
                .list(limit, args.option("after"), &context.cancel)
                .await
                .map_err(|e| failure(&e, None))?;
            Ok(json!({
                "computers": page.computers.iter().map(view).collect::<Vec<_>>(),
                "next_cursor": page.next_cursor,
            }))
        }
        "show" => {
            let reference = reference(context, id(0)?)?;
            let computer = context
                .api
                .get(&reference, &context.cancel)
                .await
                .map_err(|e| failure(&e, None))?;
            Ok(view(&computer))
        }
        "create" => create(context, args).await,
        "exec" => {
            exec(
                context,
                id(0)?,
                &positional[1.min(positional.len())..],
                args,
            )
            .await
        }
        "output" => {
            let reference = reference(context, id(0)?)?;
            let command = positional
                .get(1)
                .ok_or_else(|| Failure::new("usage", "`cloud output` needs a command ID."))?;
            let command = context
                .api
                .command_reference(&reference, command)
                .map_err(|e| failure(&e, None))?;
            let stdout = args
                .number("stdout", 0u64)
                .map_err(|m| Failure::new("usage", m))?;
            let stderr = args
                .number("stderr", 0u64)
                .map_err(|m| Failure::new("usage", m))?;
            let mut reader = context
                .api
                .output_at(&command, stdout, stderr)
                .map_err(|e| failure(&e, None))?;
            let page = reader
                .next_page(&context.api, &context.cancel)
                .await
                .map_err(|e| failure(&e, None))?;
            Ok(match page {
                Some(page) => json!({
                    "command": command_view(page.command.data()),
                    "stdout": String::from_utf8_lossy(&page.stdout),
                    "stderr": String::from_utf8_lossy(&page.stderr),
                    "stdout_gap": page.stdout_gap,
                    "stderr_gap": page.stderr_gap,
                    "next": {"stdout": stdout + page.stdout.len() as u64, "stderr": stderr + page.stderr.len() as u64},
                    "terminal": page.terminal,
                    "has_more": page.has_more,
                }),
                None => json!({"terminal": true, "has_more": false}),
            })
        }
        "cancel" => {
            let reference = reference(context, id(0)?)?;
            let command = positional
                .get(1)
                .ok_or_else(|| Failure::new("usage", "`cloud cancel` needs a command ID."))?;
            let command = context
                .api
                .command_reference(&reference, command)
                .map_err(|e| failure(&e, None))?;
            let command = context
                .api
                .cancel_command(&command, &context.cancel)
                .await
                .map_err(|e| failure(&e, None))?;
            Ok(json!({"cancel_requested": true, "command": command_view(command.data())}))
        }
        "read" => {
            let reference = reference(context, id(0)?)?;
            let path = positional.get(1).ok_or_else(|| {
                Failure::new("usage", "`cloud read` needs a path on the computer.")
            })?;
            let out = args
                .option("out")
                .ok_or_else(|| Failure::new("usage", "`cloud read` needs --out FILE."))?;
            let file = context
                .api
                .read_file(&reference, path, &context.cancel)
                .await
                .map_err(|e| file_failure(&e))?;
            let out = Path::new(out);
            if out.exists() && !args.switch("overwrite") {
                return Err(Failure::new(
                    "file_exists",
                    format!(
                        "{} already exists. Add --overwrite to replace it.",
                        out.display()
                    ),
                ));
            }
            std::fs::write(out, &file.bytes).map_err(|e| {
                Failure::new("local", format!("Couldn't write {}: {e}", out.display()))
            })?;
            Ok(json!({"path": path, "out": out, "size": file.bytes.len(), "sha256": file.sha256}))
        }
        "write" => {
            let reference = reference(context, id(0)?)?;
            let (Some(local), Some(path)) = (positional.get(1), positional.get(2)) else {
                return Err(Failure::new(
                    "usage",
                    "`cloud write` needs a local file and a path on the computer.",
                ));
            };
            let bytes = std::fs::read(local)
                .map_err(|e| Failure::new("local", format!("Couldn't read {local}: {e}")))?;
            let mut intent = context
                .api
                .write_intent(&reference, path, &bytes)
                .map_err(|e| file_failure(&e))?;
            if args.switch("overwrite") {
                intent = intent.allow_overwrite();
            }
            match context
                .api
                .write_file(&intent, &bytes, &context.cancel)
                .await
            {
                Ok(_) => {
                    Ok(json!({"path": path, "size": intent.size(), "sha256": intent.sha256()}))
                }
                Err(error @ (Error::Uncertain { .. } | Error::Network { .. })) => {
                    // A lost answer can mean the write landed: compare, never resend.
                    match context.api.reconcile_write(&intent, &context.cancel).await {
                        Ok(Some(_)) => Ok(
                            json!({"path": path, "size": intent.size(), "sha256": intent.sha256(), "observed": true}),
                        ),
                        _ => Err(file_failure(&error)),
                    }
                }
                Err(error) => Err(file_failure(&error)),
            }
        }
        "stop" | "wake" | "delete" => {
            let reference = reference(context, id(0)?)?;
            let wait = wait(args, 0, MAX_LIFECYCLE_WAIT)?;
            let (result, target) = match verb {
                "stop" => (
                    context
                        .api
                        .stop(&reference, &context.cancel)
                        .await
                        .map(Some),
                    WaitFor::Stopped,
                ),
                "wake" => (
                    context
                        .api
                        .wake(&reference, &context.cancel)
                        .await
                        .map(Some),
                    WaitFor::Awake,
                ),
                _ => (
                    context
                        .api
                        .delete(&reference, &context.cancel)
                        .await
                        .map(|()| None),
                    WaitFor::Deleted,
                ),
            };
            let accepted = result.map_err(|e| failure(&e, None))?;
            let mut value = json!({"id": reference.id(), "requested": verb});
            if let Some(computer) = &accepted {
                value["computer"] = view(computer);
            }
            if wait > 0 {
                match context
                    .api
                    .wait(
                        &reference,
                        target,
                        Duration::from_secs(wait),
                        &context.cancel,
                    )
                    .await
                {
                    Ok(computer) => value["computer"] = view(&computer),
                    Err(Error::WaitTimedOut) => {
                        value["waiting"] = json!(format!(
                            "The {verb} is still in progress. Check it with `openagents computer cloud show {}`.",
                            reference.id()
                        ));
                    }
                    // A deleted computer can stop answering reads.
                    Err(error) if verb == "delete" && error.refusal() == Some(Refusal::Missing) => {
                        value["deleted"] = json!(true);
                    }
                    Err(error) => return Err(failure(&error, None)),
                }
            }
            Ok(value)
        }
        other => Err(Failure::new(
            "usage",
            format!("`cloud {other}` isn't a command. Run `openagents computer --help`."),
        )),
    }
}

fn reference(
    context: &Context<'_>,
    id: &str,
) -> Result<openagents_sdk::computers::ComputerRef, Failure> {
    context.api.reference(id).map_err(|_| {
        Failure::new(
            "usage",
            format!("{id} isn't a cloud computer ID. IDs look like cmp_ and 20 hex digits; `openagents computer cloud list` shows them."),
        )
    })
}

fn wait(args: &Args, fallback: u64, max: u64) -> Result<u64, Failure> {
    let wait = args
        .number("wait", fallback)
        .map_err(|m| Failure::new("usage", m))?;
    if wait > max {
        return Err(Failure::new(
            "usage",
            format!("--wait is at most {max} seconds."),
        ));
    }
    Ok(wait)
}

fn view(computer: &Computer) -> Value {
    let data = computer.data();
    json!({
        "id": data.id,
        "name": data.name,
        "size": data.size,
        "state": data.state,
        "desired": data.desired,
        "online": data.online,
        "awake": computer.awake(),
        "capabilities": data.capabilities.as_ref().and_then(|c| serde_json::to_value(c).ok()),
        "last_error": data.last_error,
    })
}

fn command_view(data: &openagents_sdk::generated::computers::AppCommand) -> Value {
    json!({
        "id": data.id,
        "computer": data.computer,
        "state": data.state,
        "exit_code": data.exit_code,
        "cancel_requested": data.cancel_requested,
        "stdout_truncated": data.stdout_truncated,
        "stderr_truncated": data.stderr_truncated,
    })
}

// Saved requests.

fn saved_path(dir: &Path, kind: &str, key: &str) -> PathBuf {
    dir.join(format!("{kind}-{key}.json"))
}

/// Read the request saved under `key`, if any.
fn load<T: serde::de::DeserializeOwned>(
    dir: &Path,
    kind: &str,
    key: &str,
) -> Result<Option<T>, Failure> {
    let path = saved_path(dir, kind, key);
    match std::fs::read(&path) {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|_| {
            Failure::new(
                "local",
                format!("The saved request {} couldn't be read.", path.display()),
            )
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(Failure::new(
            "local",
            format!("Couldn't read {}: {error}", path.display()),
        )),
    }
}

/// Save a request before it is sent: private, created once, synced.
fn save<T: serde::Serialize>(
    dir: &Path,
    kind: &str,
    key: &str,
    request: &T,
) -> Result<(), Failure> {
    use std::io::Write;
    let fail = |error: std::io::Error| {
        Failure::new("local", format!("Couldn't save the request: {error}"))
    };
    std::fs::create_dir_all(dir).map_err(fail)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let bytes = serde_json::to_vec(request)
        .map_err(|_| Failure::new("local", "Couldn't save the request."))?;
    let mut file = options.open(saved_path(dir, kind, key)).map_err(fail)?;
    file.write_all(&bytes)
        .and_then(|()| file.sync_all())
        .map_err(fail)
}

/// The settings a saved request was made with, for comparing a retry.
fn settings<T: serde::Serialize>(value: &T) -> Value {
    let mut value = serde_json::to_value(value).unwrap_or(Value::Null);
    if let Some(object) = value.as_object_mut() {
        object.remove("idempotency_key");
    }
    value
}

async fn create(context: &Context<'_>, args: &Args) -> Result<Value, Failure> {
    let placement = args.option("placement");
    if let Some(placement) = placement
        && !matches!(placement, "auto" | "our-cloud" | "fast-sandbox")
    {
        return Err(Failure::new(
            "usage",
            "--placement is auto, our-cloud, or fast-sandbox.",
        ));
    }
    let (environment, version) = (args.option("environment"), args.option("version"));
    if environment.is_some() != version.is_some() {
        return Err(Failure::new(
            "usage",
            "Pass --environment and --version together.",
        ));
    }
    let wait = wait(args, 0, MAX_LIFECYCLE_WAIT)?;
    let options = CreateOptions {
        size: args.option("size").map(str::to_owned),
        placement: placement.map(str::to_owned),
        environment: environment.map(str::to_owned),
        version: version.map(str::to_owned),
        idempotency_key: args.option("key").map(str::to_owned),
        ..CreateOptions::default()
    };
    let request: CreateRequest = match args.option("key") {
        Some(key) => match load::<CreateRequest>(&context.requests, "create", key)? {
            Some(saved) => {
                let given = ["size", "placement", "environment", "version"]
                    .iter()
                    .any(|name| args.option(name).is_some());
                let fresh = context
                    .api
                    .create_request(options)
                    .map_err(|e| failure(&e, None))?;
                if given && settings(&fresh_body(&fresh)) != settings(&fresh_body(&saved)) {
                    return Err(conflict(key));
                }
                saved
            }
            None => {
                let request = context
                    .api
                    .create_request(options)
                    .map_err(|_| Failure::new("usage", "--key takes 1 to 128 letters, digits, `_`, `-`, or `.`, not starting with `.`."))?;
                save(&context.requests, "create", key, &request)?;
                request
            }
        },
        None => {
            let request = context
                .api
                .create_request(options)
                .map_err(|e| failure(&e, None))?;
            save(
                &context.requests,
                "create",
                request.key().as_str(),
                &request,
            )?;
            request
        }
    };
    let key = request.key().as_str().to_owned();
    let computer = context
        .api
        .create(&request, &context.cancel)
        .await
        .map_err(|e| failure(&e, Some(&key)))?;
    let mut value = json!({"key": key, "computer": view(&computer)});
    if wait > 0 && !computer.awake() {
        match context
            .api
            .wait(
                computer.reference(),
                WaitFor::Awake,
                Duration::from_secs(wait),
                &context.cancel,
            )
            .await
        {
            Ok(awake) => value["computer"] = view(&awake),
            Err(Error::WaitTimedOut) => {
                value["waiting"] = json!("The computer is still starting. Check it with `show`.");
            }
            Err(error) => return Err(failure(&error, Some(&key))),
        }
    }
    Ok(value)
}

/// The create body a request carries, read back through its own encoding.
fn fresh_body(request: &CreateRequest) -> Value {
    serde_json::to_value(request)
        .ok()
        .and_then(|mut v| v.get_mut("body").map(Value::take))
        .unwrap_or(Value::Null)
}

fn command_body(request: &CommandRequest) -> Value {
    serde_json::to_value(request.options()).unwrap_or(Value::Null)
}

fn conflict(key: &str) -> Failure {
    Failure::new(
        "key_conflict",
        format!(
            "The key {key} was saved for different settings, so nothing was sent. Use a new key, or rerun with only --key {key} to check the saved request."
        ),
    )
}

/// `exec ID [--cwd DIR] [--timeout S] [--key KEY] [--wait S] -- CMD...`.
async fn exec(
    context: &Context<'_>,
    id: &str,
    words: &[String],
    args: &Args,
) -> Result<Value, Failure> {
    let computer = reference(context, id)?;
    let wait = wait(args, DEFAULT_COMMAND_WAIT, MAX_COMMAND_WAIT)?;
    let timeout: Option<u64> = args
        .option("timeout")
        .map(|t| t.parse::<u64>())
        .transpose()
        .map_err(|_| Failure::new("usage", "--timeout takes seconds."))?;
    if timeout.is_some_and(|t| t == 0 || t > MAX_COMMAND_WAIT) {
        return Err(Failure::new(
            "usage",
            format!("--timeout is 1 to {MAX_COMMAND_WAIT} seconds."),
        ));
    }
    let text = match words {
        [] => None,
        [one] => Some(one.clone()),
        many => Some(
            many.iter()
                .map(|w| shell_word(w))
                .collect::<Vec<_>>()
                .join(" "),
        ),
    };
    let options = |text: String| CommandOptions {
        command: text,
        cwd: args.option("cwd").map(str::to_owned),
        timeout_seconds: timeout,
        idempotency_key: args.option("key").map(str::to_owned),
        ..CommandOptions::default()
    };
    let request: CommandRequest = match (args.option("key"), text) {
        (Some(key), text) => match load::<CommandRequest>(&context.requests, "command", key)? {
            Some(saved) => {
                if saved.computer() != &computer {
                    return Err(conflict(key));
                }
                if let Some(text) = text {
                    let fresh = context
                        .api
                        .command_request(&computer, options(text))
                        .map_err(|e| failure(&e, None))?;
                    if settings(&command_body(&fresh)) != settings(&command_body(&saved)) {
                        return Err(conflict(key));
                    }
                }
                saved
            }
            None => {
                let text = text.ok_or_else(|| {
                    Failure::new(
                        "usage",
                        format!("No request is saved under {key}. Give the command after --."),
                    )
                })?;
                let request = context
                    .api
                    .command_request(&computer, options(text))
                    .map_err(|_| Failure::new("usage", "Check the command, --cwd, and --key."))?;
                save(&context.requests, "command", key, &request)?;
                request
            }
        },
        (None, Some(text)) => {
            let request = context
                .api
                .command_request(&computer, options(text))
                .map_err(|_| Failure::new("usage", "Check the command and --cwd."))?;
            save(
                &context.requests,
                "command",
                request.key().as_str(),
                &request,
            )?;
            request
        }
        (None, None) => {
            return Err(Failure::new(
                "usage",
                "`cloud exec` needs the command after --.",
            ));
        }
    };
    let key = request.key().as_str().to_owned();
    let command = context
        .api
        .execute(&request, &context.cancel)
        .await
        .map_err(|e| failure(&e, Some(&key)))?;
    let mut value = json!({"key": key, "command": command_view(command.data())});
    let finished = if command.outcome().is_some() {
        Some(command)
    } else if wait == 0 {
        None
    } else {
        match context
            .api
            .wait_command(
                command.reference(),
                Duration::from_secs(wait),
                &context.cancel,
            )
            .await
        {
            Ok(done) => Some(done),
            Err(Error::WaitTimedOut) => None,
            Err(error) => return Err(failure(&error, Some(&key))),
        }
    };
    let Some(done) = finished else {
        value["running"] = json!(true);
        value["note"] = json!(format!(
            "The command is still running; waiting ended without stopping it. Read it with `openagents computer cloud output {id} {}`, or stop it with `cancel`.",
            command_id(&value)
        ));
        return Ok(value);
    };
    value["command"] = command_view(done.data());
    value["outcome"] = json!(match done.outcome() {
        Some(CommandOutcome::Succeeded) => "succeeded",
        Some(CommandOutcome::Failed) => "failed",
        Some(CommandOutcome::Cancelled) => "cancelled",
        Some(CommandOutcome::TimedOut) => "timed_out",
        Some(CommandOutcome::Unknown) | None => "unknown",
    });
    let mut reader = context
        .api
        .output(done.reference())
        .map_err(|e| failure(&e, Some(&key)))?;
    let (mut stdout, mut stderr, mut gap) = (Vec::new(), Vec::new(), false);
    while stdout.len() < MAX_OUTPUT && stderr.len() < MAX_OUTPUT {
        match reader.next_page(&context.api, &context.cancel).await {
            Ok(Some(page)) => {
                stdout.extend_from_slice(&page.stdout);
                stderr.extend_from_slice(&page.stderr);
                gap |= page.stdout_gap || page.stderr_gap;
            }
            Ok(None) => break,
            Err(error)
                if error.refusal() == Some(Refusal::Missing) || error.status() == Some(410) =>
            {
                value["output_expired"] = json!(true);
                break;
            }
            Err(error) => return Err(failure(&error, Some(&key))),
        }
    }
    value["stdout"] = json!(String::from_utf8_lossy(&stdout));
    value["stderr"] = json!(String::from_utf8_lossy(&stderr));
    if gap {
        value["truncated"] = json!(true);
    }
    Ok(value)
}

fn command_id(value: &Value) -> String {
    value["command"]["id"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// One word for `sh`, quoted when it needs to be.
fn shell_word(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./=:@%+,".contains(&b))
    {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
}

/// A failure of a file transfer, with the file-specific refusals named.
fn file_failure(error: &Error) -> Failure {
    match (error, error.code()) {
        (Error::Unsupported, _) => Failure::new(
            "unsupported",
            "This computer doesn't offer file transfers. Use `cloud exec` instead.",
        ),
        (Error::TooLarge, _) => Failure::new("too_large", "Files are at most 4 MiB each."),
        (Error::InvalidInput, _) => Failure::new(
            "usage",
            "Paths are relative to the computer's workspace, with no `..`, and files are at most 4 MiB.",
        ),
        (_, Some("file_exists")) => Failure::new(
            "file_exists",
            "A different file is already at that path. Add --overwrite to replace it.",
        ),
        (_, Some("file_changed")) => Failure::new(
            "file_changed",
            "The file changed while it was being copied. Try again.",
        ),
        _ => failure(error, None),
    }
}

/// The typed failure for an SDK error. `key` is the saved request to
/// retry with when the result is unknown.
pub(crate) fn failure(error: &Error, key: Option<&str>) -> Failure {
    let retry = |message: &str| match key {
        Some(key) => format!(
            "{message} Rerun with --key {key} to check the same request; nothing new is started."
        ),
        None => message.to_owned(),
    };
    let kind_message: (&'static str, String) = match error {
        Error::Uncertain { .. } | Error::Network { .. } => (
            "uncertain",
            retry("We couldn't confirm whether the request went through."),
        ),
        Error::Session(openagents_sdk::Error::ExpiredSession | openagents_sdk::Error::Unauthorized) => (
            "signed_out",
            retry("Your sign-in expired. Log in again with `coder login`, then try again."),
        ),
        Error::Cancelled => ("cancelled", "Stopped waiting. Remote work keeps running.".into()),
        Error::WaitTimedOut => (
            "wait_timed_out",
            "Waiting ended. The work keeps running; check it again.".into(),
        ),
        _ => match (error.refusal(), error.status(), error.code()) {
            (Some(Refusal::Authentication), _, _) | (_, Some(401), _) => {
                (
                    "signed_out",
                    retry("Your sign-in expired. Log in again with `coder login`, then try again."),
                )
            }
            (Some(Refusal::Unsupported), _, _) | (_, Some(422), Some("unsupported")) => (
                "unsupported",
                "That isn't available for this computer or placement, so nothing was started. `openagents computer cloud capabilities` lists what you can use.".into(),
            ),
            (Some(Refusal::Quota), _, _) => (
                "quota",
                "You've reached your computer limit. Delete a computer you don't need, then try again.".into(),
            ),
            (Some(Refusal::RateLimited), _, _) => (
                "rate_limited",
                retry(&match error.retry_after() {
                    Some(after) => format!("Too many requests. Try again in {} seconds.", after.as_secs().max(1)),
                    None => "Too many requests. Try again later.".to_owned(),
                }),
            ),
            (Some(Refusal::AccessDenied), _, _) => (
                "access_denied",
                "This account can't use cloud computers or that computer.".into(),
            ),
            (Some(Refusal::Missing), _, _) => (
                "missing",
                "That computer or command isn't on your account anymore.".into(),
            ),
            (Some(Refusal::PaymentRequired), _, _) => (
                "payment_required",
                "This needs payment, and none was sent.".into(),
            ),
            (Some(Refusal::Conflict), _, Some("idempotency_conflict")) => (
                "key_conflict",
                "That key already names a different request. Use a new key.".into(),
            ),
            (Some(Refusal::Conflict), _, Some("computer_not_ready")) => (
                "not_awake",
                "The computer isn't awake. Wake it with `openagents computer cloud wake ID --wait 300`, then try again.".into(),
            ),
            (Some(Refusal::Unavailable), _, _) => (
                "unavailable",
                retry("The Computers service is unavailable right now."),
            ),
            _ => ("refused", error.to_string()),
        },
    };
    Failure {
        kind: kind_message.0,
        message: kind_message.1,
        key: key.map(str::to_owned),
    }
}

#[cfg(test)]
#[path = "computer_cloud_tests.rs"]
mod tests;
