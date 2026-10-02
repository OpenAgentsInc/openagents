//! `openagents terminal`: OpenAgents Terminal, a full-screen chat with
//! OpenAgents (`docs/terminal/README.md`). Bare `openagents` on a terminal
//! opens it too.
//!
//! The screen itself is `openagents_terminal`; this module opens the shared
//! chat client the way `openagents chat` does (this computer's host when
//! one answers, else this command's own store, or a scratch store) and
//! hands the screen what only this program can do through
//! [`ProgramExtras`]: pairing a phone over the host's control socket,
//! installing the host as a user service, listing plugins, and reading the
//! Coder settings.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::task::chat_client::{Control, Here};
use openagents_chat::client::{self, Client, Event, Kind, Place};
use openagents_chat::router::Caller;
use openagents_connect::control::{self, Op, Reply, Request};
use openagents_terminal::{Extras, Interrupter, Invite, Launch, Plugin, Resume, Settings};
use serde_json::Value;

use crate::{Args, Output, runtime};

pub(crate) const USAGE: &str =
    "usage: openagents terminal [--thread ID] [--continue] [--scratch] [--local] [--socket PATH]
                          [--computer HOST] [--resume [ID|TITLE]]
OpenAgents Terminal: a full-screen chat with OpenAgents in this terminal.
Type a message and press Enter. It opens on a new thread; --continue opens
the last thread you had open in this folder, --thread ID opens that thread,
--resume ID|TITLE opens the thread an ID, ID prefix, or title names (bare
--resume is --continue), and /resume or Ctrl+T lists them all. When
this computer's host runs, the threads are the desktop app's threads;
--socket names another control socket and --local skips the host. --scratch
uses a throwaway identity and thread store; reopen that thread with
--scratch --thread ID. --computer HOST opens another computer's threads
instead, as a paired phone does (HOST is a name, key, or key prefix from
`openagents computer list`; pair first with `openagents computer link`):
continue them there, and its Coder runs there. Bare `openagents` with no
command opens this screen when it runs on a terminal.";

/// What the command does and where the phone runs it, for the chat
/// router's command tree (`coder::cli_route::tree`). It holds the
/// terminal until the person quits.
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[Declared::computer("", Effect::LongRunning)];

const OPTIONS: &[&str] = &["thread", "socket", "computer", "resume"];
// `--new` is the default now and still accepted.
// `--resume` takes an optional value: the words after it.
const SWITCHES: &[&str] = &["scratch", "local", "new", "continue", "resume"];

/// How a Coder question is answered in the screen.
const ANSWER_HINT: &str = "Type your answer and press Enter.";

