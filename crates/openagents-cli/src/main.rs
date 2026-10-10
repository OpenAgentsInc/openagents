//! `openagents`: one command over every OpenAgents surface an agent needs.
//!
//! The program is a thin front. Each command group calls the crate that
//! owns the protocol — `coder-host` for enrollment, `coder-computers` for
//! reaching hosts, `verse` for shared worlds, `verse-lagrange` for the L1
//! construction zone — and never reimplements a NIP. Every group answers
//! `--help`, and `--json` before the group name switches output to one
//! JSON document per command so a program can read it.
//!
//! Exit codes: 0 success, 1 the operation was refused or failed,
//! 64 invalid usage.

use std::io::IsTerminal;
use std::path::Path;
use std::process::ExitCode;

// `connect`, `labor`, `pay`, `service`, `ssh`, `wallet`, and `x402` are Unix-only
// (see the dispatch below); Windows builds the rest.
mod agent;
mod approval_gate;
mod argv;
mod artifact;
#[cfg(unix)]
mod background;
mod boat_run;
#[cfg(unix)]
mod brainstorm_pilot;
mod browser;
mod capacity;
mod catalog;
mod chamber;
mod chat;
mod cloud;
mod computer;
#[cfg(unix)]
mod connect;
mod customer;
mod deploy;
mod discover;
mod efficiency;
mod eval;
mod eval_engine;
mod ext_defaults;
mod ext_eval;
mod ext_eval_init;
mod ext_run;
mod github_verbs;
mod gym;
mod host_observers;
mod hosts;
mod inference;
mod issue;
mod jev_judge;
mod kb;
mod key;
#[cfg(unix)]
mod labor;
mod lease;
mod lease_place;
mod mac;
mod mac_serve;
mod mcp;
mod out;
#[cfg(unix)]
mod pay;
#[cfg(unix)]
mod pay_commission;
#[cfg(unix)]
mod pay_hosted;
#[cfg(unix)]
mod pay_payout;
#[cfg(unix)]
mod pay_plugin;
#[cfg(unix)]
mod pay_reconcile;
mod playtest;
#[cfg(unix)]
mod plugin_discovery;
#[cfg(unix)]
mod plugin_local;
#[cfg(unix)]
mod plugin_new;
#[cfg(unix)]
mod plugin_registry;
#[cfg(unix)]
mod plugin_team;
#[cfg(unix)]
mod plugin_use;
#[cfg(unix)]
mod plugin_workbench;
mod pr;
mod provider_key;
#[cfg(unix)]
mod pylon_wallet;
mod quest;
mod reach;
mod relay;
mod sales;
mod sales_weekly;
mod scratch;
mod screen;
#[cfg(unix)]
mod service;
mod session;
mod settings;
mod shadow;
mod sov;
mod sov_host;
#[cfg(unix)]
mod ssh;
mod studio;
mod studio_host;
mod studio_up;
mod study;
mod terminal;
#[cfg(test)]
mod tree;
mod verse_terminal;
mod verse_town;
mod verse_town_rumor;
mod walkers;
#[cfg(unix)]
mod wallet;
#[cfg(unix)]
mod workflow_template;
#[cfg(unix)]
mod worktree;
mod world;
#[cfg(unix)]
mod x402;
#[cfg(unix)]
mod x402_native;
#[cfg(unix)]
mod x402_node;
#[cfg(unix)]
mod x402_phone;
#[cfg(unix)]
mod x402_spark;
mod zone;

pub use argv::Args;
pub use out::{EXIT_FAILURE, EXIT_USAGE, Output};

pub const USAGE: &str = "usage: openagents [--json] COMMAND [ARGS]

Pairing and computers (NIP-HOST, NIP-REACH):
  host         Run and administer the resident host on this machine.
  connect      Pair a phone with this computer by QR code, over its local host.
  pair         Show a QR code to read this computer's chats on a phone.
  computer     Enroll with hosts, list them, run commands, order and steer work.
  study        Launch and read Microcoder study runs on a host.
  reach        Owner directory, host presence, and route probes.
  session      Observe a paired computer's chats (NIP-SESS): pair, list, read, tail.

