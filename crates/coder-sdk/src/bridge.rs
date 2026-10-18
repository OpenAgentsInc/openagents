//! The hook bridge (#11500): one hook program for a harness that only runs
//! shell command hooks (the "command" tier).
//!
//! The harness runs the program on an event with its hook input on
//! standard input. The program asks the session's in-process registry,
//! held by the host that started the session ([`Host`], at the path named
//! by [`SOCKET_VAR`]: a Unix socket, or on Windows a file naming a loopback
//! port and the token that port asks for), and prints the reply the harness expects. So a
//! Codex-style `-c hooks.PreToolUse=...` harness gets the whole registry,
//! composed by the same rules, instead of one hard-wired hook.
//!
//! When no host answers (the variable is unset, the host is gone, or it is
//! too slow), the program's own fallback answers, such as the floors'
//! table, so a floor is never lost to a missing host.
//!
//! Formats: Claude Code's hook input and output, and Codex's, which reads
//! the same input and accepts a narrower output (no `ask`, `allow` only
//! with `updatedInput`, no `continue: false` before a tool).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
#[cfg(unix)]
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use serde_json::{Value, json};

use super::compose::Runner;
use super::events::Event;

/// The variable naming the host's socket, set on the harness's
/// environment so its hook processes inherit it.
pub const SOCKET_VAR: &str = coder_delegate::delegate::HOOK_SOCKET_VAR;
/// The Coder subcommand that runs the bridge: `coder coder-hook --format
/// claude|codex|acp`.
pub const COMMAND: &str = "coder-hook";
/// How long the bridge waits for the host.
pub const WAIT: Duration = Duration::from_secs(60);

/// Which harness's hook protocol.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    Claude,
    Codex,
    /// An ACP agent's own hooks (#11508): Claude Code's hook input with the
    /// agent's tool names (Grok Build's `run_terminal_command`, Devin's
    /// `exec`), read as Claude Code's ([`super::acp::claude_tool`]); the
    /// reply is narrowed as Codex's is, since nobody can answer from
    /// inside the agent and a rewrite in Claude Code's shapes would not fit
    /// the agent's tool.
    Acp,
    /// OpenCode as a Coder harness (#11507): the call Coder's OpenCode
    /// plugin sends before a tool ([`crate::opencode`]).
    OpenCode,
}

impl Format {
    #[must_use]
    pub fn from_name(name: &str) -> Option<Format> {
        match name {
            "claude" => Some(Format::Claude),
            "codex" => Some(Format::Codex),
            "acp" => Some(Format::Acp),
            "opencode" => Some(Format::OpenCode),
            _ => None,
        }
    }

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Format::Claude => "claude",
            Format::Codex => "codex",
            Format::Acp => "acp",
            Format::OpenCode => "opencode",
        }
    }
}

/// The event a hook input is for.
#[must_use]
pub fn event_of(input: &Value) -> Option<Event> {
    match input["hook_event_name"].as_str()? {
        "CwdChanged" => Some(Event::FileChanged),
        name => Event::from_name(name),
    }
}

/// The registry's answer to one hook input, in `format`; `None` lets the
/// harness go on unchanged (the program prints nothing).
pub fn answer(runner: &mut Runner, format: Format, input: &Value) -> Option<Value> {
    if format == Format::OpenCode {
        let claude = super::opencode::to_claude(input);
        let output = answer(runner, Format::Codex, &claude);
        return super::opencode::reply(input, output.as_ref());
    }
    let event = event_of(input)?;
    let input = &normalized(format, input);
    let applied = runner.fire(&super::claude::call(event, input));
    let output = super::claude::output(&applied);
    if output == claude_agent_sdk::SyncHookJSONOutput::continue_execution() {
        return None;
    }
    let output = serde_json::to_value(output).ok()?;
    match format {
        Format::Claude => Some(output),
        Format::Codex => codex(event, output, &applied.input),
        Format::Acp => {
            let mut output = codex(event, output, &applied.input)?;
            // A rewrite is in Claude Code's shapes, which the agent's tool
            // would not read: the call goes as the agent asked, and only a
            // denial or added context is sent.
            if let Some(specific) = output
                .get_mut("hookSpecificOutput")
                .and_then(Value::as_object_mut)
                && specific.remove("updatedInput").is_some()
                && specific.get("permissionDecision") == Some(&json!("allow"))
            {
                specific.remove("permissionDecision");
            }
            let empty = output["hookSpecificOutput"]
                .as_object()
                .is_some_and(|fields| fields.len() <= 1);
            if empty && let Some(object) = output.as_object_mut() {
                object.remove("hookSpecificOutput");
            }
            output
                .as_object()
                .is_some_and(|o| !o.is_empty())
                .then_some(output)
        }
        Format::OpenCode => None,
    }
}

