//! `openagents settings`: the local capability settings every OpenAgents
//! program on this computer reads (`coder::task::settings`), shown and
//! edited as flat `coder.*` keys.

use serde_json::{Value, json};

use crate::{Args, Output};
#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::task::settings::{self, Settings};

pub(crate) const USAGE: &str = "usage: openagents settings COMMAND [OPTIONS]
  show                    Print every setting, the defaults where none is set.
  get KEY                 Print one setting.
  set KEY VALUE           Change one setting.
  unset KEY               Return one setting to its default.
Keys:
  coder.providers                 Coding agents Coder may use, first preferred:
                                  codex, claude, grok, opencode:PROVIDER/MODEL, devin,
                                  comma-separated, each optionally NAME:MODEL
                                  (default codex,claude,grok).
  coder.start                     at_once or ask_first (default at_once).
  coder.usage_threshold_percent   1 to 100, or off (default 90).
  coder.projects                  Folders whose Git checkouts are projects,
                                  comma-separated; empty is any (default).
  coder.access                    full, toolchains, or boundary (default full: every step approved).
Settings live in ~/.openagents/settings.json (OPENAGENTS_SETTINGS overrides the
file); openagents chat, the desktop app, and a host on this computer read it.";

/// What each command above does and where the phone runs it, for the
/// chat router's command tree (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("get", Effect::ReadOnly),
    Declared::computer("set", Effect::LocalWrite),
    Declared::computer("unset", Effect::LocalWrite),
];

pub(crate) fn render(value: &Value) -> String {
    match value {
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map_or_else(|| item.to_string(), str::to_owned)
            })
            .collect::<Vec<_>>()
            .join(","),
        Value::String(text) => text.clone(),
        Value::Null => "off".into(),
        other => other.to_string(),
    }
}

pub fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("settings", "a command is required", USAGE);
    };
    let args = match Args::parse(rest, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("settings", &message, USAGE),
    };
    let file = settings::path();
    let words = args.positional();
    let want = |count: usize| {
        if words.len() == count {
            Ok(())
        } else {
            Err(output.usage(
                "settings",
                &format!("`{command}` takes {count} word(s)"),
                USAGE,
            ))
        }
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let mut loaded = match Settings::load(&file) {
        Ok(loaded) => loaded,
        // A file that is not valid is named, never overwritten.
        Err(message) => return output.fail("settings", &message),
    };
    match command.as_str() {
        "show" => {
            if let Err(code) = want(0) {
                return code;
            }
            let values = loaded.values();
            output.emit(
                &json!({ "path": file.display().to_string(), "exists": file.exists(), "settings": values }),
                |value| {
                    value["settings"]
                        .as_object()
                        .map(|map| {
                            map.iter()
                                .map(|(key, value)| format!("{key:<30} {}", render(value)))
                                .collect::<Vec<_>>()
                                .join("\n")
                        })
                        .unwrap_or_default()
                },
            );
            0
        }
        "get" => {
            if let Err(code) = want(1) {
                return code;
            }
            match loaded.get(&words[0]) {
                Ok(value) => {
                    output.emit(&json!({ "key": words[0], "value": value }), |value| {
                        render(&value["value"])
                    });
                    0
                }
                Err(message) => output.usage("settings", &message, USAGE),
            }
        }
        "set" | "unset" => {
            let changed = if command == "set" {
                if let Err(code) = want(2) {
                    return code;
                }
                let here = std::env::current_dir().unwrap_or_default();
                loaded.set(&words[0], &words[1], &here)
            } else {
                if let Err(code) = want(1) {
                    return code;
                }
                loaded.unset(&words[0])
            };
            if let Err(message) = changed {
                return output.fail("settings", &message);
            }
            if let Err(message) = loaded.save(&file) {
                return output.fail("settings", &message);
            }
            let value = loaded.get(&words[0]).unwrap_or(Value::Null);
            output.emit(
                &json!({ "key": words[0], "value": value, "path": file.display().to_string() }),
                |value| format!("{} {}", words[0], render(&value["value"])),
            );
            0
        }
        other => output.usage("settings", &format!("unknown command `{other}`"), USAGE),
    }
}