Chat:
  chat         Talk to OpenAgents, the chat router: send a message, continue a
               thread, list, read, and export threads as ATIF.
  terminal     OpenAgents Terminal: a full-screen chat with OpenAgents; bare
               `openagents` on a terminal opens it.

Coder:
  coder        Coder chat, models, plugins, ACP agents, sessions, and ATIF exports.
  task         Durable local task requests and explicit execution.
  issue        GitHub issues: create, comment, close, reopen, list, view, and
               claim, release, and pick up (the claim record every agent and
               Coder share).
  project      GitHub Project boards: list items by status, add an issue, and
               move it to a status.
  lease        Run a command under a lease on a shared resource (build slots,
               quiet, the screen, the GPU), and list holders and waiters.
  sales        Inspect and update the host-private lead/account pipeline.
  customer     Bind gateway customers, credentials, and exact approved purchases.
  scratch      Make and print this session's durable scratch directory, under
               ~/.openagents/scratch, for files that must outlive a reboot.
  browser      Run a command beside a Chrome of its own: a fresh profile and
               debugging port, removed when the command ends.
  capacity     The shared usage-limit book: which providers have capacity, check
               one before starting an agent, and record a limit an agent hit.
  artifact     The queue for single-digest artifacts such as the Everglade pack:
               submit a change, list the queue, and land it with one repin.
  settings    What Coder may use on this computer: providers, ask first, and more.
  service      Install, update, and roll back the resident host service.
  background   The host's background rules, built in and from plugins turned on here.
  worktree     Coder's task worktrees here: list them with size and what removing
               would lose, archive an ended task's, and restore an archived one.
  ssh          Start or adopt a host over SSH and tunnel to it.
  boat         Build and test this checkout's change on a Boat sandbox, not here.
  studio       Agent Studio: one command to launch it on a repository, seats, goals a
               lead plans, plan entries released as their dependencies finish,
               shared memory, and messages to seats.
  agent        The workshop agent, Alice: make her with her own key, ask her,
               answer her proposals, stop, pause, or retire her, her memory,
               journal, and standing jobs.
  shadow       What a sample of Coder runs would have cost through the raw engine
               (off unless set: coder.shadow).
  efficiency   Routed against raw delegation, from recorded runs: cost per
               checked result, time to it, and pass rate, with intervals.
  cloud        A GCE spot pool granted as one computer: up, down, status.
  deploy       Ship the website to staging, then promote that digest to
               production once the owner approves.
  pr           Read, review, and merge a GitHub pull request; merge waits for
               the owner and passing checks.
  mac          Send Mac-only steps (iOS builds, the release gate, TestFlight
               uploads, desktop captures) to a Mac linked to your account.

Verse (NIP-MV):
  verse        See who is around, listen, speak, move, gesture, drive owned
               entities, and read quests, XP, and the board.
  xp           This identity's XP and level (openagents verse xp), and
               verify-card to re-derive a trainer card from the relays.
  zone         Drive the Lagrange 1 construction zone.
  chamber      Host or play the authoritative combat chamber.
  sov          Sovereign agents under NIP-SOV: profile, spawn, status.

Gym (NIP-EVAL):
  eval         Score doors against a suite, report a store, compare its sides.
  gym          Connect to a granted Gym host, observe it, launch admitted recipes.

Labor (NIP-MKT, NIP-LAB):
  labor        Admit, negotiate, execute, deliver, and accept free labor orders.

Models (the OpenAgents API):
  inference    Run a model through the OpenAgents API: one answer, or a stream.
               Also `inference models` and `inference rates`.