/// How long `/plugins` waits for the catalog.
const PLUGINS_WAIT: Duration = Duration::from_secs(15);

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|word| matches!(word.as_str(), "help" | "-h" | "--help"))
    {
        println!("{USAGE}");
        return 0;
    }
    let args = match Args::parse(words, SWITCHES) {
        Ok(args) => args,
        Err(message) => return output.usage("terminal", &message, USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !OPTIONS.contains(name))
    {
        return output.usage("terminal", &format!("unknown option `--{name}`"), USAGE);
    }
    let find = match args.option("resume") {
        Some(value) => Some(value.to_owned()),
        None => args.switch("resume").then(|| args.positional().join(" ")),
    };
    if !args.switch("resume")
        && let Some(word) = args.positional().first()
    {
        return output.usage("terminal", &format!("unexpected argument `{word}`"), USAGE);
    }
    let thread = args.option("thread").map(str::to_owned);
    if let Some(id) = &thread
        && !client::thread_id(id)
    {
        return output.usage(
            "terminal",
            "a thread ID is 32 lowercase hex characters",
            USAGE,
        );
    }
    if thread.is_some() && (args.switch("new") || args.switch("continue")) {
        return output.usage(
            "terminal",
            "--thread goes alone, without --new or --continue",
            USAGE,
        );
    }
    if args.switch("new") && args.switch("continue") {
        return output.usage("terminal", "--new and --continue do not go together", USAGE);
    }
    if find.is_some()
        && (thread.is_some()
            || args.switch("new")
            || args.switch("continue")
            || args.switch("scratch"))
    {
        return output.usage(
            "terminal",
            "--resume goes without --thread, --new, --continue, or --scratch",
            USAGE,
        );
    }
    let computer = args.option("computer").map(str::to_owned);
    if computer.is_some()
        && (args.switch("scratch") || args.switch("local") || args.option("socket").is_some())
    {
        return output.usage(
            "terminal",
            "--computer goes without --scratch, --local, or --socket",
            USAGE,
        );
    }
    let scratch = args.switch("scratch");
    // A scratch store holds one thread: a fresh one, or the one named.
    let (store_thread, new, resume) = match (scratch, thread) {
        (true, None) => {
            let id = client::new_id();
            (Some(id.clone()), true, Resume::New(Some(id)))
        }
        (true, Some(id)) => (Some(id.clone()), false, Resume::Thread(id)),
        (false, Some(id)) => (Some(id.clone()), false, Resume::Thread(id)),
        (false, None) if args.switch("continue") => (None, false, Resume::LastForFolder),
        (false, None) => match find {
            Some(arg) if arg.trim().is_empty() => (None, false, Resume::LastForFolder),
            Some(arg) => (None, false, Resume::Find(arg)),
            None => (None, false, Resume::New(None)),
        },
    };
    let place = if scratch {
        Place::Scratch
    } else if args.switch("local") {
        Place::Local
    } else {
        Place::Auto {
            socket: args.option("socket").map(PathBuf::from),
        }
    };
    let home = match (&place, &store_thread) {
        (Place::Scratch, Some(id)) => client::scratch_dir(id),
        _ => client::home(),
    };
    let interrupter = Interrupter::new();
    let mut options = client::Options::new(Caller::TERMINAL);
    options.place = place;
    options.interrupt = interrupter.interrupt();
    options.hint = Some(answer_hint);
    let socket = args
        .option("socket")
        .map(PathBuf::from)
        .or_else(control::socket_path);
    let runtime = runtime();
    // Another computer's threads: this device's grant for it, its link up.
    // The live service stays open while the screen runs.
    let (remote, _live) = match &computer {
        Some(name) => match open_computer(&args, &runtime, name) {
            Ok((remote, label, live)) => (Some((remote, label)), Some(live)),
            Err(message) => return output.fail("terminal", &message),
        },
        None => (None, None),
    };
    let flag = match &computer {
        Some(name) => format!(" --computer {name}"),
        None => String::new(),
    };
    let result = runtime.block_on(async move {
        let mut notices = Vec::new();
        let client = match remote {
            Some((remote, label)) => {
                Client::over_computer(Box::new(remote), label, options, Arc::new(Here))
            }
            None => Client::open(
                options,
                &Control,
                Arc::new(Here),
                store_thread.as_deref(),
                new,
                &mut |event| {
                    if let Some(notice) = notice(event) {
                        notices.push(notice);
                    }
                },
            )
            .await
            .map_err(Failure::Client)?,
        };
        let kind = client.kind();
        let launch = Launch {
            client,
            coder: Arc::new(Here),
            interrupter,
            extras: Arc::new(ProgramExtras::new(socket)),
            resume,
            folder: std::env::current_dir().ok(),
            home,
            notices,
            version: crate::version_line(),
        };
        let exit = openagents_terminal::run(launch)
            .await
            .map_err(Failure::Io)?;
        Ok::<_, Failure>((kind, exit))
    });
    match result {
        Ok((kind, exit)) => {
            // The screen has given the terminal back by now.
            if let Some(id) = exit.thread {
                let flag = if flag.is_empty() {
                    self::flag(kind).to_owned()
                } else {
                    flag
                };
                for line in closing(&flag, &id, exit.running) {
                    eprintln!("{line}");
                }
            }
            0
        }
        Err(Failure::Client(client::Error::Usage(message))) => {
            output.usage("terminal", &message, USAGE)
        }
        Err(Failure::Client(client::Error::Failed(message))) => output.fail("terminal", &message),
        Err(Failure::Io(error)) => output.fail("terminal", &error.to_string()),
    }
}

enum Failure {
    Client(client::Error),
    Io(std::io::Error),
}

fn answer_hint(_: Kind, _: &str) -> String {
    ANSWER_HINT.to_owned()
}

/// What opening the client said that the screen shows first, in the words
/// `openagents chat` prints.
fn notice(event: Event) -> Option<String> {
    match event {
        Event::Migrated { moved } => Some(format!(
            "moved {moved} thread{} kept without a host into this computer's host",
            if moved == 1 { "" } else { "s" }
        )),
        Event::Kept { home, message } => Some(format!(
            "threads in {} stay there for now: {message}",
            home.display()
        )),
        _ => None,
    }
}

/// The switch that reaches the thread's store again, as `openagents chat`
/// names it.
fn flag(kind: Kind) -> &'static str {
    match kind {
        Kind::Scratch => " --scratch",
        Kind::InProcess => " --local",
        Kind::Host | Kind::Computer => "",
    }
}

/// Another computer's threads: this device's live service for its paired
/// computers, the computer `name` names, its link up, and its label.
fn open_computer(
    args: &Args,
    runtime: &tokio::runtime::Runtime,
    name: &str,
) -> Result<
    (
        openagents_chat_app::computer_chats::Remote,
        String,
        coder_computers::live::Live,
    ),
    String,
> {
    use coder_computers::ComputersService as _;
    let mut live = crate::computer::open(args, runtime)?;
    let host = crate::computer::host_arg_text(&mut live, args, name)?;
    crate::computer::connected(&mut live, &host, args)?;
    let label = live
        .snapshot()
        .ok()
        .and_then(|snapshot| snapshot.host(&host).map(|record| record.label.clone()))
        .unwrap_or_else(|| name.to_owned());
    let link = Arc::new(openagents_chat_app::host_threads::Live::new(
        live.terminals(),
        runtime.handle().clone(),
    ));
    Ok((
        openagents_chat_app::computer_chats::Remote::new(link, host),
        label,
        live,
    ))
}