/// The hook input as the registry reads it: an ACP agent's tool name and
/// input in Claude Code's, with the session's folder on a command.
#[must_use]
pub fn normalized(format: Format, input: &Value) -> Value {
    if format != Format::Acp {
        return input.clone();
    }
    let mut input = input.clone();
    if let Some(tool) = input["tool_name"].as_str().map(str::to_owned) {
        let (tool, mut tool_input) = super::acp::claude_tool(&tool, &input["tool_input"]);
        if tool == super::acp::COMMAND_TOOL
            && tool_input.get("cwd").and_then(Value::as_str).is_none()
            && let Some(cwd) = input["cwd"].as_str()
        {
            tool_input["cwd"] = json!(cwd);
        }
        input["tool_name"] = json!(tool);
        input["tool_input"] = tool_input;
    }
    input
}

/// Claude Code's reply narrowed to what Codex accepts. Codex has no one to
/// ask, so an ask is held: denied with its step for the person. `allow`
/// alone is dropped (Codex fails it open); a rewrite is sent as Codex
/// wants it, `allow` with `updatedInput`, after the floors judged it.
/// Before a tool, ending the session becomes a denial with its reason.
fn codex(event: Event, mut output: Value, input: &Value) -> Option<Value> {
    let object = output.as_object_mut()?;
    if event == Event::PreToolUse {
        let ended = object.remove("continue") == Some(json!(false));
        let stop_reason = object.remove("stopReason");
        let specific = object
            .entry("hookSpecificOutput")
            .or_insert_with(|| json!({"hookEventName": "PreToolUse"}));
        match specific["permissionDecision"].as_str() {
            Some("ask") => {
                let step = specific["permissionDecisionReason"]
                    .as_str()
                    .unwrap_or("This step needs the person.")
                    .to_owned();
                specific["permissionDecision"] = json!("deny");
                specific["permissionDecisionReason"] = json!(held(&step));
                // Codex fails a denial with an input open.
                if let Some(fields) = specific.as_object_mut() {
                    fields.remove("updatedInput");
                }
            }
            Some("allow") if specific.get("updatedInput").is_none() => {
                if let Some(fields) = specific.as_object_mut() {
                    fields.remove("permissionDecision");
                }
            }
            None if specific.get("updatedInput").is_some() => {
                specific["permissionDecision"] = json!("allow");
                specific["updatedInput"] = input.clone();
            }
            _ => {}
        }
        if ended && specific["permissionDecision"] != json!("deny") {
            specific["permissionDecision"] = json!("deny");
            specific["permissionDecisionReason"] =
                stop_reason.unwrap_or_else(|| json!("A Coder hook ended the session."));
            if let Some(fields) = specific.as_object_mut() {
                fields.remove("updatedInput");
            }
        }
        if specific.as_object().is_some_and(|fields| fields.len() == 1) {
            object.remove("hookSpecificOutput");
        }
    }
    (!object.is_empty()).then_some(output)
}

/// What a held step reads where nobody can answer.
fn held(step: &str) -> String {
    if step.starts_with("Held for the person") {
        step.to_owned()
    } else {
        format!(
            "Held for the person: {step} Nobody can answer from here, so hand this step back to them."
        )
    }
}

/// The host side: answers the bridge from one session's runner, until
/// dropped: on a Unix socket, or where there is none (Windows) on a
/// loopback port that only answers a caller holding its token.
pub struct Host {
    path: PathBuf,
    /// The loopback port, when the host isn't on a Unix socket.
    port: Option<u16>,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

static NEXT_SOCKET: AtomicU64 = AtomicU64::new(1);

impl Host {
    /// Starts answering in a new socket under the temp directory.
    ///
    /// # Errors
    ///
    /// When the socket can't be made.
    pub fn start(runner: Arc<Mutex<Runner>>) -> std::io::Result<Host> {
        let path = std::env::temp_dir().join(format!(
            "coder-hook-{}-{}.sock",
            std::process::id(),
            NEXT_SOCKET.fetch_add(1, Ordering::Relaxed)
        ));
        Self::start_at(runner, path)
    }

