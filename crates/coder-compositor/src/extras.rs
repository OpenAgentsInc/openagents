//! A host's own launchers and window rules, read from the grant.
//!
//! The launcher rows and window rules in `crates/coder-binds` are the ones
//! the public CoderOS modules carry. A module in a host's own flake, such
//! as a game launcher, sets `coderos.desktop.extraBinds` and
//! `coderos.desktop.extraWindowRules` instead, and
//! `os/modules/coderos/desktop.nix` writes them to the grant at
//! `/etc/coderos/compositor.json` as `extraBinds` and `extraRules`. This
//! compositor reads that file once at start. An entry that does not parse is
//! logged and skipped, so one bad entry costs its own chord or rule and
//! nothing else.

use coder_binds::{Case, Effects, ExtraBind, ExtraPattern, ExtraRule, Field};
use serde_json::Value;

/// The environment variable that names the grant, which
/// `os/bin/coder-compositor-session` also reads.
pub const GRANT_VAR: &str = "CODER_COMPOSITOR_CONFIG";

/// Where the grant is when nothing names it.
pub const DEFAULT_GRANT: &str = "/etc/coderos/compositor.json";

/// The host's own launchers and window rules.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Extras {
    pub binds: Vec<ExtraBind>,
    pub rules: Vec<ExtraRule>,
}

impl Extras {
    /// The entries of the grant the environment names, or none when there
    /// is no grant, which is a checkout run by hand off CoderOS.
    pub fn read() -> Extras {
        let path = std::env::var(GRANT_VAR)
            .ok()
            .filter(|path| !path.trim().is_empty())
            .unwrap_or_else(|| DEFAULT_GRANT.to_string());
        match std::fs::read_to_string(&path) {
            Ok(text) => Extras::from_json(&text),
            Err(err) => {
                log::debug!("no grant at {path}: {err}; the host adds no launchers or rules");
                Extras::default()
            }
        }
    }