/// What the program prints once the screen closes. `flag` reaches the
/// thread's store again.
fn closing(flag: &str, thread: &str, running: bool) -> Vec<String> {
    let mut lines = vec![
        format!("thread {thread}"),
        format!("Resume with: openagents terminal --thread {thread}{flag}"),
    ];
    if running {
        lines
            .push("Coder keeps working on it; the screen follows it again when you resume.".into());
    }
    lines
}

/// What the screen needs from this computer: the host's control socket for
/// pairing, the service installer, the plugin catalog, and the settings.
pub(crate) struct ProgramExtras {
    socket: Option<PathBuf>,
    /// The devices each open invitation's host already knew, so `paired`
    /// reports only the phone that redeemed it.
    known: Mutex<HashMap<String, Vec<String>>>,
}

impl ProgramExtras {
    fn new(socket: Option<PathBuf>) -> Self {
        Self {
            socket,
            known: Mutex::new(HashMap::new()),
        }
    }

    /// One call to the host's control socket, blocking.
    fn call(&self, op: Op) -> Result<Reply, String> {
        let socket = self.socket.clone().ok_or_else(no_host)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Cannot start the runtime to reach the host.".to_owned())?;
        runtime.block_on(async move {
            let mut stream = crate::dial_control(&socket).await.map_err(|_| no_host())?;
            match control::call(&mut stream, &Request::new(1, op)).await {
                Ok(Reply::Refused { message, .. }) => Err(format!("The host refused: {message}.")),
                Ok(reply) => Ok(reply),
                Err(error) => Err(format!("The host did not answer: {error}.")),
            }
        })
    }

    fn devices(&self) -> Result<Vec<control::Device>, String> {
        match self.call(Op::DeviceList {})? {
            Reply::Devices { devices } => Ok(devices),
            _ => Err(other_answer()),
        }
    }

    fn host_answers(&self) -> bool {
        self.socket
            .as_ref()
            .is_some_and(|socket| crate::host_answers_at(socket))
    }

    /// What `sync` needs to know, read without changing anything. Past a
    /// host that answers, nothing more is read.
    fn observe(&self) -> Facts {
        if self.host_answers() {
            return Facts {
                host_answers: true,
                ..Facts::default()
            };
        }
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .filter(|home| home.is_absolute());
        let bundle_selected = home.as_ref().is_some_and(|home| {
            matches!(
                coder_service::bundle::selected(&home.join(".openagents/host-bundle")),
                Ok(Some(_))
            )
        });
        let program = std::env::current_exe().ok();
        let launcher_present = program
            .as_ref()
            .and_then(|program| program.parent())
            .is_some_and(|dir| dir.join("coder-service").is_file());
        // The host's public key, from the program's own read-only command.
        let host_key = program.as_ref().and_then(|program| {
            let output = Command::new(program)
                .args(["host", "public-key"])
                .stdin(Stdio::null())
                .output()
                .ok()?;
            let key = String::from_utf8_lossy(&output.stdout).trim().to_owned();
            (output.status.success() && coder_service::descriptor::validate_host_key(&key).is_ok())
                .then_some(key)
        });
        Facts {
            host_answers: false,
            service_manager: service_manager_here(),
            bundle_selected,
            launcher_present,
            host_key,
        }
    }
}

fn no_host() -> String {
    "Pairing needs this computer's host. Open the OpenAgents app, or run `openagents host serve --control`.".into()
}

fn other_answer() -> String {
    "The host answered another question.".into()
}

impl Extras for ProgramExtras {
    fn invite(&self) -> Result<Invite, String> {
        let before = self.devices()?;
        let Reply::Invite {
            invitation,
            code,
            expires_at,
            ..
        } = self.call(Op::InviteCreate {})?
        else {
            return Err(other_answer());
        };
        // The QR carries the link form, so a phone's own camera opens the app.
        let qr = openagents_connect::code::link(&code)
            .and_then(|link| {
                coder_connect::pairing::text_qr_rows_prefixed(
                    openagents_connect::code::LINK_PREFIX,
                    &link,
                )
                .ok()
            })
            .ok_or_else(|| "The pairing code cannot be drawn as a QR code.".to_owned())?;
        if let Ok(mut known) = self.known.lock() {
            known.insert(
                invitation.clone(),
                before.into_iter().map(|device| device.device).collect(),
            );
        }
        Ok(Invite {
            invitation,
            code,
            qr,
            expires_at,
        })
    }

    fn paired(&self, invite: &Invite) -> Result<Option<String>, String> {
        let known = self
            .known
            .lock()
            .ok()
            .and_then(|known| known.get(&invite.invitation).cloned())
            .unwrap_or_default();
        Ok(self
            .devices()?
            .into_iter()
            .find(|device| !device.revoked && !known.contains(&device.device))
            .map(|device| {
                if device.label.trim().is_empty() {
                    device.device
                } else {
                    device.label
                }
            }))
    }

