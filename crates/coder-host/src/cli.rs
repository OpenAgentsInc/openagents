//! `coder host`: set up, enroll devices for, and run the resident host.
//!
//! The `coder` binary calls [`run`] with its task owner. Commands never take
//! a secret key as an argument and never print one; an invitation is printed
//! because the operator shows it to the device being enrolled.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use coder_access::host::Host;
use coder_access::{RelayPolicy, Rights};

use crate::config::{Config, Ready, WebsocketTls};
use crate::settings::{ServeSettings, TailnetSetting, TlsSetting, parse_advertise};
use crate::tasks::Tasks;
use crate::{Error, Result, generation};

/// Exit code for a usage error.
pub const EXIT_USAGE: u8 = 2;
/// Exit code for a refused or failed command.
pub const EXIT_FAILED: u8 = 1;

pub const USAGE: &str = "usage: coder host COMMAND [OPTIONS]
  init --owner KEY --relay URL [--relay URL]... [--workspace LABEL=PATH]...
       [--listen-websocket ADDR] [--allow-nonloopback] [--advertise CLASS=HOST:PORT|URL]...
       [--websocket-tls-cert FILE --websocket-tls-key FILE --websocket-name NAME]
       [--tailnet-admission RIGHTS [--no-tailnet-chats]]
  public-key
  invite [--relay URL] [--rights LIST] [--grant-secs N]
  request [--relay URL] [--rights LIST]
  list [--json]
  revoke --device KEY
  spend request|list|show ...   ask the owner's phone to pay (`coder host spend help`)
  adopt [--keys DIR]  move a host set up the old way under the desktop app
  adopt detect        report whether there is such a host to move, changing nothing
  serve [--owner KEY] [--relay URL]... [--workspace LABEL=PATH]... [--listen ADDR]
        [--listen-websocket ADDR] [--allow-nonloopback]
        [--websocket-tls-cert FILE --websocket-tls-key FILE --websocket-name NAME]
        [--advertise lan|tailnet|public=HOST:PORT|URL]...
        [--tailnet-admission RIGHTS [--no-tailnet-chats]]
        [--generation N] [--runtime FILE | --no-runtime] [--tasks DIR] [--loopback]
        [--no-telemetry]
        [--iroh [--iroh-relay URL | --no-iroh-relay] [--iroh-bind ADDR]...]
        [--control | --control-socket PATH] [--keychain | --keys DIR] [--label NAME]
