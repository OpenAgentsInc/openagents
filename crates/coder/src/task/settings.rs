//! Local capability settings: what Coder may use on this computer when the
//! person at it asks for coding (`coder::task::local`).
//!
//! One file, read by `openagents chat`, the desktop, and a host on this
//! computer: `$OPENAGENTS_SETTINGS`, else `~/.openagents/settings.json`
//! ([`path`]). A missing file, or a missing field, means the default, and
//! the defaults are exactly what a computer does with no file at all
//! (#10032, #10045, #10091): Codex, then Claude Code, then Grok Build (each
//! only when signed in here), a coding request runs at once,
//! a fresh usage reading at or above 90% passes a provider over, any Git
//! checkout is a project, and commands run in the filesystem boundary with
//! this computer's toolchains.
//!
//! ```json
//! {
//!   "schema": "openagents.settings.v1",
//!   "coder": {
//!     "providers": ["codex", "claude", "grok"],
//!     "start": "at_once",
//!     "usage_threshold_percent": 90,
//!     "projects": [],
//!     "access": "toolchains"
//!   }
//! }
//! ```
//!
//! OpenCode and Devin are never on by default: OpenCode has no default
//! model (it names its own `provider/model`), and Devin bills a paid API
//! per run, so each runs only when the person names it.
//!
//! A file that does not parse, or names a value outside its closed set, is
//! never read as the defaults: a local run then refuses and names the file,
//! so a broken opt-out never quietly becomes an opt-in. Other top-level
//! sections are kept as they are when the file is saved, so other settings
//! (the desktop's, #10021) can live beside these.
//!
//! [`Settings`] is the typed API; [`keys`], [`Settings::get`],
//! [`Settings::set`], and [`Settings::unset`] are the flat `coder.*` keys
//! `openagents settings` and a settings screen edit.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::adapter::Access;
use super::autostart::Route;
use super::capacity::Provider;
use super::usage;

/// The file's schema.
pub const SCHEMA: &str = "openagents.settings.v1";
/// Names another settings file than `~/.openagents/settings.json`.
pub const PATH_VAR: &str = "OPENAGENTS_SETTINGS";
/// The most routes a local run admits: a first route and its fallbacks.
pub const MAX_PROVIDERS: usize = 1 + super::adapter::MAX_FALLBACKS;
/// The providers a local run can use, in their default order.
pub const PROVIDERS: [Provider; 5] = [
    Provider::Codex,
    Provider::Claude,
    Provider::Grok,
    Provider::OpenCode,
    Provider::Devin,
];

/// The settings file: `$OPENAGENTS_SETTINGS`, else
/// `~/.openagents/settings.json`.
#[must_use]
pub fn path() -> PathBuf {
    if let Some(file) = std::env::var_os(PATH_VAR).filter(|v| !v.is_empty()) {
        return PathBuf::from(file);
    }
    std::env::var_os("HOME")
        .map_or_else(|| PathBuf::from("."), PathBuf::from)
        .join(".openagents/settings.json")
}

/// The settings in [`path`]: the defaults when there is no file.
///
/// # Errors
/// A sentence naming the file when it cannot be read or is not valid.
pub fn load() -> Result<Settings, String> {
    Settings::load(&path())
}

/// The model a provider named alone runs: the models the desktop's
/// auto-start admits for Codex and Claude Code, and Devin's and Grok
/// Build's own defaults. OpenCode has none; it names its own
/// `provider/model`.
#[must_use]
pub fn default_model(provider: Provider) -> Option<&'static str> {
    match provider {
        Provider::Codex => Some("gpt-6-luna"),
        Provider::Claude => Some("claude-opus-5-5"),
        Provider::Devin => Some(acp_client::devin::DEFAULT_MODEL),
        Provider::Grok => Some(acp_client::grok::DEFAULT_MODEL),
        Provider::OpenCode | Provider::Vertex => None,
    }
}

/// The provider that runs `engine`, the coding engine a chat's dispatch
/// offer names as the person's request (#10076).
#[must_use]
pub fn provider_of(engine: nostr::cj_conversation::Engine) -> Provider {
    use nostr::cj_conversation::Engine;
    match engine {
        Engine::Codex => Provider::Codex,
        Engine::ClaudeCode => Provider::Claude,
        Engine::GrokBuild => Provider::Grok,
        Engine::OpenCode => Provider::OpenCode,
        Engine::Devin => Provider::Devin,
    }
}