Keys, relays, and money:
  key          Show or create Nostr identities.
  wallet       Your OpenAgents wallet: your balance and an address to get paid at.
  x402         Sell a command over HTTP for an exact bitcoin amount, or buy one (http:1),
               through this computer's Lightning node (`x402 node`).
  pay          Sell many priced routes from one wallet: x402 and the HTTP Payment scheme.
  kb           Search, publish, and sync knowledge entries (NIP-KB).
  relay        Query, publish to, and follow a relay.
  pylon        Share this computer's model as a NIP-PYLON pylon, or use one.

Playtesting:
  playtest     Triage inbox: read reports, draft and file issues, keep the triage log.

Plugins (NIP-EXT, NIP-EVAL):
  plugin       List published plugins, test a plugin with and without it,
               add the result to the Gym, check a result, sync Coder's
               default plugins, and install and turn on plugins here.
  ext          Another name for plugin.

Discovery (NIP-CAP, NIP-PRG), read-only:
  cap          List and describe published capability heads.
  prg          List and describe published program heads.
  discover     Show the well-known agent card and agent-skills index.

  mcp          Serve every group as an MCP tool over stdio (mcp serve).
  completions  Print a bash, zsh, or fish completion script.
  doctor       Show the identities, stores, and relays this command uses.
  version      Show the repository, commit, and tree state.

Global options: --json; --openrouter-key KEY, --vercel-key KEY, --typesafe-key KEY
run one command on your own provider key, never stored. A key in argv shows up in
shell history and `ps`: prefer `openagents settings provider-key set PROVIDER` or
OPENAGENTS_OPENROUTER_KEY, OPENAGENTS_VERCEL_KEY, and OPENAGENTS_TYPESAFE_KEY.

Run `openagents COMMAND --help` for each group's syntax.
Exit codes: 0 success, 1 refused or failed, 64 invalid usage.";

/// Windows sets no `HOME`, and this program and the crates it runs keep
/// their files under it (`~/.openagents`, the chat home, the settings): it
/// is the user's profile folder there, as Git for Windows sets it, rather
/// than the current folder.
#[cfg(windows)]
fn home_from_profile() {
    if std::env::var_os("HOME").is_none_or(|home| home.is_empty())
        && let Some(profile) = std::env::var_os("USERPROFILE")
    {
        // SAFETY: the first thing `main` does, before any other thread
        // exists, so nothing reads the environment while it changes.
        unsafe { std::env::set_var("HOME", profile) };
    }
}

fn top_level_suggestion(word: &str) -> Option<&'static str> {
    // Models belong to the decision API's caller, not this CLI.
    if word == "models" {
        return Some("settings");
    }
    if word.len() > 32 {
        return None;
    }
    USAGE
        .lines()
        .filter(|line| line.starts_with("  ") && !line.starts_with("   "))
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| name.bytes().all(|byte| byte.is_ascii_lowercase()))
        .map(|name| (name, chat::edit_distance(word, name)))
        .filter(|(_, distance)| *distance > 0 && *distance <= 2)
        .min_by_key(|(_, distance)| *distance)
        .map(|(name, _)| name)
}

