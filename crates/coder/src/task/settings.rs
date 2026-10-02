//! Local capability settings: what Coder may use on this computer when the
//! person at it asks for coding (`coder::task::local`).
//!
//! One file, read by `openagents chat`, the desktop, and a host on this
//! computer: `$OPENAGENTS_SETTINGS`, else `~/.openagents/settings.json`
//! ([`path`]). A missing file, or a missing field, means the default, and
//! the defaults are exactly what a computer does with no file at all
//! (#10032, #10045, #10091, #10104, #10184): every coding agent signed in
//! here, Codex first, then Claude Code, Grok Build, Devin, and OpenCode
//! (each only when signed in here), a coding request runs at once,
//! a fresh usage reading at or above 90% passes a provider over, any Git
//! checkout is a project, and every step is approved: commands run as the
//! person's own user with full access, so each engine's own permission
//! asks are granted and Coder commits and pushes without asking.
//!
//! ```json
//! {
//!   "schema": "openagents.settings.v1",
//!   "coder": {
//!     "providers": ["claude"],
//!     "disabled": ["devin"],
//!     "start": "at_once",
//!     "usage_threshold_percent": 90,
//!     "projects": [],
//!     "access": "full"
//!   }
//! }
//! ```
//!
//! Coding agents are opt-out (#10184): no one edits a settings file to use
//! an agent they have signed in to. `coder.disabled` names the agents the
//! person turned off, and only those are never used; `coder.providers` is
//! an order preference (and a model per agent, `NAME:MODEL`): the agents it
//! names are tried first, in its order, and every other agent follows in
//! the default order. A file from before #10184 whose `providers` left an
//! agent out therefore reads as that order, with the agent left out still
//! available. OpenCode runs on the model its own configuration names (its
//! `model`), unless `providers` names one as `opencode:PROVIDER/MODEL`.
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
/// The providers a local run can use, in their default order: Codex
/// first (the owner's choice), then Claude Code, Grok Build, Devin, and
/// OpenCode.
pub const PROVIDERS: [Provider; 5] = [
    Provider::Codex,
    Provider::Claude,
    Provider::Grok,
    Provider::Devin,
    Provider::OpenCode,
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
        Provider::Codex => Some("gpt-6.1-sol"),
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

impl Choice {
    /// What tells two choices apart: its route, or OpenCode named without
    /// a model, which runs on OpenCode's own configured one (#10184).
    fn key(&self) -> Result<Route, String> {
        if self.provider == Provider::OpenCode && self.model.is_none() {
            return Ok(Route {
                provider: Provider::OpenCode,
                model: String::new(),
                effort: None,
            });
        }
        self.route()
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
        choice.key()?;
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

/// The default model OpenCode's own configuration names on this computer
/// ([`acp_client::opencode::configured_model`]).
#[must_use]
pub fn opencode_model() -> Option<String> {
    acp_client::opencode::configured_model(&|name| std::env::var_os(name))
}

#[allow(clippy::unnecessary_wraps)]
fn default_threshold() -> Option<u8> {
    Some(usage::DEFAULT_THRESHOLD_PERCENT)
}

/// Full access (#10104): the person runs Coder on their own computer to
/// have the work done end to end, so every step is approved by default.
/// `toolchains` and `boundary` stay for a person who names them.
fn default_access() -> Access {
    Access::Full
}

/// A file names `access` only when the person chose other than the
/// default, so a file saved for another setting keeps following it.
fn is_default_access(access: &Access) -> bool {
    *access == default_access()
}

/// What Coder may use on this computer for a person's own runs.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Coder {
    /// The order Coder tries agents in, first preferred, and the model
    /// each runs (#10184): an order preference, never a list of what may
    /// run. Agents it leaves out follow in the default order. Empty: the
    /// default order. At most [`MAX_PROVIDERS`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub providers: Vec<Choice>,
    /// The agents the person turned off: the only ones never used
    /// (#10184). Empty: every agent signed in here may run.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disabled: Vec<Provider>,
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
    /// What a run's commands may reach: `full` (the default: no sandbox,
    /// as the person's own user, every engine's permission asks granted),
    /// `toolchains` (the filesystem boundary with this computer's developer
    /// tools), or `boundary` (the plain boundary).
    #[serde(default = "default_access", skip_serializing_if = "is_default_access")]
    pub access: Access,
}

impl Default for Coder {
    fn default() -> Self {
        Coder {
            providers: Vec::new(),
            disabled: Vec::new(),
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
        if self.providers.len() > MAX_PROVIDERS {
            return Err(format!(
                "coder.providers names more than {MAX_PROVIDERS} providers"
            ));
        }
        let mut routes: Vec<Route> = Vec::new();
        for choice in &self.providers {
            let route = choice.key()?;
            if routes.contains(&route) {
                return Err(format!("coder.providers names {choice} twice"));
            }
            routes.push(route);
        }
        for provider in &self.disabled {
            if !PROVIDERS.contains(provider) {
                return Err(format!(
                    "coder.disabled names {provider}, which is no agent here"
                ));
            }
        }
        if PROVIDERS
            .iter()
            .all(|provider| self.disabled.contains(provider))
        {
            return Err(
                "coder.disabled turns off every coding agent; leave at least one on".into(),
            );
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

    /// Whether the person turned `provider` off.
    #[must_use]
    pub fn is_disabled(&self, provider: Provider) -> bool {
        self.disabled.contains(&provider)
    }

    /// Every agent not turned off, in the order Coder tries them (#10184):
    /// the ones `providers` names, in its order, then every other one in
    /// [`PROVIDERS`]' order, each with its model. OpenCode named without a
    /// model runs on `opencode_model`, OpenCode's own configured default,
    /// and is left out when there is none. At most [`MAX_PROVIDERS`].
    #[must_use]
    pub fn choices_with(&self, opencode_model: Option<&str>) -> Vec<Choice> {
        let on = |provider: Provider| !self.is_disabled(provider);
        let mut out: Vec<Choice> = self
            .providers
            .iter()
            .filter(|choice| on(choice.provider))
            .cloned()
            .collect();
        for provider in PROVIDERS {
            if on(provider) && !self.providers.iter().any(|c| c.provider == provider) {
                out.push(Choice::new(provider));
            }
        }
        out.into_iter()
            .filter_map(|choice| {
                if choice.provider == Provider::OpenCode && choice.model.is_none() {
                    return opencode_model.map(|model| Choice {
                        provider: Provider::OpenCode,
                        model: Some(model.to_owned()),
                    });
                }
                Some(choice)
            })
            .take(MAX_PROVIDERS)
            .collect()
    }

    /// The routes a run may use, in preference order: [`Coder::choices_with`].
    ///
    /// # Errors
    /// As [`Coder::validate`], or no agent is left to run.
    pub fn routes_with(&self, opencode_model: Option<&str>) -> Result<Vec<Route>, String> {
        self.validate()?;
        let routes = self
            .choices_with(opencode_model)
            .iter()
            .map(Choice::route)
            .collect::<Result<Vec<_>, _>>()?;
        if routes.is_empty() {
            return Err(
                "every coding agent but OpenCode is turned off, and OpenCode names no model: \
                 set one in OpenCode's own configuration (its `model`)"
                    .into(),
            );
        }
        Ok(routes)
    }

    /// [`Coder::routes_with`] with OpenCode's configured model here.
    ///
    /// # Errors
    /// As [`Coder::routes_with`].
    pub fn routes(&self) -> Result<Vec<Route>, String> {
        self.routes_with(opencode_model().as_deref())
    }

    /// Every agent not turned off, in the order Coder tries them, each
    /// once, whether or not it has a model ([`Coder::choices_with`]).
    #[must_use]
    pub fn provider_list(&self) -> Vec<Provider> {
        let mut out: Vec<Provider> = Vec::new();
        for choice in &self.providers {
            if !out.contains(&choice.provider) && !self.is_disabled(choice.provider) {
                out.push(choice.provider);
            }
        }
        for provider in PROVIDERS {
            if !out.contains(&provider) && !self.is_disabled(provider) {
                out.push(provider);
            }
        }
        out
    }

    /// Every agent in the order a settings screen lists them: the ones not
    /// turned off, in the order Coder tries them, then the ones turned off.
    #[must_use]
    pub fn listing(&self) -> Vec<(Provider, bool)> {
        let on = self.provider_list();
        let off = PROVIDERS.into_iter().filter(|p| !on.contains(p));
        on.iter()
            .map(|p| (*p, true))
            .chain(off.map(|p| (p, false)))
            .collect()
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
pub const fn keys() -> [&'static str; 6] {
    [
        "coder.providers",
        "coder.disabled",
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
            "coder.disabled" => json!(
                coder
                    .disabled
                    .iter()
                    .map(|p| p.as_str())
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
            "coder.disabled" => {
                let mut disabled: Vec<Provider> = Vec::new();
                for item in list(value) {
                    let provider = agent(&item)?;
                    if !disabled.contains(&provider) {
                        disabled.push(provider);
                    }
                }
                coder.disabled = disabled;
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

    /// Turn `provider` on or off, as a settings screen's toggle does
    /// (#10184): only a turn-off is recorded (`coder.disabled`); turning it
    /// back on removes that, and `coder.providers`' order is kept as it is.
    ///
    /// # Errors
    /// The change is not valid ([`Coder::validate`]): it would turn every
    /// agent off. The settings are unchanged then.
    pub fn allow(&mut self, provider: Provider, on: bool) -> Result<(), String> {
        let mut coder = self.coder.clone();
        if on {
            coder.disabled.retain(|p| *p != provider);
        } else if !coder.disabled.contains(&provider) {
            coder.disabled.push(provider);
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
            "coder.disabled" => coder.disabled = default.disabled,
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

/// The agent a person names: `codex`, `claude`, `grok`, `devin`, or
/// `opencode`.
///
/// # Errors
/// `name` is no coding agent here.
pub fn agent(name: &str) -> Result<Provider, String> {
    Provider::from_config(name.trim())
        .filter(|provider| PROVIDERS.contains(provider))
        .ok_or_else(|| {
            format!(
                "`{}` is not a coding agent here: codex, claude, grok, devin, or opencode",
                name.trim()
            )
        })
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

    const ALL: [Provider; 5] = [
        Provider::Codex,
        Provider::Claude,
        Provider::Grok,
        Provider::Devin,
        Provider::OpenCode,
    ];

    /// A settings screen's toggle records only a turn-off (#10184):
    /// turning an agent off puts it in `coder.disabled`, turning it back on
    /// takes it out, the order is kept, and turning every agent off is
    /// refused, changing nothing then.
    #[test]
    fn a_toggle_records_only_what_is_turned_off() {
        let mut settings = Settings::default();
        settings.allow(Provider::Grok, false).unwrap();
        assert_eq!(settings.coder.disabled, vec![Provider::Grok]);
        assert!(settings.coder.providers.is_empty());
        assert_eq!(
            settings.coder.provider_list(),
            vec![
                Provider::Codex,
                Provider::Claude,
                Provider::Devin,
                Provider::OpenCode
            ]
        );
        settings.allow(Provider::Grok, true).unwrap();
        assert_eq!(settings, Settings::default());
        settings.allow(Provider::Grok, true).unwrap();
        assert_eq!(settings, Settings::default());
        // Turning on OpenCode with no model named is no error: it runs on
        // OpenCode's own configured model.
        settings.allow(Provider::OpenCode, true).unwrap();
        settings
            .set(
                "coder.providers",
                "codex:gpt-6-mini,claude,codex",
                Path::new("/"),
            )
            .unwrap();
        settings.allow(Provider::Codex, false).unwrap();
        assert_eq!(
            settings.coder.provider_list(),
            vec![
                Provider::Claude,
                Provider::Grok,
                Provider::Devin,
                Provider::OpenCode
            ]
        );
        for provider in [Provider::Grok, Provider::Devin, Provider::OpenCode] {
            settings.allow(provider, false).unwrap();
        }
        let before = settings.clone();
        assert!(settings.allow(Provider::Claude, false).is_err());
        assert_eq!(settings, before);
        // The order survives a turn-off and back.
        settings.allow(Provider::Codex, true).unwrap();
        assert_eq!(
            settings.get("coder.providers").unwrap(),
            json!(["codex:gpt-6-mini", "claude", "codex"])
        );
    }

    /// Opt-out (#10184): no settings means every agent, Codex first; a
    /// turn-off excludes only that agent; a list from before #10184 is an
    /// order, so an agent it leaves out still runs after the ones it names.
    #[test]
    fn every_agent_runs_unless_turned_off_and_a_list_is_only_an_order() {
        let order = |coder: &Coder| -> Vec<Provider> {
            coder
                .routes_with(Some("anthropic/claude-sonnet-5"))
                .unwrap()
                .iter()
                .map(|route| route.provider)
                .collect()
        };
        let none = Coder::default();
        assert_eq!(order(&none), ALL.to_vec());
        assert_eq!(none.provider_list(), ALL.to_vec());
        // Devin is on with nothing set: no one enables a signed-in agent.
        assert!(
            none.routes_with(None)
                .unwrap()
                .iter()
                .any(|r| r.provider == Provider::Devin)
        );
        // OpenCode runs on its own configured model, and is left out of
        // the routes (not the list) when it names none.
        let opencode = none
            .routes_with(Some("anthropic/claude-sonnet-5"))
            .unwrap()
            .into_iter()
            .find(|route| route.provider == Provider::OpenCode)
            .unwrap();
        assert_eq!(opencode.model, "anthropic/claude-sonnet-5");
        assert!(
            !none
                .routes_with(None)
                .unwrap()
                .iter()
                .any(|r| r.provider == Provider::OpenCode)
        );

        let mut off = Coder::default();
        off.disabled = vec![Provider::Devin];
        assert_eq!(
            order(&off),
            vec![
                Provider::Codex,
                Provider::Claude,
                Provider::Grok,
                Provider::OpenCode
            ]
        );

        // The owner's file before #10184: Devin was left out, so it never
        // ran. It now reads as an order, and Devin follows.
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        std::fs::write(
            &file,
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"providers":["claude","codex"]}}}}"#),
        )
        .unwrap();
        let legacy = Settings::load(&file).unwrap().coder;
        assert_eq!(
            order(&legacy),
            vec![
                Provider::Claude,
                Provider::Codex,
                Provider::Grok,
                Provider::Devin,
                Provider::OpenCode
            ]
        );
        // A named model is kept, and a disabled agent the order names is
        // still off.
        let mut named = legacy.clone();
        named.providers = vec![
            "devin".parse().unwrap(),
            "codex:gpt-6-mini".parse().unwrap(),
        ];
        named.disabled = vec![Provider::Devin, Provider::OpenCode];
        let routes = named
            .routes_with(Some("anthropic/claude-sonnet-5"))
            .unwrap();
        assert_eq!(routes[0].provider, Provider::Codex);
        assert_eq!(routes[0].model, "gpt-6-mini");
        assert_eq!(
            routes.iter().map(|r| r.provider).collect::<Vec<_>>(),
            vec![Provider::Codex, Provider::Claude, Provider::Grok]
        );
        assert_eq!(
            named.listing(),
            vec![
                (Provider::Codex, true),
                (Provider::Claude, true),
                (Provider::Grok, true),
                (Provider::Devin, false),
                (Provider::OpenCode, false),
            ]
        );
    }

    #[test]
    fn no_file_and_an_empty_section_are_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        let settings = Settings::load(&file).unwrap();
        assert_eq!(settings, Settings::default());
        let coder = &settings.coder;
        assert_eq!(
            coder.routes_with(None).unwrap()[..3].to_vec(),
            vec![
                Route {
                    provider: Provider::Codex,
                    model: "gpt-6.1-sol".into(),
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
        // Every agent is on with nothing set (#10184).
        assert_eq!(coder.provider_list(), ALL.to_vec());
        assert_eq!(settings.get("coder.providers").unwrap(), json!([]));
        assert_eq!(settings.get("coder.disabled").unwrap(), json!([]));
        assert_eq!(coder.start, Start::AtOnce);
        assert_eq!(
            coder.usage_threshold_percent,
            Some(usage::DEFAULT_THRESHOLD_PERCENT)
        );
        assert!(coder.projects.is_empty());
        assert_eq!(coder.access, Access::Full);
        std::fs::write(&file, format!(r#"{{"schema":"{SCHEMA}","coder":{{}}}}"#)).unwrap();
        assert_eq!(Settings::load(&file).unwrap(), Settings::default());
        std::fs::write(&file, format!(r#"{{"schema":"{SCHEMA}"}}"#)).unwrap();
        assert_eq!(Settings::load(&file).unwrap(), Settings::default());
    }

    /// Every step is approved by default (#10104): no file means full
    /// access, a file saved for another setting does not pin a narrower
    /// one, and a file that names `toolchains` or `boundary` keeps it.
    #[test]
    fn full_access_is_the_default_and_a_named_boundary_still_reads() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        assert_eq!(Settings::default().coder.access, Access::Full);
        let mut settings = Settings::default();
        settings.set("coder.start", "at_once", dir.path()).unwrap();
        settings.save(&file).unwrap();
        let saved = std::fs::read_to_string(&file).unwrap();
        assert!(!saved.contains("\"access\""), "{saved}");
        assert_eq!(Settings::load(&file).unwrap().coder.access, Access::Full);
        for (named, access) in [
            ("toolchains", Access::Toolchains),
            ("boundary", Access::Boundary),
            ("full", Access::Full),
        ] {
            std::fs::write(
                &file,
                format!(r#"{{"schema":"{SCHEMA}","coder":{{"access":"{named}"}}}}"#),
            )
            .unwrap();
            let loaded = Settings::load(&file).unwrap();
            assert_eq!(loaded.coder.access, access, "{named}");
            loaded.save(&file).unwrap();
            assert_eq!(
                Settings::load(&file).unwrap().coder.access,
                access,
                "{named}"
            );
        }
    }

    #[test]
    fn a_broken_file_is_never_the_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("settings.json");
        for text in [
            "{".to_owned(),
            r#"{"schema":"other"}"#.to_owned(),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"provider":["claude"]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"providers":["vertex"]}}}}"#),
            format!(r#"{{"schema":"{SCHEMA}","coder":{{"disabled":["vertex"]}}}}"#),
            format!(
                r#"{{"schema":"{SCHEMA}","coder":{{"disabled":["codex","claude","grok","devin","opencode"]}}}}"#
            ),
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
        settings
            .set("coder.disabled", "grok, grok", dir.path())
            .unwrap();
        settings.save(&file).unwrap();
        let loaded = Settings::load(&file).unwrap();
        assert_eq!(loaded, settings);
        assert_eq!(loaded.other["appearance"]["theme"], "dark");
        assert_eq!(
            loaded.get("coder.providers").unwrap(),
            json!(["claude", "opencode:anthropic/claude-sonnet-5", "devin"])
        );
        assert_eq!(loaded.get("coder.disabled").unwrap(), json!(["grok"]));
        assert_eq!(
            loaded.coder.provider_list(),
            vec![
                Provider::Claude,
                Provider::OpenCode,
                Provider::Devin,
                Provider::Codex
            ]
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
            ("coder.providers", "gemini"),
            ("coder.disabled", "vertex"),
            ("coder.disabled", "codex,claude,grok,devin,opencode"),
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