/// The engine a person names for `provider`: [`provider_of`] read back
/// (#10081). Vertex, Coder's own fallback, is no engine a person names.
#[must_use]
pub fn engine_of(provider: Provider) -> Option<nostr::cj_conversation::Engine> {
    use nostr::cj_conversation::Engine;
    match provider {
        Provider::Codex => Some(Engine::Codex),
        Provider::Claude => Some(Engine::ClaudeCode),
        Provider::Grok => Some(Engine::GrokBuild),
        Provider::OpenCode => Some(Engine::OpenCode),
        Provider::Devin => Some(Engine::Devin),
        Provider::Vertex => None,
    }
}

/// The name a person reads for `provider`.
#[must_use]
pub fn provider_name(provider: Provider) -> &'static str {
    match provider {
        Provider::Codex => "Codex",
        Provider::Claude => "Claude Code",
        Provider::OpenCode => "OpenCode",
        Provider::Devin => "Devin",
        Provider::Grok => "Grok Build",
        Provider::Vertex => "the OpenAgents cloud",
    }
}

/// One provider a local run may use, with the model it runs: `codex`, or
/// `codex:MODEL` for another model than [`default_model`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    pub provider: Provider,
    /// The model, when not the provider's default.
    pub model: Option<String>,
}

impl Choice {
    #[must_use]
    pub const fn new(provider: Provider) -> Self {
        Choice {
            provider,
            model: None,
        }
    }

    /// The route this choice admits.
    ///
    /// # Errors
    /// OpenCode named without a model.
    pub fn route(&self) -> Result<Route, String> {
        let model = match &self.model {
            Some(model) => model.clone(),
            None => default_model(self.provider)
                .ok_or_else(|| {
                    format!(
                        "{} needs a model: write it as `opencode:PROVIDER/MODEL`",
                        provider_name(self.provider)
                    )
                })?
                .to_owned(),
        };
        Ok(Route {
            provider: self.provider,
            model,
            effort: None,
        })
    }
}

impl std::fmt::Display for Choice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.model {
            Some(model) => write!(f, "{}:{model}", self.provider),
            None => f.write_str(self.provider.as_str()),
        }
    }
}

impl std::str::FromStr for Choice {
    type Err = String;

    fn from_str(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (name, model) = match text.split_once(':') {
            Some((name, model)) => (name, Some(model.trim())),
            None => (text, None),
        };
        let provider = Provider::from_config(name.trim())
            .filter(|provider| PROVIDERS.contains(provider))
            .ok_or_else(|| {
                format!("`{name}` is not a provider here: codex, claude, grok, opencode, or devin")
            })?;
        let model = match model {
            None => None,
            Some("") => return Err(format!("`{text}` names no model after the colon")),
            Some(model) => {
                if model.len() > 128
                    || !model
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '/'))
                {
                    return Err(format!("`{model}` is not a model name"));
                }
                if provider == Provider::OpenCode {
                    acp_client::opencode::Model::parse(model)
                        .map_err(|why| format!("`{model}`: {why}"))?;
                }
                if provider == Provider::Grok {
                    acp_client::grok::parse_model(model)
                        .map_err(|why| format!("`{model}`: {why}"))?;
                }
                Some(model.to_owned())
            }
        };
        let choice = Choice { provider, model };
        choice.route()?;
        Ok(choice)
    }
}

impl Serialize for Choice {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

impl<'de> Deserialize<'de> for Choice {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// Whether a coding request from a chat starts Coder at once or waits for
/// the person to accept the offer.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Start {
    /// Coder starts as soon as the router judges the message is coding
    /// work (#10032).
    #[default]
    AtOnce,
    /// The reply offers Coder; it starts only when the person accepts
    /// (`openagents chat run-coder`, or **Run Coder** in the app).
    AskFirst,
}

impl Start {
    const fn as_str(self) -> &'static str {
        match self {
            Start::AtOnce => "at_once",
            Start::AskFirst => "ask_first",
        }
    }
}

/// Codex, then Claude Code, then Grok Build (#10091): the engines that
/// need no model named and no paid API of their own. A provider that is not
/// signed in here is passed over with that reason, so allowing it costs
/// nothing. OpenCode needs a model and Devin is a paid API, so neither is a
/// default.
fn default_providers() -> Vec<Choice> {
    vec![
        Choice::new(Provider::Codex),
        Choice::new(Provider::Claude),
        Choice::new(Provider::Grok),
    ]
}

