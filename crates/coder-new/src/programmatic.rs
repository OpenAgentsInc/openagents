//! Programmatic access to the terminal's settings, chats, and plugin runtime.

use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    App, Mode, bundled_runtime::RuntimeEvent, live, models, plugin_definition::DEFINITIONS,
    plugins::SettingsFocus, sessions as local_sessions, trajectory,
};

macro_rules! command_usage {
    ($demo:literal) => {
        concat!(
            "usage: openagents coder COMMAND [OPTIONS]
  status                              Provider, model, plugins, and working directory.
  chat [-p TEXT | --prompt-file FILE | --stdin] [--session ID] [--delegation ID]
       [--instructions TEXT | --instructions-file FILE]",
            $demo,
            "
                                      Run the same chat and tools as the terminal.
  delegate AGENT --task TEXT [--session ID]
       [--on boat|gce] [--mode integrated|coder] [--model ID] [--reasoning EFFORT]
       [--job ID] [--size small|default|large|xlarge] [--template NAME]
       [--credential-env NAME] [--timeout SECONDS]
       [--revision REF] [--workspace-path PATH] [--include FILE] [--no-workspace]
  remote list                         List retained remote jobs.
  remote status ID                    Read one retained remote job.
  remote follow ID                    Observe a remote job to completion.
  remote cancel ID                    Request cancellation of a remote job.
  remote artifacts ID                 Read a remote job's artifacts.
  remote apply ID                     Apply a remote job's artifacts locally.
  remote continue ID --task TEXT       Resume a remote job with a new task.
  remote steer ID --message TEXT       Send a correction to a remote job.
                                      Run Microcoder or an enabled ACP subagent.
  plugins list                        List registered plugins and their status.
  plugins enable ID                   Turn a registered plugin on.
  plugins disable ID                  Turn a registered plugin off.
  plugins configure ID --stdin        Configure a plugin with a JSON object:
                                      api_key, model, endpoint, enabled.
                                      An api_key of null removes the saved key.
                                      Cloud: mode, size, template, credential_names, workspace_paths.
  plugins check ID                    Check the configured API connection.
  models list                         Read the curated model catalog.
  models set SLUG [--reasoning EFFORT] [--max-tokens N]
                                      Choose model and generation settings.
  agents list                         Read discovered ACP agent choices.
  agents enable ID                    Enable a discovered ACP agent.
  agents disable ID                   Disable a discovered ACP agent.
  agents refresh                      Discover installed ACP agents again.
  sessions list                       List retained local chats.
  sessions read ID                    Read one retained ATIF trajectory.
  sessions delete ID                  Remove one retained chat.
  export ID [--output FILE]            Export a retained chat as ATIF-v1.8.
  import FILE [--session ID]           Retain ATIF for viewing or continuing.
Options: --json streams NDJSON events for chat and delegation.
         --approvals stdin asks before any command that is not read-only:
         an approval event, answered by `confirm ID` or `reject ID` on stdin.
         Codex and other tools start with full filesystem and network access.
         --codex-writes is accepted for compatibility; full access is the default.
         With --approvals, Codex stays read-only and asks nobody.
         --approvals tool-free refuses all model tools, including reads.
         --in DIR sets the working directory; --state DIR sets the Coder store.
Settings default to ~/.openagents/coder-new. Sessions use its sessions directory.
Keys are accepted through environment variables or configuration stdin, never printed."
        )
    };
}

pub const USAGE: &str = if crate::DEMO_AVAILABLE {
    command_usage!(" [--demo]")
} else {
    command_usage!("")
};

/// Set by the host when a model-owned tool launches the companion CLI. It
/// narrows the nested frontend's authority; tool arguments cannot clear it.
pub(crate) const MODEL_INPUT_ENV: &str = "OPENAGENTS_CODER_MODEL_INPUT";

pub(crate) fn command_environment(
    mut get: impl FnMut(&str) -> Option<String>,
) -> BTreeMap<String, String> {
    [
        "OPENROUTER_API_KEY",
        "TYPESAFE_API_KEY",
        "TYPESAFE_BASE_URL",
        "AI_GATEWAY_API_KEY",
        "TYPESAFE_DEFAULT_MODEL",
        "PATH",
        "HOME",
        "USERPROFILE",
        "GROK_BIN",
        "DEVIN_BIN",
        "OPENCODE_BIN",
        MODEL_INPUT_ENV,
        crate::delegation_events::CHANNEL_ENV,
    ]
    .into_iter()
    .filter_map(|name| get(name).map(|value| (name.into(), value)))
    .collect()
}

/// Explicit roots and environment make command execution usable from other hosts.
pub struct Context {
    pub root: PathBuf,
    pub cwd: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub input: Option<String>,
    pub canceled: Option<Arc<AtomicBool>>,
    /// The approval desk of a gated chat (`--approvals stdin`), whose
    /// answers the caller feeds ([`crate::approval`]).
    pub approvals: Option<Arc<crate::approval::Desk>>,
}

#[derive(Debug)]
pub struct Error {
    pub message: String,
    pub usage: bool,
}

impl From<String> for Error {
    fn from(message: String) -> Self {
        Self {
            message,
            usage: false,
        }
    }
}
impl From<&str> for Error {
    fn from(message: &str) -> Self {
        message.to_owned().into()
    }
}

fn usage(message: &str) -> Error {
    Error {
        message: message.into(),
        usage: true,
    }
}

fn check_demo(arguments: &[String], available: bool) -> Result<(), Error> {
    if !available && arguments.iter().any(|argument| argument == "--demo") {
        return Err(usage(
            "Demo mode is available only in local development builds.",
        ));
    }
    Ok(())
}