fn main() -> ExitCode {
    #[cfg(windows)]
    home_from_profile();
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    // `--json` is this program's switch only before `--`; after it, the
    // words belong to the command a group runs (`lease`, `boat run`).
    // `openagents inference MODEL --json BODY`: after `inference`, `--json`
    // is that command's request body.
    let group = arguments.iter().position(|argument| argument != "--json");
    let end = match group {
        Some(index) if arguments[index] == "inference" => index,
        _ => arguments
            .iter()
            .position(|argument| argument == "--")
            .unwrap_or(arguments.len()),
    };
    let json = arguments[..end].iter().any(|argument| argument == "--json");
    let tail = arguments.split_off(end);
    arguments.retain(|argument| argument != "--json");
    arguments.extend(tail);
    // `--openrouter-key`, `--vercel-key`, `--typesafe-key`, and the
    // `OPENAGENTS_*_KEY` variables: the person's own keys for this one
    // command, never stored (BYOK). An ambient `OPENROUTER_API_KEY` never
    // counts.
    match model_access::Keys::once(&|name| std::env::var(name).ok(), &mut arguments) {
        Ok(keys) => {
            model_access::remember_once(keys);
            // Every model call this process makes asks this access who pays.
            model_access::install(coder::task::settings::access());
        }
        Err(message) => {
            eprintln!("openagents: {message}");
            return ExitCode::from(EXIT_USAGE);
        }
    }
    // `openagents help GROUP [COMMAND]` is `openagents GROUP [COMMAND] --help`.
    if arguments.len() > 1 && arguments[0] == "help" {
        arguments.remove(0);
        arguments.push("--help".into());
    }
    argv::normalize_help(&mut arguments);
    let output = Output::new(json);
    let Some((command, rest)) = arguments.split_first() else {
        // On a terminal, bare `openagents` is OpenAgents Terminal.
        if !json && std::io::stdin().is_terminal() && std::io::stdout().is_terminal() {
            return ExitCode::from(screen::run(&output, &[]));
        }
        eprintln!("{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    };
    let rest = rest.to_vec();
    // The commands that run Coder or its delegates put the lease shims on
    // the delegates' `PATH`, so their heavy `cargo` commands take build
    // leases (#10756).
    if matches!(
        command.as_str(),
        "coder" | "host" | "task" | "chat" | "terminal" | "studio"
    ) {
        coder::task::targets::enable_lease_shims();
    }
    let code = match command.as_str() {
        "coder" => coder_new::programmatic::run(&rest, json),
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
            0
        }
        "version" | "doctor" if rest.first().is_some_and(|word| word == "--help") => {
            println!("usage: openagents {command} [--json]");
            0
        }
        "version" | "--version" => {
            output.emit(&serde_json::json!({ "version": version_line() }), |value| {
                value["version"].as_str().unwrap_or("").to_owned()
            });
            0
        }
        "doctor" => doctor(&output),
        "host" => runtime().block_on(host(&rest)),
        "pair" => runtime().block_on(pair(&rest)),
        "task" => runtime().block_on(coder::task::cli::run_with_json(&rest, json)),
        "issue" => issue::run(&output, &rest),
        "project" => issue::project(&output, &rest),
        "lease" | "leases" => lease::run(&output, &rest),
        "sales" => sales::run(&output, &rest),
        "customer" => customer::run(&output, &rest),
        "scratch" => scratch::run(&output, &rest),
        "browser" => browser::run(&output, &rest),
        "capacity" => capacity::run(&output, &rest),
        "artifact" | "artifacts" => artifact::run(&output, &rest),
        "chat" => chat::run(&output, &rest),
        "terminal" => screen::run(&output, &rest),
        "computer" | "computers" => computer::run(&output, &rest),
        #[cfg(unix)]
        "connect" => connect::run(&output, &rest),
        "verse" => world::run(&output, &rest),
        // `openagents xp …` is `openagents verse xp …`.
        "xp" if rest.first().is_some_and(|word| word == "--help") => {
            println!("{}", world::xp_usage());
            0
        }
        "xp" => world::run_xp(&output, &rest),
        "zone" => zone::run(&output, &rest),
        "chamber" => chamber::run(&output, &rest),
        "study" => study::run(&output, &rest),
        "session" | "sessions" => session::run(&output, &rest),
        "sov" => sov::run(&output, &rest),
        "eval" => eval::run(&output, &rest),
        "gym" => gym::run(&output, &rest),
        #[cfg(unix)]
        "labor" => labor::run(&output, &rest),
        "key" => key::run(&output, &rest),
        "inference" => inference::run(&output, &rest),
        // `wallet serve` is the x402 node's resident under its old name,
        // which services installed before 2026-10-02 still start.
        #[cfg(unix)]
        "wallet" if rest.first().is_some_and(|word| word == "serve") => {
            x402_node::run(&output, &rest)
        }
        #[cfg(unix)]
        "wallet" => wallet::run(&output, &rest),
        #[cfg(unix)]
        "x402" => x402::run(&output, &rest),
        #[cfg(unix)]
        "pay" => pay::run(&output, &rest),
        "kb" => kb::run(&output, &rest),
        "reach" => reach::run(&output, &rest),
        "playtest" => playtest::run(&output, &rest),
        "relay" => relay::run(&output, &rest),
        #[cfg(unix)]
        "pylon" => pylon_wallet::run(output.json(), &rest),
        #[cfg(not(unix))]
        "pylon" => pylon::cli::run(output.json(), &rest),
        #[cfg(unix)]
        "service" => service::run(&output, &rest),
        #[cfg(unix)]
        "background" => background::run(&output, &rest),
        #[cfg(unix)]
        "worktree" | "worktrees" => worktree::run(&output, &rest),
        "settings" => settings::run(&output, &rest),
        "boat" => boat_run::run(&output, &rest),
        "studio" => studio::run(&output, &rest),
        "agent" | "agents" => agent::run(&output, &rest),
        "shadow" => shadow::run(&output, &rest),
        "efficiency" => efficiency::run(&output, &rest),
        "cloud" => cloud::run(&output, &rest),
        // Website deploys and pull request reviews and merges, under the
        // owner's approval policy (#11169, #11170).
        "deploy" => deploy::run(&output, &rest),
        "pr" => pr::run(&output, &rest),
        // Mac-only steps on a Mac linked to the account (#11223).
        "mac" => mac::run(&output, &rest),
        #[cfg(unix)]
        "ssh" => ssh::run(&output, &rest),
        "cap" => catalog::cap(&output, &rest),
        "prg" => catalog::prg(&output, &rest),
        // `ext` is the older name for `plugin`, kept working.
        "plugin" | "plugins" | "ext" => catalog::ext(&output, &rest),
        "discover" => discover::run(&output, &rest),
        "mcp" => mcp::run(&output, &rest, USAGE),
        "completions" => mcp::completions(&output, &rest, USAGE),
        // These groups stand on Unix pieces: the host's control socket and
        // service manager, the resident wallet, Unix file modes, and the
        // system ssh's process groups.
        #[cfg(not(unix))]
        "background" | "worktree" | "worktrees" | "connect" | "labor" | "service" | "ssh"
        | "wallet" | "x402" | "pay" => {
            eprintln!("openagents {command}: not available on Windows; run it from macOS or Linux");
            EXIT_FAILURE
        }
        other => {
            let suggestion = top_level_suggestion(other)
                .map(|name| format!("; did you mean `openagents {name}`?"))
                .unwrap_or_default();
            eprintln!(
                "openagents: unknown command `{other}`{suggestion}\nSee `openagents --help` for available commands."
            );
            EXIT_USAGE
        }
    };
    ExitCode::from(code)
}

/// The stream to this computer's host control socket. Windows has no
/// control socket (`openagents_connect::control::socket_path` answers
/// `None` there), so nothing dials one; the type keeps the code one shape.
#[cfg(unix)]
pub(crate) type ControlStream = tokio::net::UnixStream;
#[cfg(not(unix))]
pub(crate) type ControlStream = tokio::net::TcpStream;

/// Connects to the host control socket at `socket`.
pub(crate) async fn dial_control(socket: &Path) -> std::io::Result<ControlStream> {
    #[cfg(unix)]
    {
        ControlStream::connect(socket).await
    }
    #[cfg(not(unix))]
    {
        let _ = socket;
        Err(std::io::ErrorKind::Unsupported.into())
    }
}

/// Whether a host answers on the control socket at `socket`, blocking.
pub(crate) fn host_answers_at(socket: &Path) -> bool {
    #[cfg(unix)]
    {
        std::os::unix::net::UnixStream::connect(socket).is_ok()
    }
    #[cfg(not(unix))]
    {
        let _ = socket;
        false
    }
}

/// What `openagents --version` prints: this program's release
/// (`1.0.0-rc.1`), then the repository, the commit, and the tree state the
/// build came from.
pub(crate) fn version_line() -> String {
    coder::identity::program_line("openagents", env!("CARGO_PKG_VERSION"))
}

pub fn runtime() -> tokio::runtime::Runtime {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("a Tokio runtime builds")
}

async fn host(arguments: &[String]) -> u8 {
    let observers = match host_observers::Options::take(arguments) {
        Ok(options) => options,
        Err(why) => {
            eprintln!("openagents host: {why}");
            return EXIT_FAILURE;
        }
    };
    let arguments = &observers.arguments;
    // The host root, as `coder host` reads it: `--root DIR`, else
    // ~/.openagents/host. The workshop agents live there.
    let root = arguments
        .windows(2)
        .find(|pair| pair[0] == "--root")
        .map(|pair| std::path::PathBuf::from(&pair[1]))
        .or_else(|| {
            std::env::var_os("HOME")
                .map(|home| std::path::PathBuf::from(home).join(".openagents/host"))
        });
    // The packaged environment owners' providers (Boat, or the optional
    // dedicated GCE adapter), built before the
    // host opens its owners (ENV-08). Their loop starts and stops with the
    // cloud operator they are composed with.
    #[cfg(unix)]
    let environment = match &observers.environment {
        Some(path) => {
            let config = match coder_environment_operator::Config::load(path) {
                Ok(c) => c,
                Err(why) => {
                    eprintln!("openagents host: {why}");
                    return EXIT_FAILURE;
                }
            };
            let janitor = match coder_environment_operator::gce::janitor_for(&config) {
                Ok(j) => j,
                Err(why) => {
                    eprintln!("openagents host: {why}");
                    return EXIT_FAILURE;
                }
            };
            match coder_environment_operator::gce::providers(&config).await {
                Ok(providers) => Some((config, providers, janitor)),
                Err(why) => {
                    eprintln!("openagents host: {why}");
                    return EXIT_FAILURE;
                }
            }
        }
        None => None,
    };
    let open = Box::new(
        move |store: &Path, workspaces: &std::collections::BTreeMap<String, std::path::PathBuf>| {
            let mut inbox = coder::task::remote::Inbox::new(store, workspaces.clone());
            #[cfg(unix)]
            if let Some(path) = &observers.projects {
                let observer = coder_project::observe::Observer::load(path, workspaces)?;
                inbox = inbox.with_projects(std::sync::Arc::new(observer));
            }
            #[cfg(unix)]
            if let Some(path) = &observers.cloud {
                let state = observers
                    .state
                    .as_ref()
                    .ok_or("operator cloud needs an explicit access state directory")?;
                let root = observers
                    .root
                    .as_ref()
                    .ok_or("operator cloud needs an explicit host root directory")?;
                let authority = coder_host::cloud::authority(coder_access::host::Host::new(
                    state,
                    observers.policy,
                ))
                .map_err(|_| "operator cloud authority is unavailable")?;
                let state = root.join("cloud-operator");
                let mut operator = coder_cloud::operator::Operator::load(path, &state, authority)?;
                if let Some((config, providers, janitor)) = environment {
                    let owners = coder_environment_operator::Owners::open(
                        &state,
                        providers,
                        coder_environment_operator::environment_custody(),
                    )?
                    .with_janitor(janitor);
                    // The service handle lives in the composed operator.
                    operator = coder_environment_operator::attach(
                        std::sync::Arc::new(owners),
                        operator,
                        config.cadence(),
                    )?
                    .0;
                }
                inbox = inbox.with_cloud(std::sync::Arc::new(operator));
            }
            if let Some(root) = root.clone() {
                inbox = inbox.with_agents(coder::task::agent_host::Agents::new(
                    root,
                    store,
                    workspaces.clone(),
                ));
            }
            Ok(std::sync::Arc::new(inbox) as std::sync::Arc<dyn coder_host::Tasks>)
        },
    );
    // A host that keeps its keys in the keychain keeps the agents' keys
    // there too.
    if let Err(why) = coder::task::agent_key::install_for_host(arguments) {
        eprintln!("openagents host: {why}");
        return EXIT_FAILURE;
    }
    coder_host::control::set_local_coder(coder::task::local::ready_here);
    coder_host::control::set_local_runner(coder::task::local::runner_here);
    coder_host::control::set_local_engines(coder::task::local::engines_here);
    coder_host::control::set_local_result(coder::task::local::result_in);
    // `serve --studio-sim`: a scratch host's simulated studio (#10572).
    coder_host::cli::set_studio_sim(coder::task::studio_sim::open_host);
    #[cfg(unix)]
    coder_host::background::set_facts(coder::task::background_facts);
    #[cfg(unix)]
    if let Some(judge) = background::JevJudge::from_env() {
        coder_host::background::set_judge(std::sync::Arc::new(judge));
    }
    #[cfg(unix)]
    coder_host::background::set_services(background::HostServices::at);
    coder_host::cli::run(arguments, open).await
}

async fn pair(arguments: &[String]) -> u8 {
    match coder_connect::cli::pair(arguments).await {
        Ok(()) => 0,
        Err(error) => {
            eprintln!("openagents pair: {error}");
            EXIT_FAILURE
        }
    }
}

/// The paths and identities every group reads, so a person can see what a
/// command will use before it runs.
fn doctor(output: &Output) -> u8 {
    let home = std::env::var_os("HOME").map_or_else(|| Path::new(".").to_path_buf(), Into::into);
    let openagents = home.join(".openagents");
    let entries = [
        ("verse_keys", ::verse::identity::home()),
        ("coder_access", openagents.join("coder-access")),
        ("host_root", openagents.join("host")),
        ("computers", computer::store_dir(None)),
        ("tasks", openagents.join("tasks")),
        ("sov", sov::home()),
        ("wallet", openagents_spark::computer::home()),
        ("x402_node", openagents_wallet::config::home()),
    ];
    let mut report = serde_json::Map::new();
    report.insert("version".into(), version_line().into());
    report.insert(
        "default_relay".into(),
        ::verse::session::PUBLIC_RELAY.into(),
    );
    report.insert("world".into(), ::verse::session::WORLD.into());
    let mut paths = serde_json::Map::new();
    for (name, path) in entries {
        paths.insert(
            name.into(),
            serde_json::json!({
                "path": path.display().to_string(),
                "exists": path.exists(),
            }),
        );
    }
    report.insert("paths".into(), paths.into());
    // Where `openagents chat` keeps threads, and its public identity; the
    // device key itself is never printed.
    report.insert("chat".into(), chat::doctor());
    output.emit(&serde_json::Value::Object(report), |value| {
        let mut lines = vec![
            format!("version   {}", value["version"].as_str().unwrap_or("")),
            format!(
                "relay     {}",
                value["default_relay"].as_str().unwrap_or("")
            ),
            format!(
                "world     {} (the Verse world `openagents verse` joins)",
                value["world"].as_str().unwrap_or("")
            ),
        ];
        let chat = &value["chat"];
        lines.push(format!(
            "chat           {} ({})",
            match chat["backend"].as_str() {
                Some("host") => "in this computer's host",
                _ => "stored on this computer",
            },
            if chat["host_running"].as_bool().unwrap_or(false) {
                chat["host_socket"].as_str().unwrap_or("").to_owned()
            } else {
                format!(
                    "{}, identity {}",
                    chat["home"].as_str().unwrap_or(""),
                    chat["identity"].as_str().unwrap_or("not created yet")
                )
            }
        ));
        if let Some(paths) = value["paths"].as_object() {
            for (name, entry) in paths {
                let mark = if entry["exists"].as_bool().unwrap_or(false) {
                    "present"
                } else {
                    "absent "
                };
                lines.push(format!(
                    "{name:<14} {mark} {}",
                    entry["path"].as_str().unwrap_or("")
                ));
            }
        }
        lines.join("\n")
    });
    0
}
