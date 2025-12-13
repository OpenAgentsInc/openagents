//! Nostr protocol implementation for OpenAgents.
//!
//! This crate provides:
//! - NIP-01: Basic protocol (events, signing, verification)
//! - NIP-06: Key derivation from BIP39 mnemonic seed phrases (requires `full` feature)
//! - NIP-28: Public Chat (channels, messages, moderation)
//! - NIP-90: Data Vending Machine (DVM) job requests/results/feedback
//!
//! # Features
//!
//! - `full` (default): Full crypto support including key generation and signing
//! - `minimal`: Just Event type and serialization (for WASM/relay use)

mod nip01;
#[cfg(feature = "full")]
mod nip06;
mod nip28;
mod nip90;

// NIP-01: Basic protocol (Event type always available)
pub use nip01::{
    Event, EventTemplate, KIND_CONTACTS, KIND_METADATA, KIND_RECOMMEND_RELAY, KIND_SHORT_TEXT_NOTE,
    KindClassification, Nip01Error, UnsignedEvent, classify_kind, is_addressable_kind,
    is_ephemeral_kind, is_regular_kind, is_replaceable_kind, serialize_event, sort_events,
};

// NIP-01: Validation functions (no crypto needed)
pub use nip01::validate_unsigned_event;

// NIP-01: Crypto functions (require full feature)
#[cfg(feature = "full")]
pub use nip01::{
    finalize_event, generate_secret_key, get_event_hash, get_public_key, get_public_key_hex,
    validate_event, verify_event,
};

// NIP-06: Key derivation from mnemonic (requires full feature)
#[cfg(feature = "full")]
pub use nip06::{
    Keypair, Nip06Error, derive_keypair, derive_keypair_full, derive_keypair_with_account,
    mnemonic_to_seed, npub_to_public_key, nsec_to_private_key, private_key_to_nsec,
    public_key_to_npub,
};

// NIP-28: Public Chat
pub use nip28::{
    ChannelCreateEvent, ChannelHideMessageEvent, ChannelMessageEvent, ChannelMetadata,
    ChannelMetadataEvent, ChannelMuteUserEvent, KIND_CHANNEL_CREATION, KIND_CHANNEL_HIDE_MESSAGE,
    KIND_CHANNEL_MESSAGE, KIND_CHANNEL_METADATA, KIND_CHANNEL_MUTE_USER, ModerationReason,
    Nip28Error, is_channel_creation_kind, is_channel_kind, is_channel_message_kind,
    is_channel_metadata_kind, is_moderation_kind,
};

// NIP-90: Data Vending Machine
pub use nip90::{
    InputType, JOB_REQUEST_KIND_MAX, JOB_REQUEST_KIND_MIN, JOB_RESULT_KIND_MAX,
    JOB_RESULT_KIND_MIN, JobFeedback, JobInput, JobParam, JobRequest, JobResult, JobStatus,
    KIND_JOB_FEEDBACK, KIND_JOB_IMAGE_GENERATION, KIND_JOB_SPEECH_TO_TEXT, KIND_JOB_SUMMARIZATION,
    KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION, KIND_JOB_TRANSLATION, Nip90Error,
    get_request_kind, get_result_kind, is_dvm_kind, is_job_feedback_kind, is_job_request_kind,
    is_job_result_kind,
};