The desktop app runs `serve --keychain --iroh --control`: the owner, host,
and iroh keys live in the keychain, the host establishes its own owner on
first start, and `openagents connect` and the app reach it through the
same-user control socket.
Every command also takes --state DIR (the access store, default
~/.openagents/coder-access), --root DIR (default ~/.openagents/host), and
--loopback-test (allow ws:// to a numeric loopback relay, for fixtures only).
LIST is standard, admin, all, or comma-separated rights.";

const DEFAULT_GRANT_SECS: u64 = 7 * 24 * 60 * 60;
/// Opens the task owner for a task store directory and the workspace labels
/// the host admits.
pub type OpenTasks =
    dyn FnOnce(&Path, &BTreeMap<String, PathBuf>) -> std::result::Result<Arc<dyn Tasks>, String>;

/// Run `coder host ARGS`. Returns the process exit code.
pub async fn run(args: &[String], open_tasks: Box<OpenTasks>) -> u8 {
    let Some((command, rest)) = args.split_first() else {
        eprintln!("{USAGE}");
        return EXIT_USAGE;
    };
    if command == "spend" {
        let state = match home(".openagents/coder-access") {
            Ok(state) => state,
            Err(error) => {
                eprintln!("coder host: {error}");
                return EXIT_FAILED;
            }
        };
        return crate::spend::cli::run(rest, &state);
    }
    if command == "adopt" {
        return adopt(rest);
    }
    let mut options = match Options::parse(rest) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("coder host: {message}\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    let common = match Common::take(&mut options) {
        Ok(common) => common,
        Err(error) => {
            eprintln!("coder host: {error}\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    let result = match command.as_str() {
        "init" => init(&common, &mut options),
        "public-key" => public_key(&common, &mut options),
        "invite" => invite(&common, &mut options),
        "request" => request(&common, &mut options).await,
        "list" => list(&common, &mut options),
        "revoke" => revoke(&common, &mut options),
        "serve" => serve(&common, &mut options, open_tasks).await,
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            return 0;
        }
        _ => {
            eprintln!("coder host: unknown command `{command}`\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    match result {
        Ok(()) => 0,
        Err(Error::Config(message)) if message.starts_with("usage:") => {
            eprintln!("coder host: {}\n\n{USAGE}", &message[6..]);
            EXIT_USAGE
        }
        Err(error) => {
            eprintln!("coder host: {error}");
            EXIT_FAILED
        }
    }
}

fn usage(message: &str) -> Error {
    Error::Config(format!("usage:{message}"))
}

/// The options every command takes.
struct Common {
    policy: RelayPolicy,
    state: PathBuf,
    root: PathBuf,
}

impl Common {
    fn take(options: &mut Options) -> Result<Self> {
        Ok(Self {
            policy: options.policy(),
            state: options.state()?,
            root: options.root()?,
        })
    }
}

/// Parsed options: repeatable values and flags.
struct Options {
    values: BTreeMap<String, Vec<String>>,
    flags: Vec<String>,
}

const FLAGS: [&str; 11] = [
    "--json",
    "--loopback",
    "--loopback-test",
    "--allow-nonloopback",
    "--no-runtime",
    "--no-telemetry",
    "--help",
    "--iroh",
    "--no-iroh-relay",
    "--control",
    "--keychain",
];

/// The Nostr relay a host serves when none is recorded or given and it
/// serves iroh: the relay the phone app uses by default, so a connect
/// code's invitation names a relay the phone already signs for.
pub const DEFAULT_RELAY: &str = "wss://relay.openagents.com/";

impl Options {
    fn parse(args: &[String]) -> std::result::Result<Self, String> {
        let mut options = Self {
            values: BTreeMap::new(),
            flags: Vec::new(),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            if FLAGS.contains(&arg.as_str()) {
                if options.flags.contains(arg) {
                    return Err(format!("{arg} is given twice"));
                }
                options.flags.push(arg.clone());
            } else if arg.starts_with("--") {
                let value = args
                    .next()
                    .filter(|v| !v.starts_with("--"))
                    .ok_or_else(|| format!("{arg} needs a value"))?;
                options
                    .values
                    .entry(arg.clone())
                    .or_default()
                    .push(value.clone());
            } else {
                return Err(format!("unexpected argument `{arg}`"));
            }
        }
        Ok(options)
    }

    fn flag(&mut self, name: &str) -> bool {
        let position = self.flags.iter().position(|f| f == name);
        position.map(|i| self.flags.remove(i)).is_some()
    }

    fn all(&mut self, name: &str) -> Vec<String> {
        self.values.remove(name).unwrap_or_default()
    }

    fn one(&mut self, name: &str) -> Result<Option<String>> {
        let mut values = self.all(name);
        match values.len() {
            0 => Ok(None),
            1 => Ok(values.pop()),
            _ => Err(usage(&format!(" {name} is given twice"))),
        }
    }

    fn required(&mut self, name: &str) -> Result<String> {
        self.one(name)?
            .ok_or_else(|| usage(&format!(" {name} is required")))
    }

    fn finish(&self) -> Result<()> {
        match (self.values.keys().next(), self.flags.first()) {
            (None, None) => Ok(()),
            (Some(name), _) | (None, Some(name)) => {
                Err(usage(&format!(" {name} does not apply to this command")))
            }
        }
    }

    fn policy(&mut self) -> RelayPolicy {
        if self.flag("--loopback-test") {
            RelayPolicy::LoopbackTest
        } else {
            RelayPolicy::Production
        }
    }

    fn state(&mut self) -> Result<PathBuf> {
        match self.one("--state")? {
            Some(path) => Ok(PathBuf::from(path)),
            None => home(".openagents/coder-access"),
        }
    }

    fn root(&mut self) -> Result<PathBuf> {
        match self.one("--root")? {
            Some(path) => Ok(PathBuf::from(path)),
            None => home(".openagents/host"),
        }
    }

    fn workspaces(&mut self) -> Result<BTreeMap<String, PathBuf>> {
        let mut workspaces = BTreeMap::new();
        for entry in self.all("--workspace") {
            let (label, path) = entry
                .split_once('=')
                .ok_or_else(|| usage(" --workspace takes LABEL=PATH"))?;
            let path = std::fs::canonicalize(path)
                .map_err(|_| Error::Config("a workspace root does not exist".into()))?;
            workspaces.insert(label.to_owned(), path);
        }
        Ok(workspaces)
    }
}

/// Check each recorded workspace root before the host serves. A root under
/// the host's own directory is created; any other missing or unreadable
/// root is reported so a later `terminal.open` refusal has a cause in the
/// log. Returns one line per problem or repair.
fn check_workspaces(workspaces: &BTreeMap<String, PathBuf>, root: &Path) -> Vec<String> {
    let mut lines = Vec::new();
    for (label, path) in workspaces {
        if path.is_dir() {
            continue;
        }
        if path.starts_with(root) {
            match std::fs::create_dir_all(path) {
                Ok(()) => lines.push(format!(
                    "created workspace `{label}` at {}",
                    path.display()
                )),
                Err(error) => lines.push(format!(
                    "workspace `{label}` at {} cannot be created ({error}); terminal.open is refused until it exists",
                    path.display()
                )),
            }
        } else {
            lines.push(format!(
                "workspace `{label}` root {} is missing; terminal.open is refused until it exists",
                path.display()
            ));
        }
    }
    lines
}

fn home(relative: &str) -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| Error::Config("HOME is not set; pass the directory explicitly".into()))?;
    // A unit test that reached the real home would write into the person's
    // host state; tests pass `--state`, `--root`, and `--tasks`, or set a
    // temporary HOME.
    #[cfg(all(test, unix))]
    assert!(
        home.canonicalize().ok() != coder_service::adopt::test_home::real_home(),
        "a test reached the real home; give it a temporary one"
    );
    Ok(home.join(relative))
}

/// Establish the owner locally and record the relays, workspaces, and
/// listeners serve uses by default.
fn init(common: &Common, options: &mut Options) -> Result<()> {
    let (policy, state, root) = (common.policy, &common.state, &common.root);
    let owner = public_key_text(&options.required("--owner")?)?;
    let relays = options.all("--relay");
    let workspaces = options.workspaces()?;
    let mut settings = ServeSettings::new(relays, workspaces);
    settings.listen_websocket = listen_websocket(options)?;
    settings.websocket_tls = websocket_tls(options)?.map(|tls| TlsSetting {
        cert: tls.cert,
        key: tls.key,
        name: tls.name,
    });
    settings.allow_nonloopback = options.flag("--allow-nonloopback");
    settings.tailnet_admission = tailnet_admission(options)?;
    settings.advertise = options
        .all("--advertise")
        .iter()
        .map(|entry| parse_advertise(entry))
        .collect::<Result<_>>()?;
    options.finish()?;
    if settings.relays.is_empty() {
        return Err(usage(" init needs at least one --relay"));
    }
    for relay in &settings.relays {
        policy
            .validate(relay)
            .map_err(|_| Error::Config("a relay is not allowed by the relay policy".into()))?;
    }
    // The same checks serve makes, so a recorded setting that could never
    // serve refuses now.
    let mut check = Config::new(state.clone(), settings.relays.clone(), 1);
    check.policy = policy;
    check.listen_websocket = settings.listen_websocket;
    check.websocket_tls = settings.tls();
    check.allow_nonloopback = settings.allow_nonloopback;
    check.advertise = settings.advertised()?;
    check.workspaces = settings.workspaces.clone();
    check.validate()?;
    coder_access::host::ensure_parent(state)?;
    let host = Host::new(state, policy).init(&owner)?;
    settings.save(root)?;
    println!("{host}");
    Ok(())
}

/// `--tailnet-admission RIGHTS [--no-tailnet-chats]`: the rights are chosen
/// explicitly, as for any invitation.
fn tailnet_admission(options: &mut Options) -> Result<Option<TailnetSetting>> {
    let rights = options.one("--tailnet-admission")?;
    let no_chats = options.flag("--no-tailnet-chats");
    match rights {
        Some(rights) => {
            Rights::parse_list(&rights)?;
            Ok(Some(TailnetSetting { rights, no_chats }))
        }
        None if no_chats => Err(usage(" --no-tailnet-chats goes with --tailnet-admission")),
        None => Ok(None),
    }
}

fn listen_websocket(options: &mut Options) -> Result<Option<SocketAddr>> {
    options
        .one("--listen-websocket")?
        .map(|text| {
            text.parse::<SocketAddr>()
                .map_err(|_| usage(" --listen-websocket takes HOST:PORT"))
        })
        .transpose()
}

fn public_key(common: &Common, options: &mut Options) -> Result<()> {
    options.finish()?;
    println!("{}", Host::new(&common.state, common.policy).public_key()?);
    Ok(())
}

/// Print one `coder-host:` invitation line. It admits one device for five
/// minutes; show it only to the device being enrolled.
fn invite(common: &Common, options: &mut Options) -> Result<()> {
    let relay = match options.one("--relay")? {
        Some(relay) => relay,
        None => ServeSettings::load(&common.root)?
            .relays
            .into_iter()
            .next()
            .ok_or_else(|| usage(" invite needs --relay, or run init first"))?,
    };
    let rights = match options.one("--rights")? {
        Some(list) => Rights::parse_list(&list)?,
        None => Rights::standard(),
    };
    let grant_secs = match options.one("--grant-secs")? {
        Some(n) => n
            .parse::<u64>()
            .map_err(|_| usage(" --grant-secs takes a whole number"))?,
        None => DEFAULT_GRANT_SECS,
    };
    options.finish()?;
    let host = Host::new(&common.state, common.policy);
    let issued = retry_busy(|| {
        let now = coder_access::unix_time()?;
        host.invite(&relay, rights.clone(), now, now.saturating_add(grant_secs))
    })?;
    println!("{}", issued.code);
    Ok(())
}

/// Reverse enrollment for a host without a screen: publish a request, print
/// its short code, and wait while the running host answers the approval.
async fn request(common: &Common, options: &mut Options) -> Result<()> {
    let relay = match options.one("--relay")? {
        Some(relay) => relay,
        None => ServeSettings::load(&common.root)?
            .relays
            .into_iter()
            .next()
            .ok_or_else(|| usage(" request needs --relay, or run init first"))?,
    };
    let rights = match options.one("--rights")? {
        Some(list) => Rights::parse_list(&list)?,
        None => Rights::standard(),
    };
    options.finish()?;
    let requested = crate::enroll::request(&common.state, common.policy, &relay, rights).await?;
    println!("enrollment {}", requested.id);
    println!("code {}", requested.code);
    eprintln!(
        "Approve this request from the owner or a device with access_admin, typing the code. \
         `coder host serve` must be running on {relay} to answer. It expires at {}.",
        requested.expires_at
    );
    let outcome = crate::enroll::wait(
        &common.state,
        common.policy,
        &requested.id,
        Duration::from_millis(500),
    )
    .await?;
    match outcome {
        coder_access::host::EnrollmentStatus::Approved { device, grant } => {
            println!("approved device {device} grant {grant}");
            Ok(())
        }
        other => Err(Error::Config(format!(
            "the enrollment request was {}",
            crate::enroll::describe(&other)
        ))),
    }
}

fn list(common: &Common, options: &mut Options) -> Result<()> {
    let json = options.flag("--json");
    options.finish()?;
    let host = Host::new(&common.state, common.policy);
    let devices = retry_busy(|| host.devices(coder_access::unix_time()?))?;
    if json {
        let text = serde_json::to_string_pretty(&devices)
            .map_err(|_| Error::Config("the device list cannot be encoded".into()))?;
        println!("{text}");
    } else {
        for d in devices {
            println!(
                "{} {:?} {} epoch {} expires {} last-seen {}",
                d.device,
                d.state,
                d.rights.to_list(),
                d.epoch,
                d.expires_at,
                d.last_seen
                    .map_or_else(|| "none".to_owned(), |at| at.to_string())
            );
        }
    }
    Ok(())
}

/// Revoke every grant a device holds. A running host closes the device's
/// channels and ends its terminal attachments on their next check.
fn revoke(common: &Common, options: &mut Options) -> Result<()> {
    let device = public_key_text(&options.required("--device")?)?;
    options.finish()?;
    let host = Host::new(&common.state, common.policy);
    let (epoch, grants) = retry_busy(|| host.revoke(&device, coder_access::unix_time()?))?;
    println!("revoked {} grants; epoch {epoch}", grants.len());
    Ok(())
}

async fn serve(common: &Common, options: &mut Options, open_tasks: Box<OpenTasks>) -> Result<()> {
    let (policy, state, root) = (common.policy, common.state.clone(), &common.root);
    // The listener is loopback by default; this flag states it explicitly,
    // as an SSH launcher does.
    let _ = options.flag("--loopback");
    // An SSH launcher starts a host on a machine it set up in the same
    // command: `--owner` establishes the owner on first start, and is a
    // no-op for the same owner afterwards. Another owner is refused.
    if let Some(owner) = options.one("--owner")? {
        let owner = public_key_text(&owner)?;
        coder_access::host::ensure_parent(&state)?;
        Host::new(&state, policy).init(&owner)?;
    }
    let settings = ServeSettings::load(root)?;
    let mut relays = options.all("--relay");
    if relays.is_empty() {
        relays = settings.relays.clone();
    }
    let mut workspaces = options.workspaces()?;
    if workspaces.is_empty() {
        workspaces = settings.workspaces.clone();
    }
    for line in check_workspaces(&workspaces, root) {
        eprintln!("coder host: {line}");
    }
    let listen = match options
        .one("--listen")?
        .or_else(|| std::env::var("OPENAGENTS_HOST_LISTEN").ok())
    {
        Some(text) => text
            .parse::<SocketAddr>()
            .map_err(|_| usage(" --listen takes HOST:PORT"))?,
        None => SocketAddr::from(([127, 0, 0, 1], 0)),
    };
    // The WebSocket listener and its TLS go together: given here, both come
    // from the options; otherwise both come from the recorded settings.
    let (listen_websocket, websocket_tls, allow_nonloopback) = match listen_websocket(options)? {
        Some(listen) => (
            Some(listen),
            websocket_tls(options)?,
            options.flag("--allow-nonloopback"),
        ),
        None => {
            let given_tls = websocket_tls(options)?;
            let given_allow = options.flag("--allow-nonloopback");
            (
                settings.listen_websocket,
                given_tls.or_else(|| settings.tls()),
                given_allow || settings.allow_nonloopback,
            )
        }
    };
    let telemetry = !options.flag("--no-telemetry");
    let connect = connect_options(options, root)?;
    if relays.is_empty() && connect.iroh.is_some() {
        relays.push(DEFAULT_RELAY.to_owned());
    }
    let tailnet = tailnet_admission(options)?.or_else(|| settings.tailnet_admission.clone());
    let mut advertise = options
        .all("--advertise")
        .iter()
        .map(|entry| parse_advertise(entry))
        .collect::<Result<Vec<_>>>()?;
    if advertise.is_empty() {
        advertise = settings.advertise.clone();
    }
    let advertise = ServeSettings {
        advertise,
        ..ServeSettings::default()
    }
    .advertised()?;
    let generation = match options
        .one("--generation")?
        .or_else(|| std::env::var("OPENAGENTS_HOST_GENERATION").ok())
    {
        Some(text) => generation::Source::Given(
            text.parse::<u64>()
                .map_err(|_| usage(" --generation takes a whole number"))?,
        ),
        None => generation::Source::Next,
    };
    let runtime = if options.flag("--no-runtime") {
        None
    } else {
        Some(match options.one("--runtime")? {
            Some(path) => PathBuf::from(path),
            None => root.join("runtime"),
        })
    };
    let tasks_dir = match options.one("--tasks")? {
        Some(path) => PathBuf::from(path),
        None => home(".openagents/tasks")?,
    };
    options.finish()?;

    let ready = match (
        std::env::var_os("OPENAGENTS_HOST_READY_FILE"),
        std::env::var("OPENAGENTS_HOST_VERSION"),
    ) {
        (Some(file), Ok(version)) => Some(Ready {
            file: PathBuf::from(file),
            version,
        }),
        _ => None,
    };
    let tasks = open_tasks(&tasks_dir, &workspaces).map_err(Error::Config)?;
    // The last step before serving, so a refused start uses no generation.
    let generation = generation::resolve(&generation::counter_root(root), generation)?;
    let mut config = Config::new(state.clone(), relays.clone(), generation);
    config.policy = policy;
    config.listen = listen;
    config.listen_websocket = listen_websocket;
    let admission = match &tailnet {
        Some(setting) => Some(crate::tailnet::Settings {
            state: state.clone(),
            policy,
            relay: relays
                .first()
                .cloned()
                .ok_or_else(|| usage(" --tailnet-admission needs a relay"))?,
            rights: Rights::parse_list(&setting.rights)?,
            grant_secs: DEFAULT_GRANT_SECS,
            port: crate::tailnet::PORT,
            tailscale: crate::tailnet::program(),
            chats: (!setting.no_chats)
                .then(|| coder_history::Config {
                    // This host's own task directory, which --tasks can move.
                    coder: Some(tasks_dir.clone()).filter(|path| path.is_dir()),
                    ..crate::tailnet::default_sources()
                })
                .filter(|sources| sources.coder.is_some())
                .map(|sources| -> Result<_> {
                    Ok(crate::tailnet::Chats {
                        observer: home(".openagents/coder-connect")?,
                        sources,
                    })
                })
                .transpose()?,
        }),
        None => None,
    };
    config.websocket_tls = websocket_tls;
    config.allow_nonloopback = allow_nonloopback;
    config.telemetry = telemetry;
    config.advertise = advertise;
    config.workspaces = workspaces;
    config.ready = ready;
    config.runtime = runtime;
    config.keys = connect.keys;
    config.iroh = connect.iroh;
    config.control = connect.control;
    config.label = connect.label;
    // A phone that pairs with a connect code reads this host's Coder chats
    // as a tailnet-admitted one does: the same observer and sources. When
    // tailnet admission serves the observer, this only issues invitations.
    if config.iroh.is_some() && tasks_dir.is_dir() {
        config.chats = Some(crate::tailnet::Chats {
            observer: home(".openagents/coder-connect")?,
            sources: coder_history::Config {
                coder: Some(tasks_dir.clone()),
                ..crate::tailnet::default_sources()
            },
        });
        config.serve_chats = !admission
            .as_ref()
            .is_some_and(|admission| admission.chats.is_some());
    } else if let Some(chats) = admission
        .as_ref()
        .and_then(|admission| admission.chats.clone())
    {
        // Tailnet admission serves the observer; `chats.invite` issues from
        // the same store, so a device renews its chats over any link.
        config.chats = Some(chats);
    }

    raise_open_file_limit();
    let running = crate::serve::start(config, tasks).await?;
    if let Some(address) = running.iroh_addr() {
        eprintln!("coder host: iroh endpoint {}", address.id);
    }
    if let Some(path) = running.control_path() {
        eprintln!("coder host: control socket {}", path.display());
    }
    eprintln!(
        "coder host: serving {} at generation {} on {}",
        running.host_key(),
        running.generation(),
        running.local_addr()
    );
    if let (Some(address), Some(url)) = (running.websocket_addr(), running.websocket_url()) {
        eprintln!("coder host: WebSocket direct channels on {address} as {url}");
    }
    if let Some(admission) = admission {
        let chats = admission.chats.is_some();
        match crate::tailnet::start(admission).await {
            Ok(address) => eprintln!(
                "coder host: tailnet admission on {address}{}",
                if chats { " with chats" } else { "" }
            ),
            Err(error) => eprintln!("coder host: tailnet admission is off: {error}"),
        }
    }
    let restart = tokio::select! {
        () = wait_for_stop() => false,
        () = running.restart_requested() => true,
    };
    running.shutdown().await;
    if restart {
        // A local action changed what the host serves, such as its
        // projects. Start again as the same process, with the same
        // arguments, so a service manager sees no exit.
        eprintln!("coder host: starting again to serve the changed settings");
        return Err(reexec());
    }
    Ok(())
}

/// Replace this process with a fresh start of the same command. Returns
/// only on failure.
#[cfg(unix)]
fn reexec() -> Error {
    use std::os::unix::process::CommandExt;
    let Ok(program) = std::env::current_exe() else {
        return Error::Config("the host cannot start again".into());
    };
    let error = std::process::Command::new(program)
        .args(std::env::args_os().skip(1))
        .exec();
    Error::Config(format!("the host cannot start again: {error}"))
}

/// Start the same command again as a new process, with no console window,
/// and end this one: Windows cannot replace a running image. The new host
/// waits for this one's control pipe to close before it binds.
#[cfg(windows)]
fn reexec() -> Error {
    use std::os::windows::process::CommandExt;
    // Process creation flags, from `winbase.h`.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    let Ok(program) = std::env::current_exe() else {
        return Error::Config("the host cannot start again".into());
    };
    match std::process::Command::new(program)
        .args(std::env::args_os().skip(1))
        .stdin(std::process::Stdio::null())
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)
        .spawn()
    {
        Ok(_) => std::process::exit(0),
        Err(error) => Error::Config(format!("the host cannot start again: {error}")),
    }
}

