//! Shared agent definitions and NIP-AGENT payload validation. No execution or I/O.
use serde::{Deserialize, Serialize};
/// Runtime IDs resolve only to implementations installed and authorized by a host.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentSpec {
    pub name: String,
    pub description: String,
    pub instructions: String,
    pub runtime: String,
    pub model: Option<String>,
    pub access: Access,
    /// Runtimes to try, in order, when `runtime` isn't ready on the host.
    /// Not part of the agent's identity ([`AgentSpec::same_identity`]).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fallbacks: Vec<RuntimeChoice>,
    /// How the agent runs on its own. Not part of the agent's identity.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub autonomy: Option<Autonomy>,
}

/// How a saved agent runs on its own (#12029). Any agent can declare it;
/// hosts read it from the definition, never from the agent's name. Unknown
/// fields are ignored so later settings don't break older readers.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Autonomy {
    /// Hosts turn timed check-ins on for this agent's chat without being
    /// asked (#12030): when a computer has exactly one chat of an agent that
    /// declares this and no chat is chosen yet, it picks that chat.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub check_ins: bool,
    /// Minutes between check-ins when the host turns them on.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub every_minutes: Option<u32>,
    /// The agent asks for standing authority over other chats with its
    /// check-ins. The host grants it, logged, and the owner can turn it off.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub authority: bool,
    /// Minutes a chat or claim the agent manages may go with no progress
    /// before the host raises it (#12032). Hosts default to 20.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stall_minutes: Option<u32>,
    /// The longest one shell command may run, in seconds. The host hands it
    /// to the runtime (Claude Code's `BASH_DEFAULT_TIMEOUT_MS` and
    /// `BASH_MAX_TIMEOUT_MS`), so a probe with no timeout of its own can't
    /// hang the agent's turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub shell_timeout_secs: Option<u32>,
    /// What the agent wakes on (#12044); empty is every wake the host knows.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub wakes: Vec<WakeEvent>,
    /// Seconds events wait for quiet before they start one check-in.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub coalesce_secs: Option<u32>,
}

/// Something an agent can declare to wake on (#12044).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeEvent {
    ChatFinished,
    ChatErrored,
    ChatNeedsAnswer,
    IssueOpened,
    IssueClosed,
    ClaimStale,
    ChatStalled,
    ClaimNoProgress,
    /// A wake a newer writer knows and this reader doesn't; hosts skip it.
    #[serde(other)]
    Other,
}

/// The allowed range of [`Autonomy::coalesce_secs`].
pub const MAX_COALESCE_SECS: u32 = 30 * 60;
/// The most wakes an agent may list.
pub const MAX_WAKES: usize = 16;

/// A power the account owner grants an agent over other chats (#12044).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum GrantScope {
    #[serde(rename = "chat.message")]
    ChatMessage,
    #[serde(rename = "chat.start")]
    ChatStart,
    #[serde(rename = "chat.steer")]
    ChatSteer,
    #[serde(rename = "chat.stop")]
    ChatStop,
    #[serde(rename = "land.submit")]
    LandSubmit,
    #[serde(rename = "work.dispatch")]
    WorkDispatch,
}

/// Powers no agent can be granted: money, keys, secrets, deploys, and
/// closing other agents' issues stay with the owner.
pub const NEVER_GRANTED: [&str; 5] = ["money", "keys", "secrets", "deploy", "close_issue"];

/// The refusal for a never-list scope.
pub const NEVER_GRANTED_REFUSAL: &str =
    "Money, keys, secrets, deploys and closing other agents' issues can't be granted to any agent.";

impl GrantScope {
    pub const ALL: [GrantScope; 6] = [
        GrantScope::ChatMessage,
        GrantScope::ChatStart,
        GrantScope::ChatSteer,
        GrantScope::ChatStop,
        GrantScope::LandSubmit,
        GrantScope::WorkDispatch,
    ];

    pub fn word(self) -> &'static str {
        match self {
            GrantScope::ChatMessage => "chat.message",
            GrantScope::ChatStart => "chat.start",
            GrantScope::ChatSteer => "chat.steer",
            GrantScope::ChatStop => "chat.stop",
            GrantScope::LandSubmit => "land.submit",
            GrantScope::WorkDispatch => "work.dispatch",
        }
    }

    /// A scope someone asks to grant. Never-list words are refused with a
    /// plain reason, and anything else unknown is refused too.
    pub fn grantable(word: &str) -> Result<GrantScope, &'static str> {
        let word = word.trim().to_ascii_lowercase();
        let bare = word.replace(['.', '-'], "_");
        if bare == "deploys"
            || NEVER_GRANTED
                .iter()
                .any(|never| bare == *never || bare.starts_with(&format!("{never}_")))
        {
            return Err(NEVER_GRANTED_REFUSAL);
        }
        GrantScope::ALL
            .into_iter()
            .find(|s| s.word() == word)
            .ok_or("Choose powers an agent can be granted.")
    }
}

