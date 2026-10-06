//! Programmatic access to the terminal's settings, chats, and plugin runtime.

use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
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
    plugins::SettingsFocus, trajectory,
};

pub const USAGE: &str = "usage: openagents coder COMMAND [OPTIONS]
  status                              Provider, model, plugins, and working directory.
  chat [-p TEXT | --prompt-file FILE | --stdin] [--session ID] [--delegation ID] [--demo]
                                      Run the same chat and tools as the terminal.
  delegate AGENT --task TEXT [--session ID]
                                      Run Microcoder or an enabled ACP subagent.
  plugins list                        List registered plugins and their status.
  plugins enable ID                   Turn a registered plugin on.
  plugins disable ID                  Turn a registered plugin off.
  plugins configure ID --stdin        Configure OpenRouter or Jev with a JSON object:
                                      api_key, model, endpoint, enabled.
                                      An api_key of null removes the saved key.
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
         --in DIR sets the working directory; --state DIR sets the Coder store.
Settings default to ~/.openagents/coder-new. Sessions use its sessions directory.
Keys are accepted through environment variables or configuration stdin, never printed.";

/// Explicit roots and environment make command execution usable from other hosts.
pub struct Context {
    pub root: PathBuf,
    pub cwd: PathBuf,
    pub environment: BTreeMap<String, String>,
    pub input: Option<String>,
    pub canceled: Option<Arc<AtomicBool>>,
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

/// Dispatch one CLI call. JSON mode streams events, followed by a result document.
pub fn run(arguments: &[String], json_mode: bool) -> u8 {
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
    let mut environment: BTreeMap<String, String> = [
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
    ]
    .into_iter()
    .filter_map(|name| std::env::var(name).ok().map(|value| (name.into(), value)))
    .collect();
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
        .is_some_and(|arg| matches!(arg.as_str(), "chat" | "run" | "delegate"))
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
        "openagents/gateway".into()
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
            json!({"plugin":crate::plugin_definition::FALLBACK_PROVIDER.id,"id":"openagents/gateway","name":"OpenAgents AI Gateway","reasoning":[],"default":true}),
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
    if slug == "openagents/gateway" {
        if reasoning.is_some() || max_tokens.is_some() {
            return Err(usage(
                "The automatic gateway does not expose reasoning or output limits.",
            ));
        }
        if app.plugins.enabled && !app.plugins.toggle_enabled() {
            return Err("Cannot save provider selection.".into());
        }
        return Ok(json!({"model":"openagents/gateway","provider":"openagents-gateway"}));
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

fn chat(
    app: &mut App,
    args: &[String],
    context: &Context,
    emit: &mut dyn FnMut(Value),
) -> Result<Value, Error> {
    let mut args = args.to_vec();
    let session = take_option(&mut args, "--session")?.unwrap_or_else(new_id);
    let delegation = take_option(&mut args, "--delegation")?;
    let prompt = take_option(&mut args, "-p")?.or(take_option(&mut args, "--prompt")?);
    let file = take_option(&mut args, "--prompt-file")?;
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
    let _lock = lock_session(context, &session)?;
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
    if session_path(context, &session)?.exists() {
        trajectory::restore_app(app, &read_document(&session_path(context, &session)?)?)?;
    }
    if let Some(id) = delegation {
        app.selected_agent = Some(
            app.delegations
                .iter()
                .position(|child| child.id == id)
                .ok_or_else(|| usage("That delegation is not part of the selected session."))?,
        );
    }
    if demo {
        let target = if let Some(index) = app.selected_agent {
            &mut app.delegations[index].chat
        } else {
            &mut app.live
        };
        target.entries.push(live::Entry::User(prompt));
        target.entries.push(live::Entry::Assistant {
            text: "Demo reply. Live execution uses the terminal's provider and plugin runtime."
                .into(),
            model: Some("demo/local".into()),
        });
    } else {
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
        let mut previous = BTreeMap::new();
        let mut child_previous = BTreeMap::new();
        let mut child_partial = BTreeMap::new();
        let started = std::time::Instant::now();
        while app.live.busy || app.request.is_some() {
            app.elapsed_seconds = started.elapsed().as_secs();
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
            }
            for (index, entry) in app.live.entries.iter().enumerate() {
                let value = entry_value(entry);
                if previous.get(&index) != Some(&value) {
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
                        emit(
                            json!({"event":"delegation_entry","session":session,"delegation":child.id,"index":index,"entry":value}),
                        );
                        child_previous.insert(key, value);
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }
    let mut document = trajectory::main_document(app, &context.cwd);
    document["session_id"] = json!(session);
    document["trajectory_id"] = json!(session);
    save_session(context, &session, &document)?;
    if let Some(error) = &app.live.notice {
        return Err(error.clone().into());
    }
    let target = app
        .selected_agent
        .and_then(|index| app.delegations.get(index))
        .map_or(&app.live, |child| &child.chat);
    if let Some(error) = &target.notice {
        return Err(error.clone().into());
    }
    let reply = target
        .entries
        .iter()
        .rev()
        .find_map(|entry| {
            if let live::Entry::Assistant { text, .. } = entry {
                Some(text.as_str())
            } else {
                None
            }
        })
        .unwrap_or("");
    Ok(
        json!({"event":"finished","session":session,"reply":reply,"tokens":target.tokens,"trajectory":session_path(context,&session)?}),
    )
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
        live::Entry::Assistant { text, model } => {
            json!({"source":"assistant","text":text,"model":model})
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
    let _lock = lock_session(context, &session)?;
    if session_path(context, &session)?.exists() {
        trajectory::restore_app(app, &read_document(&session_path(context, &session)?)?)?;
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
    let result = loop {
        app.elapsed_seconds = started.elapsed().as_secs();
        match receiver.recv_timeout(Duration::from_millis(15)) {
            Ok(event) => {
                emit(runtime_value(&event));
                match event {
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
            save_session(context, &session, &document)?;
            return Err(error.into());
        }
    };
    let text = output
        .get("reply")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_owned();
    app.live.entries.push(live::Entry::Assistant {
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
    save_session(context, &session, &document)?;
    Ok(json!({"session":session,"agent":agent,"reply":text,"result":output}))
}

fn runtime_value(event: &RuntimeEvent) -> Value {
    match event {
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

fn new_id() -> String {
    atif::log::session_id(atif::now_ms())
}
fn session_path(context: &Context, id: &str) -> Result<PathBuf, Error> {
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"_-".contains(&byte))
    {
        return Err(usage(
            "Session IDs accept letters, numbers, underscores, and hyphens, up to 128 bytes.",
        ));
    }
    Ok(context
        .root
        .join("sessions")
        .join(format!("{id}.atif.json")))
}
fn lock_session(context: &Context, id: &str) -> Result<File, Error> {
    let path = session_path(context, id)?.with_extension("lock");
    let parent = path
        .parent()
        .ok_or_else(|| usage("Invalid session path."))?;
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(parent)
        .map_err(|_| Error::from("Cannot create the chat session directory."))?;
    if !fs::symlink_metadata(parent).is_ok_and(|metadata| metadata.is_dir()) {
        return Err("The chat session directory is not a regular directory.".into());
    }
    if fs::symlink_metadata(&path).is_ok_and(|metadata| !metadata.is_file()) {
        return Err("The chat session lock is not a regular file.".into());
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|_| Error::from("Cannot lock the chat session."))?;
    file.try_lock()
        .map_err(|_| Error::from("Another process is using this chat session."))?;
    Ok(file)
}

fn save_session(context: &Context, id: &str, document: &Value) -> Result<(), Error> {
    let path = session_path(context, id)?;
    let temporary = path.with_extension(format!("{}.tmp", new_id()));
    let result = trajectory::write(&temporary, document)
        .map_err(Error::from)
        .and_then(|()| {
            fs::rename(&temporary, &path).map_err(|_| Error::from("Cannot save the chat session."))
        });
    if result.is_err() {
        let _ = fs::remove_file(temporary);
    }
    result
}
fn sessions(args: &[String], context: &Context) -> Result<Value, Error> {
    if args.is_empty() || args == ["list"] {
        let root = context.root.join("sessions");
        let mut values = Vec::new();
        if root.exists() {
            for entry in
                fs::read_dir(&root).map_err(|_| Error::from("Cannot list chat sessions."))?
            {
                let entry = entry.map_err(|_| Error::from("Cannot list chat sessions."))?;
                let name = entry.file_name().to_string_lossy().into_owned();
                if let Some(id) = name.strip_suffix(".atif.json") {
                    values.push(json!({"id":id,"path":entry.path()}));
                }
            }
        }
        values.sort_by_key(|value| value["id"].as_str().unwrap_or("").to_owned());
        return Ok(json!({"sessions":values}));
    }
    if args.len() != 2 {
        return Err(usage("Use sessions list, read ID, or delete ID."));
    }
    let path = session_path(context, &args[1])?;
    let _lock = lock_session(context, &args[1])?;
    match args[0].as_str() {
        "read" => read_document(&path),
        "delete" => {
            fs::remove_file(path).map_err(|_| Error::from("Cannot remove the chat session."))?;
            Ok(json!({"deleted":args[1]}))
        }
        _ => Err(usage("Use sessions list, read ID, or delete ID.")),
    }
}
fn read_document(path: &Path) -> Result<Value, Error> {
    let metadata =
        fs::symlink_metadata(path).map_err(|_| Error::from("Cannot read the chat session."))?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err("The chat session is not a regular file or exceeds 64 MiB.".into());
    }
    let value = serde_json::from_slice(
        &fs::read(path).map_err(|_| Error::from("Cannot read the chat session."))?,
    )
    .map_err(|_| Error::from("The chat session is not valid JSON."))?;
    if !atif::validate(&value).is_empty() {
        return Err("The chat session is not valid ATIF.".into());
    }
    trajectory::from_document(&value)?;
    Ok(value)
}
fn export(args: &[String], context: &Context) -> Result<Value, Error> {
    let mut args = args.to_vec();
    let output = take_option(&mut args, "--output")?;
    if args.len() != 1 {
        return Err(usage("Use export ID [--output FILE]."));
    }
    let document = read_document(&session_path(context, &args[0])?)?;
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
    let _lock = lock_session(context, &session)?;
    let path = session_path(context, &session)?;
    if path.exists() {
        return Err("That session already exists. Choose another ID.".into());
    }
    let mut document = read_document(&context.cwd.join(&args[0]))?;
    trajectory::from_document(&document)?;
    document["session_id"] = json!(session);
    document["trajectory_id"] = json!(session);
    atif::upgrade(&mut document).map_err(|_| Error::from("Unsupported ATIF version."))?;
    save_session(context, &session, &document)?;
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
        assert_eq!(status["model"], "openagents/gateway");
        let catalog = execute_words(&["models", "list"], &context).unwrap();
        assert_eq!(catalog["models"][0]["id"], "openagents/gateway");
        execute_words(&["plugins", "enable", "openrouter-byok"], &context).unwrap();
        let status = execute_words(&["status"], &context).unwrap();
        assert_eq!(status["model"], "openagents/gateway");
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