/// Dispatch one CLI call. JSON mode streams events, followed by a result document.
pub fn run(arguments: &[String], json_mode: bool) -> u8 {
    if let Err(error) = check_demo(arguments, crate::DEMO_AVAILABLE) {
        return print_error(error, json_mode);
    }
    if arguments.is_empty()
        || arguments
            .iter()
            .any(|arg| matches!(arg.as_str(), "--help" | "-h"))
    {
        println!("{USAGE}");
        return 0;
    }
    let mut args = arguments.to_vec();
    let state = match take_option(&mut args, "--state") {
        Ok(value) => value,
        Err(error) => return print_error(error, json_mode),
    };
    let cwd = match take_option(&mut args, "--in") {
        Ok(value) => value,
        Err(error) => return print_error(error, json_mode),
    };
    let root = match state
        .map(PathBuf::from)
        .or_else(|| model_access::store::openagents_dir().map(|root| root.join("coder-new")))
    {
        Some(root) => root,
        None => {
            return print_error(
                "Set a home directory or pass --state DIR.".into(),
                json_mode,
            );
        }
    };
    let cwd = match cwd
        .map(PathBuf::from)
        .map_or_else(std::env::current_dir, |path| path.canonicalize())
    {
        Ok(path) if path.is_dir() => path,
        _ => return print_error("The working directory is unavailable.".into(), json_mode),
    };
    let approvals = match take_option(&mut args, "--approvals") {
        Ok(None) => None,
        Ok(Some(source)) if source == "tool-free" => Some(crate::approval::Desk::tool_free()),
        Ok(Some(source)) if source == "stdin" && !args.iter().any(|arg| arg == "--stdin") => {
            let desk = crate::approval::Desk::new();
            let reader = Arc::clone(&desk);
            std::thread::spawn(move || {
                let mut line = String::new();
                let stdin = io::stdin();
                loop {
                    line.clear();
                    match io::BufRead::read_line(&mut stdin.lock(), &mut line) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {
                            if let Err(error) = reader.answer(&line) {
                                eprintln!("Coder: {error}");
                            }
                        }
                    }
                }
                reader.close();
            });
            Some(desk)
        }
        Ok(Some(_)) => {
            return print_error(
                usage(
                    "Use --approvals stdin or tool-free, with the prompt in -p or --prompt-file.",
                ),
                json_mode,
            );
        }
        Err(error) => return print_error(error, json_mode),
    };
    // The host lets this chat's Codex delegations edit the working
    // directory; a gated chat keeps Codex read-only.
    if args.iter().any(|arg| arg == "--codex-writes") {
        if args.first().is_none_or(|arg| arg != "chat") {
            return print_error(usage("--codex-writes applies to chat only."), json_mode);
        }
        args.retain(|arg| arg != "--codex-writes");
        crate::bundled_runtime::allow_codex_writes(true);
    }
    let input = if args.iter().any(|arg| arg == "--stdin") {
        let mut bytes = Vec::new();
        if io::stdin()
            .take(1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .is_err()
            || bytes.len() > 1024 * 1024
        {
            return print_error(
                "Cannot read stdin or stdin exceeds 1 MiB.".into(),
                json_mode,
            );
        }
        match String::from_utf8(bytes) {
            Ok(text) => Some(text),
            Err(_) => return print_error(usage("stdin must contain UTF-8 text."), json_mode),
        }
    } else {
        None
    };
    let mut environment = command_environment(|name| std::env::var(name).ok());
    if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "chat" | "run" | "delegate" | "status"))
        || args.first().is_some_and(|arg| arg == "plugins")
            && args.get(1).is_some_and(|arg| arg == "check")
    {
        let once = model_access::once_keys();
        for (provider, name) in [
            (model_access::Provider::OpenRouter, "OPENROUTER_API_KEY"),
            (model_access::Provider::TypeSafe, "TYPESAFE_API_KEY"),
            (model_access::Provider::Vercel, "AI_GATEWAY_API_KEY"),
        ] {
            if let Some(key) = once.get(provider) {
                environment.insert(name.into(), key.expose().into());
            }
        }
    }
    let canceled = if args
        .first()
        .is_some_and(|arg| matches!(arg.as_str(), "chat" | "run" | "delegate" | "remote"))
        && !args.iter().any(|arg| arg == "--demo")
    {
        let flag = Arc::new(AtomicBool::new(false));
        let worker = Arc::clone(&flag);
        std::thread::spawn(move || {
            if let Ok(runtime) = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                runtime.block_on(async move {
                    if tokio::signal::ctrl_c().await.is_ok() {
                        worker.store(true, Ordering::Relaxed);
                    }
                });
            }
        });
        Some(flag)
    } else {
        None
    };
    let context = Context {
        root,
        cwd,
        environment,
        input,
        canceled,
        approvals,
    };
    let mut emit = |event: Value| {
        if json_mode {
            println!("{event}");
        }
    };
    match execute(&args, &context, &mut emit) {
        Ok(result) => {
            if json_mode {
                println!("{result}");
            } else if let Some(reply) = result.get("reply").and_then(Value::as_str) {
                println!("{reply}");
            } else {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&result).unwrap_or_default()
                );
            }
            0
        }
        Err(error) => {
            let code = print_error(error, json_mode);
            if context
                .canceled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                130
            } else {
                code
            }
        }
    }
}

fn print_error(error: Error, json_mode: bool) -> u8 {
    if json_mode {
        println!("{}", json!({"error":error.message}));
    } else {
        eprintln!("Coder: {}", error.message);
    }
    if error.usage { 64 } else { 1 }
}

/// Execute with supplied roots. The caller controls how progress is presented.
pub fn execute(
    arguments: &[String],
    context: &Context,
    emit: &mut dyn FnMut(Value),
) -> Result<Value, Error> {
    let mut parent = crate::delegation_events::Publisher::connect(
        context
            .environment
            .get(crate::delegation_events::CHANNEL_ENV)
            .map(String::as_str),
    );
    let mut events = |value: Value| {
        parent.send(&value);
        emit(value);
    };
    execute_with_demo_policy(arguments, context, &mut events, crate::DEMO_AVAILABLE)
}

fn execute_with_demo_policy(
    arguments: &[String],
    context: &Context,
    emit: &mut dyn FnMut(Value),
    demo_available: bool,
) -> Result<Value, Error> {
    check_demo(arguments, demo_available)?;
    let Some((command, rest)) = arguments.split_first() else {
        return Err(usage("Choose a Coder command."));
    };
    if command == "sessions" {
        return sessions(rest, context);
    }
    if command == "export" {
        return export(rest, context);
    }
    if command == "import" {
        return import(rest, context);
    }
    if command == "remote" || command == "delegate" && rest.iter().any(|v| v == "--on") {
        let app = bootstrap(context)?;
        let mut args = rest.to_vec();
        let placement = if command == "delegate" {
            args.iter()
                .position(|a| a == "--on")
                .and_then(|i| args.get(i + 1))
                .and_then(|p| match p.as_str() {
                    "boat" => Some(coder_cloud::Placement::Boat),
                    "gce" | "cloud" => Some(coder_cloud::Placement::Gce),
                    _ => None,
                })
        } else if args
            .first()
            .is_some_and(|op| matches!(op.as_str(), "follow" | "continue" | "steer"))
        {
            let r = coder_cloud::Store::under(context.root.join("remote"))
                .read(args.get(1).ok_or("Supply a remote job ID.")?)?;
            if r.state.terminal() && r.cleanup_complete && args[0] == "follow" {
                None
            } else {
                Some(r.spec.placement)
            }
        } else {
            None
        };
        if let Some(p) = placement {
            let config = app.plugins.bundled.cloud(p);
            if !config.enabled {
                return Err(Error::from(if p == coder_cloud::Placement::Boat {
                    "Enable boat-cloud with plugins enable boat-cloud before dispatch."
                } else {
                    "Enable gce-cloud with plugins enable gce-cloud before dispatch."
                }));
            }
            if command == "delegate" {
                for (flag, value) in [
                    (
                        "--mode",
                        Some(if config.mode == coder_cloud::Mode::Coder {
                            "coder".into()
                        } else {
                            "integrated".into()
                        }),
                    ),
                    ("--size", Some(config.size.clone())),
                    ("--template", config.template.clone()),
                ] {
                    if !args.iter().any(|a| a == flag) {
                        if let Some(value) = value {
                            args.extend([flag.into(), value]);
                        }
                    }
                }
                if !args.iter().any(|a| a == "--credential-env") {
                    for n in &config.credential_names {
                        args.extend(["--credential-env".into(), n.clone()]);
                    }
                }
                if context
                    .environment
                    .get(MODEL_INPUT_ENV)
                    .is_some_and(|s| s == "model")
                {
                    for pair in args.windows(2).filter(|pair| pair[0] == "--credential-env") {
                        if !config.credential_names.contains(&pair[1]) {
                            return Err(Error::from(
                                "This credential variable is not admitted in cloud settings.",
                            ));
                        }
                    }
                }
                if !args
                    .iter()
                    .any(|a| a == "--workspace-path" || a == "--no-workspace")
                {
                    for p in &config.workspace_paths {
                        args.extend(["--workspace-path".into(), p.clone()]);
                    }
                }
            }
        }
        return crate::cloud::execute(command, &args, context, emit).map_err(Error::from);
    }
    let mut app = bootstrap(context)?;
    match command.as_str() {
        "status" if rest.is_empty() => Ok(status(&app, context)),
        "plugins" => plugins(&mut app, rest, context),
        "models" => model_command(&mut app, rest),
        "agents" => agents(&mut app, rest),
        "chat" | "run" => chat(&mut app, rest, context, emit),
        "delegate" => delegate(&mut app, rest, context, emit),
        _ => Err(usage(
            "Unknown Coder command or unexpected arguments. Use openagents coder --help.",
        )),
    }
}