/// What a grant covers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantOver {
    /// These chats, by ID.
    Chats(Vec<String>),
    /// Every chat in this project, by ID.
    Project(String),
    /// Every chat on the account.
    Account,
}

/// The most chats one grant may name.
pub const MAX_GRANT_CHATS: usize = 64;

impl GrantOver {
    pub fn validate(&self) -> Result<(), &'static str> {
        let id =
            |s: &String| !s.trim().is_empty() && s.len() <= 128 && !s.chars().any(char::is_control);
        match self {
            GrantOver::Account => Ok(()),
            GrantOver::Project(p) if id(p) => Ok(()),
            GrantOver::Chats(c)
                if !c.is_empty() && c.len() <= MAX_GRANT_CHATS && c.iter().all(id) =>
            {
                Ok(())
            }
            _ => Err("Choose the chats or the project the grant covers."),
        }
    }
}

/// One grant the account owner gave an agent. Granted powers belong to the
/// account, not the definition: a fork asks its own owner for its grants.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentGrant {
    pub id: String,
    pub scopes: Vec<GrantScope>,
    pub over: GrantOver,
    /// Unix milliseconds.
    pub granted_at: i64,
}

/// The most UTF-8 bytes a saved agent's `instructions` may hold, skills
/// folded in. It was 16,384 until 2026-10-18 (#12114); a reader from
/// before refuses a longer definition as invalid.
/// Every limit on instructions (the catalog server, hosts, the Lab and
/// mobile forms) reads this. It can't pass 64 KiB without raising the most
/// a native text field holds (`rust_native::input::MAX_INPUT_VALUE_BYTES`).
pub const MAX_INSTRUCTIONS_BYTES: usize = 65_536;

/// The allowed range of [`Autonomy::every_minutes`].
pub const MIN_CHECK_IN_MINUTES: u32 = 5;
pub const MAX_CHECK_IN_MINUTES: u32 = 24 * 60;
/// The allowed range of [`Autonomy::stall_minutes`].
pub const MIN_STALL_MINUTES: u32 = 5;
pub const MAX_STALL_MINUTES: u32 = 240;
/// The allowed range of [`Autonomy::shell_timeout_secs`].
pub const MIN_SHELL_TIMEOUT_SECS: u32 = 10;
pub const MAX_SHELL_TIMEOUT_SECS: u32 = 3600;

/// One runtime the host may use for an agent, with its model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeChoice {
    pub runtime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

/// `runtime` value meaning "the host's own default runtime".
pub const ANY_RUNTIME: &str = "any";
/// Most fallback runtimes an agent may list.
pub const MAX_FALLBACKS: usize = 4;

