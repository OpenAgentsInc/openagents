//! Ephemeral local voice contracts. These must never enter the document journal.
use serde::{Deserialize, Serialize};
pub mod remote;

pub const MAX_AUDIO_BYTES: usize = 16_384;
pub const MEDIA_QUEUE_FRAMES: usize = 8;
pub const MAX_TRANSCRIPT_BYTES: usize = 32_768;

/// Id prefix of the chat that hosts a voice orchestrator session. Only each
/// device's current one takes a chat-list row ([`listed_orchestrators`]),
/// and chats it creates are top-level sessions rather than its side chats.
pub const ORCHESTRATOR_CHAT_PREFIX: &str = "voice-orchestrator-";
/// Title of every orchestrator chat; transcripts never trigger a titler.
pub const ORCHESTRATOR_CHAT_TITLE: &str = "Voice transcript";
/// Titles earlier builds gave orchestrator chats; a call renames them.
pub const EARLIER_ORCHESTRATOR_TITLES: &[&str] = &["Voice session"];
/// Codex voice styles offered before a host reports its own list.
pub const DEFAULT_VOICES: &[&str] = &[
    "juniper", "maple", "spruce", "ember", "vale", "breeze", "arbor", "sol", "cove",
];

pub fn is_orchestrator_chat(chat_id: &str) -> bool {
    chat_id.starts_with(ORCHESTRATOR_CHAT_PREFIX)
}

/// `device`'s current orchestrator chat: its newest non-archived one. Calls
/// on that device run in it; older segments stay out of every list.
pub fn current_orchestrator<'a>(chats: &'a [crate::Chat], device: &str) -> Option<&'a crate::Chat> {
    chats
        .iter()
        .filter(|c| is_orchestrator_chat(&c.id) && c.device_id == device && !c.archived)
        .max_by_key(|c| c.created_at)
}

/// Ids of every device's current orchestrator chat, the voice chats the chat
/// lists show beside top-level sessions (see [`crate::Chat::is_listed`]).
pub fn listed_orchestrators(chats: &[crate::Chat]) -> std::collections::HashSet<&str> {
    let mut newest: std::collections::HashMap<&str, &crate::Chat> = Default::default();
    for chat in chats
        .iter()
        .filter(|c| is_orchestrator_chat(&c.id) && !c.archived)
    {
        let slot = newest.entry(chat.device_id.as_str()).or_insert(chat);
        // Ties go to the later row, as `current_orchestrator` picks.
        if chat.created_at >= slot.created_at {
            *slot = chat;
        }
    }
    newest.into_values().map(|c| c.id.as_str()).collect()
}

/// The id a voice chat publishes under in the account's session index,
/// which takes bare UUIDs: the orchestrator's id without its prefix.
pub fn published_id(chat_id: &str) -> &str {
    chat_id
        .strip_prefix(ORCHESTRATOR_CHAT_PREFIX)
        .unwrap_or(chat_id)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceRejection {
    VoiceAgentRequired,
    Disabled,
    Unsupported,
    RemoteHost,
    WrongHarness,
    ChatgptRequired,
    IncludedUsageUnavailable,
    CreditExclusionUnverified,
    AudioFormatUnverified,
    DuplexUnverified,
    Busy,
    InvalidLease,
    StaleGeneration,
    Overflow,
    DeviceUnavailable,
    MicrophonePermissionDenied,
    MicrophoneMetadataMissing,
    Protocol,
    NativeRuntimeUnavailable,
}

/// The chat open on the calling device. This is context, never a routing target.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceChatContext {
    pub chat_id: String,
    pub title: Option<String>,
}
impl VoiceChatContext {
    pub fn new(chat_id: String, title: Option<String>) -> Option<Self> {
        let value = Self {
            chat_id,
            title: title.map(|s| s.chars().filter(|c| !c.is_control()).take(200).collect()),
        };
        value.valid().then_some(value)
    }
    pub fn valid(&self) -> bool {
        !self.chat_id.is_empty()
            && self.chat_id.len() <= 256
            && !self.chat_id.chars().any(char::is_control)
            && self
                .title
                .as_ref()
                .is_none_or(|t| t.len() <= 1024 && !t.chars().any(char::is_control))
    }
}

