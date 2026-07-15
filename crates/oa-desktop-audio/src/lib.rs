//! Native media-only boundary for OpenAgents Desktop persistent voice.
//! This crate owns bytes and device transport, never transcript, command,
//! Sync, storage-policy, retention, or outcome authority.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub mod native_transport;

pub const AUDIO_PROTOCOL_VERSION: &str = "openagents.audio.v1";
pub const MAX_AUDIO_PAYLOAD_BYTES: u64 = 24_000;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceIdentity {
    pub owner_ref: String,
    pub device_ref: String,
    pub thread_ref: String,
    pub session_ref: String,
    pub generation: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MediaHeader {
    ClientAudio(ClientAudioHeader),
    ServerTts(ServerTtsHeader),
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClientAudioHeader {
    pub schema: String,
    pub identity: VoiceIdentity,
    pub sequence: u64,
    pub codec: Codec,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub payload_length: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ServerTtsHeader {
    pub schema: String,
    pub identity: VoiceIdentity,
    pub sequence: u64,
    pub turn_ref: String,
    pub speech_ref: String,
    pub codec: Codec,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub payload_length: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec {
    PcmS16le,
    Opus,
}

fn valid_ref(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}
fn valid_identity(i: &VoiceIdentity) -> bool {
    i.generation > 0
        && valid_ref(&i.owner_ref)
        && valid_ref(&i.device_ref)
        && valid_ref(&i.thread_ref)
        && valid_ref(&i.session_ref)
}
fn common_valid(
    schema: &str,
    identity: &VoiceIdentity,
    sequence: u64,
    channels: u8,
    length: u64,
    digest: &str,
) -> bool {
    schema == AUDIO_PROTOCOL_VERSION
        && valid_identity(identity)
        && sequence <= 9_007_199_254_740_991
        && channels == 1
        && length <= MAX_AUDIO_PAYLOAD_BYTES
        && digest.len() == 64
        && digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl MediaHeader {
    pub fn validate(&self) -> bool {
        match self {
            Self::ClientAudio(h) => {
                common_valid(
                    &h.schema,
                    &h.identity,
                    h.sequence,
                    h.channels,
                    h.payload_length,
                    &h.sha256,
                ) && matches!(h.sample_rate_hz, 16_000 | 24_000 | 48_000)
            }
            Self::ServerTts(h) => {
                common_valid(
                    &h.schema,
                    &h.identity,
                    h.sequence,
                    h.channels,
                    h.payload_length,
                    &h.sha256,
                ) && valid_ref(&h.turn_ref)
                    && valid_ref(&h.speech_ref)
                    && matches!(h.sample_rate_hz, 24_000 | 48_000)
            }
        }
    }
}

pub fn decode_media_header(value: &serde_json::Value) -> Result<MediaHeader, String> {
    let header: MediaHeader = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    header
        .validate()
        .then_some(header)
        .ok_or_else(|| "invalid audio media header".to_string())
}

pub const HELPER_PROTOCOL_VERSION: u32 = 1;
pub const MAX_UNACKNOWLEDGED_PACKETS: u64 = 32;

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
pub enum HelperCommand {
    Start {
        protocol_version: u32,
        identity: VoiceIdentity,
        disclosure_ref: String,
        gateway_url: Option<String>,
        application_grant: Option<String>,
    },
    SetCapture {
        enabled: bool,
    },
    Ack {
        generation: u32,
        sequence: u64,
    },
    DeviceChanged,
    NetworkLost,
    NetworkRestored {
        generation: u32,
    },
    Play {
        generation: u32,
        sequence: u64,
        payload_length: u64,
        sha256: String,
    },
    Stop {
        reason: StopReason,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    Stop,
    Revoke,
    Replace,
    Suspend,
    Shutdown,
    ParentExit,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum HelperState {
    Idle,
    Live {
        generation: u32,
        next_sequence: u64,
        acknowledged_sequence: u64,
        capture_enabled: bool,
    },
    Offline {
        generation: u32,
    },
    Backpressured {
        generation: u32,
        unacknowledged: u64,
    },
    DeviceChanged {
        generation: u32,
    },
    Stopped {
        reason: String,
    },
    Refused {
        reason: String,
    },
}

#[derive(Debug)]
pub struct MediaLifecycle {
    state: HelperState,
}

impl Default for MediaLifecycle {
    fn default() -> Self {
        Self {
            state: HelperState::Idle,
        }
    }
}

impl MediaLifecycle {
    pub fn state(&self) -> &HelperState {
        &self.state
    }
    pub fn apply(&mut self, command: HelperCommand) -> &HelperState {
        match command {
            HelperCommand::Start {
                protocol_version,
                identity,
                disclosure_ref,
                ..
            } => {
                self.state = if protocol_version != HELPER_PROTOCOL_VERSION
                    || !valid_identity(&identity)
                    || !valid_ref(&disclosure_ref)
                {
                    HelperState::Refused {
                        reason: "invalid_start".into(),
                    }
                } else {
                    HelperState::Live {
                        generation: identity.generation,
                        next_sequence: 0,
                        acknowledged_sequence: 0,
                        capture_enabled: true,
                    }
                }
            }
            HelperCommand::SetCapture { enabled } => {
                if let HelperState::Live {
                    capture_enabled, ..
                } = &mut self.state
                {
                    *capture_enabled = enabled
                }
            }
            HelperCommand::Ack {
                generation,
                sequence,
            } => {
                if let HelperState::Live {
                    generation: active,
                    acknowledged_sequence,
                    ..
                } = &mut self.state
                {
                    if generation == *active {
                        *acknowledged_sequence = (*acknowledged_sequence).max(sequence)
                    } else {
                        self.state = HelperState::Refused {
                            reason: "stale_generation".into(),
                        }
                    }
                }
            }
            HelperCommand::DeviceChanged => {
                if let Some(generation) = self.generation() {
                    self.state = HelperState::DeviceChanged { generation }
                }
            }
            HelperCommand::NetworkLost => {
                if let Some(generation) = self.generation() {
                    self.state = HelperState::Offline { generation }
                }
            }
            HelperCommand::NetworkRestored { generation } => match self.generation() {
                Some(active) if active == generation => {
                    self.state = HelperState::Live {
                        generation,
                        next_sequence: 0,
                        acknowledged_sequence: 0,
                        capture_enabled: true,
                    }
                }
                _ => {
                    self.state = HelperState::Refused {
                        reason: "stale_generation".into(),
                    }
                }
            },
            HelperCommand::Play {
                generation,
                payload_length,
                sha256,
                ..
            } => {
                if self.generation() != Some(generation)
                    || payload_length > MAX_AUDIO_PAYLOAD_BYTES
                    || !valid_digest(&sha256)
                {
                    self.state = HelperState::Refused {
                        reason: "invalid_playback".into(),
                    }
                }
            }
            HelperCommand::Stop { reason } => {
                self.state = HelperState::Stopped {
                    reason: format!("{reason:?}").to_lowercase(),
                }
            }
        }
        &self.state
    }
    fn generation(&self) -> Option<u32> {
        match self.state {
            HelperState::Live { generation, .. }
            | HelperState::Offline { generation }
            | HelperState::Backpressured { generation, .. }
            | HelperState::DeviceChanged { generation } => Some(generation),
            _ => None,
        }
    }
    pub fn packetize(&mut self, pcm_mono_s16le: &[u8]) -> Result<(u64, String), &'static str> {
        match &mut self.state {
            HelperState::Live {
                generation,
                next_sequence,
                acknowledged_sequence,
                capture_enabled,
            } if *capture_enabled => {
                if pcm_mono_s16le.len() as u64 > MAX_AUDIO_PAYLOAD_BYTES {
                    return Err("payload_too_large");
                }
                let unacknowledged = next_sequence.saturating_sub(*acknowledged_sequence);
                if unacknowledged >= MAX_UNACKNOWLEDGED_PACKETS {
                    self.state = HelperState::Backpressured {
                        generation: *generation,
                        unacknowledged,
                    };
                    return Err("backpressure");
                }
                let sequence = *next_sequence;
                *next_sequence += 1;
                Ok((sequence, format!("{:x}", Sha256::digest(pcm_mono_s16le))))
            }
            _ => Err("capture_disabled"),
        }
    }
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Deserialize)]
    struct Corpus {
        cases: Vec<Case>,
    }
    #[derive(Deserialize)]
    struct Case {
        name: String,
        accept: bool,
        header: serde_json::Value,
    }
    #[test]
    fn matches_effect_golden_corpus() {
        let corpus: Corpus = serde_json::from_str(include_str!(
            "../../../fixtures/audio-contract/media-v1.json"
        ))
        .unwrap();
        for case in corpus.cases {
            assert_eq!(
                decode_media_header(&case.header).is_ok(),
                case.accept,
                "{}",
                case.name
            );
        }
    }

    fn identity(generation: u32) -> VoiceIdentity {
        VoiceIdentity {
            owner_ref: "owner".into(),
            device_ref: "device".into(),
            thread_ref: "thread".into(),
            session_ref: "session".into(),
            generation,
        }
    }

    #[test]
    fn capture_is_generation_fenced_mutable_and_backpressured() {
        let mut lifecycle = MediaLifecycle::default();
        lifecycle.apply(HelperCommand::Start {
            protocol_version: 1,
            identity: identity(7),
            disclosure_ref: "disclosure.v1".into(),
            gateway_url: None,
            application_grant: None,
        });
        let (sequence, digest) = lifecycle.packetize(&[0, 1, 2, 3]).unwrap();
        assert_eq!(sequence, 0);
        assert_eq!(digest.len(), 64);
        lifecycle.apply(HelperCommand::SetCapture { enabled: false });
        assert_eq!(
            lifecycle.packetize(&[4, 5]).unwrap_err(),
            "capture_disabled"
        );
        lifecycle.apply(HelperCommand::SetCapture { enabled: true });
        for _ in 1..MAX_UNACKNOWLEDGED_PACKETS {
            lifecycle.packetize(&[0, 0]).unwrap();
        }
        assert_eq!(lifecycle.packetize(&[0, 0]).unwrap_err(), "backpressure");
        assert!(matches!(
            lifecycle.state(),
            HelperState::Backpressured { generation: 7, .. }
        ));
    }

    #[test]
    fn stale_reconnect_and_playback_fail_closed() {
        let mut lifecycle = MediaLifecycle::default();
        lifecycle.apply(HelperCommand::Start {
            protocol_version: 1,
            identity: identity(3),
            disclosure_ref: "d".into(),
            gateway_url: None,
            application_grant: None,
        });
        lifecycle.apply(HelperCommand::NetworkLost);
        assert!(matches!(
            lifecycle.state(),
            HelperState::Offline { generation: 3 }
        ));
        lifecycle.apply(HelperCommand::NetworkRestored { generation: 2 });
        assert_eq!(
            lifecycle.state(),
            &HelperState::Refused {
                reason: "stale_generation".into()
            }
        );
        lifecycle.apply(HelperCommand::Start {
            protocol_version: 1,
            identity: identity(4),
            disclosure_ref: "d".into(),
            gateway_url: None,
            application_grant: None,
        });
        lifecycle.apply(HelperCommand::Play {
            generation: 4,
            sequence: 0,
            payload_length: 4,
            sha256: "x".repeat(64),
        });
        assert_eq!(
            lifecycle.state(),
            &HelperState::Refused {
                reason: "invalid_playback".into()
            }
        );
    }

    #[test]
    fn device_change_and_every_terminal_reason_are_explicit() {
        let mut lifecycle = MediaLifecycle::default();
        lifecycle.apply(HelperCommand::Start {
            protocol_version: 1,
            identity: identity(1),
            disclosure_ref: "d".into(),
            gateway_url: None,
            application_grant: None,
        });
        lifecycle.apply(HelperCommand::DeviceChanged);
        assert!(matches!(
            lifecycle.state(),
            HelperState::DeviceChanged { generation: 1 }
        ));
        lifecycle.apply(HelperCommand::Stop {
            reason: StopReason::ParentExit,
        });
        assert_eq!(
            lifecycle.state(),
            &HelperState::Stopped {
                reason: "parentexit".into()
            }
        );
    }
}
