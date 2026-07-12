//! Native media-only boundary for OpenAgents Desktop persistent voice.
//! This crate owns bytes and device transport, never transcript, command,
//! Sync, storage-policy, retention, or outcome authority.

use serde::Deserialize;

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
    pub speech_ref: String,
    pub codec: Codec,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub payload_length: u64,
    pub sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Codec { PcmS16le, Opus }

fn valid_ref(value: &str) -> bool { !value.trim().is_empty() && value.len() <= 256 }
fn valid_identity(i: &VoiceIdentity) -> bool {
    i.generation > 0 && valid_ref(&i.owner_ref) && valid_ref(&i.device_ref)
        && valid_ref(&i.thread_ref) && valid_ref(&i.session_ref)
}
fn common_valid(schema: &str, identity: &VoiceIdentity, sequence: u64, channels: u8, length: u64, digest: &str) -> bool {
    schema == AUDIO_PROTOCOL_VERSION && valid_identity(identity) && sequence <= 9_007_199_254_740_991
        && channels == 1 && length <= MAX_AUDIO_PAYLOAD_BYTES
        && digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl MediaHeader {
    pub fn validate(&self) -> bool {
        match self {
            Self::ClientAudio(h) => common_valid(&h.schema, &h.identity, h.sequence, h.channels, h.payload_length, &h.sha256)
                && matches!(h.sample_rate_hz, 16_000 | 24_000 | 48_000),
            Self::ServerTts(h) => common_valid(&h.schema, &h.identity, h.sequence, h.channels, h.payload_length, &h.sha256)
                && valid_ref(&h.speech_ref) && matches!(h.sample_rate_hz, 24_000 | 48_000),
        }
    }
}

pub fn decode_media_header(value: &serde_json::Value) -> Result<MediaHeader, String> {
    let header: MediaHeader = serde_json::from_value(value.clone()).map_err(|e| e.to_string())?;
    header.validate().then_some(header).ok_or_else(|| "invalid audio media header".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[derive(Deserialize)] struct Corpus { cases: Vec<Case> }
    #[derive(Deserialize)] struct Case { name: String, accept: bool, header: serde_json::Value }
    #[test]
    fn matches_effect_golden_corpus() {
        let corpus: Corpus = serde_json::from_str(include_str!("../../../fixtures/audio-contract/media-v1.json")).unwrap();
        for case in corpus.cases {
            assert_eq!(decode_media_header(&case.header).is_ok(), case.accept, "{}", case.name);
        }
    }
}
