//! Original local Brainstorm settings and labeled fixture observations.
use super::{Draft, Key, KeyCode, plugins::Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
pub const PLUGIN: &str = "brainstorm";
pub const USAGE: &str =
    "/brainstorm search <public query> · /brainstorm rank <hex-or-npub> [more keys]";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Perspective {
    #[default]
    House,
}
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Preferences {
    pub enabled: bool,
    pub origin: String,
    pub perspective: Perspective,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            enabled: false,
            origin: "https://api.brainstorm.world".into(),
            perspective: Perspective::House,
        }
    }
}
impl Preferences {
    fn valid(&self) -> bool {
        self.origin.len() <= 2048
            && !self
                .origin
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            && url::Url::parse(&self.origin).is_ok_and(|u| {
                u.scheme() == "https"
                    && u.host_str().is_some()
                    && u.username().is_empty()
                    && u.password().is_none()
                    && u.query().is_none()
                    && u.fragment().is_none()
            })
    }
}
#[derive(Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Focus {
    Origin,
    Test,
    Save,
    Cancel,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct House {
    pub pubkey: String,
    pub discovered_at_ms: u64,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Discovery {
    pub house: House,
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub preferences: Preferences,
    pub focus: Focus,
    pub error: Option<String>,
    pub connection: Connection,
    pub discovery: Option<Discovery>,
    pub check_requested: bool,
    pub save_requested: bool,
    pub fixture: bool,
    pub draft: Draft,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            preferences: Default::default(),
            focus: Focus::Origin,
            error: None,
            connection: Default::default(),
            discovery: None,
            check_requested: false,
            save_requested: false,
            fixture: false,
            draft: Default::default(),
        }
    }
}
impl Settings {
    pub fn status(&self) -> &str {
        if !self.preferences.enabled {
            "Disabled"
        } else {
            "Demo fixture"
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
    pub fn handle(&mut self, key: Key) -> bool {
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
    pub fn edited_preferences(&self) -> Result<Preferences, &'static str> {
        let mut preferences = Preferences {
            origin: self.draft.text.trim().into(),
            ..self.preferences.clone()
        };
        if !preferences.valid()
            || url::Url::parse(&preferences.origin).is_ok_and(|u| u.path() != "/" && u.path() != "")
        {
            return Err(
                "Enter an HTTPS origin without credentials, a path prefix, query, or fragment.",
            );
        }
        preferences.origin = preferences.origin.trim_end_matches('/').to_owned();
        Ok(preferences)
    }
}
pub const FIXTURE: &str = "Brainstorm demo fixture. No service read.\n\nPublic key: fixture account\nRelevance: 0.8 · Raw influence: 0.0 · Coverage: unknown\nHouse perspective and response times are fixture values, not live evidence.";

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
