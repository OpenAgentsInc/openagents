//! OpenCode's own model picker, for /models on an OpenCode chat (#11556).
//!
//! Ported from OpenCode's TUI model dialog (`packages/tui/src/component/
//! dialog-model.tsx`, MIT, github.com/anomalyco/opencode): the models
//! this computer's OpenCode reaches (`opencode models --verbose`, which
//! lists only providers the person configured or that are free), its
//! favorites and recent models from its own state (`model.json`), then
//! every other model grouped by provider, OpenCode's own providers first,
//! with free models marked "Free" and newer models first. Search matches
//! the typed words fuzzily in the name, the provider or the id.
//!
//! One change from OpenCode's dialog, asked for by the owner: the free
//! models lead under a "Free" heading: Coder's free OpenCode model (Cat
//! Jam), its fallback (Big Pickle), then the rest newest first. Free
//! is read from OpenCode's prices (every input price zero, as OpenCode's
//! `free()`), and only from a `-free` id when no price is listed. Models
//! that can't write text (image, video and music models) are left out:
//! they can't answer a chat turn.

use std::path::PathBuf;
use std::sync::Mutex;

use serde_json::Value;

/// One model OpenCode lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    pub provider: String,
    pub id: String,
    pub name: String,
    pub free: bool,
    /// `YYYY-MM-DD`, or empty.
    pub released: String,
    pub deprecated: bool,
    pub writes_text: bool,
}

impl Listed {
    /// `provider/id`, as OpenCode and Coder name the model.
    #[must_use]
    pub fn key(&self) -> String {
        format!("{}/{}", self.provider, self.id)
    }

    fn from_id(key: &str) -> Option<Self> {
        let (provider, id) = key.split_once('/')?;
        Some(Self {
            provider: provider.to_owned(),
            id: id.to_owned(),
            name: key.to_owned(),
            free: id.ends_with("-free"),
            released: String::new(),
            deprecated: false,
            writes_text: true,
        })
    }
}

/// `opencode models --verbose`: each model's `provider/id` line, then its
/// metadata as JSON. A model whose JSON doesn't read is kept by its id.
#[must_use]
pub fn parse_verbose(text: &str) -> Vec<Listed> {
    let mut blocks: Vec<(String, String)> = Vec::new();
    for line in text.lines() {
        let starts_block = line
            .chars()
            .next()
            .is_some_and(|first| !first.is_whitespace() && first != '{' && first != '}');
        if starts_block {
            blocks.push((line.trim().to_owned(), String::new()));
        } else if let Some((_, json)) = blocks.last_mut() {
            json.push_str(line);
            json.push('\n');
        }
    }
    blocks
        .into_iter()
        .filter_map(|(key, json)| {
            let mut listed = Listed::from_id(&key)?;
            if let Ok(value) = serde_json::from_str::<Value>(&json) {
                if let Some(name) = value["name"].as_str() {
                    listed.name = name.to_owned();
                }
                // OpenCode's `free()`: a listed price whose input is zero.
                // With no price listed, the `-free` id says it.
                match &value["cost"] {
                    Value::Object(cost) => {
                        listed.free = cost.get("input").and_then(Value::as_f64) == Some(0.0);
                    }
                    Value::Array(costs) if !costs.is_empty() => {
                        listed.free = costs.iter().all(|cost| cost["input"].as_f64() == Some(0.0));
                    }
                    _ => {}
                }
                listed.released = value["release_date"].as_str().unwrap_or_default().into();
                listed.deprecated = value["status"].as_str() == Some("deprecated");
                listed.writes_text = value["capabilities"]["output"]["text"]
                    .as_bool()
                    .unwrap_or(true);
            }
            Some(listed)
        })
        .collect()
}

/// `opencode models`: one `provider/id` per line.
#[must_use]
pub fn parse_plain(text: &str) -> Vec<Listed> {
    text.lines()
        .map(str::trim)
        .filter(|line| acp_client::opencode::Model::parse(line).is_ok())
        .filter_map(Listed::from_id)
        .collect()
}

/// OpenCode's favorites and recent models, from its `model.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Preferences {
    pub favorite: Vec<String>,
    pub recent: Vec<String>,
}

impl Preferences {
    /// Reads OpenCode's `model.json` text; anything unreadable is empty.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        let value: Value = serde_json::from_str(text).unwrap_or_default();
        let list = |name: &str| {
            value[name]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|item| {
                    Some(format!(
                        "{}/{}",
                        item["providerID"].as_str().filter(|s| !s.is_empty())?,
                        item["modelID"].as_str().filter(|s| !s.is_empty())?
                    ))
                })
                .collect()
        };
        Self {
            favorite: list("favorite"),
            recent: list("recent"),
        }
    }

    /// OpenCode's own `model.json` under its state folder
    /// (`$XDG_STATE_HOME/opencode`, else `~/.local/state/opencode`). Only
    /// read, never written: OpenCode guards it with its own lock.
    #[must_use]
    pub fn read() -> Self {
        if cfg!(test) {
            return Self::default();
        }
        let state = std::env::var_os("XDG_STATE_HOME")
            .filter(|dir| !dir.is_empty())
            .map(PathBuf::from)
            .or_else(|| user_home::home_os().map(|home| PathBuf::from(home).join(".local/state")));
        state
            .and_then(|dir| std::fs::read_to_string(dir.join("opencode/model.json")).ok())
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }
}

