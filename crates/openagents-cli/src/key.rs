//! Nostr identities: the Verse profile keys under `~/.openagents/verse/`.
//! A key is created on first use and is never printed; only its public key
//! and file path are shown.

use serde_json::json;

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};

pub(crate) const USAGE: &str = "usage: openagents key COMMAND [OPTIONS]
  show [--as PROFILE]     Print the profile's public key, creating the key if absent.
  list                    List the profiles that hold a key.
Keys live in ~/.openagents/verse/PROFILE.key (VERSE_HOME overrides the
directory). The secret is never printed.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::screen("show", Effect::LocalWrite, "account.keys"),
    Declared::screen("list", Effect::ReadOnly, "account.keys"),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("key", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("key", &message, USAGE),
    };
    match command.as_str() {
        "--help" | "-h" | "help" => {
            println!("{USAGE}");
            0
        }
        "show" | "create" => match crate::relay::identity_for(args.option("as")) {
            Ok(identity) => {
                output.emit(
                    &json!({
                        "profile": identity.profile,
                        "pubkey": identity.signer.pubkey(),
                        "created": identity.created,
                        "path": verse::identity::home()
                            .join(format!("{}.key", identity.profile))
                            .display()
                            .to_string(),
                    }),
                    |value| {
                        format!(
                            "{} {}{}",
                            value["profile"].as_str().unwrap_or(""),
                            value["pubkey"].as_str().unwrap_or(""),
                            if value["created"].as_bool().unwrap_or(false) {
                                " (new)"
                            } else {
                                ""
                            }
                        )
                    },
                );
                0
            }
            Err(message) => output.fail("key", &message),
        },
        "list" => {
            let home = verse::identity::home();
            let mut profiles = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&home) {
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if let Some(profile) = name.strip_suffix(".key") {
                        let pubkey = verse::identity::load_or_create(&home, profile)
                            .map(|identity| identity.signer.pubkey().to_owned())
                            .unwrap_or_default();
                        profiles.push(json!({ "profile": profile, "pubkey": pubkey }));
                    }
                }
            }
            profiles.sort_by(|a, b| a["profile"].as_str().cmp(&b["profile"].as_str()));
            output.emit(&json!({ "profiles": profiles }), |value| {
                value["profiles"]
                    .as_array()
                    .filter(|profiles| !profiles.is_empty())
                    .map_or_else(
                        || "No keys yet; `openagents key show` creates one.".to_owned(),
                        |profiles| {
                            profiles
                                .iter()
                                .map(|profile| {
                                    format!(
                                        "{} {}",
                                        profile["profile"].as_str().unwrap_or(""),
                                        profile["pubkey"].as_str().unwrap_or("")
                                    )
                                })
                                .collect::<Vec<_>>()
                                .join("\n")
                        },
                    )
            });
            0
        }
        other => output.usage("key", &format!("unknown command `{other}`"), USAGE),
    }
}