    fn cancel(&self, invite: &Invite) {
        let _ = self.call(Op::InviteCancel {
            invitation: invite.invitation.clone(),
        });
        if let Ok(mut known) = self.known.lock() {
            known.remove(&invite.invitation);
        }
    }

    fn sync(&self) -> Result<String, String> {
        match decide(&self.observe()) {
            Sync::AlreadyRuns => Ok(ALREADY_RUNS.into()),
            Sync::Missing(why) => Err(why),
            Sync::Install { host_key } => install(&host_key),
        }
    }

    fn plugins(&self) -> Result<Vec<Plugin>, String> {
        let mut rows = installed_plugins(&crate::ext_eval::openagents_home().join("extensions"));
        // Whether each is on here, as the host reads it.
        #[cfg(unix)]
        {
            let here = crate::plugin_local::installed_here();
            for row in &mut rows {
                let key = row.key.as_deref().map(|key| {
                    PathBuf::from(key)
                        .canonicalize()
                        .unwrap_or_else(|_| key.into())
                });
                if let Some(found) = here.iter().find(|plugin| key.as_ref() == Some(&plugin.dir)) {
                    row.on = Some(found.enabled);
                    row.id = Some(found.id.clone());
                }
            }
        }
        let published = std::env::current_exe()
            .map_err(|_| "Cannot find this openagents program to list plugins.".to_owned())
            .and_then(|program| {
                captured(
                    Command::new(program).args(["--json", "plugin", "search", "--limit", "30"]),
                    PLUGINS_WAIT,
                )
                .map_err(|why| format!("The plugin catalog could not be read: {why}"))
            })
            .and_then(|text| plugin_rows(&text));
        match published {
            Ok(published) => {
                let names: Vec<String> = rows.iter().map(|row| row.name.clone()).collect();
                let ids: Vec<String> = rows.iter().filter_map(|row| row.id.clone()).collect();
                rows.extend(
                    published
                        .into_iter()
                        .filter(|row| !names.contains(&row.name))
                        .filter(|row| row.id.as_ref().is_none_or(|id| !ids.contains(id))),
                );
            }
            // The ones installed here still run without the catalog.
            Err(why) if rows.is_empty() => return Err(why),
            Err(_) => {}
        }
        Ok(rows)
    }

    fn run_plugin(
        &self,
        key: &str,
        request: &str,
        folder: Option<&std::path::Path>,
    ) -> Result<String, String> {
        let program = std::env::current_exe()
            .map_err(|_| "Cannot find this openagents program to run the plugin.".to_owned())?;
        let mut command = Command::new(program);
        command.args(["--json", "plugin", "run", key, "--request", request]);
        if let Some(folder) = folder {
            command.arg("--in").arg(folder);
        }
        let output = command
            .stdin(Stdio::null())
            .stderr(Stdio::null())
            .output()
            .map_err(|error| format!("it could not start: {error}."))?;
        plugin_reply(&String::from_utf8_lossy(&output.stdout))
    }

    #[cfg(unix)]
    fn turn_plugin(&self, id: &str, on: bool) -> Result<String, String> {
        crate::plugin_local::turn(id, on)
    }

    #[cfg(unix)]
    fn install_plugin(&self, id: &str) -> Result<String, String> {
        let program = std::env::current_exe()
            .map_err(|_| "Cannot find this openagents program to install it.".to_owned())?;
        let text = captured(
            Command::new(program).args(["--json", "plugin", "install", id]),
            PLUGINS_WAIT * 8,
        )?;
        let value: Value = serde_json::from_str(text.trim())
            .map_err(|_| "The install answered something else.".to_owned())?;
        match value["error"].as_str() {
            Some(error) => Err(error.to_owned()),
            None => Ok(value["text"].as_str().unwrap_or("Installed.").to_owned()),
        }
    }

    fn import(&self) -> Result<String, String> {
        if !self.host_answers() {
            return Err("Importing sessions needs this computer's host, which keeps the threads every app shows. Open the OpenAgents app, or run `openagents host serve --control`.".into());
        }
        match self.call(Op::ChatImport {})? {
            Reply::ChatImported {
                imported,
                present,
                skipped,
            } => Ok(imported_words(imported, present, skipped)),
            _ => Err(other_answer()),
        }
    }

    fn settings(&self) -> Settings {
        coder_settings(&coder::task::settings::path())
    }

    fn change(&self, key: &str, on: bool) -> Result<Settings, String> {
        let path = coder::task::settings::path();
        change_setting(&path, key, on)?;
        Ok(coder_settings(&path))
    }

    fn set_secret(&self, key: &str, value: &str) -> Result<Settings, String> {
        let path = coder::task::settings::path();
        keep_provider_key(&path, key, value)?;
        Ok(coder_settings(&path))
    }

    #[cfg(unix)]
    fn background(&self) -> Result<Vec<openagents_terminal::BackgroundRow>, String> {
        crate::background::rows()
    }

