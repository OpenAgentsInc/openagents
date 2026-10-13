//! The models an ACP agent offers, as its own CLI lists them (#11556).
//!
//! Devin CLI lists its models by family with `devin models list`, and
//! Grok Build lists its own with `grok models`, naming the default. /models
//! shows those lists for the agent the chat is on, the rail names the
//! model by the agent's own display name ("Devin CLI (SWE-2 Medium)",
//! "Grok Build (Grok 4.7)"), and a turn runs the person's pick, else
//! Coder's default for the agent: [`acp_client::devin::CODER_MODEL`] for
//! Devin, and the default `grok models` names for Grok (read before the
//! turn, which also refreshes Grok's login so the session never starts on
//! its built-in fallback model).

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Mutex;

/// Devin CLI's agent id.
pub const DEVIN: &str = "devin-cli";
/// Grok Build's agent id.
pub const GROK: &str = "grok-build";
/// Oh My Pi's agent id.
pub const OMP: &str = "oh-my-pi";

/// The plugin name /models rows of an agent's own list carry:
/// `agent:<id>`.
#[must_use]
pub fn picker(agent: &str) -> String {
    format!("agent:{agent}")
}

/// One model an agent's CLI lists.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentModel {
    pub id: String,
    /// The agent's own name for it ("SWE-2 Medium").
    pub name: String,
    /// The family heading it sits under ("SWE-2"), when the agent groups.
    pub family: Option<String>,
    /// Whether the agent names it its default.
    pub default: bool,
}

/// Whether Coder lists `agent`'s own models in /models.
#[must_use]
pub fn lists_models(agent: &str) -> bool {
    matches!(agent, DEVIN | GROK | OMP)
}

/// Whether `model` is one of the Clef decision models (`clef:latest`,
/// `clef-flash:latest`, on any provider): they judge, they never chat, so
/// no harness may run a chat turn on one (#11556).
#[must_use]
pub fn is_clef(model: &str) -> bool {
    let name = model
        .rsplit('/')
        .next()
        .unwrap_or(model)
        .to_ascii_lowercase();
    name == "clef" || name.starts_with("clef:") || name.starts_with("clef-")
}

/// `omp models --json`: `{"models":[{"provider","id","selector","name"}]}`.
/// The Clef decision models are never listed.
#[must_use]
pub fn parse_omp(text: &str) -> Vec<AgentModel> {
    let value: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
    value["models"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|model| {
            let provider = model["provider"].as_str().unwrap_or_default();
            let id = model["id"].as_str()?;
            let selector = model["selector"]
                .as_str()
                .map_or_else(|| format!("{provider}/{id}"), str::to_owned);
            (!is_clef(&selector)).then(|| AgentModel {
                name: model["name"].as_str().unwrap_or(id).to_owned(),
                family: (!provider.is_empty()).then(|| provider.to_owned()),
                default: false,
                id: selector,
            })
        })
        .collect()
}

/// Whether a local Ollama model can chat: its `/api/show` capabilities
/// name `completion`. A model Ollama doesn't answer for is left out.
#[cfg(not(test))]
fn ollama_chats(model: &str) -> bool {
    use std::io::{Read, Write};
    let name = model.strip_prefix("ollama/").unwrap_or(model);
    let Ok(mut stream) = std::net::TcpStream::connect_timeout(
        &std::net::SocketAddr::from(([127, 0, 0, 1], 11434)),
        std::time::Duration::from_millis(500),
    ) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(3)));
    let body = serde_json::json!({ "model": name }).to_string();
    let request = format!(
        "POST /api/show HTTP/1.1\r\nHost: 127.0.0.1\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    if stream.write_all(request.as_bytes()).is_err() {
        return false;
    }
    let mut reply = String::new();
    let _ = stream.read_to_string(&mut reply);
    let json = reply.split_once("\r\n\r\n").map_or("", |(_, body)| body);
    // A chunked reply carries the JSON between its size lines.
    let json = json
        .find('{')
        .and_then(|start| json.rfind('}').map(|end| &json[start..=end]))
        .unwrap_or_default();
    serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|value| value["capabilities"].as_array().cloned())
        .is_some_and(|capabilities| {
            capabilities
                .iter()
                .any(|capability| capability.as_str() == Some("completion"))
        })
}