/// `coder host adopt [detect] [--keys DIR]`: move a host set up the old
/// way under the desktop app, keeping its host key, owner, grants, and
/// settings, so every phone paired before keeps working. The keys go into
/// the keychain, or into `DIR` with `--keys` (a Linux desktop with no
/// Secret Service). `detect` changes nothing. Adopting from this program,
/// the one that later reads the keys, keeps the keychain from asking for
/// them. Prints one line of JSON with no secret:
/// `{"kind": "none" | "found" | "adopted" | "failed", ...}`; `none` and
/// `found` also say where the desktop app's host keeps its keys here,
/// `"keys": "keychain" | "files"`.
fn adopt(args: &[String]) -> u8 {
    let (detect_only, keys_dir) = match args {
        [] => (false, None),
        [only] if only == "detect" => (true, None),
        [flag, dir] if flag == "--keys" => (false, Some(PathBuf::from(dir))),
        _ => {
            eprintln!("coder host: adopt takes nothing, `detect`, or `--keys DIR`\n\n{USAGE}");
            return EXIT_USAGE;
        }
    };
    let (report, code) = match adopt_report(detect_only, keys_dir) {
        Ok(report) => (report, 0),
        Err(message) => (
            serde_json::json!({"kind": "failed", "message": message}),
            EXIT_FAILED,
        ),
    };
    println!("{report}");
    code
}