    #[cfg(unix)]
    fn background_act(
        &self,
        id: &str,
        act: openagents_terminal::BackgroundAct,
    ) -> Result<Vec<String>, String> {
        crate::background::act(id, act)
    }

    #[cfg(unix)]
    fn background_notice(&self) -> Option<(u64, String)> {
        crate::background::notice()
    }

    #[cfg(unix)]
    fn watchers(&self) -> Vec<String> {
        crate::background::watchers()
    }
}

/// What an import did, in words.
fn imported_words(imported: u32, present: u32, skipped: u32) -> String {
    let sessions = |count: u32| format!("{count} session{}", if count == 1 { "" } else { "s" });
    let mut words = if imported == 0 {
        "No new Claude Code or Codex sessions to copy.".to_owned()
    } else {
        format!(
            "Copied {} from Claude Code and Codex in as threads; Ctrl+T lists them.",
            sessions(imported)
        )
    };
    if present > 0 {
        let were = if present == 1 { "was" } else { "were" };
        words.push_str(&format!(" {} {were} copied before.", sessions(present)));
    }
    if skipped > 0 {
        words.push_str(&format!(" {} held nothing to copy.", sessions(skipped)));
    }
    words
}

/// The key of the choice "Start Coder at once".
const START_KEY: &str = "start";
/// The prefix of each coding agent's choice key; the provider's word follows.
const AGENT_KEY: &str = "agent:";

/// What the settings list shows from `file`: when Coder starts, then each
/// coding agent, the ones on first in the order Coder tries them, then the
/// ones turned off, as the desktop app's settings page shows them. Agents
/// are opt-out (#10184): every agent is on unless the person turned it off.
fn coder_settings(file: &std::path::Path) -> Settings {
    coder_settings_with(file, &crate::provider_key::stored())
}

/// [`coder_settings`] with the person's provider keys as `keys`.
fn coder_settings_with(file: &std::path::Path, keys: &model_access::Keys) -> Settings {
    use coder::task::settings::{self, Start};
    let loaded = match settings::Settings::load(file) {
        Ok(loaded) => loaded,
        Err(why) => {
            return Settings {
                path: file.to_path_buf(),
                problem: Some(format!(
                    "The settings file cannot be read, so nothing changes here: {why}"
                )),
                choices: Vec::new(),
                status: None,
            };
        }
    };
    let mut choices = vec![openagents_terminal::Choice {
        key: START_KEY.into(),
        label: "Start Coder at once".into(),
        on: loaded.coder.start == Start::AtOnce,
        blocked: None,
        secret: false,
    }];
    choices.extend(loaded.coder.listing().into_iter().map(|(provider, on)| {
        openagents_terminal::Choice {
            key: format!("{AGENT_KEY}{}", provider.as_str()),
            label: settings::provider_name(provider).to_owned(),
            on,
            blocked: None,
            secret: false,
        }
    }));
    // BYOK (#10176): the mode, then one masked field per provider.
    choices.push(openagents_terminal::Choice {
        key: PAYER_KEY.into(),
        label: "Use my keys for everything".into(),
        on: loaded.models.payer == model_access::Mode::Mine,
        blocked: (!keys.chat_capable()).then(|| {
            if keys.get(model_access::Provider::TypeSafe).is_some() {
                model_access::TYPESAFE_ONLY.to_owned()
            } else {
                "Add an OpenRouter or Vercel AI Gateway key first.".to_owned()
            }
        }),
        secret: false,
    });
    for provider in model_access::PROVIDERS {
        let key = keys.get(provider);
        choices.push(openagents_terminal::Choice {
            key: format!("{PROVIDER_KEY}{}", provider.word()),
            label: match key {
                Some(key) => format!("{} …{}", provider.name(), key.last_four()),
                None => provider.name().to_owned(),
            },
            on: key.is_some(),
            blocked: None,
            secret: true,
        });
    }
    Settings {
        path: file.to_path_buf(),
        problem: None,
        choices,
        status: Some(model_access::status_line(loaded.models.payer, keys, None)),
    }
}

/// The key of the choice "Use my keys for everything" (`models.payer`).
const PAYER_KEY: &str = "models.payer";
/// The prefix of each provider key's choice; the provider's word follows.
const PROVIDER_KEY: &str = "provider-key:";

/// Keep a pasted provider key, after the provider's own check: a refused
/// key is not kept.
fn keep_provider_key(file: &std::path::Path, key: &str, value: &str) -> Result<(), String> {
    let provider = key
        .strip_prefix(PROVIDER_KEY)
        .map(model_access::Provider::parse)
        .transpose()?
        .ok_or_else(|| format!("`{key}` is not a key here"))?;
    let _ = file;
    crate::provider_key::keep(provider, &model_access::ApiKey::new(value))
}