#[allow(clippy::unnecessary_wraps)]
fn default_threshold() -> Option<u8> {
    Some(usage::DEFAULT_THRESHOLD_PERCENT)
}

fn default_access() -> Access {
    Access::Toolchains
}

/// What Coder may use on this computer for a person's own runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coder {
    /// The providers a run may use, first preferred; each only when it is
    /// signed in here and has capacity. At least one, at most
    /// [`MAX_PROVIDERS`].
    #[serde(default = "default_providers")]
    pub providers: Vec<Choice>,
    /// Whether a coding request runs at once or asks first.
    #[serde(default)]
    pub start: Start,
    /// Utilization (1 to 100 percent) at or above which a fresh usage
    /// reading passes a provider over for a later one below it; `null`
    /// ignores readings, so only a recorded refusal passes one over.
    #[serde(default = "default_threshold")]
    pub usage_threshold_percent: Option<u8>,
    /// The folders whose Git checkouts count as projects, as absolute
    /// paths. Empty: any Git checkout.
    #[serde(default)]
    pub projects: Vec<PathBuf>,
    /// What a run's commands may reach: `toolchains` (the filesystem
    /// boundary with this computer's developer tools), `full` (no sandbox,
    /// as the person's own user), or `boundary` (the plain boundary).
    #[serde(default = "default_access")]
    pub access: Access,
}

impl Default for Coder {
    fn default() -> Self {
        Coder {
            providers: default_providers(),
            start: Start::default(),
            usage_threshold_percent: default_threshold(),
            projects: Vec::new(),
            access: default_access(),
        }
    }
}

impl Coder {
    /// # Errors
    /// The first value outside what a local run admits.
    pub fn validate(&self) -> Result<(), String> {
        if self.providers.is_empty() {
            return Err("coder.providers names no provider; name at least one".into());
        }
        if self.providers.len() > MAX_PROVIDERS {
            return Err(format!(
                "coder.providers names more than {MAX_PROVIDERS} providers"
            ));
        }
        let mut routes: Vec<Route> = Vec::new();
        for choice in &self.providers {
            let route = choice.route()?;
            if routes.contains(&route) {
                return Err(format!("coder.providers names {choice} twice"));
            }
            routes.push(route);
        }
        if let Some(percent) = self.usage_threshold_percent
            && !(1..=100).contains(&percent)
        {
            return Err("coder.usage_threshold_percent is 1 to 100, or null for off".into());
        }
        for folder in &self.projects {
            if !folder.is_absolute() {
                return Err(format!(
                    "coder.projects: {} is not an absolute path",
                    folder.display()
                ));
            }
        }
        Ok(())
    }

    /// The admitted routes, in preference order.
    ///
    /// # Errors
    /// As [`Coder::validate`].
    pub fn routes(&self) -> Result<Vec<Route>, String> {
        self.validate()?;
        self.providers.iter().map(Choice::route).collect()
    }

    /// The providers admitted, in preference order, each once.
    #[must_use]
    pub fn provider_list(&self) -> Vec<Provider> {
        let mut out: Vec<Provider> = Vec::new();
        for choice in &self.providers {
            if !out.contains(&choice.provider) {
                out.push(choice.provider);
            }
        }
        out
    }

    /// Whether the Git checkout whose top level is `top` counts as a
    /// project: always when no folder is named, else when it is in one.
    #[must_use]
    pub fn admits_project(&self, top: &Path) -> bool {
        if self.projects.is_empty() {
            return true;
        }
        let top = top.canonicalize().unwrap_or_else(|_| top.to_path_buf());
        self.projects.iter().any(|folder| {
            let folder = folder.canonicalize().unwrap_or_else(|_| folder.clone());
            top.starts_with(&folder)
        })
    }
}

/// The settings file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Settings {
    pub schema: String,
    #[serde(default)]
    pub coder: Coder,
    /// Other sections, kept as they are.
    #[serde(flatten)]
    pub other: Map<String, Value>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            schema: SCHEMA.into(),
            coder: Coder::default(),
            other: Map::new(),
        }
    }
}

/// The flat keys [`Settings::get`] and [`Settings::set`] take.
#[must_use]
pub const fn keys() -> [&'static str; 5] {
    [
        "coder.providers",
        "coder.start",
        "coder.usage_threshold_percent",
        "coder.projects",
        "coder.access",
    ]
}