#[cfg(test)]
fn ollama_chats(_model: &str) -> bool {
    false
}

/// The reply text of an agent that turned its model's failure into words
/// of its answer ("400 \"clef:latest\" does not support chat ...
/// raw-http-request=...") instead of failing the turn.
#[must_use]
pub fn raw_failure(text: &str) -> bool {
    let text = text.trim();
    if text.contains("raw-http-request=") {
        return true;
    }
    let mut words = text.split_whitespace();
    let status = words.next().unwrap_or_default();
    // "400 \"…\"" or "500 {…}": an HTTP status and the provider's body.
    status.len() == 3
        && status.starts_with(['4', '5'])
        && status.chars().all(|c| c.is_ascii_digit())
        && words
            .next()
            .is_some_and(|body| body.starts_with(['"', '{']))
        && text.len() < 4_000
}

/// The raw words of the last model failure an agent put in its reply,
/// for the routing record; never shown in the chat.
static RAW_FAILURE: Mutex<Option<String>> = Mutex::new(None);

/// Keeps `text` as the last raw failure.
pub fn keep_raw_failure(text: &str) {
    *RAW_FAILURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = Some(text.to_owned());
}

/// Takes the last raw failure, once.
#[must_use]
pub fn take_raw_failure() -> Option<String> {
    RAW_FAILURE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take()
}

/// The plain line a model failure reads as in the chat.
#[must_use]
pub fn failure_line(name: &str, text: &str) -> String {
    if text.contains("does not support") {
        format!("{name} couldn't answer: its model can't chat. Pick another with /models.")
    } else {
        format!("{name} couldn't answer: its model refused the request. Pick another with /models.")
    }
}

/// `devin models list`: a family heading `Name (slug)` at the left edge,
/// then each model indented as `id   Display Name  [prices]`; `aliases:`
/// lines are skipped.
#[must_use]
pub fn parse_devin(text: &str) -> Vec<AgentModel> {
    let mut family: Option<String> = None;
    let mut models = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() || line.starts_with("Available models") {
            continue;
        }
        if !line.starts_with(' ') {
            // "SWE-2 (swe-2)" names the family "SWE-2".
            family = Some(
                line.rsplit_once(" (")
                    .map_or(line, |(name, _)| name)
                    .trim()
                    .to_owned(),
            );
            continue;
        }
        let trimmed = line.trim();
        if trimmed.starts_with("aliases:") {
            continue;
        }
        let Some((id, rest)) = trimmed.split_once(char::is_whitespace) else {
            continue;
        };
        let name = rest.trim();
        let name = name.split_once("  [").map_or(name, |(name, _)| name).trim();
        if id.is_empty() || name.is_empty() {
            continue;
        }
        models.push(AgentModel {
            id: id.to_owned(),
            name: name.to_owned(),
            family: family.clone(),
            default: false,
        });
    }
    models
}

/// `grok models`: `Default model: ID`, then `  * ID (default)` and
/// `  - ID` rows.
#[must_use]
pub fn parse_grok(text: &str) -> Vec<AgentModel> {
    let named_default = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("Default model:"))
        .map(|id| id.trim().to_owned());
    let mut models = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        let Some(rest) = trimmed
            .strip_prefix("* ")
            .or_else(|| trimmed.strip_prefix("- "))
        else {
            continue;
        };
        let marked = rest.ends_with("(default)");
        let id = rest.trim_end_matches("(default)").trim();
        if id.is_empty() || id.contains(char::is_whitespace) {
            continue;
        }
        models.push(AgentModel {
            id: id.to_owned(),
            name: crate::ui::rail_model_name(id),
            family: None,
            default: marked || named_default.as_deref() == Some(id),
        });
    }
    models
}

/// The lists read this run, by agent id.
static LISTED: Mutex<BTreeMap<String, Vec<AgentModel>>> = Mutex::new(BTreeMap::new());