/// Turn the choice `key` on or off in `file`, through Coder's own loader,
/// which records only a turn-off and refuses one that leaves no agent.
fn change_setting(file: &std::path::Path, key: &str, on: bool) -> Result<(), String> {
    use coder::task::settings::{Settings as File, Start, agent};
    let mut settings = File::load(file)?;
    if key == PAYER_KEY {
        let mode = if on {
            model_access::Mode::Mine
        } else {
            model_access::Mode::Ours
        };
        settings.set_payer(mode, &crate::provider_key::stored())?;
        settings.save(file)?;
        model_access::install(coder::task::settings::access());
        return Ok(());
    }
    if let Some(word) = key.strip_prefix(PROVIDER_KEY) {
        let provider = model_access::Provider::parse(word)?;
        if on {
            return Err(format!("Paste your {} key to add it.", provider.name()));
        }
        crate::provider_key::forget(provider)?;
        let mut settings = File::load(file)?;
        if settings.settle_payer(&crate::provider_key::stored()) {
            settings.save(file)?;
        }
        model_access::install(coder::task::settings::access());
        return Ok(());
    }
    if key == START_KEY {
        settings.coder.start = if on { Start::AtOnce } else { Start::AskFirst };
    } else {
        let provider = key
            .strip_prefix(AGENT_KEY)
            .and_then(|name| agent(name).ok())
            .ok_or_else(|| format!("`{key}` is not a setting here"))?;
        settings
            .allow(provider, on)
            .map_err(|why| format!("That cannot change: {why}"))?;
    }
    settings.save(file)
}

const ALREADY_RUNS: &str = "This computer's host already runs; chats sync with your phone.";

/// What `sync` found on this computer.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Facts {
    /// A host answers on the control socket.
    host_answers: bool,
    /// This platform has a service manager the host installs into.
    service_manager: bool,
    /// A host bundle is staged and selected under `~/.openagents/host-bundle`.
    bundle_selected: bool,
    /// The `coder-service` launcher sits beside this program.
    launcher_present: bool,
    /// The host's public key, when this computer has a host identity.
    host_key: Option<String>,
}

/// What `sync` does with them.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Sync {
    AlreadyRuns,
    Install {
        host_key: String,
    },
    /// Something is missing: the sentence that names it and what to do.
    Missing(String),
}

fn decide(facts: &Facts) -> Sync {
    const OPEN_APP: &str = "Open the OpenAgents app, which runs the host.";
    if facts.host_answers {
        return Sync::AlreadyRuns;
    }
    if !facts.service_manager {
        return Sync::Missing(
            "This computer has no service manager to run the host. Run `openagents host serve --control` in another terminal."
                .into(),
        );
    }
    if !facts.bundle_selected {
        return Sync::Missing(format!(
            "No host bundle is staged on this computer. {OPEN_APP}"
        ));
    }
    if !facts.launcher_present {
        return Sync::Missing(format!(
            "The coder-service launcher is not beside this openagents program. {OPEN_APP}"
        ));
    }
    match &facts.host_key {
        Some(host_key) => Sync::Install {
            host_key: host_key.clone(),
        },
        None => Sync::Missing(format!(
            "This computer has no host identity yet. {OPEN_APP}"
        )),
    }
}

/// `openagents service install --host-key KEY`, as its own process.
fn install(host_key: &str) -> Result<String, String> {
    let program = std::env::current_exe()
        .map_err(|_| "Cannot find this openagents program to install the host.".to_owned())?;
    let output = Command::new(program)
        .args(["service", "install", "--host-key", host_key])
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("The host service did not install: {error}."))?;
    if output.status.success() {
        Ok("Installed this computer's host as a user service; chats sync with your phone once it starts.".into())
    } else {
        let said = String::from_utf8_lossy(&output.stderr);
        let why = said
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .map(|line| line.trim_start_matches("openagents service: "))
            .unwrap_or("it gave no reason");
        Err(format!("The host service did not install: {why}"))
    }
}

/// A command's stdout, waiting at most `wait`.
fn captured(command: &mut Command, wait: Duration) -> Result<String, String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{error}."))?;
    let mut stdout = child.stdout.take().ok_or("no output.")?;
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        let _ = stdout.read_to_string(&mut text);
        text
    });
    let started = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if started.elapsed() < wait => {
                std::thread::sleep(Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the relay did not answer in time.".into());
            }
        }
    }
    reader.join().map_err(|_| "no output.".to_owned())
}

/// The plugins installed under `extensions`
/// (`<key>/<slug>/<version>/package.json`), the newest version of each,
/// keyed by their folder, which `openagents plugin run` takes.
fn installed_plugins(extensions: &std::path::Path) -> Vec<Plugin> {
    let sorted = |dir: &std::path::Path| {
        let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
            .map(|read| {
                read.filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .collect()
            })
            .unwrap_or_default();
        paths.sort();
        paths
    };
    let mut rows = Vec::new();
    for key in sorted(extensions) {
        for slug in sorted(&key) {
            let Some(version) = sorted(&slug)
                .into_iter()
                .filter(|version| version.join("package.json").is_file())
                .next_back()
            else {
                continue;
            };
            let Ok(package) = coder::package::Package::load(&version.join("package.json")) else {
                continue;
            };
            rows.push(Plugin {
                name: if package.name.trim().is_empty() {
                    package.slug.clone()
                } else {
                    package.name.clone()
                },
                about: package.summary.clone(),
                key: Some(version.display().to_string()),
                on: None,
                id: None,
            });
        }
    }
    rows
}