/// Where the desktop app's host keeps its keys on this computer: the
/// keychain, except on a Linux desktop where no Secret Service answers,
/// where they are private files as a command-line install keeps them.
fn keys_here() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        use crate::serve::keys::{KeyName, KeySource as _, SecretService};
        if SecretService.load(KeyName::HostIroh).is_err() {
            return "files";
        }
    }
    "keychain"
}

fn adopt_report(
    detect_only: bool,
    keys_dir: Option<PathBuf>,
) -> std::result::Result<serde_json::Value, String> {
    use coder_service::adopt::{self, Paths};
    let home = std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .ok_or_else(|| "HOME is not set".to_owned())?;
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut paths = Paths::under(&home);
    #[cfg(unix)]
    {
        paths.registrations = coder_service::service::Platform::current()
            .map(|platform| platform.default_registration_dir(&home));
    }
    let now = coder_access::unix_time().map_err(|error| error.to_string())?;
    let found = adopt::detect(&paths, now).map_err(|error| error.to_string())?;
    let keys = keys_here();
    let found = match found {
        Some(found) if !found.problems.is_empty() || found.pending() => found,
        // Nothing here, or a setup already adopted.
        _ => return Ok(serde_json::json!({"kind": "none", "keys": keys})),
    };
    if detect_only {
        return Ok(serde_json::json!({
            "kind": "found",
            "phones": found.kept.active_grants,
            "problems": found.problems,
            "keys": keys,
        }));
    }
    let source = match keys_dir {
        Some(dir) => {
            crate::serve::keys::Keys(Arc::new(crate::serve::keys::FileKeySource::new(dir)))
        }
        None => keychain_keys().map_err(|error| error.to_string())?,
    };
    #[cfg(unix)]
    let mut runner = coder_service::service::SystemRunner;
    #[cfg(not(unix))]
    let mut runner = ();
    let adopted = adopt::adopt(
        &paths,
        now,
        &mut crate::serve::keys::AdoptInto(source.0.as_ref()),
        &mut runner,
        // The desktop app registers its own agent once this returns.
        &mut |_| Ok(()),
    )
    .map_err(|error| error.to_string())?;
    Ok(serde_json::json!({"kind": "adopted", "phones": adopted.kept.active_grants}))
}