/// The person's picks this run, by agent id; loaded from and saved to
/// `agent-models.json` in Coder's data folder outside tests.
static PICKS: Mutex<Option<BTreeMap<String, String>>> = Mutex::new(None);

const PICKS_FILE: &str = "agent-models.json";

/// What `agent`'s CLI listed, once read this run.
#[must_use]
pub fn listed(agent: &str) -> Option<Vec<AgentModel>> {
    LISTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(agent)
        .cloned()
}

/// Keeps `models` as `agent`'s list (a read, or a test's fixture).
pub fn keep(agent: &str, models: Vec<AgentModel>) {
    if models.is_empty() {
        return;
    }
    LISTED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(agent.to_owned(), models);
}

/// Reads `agent`'s list from `program` now: `devin models list` or
/// `grok models`. Never in a unit test.
pub fn read(agent: &str, program: &Path) -> Option<Vec<AgentModel>> {
    if cfg!(test) {
        return listed(agent);
    }
    let arguments: &[&str] = match agent {
        DEVIN => &["models", "list"],
        GROK => &["models"],
        OMP => &["models", "--json"],
        _ => return None,
    };
    let output = std::process::Command::new(program)
        .args(arguments)
        .stdin(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let text = String::from_utf8_lossy(&output.stdout);
    let models = match agent {
        DEVIN => parse_devin(&text),
        // Oh My Pi lists every model it has auth for; a local Ollama one
        // stays only when it can chat.
        OMP => parse_omp(&text)
            .into_iter()
            .filter(|model| !model.id.starts_with("ollama/") || ollama_chats(&model.id))
            .collect(),
        _ => parse_grok(&text),
    };
    keep(agent, models.clone());
    (!models.is_empty()).then_some(models)
}

/// [`read`] on a thread of its own, when the list isn't read yet.
pub fn warm(agent: &str, program: &Path) {
    if !lists_models(agent) || listed(agent).is_some() || cfg!(test) {
        return;
    }
    let agent = agent.to_owned();
    let program = program.to_owned();
    std::thread::spawn(move || {
        let _ = read(&agent, &program);
    });
}

fn picks() -> std::sync::MutexGuard<'static, Option<BTreeMap<String, String>>> {
    let mut held = PICKS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if held.is_none() {
        let saved = if cfg!(test) {
            None
        } else {
            crate::home::data_dir()
                .and_then(|dir| std::fs::read(dir.join(PICKS_FILE)).ok())
                .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        };
        *held = Some(saved.unwrap_or_default());
    }
    held
}

/// The model the person picked for `agent` in /models.
#[must_use]
pub fn choice(agent: &str) -> Option<String> {
    picks().as_ref().and_then(|picks| picks.get(agent).cloned())
}

/// Makes `model` the pick for `agent` and saves it.
///
/// # Errors
/// The pick could not be saved.
pub fn choose(agent: &str, model: &str) -> Result<(), String> {
    let mut held = picks();
    let picks = held.get_or_insert_with(BTreeMap::new);
    picks.insert(agent.to_owned(), model.to_owned());
    if cfg!(test) {
        return Ok(());
    }
    let dir = crate::home::data_dir().ok_or("Coder's settings folder is unavailable.")?;
    std::fs::create_dir_all(&dir).map_err(|_| "Coder could not save the pick.".to_owned())?;
    let bytes = serde_json::to_vec_pretty(&*picks).map_err(|error| error.to_string())?;
    std::fs::write(dir.join(PICKS_FILE), bytes)
        .map_err(|_| "Coder could not save the pick.".to_owned())
}

/// A new chat starts on each agent's default, not an earlier chat's pick
/// (#11556).
pub fn reset_choices() {
    if let Some(picks) = picks().as_mut() {
        picks.clear();
    }
}

/// Forgets a test's pick.
#[cfg(test)]
pub fn forget(agent: &str) {
    if let Some(picks) = picks().as_mut() {
        picks.remove(agent);
    }
}

