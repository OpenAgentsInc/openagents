//! Explicit public lookups and host-owned Brainstorm configuration.

use brainstorm_client::{Cancellation, Client, Config, Discovery, Error, Limits, Observation};
use crossterm::event::{KeyCode, KeyEvent};
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, future::Future, pin::Pin, sync::Arc};
use tokio::sync::watch;

mod native;
pub use native::Native;
pub(crate) use native::tool_definition;

pub const PLUGIN: &str = "brainstorm";
pub const USAGE: &str =
    "/brainstorm search <public query> · /brainstorm rank <hex-or-npub> [more keys]";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Perspective {
    #[default]
    House,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    pub enabled: bool,
    pub origin: String,
    pub perspective: Perspective,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: false,
            origin: brainstorm_client::DEFAULT_ORIGIN.into(),
            perspective: Perspective::House,
        }
    }
}

impl Preferences {
    pub(crate) fn valid(&self) -> bool {
        self.origin.len() <= 2048
            && !self
                .origin
                .chars()
                .any(|character| character.is_control() || character.is_whitespace())
            && Client::new(self.configuration()).is_ok()
    }
    pub fn configuration(&self) -> Config {
        Config {
            origin: self.origin.clone(),
            limits: Limits::default(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    Search(String),
    Rank(Vec<String>),
    Test,
}

impl Command {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Search(_) => "brainstorm.search_people",
            Self::Rank(_) => "brainstorm.rank",
            Self::Test => "brainstorm.connection",
        }
    }
    pub fn input(&self, origin: &str) -> serde_json::Value {
        match self {
            Self::Search(query) => {
                serde_json::json!({ "query": query, "recipient": origin, "algorithm": "relevance", "limit": 10 })
            }
            Self::Rank(keys) => {
                serde_json::json!({ "pubkeys": keys, "recipient": origin, "algorithm": "graperank" })
            }
            Self::Test => {
                serde_json::json!({ "recipient": origin, "operation": "public discovery" })
            }
        }
    }
}

/// Recognize one exact command word. Invalid arguments never become chat text.
pub fn parse(text: &str) -> Option<Result<Command, &'static str>> {
    let rest = text.strip_prefix("/brainstorm")?;
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let rest = rest.trim_start();
    let Some((operation, argument)) = rest.split_once(char::is_whitespace) else {
        return Some(Err(USAGE));
    };
    let argument = argument.trim_start();
    Some(match operation {
        "search"
            if !argument.trim().is_empty()
                && argument.chars().count() <= 512
                && argument.len() <= 1024 =>
        {
            Ok(Command::Search(argument.into()))
        }
        "search" => Err("Enter a public query using at most 512 characters and 1 KiB."),
        "rank" => {
            let mut found = HashSet::new();
            let mut keys = Vec::new();
            for argument in argument.split_whitespace() {
                if keys.len() == 20 {
                    return Some(Err("Rank at most 20 public keys."));
                }
                let Some(key) = public_key(argument) else {
                    return Some(Err(
                        "Enter a valid public hex key or npub. Secret keys and profile URLs are refused.",
                    ));
                };
                if !found.insert(key.clone()) {
                    return Some(Err("Each rank subject must be a different public key."));
                }
                keys.push(key);
            }
            if keys.is_empty() {
                Err(USAGE)
            } else {
                Ok(Command::Rank(keys))
            }
        }
        _ => Err(USAGE),
    })
}

fn public_key(value: &str) -> Option<String> {
    let bytes = if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        let mut bytes = [0; 32];
        for (index, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
        }
        nostr::nip19::decode_npub(&nostr::nip19::encode_npub(&bytes)).ok()?
    } else {
        nostr::nip19::decode_npub(value).ok()?
    };
    Some(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

pub enum Outcome {
    Discovery(Discovery),
    Observation(Observation),
}

/// Keep lookup evidence in the conversation that admitted its public input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Conversation {
    Main,
    Delegation(String),
}

impl Conversation {
    pub(crate) fn selected(app: &crate::App) -> Option<Self> {
        match app.selected_agent {
            None => Some(Self::Main),
            Some(index) => app
                .delegations
                .get(index)
                .map(|child| Self::Delegation(child.id.clone())),
        }
    }

