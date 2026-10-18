//! Agent wakes and grants (#12044). Any saved agent whose definition
//! declares wakes gets a wake engine on the host that runs its chat, keyed by
//! the agent's definition ID: timed check-ins, event wakes, the watchdog over
//! its own turn (#12029) and the stall ladder (#12032). Grants are the powers
//! the account owner gives an agent over other chats; the host enforces them
//! on every command. The same never-list applies to every agent. Nothing here
//! knows any agent by name.

use serde::{Deserialize, Serialize};

/// The default and allowed check-in intervals, in minutes. The default
/// keeps a visible status in the agent's chat at least every 10 minutes.
pub const DEFAULT_EVERY_MINUTES: u32 = 10;
pub const MIN_EVERY_MINUTES: u32 = 5;
pub const MAX_EVERY_MINUTES: u32 = 24 * 60;
/// How long the agent's own turn may go with no output before the host
/// stops it and checks in (#12029), and the allowed range.
pub const DEFAULT_STUCK_MINUTES: u32 = 10;
pub const MIN_STUCK_MINUTES: u32 = 2;
pub const MAX_STUCK_MINUTES: u32 = 120;

fn default_stuck_minutes() -> u32 {
    DEFAULT_STUCK_MINUTES
}

/// How long a managed chat or a claim may go with no progress before the
/// host raises it, once per stall (#12032), and the allowed range.
pub const DEFAULT_STALL_MINUTES: u32 = 20;
pub const MIN_STALL_MINUTES: u32 = 5;
pub const MAX_STALL_MINUTES: u32 = 240;
/// Minutes between the rungs of the stall ladder: message, then interrupt
/// and restart with a brief, then tell the owner.
pub const STALL_RUNG_MINUTES: u32 = 10;

fn default_stall_minutes() -> u32 {
    DEFAULT_STALL_MINUTES
}

/// Something an agent can wake on, as its definition declares it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Wake {
    /// A chat it manages finished its turn.
    ChatFinished,
    /// A chat it manages stopped with an error.
    ChatErrored,
    /// A chat it manages is waiting for an answer.
    ChatNeedsAnswer,
    IssueOpened,
    IssueClosed,
    /// A claim with no chat had no newer comment for six hours.
    ClaimStale,
    /// A chat it manages has been working with no output (#12032).
    ChatStalled,
    /// A claim has shown no progress (#12032).
    ClaimNoProgress,
}

impl Wake {
    /// Every wake: what an agent gets when its definition lists none.
    pub const ALL: [Wake; 8] = [
        Wake::ChatFinished,
        Wake::ChatErrored,
        Wake::ChatNeedsAnswer,
        Wake::IssueOpened,
        Wake::IssueClosed,
        Wake::ClaimStale,
        Wake::ChatStalled,
        Wake::ClaimNoProgress,
    ];
}

/// A power the account owner grants an agent over other chats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Scope {
    /// Send a message to a chat or answer its question.
    #[serde(rename = "chat.message")]
    ChatMessage,
    /// Start a new chat.
    #[serde(rename = "chat.start")]
    ChatStart,
    /// Steer a running chat.
    #[serde(rename = "chat.steer")]
    ChatSteer,
    /// Stop a chat's turn.
    #[serde(rename = "chat.stop")]
    ChatStop,
    /// Submit a change to the landing queue.
    #[serde(rename = "land.submit")]
    LandSubmit,
    /// Dispatch issue work to a cloud pool.
    #[serde(rename = "work.dispatch")]
    WorkDispatch,
}

/// Powers no agent can be granted, whoever asks: money, keys, secrets,
/// deploys, and closing other agents' issues stay with the owner.
pub const NEVER: [&str; 5] = ["money", "keys", "secrets", "deploy", "close_issue"];

/// The refusal for a never-list scope.
pub const NEVER_REFUSAL: &str =
    "Money, keys, secrets, deploys and closing other agents' issues can't be granted to any agent.";

impl Scope {
    /// The four chat scopes: what "standing authority" grants.
    pub const CHAT: [Scope; 4] = [
        Scope::ChatMessage,
        Scope::ChatStart,
        Scope::ChatSteer,
        Scope::ChatStop,
    ];