/// What the QR pairing path adds to `serve`.
struct Connect {
    keys: Option<crate::serve::keys::Keys>,
    iroh: Option<crate::config::Iroh>,
    control: Option<crate::config::Control>,
    label: String,
}

fn connect_options(options: &mut Options, root: &Path) -> Result<Connect> {
    use crate::serve::keys::{FileKeySource, Keys};
    let keychain = options.flag("--keychain");
    let keys_dir = options.one("--keys")?;
    let keys = match (keychain, keys_dir) {
        (true, Some(_)) => return Err(usage(" --keychain and --keys do not go together")),
        (true, None) => Some(keychain_keys()?),
        (false, Some(dir)) => Some(Keys(Arc::new(FileKeySource::new(dir)))),
        (false, None) => None,
    };
    let iroh_on = options.flag("--iroh");
    let no_relay = options.flag("--no-iroh-relay");
    let relay = options.one("--iroh-relay")?;
    let bind = options
        .all("--iroh-bind")
        .iter()
        .map(|text| {
            text.parse::<SocketAddr>()
                .map_err(|_| usage(" --iroh-bind takes HOST:PORT"))
        })
        .collect::<Result<Vec<_>>>()?;
    if !iroh_on && (no_relay || relay.is_some() || !bind.is_empty()) {
        return Err(usage(
            " --iroh-relay, --no-iroh-relay, and --iroh-bind go with --iroh",
        ));
    }
    if no_relay && relay.is_some() {
        return Err(usage(
            " --iroh-relay and --no-iroh-relay do not go together",
        ));
    }
    let iroh = iroh_on.then(|| crate::config::Iroh {
        relay: if no_relay {
            None
        } else {
            Some(relay.unwrap_or_else(|| openagents_connect::RELAY_URL.to_owned()))
        },
        bind,
    });
    let control_on = options.flag("--control");
    let control = match options.one("--control-socket")? {
        Some(path) => Some(PathBuf::from(path)),
        None if control_on => Some(crate::control::default_path().ok_or_else(|| {
            Error::Config("no default control socket here; pass --control-socket PATH".into())
        })?),
        None => None,
    }
    .map(|path| crate::config::Control {
        path,
        root: root.to_path_buf(),
        autostart: autostart_program(),
        uid: crate::control::own_uid(),
    });
    let label = match options.one("--label")? {
        Some(label) => label,
        None => computer_name(),
    };
    Ok(Connect {
        keys,
        iroh,
        control,
        label,
    })
}