    pub(crate) fn chat<'a>(&self, app: &'a crate::App) -> Option<&'a crate::live::Chat> {
        match self {
            Self::Main => Some(&app.live),
            Self::Delegation(id) => app
                .delegations
                .iter()
                .find(|child| &child.id == id)
                .map(|child| &child.chat),
        }
    }

    pub(crate) fn chat_mut<'a>(
        &self,
        app: &'a mut crate::App,
    ) -> Option<&'a mut crate::live::Chat> {
        match self {
            Self::Main => Some(&mut app.live),
            Self::Delegation(id) => app
                .delegations
                .iter_mut()
                .find(|child| &child.id == id)
                .map(|child| &mut child.chat),
        }
    }
}

pub(crate) trait Lookup: Send + Sync {
    fn set_enabled(&self, enabled: bool);
    fn read<'a>(
        &'a self,
        command: &'a Command,
        cancellation: &'a Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Outcome, Error>> + Send + 'a>>;
}

impl Lookup for Client {
    fn set_enabled(&self, enabled: bool) {
        Client::set_enabled(self, enabled);
    }
    fn read<'a>(
        &'a self,
        command: &'a Command,
        cancellation: &'a Cancellation,
    ) -> Pin<Box<dyn Future<Output = Result<Outcome, Error>> + Send + 'a>> {
        Box::pin(async move {
            match command {
                Command::Search(query) => self
                    .search(query, 10, cancellation)
                    .await
                    .map(Outcome::Observation),
                Command::Rank(keys) => self
                    .rank(keys, cancellation)
                    .await
                    .map(Outcome::Observation),
                Command::Test => self.discover(cancellation).await.map(Outcome::Discovery),
            }
        })
    }
}

struct Binding {
    lookup: Arc<dyn Lookup>,
    enabled: watch::Sender<bool>,
    origin: String,
    generation: u64,
    admissions: std::sync::Mutex<std::collections::VecDeque<native::Admission>>,
}

/// A host snapshot of an explicitly admitted command and recipient.
#[derive(Clone)]
pub struct Job {
    pub generation: u64,
    pub command: Command,
    pub origin: String,
    pub cancellation: Cancellation,
    binding: Arc<Binding>,
    input_ref: Option<String>,
}

impl Job {
    pub(crate) fn input_reference(&self) -> Option<&str> {
        self.input_ref.as_deref()
    }
    pub async fn run(&self) -> Result<Outcome, Error> {
        if self.generation != self.binding.generation || self.origin != self.binding.origin {
            return Err(Error::InvalidInput {
                field: "configuration".into(),
            });
        }
        if !*self.binding.enabled.borrow() {
            return Err(Error::Disabled);
        }
        if self.cancellation.is_cancelled() {
            return Err(Error::Cancelled);
        }
        if let Some(reference) = &self.input_ref {
            native::check(&self.binding, reference, &self.command).map_err(|_| {
                Error::InvalidInput {
                    field: "admission".into(),
                }
            })?;
        }
        let mut enabled = self.binding.enabled.subscribe();
        tokio::select! {
            biased;
            _ = enabled.wait_for(|enabled| !*enabled) => Err(Error::Disabled),
            result = self.binding.lookup.read(&self.command, &self.cancellation) => {
                if let (Some(reference), Ok(Outcome::Observation(observation))) = (&self.input_ref, &result) {
                    native::retain_subjects(&self.binding, reference, observation);
                }
                result
            },
        }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub enum Focus {
    #[default]
    Origin,
    Test,
    Save,
    Cancel,
}

pub struct Settings {
    pub preferences: Preferences,
    pub focus: Focus,
    pub error: Option<String>,
    pub connection: crate::plugins::Connection,
    pub discovery: Option<Discovery>,
    pub generation: u64,
    pub check_requested: bool,
    pub save_requested: bool,
    pub fixture: bool,
    draft: crate::Draft,
    live: bool,
    binding: Option<Arc<Binding>>,
    #[cfg(test)]
    fixture_lookup: Option<Arc<dyn Lookup>>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            preferences: Preferences::default(),
            focus: Focus::Origin,
            error: None,
            connection: crate::plugins::Connection::Unchecked,
            discovery: None,
            generation: 0,
            check_requested: false,
            save_requested: false,
            fixture: false,
            draft: crate::Draft::default(),
            live: false,
            binding: None,
            #[cfg(test)]
            fixture_lookup: None,
        }
    }
}