fn unknown(key: &str) -> String {
    format!(
        "`{key}` is not a setting; the settings are {}",
        keys().join(", ")
    )
}

impl Settings {
    /// The settings in `file`: the defaults when it does not exist.
    ///
    /// # Errors
    /// A sentence naming `file` when it cannot be read or is not valid.
    pub fn load(file: &Path) -> Result<Settings, String> {
        let bytes = match std::fs::read(file) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Settings::default());
            }
            Err(_) => return Err(format!("cannot read the settings in {}", file.display())),
        };
        let settings: Settings = serde_json::from_slice(&bytes)
            .map_err(|why| format!("the settings in {} are not valid: {why}", file.display()))?;
        if settings.schema != SCHEMA {
            return Err(format!(
                "the settings in {} have schema `{}`, not `{SCHEMA}`",
                file.display(),
                settings.schema
            ));
        }
        settings
            .coder
            .validate()
            .map_err(|why| format!("the settings in {} are not valid: {why}", file.display()))?;
        Ok(settings)
    }

    /// Write these settings to `file`, readable only by this user.
    ///
    /// # Errors
    /// They are not valid, or the file cannot be written.
    pub fn save(&self, file: &Path) -> Result<(), String> {
        self.coder.validate()?;
        let mut bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        bytes.push(b'\n');
        super::autostart::write_private(file, &bytes)
    }

    /// Every `coder.*` setting and its value.
    #[must_use]
    pub fn values(&self) -> Map<String, Value> {
        keys()
            .iter()
            .filter_map(|key| Some(((*key).to_owned(), self.get(key).ok()?)))
            .collect()
    }

    /// The value of `key`.
    ///
    /// # Errors
    /// `key` is not a setting.
    pub fn get(&self, key: &str) -> Result<Value, String> {
        let coder = &self.coder;
        Ok(match key {
            "coder.providers" => json!(
                coder
                    .providers
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
            ),
            "coder.start" => json!(coder.start.as_str()),
            "coder.usage_threshold_percent" => json!(coder.usage_threshold_percent),
            "coder.projects" => json!(
                coder
                    .projects
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
            ),
            "coder.access" => json!(coder.access.as_str()),
            _ => return Err(unknown(key)),
        })
    }

    /// Set `key` from the words a person types: a comma-separated list for
    /// `coder.providers` and `coder.projects` (relative folders resolve
    /// against `dir`; an empty value is none), `at_once` or `ask_first`,
    /// a percent or `off`, and `toolchains`, `full`, or `boundary`.
    ///
    /// # Errors
    /// `key` is not a setting, or `value` is not one of its values; the
    /// settings are unchanged then.
    pub fn set(&mut self, key: &str, value: &str, dir: &Path) -> Result<(), String> {
        let mut coder = self.coder.clone();
        let list = |value: &str| -> Vec<String> {
            value
                .split(',')
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .map(str::to_owned)
                .collect()
        };
        match key {
            "coder.providers" => {
                coder.providers = list(value)
                    .iter()
                    .map(|item| item.parse())
                    .collect::<Result<_, _>>()?;
            }
            "coder.start" => {
                coder.start = match value.trim() {
                    "at_once" | "at-once" => Start::AtOnce,
                    "ask_first" | "ask-first" => Start::AskFirst,
                    other => return Err(format!("`{other}` is not at_once or ask_first")),
                };
            }
            "coder.usage_threshold_percent" => {
                coder.usage_threshold_percent = match value.trim() {
                    "off" | "null" | "none" => None,
                    number => Some(
                        number
                            .trim_end_matches('%')
                            .parse::<u8>()
                            .ok()
                            .filter(|p| (1..=100).contains(p))
                            .ok_or_else(|| format!("`{number}` is not 1 to 100, or off"))?,
                    ),
                };
            }
            "coder.projects" => {
                coder.projects = list(value)
                    .iter()
                    .map(|item| {
                        let folder = expand_home(item);
                        let folder = if folder.is_absolute() {
                            folder
                        } else {
                            dir.join(folder)
                        };
                        folder
                            .canonicalize()
                            .map_err(|_| format!("{} is not a folder here", folder.display()))
                    })
                    .collect::<Result<_, _>>()?;
            }
            "coder.access" => {
                coder.access = match value.trim() {
                    "toolchains" => Access::Toolchains,
                    "full" => Access::Full,
                    "boundary" => Access::Boundary,
                    other => return Err(format!("`{other}` is not toolchains, full, or boundary")),
                };
            }
            _ => return Err(unknown(key)),
        }
        coder.validate()?;
        self.coder = coder;
        Ok(())
    }

    /// Let `provider` run or not, as a settings screen's toggle does:
    /// turned on, it is added after the providers already allowed, with
    /// its default model; turned off, every entry naming it is removed.
    ///
    /// # Errors
    /// The change is not valid ([`Coder::validate`]): it would leave no
    /// provider, or the provider has no default model (OpenCode names its
    /// own). The settings are unchanged then.
    pub fn allow(&mut self, provider: Provider, on: bool) -> Result<(), String> {
        let mut coder = self.coder.clone();
        if on {
            if coder
                .providers
                .iter()
                .any(|choice| choice.provider == provider)
            {
                return Ok(());
            }
            let choice = Choice::new(provider);
            choice.route()?;
            coder.providers.push(choice);
        } else {
            coder.providers.retain(|choice| choice.provider != provider);
        }
        coder.validate()?;
        self.coder = coder;
        Ok(())
    }

    /// Return `key` to its default.
    ///
    /// # Errors
    /// `key` is not a setting.
    pub fn unset(&mut self, key: &str) -> Result<(), String> {
        let default = Coder::default();
        let coder = &mut self.coder;
        match key {
            "coder.providers" => coder.providers = default.providers,
            "coder.start" => coder.start = default.start,
            "coder.usage_threshold_percent" => {
                coder.usage_threshold_percent = default.usage_threshold_percent;
            }
            "coder.projects" => coder.projects = default.projects,
            "coder.access" => coder.access = default.access,
            _ => return Err(unknown(key)),
        }
        Ok(())
    }
}