    /// Starts answering on `path`: a Unix socket where the platform has
    /// them, else a loopback port named in a file at `path`.
    ///
    /// # Errors
    ///
    /// When the socket can't be made.
    pub fn start_at(runner: Arc<Mutex<Runner>>, path: PathBuf) -> std::io::Result<Host> {
        #[cfg(unix)]
        {
            Self::start_unix(runner, path)
        }
        #[cfg(not(unix))]
        {
            Self::start_loopback(runner, path)
        }
    }

    #[cfg(unix)]
    fn start_unix(runner: Arc<Mutex<Runner>>, path: PathBuf) -> std::io::Result<Host> {
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path)?;
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::Builder::new()
            .name("coder hook host".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let runner = runner.clone();
                    // One connection per hook firing; each answers alone.
                    let _ = std::thread::Builder::new()
                        .name("coder hook".into())
                        .spawn(move || serve_one::<UnixStream>(&runner, &stream, None));
                }
            })?;
        Ok(Host {
            path,
            port: None,
            stop,
            thread: Some(thread),
        })
    }

    /// The host on a loopback port: the file at `path` holds
    /// `127.0.0.1:PORT TOKEN`, and a request without the token is
    /// answered with nothing. The file is made only by this user (the
    /// temporary folder is theirs), so the token is the access check.
    pub(crate) fn start_loopback(
        runner: Arc<Mutex<Runner>>,
        path: PathBuf,
    ) -> std::io::Result<Host> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let token = loopback_token()?;
        let _ = std::fs::remove_file(&path);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        writeln!(file, "{address} {token}")?;
        drop(file);
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let thread = std::thread::Builder::new()
            .name("coder hook host".into())
            .spawn(move || {
                let token = Arc::new(token);
                for stream in listener.incoming() {
                    if stopping.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(stream) = stream else { continue };
                    let runner = runner.clone();
                    let token = token.clone();
                    let _ = std::thread::Builder::new()
                        .name("coder hook".into())
                        .spawn(move || serve_one::<TcpStream>(&runner, &stream, Some(&token)));
                }
            })?;
        Ok(Host {
            path,
            port: Some(address.port()),
            stop,
            thread: Some(thread),
        })
    }

    /// The socket, for [`SOCKET_VAR`].
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Starts a host answering from `registry` and installs the bridge
/// (`program coder-hook`) as `cli`'s own `PreToolUse` hook (#11509): Claude
/// Code's `--settings`, Codex's `-c hooks.PreToolUse`, and the host's socket
/// in the session's environment. Keep the host while the session runs.
/// `None`, with `cli` unchanged, when the socket can't be made.
pub fn install(
    cli: &mut coder_delegate::delegate::Cli,
    registry: super::registry::Registry,
    program: PathBuf,
) -> Option<Host> {
    let runner = Runner::new(registry, super::compose::Budget::default());
    let host = Host::start(Arc::new(Mutex::new(runner))).ok()?;
    cli.hooks = Some(hook_bridge(&host, program));
    Some(host)
}