impl Settings {
    pub(crate) fn configure(&mut self, preferences: Preferences, live: bool) {
        if preferences == self.preferences && live == self.live && self.binding.is_some() {
            return;
        }
        if let Some(binding) = self.binding.take() {
            binding.enabled.send_replace(false);
            binding.lookup.set_enabled(false);
        }
        self.generation = self.generation.wrapping_add(1);
        self.preferences = preferences;
        self.live = live;
        self.connection = crate::plugins::Connection::Unchecked;
        self.discovery = None;
        self.fixture = false;
        if live && cfg!(any(unix, windows)) {
            #[cfg(test)]
            let lookup = self.fixture_lookup.clone().or_else(|| {
                Client::new(self.preferences.configuration())
                    .ok()
                    .map(|client| Arc::new(client) as Arc<dyn Lookup>)
            });
            #[cfg(not(test))]
            let lookup = Client::new(self.preferences.configuration())
                .ok()
                .map(|client| Arc::new(client) as Arc<dyn Lookup>);
            if let Some(lookup) = lookup {
                lookup.set_enabled(self.preferences.enabled);
                self.binding = Some(Arc::new(Binding {
                    lookup,
                    enabled: watch::channel(self.preferences.enabled).0,
                    origin: self.preferences.origin.clone(),
                    generation: self.generation,
                    admissions: Default::default(),
                }));
            }
        }
    }

    pub fn status(&self) -> &str {
        if !self.preferences.enabled {
            return "Disabled";
        }
        if !self.live {
            return "Demo fixture";
        }
        if self.binding.is_none() {
            return "Unavailable";
        }
        match &self.connection {
            crate::plugins::Connection::Checking => "Checking",
            crate::plugins::Connection::Verified => {
                if self
                    .discovery
                    .as_ref()
                    .is_some_and(|discovery| discovery.expires_at_ms <= atif::now_ms())
                {
                    "Expired"
                } else {
                    "Verified"
                }
            }
            crate::plugins::Connection::Failed(_) => "Unavailable",
            crate::plugins::Connection::Unchecked => "Configured",
        }
    }

    pub fn begin(&mut self) {
        self.draft.text.clone_from(&self.preferences.origin);
        self.draft.cursor = self.draft.text.len();
        self.focus = Focus::Origin;
        self.error = None;
        self.check_requested = false;
        self.save_requested = false;
    }
    pub fn field(&self) -> (String, usize) {
        (self.draft.text.clone(), self.draft.cursor)
    }
    pub fn paste(&mut self, text: &str) {
        if self.focus == Focus::Origin && self.draft.text.len().saturating_add(text.len()) <= 2048 {
            self.draft.insert(text);
        }
    }
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        self.check_requested = false;
        self.save_requested = false;
        match key.code {
            KeyCode::Esc => {
                self.begin();
                return true;
            }
            KeyCode::Tab | KeyCode::BackTab => {
                let fields = [Focus::Origin, Focus::Test, Focus::Save, Focus::Cancel];
                let index = fields
                    .iter()
                    .position(|focus| *focus == self.focus)
                    .unwrap_or(0);
                self.focus = fields[(index + if key.code == KeyCode::BackTab { 3 } else { 1 }) % 4];
            }
            KeyCode::Enter => match self.focus {
                Focus::Origin => self.focus = Focus::Test,
                Focus::Test => self.check_requested = true,
                Focus::Save => self.save_requested = true,
                Focus::Cancel => {
                    self.begin();
                    return true;
                }
            },
            _ if self.focus == Focus::Origin => {
                if self.draft.text.len() < 2048 || !matches!(key.code, KeyCode::Char(_)) {
                    self.draft.edit(key);
                }
            }
            _ => {}
        }
        false
    }
    pub(crate) fn edited_preferences(&self) -> Result<Preferences, &'static str> {
        let preferences = Preferences {
            origin: self.draft.text.trim().into(),
            ..self.preferences.clone()
        };
        if preferences.valid() {
            let origin = Client::new(preferences.configuration())
                .map_err(|_| "The Brainstorm origin is unavailable on this host.")?
                .configuration()
                .origin
                .clone();
            Ok(Preferences {
                origin,
                ..preferences
            })
        } else {
            Err("Enter an HTTPS origin without credentials, a path prefix, query, or fragment.")
        }
    }
    pub fn job(&self, command: Command) -> Result<Job, String> {
        if !self.preferences.enabled {
            return Err("Enable Brainstorm in /plugins before a public lookup.".into());
        }
        let Some(binding) = self.binding.clone() else {
            return Err("Brainstorm is unavailable on this host.".into());
        };
        let command = native::validate(command)?;
        let input_ref = if matches!(command, Command::Test) {
            None
        } else {
            Some(native::admit(&binding, command.clone())?)
        };
        Ok(Job {
            generation: self.generation,
            command,
            origin: self.preferences.origin.clone(),
            cancellation: Cancellation::default(),
            binding,
            input_ref,
        })
    }

    pub fn native(&self) -> Option<Native> {
        self.preferences
            .enabled
            .then(|| self.binding.clone())
            .flatten()
            .filter(|binding| *binding.enabled.borrow())
            .map(|binding| Native {
                generation: self.generation,
                origin: self.preferences.origin.clone(),
                binding,
            })
    }
}