/// The reply in what `openagents --json plugin run` printed, or why it
/// did not run or finish.
fn plugin_reply(text: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(text.trim()).map_err(|_| "it answered something else.".to_owned())?;
    if let Some(error) = value["error"].as_str() {
        return Err(error.to_owned());
    }
    let reply = value["reply"].as_str().unwrap_or_default().to_owned();
    if value["finished"].as_bool() == Some(true) {
        Ok(reply)
    } else {
        Err(value["stopped"]
            .as_str()
            .map_or_else(|| "it did not finish.".to_owned(), str::to_owned))
    }
}

/// The `(name, what it does)` rows of `openagents --json plugin list`.
fn plugin_rows(text: &str) -> Result<Vec<Plugin>, String> {
    let value: Value = serde_json::from_str(text.trim()).map_err(|_| {
        "The plugin catalog could not be read: it answered something else.".to_owned()
    })?;
    if let Some(error) = value["error"].as_str() {
        return Err(format!("The plugin catalog could not be read: {error}"));
    }
    let items = value["items"].as_array().ok_or_else(|| {
        "The plugin catalog could not be read: it answered something else.".to_owned()
    })?;
    Ok(items
        .iter()
        .filter_map(|item| {
            let id = item["id"].as_str().filter(|id| !id.is_empty())?;
            let name = [&item["title"], &item["slug"]]
                .into_iter()
                .filter_map(Value::as_str)
                .find(|name| !name.is_empty())
                .unwrap_or(id);
            Some(Plugin {
                name: name.to_owned(),
                about: item["description"].as_str().unwrap_or_default().to_owned(),
                key: None,
                on: None,
                id: Some(id.to_owned()),
            })
        })
        .collect())
}

/// Whether this computer has a service manager a host can be installed
/// under (launchd or systemd). Windows has none `coder_service` drives.
#[cfg(unix)]
fn service_manager_here() -> bool {
    coder_service::service::Platform::current().is_some()
}