/// The model `agent` runs when the person picked none: Devin CLI's is
/// Coder's SWE-2 default, Grok Build's the default its CLI names.
#[must_use]
pub fn default_model(agent: &str) -> Option<String> {
    match agent {
        DEVIN => Some(acp_client::devin::CODER_MODEL.to_owned()),
        GROK => listed(GROK)
            .and_then(|models| models.into_iter().find(|model| model.default))
            .map(|model| model.id),
        // Oh My Pi's first model that can chat; never a Clef model.
        OMP => listed(OMP)
            .and_then(|models| models.into_iter().find(|model| !is_clef(&model.id)))
            .map(|model| model.id),
        _ => None,
    }
}

/// The model `agent`'s next turn runs: the pick, else [`default_model`].
#[must_use]
pub fn turn_model(agent: &str) -> Option<String> {
    choice(agent).or_else(|| default_model(agent))
}

/// [`turn_model`] for a turn about to start: Grok's list is read first
/// when it isn't yet, so its default is known.
#[must_use]
pub fn turn_model_reading(agent: &str, program: &Path) -> Option<String> {
    if matches!(agent, GROK | OMP) && choice(agent).is_none() && listed(agent).is_none() {
        let _ = read(agent, program);
    }
    turn_model(agent)
}

/// What a person reads for `model` on `agent`: the agent's own name for
/// it when its list has it, else the model's words ("composer-2.5[fast=true]"
/// reads "Composer 2.5 Fast").
#[must_use]
pub fn display(agent: &str, model: &str) -> String {
    if let Some(found) = listed(agent)
        .unwrap_or_default()
        .into_iter()
        .find(|known| known.id == model)
    {
        return found.name;
    }
    if agent == DEVIN
        && let Some(name) = devin_name(model)
    {
        return name;
    }
    crate::ui::rail_model_name(model)
}

/// Devin's own names for its SWE models, before its list is read:
/// `swe-2-medium` reads "SWE-2 Medium", `swe-1-6-slow` "SWE-1.6 Slow".
fn devin_name(model: &str) -> Option<String> {
    let rest = model.strip_prefix("swe-")?;
    let mut parts = rest.split('-');
    let major = parts.next()?;
    let mut version = major.to_owned();
    let mut words = Vec::new();
    for part in parts {
        if words.is_empty() && part.chars().all(|c| c.is_ascii_digit()) {
            version.push('.');
            version.push_str(part);
        } else {
            let mut chars = part.chars();
            words.push(
                chars
                    .next()
                    .map(|first| first.to_uppercase().chain(chars).collect::<String>())
                    .unwrap_or_default(),
            );
        }
    }
    let mut name = format!("SWE-{version}");
    for word in words {
        name.push(' ');
        name.push_str(&word);
    }
    Some(name)
}