#[cfg(target_os = "macos")]
fn keychain_keys() -> Result<crate::serve::keys::Keys> {
    Ok(crate::serve::keys::Keys(Arc::new(
        crate::serve::keys::Keychain::default(),
    )))
}

#[cfg(target_os = "linux")]
fn keychain_keys() -> Result<crate::serve::keys::Keys> {
    Ok(crate::serve::keys::Keys(Arc::new(
        crate::serve::keys::SecretService,
    )))
}

#[cfg(windows)]
#[allow(clippy::unnecessary_wraps)]
fn keychain_keys() -> Result<crate::serve::keys::Keys> {
    Ok(crate::serve::keys::Keys(Arc::new(
        crate::serve::keys::CredentialManager::default(),
    )))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", windows)))]
fn keychain_keys() -> Result<crate::serve::keys::Keys> {
    Err(Error::Config(
        "--keychain is macOS, Linux, and Windows only here; pass --keys DIR".into(),
    ))
}

/// The `coder` program that runs `coder host autostart`, when this host is
/// that program.
fn autostart_program() -> Option<PathBuf> {
    #[cfg(windows)]
    let named = |name: &std::ffi::OsStr| name.eq_ignore_ascii_case("coder.exe");
    #[cfg(not(windows))]
    let named = |name: &std::ffi::OsStr| name == "coder";
    std::env::current_exe()
        .ok()
        .filter(|path| path.file_name().is_some_and(named))
}

/// This computer's name for a connect code and a phone's list: the host
/// name without a `.local` suffix, at most 48 bytes.
fn computer_name() -> String {
    let Some(name) = host_name() else {
        return String::new();
    };
    let name = name.strip_suffix(".local").unwrap_or(&name);
    let mut label = String::new();
    for c in name.chars().filter(|c| !c.is_control()) {
        if label.len() + c.len_utf8() > openagents_connect::code::MAX_LABEL_BYTES {
            break;
        }
        label.push(c);
    }
    label
}

/// The system's host name.
#[cfg(unix)]
fn host_name() -> Option<String> {
    let mut buffer = [0_u8; 256];
    // SAFETY: gethostname writes at most `buffer.len()` bytes into `buffer`.
    let status = unsafe { libc::gethostname(buffer.as_mut_ptr().cast(), buffer.len()) };
    if status != 0 {
        return None;
    }
    let end = buffer.iter().position(|b| *b == 0).unwrap_or(buffer.len());
    Some(String::from_utf8_lossy(&buffer[..end]).into_owned())
}

/// The computer's DNS host name, as Settings > System > About shows it.
#[cfg(windows)]
fn host_name() -> Option<String> {
    use windows_sys::Win32::System::SystemInformation::{
        ComputerNameDnsHostname, GetComputerNameExW,
    };
    let mut buffer = [0_u16; 256];
    let mut length = u32::try_from(buffer.len()).ok()?;
    // SAFETY: `length` is the buffer's size in UTF-16 units, and the call
    // writes at most that many.
    if unsafe { GetComputerNameExW(ComputerNameDnsHostname, buffer.as_mut_ptr(), &mut length) } == 0
    {
        return None;
    }
    Some(String::from_utf16_lossy(buffer.get(..length as usize)?))
}

/// `--websocket-tls-cert FILE --websocket-tls-key FILE --websocket-name NAME`:
/// all three or none. The files are read and checked when the host starts.
fn websocket_tls(options: &mut Options) -> Result<Option<WebsocketTls>> {
    let cert = options.one("--websocket-tls-cert")?;
    let key = options.one("--websocket-tls-key")?;
    let name = options.one("--websocket-name")?;
    match (cert, key, name) {
        (None, None, None) => Ok(None),
        (Some(cert), Some(key), Some(name)) => Ok(Some(WebsocketTls {
            cert: PathBuf::from(cert),
            key: PathBuf::from(key),
            name,
        })),
        _ => Err(usage(
            " --websocket-tls-cert, --websocket-tls-key, and --websocket-name go together",
        )),
    }
}

/// The most open files the host asks for. macOS refuses a soft limit above
/// its per-process maximum, 10,240 by default.
#[cfg(unix)]
const OPEN_FILES: u64 = 10_240;

/// Raise the soft open-file limit toward the hard limit. A launchd agent
/// starts with a soft limit of 256, too few for the terminals, channels, and
/// task owners the host starts, which inherit it: a task owner's workspace
/// snapshot of a full checkout failed with `Too many open files`.
#[cfg(windows)]
fn raise_open_file_limit() {
    // Windows has no per-process open-file limit to raise.
}

#[cfg(unix)]
fn raise_open_file_limit() {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `getrlimit` and `setrlimit` read or write only `limit`.
    unsafe {
        if libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) != 0 {
            return;
        }
        let wanted = limit.rlim_max.min(OPEN_FILES as libc::rlim_t);
        if limit.rlim_cur < wanted {
            limit.rlim_cur = wanted;
            let _ = libc::setrlimit(libc::RLIMIT_NOFILE, &raw const limit);
        }
    }
}

