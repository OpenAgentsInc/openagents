//! Nostr protocol implementation for OpenAgents.
//!
//! This crate provides:
//! - NIP-01: Basic protocol (events, signing, verification)
//! - NIP-06: Key derivation from BIP39 mnemonic seed phrases
//! - NIP-28: Public Chat (channels, messages, moderation)
//! - NIP-90: Data Vending Machine (DVM) job requests/results/feedback

mod nip01;
mod nip06;
mod nip28;
mod nip90;

// NIP-01: Basic protocol
pub use nip01::{
    Event, EventTemplate, KindClassification, Nip01Error, UnsignedEvent,
    KIND_CONTACTS, KIND_METADATA, KIND_RECOMMEND_RELAY, KIND_SHORT_TEXT_NOTE,
    classify_kind, finalize_event, generate_secret_key, get_event_hash, get_public_key,
    get_public_key_hex, is_addressable_kind, is_ephemeral_kind, is_regular_kind,
    is_replaceable_kind, serialize_event, sort_events, validate_event, validate_unsigned_event,
    verify_event,
};

// NIP-06: Key derivation from mnemonic
pub use nip06::{
    Keypair, Nip06Error, derive_keypair, derive_keypair_full, derive_keypair_with_account,
    mnemonic_to_seed, npub_to_public_key, nsec_to_private_key, private_key_to_nsec,
    public_key_to_npub,
};

// NIP-28: Public Chat
pub use nip28::{
    ChannelCreateEvent, ChannelHideMessageEvent, ChannelMessageEvent, ChannelMetadata,
    ChannelMetadataEvent, ChannelMuteUserEvent, ModerationReason, Nip28Error,
    KIND_CHANNEL_CREATION, KIND_CHANNEL_HIDE_MESSAGE, KIND_CHANNEL_MESSAGE,
    KIND_CHANNEL_METADATA, KIND_CHANNEL_MUTE_USER, is_channel_creation_kind, is_channel_kind,
    is_channel_message_kind, is_channel_metadata_kind, is_moderation_kind,
};

// NIP-90: Data Vending Machine
pub use nip90::{
    InputType, JobFeedback, JobInput, JobParam, JobRequest, JobResult, JobStatus, Nip90Error,
    JOB_REQUEST_KIND_MAX, JOB_REQUEST_KIND_MIN, JOB_RESULT_KIND_MAX, JOB_RESULT_KIND_MIN,
    KIND_JOB_FEEDBACK, KIND_JOB_IMAGE_GENERATION, KIND_JOB_SPEECH_TO_TEXT,
    KIND_JOB_SUMMARIZATION, KIND_JOB_TEXT_EXTRACTION, KIND_JOB_TEXT_GENERATION,
    KIND_JOB_TRANSLATION, get_request_kind, get_result_kind, is_dvm_kind,
    is_job_feedback_kind, is_job_request_kind, is_job_result_kind,
};