/// /models for `agent`: its own list in its own order, Devin's grouped by
/// family with SWE-2 first, Grok's default marked.
#[must_use]
pub fn catalog(agent: &str) -> Vec<crate::models::Model> {
    let mut models = listed(agent).unwrap_or_default();
    if agent == DEVIN {
        // SWE-2 leads (owner, 2026-10-13); the rest keep Devin's order.
        models.sort_by_key(|model| u8::from(model.family.as_deref() != Some("SWE-2")));
    }
    models
        .into_iter()
        .map(|model| crate::models::Model {
            plugin: picker(agent),
            provider: match agent {
                DEVIN => "Devin CLI".into(),
                OMP => model.family.clone().unwrap_or_default(),
                _ => "xAI".into(),
            },
            description: if model.default {
                "Default".into()
            } else {
                String::new()
            },
            name: model.name,
            id: model.id,
            context_length: None,
            max_output_tokens: None,
            efforts: Vec::new(),
            default_effort: None,
            supports_output_limit: false,
            group: model.family,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const DEVIN_LIST: &str = "Available models (3 families)

Adaptive (adaptive)
  adaptive                                       Adaptive  [$0.5 / 1M Input]

SWE-2 (swe-2)
  aliases: swe
  swe-2-high                                     SWE-2 High
  swe-2-medium                                   SWE-2 Medium
  swe-2-max                                      SWE-2 Max

SWE-1.6 Slow (swe-1.6-slow)
  swe-1-6-slow                                   SWE-1.6 Slow  [200K context, $0.5 / 1M Input]
";

    pub(crate) const GROK_LIST: &str = "You are logged in with grok.com.

Default model: grok-4.7

Available models:
  * grok-4.7 (default)
  - grok-4.7-build-fast
  - grok-4.6
  - grok-4.5
";

    #[test]
    fn devins_list_reads_by_family_with_its_own_names() {
        let models = parse_devin(DEVIN_LIST);
        assert_eq!(models.len(), 5);
        assert_eq!(models[1].id, "swe-2-high");
        assert_eq!(models[1].name, "SWE-2 High");
        assert_eq!(models[1].family.as_deref(), Some("SWE-2"));
        assert_eq!(models[4].name, "SWE-1.6 Slow");
        assert_eq!(models[0].name, "Adaptive");
    }

    #[test]
    fn grok_names_its_default() {
        let models = parse_grok(GROK_LIST);
        let ids: Vec<_> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(
            ids,
            ["grok-4.7", "grok-4.7-build-fast", "grok-4.6", "grok-4.5"]
        );
        assert!(models[0].default && !models[2].default);
        assert_eq!(models[0].name, "Grok 4.7");
    }

    #[test]
    fn devin_defaults_to_swe_2_and_names_models_as_devin_does() {
        forget(DEVIN);
        assert_eq!(turn_model(DEVIN).as_deref(), Some("swe-2-medium"));
        assert_eq!(devin_name("swe-1-6-slow").as_deref(), Some("SWE-1.6 Slow"));
        assert_eq!(devin_name("swe-2-medium").as_deref(), Some("SWE-2 Medium"));
        assert_eq!(display(DEVIN, "swe-2-medium"), "SWE-2 Medium");
        keep(DEVIN, parse_devin(DEVIN_LIST));
        let catalog = catalog(DEVIN);
        assert_eq!(catalog[0].id, "swe-2-high");
        assert_eq!(catalog[0].group.as_deref(), Some("SWE-2"));
        assert!(
            catalog
                .iter()
                .all(|model| model.plugin == "agent:devin-cli")
        );
    }

    #[test]
    fn grok_runs_the_default_its_cli_names() {
        keep(GROK, parse_grok(GROK_LIST));
        assert_eq!(default_model(GROK).as_deref(), Some("grok-4.7"));
        assert_eq!(display(GROK, "grok-4.7"), "Grok 4.7");
    }

    #[test]
    fn oh_my_pi_never_lists_or_runs_a_clef_model() {
        let models = parse_omp(
            r#"{"models":[{"provider":"ollama","id":"clef-flash:latest","selector":"ollama/clef-flash:latest","name":"clef-flash:latest"},{"provider":"ollama","id":"clef:latest","selector":"ollama/clef:latest","name":"clef:latest"},{"provider":"openai","id":"gpt-5.2","selector":"openai/gpt-5.2","name":"GPT-5.2"}]}"#,
        );
        let ids: Vec<_> = models.iter().map(|model| model.id.as_str()).collect();
        assert_eq!(ids, ["openai/gpt-5.2"]);
        keep(OMP, models);
        assert_eq!(default_model(OMP).as_deref(), Some("openai/gpt-5.2"));
        assert!(is_clef("ollama/clef:latest") && is_clef("clef-flash:latest"));
        assert!(!is_clef("ollama/llama3:latest"));
    }

    #[test]
    fn a_model_failure_in_the_reply_reads_as_one_plain_line() {
        let raw = "400 \"clef:latest\" does not support chat\nraw-http-request=/Users/x/.omp/logs/http-400-requests/a.json";
        assert!(raw_failure(raw));
        assert!(!raw_failure("Yes, I'm here."));
        assert!(!raw_failure("404 lines changed in the error handler"));
        let line = failure_line("Oh My Pi", raw);
        assert_eq!(
            line,
            "Oh My Pi couldn't answer: its model can't chat. Pick another with /models."
        );
        assert!(!line.contains("raw-http-request") && !line.contains(".omp"));
    }

    #[test]
    fn a_cursor_slug_reads_as_words() {
        assert_eq!(
            display("cursor", "composer-2.5[fast=true]"),
            "Composer 2.5 Fast"
        );
    }
}