#[cfg(test)]
mod context_tests {
    use super::*;
    #[test]
    fn voice_context_is_bounded_and_does_not_choose_a_target() {
        let context = VoiceChatContext::new("chat-x".into(), Some("A\n".repeat(2000))).unwrap();
        assert_eq!(context.title.as_ref().unwrap().len(), 200);
        assert_eq!(
            serde_json::from_value::<VoiceChatContext>(serde_json::to_value(&context).unwrap())
                .unwrap(),
            context
        );
        assert!(VoiceChatContext::new("bad\nchat".into(), None).is_none());
        assert!(
            !VoiceChatContext {
                chat_id: "x".repeat(257),
                title: None
            }
            .valid()
        );
        assert!(
            !VoiceChatContext {
                chat_id: "chat".into(),
                title: Some("x".repeat(1025))
            }
            .valid()
        );
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceEligibility {
    pub available: bool,
    #[serde(default)]
    pub reason: Option<VoiceRejection>,
    /// Provider-reported permission; None is unknown, never permission.
    #[serde(default)]
    pub ordinary_usage_allowed: Option<bool>,
    pub credits_excluded: bool,
    #[serde(default)]
    pub format: Option<VoiceFormat>,
    #[serde(default)]
    pub duplex_verified: bool,
    /// Native subscription WebRTC transport; PCM never leaves the voice helper.
    #[serde(default)]
    pub native_webrtc: bool,
    #[serde(default)]
    pub voices: Vec<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoicePhase {
    #[default]
    Closed,
    Checking,
    Starting,
    Active,
    Stopping,
    Failed,
}
impl VoicePhase {
    pub fn replaces_composer(self) -> bool {
        matches!(self, Self::Starting | Self::Active | Self::Stopping)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceWork {
    #[default]
    Idle,
    Working,
    AwaitingInput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceSnapshot {
    pub session_id: String,
    pub chat_id: String,
    pub generation: u64,
    pub phase: VoicePhase,
    pub muted: bool,
    pub playing: bool,
    pub work: VoiceWork,
    #[serde(default)]
    pub reason: Option<VoiceRejection>,
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub voices: Vec<String>,
}

/// Debug deliberately omits the bearer secret. Not a durable preference.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceLease {
    pub session_id: String,
    pub generation: u64,
    pub token: String,
}
impl std::fmt::Debug for VoiceLease {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VoiceLease")
            .field("session_id", &self.session_id)
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceEncoding {
    Pcm16Le,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceFormat {
    pub encoding: VoiceEncoding,
    pub sample_rate: u32,
    pub channels: u16,
}
impl VoiceFormat {
    pub fn validate(&self, bytes: usize) -> bool {
        self.sample_rate == 24_000
            && self.channels == 1
            && bytes > 0
            && bytes <= MAX_AUDIO_BYTES
            && bytes.is_multiple_of(2)
    }
}

/// Base64 PCM. No Debug implementation: audio must never be logged implicitly.
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceFrame {
    pub generation: u64,
    pub sequence: u64,
    pub format: VoiceFormat,
    pub data: String,
    #[serde(default)]
    pub item_id: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VoiceRole {
    User,
    Assistant,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceTranscript {
    pub session_id: String,
    pub item_id: String,
    pub role: VoiceRole,
    pub text: String,
    #[serde(default)]
    pub promoted_message_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum VoiceEvent {
    Snapshot {
        snapshot: VoiceSnapshot,
    },
    Audio {
        frame: VoiceFrame,
    },
    Partial {
        generation: u64,
        #[serde(default)]
        item_id: Option<String>,
        text: String,
    },
    Final {
        transcript: VoiceTranscript,
    },
    InvalidatePlayout {
        generation: u64,
        item_id: String,
    },
    Levels {
        generation: u64,
        microphone: u16,
        speaker: u16,
    },
    Closed {
        generation: u64,
        reason: Option<VoiceRejection>,
    },
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_permission_is_not_authorization_and_secrets_are_not_debugged() {
        let old = r#"{"available":false,"creditsExcluded":false}"#;
        let eligibility: VoiceEligibility = serde_json::from_str(old).unwrap();
        assert_eq!(eligibility.ordinary_usage_allowed, None);
        let lease = VoiceLease {
            session_id: "id".into(),
            generation: 1,
            token: "SECRET".into(),
        };
        assert!(!format!("{lease:?}").contains("SECRET"));
        assert!(serde_json::from_str::<VoiceEncoding>("\"unknown\"").is_err());
        for phase in [
            VoicePhase::Starting,
            VoicePhase::Active,
            VoicePhase::Stopping,
        ] {
            assert!(phase.replaces_composer());
        }
        assert!(!VoicePhase::Checking.replaces_composer());
    }

    #[test]
    fn orchestrator_chats_are_never_top_level_sessions() {
        let mut chat: crate::Chat = serde_json::from_value(serde_json::json!({
            "id": format!("{ORCHESTRATOR_CHAT_PREFIX}1"),
            "deviceId": "device",
            "title": null,
            "archived": false,
            "cwd": null,
            "branch": null,
            "checkoutId": null,
            "config": null,
            "lastMessagePreview": null,
            "lastMessageAt": null,
            "createdAt": "2026-10-01T00:00:00Z",
        }))
        .unwrap();
        assert!(!chat.is_top_level());
        chat.id = "ordinary".into();
        assert!(chat.is_top_level());
        // A side chat from an older build lists as an ordinary chat.
        chat.parent_chat_id = Some("parent".into());
        assert!(chat.is_top_level());
    }

    fn orchestrator(id: &str, device: &str, created: &str, archived: bool) -> crate::Chat {
        serde_json::from_value(serde_json::json!({
            "id": format!("{ORCHESTRATOR_CHAT_PREFIX}{id}"),
            "deviceId": device,
            "title": ORCHESTRATOR_CHAT_TITLE,
            "archived": archived,
            "createdAt": created,
        }))
        .unwrap()
    }

    #[test]
    fn only_each_devices_newest_orchestrator_is_listed() {
        let chats = vec![
            orchestrator("old", "mac", "2026-10-01T00:00:00Z", false),
            orchestrator("new", "mac", "2026-10-02T00:00:00Z", false),
            orchestrator("gone", "mac", "2026-10-03T00:00:00Z", true),
            orchestrator("other", "pc", "2026-10-01T00:00:00Z", false),
        ];
        let listed = listed_orchestrators(&chats);
        let id = |s: &str| format!("{ORCHESTRATOR_CHAT_PREFIX}{s}");
        assert_eq!(listed.len(), 2);
        assert!(listed.contains(id("new").as_str()) && listed.contains(id("other").as_str()));
        assert_eq!(current_orchestrator(&chats, "mac").unwrap().id, id("new"));
        assert!(chats[1].is_listed(&listed) && !chats[0].is_listed(&listed));
        assert!(!chats[2].is_listed(&listed));
        // Lists show it, but it stays out of everything top-level gates.
        assert!(!chats[1].is_top_level());
        assert_eq!(published_id(&id("new")), "new");
        assert_eq!(published_id("plain"), "plain");
    }
}
