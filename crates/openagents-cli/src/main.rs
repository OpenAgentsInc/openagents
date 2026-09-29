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

use std::path::Path;
use std::process::ExitCode;

mod catalog;
mod computer;
mod connect;
mod discover;
mod eval;
mod ext_defaults;
mod ext_eval;
mod ext_eval_init;
mod gym;
mod hosts;
mod kb;
mod key;
mod labor;
mod mcp;
mod out;
mod playtest;
mod quest;
mod reach;
mod relay;
mod service;
mod session;
mod sov;
mod sov_host;
mod ssh;
mod study;
mod terminal;
#[cfg(test)]
mod tree;
mod wallet;
mod world;
mod x402;
mod x402_native;
mod x402_phone;
mod zone;

pub use coder::argv::Args;
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

Coder:
  task         Durable local task requests and explicit execution.
  service      Install, update, and roll back the resident host service.
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
  wallet       Lightning node for x402 (ldk-node): invoices, payments, channels.
  x402         Sell a command over HTTP for an exact bitcoin amount, or buy one (http:1).
  kb           Search, publish, and sync knowledge entries (NIP-KB).
  relay        Query, publish to, and follow a relay.

Playtesting:
  playtest     Triage inbox: read reports, draft and file issues, keep the triage log.

Discovery (NIP-CAP, NIP-PRG, NIP-EXT), read-only:
  cap          List and describe published capability heads.
  prg          List and describe published program heads.
  ext          List extension records, and run and check extension evals.
  discover     Show the well-known agent card and agent-skills index.

  mcp          Serve every group as an MCP tool over stdio (mcp serve).
  completions  Print a bash, zsh, or fish completion script.
  doctor       Show the identities, stores, and relays this command uses.
  version      Show the repository, commit, and tree state.

Run `openagents COMMAND --help` for each group's syntax.
Exit codes: 0 success, 1 refused or failed, 64 invalid usage.";

fn main() -> ExitCode {
    let mut arguments: Vec<String> = std::env::args().skip(1).collect();
    let json = arguments.iter().any(|argument| argument == "--json");
    arguments.retain(|argument| argument != "--json");
    let output = Output::new(json);
    let Some((command, rest)) = arguments.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(EXIT_USAGE);
    };
    let rest = rest.to_vec();
    let code = match command.as_str() {
        "help" | "--help" | "-h" => {
            println!("{USAGE}");
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
        "computer" | "computers" => computer::run(&output, &rest),
        "connect" => connect::run(&output, &rest),
        "verse" => world::run(&output, &rest),
        // `openagents xp …` is `openagents verse xp …`.
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
        "labor" => labor::run(&output, &rest),
        "key" => key::run(&output, &rest),
        "wallet" => wallet::run(&output, &rest),
        "x402" => x402::run(&output, &rest),
        "kb" => kb::run(&output, &rest),
        "reach" => reach::run(&output, &rest),
        "playtest" => playtest::run(&output, &rest),
        "relay" => relay::run(&output, &rest),
        "service" => service::run(&output, &rest),
        "ssh" => ssh::run(&output, &rest),
        "cap" => catalog::cap(&output, &rest),
        "prg" => catalog::prg(&output, &rest),
        "ext" => catalog::ext(&output, &rest),
        "discover" => discover::run(&output, &rest),
        "mcp" => mcp::run(&output, &rest, USAGE),
        "completions" => mcp::completions(&output, &rest, USAGE),
        other => {
            eprintln!("openagents: unknown command `{other}`\n\n{USAGE}");
            EXIT_USAGE
        }
    };
    ExitCode::from(code)
}

fn version_line() -> String {
    coder::identity::line().replacen("coder ", "openagents ", 1)
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
        ("wallet", openagents_wallet::config::home()),
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
    output.emit(&serde_json::Value::Object(report), |value| {
        let mut lines = vec![
            format!("version   {}", value["version"].as_str().unwrap_or("")),
            format!(
                "relay     {}",
                value["default_relay"].as_str().unwrap_or("")
            ),
            format!("world     {}", value["world"].as_str().unwrap_or("")),
        ];
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