pub(crate) fn is_tool(name: &str) -> bool {
    name.starts_with("brainstorm.")
        || matches!(name, "brainstorm_search_people" | "brainstorm_rank")
}

pub fn output(outcome: Outcome) -> serde_json::Value {
    match outcome {
        Outcome::Observation(observation) => serde_json::json!({ "observation": observation }),
        Outcome::Discovery(discovery) => serde_json::json!({ "discovery": discovery }),
    }
}

pub fn context(output: &serde_json::Value) -> Option<String> {
    if let Some(value) = output.get("observation") {
        let observation: Observation = serde_json::from_value(value.clone()).ok()?;
        let text = match observation.model_context() {
            Ok(text) => text,
            Err(error) => return Some(serde_json::json!({"error":error.to_string(),"state":error,
                "recipient":observation.configuration.origin,"configuration_digest":observation.configuration_digest}).to_string()),
        };
        let mut projected: serde_json::Value = serde_json::from_str(&text).ok()?;
        let now = atif::now_ms();
        projected["projected_at_ms"] = serde_json::json!(now);
        projected["observation_is_fresh"] = serde_json::json!(observation.is_fresh_at(now));
        if let Some(reference) = output
            .get("input_ref")
            .and_then(serde_json::Value::as_str)
            .filter(|value| value.len() <= 80)
        {
            projected["input_ref"] = serde_json::json!(reference);
        }
        loop {
            let text = serde_json::to_string(&projected).ok()?;
            if text.len() <= 8 * 1024 {
                return Some(text);
            }
            let subjects = projected["observation"]["subjects"].as_array_mut()?;
            subjects.pop()?;
            let omitted = projected["omitted_subjects"].as_u64().unwrap_or(0) + 1;
            projected["omitted_subjects"] = serde_json::json!(omitted);
            projected["context_truncated"] = serde_json::json!(true);
        }
    } else if output.get("error").is_some() {
        serde_json::to_string(output)
            .ok()
            .filter(|text| text.len() <= 8 * 1024)
    } else {
        None
    }
}

impl Drop for Settings {
    fn drop(&mut self) {
        if let Some(binding) = &self.binding {
            binding.enabled.send_replace(false);
            binding.lookup.set_enabled(false);
        }
    }
}