/// The bridge a delegated CLI runs for `host`: `program coder-hook`.
#[must_use]
pub fn hook_bridge(host: &Host, program: PathBuf) -> coder_delegate::delegate::HookBridge {
    coder_delegate::delegate::HookBridge {
        program,
        args: vec![COMMAND.to_owned()],
        socket: host.path().to_path_buf(),
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // Wake the accept loop so it sees the stop.
        match self.port {
            Some(port) => drop(TcpStream::connect((Ipv4Addr::LOCALHOST, port))),
            #[cfg(unix)]
            None => drop(UnixStream::connect(&self.path)),
            #[cfg(not(unix))]
            None => {}
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// One request line `{"format", "input"}` in (with `"token"` on a
/// loopback port), one reply line out (`null` for no change).
fn serve_one<S>(runner: &Mutex<Runner>, stream: &S, token: Option<&str>)
where
    for<'a> &'a S: Read + Write,
{
    let mut line = String::new();
    if BufReader::new(stream).read_line(&mut line).is_err() {
        return;
    }
    let reply = serde_json::from_str::<Value>(&line)
        .ok()
        .filter(|request| token.is_none_or(|token| request["token"].as_str() == Some(token)))
        .and_then(|request| {
            let format = Format::from_name(request["format"].as_str()?)?;
            let mut runner = runner.lock().unwrap_or_else(PoisonError::into_inner);
            answer(&mut runner, format, &request["input"])
        })
        .unwrap_or(Value::Null);
    let mut stream = stream;
    let _ = writeln!(stream, "{reply}");
}

/// A fresh token for a loopback host: 32 random hex digits.
fn loopback_token() -> std::io::Result<String> {
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(std::io::Error::other)?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// The loopback port and token in the file at `path`, when it is one.
fn loopback_address(path: &Path) -> Option<(std::net::SocketAddr, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let (address, token) = text.trim().split_once(' ')?;
    let address: std::net::SocketAddr = address.parse().ok()?;
    (address.ip().is_loopback() && !token.is_empty()).then(|| (address, token.to_owned()))
}

/// One request to the host and its reply line, over `stream`.
fn exchange<S>(stream: &S, request: &Value) -> std::io::Result<Option<Value>>
where
    for<'a> &'a S: Read + Write,
{
    let mut writer = stream;
    writeln!(writer, "{request}")?;
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    let reply: Value = serde_json::from_str(line.trim())
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    Ok((!reply.is_null()).then_some(reply))
}

/// Asks the host on `socket`.
///
/// # Errors
///
/// When the host can't be reached or doesn't answer in `wait`.
pub fn ask(
    socket: &Path,
    format: Format,
    input: &Value,
    wait: Duration,
) -> std::io::Result<Option<Value>> {
    let mut request = json!({"format": format.name(), "input": input});
    if let Some((address, token)) = loopback_address(socket) {
        request["token"] = Value::String(token);
        let stream = TcpStream::connect_timeout(&address, wait)?;
        stream.set_read_timeout(Some(wait))?;
        stream.set_write_timeout(Some(wait))?;
        return exchange::<TcpStream>(&stream, &request);
    }
    #[cfg(unix)]
    {
        let stream = UnixStream::connect(socket)?;
        stream.set_read_timeout(Some(wait))?;
        stream.set_write_timeout(Some(wait))?;
        exchange::<UnixStream>(&stream, &request)
    }
    #[cfg(not(unix))]
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "no hook host is listening there",
    ))
}

/// The bridge on one hook input: the host named by [`SOCKET_VAR`] when it
/// answers, else `fallback`.
pub fn run(
    format: Format,
    input: &Value,
    fallback: impl FnOnce(&Value) -> Option<Value>,
) -> Option<Value> {
    if let Some(socket) = std::env::var_os(SOCKET_VAR).filter(|socket| !socket.is_empty())
        && let Ok(reply) = ask(Path::new(&socket), format, input, WAIT)
    {
        return reply;
    }
    if format == Format::OpenCode {
        // The fallback reads Claude Code's input; its answer is narrowed as
        // for Codex (nobody can answer an ask) and read back.
        let claude = super::opencode::to_claude(input);
        let output = fallback(&claude)
            .and_then(|output| codex(Event::PreToolUse, output, &claude["tool_input"]));
        return super::opencode::reply(input, output.as_ref());
    }
    fallback(&normalized(format, input))
}

/// The bridge program: `args` may name `--format claude|codex` (Claude by
/// default); the hook input is read from standard input and the reply
/// printed. Always exits cleanly: an unreadable input is let through for
/// the harness's own rules.
pub fn main(args: &[String], fallback: impl FnOnce(&Value) -> Option<Value>) -> i32 {
    let mut format = Format::Claude;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if arg == "--format"
            && let Some(name) = rest.next()
        {
            match Format::from_name(name) {
                Some(named) => format = named,
                None => {
                    eprintln!("Unknown hook format {name}. Use claude, codex, acp or opencode.");
                    return 0;
                }
            }
        }
    }
    let mut text = String::new();
    let _ = std::io::stdin().read_to_string(&mut text);
    let Ok(input) = serde_json::from_str::<Value>(&text) else {
        return 0;
    };
    if let Some(reply) = run(format, &input, fallback) {
        let _ = writeln!(std::io::stdout(), "{reply}");
    }
    0
}

#[cfg(test)]
mod tests {
    use super::super::compose::Budget;
    use super::super::events::Call;
    use super::super::outcome::{Context, HookOutcome, Permission, Stop};
    use super::super::registry::{Hook, Registry, Source};
    use super::*;

    /// A built-in squeezer that rewrites `cargo test`, the floors that
    /// hold a deploy and ask on a push, a context hook after tools, and a
    /// stop hook.
    fn runner() -> Runner {
        let mut registry = Registry::new();
        registry
            .register(
                Hook::new(
                    "squeeze",
                    Event::PreToolUse,
                    Source::BuiltIn,
                    |call: &Call| {
                        let command = call.input["command"].as_str().unwrap_or_default();
                        Ok(if command == "cargo test" {
                            HookOutcome::rewrite(json!({"command": "coder-squeeze cargo test"}))
                        } else {
                            HookOutcome::none()
                        })
                    },
                )
                .matching("Bash|shell"),
            )
            .unwrap();
        registry
            .register(
                Hook::floor(
                    "floors",
                    Event::PreToolUse,
                    Permission::ask("confirm"),
                    |call: &Call| {
                        let command = call.input["command"].as_str().unwrap_or_default();
                        Ok(if command.contains("deploy") {
                            HookOutcome::deny("A production deploy is the person's.")
                        } else if command.contains("git push") {
                            HookOutcome::ask("Pushing needs the person's yes.")
                        } else {
                            HookOutcome::none()
                        })
                    },
                )
                .matching("Bash|shell"),
            )
            .unwrap();
        registry
            .register(Hook::new(
                "note",
                Event::PostToolUse,
                Source::BuiltIn,
                |_| {
                    Ok(HookOutcome::context(Context::new(
                        "note",
                        "tests are slow here",
                        0,
                    )))
                },
            ))
            .unwrap();
        registry
            .register(Hook::new("done", Event::Stop, Source::Decision, |_| {
                Ok(HookOutcome::stopping(Stop::Block {
                    reason: "No test ran yet.".into(),
                }))
            }))
            .unwrap();
        Runner::new(registry, Budget::default())
    }

    /// Recorded hook inputs, as each harness sends them.
    fn claude_pre(command: &str) -> Value {
        json!({
            "session_id": "8c1f", "transcript_path": "/tmp/t.jsonl", "cwd": "/work",
            "permission_mode": "bypassPermissions", "hook_event_name": "PreToolUse",
            "tool_name": "Bash", "tool_input": {"command": command, "description": "run"},
            "tool_use_id": "toolu_01",
        })
    }

    fn codex_pre(command: &str) -> Value {
        json!({
            "session_id": "019a", "turn_id": "1", "transcript_path": null, "cwd": "/work",
            "model": "gpt-6.1-sol", "permission_mode": "bypassPermissions",
            "hook_event_name": "PreToolUse", "tool_name": "shell",
            "tool_input": {"command": command}, "tool_use_id": "call_1",
        })
    }

    #[test]
    fn claude_format_replies() {
        let mut runner = runner();
        assert_eq!(answer(&mut runner, Format::Claude, &claude_pre("ls")), None);
        let deny = answer(&mut runner, Format::Claude, &claude_pre("make deploy")).unwrap();
        assert_eq!(deny["hookSpecificOutput"]["permissionDecision"], "deny");
        let ask = answer(&mut runner, Format::Claude, &claude_pre("git push")).unwrap();
        assert_eq!(ask["hookSpecificOutput"]["permissionDecision"], "ask");
        let squeezed = answer(&mut runner, Format::Claude, &claude_pre("cargo test")).unwrap();
        assert_eq!(
            squeezed["hookSpecificOutput"]["updatedInput"]["command"],
            "coder-squeeze cargo test"
        );
        let post = json!({
            "session_id": "8c1f", "cwd": "/work", "hook_event_name": "PostToolUse",
            "tool_name": "Bash", "tool_input": {"command": "ls"}, "tool_response": {"stdout": "a"},
        });
        let post = answer(&mut runner, Format::Claude, &post).unwrap();
        assert_eq!(
            post["hookSpecificOutput"]["additionalContext"],
            "[note] tests are slow here"
        );
        let stop = json!({"session_id": "8c1f", "cwd": "/work", "hook_event_name": "Stop",
            "stop_hook_active": false});
        let stop = answer(&mut runner, Format::Claude, &stop).unwrap();
        assert_eq!(stop["decision"], "block");
        assert_eq!(stop["reason"], "No test ran yet.");
        assert_eq!(
            answer(
                &mut runner,
                Format::Claude,
                &json!({"hook_event_name": "Mystery"})
            ),
            None
        );
    }

    #[test]
    fn codex_format_replies_within_what_codex_accepts() {
        let mut runner = runner();
        assert_eq!(answer(&mut runner, Format::Codex, &codex_pre("ls")), None);
        let deny = answer(&mut runner, Format::Codex, &codex_pre("make deploy")).unwrap();
        assert_eq!(deny["hookSpecificOutput"]["permissionDecision"], "deny");
        // Nobody can answer inside Codex: the ask is held.
        let held = answer(&mut runner, Format::Codex, &codex_pre("git push")).unwrap();
        assert_eq!(held["hookSpecificOutput"]["permissionDecision"], "deny");
        assert!(
            held["hookSpecificOutput"]["permissionDecisionReason"]
                .as_str()
                .unwrap()
                .starts_with("Held for the person: Pushing")
        );
        // A rewrite is allow plus updatedInput, as Codex requires.
        let squeezed = answer(&mut runner, Format::Codex, &codex_pre("cargo test")).unwrap();
        assert_eq!(
            squeezed["hookSpecificOutput"]["permissionDecision"],
            "allow"
        );
        assert_eq!(
            squeezed["hookSpecificOutput"]["updatedInput"]["command"],
            "coder-squeeze cargo test"
        );
        assert!(squeezed.get("continue").is_none());
    }

    fn opencode_bash(command: &str) -> Value {
        json!({"tool": "bash", "sessionID": "ses_1", "callID": "c1", "cwd": "/work",
            "args": {"command": command, "description": "run"}})
    }

    /// OpenCode's plugin call: a floor denies, an ask is held, a rewrite
    /// comes back as OpenCode's arguments (#11507).
    #[test]
    fn opencode_format_replies_with_a_denial_or_arguments() {
        let mut runner = runner();
        assert_eq!(
            answer(&mut runner, Format::OpenCode, &opencode_bash("ls")),
            None
        );
        let deny = answer(&mut runner, Format::OpenCode, &opencode_bash("make deploy")).unwrap();
        assert_eq!(
            deny,
            json!({"deny": "A production deploy is the person's."})
        );
        let held = answer(&mut runner, Format::OpenCode, &opencode_bash("git push")).unwrap();
        assert!(
            held["deny"]
                .as_str()
                .unwrap()
                .starts_with("Held for the person: Pushing")
        );
        let squeezed = answer(&mut runner, Format::OpenCode, &opencode_bash("cargo test")).unwrap();
        assert_eq!(squeezed["args"]["command"], "coder-squeeze cargo test");
        // With no host the fallback reads Claude Code's input.
        let fallback = run(Format::OpenCode, &opencode_bash("rm -rf /"), |input| {
            assert_eq!(input["tool_name"], "Bash");
            Some(json!({"hookSpecificOutput": {"hookEventName": "PreToolUse",
                "permissionDecision": "deny", "permissionDecisionReason": "no"}}))
        });
        assert_eq!(fallback, Some(json!({"deny": "no"})));
        assert_eq!(Format::from_name("opencode"), Some(Format::OpenCode));
    }

    #[test]
    fn the_host_answers_the_bridge_and_a_missing_host_falls_back() {
        let host = Host::start(Arc::new(Mutex::new(runner()))).unwrap();
        let reply = ask(host.path(), Format::Codex, &codex_pre("make deploy"), WAIT)
            .unwrap()
            .unwrap();
        assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");
        assert_eq!(
            ask(host.path(), Format::Codex, &codex_pre("ls"), WAIT).unwrap(),
            None
        );
        let path = host.path().to_path_buf();
        drop(host);
        assert!(!path.exists());
        assert!(ask(&path, Format::Codex, &codex_pre("ls"), WAIT).is_err());
        // No host: the fallback answers.
        let fallback = run(Format::Codex, &codex_pre("ls"), |_| {
            Some(json!({"fallback": true}))
        });
        assert_eq!(fallback, Some(json!({"fallback": true})));
    }
    #[test]
    fn a_loopback_host_answers_only_with_its_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("hook-host");
        let host = Host::start_loopback(Arc::new(Mutex::new(runner())), path.clone()).unwrap();
        let reply = ask(&path, Format::Codex, &codex_pre("make deploy"), WAIT)
            .unwrap()
            .unwrap();
        assert_eq!(reply["hookSpecificOutput"]["permissionDecision"], "deny");
        // The right port with the wrong token gets nothing.
        let (address, _) = loopback_address(&path).unwrap();
        let stream = TcpStream::connect(address).unwrap();
        let forged = json!({"format": "codex", "input": codex_pre("make deploy"), "token": "0"});
        assert_eq!(exchange::<TcpStream>(&stream, &forged).unwrap(), None);
        drop(host);
        assert!(!path.exists());
        assert!(ask(&path, Format::Codex, &codex_pre("ls"), WAIT).is_err());
    }
}
