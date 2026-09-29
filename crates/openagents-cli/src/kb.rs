//! Knowledge entry commands over the knowledge and Microcoder implementations.

use serde_json::Value;

use crate::{EXIT_USAGE, Output, runtime};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents kb COMMAND [OPTIONS]
  search TEXT [--dir DIR] [--lexical] [--limit N]
        Search local and trusted cached entries.
  show ID [--dir DIR]        Show an entry and its digest.
  withdraw ID [--dir DIR] [--reason TEXT]
        Mark a local entry as withdrawn.
  publish [IDS] --relay URL --timeout SECONDS [--dir DIR] [--key-file FILE]
        Sign and publish local entries, heads, and withdrawals.
  sync --relay URL --timeout SECONDS [--author KEY]... [--remote DIR]
        Fetch, verify, and cache entries from the relay.
  head ID --relay URL --timeout SECONDS [--author KEY] [--key-file FILE]
        Read the latest verified head for an author and entry.
--json returns one JSON document; errors return 1, and invalid usage returns 64.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("search", Effect::ReadOnly),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("withdraw", Effect::LocalWrite),
    Declared::computer("publish", Effect::Publishes),
    Declared::computer("sync", Effect::LocalWrite),
    Declared::computer("head", Effect::ReadOnly),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, _)) = words.split_first() else {
        return output.usage("kb", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "help" | "-h" | "--help") {
        println!("{USAGE}");
        return 0;
    }
    let result = match command.as_str() {
        "search" | "show" | "withdraw" => runtime().block_on(knowledge::cli::result(words)),
        "publish" | "sync" | "head" => runtime().block_on(microcoder::kbnet::result(words)),
        _ => return output.usage("kb", "unknown command", USAGE),
    };
    match result {
        Ok((code, value)) => {
            output.emit(&value, |value| render(command, value));
            code
        }
        Err((EXIT_USAGE, message)) => output.usage("kb", &message, USAGE),
        Err((_, message)) => output.fail("kb", &message),
    }
}

fn render(command: &str, value: &Value) -> String {
    match command {
        "search" => {
            let mut lines = Vec::new();
            if let Some(reason) = value["lexical_only"].as_str() {
                lines.push(format!("ranked by words alone: {reason}"));
            }
            lines.extend(value["hits"].as_array().into_iter().flatten().map(|hit| {
                format!(
                    "{}: {} ({:.3})",
                    hit["id"].as_str().unwrap_or(""),
                    hit["entry"]["title"].as_str().unwrap_or(""),
                    hit["score"].as_f64().unwrap_or(0.0)
                )
            }));
            lines.join("\n")
        }
        "show" => format!(
            "{}\n{}",
            value["entry"]["digest"].as_str().unwrap_or(""),
            value["document"].as_str().unwrap_or("")
        ),
        "withdraw" => format!(
            "{} v{} withdrawn",
            value["id"].as_str().unwrap_or(""),
            value["version"]
        ),
        "publish" => format!(
            "{} published, {} already present, {} withdrawn, {} refused",
            value["published"], value["present"], value["withdrawals"], value["refused"]
        ),
        "sync" => format!(
            "{} entries accepted, {} refused, {} incomplete queries",
            value["accepted"].as_array().map_or(0, Vec::len),
            value["refused"].as_array().map_or(0, Vec::len),
            value["incomplete"]
        ),
        "head" => format!(
            "{} v{}: {}",
            value["id"].as_str().unwrap_or(""),
            value["version"],
            value["event"].as_str().unwrap_or("")
        ),
        _ => String::new(),
    }
}