/// Render a bounded observation without hiding units or unavailable scores.
pub fn summary(output: &serde_json::Value) -> String {
    if output.get("fixture").and_then(serde_json::Value::as_bool) == Some(true) {
        return "Brainstorm demo fixture. No service read.\n\nPublic key: fixture account\nRelevance: 0.8 · Raw influence: 0.0 · Coverage: unknown\nHouse perspective and response times are fixture values, not live evidence.".into();
    }
    if let Some(value) = output.get("observation") {
        if let Ok(observation) = serde_json::from_value::<Observation>(value.clone()) {
            let freshness = if observation.is_fresh_at(atif::now_ms()) {
                "fresh observation"
            } else {
                "expired observation"
            };
            let completeness = match observation.completeness {
                brainstorm_client::Completeness::Bounded => "bounded request",
                brainstorm_client::Completeness::Partial => "partial scores",
            };
            let mut text = format!(
                "Brainstorm house perspective · {completeness} · {freshness}\n\nSource: {}\nHouse key: {}\nSeparate HTTPS observation; the API does not bind or sign its effective observer.\nDiscovered: {} ms · Combined expiry: {} ms\n\n| Public key | Relevance | Raw influence | Coverage | Profile |\n| --- | --- | --- | --- | --- |\n",
                observation.house.origin,
                observation.house.pubkey,
                observation.house.discovered_at_ms,
                observation.expires_at_ms
            );
            for subject in &observation.subjects {
                let relevance = subject
                    .relevance
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unavailable".into());
                let influence = subject
                    .influence
                    .as_ref()
                    .map(|value| value.value.to_string())
                    .unwrap_or_else(|| "unavailable".into());
                let coverage = subject
                    .influence
                    .as_ref()
                    .map(|value| match value.coverage {
                        brainstorm_client::Coverage::Unknown => "unknown",
                        brainstorm_client::Coverage::Reported => "reported",
                    })
                    .unwrap_or("unavailable");
                text.push_str(&format!(
                    "| {} | {relevance} | {influence} | {coverage} | {} |\n",
                    subject.pubkey, subject.profile_url
                ));
            }
            if let Some(error) = &observation.enrichment_error {
                text.push_str(&format!("\nInfluence unavailable: {error}.\n"));
            }
            text.push_str("\nRelevance and raw continuous influence are separate units; neither is a signed 0–100 score.\n");
            for evidence in &observation.responses {
                text.push_str(&format!("\n{} · HTTP {} · {:?} · fetched {} ms · expires {} ms\nInput hash: {}\nOutput hash: {}\n", evidence.endpoint, evidence.status, evidence.requested_algorithm, evidence.fetched_at_ms, evidence.expires_at_ms, evidence.input_digest, evidence.output_digest));
            }
            return text;
        }
    }
    if let Some(value) = output.get("discovery") {
        if let Ok(discovery) = serde_json::from_value::<Discovery>(value.clone()) {
            return format!(
                "Brainstorm discovery from {}\nHouse key: {}\nSearch available: {} · Rank available: {}\nSeparate HTTPS identity observation at {} ms; not a signed or atomic observer binding.\nDiscovery expiry: {} ms.",
                discovery.house.origin,
                discovery.house.pubkey,
                discovery.search_supported,
                discovery.rank_supported,
                discovery.house.discovered_at_ms,
                discovery.expires_at_ms
            );
        }
    }
    match output.get("error").and_then(serde_json::Value::as_str) {
        Some(error) => match output.get("recipient").and_then(serde_json::Value::as_str) {
            Some(recipient) => format!("Brainstorm lookup to {recipient}: {error}"),
            None => error.into(),
        },
        None => "Brainstorm result unavailable.".into(),
    }
}

#[cfg(test)]
mod tests;

impl Settings {
    pub(crate) fn demo_view(&self) -> coder_ui::demo::brainstorm::Settings {
        use coder_ui::demo::brainstorm as view;
        view::Settings {
            preferences: view::Preferences {
                enabled: self.preferences.enabled,
                origin: self.preferences.origin.clone(),
                perspective: view::Perspective::House,
            },
            focus: match self.focus {
                Focus::Origin => view::Focus::Origin,
                Focus::Test => view::Focus::Test,
                Focus::Save => view::Focus::Save,
                Focus::Cancel => view::Focus::Cancel,
            },
            error: self.error.clone(),
            connection: crate::demo::connection(&self.connection),
            discovery: self.discovery.as_ref().map(|d| view::Discovery {
                house: view::House {
                    pubkey: d.house.pubkey.clone(),
                    discovered_at_ms: d.house.discovered_at_ms,
                },
            }),
            check_requested: self.check_requested,
            save_requested: self.save_requested,
            fixture: self.fixture,
            draft: crate::demo::draft(&self.draft),
        }
    }
}