    /// The entries one grant holds.
    pub fn from_json(text: &str) -> Extras {
        let grant: Value = match serde_json::from_str(text) {
            Ok(grant) => grant,
            Err(err) => {
                log::warn!("the grant is not JSON, so the host adds no launchers or rules: {err}");
                return Extras::default();
            }
        };
        let entries = |key: &str| {
            grant
                .get(key)
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };
        let binds = entries("extraBinds")
            .iter()
            .filter_map(|entry| {
                let parsed = bind(entry);
                if parsed.is_none() {
                    log::warn!("the grant's extra bind {entry} does not parse, so it is skipped");
                }
                parsed
            })
            .collect();
        let rules = entries("extraRules")
            .iter()
            .filter_map(|entry| {
                let parsed = rule(entry);
                if parsed.is_none() {
                    log::warn!("the grant's extra rule {entry} does not parse, so it is skipped");
                }
                parsed
            })
            .collect();
        Extras { binds, rules }
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Option<&'a str> {
    value.get(key).and_then(Value::as_str)
}

fn flag(value: &Value, key: &str) -> bool {
    value.get(key).and_then(Value::as_bool).unwrap_or(false)
}

/// One `extraBinds` entry: `mods`, `key`, and `command`.
fn bind(value: &Value) -> Option<ExtraBind> {
    ExtraBind::parse(
        text(value, "mods").unwrap_or("SUPER"),
        text(value, "key")?,
        text(value, "command")?,
    )
}

/// One pattern: `exact`, or `prefix` with an optional `holds`.
fn pattern(value: &Value) -> Option<ExtraPattern> {
    match (
        text(value, "exact"),
        text(value, "prefix"),
        text(value, "holds"),
    ) {
        (Some(exact), None, None) => Some(ExtraPattern::Exact(exact.to_string())),
        (None, Some(prefix), None) => Some(ExtraPattern::Prefix(prefix.to_string())),
        (None, Some(prefix), Some(holds)) => Some(ExtraPattern::PrefixHolding {
            prefix: prefix.to_string(),
            holds: holds.to_string(),
        }),
        _ => None,
    }
}

/// One `extraRules` entry, in the shape `desktop.nix` writes it.
fn rule(value: &Value) -> Option<ExtraRule> {
    let field = match text(value, "field").unwrap_or("class") {
        "class" => Field::AppId,
        "title" => Field::Title,
        _ => return None,
    };
    let patterns = value
        .get("patterns")?
        .as_array()?
        .iter()
        .map(pattern)
        .collect::<Option<Vec<_>>>()?;
    if patterns.is_empty() {
        return None;
    }
    let effects = value.get("effects")?;
    let border = match effects.get("border") {
        None | Some(Value::Null) => None,
        Some(size) => Some(i32::try_from(size.as_u64()?).ok()?),
    };
    Some(ExtraRule {
        name: text(value, "name").unwrap_or("").to_string(),
        field,
        patterns,
        case: if flag(value, "ignoreCase") {
            Case::Any
        } else {
            Case::Exact
        },
        effects: Effects {
            float: effects.get("float").and_then(Value::as_bool),
            center: flag(effects, "center"),
            pin: flag(effects, "pin"),
            keep_aspect: flag(effects, "keepAspectRatio"),
            border,
            shadow: effects.get("shadow").and_then(Value::as_bool),
            suppress_fullscreen: flag(effects, "suppressFullscreen"),
        },
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The grant a host with Zoom, a slide deck, and Battle.net gets, as
    /// `os/tests/extension-points.nix` sets it and `desktop.nix` writes it,
    /// less the fields this file does not read.
    pub(crate) const HOST_GRANT: &str = r#"{
      "extraBinds": [
        {"command": "coder-deck-open", "key": "D", "mods": "SUPER SHIFT"},
        {"command": "coder-zoom", "key": "Z", "mods": "SUPER"},
        {"command": "coder-battlenet", "key": "G", "mods": "SUPER"}
      ],
      "extraRules": [
        {"effects": {"border": null, "center": true, "float": true, "keepAspectRatio": false, "pin": false, "shadow": null, "suppressFullscreen": false},
         "field": "class", "ignoreCase": false, "name": "Battle.net launcher",
         "patterns": [{"exact": "battle.net.exe"}, {"exact": "Battle.net.exe"}, {"exact": "steam_app_battlenet"}]},
        {"effects": {"border": null, "center": false, "float": false, "keepAspectRatio": false, "pin": false, "shadow": null, "suppressFullscreen": true},
         "field": "class", "ignoreCase": true, "name": "StarCraft II client, by class",
         "patterns": [{"prefix": "sc2"}, {"prefix": "starcraft"}, {"holds": "sc2", "prefix": "steam_app_"}]},
        {"effects": {"border": null, "center": false, "float": false, "keepAspectRatio": false, "pin": false, "shadow": null, "suppressFullscreen": true},
         "field": "title", "ignoreCase": true, "name": "StarCraft II client, by title",
         "patterns": [{"prefix": "StarCraft II"}]}
      ]
    }"#;

    /// The host's rules from [`HOST_GRANT`].
    pub(crate) fn host_rules() -> Vec<ExtraRule> {
        Extras::from_json(HOST_GRANT).rules
    }

    #[test]
    fn the_grant_carries_the_hosts_launchers_and_rules() {
        let extras = Extras::from_json(HOST_GRANT);
        let commands: Vec<&str> = extras
            .binds
            .iter()
            .map(|bind| bind.command.as_str())
            .collect();
        assert_eq!(
            commands,
            ["coder-deck-open", "coder-zoom", "coder-battlenet"]
        );
        assert_eq!(extras.binds[0].mods, coder_binds::Mods::SUPER_SHIFT);
        assert_eq!(extras.binds[0].key, coder_binds::Key::Char('d'));
        assert_eq!(extras.rules.len(), 3);
        assert_eq!(extras.rules[1].case, Case::Any);
        assert_eq!(
            extras.rules[1].patterns[2],
            ExtraPattern::PrefixHolding {
                prefix: "steam_app_".to_string(),
                holds: "sc2".to_string()
            }
        );
        assert_eq!(extras.rules[2].field, Field::Title);
        assert_eq!(
            extras.rules[1].hyprland(),
            "windowrule = match:class (?i)^(sc2.*|starcraft.*|steam_app_.*sc2.*)$, tile on, suppress_event maximize fullscreen"
        );
    }

    #[test]
    fn a_grant_without_entries_or_with_bad_ones_adds_only_what_parses() {
        assert_eq!(Extras::from_json("{}"), Extras::default());
        assert_eq!(Extras::from_json("not json"), Extras::default());
        let extras = Extras::from_json(
            r#"{
              "extraBinds": [
                {"mods": "HYPER", "key": "G", "command": "x"},
                {"key": "G", "command": "coder-battlenet"}
              ],
              "extraRules": [
                {"field": "class", "patterns": [{"exact": "a", "prefix": "b"}], "effects": {}},
                {"field": "window", "patterns": [{"exact": "a"}], "effects": {}},
                {"field": "title", "patterns": [{"exact": "a"}], "effects": {"float": true, "border": 0}}
              ]
            }"#,
        );
        assert_eq!(extras.binds.len(), 1);
        assert_eq!(extras.binds[0].mods, coder_binds::Mods::SUPER);
        assert_eq!(extras.rules.len(), 1);
        assert_eq!(extras.rules[0].effects.float, Some(true));
        assert_eq!(extras.rules[0].effects.border, Some(0));
    }
}