fn valid_runtime(runtime: &str) -> bool {
    !runtime.is_empty()
        && runtime.len() <= 128
        && runtime
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn valid_model(model: Option<&String>) -> bool {
    model.is_none_or(|m| !m.is_empty() && m.len() <= 256 && !m.chars().any(char::is_control))
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Access {
    HostDefault,
    ReadOnly,
    WorkspaceWrite,
}

impl AgentSpec {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.name.trim().is_empty()
            || self.name.len() > 80
            || self.name.chars().any(char::is_control)
        {
            return Err("Enter a short name.");
        }
        if self.description.len() > 512 || self.description.chars().any(char::is_control) {
            return Err("Enter a shorter description.");
        }
        if self.instructions.trim().is_empty() {
            return Err("Add instructions.");
        }
        if self.instructions.contains('\0') {
            return Err("Remove the NUL character from the instructions.");
        }
        if self.instructions.len() > MAX_INSTRUCTIONS_BYTES {
            return Err(
                "Shorten the instructions: with skills, they may hold at most 65,536 bytes.",
            );
        }
        if !valid_runtime(&self.runtime) {
            return Err("Choose a supported runtime.");
        }
        if !valid_model(self.model.as_ref()) {
            return Err("Choose a supported model.");
        }
        if self.runtime == "sdk" && (self.model.is_none() || self.access != Access::HostDefault) {
            return Err("Choose an installed runner and this computer's access settings.");
        }
        if self.runtime == ANY_RUNTIME && self.model.is_some() {
            return Err("Choose a runtime before choosing a model.");
        }
        if let Some(a) = &self.autonomy
            && a.shell_timeout_secs
                .is_some_and(|t| !(MIN_SHELL_TIMEOUT_SECS..=MAX_SHELL_TIMEOUT_SECS).contains(&t))
        {
            return Err("Choose a shell timeout between 10 seconds and an hour.");
        }
        if let Some(a) = &self.autonomy
            && a.every_minutes
                .is_some_and(|m| !(MIN_CHECK_IN_MINUTES..=MAX_CHECK_IN_MINUTES).contains(&m))
        {
            return Err("Choose check-ins between every 5 minutes and once a day.");
        }
        if let Some(a) = &self.autonomy
            && a.stall_minutes
                .is_some_and(|m| !(MIN_STALL_MINUTES..=MAX_STALL_MINUTES).contains(&m))
        {
            return Err("Choose a stall window between 5 minutes and 4 hours.");
        }
        if let Some(a) = &self.autonomy
            && (a.wakes.len() > MAX_WAKES || a.coalesce_secs.is_some_and(|c| c > MAX_COALESCE_SECS))
        {
            return Err("Choose fewer wakes or a shorter wait.");
        }
        if self.fallbacks.len() > MAX_FALLBACKS {
            return Err("Choose fewer fallback runtimes.");
        }
        for (i, f) in self.fallbacks.iter().enumerate() {
            if !valid_runtime(&f.runtime)
                || f.runtime == "sdk"
                || f.runtime == ANY_RUNTIME
                || f.runtime == self.runtime
                || self.fallbacks[..i].iter().any(|e| e.runtime == f.runtime)
            {
                return Err("Choose different fallback runtimes.");
            }
            if !valid_model(f.model.as_ref()) {
                return Err("Choose a supported model.");
            }
        }
        Ok(())
    }

    /// The runtimes to try, in order. Empty when the agent leaves it to the
    /// host (`runtime: "any"` and no fallbacks).
    pub fn runtime_order(&self) -> Vec<RuntimeChoice> {
        (self.runtime != ANY_RUNTIME)
            .then(|| RuntimeChoice {
                runtime: self.runtime.clone(),
                model: self.model.clone(),
            })
            .into_iter()
            .chain(self.fallbacks.iter().cloned())
            .collect()
    }

    /// Whether two specs describe the same agent. The runtime, model,
    /// fallbacks and autonomy are settings of the agent, not part of who it is.
    pub fn same_identity(&self, other: &Self) -> bool {
        self.name == other.name
            && self.description == other.description
            && self.instructions == other.instructions
            && self.access == other.access
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AgentDefinition {
    pub id: String,
    pub revision: u64,
    pub spec: AgentSpec,
    pub deleted: bool,
}
pub fn valid_id(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

pub const SCHEMA: &str = "openagents.agent.definition.v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub schema: String,
    pub definition: AgentDefinition,
}
impl Profile {
    pub fn new(definition: AgentDefinition) -> Result<Self, &'static str> {
        let p = Self {
            schema: SCHEMA.into(),
            definition,
        };
        p.validate()?;
        Ok(p)
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA || !valid_id(&self.definition.id) || self.definition.revision == 0
        {
            return Err("Invalid agent profile.");
        }
        self.definition.spec.validate()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn spec() -> AgentSpec {
        AgentSpec {
            name: "Foreman".into(),
            description: String::new(),
            instructions: "Oversee the work.".into(),
            runtime: "claude-code".into(),
            model: Some("claude-opus-5-5".into()),
            access: Access::HostDefault,
            fallbacks: vec![],
            autonomy: None,
        }
    }
    fn choice(runtime: &str) -> RuntimeChoice {
        RuntimeChoice {
            runtime: runtime.into(),
            model: None,
        }
    }

    #[test]
    fn instructions_fit_the_raised_limit_and_a_longer_text_says_so() {
        let mut s = spec();
        // Past the old 16,384-byte limit (#12114).
        s.instructions = "a".repeat(16_385);
        s.validate().unwrap();
        s.instructions = "a".repeat(MAX_INSTRUCTIONS_BYTES);
        s.validate().unwrap();
        s.instructions.push('a');
        let err = s.validate().unwrap_err();
        assert!(err.contains("Shorten the instructions"), "{err}");
        let limit = MAX_INSTRUCTIONS_BYTES.to_string();
        let (thousands, rest) = limit.split_at(limit.len() - 3);
        assert!(err.contains(&format!("{thousands},{rest} bytes")), "{err}");
        for empty in ["", "  "] {
            s.instructions = empty.into();
            assert_eq!(s.validate(), Err("Add instructions."));
        }
        s.instructions = "a\0b".into();
        assert!(s.validate().unwrap_err().contains("NUL"));
    }

    #[test]
    fn fallbacks_are_ordered_optional_and_omitted_when_empty() {
        let mut s = spec();
        let json = serde_json::to_value(&s).unwrap();
        assert!(json.get("fallbacks").is_none());
        s.fallbacks = vec![choice("codex"), choice("opencode")];
        s.validate().unwrap();
        let order: Vec<_> = s.runtime_order().into_iter().map(|c| c.runtime).collect();
        assert_eq!(order, ["claude-code", "codex", "opencode"]);
        let back: AgentSpec = serde_json::from_value(serde_json::to_value(&s).unwrap()).unwrap();
        assert_eq!(back, s);
        s.runtime = ANY_RUNTIME.into();
        s.model = None;
        s.fallbacks.clear();
        s.validate().unwrap();
        assert!(s.runtime_order().is_empty());
    }

    #[test]
    fn autonomy_is_an_optional_setting_with_a_bounded_shell_timeout() {
        let mut s = spec();
        assert!(serde_json::to_value(&s).unwrap().get("autonomy").is_none());
        let plain = s.clone();
        s.autonomy = Some(Autonomy {
            check_ins: true,
            every_minutes: Some(10),
            authority: true,
            stall_minutes: Some(10),
            shell_timeout_secs: Some(120),
            ..Default::default()
        });
        s.validate().unwrap();
        assert!(s.same_identity(&plain), "a setting, not identity");
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(json["autonomy"]["shell_timeout_secs"], 120);
        assert_eq!(json["autonomy"]["check_ins"], true);
        assert_eq!(serde_json::from_value::<AgentSpec>(json).unwrap(), s);
        for bad in [0, 9, 3601] {
            s.autonomy = Some(Autonomy {
                shell_timeout_secs: Some(bad),
                ..Autonomy::default()
            });
            assert!(s.validate().is_err(), "{bad}");
        }
        for bad in [4, 1441] {
            s.autonomy = Some(Autonomy {
                every_minutes: Some(bad),
                ..Autonomy::default()
            });
            assert!(s.validate().is_err(), "{bad}");
        }
        for bad in [4, 241] {
            s.autonomy = Some(Autonomy {
                stall_minutes: Some(bad),
                ..Autonomy::default()
            });
            assert!(s.validate().is_err(), "{bad}");
        }
        // Unset flags stay off the wire, and unknown settings are ignored.
        let shell_only = serde_json::json!({"shell_timeout_secs": 60, "later": 1});
        let read: Autonomy = serde_json::from_value(shell_only).unwrap();
        assert!(!read.check_ins && !read.authority);
        assert_eq!(
            serde_json::to_value(&read).unwrap(),
            serde_json::json!({"shell_timeout_secs": 60})
        );
    }

    #[test]
    fn wakes_are_declared_data_and_grants_refuse_the_never_list() {
        let mut s = spec();
        s.autonomy = Some(Autonomy {
            wakes: vec![WakeEvent::ChatFinished, WakeEvent::ClaimNoProgress],
            coalesce_secs: Some(60),
            ..Autonomy::default()
        });
        s.validate().unwrap();
        let json = serde_json::to_value(&s).unwrap();
        assert_eq!(
            json["autonomy"]["wakes"],
            serde_json::json!(["chat_finished", "claim_no_progress"])
        );
        // A wake this reader doesn't know is skipped, not a broken definition.
        let newer: Autonomy =
            serde_json::from_value(serde_json::json!({"wakes": ["chat_finished", "pr_merged"]}))
                .unwrap();
        assert_eq!(newer.wakes, [WakeEvent::ChatFinished, WakeEvent::Other]);
        for word in [
            "money",
            "keys",
            "secrets",
            "deploy",
            "deploys",
            "close_issue",
            "secrets.read",
        ] {
            assert_eq!(
                GrantScope::grantable(word),
                Err(NEVER_GRANTED_REFUSAL),
                "{word}"
            );
        }
        assert_eq!(GrantScope::grantable("chat.stop"), Ok(GrantScope::ChatStop));
        assert!(GrantScope::grantable("chat.delete").is_err());
        assert!(serde_json::from_value::<GrantScope>(serde_json::json!("deploy")).is_err());
        assert!(GrantOver::Chats(vec![]).validate().is_err());
        assert!(GrantOver::Project("p".into()).validate().is_ok());
    }

    #[test]
    fn fallbacks_reject_repeats_runners_and_long_lists() {
        for fallbacks in [
            vec![choice("claude-code")],
            vec![choice("codex"), choice("codex")],
            vec![choice("sdk")],
            vec![choice(ANY_RUNTIME)],
            vec![choice("sh -c x")],
            vec![choice("a"), choice("b"), choice("c"), choice("d"), choice("e")],
        ] {
            let mut s = spec();
            s.fallbacks = fallbacks;
            assert!(s.validate().is_err());
        }
        let mut s = spec();
        s.runtime = ANY_RUNTIME.into();
        assert!(s.validate().is_err(), "a model needs a runtime");
    }

    #[test]
    fn runtime_and_model_are_not_identity() {
        let a = spec();
        let mut b = a.clone();
        b.runtime = "codex".into();
        b.model = None;
        b.fallbacks = vec![choice("opencode")];
        assert!(a.same_identity(&b));
        b.instructions = "Something else".into();
        assert!(!a.same_identity(&b));
    }
}