/// A provider's name as OpenCode's dialog heads its group.
#[must_use]
pub fn provider_name(provider: &str) -> String {
    match provider {
        "opencode" => "OpenCode Zen".into(),
        "opencode-go" => "OpenCode Go".into(),
        "openai" => "OpenAI".into(),
        "xai" => "xAI".into(),
        "openrouter" => "OpenRouter".into(),
        "github-copilot" => "GitHub Copilot".into(),
        other => {
            let mut chars = other.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect()
            })
        }
    }
}

/// The free heading.
pub const FREE: &str = "Free";

/// The rows of OpenCode's picker in order, each with its heading: Free,
/// then Favorites, Recent, then each provider (see the module notes). A
/// model shows once, in the first group it belongs to.
#[must_use]
pub fn arrange(models: &[Listed], preferences: &Preferences) -> Vec<(String, Listed)> {
    let usable: Vec<&Listed> = models
        .iter()
        .filter(|model| !model.deprecated && model.writes_text)
        .collect();
    let mut rows: Vec<(String, Listed)> = Vec::new();
    let mut shown = std::collections::BTreeSet::new();
    // Free: Coder's free OpenCode model (Cat Jam), its fallback (Big
    // Pickle), then the rest newest first.
    let mut free: Vec<&Listed> = usable.iter().copied().filter(|model| model.free).collect();
    let lead = |model: &Listed| {
        let key = model.key();
        if key == crate::opencode_chat::DEFAULT_MODEL {
            0
        } else if key == coder_delegate::delegate::OPENCODE_AUTO_FALLBACK {
            1
        } else {
            2
        }
    };
    free.sort_by(|a, b| {
        lead(a)
            .cmp(&lead(b))
            .then_with(|| b.released.cmp(&a.released))
            .then_with(|| a.name.cmp(&b.name))
    });
    for model in free {
        shown.insert(model.key());
        rows.push((FREE.into(), model.clone()));
    }
    for (heading, keys) in [
        ("Favorites", &preferences.favorite),
        ("Recent", &preferences.recent),
    ] {
        for key in keys {
            if shown.contains(key) {
                continue;
            }
            if let Some(model) = usable.iter().find(|model| model.key() == *key) {
                shown.insert(key.clone());
                rows.push((heading.into(), (*model).clone()));
            }
        }
    }
    // OpenCode's `sortModelOptions`: opencode-go, then opencode, then by
    // provider name; within a provider free first, newer first, then name.
    let mut rest: Vec<&Listed> = usable
        .iter()
        .copied()
        .filter(|model| !shown.contains(&model.key()))
        .collect();
    let rank = |model: &Listed| match model.provider.as_str() {
        "opencode-go" => 0,
        "opencode" => 1,
        _ => 2,
    };
    rest.sort_by(|a, b| {
        rank(a)
            .cmp(&rank(b))
            .then_with(|| provider_name(&a.provider).cmp(&provider_name(&b.provider)))
            .then_with(|| b.free.cmp(&a.free))
            .then_with(|| b.released.cmp(&a.released))
            .then_with(|| a.name.cmp(&b.name))
    });
    for model in rest {
        rows.push((provider_name(&model.provider), model.clone()));
    }
    rows
}

/// What this run read of OpenCode's models.
static LISTED: Mutex<Option<Vec<Listed>>> = Mutex::new(None);

/// Keeps `models` as OpenCode's list (a read, or a test's fixture).
pub fn keep(models: Vec<Listed>) {
    *LISTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(models);
}