    pub fn word(self) -> &'static str {
        match self {
            Scope::ChatMessage => "chat.message",
            Scope::ChatStart => "chat.start",
            Scope::ChatSteer => "chat.steer",
            Scope::ChatStop => "chat.stop",
            Scope::LandSubmit => "land.submit",
            Scope::WorkDispatch => "work.dispatch",
        }
    }

    /// A scope the owner asks to grant. Never-list words are refused with a
    /// plain reason, and anything else unknown is refused too.
    pub fn grantable(word: &str) -> Result<Scope, String> {
        let word = word.trim().to_ascii_lowercase();
        let bare = word.replace(['.', '-'], "_");
        if NEVER
            .iter()
            .any(|never| bare == *never || bare.starts_with(&format!("{never}_")))
            || bare == "deploys"
        {
            return Err(NEVER_REFUSAL.into());
        }
        [
            Scope::ChatMessage,
            Scope::ChatStart,
            Scope::ChatSteer,
            Scope::ChatStop,
            Scope::LandSubmit,
            Scope::WorkDispatch,
        ]
        .into_iter()
        .find(|s| s.word() == word)
        .ok_or_else(|| format!("\"{word}\" isn't a power an agent can be granted."))
    }
}

/// What a grant covers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Over {
    /// These chats, by ID.
    Chats(Vec<String>),
    /// Every chat in this project, by ID.
    Project(String),
    /// Every chat on the account.
    Account,
}

/// One grant: scopes over chats, a project, or the account.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Grant {
    pub id: String,
    pub scopes: Vec<Scope>,
    pub over: Over,
    /// Unix milliseconds.
    pub granted_at: i64,
}

impl Grant {
    /// Whether this grant lets the agent use `scope` on the chat `chat` in
    /// the project `project`.
    pub fn allows(&self, scope: Scope, chat: &str, project: Option<&str>) -> bool {
        self.scopes.contains(&scope)
            && match &self.over {
                Over::Account => true,
                Over::Project(p) => project == Some(p.as_str()),
                Over::Chats(chats) => chats.iter().any(|c| c == chat),
            }
    }

    /// Whether this is standing authority: every chat scope over the account.
    pub fn is_standing(&self) -> bool {
        self.over == Over::Account && Scope::CHAT.iter().all(|s| self.scopes.contains(s))
    }
}

/// What the owner sets for one agent's wake engine on this host: which chat
/// it runs in, how often it checks in, whether wake-ups are paused, and its
/// stuck and stall windows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeSettings {
    pub chat_id: Option<String>,
    pub every_minutes: u32,
    pub paused: bool,
    /// Minutes the chat's turn may go with no output before the host stops
    /// it and starts a check-in that says why.
    #[serde(default = "default_stuck_minutes")]
    pub stuck_minutes: u32,
    /// Minutes a managed chat or a claim may go with no progress before
    /// the host raises it (#12032).
    #[serde(default = "default_stall_minutes")]
    pub stall_minutes: u32,
}

impl Default for WakeSettings {
    fn default() -> Self {
        Self {
            chat_id: None,
            every_minutes: DEFAULT_EVERY_MINUTES,
            paused: false,
            stuck_minutes: DEFAULT_STUCK_MINUTES,
            stall_minutes: DEFAULT_STALL_MINUTES,
        }
    }
}

/// A saved agent's chat on this computer that can have wakes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakeCandidate {
    pub chat_id: String,
    pub title: String,
    /// The agent's definition ID.
    pub agent: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WakeReason {
    Schedule,
    Events,
    /// The host stopped the chat's own turn after it went the stuck window
    /// with no output (#12029).
    Stuck,
}

/// One line of an agent's decision log, oldest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DecisionEntry {
    /// The host started a check-in turn.
    #[serde(rename_all = "camelCase")]
    Wake {
        at: i64,
        id: String,
        reason: WakeReason,
        events: Vec<String>,
    },
    /// What the agent recorded about one check-in.
    #[serde(rename_all = "camelCase")]
    Decision {
        at: i64,
        saw: String,
        decided: String,
        did: String,
        /// The judgments the decision used, each with its receipt (#12042).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        judgments: Vec<JudgmentReceipt>,
    },
    /// How a check-in turn ended.
    #[serde(rename_all = "camelCase")]
    Outcome { at: i64, id: String, status: String },
    /// A standing authority turned on or off before grants (#11965). Kept so
    /// older logs read whole.
    #[serde(rename_all = "camelCase")]
    Authority {
        at: i64,
        granted: bool,
        scopes: Vec<String>,
    },
    /// The owner granted or revoked a grant.
    #[serde(rename_all = "camelCase")]
    Grant {
        at: i64,
        granted: bool,
        id: String,
        scopes: Vec<Scope>,
        over: Over,
    },
}