/// Wait for Ctrl+C or Ctrl+Break, the console's close, or the session's
/// sign-out or shutdown.
#[cfg(windows)]
async fn wait_for_stop() {
    use tokio::signal::windows;
    let (Ok(mut interrupt), Ok(mut brk), Ok(mut close), Ok(mut logoff), Ok(mut shutdown)) = (
        windows::ctrl_c(),
        windows::ctrl_break(),
        windows::ctrl_close(),
        windows::ctrl_logoff(),
        windows::ctrl_shutdown(),
    ) else {
        std::future::pending::<()>().await;
        return;
    };
    tokio::select! {
        _ = interrupt.recv() => {},
        _ = brk.recv() => {},
        _ = close.recv() => {},
        _ = logoff.recv() => {},
        _ = shutdown.recv() => {},
    }
}

/// Wait for `SIGTERM` or `SIGINT`.
#[cfg(unix)]
async fn wait_for_stop() {
    use tokio::signal::unix::{SignalKind, signal};
    let (Ok(mut term), Ok(mut interrupt)) = (
        signal(SignalKind::terminate()),
        signal(SignalKind::interrupt()),
    ) else {
        std::future::pending::<()>().await;
        return;
    };
    tokio::select! {
        _ = term.recv() => {},
        _ = interrupt.recv() => {},
    }
}

fn public_key_text(text: &str) -> Result<String> {
    let hex = if text.starts_with("npub1") {
        nostr::nip19::decode_npub(text)
            .map_err(|_| usage(" the key is not a valid npub"))?
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        text.to_owned()
    };
    coder_reach::parse_pubkey(&hex).map_err(|_| usage(" the key is not a public key"))?;
    Ok(hex)
}