fn listed() -> Option<Vec<Listed>> {
    LISTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// OpenCode's own name for `key` (`provider/id`), once its list is read.
#[must_use]
pub fn name(key: &str) -> Option<String> {
    listed()?
        .into_iter()
        .find(|model| model.key() == key && model.name != key)
        .map(|model| model.name)
}

/// Reads OpenCode's list now: `opencode models --verbose`, else the plain
/// list. Never in a unit test.
fn read() -> Vec<Listed> {
    if cfg!(test) {
        return Vec::new();
    }
    let Some(program) = crate::opencode_chat::binary() else {
        return Vec::new();
    };
    let run = |verbose: bool| {
        let mut command = acp_client::process::command(&program);
        acp_client::process::non_interactive(&mut command);
        command.arg("models");
        if verbose {
            command.arg("--verbose");
        }
        command
            .stdin(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .output()
            .ok()
            .filter(|output| output.status.success())
            .map(|output| String::from_utf8_lossy(&output.stdout).into_owned())
    };
    let models = run(true)
        .map(|text| parse_verbose(&text))
        .unwrap_or_default();
    if models.is_empty() {
        run(false)
            .map(|text| parse_plain(&text))
            .unwrap_or_default()
    } else {
        models
    }
}

/// Reads OpenCode's list on a thread of its own, when it isn't read yet,
/// so /models opens at once.
pub fn warm() {
    if cfg!(test) || listed().is_some() {
        return;
    }
    std::thread::spawn(|| {
        if listed().is_none() {
            let models = read();
            if !models.is_empty() {
                keep(models);
            }
        }
    });
}

/// /models on an OpenCode chat: OpenCode's picker rows as Coder's models,
/// read now when not read yet. `current` is always listed.
#[must_use]
pub fn catalog(current: &str) -> Vec<crate::models::Model> {
    let models = listed().unwrap_or_else(|| {
        let models = read();
        if !models.is_empty() {
            keep(models.clone());
        }
        models
    });
    let mut rows = arrange(&models, &Preferences::read());
    if !rows.iter().any(|(_, model)| model.key() == current)
        && let Some(model) = Listed::from_id(current)
    {
        rows.insert(0, ("Current".into(), model));
    }
    rows.into_iter()
        .map(|(group, model)| crate::models::Model {
            plugin: crate::opencode_chat::PLUGIN.into(),
            provider: provider_name(&model.provider),
            description: if model.free {
                FREE.into()
            } else if matches!(group.as_str(), "Favorites" | "Recent") {
                provider_name(&model.provider)
            } else {
                String::new()
            },
            id: model.key(),
            name: model.name,
            context_length: None,
            max_output_tokens: None,
            efforts: Vec::new(),
            default_effort: None,
            supports_output_limit: false,
            group: Some(group),
        })
        .collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn model(key: &str, name: &str, cost: &str, released: &str) -> String {
        format!(
            "{key}\n{{\n  \"name\": \"{name}\",\n  \"status\": \"active\",\n  \"cost\": {cost},\n  \"capabilities\": {{\"output\": {{\"text\": true}}}},\n  \"release_date\": \"{released}\"\n}}\n"
        )
    }

    /// A small `opencode models --verbose`, as OpenCode 1.18 prints it.
    pub(crate) fn fixture() -> String {
        let free = r#"{"input": 0, "output": 0}"#;
        let paid = r#"{"input": 3, "output": 15}"#;
        [
            model("opencode/big-pickle", "Big Pickle", free, "2025-10-17"),
            model("opencode/cat-jam-free", "Cat Jam Free", free, "2026-10-12"),
            model("opencode/claude-opus-5-5", "Claude Opus 5.5", paid, "2026-09-01"),
            model("opencode/longcat-2.5-preview-free", "LongCat 2.5 Preview Free", free, "2026-09-25"),
            model("opencode/exo-free", "Exo Free", free, "2026-10-06"),
            model("cursor/composer-2.5", "Composer 2.5", paid, "2026-09-20"),
            model("google/gemini-3.8-flash", "Gemini 3.8 Flash", paid, "2026-10-01"),
            "google/veo-3.1-generate-preview\n{\"name\": \"Veo 3.1\", \"cost\": {\"input\": 0}, \"capabilities\": {\"output\": {\"text\": false}}}\n".into(),
        ]
        .concat()
    }

    #[test]
    fn free_models_lead_cat_jam_then_big_pickle_then_the_rest() {
        let rows = arrange(&parse_verbose(&fixture()), &Preferences::default());
        let free: Vec<_> = rows
            .iter()
            .filter(|(group, _)| group == FREE)
            .map(|(_, model)| model.key())
            .collect();
        assert_eq!(
            free,
            [
                "opencode/cat-jam-free",
                "opencode/big-pickle",
                "opencode/exo-free",
                "opencode/longcat-2.5-preview-free"
            ]
        );
        // Then OpenCode's own provider, then the rest by name; a video
        // model is never offered.
        let rest: Vec<_> = rows
            .iter()
            .filter(|(group, _)| group != FREE)
            .map(|(group, model)| (group.as_str(), model.key()))
            .collect();
        assert_eq!(
            rest,
            [
                ("OpenCode Zen", "opencode/claude-opus-5-5".to_owned()),
                ("Cursor", "cursor/composer-2.5".to_owned()),
                ("Google", "google/gemini-3.8-flash".to_owned()),
            ]
        );
    }

    #[test]
    fn free_is_read_from_the_price_and_from_the_id_only_without_one() {
        let models = parse_verbose(
            "opencode/paid-free\n{\"cost\": {\"input\": 2}}\nopencode/no-price-free\n{\"name\": \"N\"}\n",
        );
        assert!(!models[0].free, "a price says it isn't free");
        assert!(models[1].free, "no price: the -free id says it");
    }

    #[test]
    fn recent_models_follow_the_free_ones() {
        let preferences = Preferences::parse(
            r#"{"recent":[{"providerID":"google","modelID":"gemini-3.8-flash"},{"providerID":"opencode","modelID":"big-pickle"}],"favorite":[]}"#,
        );
        let rows = arrange(&parse_verbose(&fixture()), &preferences);
        let recent: Vec<_> = rows
            .iter()
            .filter(|(group, _)| group == "Recent")
            .map(|(_, model)| model.key())
            .collect();
        // Big Pickle already shows under Free.
        assert_eq!(recent, ["google/gemini-3.8-flash"]);
    }
}
