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
  disable AGENT           Turn a coding agent off: Coder never uses it.
  enable AGENT            Turn a coding agent back on.
  provider-key set PROVIDER
                          Add your own OpenRouter, Vercel AI Gateway, or TypeSafe
                          key, read from a hidden prompt or stdin.
  provider-key show [PROVIDER]
                          Your keys: last four characters, label, spend, state.
  provider-key test [PROVIDER]
                          Test your keys now.
  provider-key clear PROVIDER
                          Remove one of your keys.
Every coding agent signed in on this computer is used, Codex first; nothing
needs enabling. Agents: codex, claude, grok, devin, opencode.
Keys:
  coder.disabled                  Coding agents turned off, comma-separated
                                  (default none).
  coder.providers                 The order Coder tries agents in, first preferred,
                                  comma-separated, each optionally NAME:MODEL
                                  (opencode:PROVIDER/MODEL); agents left out follow
                                  in the default order codex,claude,grok,devin,opencode.
  coder.start                     at_once or ask_first (default at_once).
  coder.usage_threshold_percent   1 to 100, or off (default 90).
  coder.projects                  Folders whose Git checkouts are projects,
                                  comma-separated; empty is any (default).
  coder.access                    full, toolchains, or boundary (default full: Coder runs every
                                  step without asking).
  coder.shadow                    1 to 100: that percent of finished Coder runs also run once
                                  through the raw engine, to record what they would have
                                  cost (openagents shadow report); or off (default off).
  coder.shadow_budget_usd         The most the shadow baselines may cost in all, in dollars,
                                  or off for no cap (default off).
  coder.claude                    session (one lean Claude Code session briefed by Jev)
                                  or loop (Microcoder's step loop) (default session).
  coder.codex                     session (one codex exec session briefed by Jev)
                                  or loop (Microcoder's step loop) (default loop).
  coder.build_leases              Concurrent builds, at least 1 (default cores / 4,
                                  at least 1 and at most 4). OPENAGENTS_BUILD_LEASES wins.
  coder.placement                 Where long jobs run (openagents lease run --class): CLASS=PLACE
                                  and computer=NAME entries, comma-separated (default
                                  release-gate=auto,bench=auto,soak=local,build=local, no computer).
  models.payer                   ours (default: OpenAgents pays for model calls) or mine (every
                                  model call on your own keys, never ours; needs an
                                  OpenRouter or Vercel AI Gateway key).
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
    Declared::computer("disable", Effect::LocalWrite),
    Declared::computer("enable", Effect::LocalWrite),
    Declared::computer("provider-key set", Effect::Secret),
    Declared::computer("provider-key show", Effect::Secret),
    Declared::computer("provider-key test", Effect::Secret),
    Declared::computer("provider-key clear", Effect::Secret),
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
    if command == "provider-key" {
        return crate::provider_key::run(output, rest);
    }
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
            // What an empty list means, in words: the order Coder tries,
            // nothing turned off, every project.
            let order = loaded
                .coder
                .provider_list()
                .into_iter()
                .map(settings::provider_name)
                .collect::<Vec<_>>()
                .join(", ");
            output.emit(
                &json!({ "path": file.display().to_string(), "exists": file.exists(), "settings": values }),
                |value| {
                    value["settings"]
                        .as_object()
                        .map(|map| {
                            map.iter()
                                .map(|(key, value)| {
                                    let empty = value.as_array().is_some_and(Vec::is_empty);
                                    let shown = match key.as_str() {
                                        "coder.providers" if empty => format!("{order} (default)"),
                                        "coder.disabled" if empty => "none".to_owned(),
                                        "coder.projects" if empty => {
                                            "every Git checkout (default)".to_owned()
                                        }
                                        _ => render(value),
                                    };
                                    format!("{key:<30} {shown}")
                                })
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
        "disable" | "enable" => {
            if let Err(code) = want(1) {
                return code;
            }
            let on = command == "enable";
            let changed = settings::agent(&words[0]).and_then(|provider| {
                loaded.allow(provider, on)?;
                Ok(provider)
            });
            let provider = match changed {
                Ok(provider) => provider,
                Err(message) => return output.fail("settings", &message),
            };
            if let Err(message) = loaded.save(&file) {
                return output.fail("settings", &message);
            }
            let name = settings::provider_name(provider);
            output.emit(
                &json!({
                    "agent": provider.as_str(),
                    "on": on,
                    "disabled": loaded.get("coder.disabled").unwrap_or(Value::Null),
                    "path": file.display().to_string(),
                }),
                |_| {
                    if on {
                        format!("{name} is on: Coder uses it whenever it is signed in here.")
                    } else {
                        format!("{name} is off: Coder will not use it.")
                    }
                },
            );
            0
        }
        "set" | "unset" => {
            let changed = if command == "set" {
                if let Err(code) = want(2) {
                    return code;
                }
                let here = std::env::current_dir().unwrap_or_default();
                if words[0] == "models.payer" {
                    crate::provider_key::set_payer(&mut loaded, &words[1])
                } else {
                    loaded.set(&words[0], &words[1], &here)
                }
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