impl DecisionEntry {
    pub fn at(&self) -> i64 {
        match self {
            Self::Wake { at, .. }
            | Self::Decision { at, .. }
            | Self::Outcome { at, .. }
            | Self::Authority { at, .. }
            | Self::Grant { at, .. } => *at,
        }
    }
}

/// One agent's wake engine on this host, for the settings card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWakeStatus {
    /// The agent's definition ID.
    pub agent: String,
    /// Its name, for display only.
    pub agent_name: String,
    pub settings: WakeSettings,
    pub chat_title: Option<String>,
    /// What it wakes on.
    pub wakes: Vec<Wake>,
    pub grants: Vec<Grant>,
    /// Unix milliseconds of the next scheduled check-in, when one is due.
    pub next_check_in: Option<i64>,
    /// Something the owner should know, in plain words.
    pub problem: Option<String>,
    /// Unix ms when this computer turned check-ins on for the chosen chat by
    /// itself, because its agent's definition asks for them (#12030).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub turned_on_at: Option<i64>,
    /// The latest entries, oldest first.
    pub decisions: Vec<DecisionEntry>,
}

impl AgentWakeStatus {
    /// Whether the agent holds standing authority.
    pub fn standing(&self) -> bool {
        self.grants.iter().any(Grant::is_standing)
    }
}

/// Every agent's wake engine on this host.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WakesStatus {
    pub agents: Vec<AgentWakeStatus>,
    /// This computer's saved-agent chats that could run wakes.
    pub candidates: Vec<WakeCandidate>,
    /// Something the owner should know, in plain words.
    pub problem: Option<String>,
}

/// `SetAgentWakes`: the owner's settings for one agent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetWakes {
    pub agent: String,
    pub settings: WakeSettings,
}

/// `GrantAgent`: the owner grants an agent scopes. Scopes travel as words so
/// a never-list request is refused with a plain reason.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrantRequest {
    pub agent: String,
    pub scopes: Vec<String>,
    pub over: Over,
}

/// `RevokeAgentGrant`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RevokeGrant {
    pub agent: String,
    pub grant: String,
}

/// `RecordAgentDecision`, sent by an agent's own chat.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordDecision {
    pub origin_chat_id: Option<String>,
    pub saw: String,
    pub decided: String,
    pub did: String,
    /// The agent's whole working state as of this check-in, replacing the
    /// saved one (#12057). `None` keeps the saved state as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working: Option<Vec<WorkItem>>,
    /// Receipts the `judge` tool returned, for the judgments this
    /// decision used.
    #[serde(default)]
    pub judgments: Vec<JudgmentReceipt>,
}

/// The most items a working state keeps, and the longest note.
pub const MAX_WORK_ITEMS: usize = 64;
pub const MAX_WORK_NOTE: usize = 400;

/// Where one piece of work the agent tracks stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkStatus {
    /// A chat is on it.
    Working,
    /// It waits on something named in the note.
    Waiting,
    /// It was handed to another chat or person, not yet picked up.
    Handoff,
    /// It made no progress; the note says what was done about it.
    Stalled,
    /// Watched, nobody on it yet.
    Watching,
}

/// One piece of work in an agent's working state: which issue, which chat
/// owns it, and where it stands. The host stamps `since`, the time it first
/// saw this issue, chat and status together, so stall timers survive
/// restarts and context resets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkItem {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issue: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub chat: Option<String>,
    pub status: WorkStatus,
    #[serde(default)]
    pub note: String,
    /// Unix ms; set by the host.
    #[serde(default)]
    pub since: i64,
}

/// An agent's saved working state on the host that runs its chat, keyed by
/// its definition ID when it has one, else by its chat.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WorkingState {
    pub agent: Option<String>,
    pub chat: Option<String>,
    /// Unix ms of the last change.
    pub at: i64,
    pub items: Vec<WorkItem>,
}