fn expand_home(text: &str) -> PathBuf {
    match text.strip_prefix("~/") {
        Some(rest) => std::env::var_os("HOME").map_or_else(
            || PathBuf::from(text),
            |home| PathBuf::from(home).join(rest),
        ),
        None => PathBuf::from(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A settings screen's toggle adds a provider last, removes every
    /// entry of one, and refuses to leave none or to add OpenCode with no
    /// model, changing nothing then.
    #[test]
    fn a_toggle_allows_and_removes_a_provider_and_keeps_the_settings_valid() {
        let mut settings = Settings::default();
        settings.allow(Provider::Grok, false).unwrap();
        assert_eq!(
            settings.coder.provider_list(),
            vec![Provider::Codex, Provider::Claude]
        );
        settings.allow(Provider::Grok, true).unwrap();
        assert_eq!(
            settings.coder.provider_list(),
            vec![Provider::Codex, Provider::Claude, Provider::Grok]
        );
        settings.allow(Provider::Grok, true).unwrap();
        assert_eq!(settings.coder.providers.len(), 3);
        settings.allow(Provider::Devin, true).unwrap();
        assert_eq!(settings.coder.providers.len(), 4);
        settings.allow(Provider::Devin, false).unwrap();
        settings
            .set(
                "coder.providers",
                "codex:gpt-6-mini,claude,codex",
                Path::new("/"),
            )
            .unwrap();
        settings.allow(Provider::Codex, false).unwrap();
        assert_eq!(settings.coder.provider_list(), vec![Provider::Claude]);
        let before = settings.clone();
        assert!(settings.allow(Provider::Claude, false).is_err());
        assert!(settings.allow(Provider::OpenCode, true).is_err());
        assert_eq!(settings, before);
    }

    #[test]
    fn no_file_and_an_empty_section_are_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let settings = Settings::load(&file).unwrap();
        assert_eq!(settings, Settings::default());
        let coder = &settings.coder;
        assert_eq!(
            coder.routes().unwrap(),
            vec![
                Route {
                    provider: Provider::Codex,
                    model: "gpt-6-luna".into(),
                    effort: None
                },
                Route {
                    provider: Provider::Claude,
                    model: "claude-opus-5-5".into(),
                    effort: None
                },
                Route {
                    provider: Provider::Grok,
                    model: acp_client::grok::DEFAULT_MODEL.into(),
                    effort: None
                },
            ]
        );
        // OpenCode needs a model and Devin is a paid API: neither is on
        // unless the person names it (#10091).
        assert!(!coder.provider_list().contains(&Provider::OpenCode));
        assert!(!coder.provider_list().contains(&Provider::Devin));
        assert_eq!(
            settings.get("coder.providers").unwrap(),
            json!(["codex", "claude", "grok"])
        );
        assert_eq!(coder.start, Start::AtOnce);
        assert_eq!(
            coder.usage_threshold_percent,
            Some(usage::DEFAULT_THRESHOLD_PERCENT)
        );
        assert!(coder.projects.is_empty());
        assert_eq!(coder.access, Access::Toolchains);
        std::fs::write(&file, format!(r#"{{"schema":"{SCHEMA}","coder":{{}}}}"#)).unwrap();
        assert_eq!(Settings::load(&file).unwrap(), Settings::default());
        std::fs::write(&file, format!(r#"{{"schema":"{SCHEMA}"}}"#)).unwrap();
        assert_eq!(Settings::load(&file).unwrap(), Settings::default());
    }

    #[test]
    fn a_broken_file_is_never_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        for text in [
            "{".to_owned(),
            r#"{"schema":"other"}"#.to_owned(),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"provider":["claude"]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"providers":[]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"providers":["vertex"]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"providers":["opencode"]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"providers":["codex","codex"]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"start":"later"}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"usage_threshold_percent":0}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"projects":["rel"]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"access":"root"}}}}"#),
        ] {
            std::fs::write(&file, &text).unwrap();
            let error = Settings::load(&file).unwrap_err();
            assert!(
                error.contains(&file.display().to_string()),
                "{text}: {error}"
            );
        }
    }

    #[test]
    fn every_key_sets_gets_saves_and_unsets() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("nested/settings.json");
        let mut settings = Settings::default();
        settings
            .other
            .insert("appearance".into(), json!({"theme": "dark"}));
        settings
            .set(
                "coder.providers",
                "claude, opencode:anthropic/claude-sonnet-5,devin",
                dir.path(),
            )
            .unwrap();
        settings
            .set("coder.start", "ask_first", dir.path())
            .unwrap();
        settings
            .set("coder.usage_threshold_percent", "75", dir.path())
            .unwrap();
        std::fs::create_dir_all(dir.path().join("code")).unwrap();
        settings.set("coder.projects", "code", dir.path()).unwrap();
        settings.set("coder.access", "full", dir.path()).unwrap();
        settings.save(&file).unwrap();
        let loaded = Settings::load(&file).unwrap();
        assert_eq!(loaded, settings);
        assert_eq!(loaded.other["appearance"]["theme"], "dark");
        assert_eq!(
            loaded.get("coder.providers").unwrap(),
            json!(["claude", "opencode:anthropic/claude-sonnet-5", "devin"])
        );
        assert_eq!(
            loaded.coder.provider_list(),
            vec![Provider::Claude, Provider::OpenCode, Provider::Devin]
        );
        assert_eq!(loaded.get("coder.start").unwrap(), "ask_first");
        assert_eq!(loaded.get("coder.usage_threshold_percent").unwrap(), 75);
        assert_eq!(loaded.get("coder.access").unwrap(), "full");
        let code = dir.path().join("code").canonicalize().unwrap();
        assert_eq!(loaded.coder.projects, vec![code.clone()]);
        assert!(loaded.coder.admits_project(&code.join("app")));
        assert!(!loaded.coder.admits_project(dir.path()));
        assert_eq!(loaded.values().len(), keys().len());

        // A bad value changes nothing.
        let mut edited = loaded.clone();
        for (key, value) in [
            ("coder.providers", ""),
            ("coder.providers", "gemini"),
            ("coder.start", "soon"),
            ("coder.usage_threshold_percent", "101"),
            ("coder.projects", "missing-folder"),
            ("coder.access", "root"),
            ("coder.model", "x"),
        ] {
            assert!(edited.set(key, value, dir.path()).is_err(), "{key}={value}");
        }
        assert_eq!(edited, loaded);
        edited
            .set("coder.usage_threshold_percent", "off", dir.path())
            .unwrap();
        assert_eq!(
            edited.get("coder.usage_threshold_percent").unwrap(),
            Value::Null
        );
        for key in keys() {
            edited.unset(key).unwrap();
        }
        assert_eq!(edited.coder, Coder::default());
        assert_eq!(edited.other["appearance"]["theme"], "dark");
    }
}