fn bootstrap(context: &Context) -> Result<App, Error> {
    let mut app = App::default();
    app.set_mode(Mode::Live);
    app.load_plugin_settings(crate::plugin_store::Store::under(&context.root))?;
    let imported = crate::credentials::load(&context.cwd, context.root.parent(), |name| {
        context.environment.get(name).cloned()
    })?;
    app.plugins.bootstrap_credentials(imported);
    app.plugins.bundled.discover_acp(&|name| {
        if name == "CODER_ACP_CWD" {
            Some(context.cwd.clone().into_os_string())
        } else {
            context.environment.get(name).map(std::ffi::OsString::from)
        }
    });
    Ok(app)
}

fn status(app: &App, context: &Context) -> Value {
    json!({"schema":"openagents.coder.status.v1","cwd":context.cwd,"model":active_model(app),
        "provider":if app.plugins.enabled && app.plugins.key_configured {"openrouter-byok"} else {"openagents-gateway"},
        "openrouter_connected":app.plugins.key_configured,"plugins":plugin_list(app),"agents":agent_list(app)})
}

fn active_model(app: &App) -> String {
    if app.plugins.enabled && app.plugins.key_configured {
        app.plugins.options.slug(&app.plugins.model)
    } else {
        "auto".into()
    }
}

fn plugin_list(app: &App) -> Value {
    json!(DEFINITIONS.iter().map(|definition| json!({"id":definition.id,"name":definition.name,"description":definition.description,
        "enabled":app.plugins.enabled_for(definition.id),"status":app.plugins.status_for(definition.id),"default_enabled":definition.default_enabled})).collect::<Vec<_>>())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Configuration {
    #[serde(default, deserialize_with = "present_value")]
    api_key: Option<Value>,
    #[serde(default)]
    model: Option<String>,
    #[serde(default)]
    endpoint: Option<String>,
    #[serde(default)]
    enabled: Option<bool>,
}

fn present_value<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<Value>, D::Error> {
    Value::deserialize(deserializer).map(Some)
}

fn plugins(app: &mut App, args: &[String], context: &Context) -> Result<Value, Error> {
    if args.is_empty() || args == ["list"] {
        return Ok(json!({"plugins":plugin_list(app)}));
    }
    let Some(definition) = args
        .get(1)
        .and_then(|id| DEFINITIONS.iter().find(|definition| definition.id == id))
    else {
        return Err(usage("Choose a registered plugin ID."));
    };
    match args[0].as_str() {
        "enable" | "disable" if args.len() == 2 => {
            let enabled = args[0] == "enable";
            if app.plugins.enabled_for(definition.id) != enabled {
                app.plugins.selected = DEFINITIONS
                    .iter()
                    .position(|row| row.id == definition.id)
                    .unwrap_or(0);
                if !app.plugins.toggle_selected() {
                    return Err(app
                        .plugins
                        .storage_error
                        .clone()
                        .or_else(|| app.plugins.bundled.storage_error.clone())
                        .unwrap_or("Cannot save plugin settings.".into())
                        .into());
                }
            }
        }
        "configure" if args.len() == 3 && args[2] == "--stdin" => {
            if let Some(p) = crate::cloud_settings::placement(definition.id) {
                let input: Value = serde_json::from_str(
                    context
                        .input
                        .as_deref()
                        .ok_or("Supply cloud configuration on stdin.")?,
                )
                .map_err(|_| "Invalid cloud configuration JSON.")?;
                app.plugins.bundled.configure_cloud(p, input)?;
                return Ok(json!({"id":definition.id,"settings":app.plugins.bundled.cloud(p)}));
            }
            if !matches!(definition.id, "openrouter-byok" | "jev") {
                return Err(usage(
                    "This plugin has no editable connection. Use agents to change ACP choices.",
                ));
            }
            let text = context
                .input
                .as_deref()
                .ok_or_else(|| usage("Supply the configuration on stdin."))?;
            let config: Configuration = serde_json::from_str(text).map_err(|_| {
                usage("Use a configuration object containing api_key, model, endpoint, or enabled.")
            })?;
            configure(app, definition.id, config)?;
        }
        "check" if args.len() == 2 => {
            match definition.id {
                "openrouter-byok" => app.check_key(),
                "jev" => app.check_jev_key(),
                _ => return Err(usage("This plugin has no API key to check.")),
            }
            let mut background = live::Background::default();
            while app.request.is_some() || app.checking_key {
                background.sync(app);
                std::thread::sleep(Duration::from_millis(15));
            }
            let label = if definition.id == "jev" {
                app.plugins.bundled.connection_label()
            } else {
                app.plugins.connection_label()
            };
            let connection = if definition.id == "jev" {
                &app.plugins.bundled.connection
            } else {
                &app.plugins.connection
            };
            if !matches!(connection, crate::plugins::Connection::Verified) {
                return Err(label.to_owned().into());
            }
            return Ok(json!({"id":definition.id,"connection":label}));
        }
        _ => {
            return Err(usage(
                "Use plugins list, enable, disable, configure, or check.",
            ));
        }
    }
    Ok(json!({"plugins":plugin_list(app)}))
}

fn key_config(value: Option<Value>) -> Result<Option<Option<String>>, Error> {
    value
        .map(|value| match value {
            Value::Null => Ok(None),
            Value::String(key)
                if !key.is_empty()
                    && key.len() <= 16384
                    && !key.chars().any(|c| c.is_whitespace() || c.is_control()) =>
            {
                Ok(Some(key))
            }
            _ => Err(usage(
                "api_key must be a nonempty key without whitespace, or null to remove it.",
            )),
        })
        .transpose()
}

fn configure(app: &mut App, id: &str, config: Configuration) -> Result<(), Error> {
    let key = key_config(config.api_key)?;
    // Reuse the terminal's validated editors and atomic saves.
    if id == "openrouter-byok" {
        if config.endpoint.is_some() {
            return Err(usage("OpenRouter BYOK uses OpenRouter's API endpoint."));
        }
        app.plugins.begin_settings();
        if let Some(model) = config.model {
            app.plugins.focus = SettingsFocus::Model;
            clear_openrouter(app);
            app.plugins.paste(&model);
        }
        if let Some(key) = key {
            match key {
                Some(key) => {
                    app.plugins.focus = SettingsFocus::ApiKey;
                    clear_openrouter(app);
                    app.plugins.paste(&key);
                }
                None => {
                    app.plugins.focus = SettingsFocus::RemoveKey;
                    app.plugins
                        .handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
        }
        app.plugins.focus = SettingsFocus::Save;
        if !app
            .plugins
            .handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            || !app.plugins.saved
        {
            return Err(app
                .plugins
                .storage_error
                .clone()
                .unwrap_or_else(|| {
                    app.plugins
                        .error
                        .unwrap_or("Cannot save plugin settings.")
                        .into()
                })
                .into());
        }
    } else {
        app.plugins.bundled.begin_settings();
        if let Some(endpoint) = config.endpoint {
            app.plugins.bundled.focus = SettingsFocus::Endpoint;
            clear_jev(app);
            app.plugins.bundled.paste(&endpoint);
        }
        if let Some(model) = config.model {
            app.plugins.bundled.focus = SettingsFocus::Model;
            clear_jev(app);
            app.plugins.bundled.paste(&model);
        }
        if let Some(key) = key {
            match key {
                Some(key) => {
                    app.plugins.bundled.focus = SettingsFocus::ApiKey;
                    clear_jev(app);
                    app.plugins.bundled.paste(&key);
                }
                None => {
                    app.plugins.bundled.focus = SettingsFocus::RemoveKey;
                    app.plugins
                        .bundled
                        .handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
                }
            }
        }
        app.plugins.bundled.focus = SettingsFocus::Save;
        if !app
            .plugins
            .bundled
            .handle(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            || !app.plugins.bundled.saved
        {
            return Err(app
                .plugins
                .bundled
                .storage_error
                .clone()
                .unwrap_or_else(|| {
                    app.plugins
                        .bundled
                        .error
                        .unwrap_or("Cannot save plugin settings.")
                        .into()
                })
                .into());
        }
    }
    if let Some(enabled) = config.enabled
        && app.plugins.enabled_for(id) != enabled
    {
        app.plugins.selected = DEFINITIONS
            .iter()
            .position(|definition| definition.id == id)
            .unwrap_or(0);
        if !app.plugins.toggle_selected() {
            return Err("Cannot save plugin activation.".into());
        }
    }
    Ok(())
}

fn clear_openrouter(app: &mut App) {
    app.plugins
        .handle(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    let (text, _) = app
        .plugins
        .field(app.plugins.focus == SettingsFocus::ApiKey);
    for _ in 0..text.chars().count() {
        app.plugins
            .handle(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }
}
fn clear_jev(app: &mut App) {
    app.plugins
        .bundled
        .handle(KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
    let (text, _) = if app.plugins.bundled.focus == SettingsFocus::Endpoint {
        app.plugins.bundled.endpoint_field()
    } else {
        app.plugins
            .bundled
            .field(app.plugins.bundled.focus == SettingsFocus::ApiKey)
    };
    for _ in 0..text.chars().count() {
        app.plugins
            .bundled
            .handle(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE));
    }
}

fn model_command(app: &mut App, args: &[String]) -> Result<Value, Error> {
    if args.is_empty() || args == ["list"] {
        let mut catalog = vec![
            json!({"plugin":crate::plugin_definition::FALLBACK_PROVIDER.id,"id":"auto","name":"Auto","reasoning":[],"default":true}),
        ];
        if app.plugins.enabled {
            catalog.extend(models::openrouter_catalog().iter().map(|model|json!({"plugin":model.plugin,"id":model.id,"name":model.name,"reasoning":model.efforts,"context_length":model.context_length,"max_output_tokens":model.max_output_tokens})));
        }
        return Ok(json!({"selected":active_model(app),"models":catalog}));
    }
    if args.first().is_none_or(|arg| arg != "set") {
        return Err(usage("Use models list or models set SLUG."));
    }
    let mut rest = args[1..].to_vec();
    let reasoning = take_option(&mut rest, "--reasoning")?;
    let mut max_tokens = take_option(&mut rest, "--max-tokens")?
        .map(|limit| {
            limit
                .parse::<u32>()
                .map_err(|_| usage("max-tokens must be a positive integer."))
        })
        .transpose()?;
    if rest.len() != 1 {
        return Err(usage(
            "Use models set SLUG [--reasoning EFFORT] [--max-tokens N].",
        ));
    }
    let mut slug = rest.remove(0);
    let mut reasoning = reasoning;
    while let Some((model, setting)) = slug.rsplit_once(':') {
        if matches!(
            setting,
            "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
        ) {
            if reasoning.as_deref().is_some_and(|value| value != setting) {
                return Err(usage("The reasoning suffix and --reasoning disagree."));
            }
            reasoning = Some(setting.into());
        } else if let Some(limit) = setting.strip_prefix("max-tokens=") {
            let limit = limit
                .parse::<u32>()
                .map_err(|_| usage("max-tokens must be a positive integer."))?;
            if max_tokens.is_some_and(|value| value != limit) {
                return Err(usage("The output limit suffix and --max-tokens disagree."));
            }
            max_tokens = Some(limit);
        } else {
            break;
        }
        slug = model.into();
    }
    if slug == "auto" {
        if reasoning.is_some() || max_tokens.is_some() {
            return Err(usage(
                "The automatic provider does not expose reasoning or output limits.",
            ));
        }
        if app.plugins.enabled && !app.plugins.toggle_enabled() {
            return Err("Cannot save provider selection.".into());
        }
        return Ok(json!({"model":"auto","provider":"openagents-gateway"}));
    }
    let model = models::openrouter_catalog()
        .into_iter()
        .find(|model| model.id == slug)
        .ok_or_else(|| usage("Choose a model from models list."))?;
    if !app.plugins.set_model(
        &model,
        models::GenerationOptions {
            reasoning,
            max_tokens,
        },
    ) {
        return Err(app
            .plugins
            .storage_error
            .clone()
            .unwrap_or("Cannot save model settings.".into())
            .into());
    }
    Ok(json!({"model":app.plugins.options.slug(&app.plugins.model),"options":app.plugins.options}))
}

fn agent_list(app: &App) -> Value {
    json!(app.plugins.bundled.acp_choices().iter().map(|agent|json!({"id":agent.id,"name":agent.name,"enabled":agent.enabled,"program":agent.program,"arguments":agent.arguments,"transport":agent.transport})).collect::<Vec<_>>())
}
fn agents(app: &mut App, args: &[String]) -> Result<Value, Error> {
    if args.is_empty() || args == ["list"] || args == ["refresh"] {
        return Ok(json!({"agents":agent_list(app)}));
    }
    if args.len() != 2 || !matches!(args[0].as_str(), "enable" | "disable") {
        return Err(usage("Use agents list, enable ID, disable ID, or refresh."));
    }
    let index = app
        .plugins
        .bundled
        .acp_choices()
        .iter()
        .position(|agent| agent.id == args[1])
        .ok_or_else(|| usage("This ACP agent was not discovered."))?;
    let enabled = args[0] == "enable";
    if app.plugins.bundled.acp_choices()[index].enabled != enabled {
        app.plugins.bundled.acp_selected = index;
        if !app.plugins.bundled.toggle_acp_agent() {
            return Err("Cannot save the ACP agent choice.".into());
        }
    }
    Ok(json!({"agents":agent_list(app)}))
}

fn take_option(args: &mut Vec<String>, name: &str) -> Result<Option<String>, Error> {
    let Some(index) = args.iter().position(|arg| arg == name) else {
        return Ok(None);
    };
    if index + 1 >= args.len() {
        return Err(usage("An option is missing its value."));
    }
    args.remove(index);
    let value = args.remove(index);
    if args.iter().any(|arg| arg == name) {
        return Err(usage("An option was supplied more than once."));
    }
    Ok(Some(value))
}

pub(crate) fn chat(
    app: &mut App,
    args: &[String],
    context: &Context,
    emit: &mut dyn FnMut(Value),
) -> Result<Value, Error> {
    app.model_owned_input = context.environment.contains_key(MODEL_INPUT_ENV);
    let mut args = args.to_vec();
    let session = take_option(&mut args, "--session")?.unwrap_or_else(new_id);
    let delegation = take_option(&mut args, "--delegation")?;
    let prompt = take_option(&mut args, "-p")?.or(take_option(&mut args, "--prompt")?);
    let file = take_option(&mut args, "--prompt-file")?;
    let instructions = match (
        take_option(&mut args, "--instructions")?,
        take_option(&mut args, "--instructions-file")?,
    ) {
        (Some(_), Some(_)) => {
            return Err(usage(
                "Supply --instructions or --instructions-file, not both.",
            ));
        }
        (Some(text), None) => Some(text),
        (None, Some(path)) => Some(
            fs::read_to_string(context.cwd.join(path))
                .map_err(|_| Error::from("Cannot read the instructions file."))?,
        ),
        (None, None) => None,
    };
    let stdin = args.iter().any(|arg| arg == "--stdin");
    args.retain(|arg| arg != "--stdin");
    let demo = args.iter().any(|arg| arg == "--demo");
    args.retain(|arg| arg != "--demo");
    if !args.is_empty()
        || usize::from(prompt.is_some()) + usize::from(file.is_some()) + usize::from(stdin) != 1
    {
        return Err(usage(
            "Supply one prompt with -p, --prompt-file, or --stdin.",
        ));
    }
    let lease = lock_session(context, &session)?;
    let prompt = if let Some(prompt) = prompt {
        prompt
    } else if let Some(file) = file {
        fs::read_to_string(context.cwd.join(file))
            .map_err(|_| Error::from("Cannot read the prompt file."))?
    } else {
        context
            .input
            .clone()
            .ok_or_else(|| usage("Supply a prompt on stdin."))?
    };
    if prompt.trim().is_empty() {
        return Err(usage("The prompt cannot be empty."));
    }
    if lease.exists()? {
        trajectory::restore_app(app, &lease.read()?)?;
    }
    // Standing instructions stay beside the session, never in it, so the
    // transcript, a follower, and an export show only the conversation.
    let standing = instructions_path(lease.path());
    if let Some(text) = &instructions {
        save_instructions(&standing, text)?;
    }
    if instructions.is_some() {
        forget_appended_charter(&mut app.live.entries);
    }
    app.live.instructions = instructions
        .or_else(|| fs::read_to_string(&standing).ok())
        .filter(|text| !text.trim().is_empty());
    let brainstorm_lookup = !demo && matches!(crate::brainstorm::parse(prompt.trim()), Some(Ok(_)));
    if let Some(id) = delegation {
        app.selected_agent = Some(
            app.delegations
                .iter()
                .position(|child| child.id == id)
                .ok_or_else(|| usage("That delegation is not part of the selected session."))?,
        );
    }
    let lookup_conversation = if brainstorm_lookup {
        let conversation = crate::brainstorm::Conversation::selected(app)
            .ok_or("The selected lookup conversation is unavailable.")?;
        let turn_start = conversation
            .chat(app)
            .ok_or("The selected lookup conversation is unavailable.")?
            .entries
            .len();
        Some((conversation, turn_start))
    } else {
        None
    };
    if demo {
        let target = if let Some(index) = app.selected_agent {
            &mut app.delegations[index].chat
        } else {
            &mut app.live
        };
        target.entries.push(live::Entry::User(prompt));
        target.entries.push(live::Entry::Assistant {
            elapsed_ms: None,
            text: "Demo reply. Live execution uses the terminal's provider and plugin runtime."
                .into(),
            model: Some("demo/local".into()),
        });
    } else {
        let _gate = GateGuard::install(context);
        let restored_entries = app.live.entries.len();
        app.submit(&prompt, &context.cwd);
        if !app.live.busy {
            return Err(app
                .live
                .notice
                .clone()
                .unwrap_or("The chat could not start.".into())
                .into());
        }
        let mut background = live::Background::default();
        let mut partial = PartialStream::default();
        // A continued session streams only this turn's entries, not the
        // history it restored.
        let mut previous: BTreeMap<usize, Value> = app
            .live
            .entries
            .iter()
            .enumerate()
            .take(restored_entries)
            .map(|(index, entry)| (index, entry_value(entry)))
            .collect();
        let mut child_previous: BTreeMap<(String, usize), Value> = app
            .delegations
            .iter()
            .flat_map(|child| {
                child
                    .chat
                    .entries
                    .iter()
                    .enumerate()
                    .take(match &lookup_conversation {
                        Some((crate::brainstorm::Conversation::Delegation(id), start))
                            if id == &child.id =>
                        {
                            *start
                        }
                        _ => child.chat.entries.len(),
                    })
                    .map(|(index, entry)| ((child.id.clone(), index), entry_value(entry)))
            })
            .collect();
        let mut child_partial = BTreeMap::new();
        let started = std::time::Instant::now();
        let mut changed = false;
        let mut saved = std::time::Instant::now();
        while app.live.busy || app.request.is_some() {
            app.elapsed_seconds = started.elapsed().as_secs();
            if let Some(desk) = &context.approvals {
                for mut event in desk.drain() {
                    event["session"] = json!(session);
                    emit(event);
                }
            }
            // Checkpoint while it runs, so a follower sees the work.
            if changed && saved.elapsed() >= CHECKPOINT {
                let mut document = trajectory::main_document(app, &context.cwd);
                document["session_id"] = json!(session);
                document["trajectory_id"] = json!(session);
                document["extra"]["running"] = json!(true);
                let _ = lease.save(&document);
                saved = std::time::Instant::now();
                changed = false;
            }
            if context
                .canceled
                .as_ref()
                .is_some_and(|flag| flag.load(Ordering::Relaxed))
            {
                app.cancel_request();
            }
            background.sync(app);
            if let Some(text) = stream_delta(&app.live, &mut partial) {
                emit(json!({"event":"delta","session":session,"text":text}));
                changed = true;
            }
            for (index, entry) in app.live.entries.iter().enumerate() {
                let value = entry_value(entry);
                if previous.get(&index) != Some(&value) {
                    changed = true;
                    emit(json!({"event":"entry","session":session,"index":index,"entry":value}));
                    previous.insert(index, value);
                }
            }
            for child in &app.delegations {
                let partial = child_partial.entry(child.id.clone()).or_default();
                if let Some(text) = stream_delta(&child.chat, partial) {
                    emit(
                        json!({"event":"delegation_delta","session":session,"delegation":child.id,"text":text}),
                    );
                }
                for (index, entry) in child.chat.entries.iter().enumerate() {
                    let key = (child.id.clone(), index);
                    let value = entry_value(entry);
                    if child_previous.get(&key) != Some(&value) {
                        changed = true;
                        emit(
                            json!({"event":"delegation_entry","session":session,"delegation":child.id,"index":index,"entry":value}),
                        );
                        child_previous.insert(key, value);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(15));
        }
        if let Some(desk) = &context.approvals {
            for mut event in desk.drain() {
                event["session"] = json!(session);
                emit(event);
            }
        }
    }
    let mut document = trajectory::main_document(app, &context.cwd);
    document["session_id"] = json!(session);
    document["trajectory_id"] = json!(session);
    lease.save(&document)?;
    if let Some(error) = &app.live.notice {
        return Err(error.clone().into());
    }
    let target = match &lookup_conversation {
        Some((conversation, _)) => conversation
            .chat(app)
            .ok_or("The Brainstorm lookup conversation is unavailable.")?,
        None => app
            .selected_agent
            .and_then(|index| app.delegations.get(index))
            .map_or(&app.live, |child| &child.chat),
    };
    if let Some(error) = &target.notice {
        return Err(error.clone().into());
    }
    let reply = if let Some((_, turn_start)) = &lookup_conversation {
        // A lookup completes with its own tool observation, without a model reply.
        target
            .entries
            .get(*turn_start..)
            .unwrap_or_default()
            .iter()
            .rev()
            .find_map(|entry| match entry {
                live::Entry::Tool {
                    name,
                    output,
                    running: false,
                    ..
                } if crate::brainstorm::is_tool(name) => crate::brainstorm::context(output),
                _ => None,
            })
            .ok_or("The Brainstorm lookup completed without a bounded observation.")?
    } else {
        target
            .entries
            .iter()
            .rev()
            .find_map(|entry| {
                if let live::Entry::Assistant { text, .. } = entry {
                    Some(text.clone())
                } else {
                    None
                }
            })
            .unwrap_or_default()
    };
    Ok(
        json!({"event":"finished","session":session,"reply":reply,"tokens":target.tokens,"trajectory":session_path(context,&session)?}),
    )
}

/// How often a running chat saves its session for followers.
const CHECKPOINT: Duration = Duration::from_secs(1);

/// The approval gate of a gated chat, removed when the chat ends.
struct GateGuard(bool);

impl GateGuard {
    fn install(context: &Context) -> Self {
        let Some(desk) = &context.approvals else {
            return Self(false);
        };
        crate::approval::install(Some(crate::approval::Gate {
            desk: Arc::clone(desk),
            cancel: context
                .canceled
                .clone()
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false))),
        }));
        Self(true)
    }
}

impl Drop for GateGuard {
    fn drop(&mut self) {
        if self.0 {
            crate::approval::install(None);
        }
    }
}

#[derive(Default)]
struct PartialStream {
    text: String,
    entries: usize,
}

fn stream_delta<'a>(chat: &'a live::Chat, previous: &mut PartialStream) -> Option<&'a str> {
    let text = if previous.entries == chat.entries.len() {
        chat.partial
            .strip_prefix(&previous.text)
            .unwrap_or(&chat.partial)
    } else {
        &chat.partial
    };
    previous.text.clone_from(&chat.partial);
    previous.entries = chat.entries.len();
    (!text.is_empty()).then_some(text)
}

fn entry_value(entry: &live::Entry) -> Value {
    match entry {
        live::Entry::User(text) => json!({"source":"user","text":text}),
        live::Entry::Assistant {
            text,
            model,
            elapsed_ms,
        } => {
            json!({"source":"assistant","text":text,"model":model,"elapsed_ms":elapsed_ms})
        }
        live::Entry::Tool {
            name,
            input,
            output,
            running,
        } => json!({"source":"tool","name":name,"input":input,"output":output,"running":running}),
        live::Entry::Delegation {
            id,
            name,
            task,
            running,
            output,
        } => {
            json!({"source":"delegation","id":id,"name":name,"task":task,"running":running,"output":output})
        }
    }
}

fn delegate(
    app: &mut App,
    args: &[String],
    context: &Context,
    emit: &mut dyn FnMut(Value),
) -> Result<Value, Error> {
    let mut args = args.to_vec();
    let task = take_option(&mut args, "--task")?.ok_or_else(|| usage("Supply --task TEXT."))?;
    let session = take_option(&mut args, "--session")?.unwrap_or_else(new_id);
    if args.len() != 1 {
        return Err(usage("Use delegate AGENT --task TEXT."));
    }
    let lease = lock_session(context, &session)?;
    if lease.exists()? {
        trajectory::restore_app(app, &lease.read()?)?;
    }
    let agent = args.remove(0);
    let execution = app.plugins.execution_settings(context.cwd.clone());
    let provider = app
        .plugins
        .key_for_request()
        .filter(|_| app.plugins.enabled)
        .map(|key| {
            openrouter::Client::new(openrouter::Config::new(openrouter::ApiKey::new(
                key.expose(),
            )))
            .map(|client| crate::plugin_tools::GenerationProvider {
                client,
                model: app.plugins.model.clone(),
                effort: app.plugins.options.reasoning.clone(),
            })
        })
        .transpose()
        .map_err(|_| Error::from("Cannot create the configured model client."))?;
    app.active_options = if provider.is_some() {
        app.plugins.options.clone()
    } else {
        models::GenerationOptions::default()
    };
    let (name, arguments) = if agent == "microcoder" {
        ("microcoder", json!({"task":task}))
    } else {
        ("acp_subagent", json!({"agent":agent,"task":task}))
    };
    let cancel = context
        .canceled
        .clone()
        .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
    let (sender, receiver) = std::sync::mpsc::channel();
    let (result_sender, result_receiver) = std::sync::mpsc::channel();
    let delegation = format!("{session}-delegate-{}", app.delegations.len() + 1);
    let _gate = GateGuard::install(context);
    let delegated_name = execution
        .agents
        .iter()
        .find(|registered| registered.id == agent)
        .map_or_else(|| agent.clone(), |registered| registered.name.clone());
    let delegated_name = execution.redact_text(&delegated_name);
    let delegated_task = execution.redact_text(&task);
    let display_task = delegated_task.clone();
    let mut visible_arguments = arguments.clone();
    execution.redact(&mut visible_arguments);
    std::thread::spawn(move || {
        let mut callback = move |event: RuntimeEvent| {
            let _ = sender.send(RuntimeEvent::Delegation {
                id: delegation.clone(),
                name: delegated_name.clone(),
                task: delegated_task.clone(),
                event: Box::new(event),
            });
        };
        callback(RuntimeEvent::Tool {
            name: name.into(),
            input: visible_arguments,
            output: Value::Null,
            running: true,
        });
        let result = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Cannot start the plugin runtime.".to_owned())
            .and_then(|runtime| {
                runtime.block_on(execution.execute(
                    name,
                    arguments,
                    provider,
                    &cancel,
                    &mut callback,
                ))
            });
        let output = match &result {
            Ok(output) => output.clone(),
            Err(error) => json!({"error":error}),
        };
        callback(RuntimeEvent::Tool {
            name: name.into(),
            input: Value::Null,
            output,
            running: false,
        });
        let _ = result_sender.send(result);
    });
    app.live.entries.push(live::Entry::User(display_task));
    app.live.busy = true;
    let started = std::time::Instant::now();
    app.live.reply_started_at = Some(started);
    let result = loop {
        app.elapsed_seconds = started.elapsed().as_secs();
        if let Some(desk) = &context.approvals {
            for mut event in desk.drain() {
                event["session"] = json!(session);
                emit(event);
            }
        }
        match receiver.recv_timeout(Duration::from_millis(15)) {
            Ok(event) => {
                emit(runtime_value(&event));
                match event {
                    RuntimeEvent::Tokens(tokens) => app.live.tokens = tokens,
                    RuntimeEvent::Text(text) => app.live.partial.push_str(&text),
                    RuntimeEvent::Model(model) => app.live.partial_model = Some(model),
                    RuntimeEvent::Tool {
                        name,
                        input,
                        output,
                        running,
                    } => app.live.tool(name, input, output, running),
                    RuntimeEvent::Delegation {
                        id,
                        name,
                        task,
                        event,
                    } => app.apply_update(live::Update::Delegation {
                        id: app.request_id,
                        delegation: id,
                        name,
                        task,
                        event: *event,
                    }),
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                break result_receiver.recv().unwrap_or_else(|_| {
                    Err("The plugin worker stopped before returning a result.".into())
                });
            }
        }
    };
    app.live.busy = false;
    let output = match result {
        Ok(output) => output,
        Err(error) => {
            app.live.notice = Some(error.clone());
            let mut document = trajectory::main_document(app, &context.cwd);
            document["session_id"] = json!(session);
            document["trajectory_id"] = json!(session);
            lease.save(&document)?;
            return Err(error.into());
        }
    };
    let text = output
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    app.live.entries.push(live::Entry::Assistant {
        elapsed_ms: app.live.reply_elapsed_ms(),
        text: text.clone(),
        model: output
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned),
    });
    app.live.partial.clear();
    let mut document = trajectory::main_document(app, &context.cwd);
    document["session_id"] = json!(session);
    document["trajectory_id"] = json!(session);
    lease.save(&document)?;
    Ok(json!({"session":session,"agent":agent,"reply":text,"result":output}))
}

fn runtime_value(event: &RuntimeEvent) -> Value {
    match event {
        RuntimeEvent::Tokens(tokens) => json!({"event":"usage","tokens":tokens}),
        RuntimeEvent::Text(text) => json!({"event":"delta","text":text}),
        RuntimeEvent::Model(model) => json!({"event":"model","model":model}),
        RuntimeEvent::Tool {
            name,
            input,
            output,
            running,
        } => json!({"event":"tool","name":name,"input":input,"output":output,"running":running}),
        RuntimeEvent::Delegation {
            id,
            name,
            task,
            event,
        } => {
            json!({"event":"delegation","id":id,"name":name,"task":task,"update":runtime_value(event)})
        }
    }
}

/// The marker an earlier host used to append a workshop agent's charter to
/// the owner's words.
const APPENDED_CHARTER: &str = "\n\n---\nHow you work, as ";

/// Cuts a charter an earlier host appended from each of the owner's
/// messages, now that it travels as standing instructions.
fn forget_appended_charter(entries: &mut [live::Entry]) {
    for entry in entries {
        if let live::Entry::User(text) = entry
            && let Some(at) = text.find(APPENDED_CHARTER)
        {
            text.truncate(at);
        }
    }
}

/// Where a session's standing instructions live: beside its document.
fn instructions_path(session: &Path) -> PathBuf {
    session.with_extension("instructions")
}

fn save_instructions(path: &Path, text: &str) -> Result<(), Error> {
    let temporary = path.with_extension("instructions.tmp");
    let mut options = fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let written = options
        .open(&temporary)
        .and_then(|mut file| std::io::Write::write_all(&mut file, text.as_bytes()))
        .and_then(|()| fs::rename(&temporary, path));
    written.map_err(|_| Error::from("Cannot save the session's instructions."))
}

fn new_id() -> String {
    atif::log::session_id(atif::now_ms())
}
fn session_path(context: &Context, id: &str) -> Result<PathBuf, Error> {
    local_sessions::Store::under(&context.root)
        .path(id)
        .map_err(|error| usage(&error))
}
fn lock_session(context: &Context, id: &str) -> Result<local_sessions::Lease, Error> {
    session_path(context, id)?;
    local_sessions::Store::under(&context.root)
        .lease(id)
        .map_err(Error::from)
}
fn sessions(args: &[String], context: &Context) -> Result<Value, Error> {
    let store = local_sessions::Store::under(&context.root);
    if args.is_empty() || args == ["list"] {
        return Ok(json!({"sessions":store.list()?}));
    }
    if args.len() != 2 {
        return Err(usage("Use sessions list, read ID, or delete ID."));
    }
    session_path(context, &args[1])?;
    match args[0].as_str() {
        "read" => store.read(&args[1]).map_err(Error::from),
        "delete" => {
            let lease = lock_session(context, &args[1])?;
            let _ = fs::remove_file(instructions_path(lease.path()));
            lease.delete()?;
            // Saved to the account too (#11046): delete it there.
            drop(lease);
            crate::account_sync::forget_deleted(&context.root, &args[1]);
            Ok(json!({"deleted":args[1]}))
        }
        _ => Err(usage("Use sessions list, read ID, or delete ID.")),
    }
}
fn read_document(path: &Path) -> Result<Value, Error> {
    local_sessions::read_document(path).map_err(Error::from)
}
fn export(args: &[String], context: &Context) -> Result<Value, Error> {
    let mut args = args.to_vec();
    let output = take_option(&mut args, "--output")?;
    if args.len() != 1 {
        return Err(usage("Use export ID [--output FILE]."));
    }
    session_path(context, &args[0])?;
    let document = local_sessions::Store::under(&context.root).read(&args[0])?;
    if let Some(output) = output {
        let path = context.cwd.join(output);
        trajectory::write(&path, &document)?;
        Ok(json!({"path":path,"schema_version":atif::SCHEMA_VERSION}))
    } else {
        Ok(document)
    }
}
fn import(args: &[String], context: &Context) -> Result<Value, Error> {
    let mut args = args.to_vec();
    let session = take_option(&mut args, "--session")?.unwrap_or_else(new_id);
    if args.len() != 1 {
        return Err(usage("Use import FILE [--session ID]."));
    }
    let lease = lock_session(context, &session)?;
    let path = lease.path().to_owned();
    if lease.exists()? {
        return Err("That session already exists. Choose another ID.".into());
    }
    let mut document = read_document(&context.cwd.join(&args[0]))?;
    trajectory::from_document(&document)?;
    document["session_id"] = json!(session);
    document["trajectory_id"] = json!(session);
    atif::upgrade(&mut document).map_err(|_| Error::from("Unsupported ATIF version."))?;
    lease.save(&document)?;
    Ok(json!({"session":session,"path":path}))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context(temp: &tempfile::TempDir) -> Context {
        Context {
            root: temp.path().join("state"),
            cwd: temp.path().into(),
            environment: BTreeMap::new(),
            input: None,
            canceled: None,
            approvals: None,
        }
    }
    fn execute_words(words: &[&str], context: &Context) -> Result<Value, Error> {
        execute(
            &words
                .iter()
                .map(|word| word.to_string())
                .collect::<Vec<_>>(),
            context,
            &mut |_| {},
        )
    }

    #[test]
    fn final_builds_reject_demo_before_loading_state_or_emitting_events() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(&temp);
        for words in [
            vec!["chat", "-p", "hello", "--session", "test", "--demo"],
            vec!["plugins", "enable", "openrouter-byok", "--demo"],
            vec!["export", "missing", "--output", "copy.json", "--demo"],
        ] {
            let arguments = words
                .iter()
                .map(|word| (*word).to_owned())
                .collect::<Vec<_>>();
            let error = execute_with_demo_policy(
                &arguments,
                &context,
                &mut |_| panic!("A rejected command must not emit events."),
                false,
            )
            .unwrap_err();
            assert!(error.usage);
            assert_eq!(
                error.message,
                "Demo mode is available only in local development builds."
            );
            assert!(!context.root.exists());
            assert!(!context.cwd.join("copy.json").exists());
        }
    }

    #[test]
    fn demo_policy_keeps_development_help_and_allows_regular_final_commands() {
        let demo = ["chat".into(), "--demo".into()];
        assert!(check_demo(&demo, true).is_ok());
        assert!(check_demo(&["status".into()], false).is_ok());
        assert!(command_usage!(" [--demo]").contains("[--demo]"));
        assert!(!command_usage!("").contains("demo"));
        assert_eq!(USAGE.contains("[--demo]"), crate::DEMO_AVAILABLE);
    }

    #[test]
    fn command_settings_use_the_same_store_without_exposing_keys() {
        let temp = tempfile::tempdir().unwrap();
        let mut context = context(&temp);
        context.input = Some(r#"{"api_key":"fake-secret","enabled":true}"#.into());
        let result = execute_words(
            &["plugins", "configure", "openrouter-byok", "--stdin"],
            &context,
        )
        .unwrap();
        assert!(!result.to_string().contains("fake-secret"));
        let result = execute_words(&["models", "set", "openai/gpt-6-luna:low"], &context).unwrap();
        assert_eq!(result["model"], "openai/gpt-6-luna:low");
        let result = execute_words(&["plugins", "disable", "openrouter-byok"], &context).unwrap();
        assert_eq!(result["plugins"][0]["enabled"], false);
        let saved = crate::plugin_store::Store::under(&context.root)
            .load()
            .unwrap();
        assert_eq!(saved.options.reasoning.as_deref(), Some("low"));
        assert_eq!(saved.key.unwrap().expose(), "fake-secret");
    }
    #[test]
    fn demo_session_can_be_read_exported_imported_and_continued() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(&temp);
        execute_words(
            &["chat", "-p", "hello", "--session", "test", "--demo"],
            &context,
        )
        .unwrap();
        let document =
            execute_words(&["export", "test", "--output", "copy.json"], &context).unwrap();
        assert_eq!(document["schema_version"], "ATIF-v1.8");
        execute_words(&["import", "copy.json", "--session", "imported"], &context).unwrap();
        execute_words(
            &["chat", "-p", "more", "--session", "imported", "--demo"],
            &context,
        )
        .unwrap();
        let result = execute_words(&["sessions", "read", "imported"], &context).unwrap();
        assert_eq!(result["steps"].as_array().unwrap().len(), 4);
        assert!(execute_words(&["sessions", "read", "../escape"], &context).is_err());
        assert!(
            execute_words(&["import", "copy.json", "--session", "imported"], &context).is_err()
        );
    }
    #[test]
    fn standing_instructions_stay_out_of_the_transcript_and_exports() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(&temp);
        // An earlier host appended the charter to the owner's words.
        execute_words(
            &[
                "chat",
                "-p",
                "hi\n\n---\nHow you work, as alice, the owner's workshop agent",
                "--session",
                "agent-alice",
                "--demo",
            ],
            &context,
        )
        .unwrap();
        execute_words(
            &[
                "chat",
                "-p",
                "tell me about your environment",
                "--session",
                "agent-alice",
                "--instructions",
                "HIDDEN CHARTER",
                "--demo",
            ],
            &context,
        )
        .unwrap();
        let session = execute_words(&["sessions", "read", "agent-alice"], &context).unwrap();
        let text = session.to_string();
        assert!(!text.contains("HIDDEN CHARTER"));
        assert!(!text.contains("How you work"));
        assert!(text.contains("tell me about your environment"));
        let exported = execute_words(
            &["export", "agent-alice", "--output", "copy.json"],
            &context,
        )
        .unwrap();
        assert!(!exported.to_string().contains("HIDDEN CHARTER"));
        let path = session_path(&context, "agent-alice").unwrap();
        assert_eq!(
            fs::read_to_string(instructions_path(&path)).unwrap(),
            "HIDDEN CHARTER"
        );
        assert!(
            execute_words(
                &[
                    "chat",
                    "-p",
                    "x",
                    "--instructions",
                    "a",
                    "--instructions-file",
                    "b",
                    "--demo"
                ],
                &context,
            )
            .is_err()
        );
        execute_words(&["sessions", "delete", "agent-alice"], &context).unwrap();
        assert!(!instructions_path(&path).exists());
    }

    #[test]
    fn invalid_key_and_model_options_preserve_saved_settings() {
        let temp = tempfile::tempdir().unwrap();
        let mut context = context(&temp);
        context.input = Some(r#"{"api_key":"contains whitespace"}"#.into());
        assert!(execute_words(&["plugins", "configure", "jev", "--stdin"], &context).is_err());
        assert!(
            execute_words(
                &["models", "set", "openrouter/free", "--reasoning", "high"],
                &context
            )
            .is_err()
        );
        assert!(!context.root.join("plugins.json").exists());
    }

    #[test]
    fn session_writes_are_exclusive_and_release_the_lock_after_errors() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(&temp);
        let held = lock_session(&context, "one").unwrap();
        assert!(
            execute_words(
                &["chat", "-p", "hello", "--session", "one", "--demo"],
                &context
            )
            .is_err()
        );
        drop(held);
        assert!(
            execute_words(
                &["chat", "-p", "hello", "--session", "one", "--demo"],
                &context
            )
            .is_ok()
        );
    }

    #[test]
    fn gateway_is_available_without_an_openrouter_key() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(&temp);
        let status = execute_words(&["status"], &context).unwrap();
        assert_eq!(status["model"], "auto");
        let catalog = execute_words(&["models", "list"], &context).unwrap();
        assert_eq!(catalog["models"][0]["id"], "auto");
        let selected = execute_words(&["models", "set", "auto"], &context).unwrap();
        assert_eq!(selected["model"], "auto");
        execute_words(&["plugins", "enable", "openrouter-byok"], &context).unwrap();
        let status = execute_words(&["status"], &context).unwrap();
        assert_eq!(status["model"], "auto");
        let catalog = execute_words(&["models", "list"], &context).unwrap();
        assert!(catalog["models"].as_array().unwrap().len() > 1);
    }

    #[test]
    fn failed_delegation_keeps_a_child_chat_that_can_be_continued_without_changing_the_parent() {
        let temp = tempfile::tempdir().unwrap();
        let context = context(&temp);
        execute_words(&["plugins", "disable", "microcoder"], &context).unwrap();
        assert!(
            execute_words(
                &[
                    "delegate",
                    "microcoder",
                    "--task",
                    "Review the parser",
                    "--session",
                    "parent"
                ],
                &context,
            )
            .is_err()
        );
        let retained = execute_words(&["sessions", "read", "parent"], &context).unwrap();
        assert_eq!(retained["steps"].as_array().unwrap().len(), 2);
        assert_eq!(
            retained["steps"][1]["tool_calls"][0]["function_name"],
            "delegate"
        );
        assert_eq!(
            retained["subagent_trajectories"][0]["extra"]["agent"],
            "microcoder"
        );
        assert!(retained["subagent_trajectories"][0]["extra"]["notice"].is_string());
        let child = retained["subagent_trajectories"][0]["session_id"]
            .as_str()
            .unwrap();
        let result = execute_words(
            &[
                "chat",
                "-p",
                "Try this instead",
                "--session",
                "parent",
                "--delegation",
                child,
                "--demo",
            ],
            &context,
        )
        .unwrap();
        assert!(result["reply"].as_str().unwrap().starts_with("Demo reply."));
        let retained = execute_words(&["sessions", "read", "parent"], &context).unwrap();
        assert_eq!(retained["steps"].as_array().unwrap().len(), 2);
        assert_eq!(
            retained["subagent_trajectories"][0]["steps"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
    }

    #[test]
    fn streamed_text_continues_after_tool_calls_and_handles_unicode() {
        let mut chat = live::Chat::default();
        let mut previous = PartialStream::default();
        chat.partial = "Checking κ".into();
        assert_eq!(stream_delta(&chat, &mut previous), Some("Checking κ"));
        chat.finish_partial();
        chat.tool(
            "Read".into(),
            json!({"path":"src/lib.rs"}),
            Value::Null,
            true,
        );
        chat.partial = "Checking κ again".into();
        assert_eq!(stream_delta(&chat, &mut previous), Some("Checking κ again"));
        chat.partial.push('λ');
        assert_eq!(stream_delta(&chat, &mut previous), Some("λ"));
        chat.finish_partial();
        assert_eq!(stream_delta(&chat, &mut previous), None);
    }
}