fn retry_busy<T>(
    mut operation: impl FnMut() -> coder_access::Result<T>,
) -> coder_access::Result<T> {
    let started = std::time::Instant::now();
    loop {
        match operation() {
            Err(error)
                if error.code == coder_access::Code::Conflict
                    && started.elapsed() < Duration::from_secs(5) =>
            {
                std::thread::sleep(Duration::from_millis(20));
            }
            other => return other,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serve_flags_take_no_value() {
        let args: Vec<String> = [
            "--loopback",
            "--no-telemetry",
            "--no-runtime",
            "--allow-nonloopback",
            "--relay",
            "ws://127.0.0.1:9/",
        ]
        .map(String::from)
        .to_vec();
        let mut options = Options::parse(&args).unwrap();
        for flag in [
            "--loopback",
            "--no-telemetry",
            "--no-runtime",
            "--allow-nonloopback",
        ] {
            assert!(options.flag(flag), "{flag}");
        }
        assert_eq!(options.all("--relay"), ["ws://127.0.0.1:9/"]);
        assert!(options.finish().is_ok());
    }

    fn no_tasks() -> Box<OpenTasks> {
        Box::new(|_: &Path, _: &BTreeMap<String, PathBuf>| {
            Ok(Arc::new(crate::NoTasks) as Arc<dyn Tasks>)
        })
    }

    #[test]
    fn missing_workspace_roots_are_named_or_created_under_the_host_root() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("host");
        let present = dir.path().join("present");
        std::fs::create_dir_all(&present).unwrap();
        let gone = dir.path().join("gone");
        let owned = root.join("tasks");
        let workspaces: BTreeMap<String, PathBuf> = [
            ("a".to_owned(), present.clone()),
            ("b".to_owned(), gone.clone()),
            ("c".to_owned(), owned.clone()),
        ]
        .into_iter()
        .collect();
        let lines = check_workspaces(&workspaces, &root);
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[0].contains("`b`") && lines[0].contains(gone.to_str().unwrap()));
        assert!(lines[0].contains("missing"), "{}", lines[0]);
        assert!(
            lines[1].starts_with("created workspace `c`"),
            "{}",
            lines[1]
        );
        assert!(owned.is_dir());
        assert!(!gone.exists());
        assert!(check_workspaces(&workspaces, &root).len() == 1);
    }

    #[tokio::test]
    async fn init_records_listeners_and_refuses_ones_that_could_never_serve() {
        let dir = tempfile::tempdir().unwrap();
        let (state, root) = (dir.path().join("access"), dir.path().join("host"));
        let owner = "ab".repeat(32);
        let args = |extra: &[&str]| -> Vec<String> {
            let mut args: Vec<String> = [
                "init",
                "--owner",
                &owner,
                "--relay",
                "wss://relay.example/",
                "--state",
                state.to_str().unwrap(),
                "--root",
                root.to_str().unwrap(),
            ]
            .map(String::from)
            .to_vec();
            args.extend(extra.iter().map(|arg| (*arg).to_owned()));
            args
        };
        // A tailnet listener without permission for a non-loopback address.
        assert_eq!(
            run(
                &args(&["--listen-websocket", "100.101.102.103:47101"]),
                no_tasks()
            )
            .await,
            EXIT_FAILED
        );
        assert!(!root.join(crate::settings::FILE).exists());
        assert_eq!(
            run(&args(&["--advertise", "loopback=127.0.0.1:1"]), no_tasks()).await,
            EXIT_USAGE
        );
        let tailnet = [
            "--listen-websocket",
            "100.101.102.103:47101",
            "--allow-nonloopback",
            "--advertise",
            "tailnet=ws://100.101.102.103:47101/",
        ];
        assert_eq!(run(&args(&tailnet), no_tasks()).await, 0);
        let settings = ServeSettings::load(&root).unwrap();
        assert_eq!(
            settings.listen_websocket,
            Some("100.101.102.103:47101".parse().unwrap())
        );
        assert!(settings.allow_nonloopback);
        assert_eq!(settings.advertise[0].class, "tailnet");
        // Idempotent for the same owner; another owner is refused.
        assert_eq!(run(&args(&tailnet), no_tasks()).await, 0);
        let mut other = args(&[]);
        other[2] = coder_reach::pubkey(&secp256k1::SecretKey::new(&mut secp256k1::rand::rng()));
        assert_eq!(run(&other, no_tasks()).await, EXIT_FAILED);
    }

    #[cfg(unix)]
    #[test]
    fn serve_raises_a_launchd_sized_open_file_limit() {
        let read = || {
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            // SAFETY: reads into `limit` only.
            assert_eq!(
                unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &raw mut limit) },
                0
            );
            limit
        };
        let mut low = read();
        low.rlim_cur = 256.min(low.rlim_max);
        // SAFETY: lowers this test process's own soft limit.
        assert_eq!(
            unsafe { libc::setrlimit(libc::RLIMIT_NOFILE, &raw const low) },
            0
        );
        raise_open_file_limit();
        let raised = read();
        assert_eq!(
            raised.rlim_cur,
            raised.rlim_max.min(OPEN_FILES as libc::rlim_t)
        );
    }

    #[test]
    fn websocket_tls_flags_go_together() {
        let parse = |args: &[&str]| {
            let args: Vec<String> = args.iter().map(|a| (*a).to_owned()).collect();
            websocket_tls(&mut Options::parse(&args).unwrap())
        };
        assert_eq!(parse(&[]).unwrap(), None);
        let tls = parse(&[
            "--websocket-tls-cert",
            "/tls/chain.pem",
            "--websocket-tls-key",
            "/tls/key.pem",
            "--websocket-name",
            "box.example.net",
        ])
        .unwrap()
        .unwrap();
        assert_eq!(tls.cert, PathBuf::from("/tls/chain.pem"));
        assert_eq!(tls.key, PathBuf::from("/tls/key.pem"));
        assert_eq!(tls.name, "box.example.net");
        for partial in [
            &["--websocket-tls-cert", "/tls/chain.pem"][..],
            &["--websocket-tls-key", "/tls/key.pem"][..],
            &["--websocket-name", "box.example.net"][..],
            &[
                "--websocket-tls-cert",
                "/tls/chain.pem",
                "--websocket-tls-key",
                "/tls/key.pem",
            ][..],
        ] {
            assert!(parse(partial).is_err(), "{partial:?}");
        }
    }
}