#[cfg(not(unix))]
fn service_manager_here() -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ready() -> Facts {
        Facts {
            host_answers: false,
            service_manager: true,
            bundle_selected: true,
            launcher_present: true,
            host_key: Some("ab".repeat(32)),
        }
    }

    #[test]
    fn sync_leaves_a_running_host_alone() {
        let facts = Facts {
            host_answers: true,
            ..Facts::default()
        };
        assert_eq!(decide(&facts), Sync::AlreadyRuns);
    }

    #[test]
    fn sync_installs_when_everything_is_here() {
        assert_eq!(
            decide(&ready()),
            Sync::Install {
                host_key: "ab".repeat(32)
            }
        );
    }

    #[test]
    fn sync_names_what_is_missing() {
        let missing = |facts: Facts| match decide(&facts) {
            Sync::Missing(why) => why,
            other => panic!("expected missing, got {other:?}"),
        };
        let why = missing(Facts {
            service_manager: false,
            ..ready()
        });
        assert!(why.contains("openagents host serve --control"), "{why}");
        let why = missing(Facts {
            bundle_selected: false,
            ..ready()
        });
        assert!(why.contains("host bundle") && why.contains("OpenAgents app"));
        let why = missing(Facts {
            launcher_present: false,
            ..ready()
        });
        assert!(why.contains("coder-service") && why.contains("OpenAgents app"));
        let why = missing(Facts {
            host_key: None,
            ..ready()
        });
        assert!(why.contains("host identity") && why.contains("OpenAgents app"));
    }

    #[test]
    fn plugin_rows_keep_published_plugins_with_their_ids() {
        let text = r#"{"relay":"wss://r","count":3,"items":[
            {"id":"aa:lint-fixer","slug":"lint-fixer","title":"Lint fixer","description":"Fixes lint"},
            {"id":"bb:slug-only","slug":"slug-only","title":"","description":"Why"},
            {"slug":"no-id","title":"Bad"}
        ]}"#;
        let row = |name: &str, about: &str, id: &str| Plugin {
            name: name.into(),
            about: about.into(),
            key: None,
            on: None,
            id: Some(id.into()),
        };
        assert_eq!(
            plugin_rows(text).unwrap(),
            vec![
                row("Lint fixer", "Fixes lint", "aa:lint-fixer"),
                row("slug-only", "Why", "bb:slug-only"),
            ]
        );
        assert!(
            plugin_rows(r#"{"error":"no relay"}"#)
                .unwrap_err()
                .contains("no relay")
        );
        assert!(plugin_rows("not json").is_err());
    }

    /// The list turns when Coder starts and each agent on or off through
    /// Coder's own loader, which keeps at least one agent.
    #[test]
    fn settings_change_through_the_loader() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let shown = coder_settings_with(&file, &model_access::Keys::none());
        let keys: Vec<(&str, bool)> = shown
            .choices
            .iter()
            .take(6)
            .map(|choice| (choice.key.as_str(), choice.on))
            .collect();
        // BYOK rows follow: the mode, off and blocked with no key, then a
        // masked field per provider.
        let byok: Vec<(&str, bool, bool)> = shown.choices[6..]
            .iter()
            .map(|choice| (choice.key.as_str(), choice.on, choice.secret))
            .collect();
        assert_eq!(
            byok,
            [
                ("models.payer", false, false),
                ("provider-key:openrouter", false, true),
                ("provider-key:vercel", false, true),
                ("provider-key:typesafe", false, true)
            ]
        );
        assert!(shown.choices[6].blocked.is_some());
        assert_eq!(shown.status.as_deref(), Some("Running on OpenAgents."));
        // Opt-out (#10184): every agent is on with no file, Devin and
        // OpenCode too, and none is blocked.
        assert_eq!(
            keys,
            [
                ("start", true),
                ("agent:codex", true),
                ("agent:claude", true),
                ("agent:grok", true),
                ("agent:devin", true),
                ("agent:opencode", true)
            ]
        );
        assert!(
            shown.choices[..6]
                .iter()
                .all(|choice| choice.blocked.is_none())
        );
        change_setting(&file, "start", false).unwrap();
        for agent in ["claude", "grok", "devin", "opencode"] {
            change_setting(&file, &format!("agent:{agent}"), false).unwrap();
        }
        let shown = coder_settings_with(&file, &model_access::Keys::none());
        assert!(!shown.choices[0].on);
        let on: Vec<&str> = shown
            .choices
            .iter()
            .filter(|choice| choice.on)
            .map(|choice| choice.key.as_str())
            .collect();
        assert_eq!(on, ["agent:codex"]);
        // Only the turn-offs are saved.
        let saved = coder::task::settings::Settings::load(&file).unwrap();
        assert!(saved.coder.providers.is_empty());
        assert_eq!(saved.coder.disabled.len(), 4);
        // The last agent stays on.
        assert!(change_setting(&file, "agent:codex", false).is_err());
        assert!(change_setting(&file, "agent:nobody", true).is_err());
        let loaded = coder::task::settings::Settings::load(&file).unwrap();
        assert_eq!(loaded.coder.start, coder::task::settings::Start::AskFirst);
    }

    #[test]
    fn installed_plugins_are_found_and_their_reply_read() {
        let dir = tempfile::tempdir().unwrap();
        let older = dir.path().join("ab/explain-error/1.0.0");
        let newer = dir.path().join("ab/explain-error/1.1.0");
        for version in [&older, &newer] {
            std::fs::create_dir_all(version).unwrap();
        }
        std::fs::write(newer.join("package.json"), "not a package").unwrap();
        assert!(installed_plugins(dir.path()).is_empty());
        std::fs::write(
            newer.join("package.json"),
            r#"{"v":1,"slug":"explain-error","name":"Explain this error","summary":"Says why",
                "program":{"name":"explain-error","digest":"c5d0ebed0178ee44d1d0db580583c397789c6c83ba409baf6163636234fab877"}}"#,
        )
        .unwrap();
        assert_eq!(
            installed_plugins(dir.path()),
            vec![Plugin {
                name: "Explain this error".into(),
                about: "Says why".into(),
                key: Some(newer.display().to_string()),
                on: None,
                id: None,
            }]
        );
        assert!(installed_plugins(&dir.path().join("missing")).is_empty());
        assert_eq!(
            plugin_reply(r#"{"finished":true,"reply":"It is a typo."}"#).unwrap(),
            "It is a typo."
        );
        assert_eq!(
            plugin_reply(r#"{"finished":false,"stopped":"needs writes"}"#).unwrap_err(),
            "needs writes"
        );
        assert_eq!(
            plugin_reply(r#"{"error":"no package.json"}"#).unwrap_err(),
            "no package.json"
        );
        assert!(plugin_reply("").is_err());
    }

    #[test]
    fn an_import_says_what_it_copied() {
        assert_eq!(
            imported_words(2, 0, 0),
            "Copied 2 sessions from Claude Code and Codex in as threads; Ctrl+T lists them."
        );
        assert_eq!(
            imported_words(0, 1, 3),
            "No new Claude Code or Codex sessions to copy. 1 session was copied before. 3 sessions held nothing to copy."
        );
    }

    #[test]
    fn closing_says_how_to_resume() {
        let id = "0123456789abcdef0123456789abcdef";
        assert_eq!(
            closing(flag(Kind::Scratch), id, false),
            vec![
                format!("thread {id}"),
                format!("Resume with: openagents terminal --thread {id} --scratch"),
            ]
        );
        let lines = closing(flag(Kind::Host), id, true);
        assert_eq!(
            lines[1],
            format!("Resume with: openagents terminal --thread {id}")
        );
        assert!(lines[2].contains("Coder keeps working"));
    }

    #[test]
    fn opening_notices_use_chat_words() {
        assert_eq!(
            notice(Event::Migrated { moved: 2 }).as_deref(),
            Some("moved 2 threads kept without a host into this computer's host")
        );
    }
}
