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

// `connect`, `labor`, `service`, `ssh`, `wallet`, and `x402` are Unix-only
// (see the dispatch below); Windows builds the rest.
mod argv;
#[cfg(unix)]
mod background;
mod catalog;
mod chat;
mod computer;
#[cfg(unix)]
mod connect;
mod discover;
mod eval;
mod ext_defaults;
mod ext_eval;
mod ext_eval_init;
mod ext_run;
mod gym;
mod hosts;
mod kb;
mod key;
#[cfg(unix)]
mod labor;
mod mcp;
mod out;
mod playtest;
#[cfg(unix)]
mod plugin_local;
#[cfg(unix)]
mod plugin_registry;
mod quest;
mod reach;
mod relay;
mod screen;
#[cfg(unix)]
mod service;
mod session;
mod settings;
mod sov;
mod sov_host;
#[cfg(unix)]
mod ssh;
mod study;
mod terminal;
#[cfg(test)]
mod tree;
#[cfg(unix)]
mod wallet;
mod world;
#[cfg(unix)]
mod x402;
#[cfg(unix)]
mod x402_native;
#[cfg(unix)]
mod x402_node;
#[cfg(unix)]
mod x402_phone;
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
  task         Durable local task requests and explicit execution.
  settings     What Coder may use on this computer: providers, ask first, and more.
  service      Install, update, and roll back the resident host service.
  background   The host's background rules, built in and from plugins turned on here.
  ssh          Start or adopt a host over SSH and tunnel to it.

Verse (NIP-MV):
  verse        See who is around, listen, speak, move, gesture, drive owned
               entities, and read quests, XP, and the board.
  xp           This identity's XP and level (openagents verse xp), and
               verify-card to re-derive a trainer card from the relays.
  zone         Drive the Lagrange 1 construction zone.
  sov          Sovereign agents under NIP-SOV: profile, spawn, status.

Gym (NIP-EVAL):
  eval         Score doors against a suite, report a store, compare its sides.
  gym          Connect to a granted Gym host, observe it, launch admitted recipes.

Labor (NIP-MKT, NIP-LAB):
  labor        Admit, negotiate, execute, deliver, and accept free labor orders.

Keys, relays, and money:
  key          Show or create Nostr identities.
  wallet       Your OpenAgents wallet: your balance and an address to get paid at.
  x402         Sell a command over HTTP for an exact bitcoin amount, or buy one (http:1),
               through this computer's Lightning node (`x402 node`).
  kb           Search, publish, and sync knowledge entries (NIP-KB).
  relay        Query, publish to, and follow a relay.

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

fn main() -> ExitCode {
    #[cfg(windows)]
    home_from_profile();
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    let json = arguments.iter().any(|argument| argument == "--json");
    arguments.retain(|argument| argument != "--json");
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
    let code = match command.as_str() {
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
        "task" => runtime().block_on(coder::task::cli::run(&rest)),
        "chat" => chat::run(&output, &rest),
        "terminal" => screen::run(&output, &rest),
        "computer" | "computers" => computer::run(&output, &rest),
        #[cfg(unix)]
        "connect" => connect::run(&output, &rest),
        "verse" => world::run(&output, &rest),
        // `openagents xp …` is `openagents verse xp …`.
        "xp" if rest.first().is_some_and(|word| word == "--help") => world::run(&output, &rest),
        "xp" => world::run(
            &output,
            &std::iter::once("xp".to_owned())
                .chain(rest.iter().cloned())
                .collect::<Vec<_>>(),
        ),
        "zone" => zone::run(&output, &rest),
        "study" => study::run(&output, &rest),
        "session" | "sessions" => session::run(&output, &rest),
        "sov" => sov::run(&output, &rest),
        "eval" => eval::run(&output, &rest),
        "gym" => gym::run(&output, &rest),
        #[cfg(unix)]
        "labor" => labor::run(&output, &rest),
        "key" => key::run(&output, &rest),
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
        "kb" => kb::run(&output, &rest),
        "reach" => reach::run(&output, &rest),
        "playtest" => playtest::run(&output, &rest),
        "relay" => relay::run(&output, &rest),
        #[cfg(unix)]
        "service" => service::run(&output, &rest),
        #[cfg(unix)]
        "background" => background::run(&output, &rest),
        "settings" => settings::run(&output, &rest),
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
        "background" | "connect" | "labor" | "service" | "ssh" | "wallet" | "x402" => {
            eprintln!("openagents {command}: not available on Windows; run it from macOS or Linux");
            EXIT_FAILURE
        }
        other => {
            eprintln!("openagents: unknown command `{other}`\n\n{USAGE}");
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
    let open = Box::new(
        |store: &Path, workspaces: &std::collections::BTreeMap<String, std::path::PathBuf>| {
            let inbox = coder::task::remote::Inbox::new(store, workspaces.clone());
            Ok(std::sync::Arc::new(inbox) as std::sync::Arc<dyn coder_host::Tasks>)
        },
    );
    coder_host::control::set_local_coder(coder::task::local::ready_here);
    coder_host::control::set_local_runner(coder::task::local::runner_here);
    coder_host::control::set_local_engines(coder::task::local::engines_here);
    coder_host::control::set_local_result(coder::task::local::result_in);
    #[cfg(unix)]
    coder_host::background::set_facts(coder::task::background_facts);
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
            format!("world     {}", value["world"].as_str().unwrap_or("")),
        ];
        let chat = &value["chat"];
        lines.push(format!(
            "chat           {} ({})",
            chat["backend"].as_str().unwrap_or(""),
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