/// One typed judgment a decision used: which question set was asked, what
/// it answered in a line, and the Decisions API's receipt for the call
/// (#12042). Any agent's decision log can carry these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JudgmentReceipt {
    /// The set's identifier, such as `openagents.run-progress.v1`.
    pub question_set: String,
    /// The decision's id in the Decisions API's response.
    pub decision: String,
    /// The model that answered.
    pub model: String,
    /// The door that answered (`clef`, `pylon:<slug>`, `jev`, `vertex`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door: Option<String>,
    /// The door's own receipt, when it gave one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<String>,
    /// The answers in one line, such as `progressed 0.12, activity stopped (0.70)`.
    #[serde(default)]
    pub answers: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_round_trip_with_their_kind() {
        let entry = DecisionEntry::Wake {
            at: 5,
            id: "w".into(),
            reason: WakeReason::Events,
            events: vec!["Chat \"Fix\" finished.".into()],
        };
        let wire = serde_json::to_value(&entry).unwrap();
        assert_eq!(wire["kind"], "wake");
        assert_eq!(wire["reason"], "events");
        assert_eq!(
            serde_json::from_value::<DecisionEntry>(wire).unwrap(),
            entry
        );
        assert_eq!(entry.at(), 5);
    }

    /// Logs written before grants still read whole (#12044).
    #[test]
    fn older_authority_entries_still_read() {
        let old = r#"{"kind":"authority","at":7,"granted":true,"scopes":["message","create","steer","stop"]}"#;
        let entry: DecisionEntry = serde_json::from_str(old).unwrap();
        assert!(matches!(
            entry,
            DecisionEntry::Authority {
                at: 7,
                granted: true,
                ..
            }
        ));
    }

    #[test]
    fn never_list_scopes_are_refused_by_name() {
        for word in [
            "money",
            "keys",
            "secrets",
            "deploy",
            "deploys",
            "close_issue",
            "close-issue",
            "deploy.production",
            "secrets.read",
        ] {
            assert_eq!(Scope::grantable(word), Err(NEVER_REFUSAL.into()), "{word}");
        }
        assert_eq!(Scope::grantable("chat.steer"), Ok(Scope::ChatSteer));
        assert!(Scope::grantable("chat.delete").is_err());
        // The wire format has no never-list scope either.
        assert!(serde_json::from_value::<Scope>(serde_json::json!("deploy")).is_err());
    }

    #[test]
    fn a_grant_covers_its_chats_project_or_account() {
        let grant = |over| Grant {
            id: "g".into(),
            scopes: vec![Scope::ChatMessage],
            over,
            granted_at: 0,
        };
        let chats = grant(Over::Chats(vec!["a".into()]));
        assert!(chats.allows(Scope::ChatMessage, "a", None));
        assert!(!chats.allows(Scope::ChatMessage, "b", None));
        assert!(!chats.allows(Scope::ChatStop, "a", None), "only its scopes");
        let project = grant(Over::Project("p".into()));
        assert!(project.allows(Scope::ChatMessage, "b", Some("p")));
        assert!(!project.allows(Scope::ChatMessage, "b", Some("q")));
        assert!(grant(Over::Account).allows(Scope::ChatMessage, "z", None));
        let standing = Grant {
            scopes: Scope::CHAT.to_vec(),
            ..grant(Over::Account)
        };
        assert!(standing.is_standing() && !chats.is_standing());
        assert_eq!(
            serde_json::to_value(&standing.over).unwrap(),
            serde_json::json!("account")
        );
        assert_eq!(
            serde_json::to_value(Over::Project("p".into())).unwrap(),
            serde_json::json!({"project": "p"})
        );
    }

    #[test]
    fn a_decision_keeps_its_judgments_and_older_lines_still_read() {
        let entry = DecisionEntry::Decision {
            at: 7,
            saw: "no output for 20 minutes".into(),
            decided: "message the chat".into(),
            did: "sent a nudge".into(),
            judgments: vec![JudgmentReceipt {
                question_set: "openagents.run-progress.v1".into(),
                decision: "dec_1".into(),
                model: "clef-flash".into(),
                door: Some("clef".into()),
                receipt: None,
                answers: "progressed 0.08, activity stopped (0.81)".into(),
            }],
        };
        let wire = serde_json::to_value(&entry).unwrap();
        assert_eq!(
            wire["judgments"][0]["questionSet"],
            "openagents.run-progress.v1"
        );
        assert_eq!(
            serde_json::from_value::<DecisionEntry>(wire).unwrap(),
            entry
        );
        let older = serde_json::json!({"kind": "decision", "at": 1, "saw": "a", "decided": "b", "did": "c"});
        let DecisionEntry::Decision { judgments, .. } =
            serde_json::from_value::<DecisionEntry>(older).unwrap()
        else {
            panic!("a decision");
        };
        assert!(judgments.is_empty());
        let plain = DecisionEntry::Decision {
            at: 1,
            saw: "a".into(),
            decided: "b".into(),
            did: "c".into(),
            judgments: Vec::new(),
        };
        assert!(
            serde_json::to_value(&plain)
                .unwrap()
                .get("judgments")
                .is_none()
        );
    }
}
