//! Nostr protocol implementation for OpenAgents.
//!
//! This crate provides:
//! - NIP-01: Basic protocol (events, signing, verification)
//! - NIP-02: Follow List (Contact List and Petnames)
//! - NIP-04: Encrypted Direct Messages (requires `full` feature)
//! - NIP-05: Mapping Nostr Keys to DNS-based Internet Identifiers
//! - NIP-06: Key derivation from BIP39 mnemonic seed phrases (requires `full` feature)
//! - NIP-09: Event Deletion Request
//! - NIP-10: Text Notes and Threads
//! - NIP-11: Relay Information Document
//! - NIP-12: Generic Tag Queries (deprecated, moved to NIP-01)
//! - NIP-13: Proof of Work
//! - NIP-16: Event Treatment (deprecated, moved to NIP-01)
//! - NIP-18: Reposts
//! - NIP-19: bech32-encoded entities
//! - NIP-20: Command Results (deprecated, moved to NIP-01)
//! - NIP-21: nostr: URI scheme
//! - NIP-23: Long-form Content
//! - NIP-25: Reactions
//! - NIP-27: Text Note References
//! - NIP-28: Public Chat (channels, messages, moderation)
//! - NIP-36: Sensitive Content / Content Warning
//! - NIP-40: Expiration Timestamp
//! - NIP-47: Nostr Wallet Connect (requires `full` feature)
//! - NIP-57: Lightning Zaps (tipping with Lightning payments)
//! - NIP-89: Application Handlers (social discovery of skills/agents)
//! - NIP-90: Data Vending Machine (DVM) job requests/results/feedback
//! - Identity types for marketplace participants (agents, creators, providers)
//! - Lightning payment types for marketplace transactions
//! - Compute provider types for decentralized compute marketplace
//! - Compute job types for job submission and routing
//!
//! # Features
//!
//! - `full` (default): Full crypto support including key generation and signing
//! - `minimal`: Just Event type and serialization (for WASM/relay use)

mod compute_job;
mod identity;
mod nip01;
mod nip02;
#[cfg(feature = "full")]
mod nip04;
mod nip05;
#[cfg(feature = "full")]
mod nip06;
mod nip09;
mod nip10;
mod nip11;
mod nip12;
mod nip13;
mod nip16;
mod nip18;
mod nip19;
mod nip20;
mod nip21;
mod nip23;
mod nip25;
mod nip27;
mod nip28;
mod nip36;
mod nip40;
#[cfg(feature = "full")]
mod nip47;
mod nip57;
mod nip89;
mod nip90;
#[cfg(feature = "full")]
mod payments;
mod provider;

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

// NIP-02: Follow List (Contact List and Petnames)
pub use nip02::{CONTACT_LIST_KIND, Contact, ContactList, Nip02Error};

// NIP-04: Encrypted Direct Messages (requires full feature)
#[cfg(feature = "full")]
pub use nip04::{ENCRYPTED_DM_KIND, Nip04Error, decrypt, encrypt};

// NIP-05: Mapping Nostr Keys to DNS-based Internet Identifiers
pub use nip05::{Nip05Error, Nip05Identifier, Nip05Response};

// NIP-06: Key derivation from mnemonic (requires full feature)
#[cfg(feature = "full")]
pub use nip06::{
    Keypair, Nip06Error, derive_keypair, derive_keypair_full, derive_keypair_with_account,
    mnemonic_to_seed, npub_to_public_key, nsec_to_private_key, private_key_to_nsec,
    public_key_to_npub,
};

// NIP-09: Event Deletion Request
pub use nip09::{
    DELETION_REQUEST_KIND, Nip09Error, create_deletion_tags, create_deletion_tags_for_addresses,
    get_deleted_addresses, get_deleted_event_ids, get_deleted_kinds, get_deletion_reason,
    is_deletion_request, should_delete_event,
};

// NIP-10: Text Notes and Threads
pub use nip10::{
    ETagMarker, EventReference, Nip10Error, TEXT_NOTE_KIND, TextNote,
};

// NIP-11: Relay Information Document
pub use nip11::{
    FeeSchedule, KindOrRange, Nip11Error, RELAY_INFO_ACCEPT_HEADER, RelayFees,
    RelayInformationDocument, RelayLimitation, RetentionPolicy,
};

// NIP-12: Generic Tag Queries (deprecated, moved to NIP-01)
pub use nip12::{
    Nip12Error, add_generic_tag, get_tag_values, get_tag_values_with_params, has_tag,
    matches_tag_filter, remove_tags,
};

// NIP-13: Proof of Work
pub use nip13::{
    Nip13Error, calculate_difficulty, check_pow, get_difficulty, parse_nonce_tag, validate_pow,
};

// NIP-16: Event Treatment (deprecated, moved to NIP-01)
pub use nip16::{
    EventCategory, Nip16Error, get_event_category, is_addressable, is_ephemeral, is_regular,
    is_replaceable,
};

// NIP-18: Reposts
pub use nip18::{
    GENERIC_REPOST_KIND, GenericRepost, Nip18Error, REPOST_KIND, Repost, is_repost_kind,
};

// NIP-19: bech32-encoded entities
pub use nip19::{
    AddressPointer, EventPointer, Nip19Entity, Nip19Error, ProfilePointer, decode, encode_naddr,
    encode_nevent, encode_note, encode_nprofile, encode_npub, encode_nsec,
};

// NIP-20: Command Results (deprecated, moved to NIP-01)
pub use nip20::{CommandResult, Nip20Error, NoticeMessage, OkMessage};

// NIP-21: nostr: URI scheme
pub use nip21::{
    NOSTR_URI_SCHEME, Nip21Error, from_nostr_uri, is_nostr_uri, strip_nostr_prefix, to_nostr_uri,
};

// NIP-23: Long-form Content
pub use nip23::{
    ARTICLE_KIND, Article, DRAFT_ARTICLE_KIND, Nip23Error, is_article_kind,
};

// NIP-25: Reactions
pub use nip25::{
    EXTERNAL_REACTION_KIND, Nip25Error, REACTION_KIND, Reaction, ReactionType, is_reaction_kind,
};

// NIP-27: Text Note References
pub use nip27::{
    MentionReference, Nip27Error, extract_event_references, extract_profile_references,
    extract_references, get_mentioned_event_ids, get_mentioned_pubkeys, has_references,
};

// NIP-28: Public Chat
pub use nip28::{
    ChannelCreateEvent, ChannelHideMessageEvent, ChannelMessageEvent, ChannelMetadata,
    ChannelMetadataEvent, ChannelMuteUserEvent, KIND_CHANNEL_CREATION, KIND_CHANNEL_HIDE_MESSAGE,
    KIND_CHANNEL_MESSAGE, KIND_CHANNEL_METADATA, KIND_CHANNEL_MUTE_USER, ModerationReason,
    Nip28Error, is_channel_creation_kind, is_channel_kind, is_channel_message_kind,
    is_channel_metadata_kind, is_moderation_kind,
};

// NIP-36: Sensitive Content / Content Warning
pub use nip36::{
    CONTENT_WARNING_TAG, Nip36Error, add_content_warning, get_content_warning,
    has_content_warning, reasons, remove_content_warning,
};

// NIP-40: Expiration Timestamp
pub use nip40::{
    EXPIRATION_TAG, Nip40Error, get_expiration, has_expiration, is_expired, set_expiration,
    time_until_expiration, validate_expiration,
};

// NIP-47: Nostr Wallet Connect (requires full feature)
#[cfg(feature = "full")]
pub use nip47::{
    BalanceResult, ErrorCode, ErrorResponse, GetBalanceParams, GetInfoParams, InfoResult,
    Invoice, InvoiceState, ListTransactionsParams, ListTransactionsResult, LookupInvoiceParams,
    MakeInvoiceParams, Method, MultiPayInvoiceItem, MultiPayInvoiceParams, MultiPayKeysendItem,
    MultiPayKeysendParams, Network, Nip47Error, Notification, NotificationType,
    PayInvoiceParams, PayInvoiceResult, PayKeysendParams, Request, RequestParams, Response,
    ResponseResult, TlvRecord, Transaction, TransactionType, INFO_EVENT_KIND,
    NOTIFICATION_KIND_NIP04, NOTIFICATION_KIND_NIP44, REQUEST_KIND, RESPONSE_KIND,
};

// NIP-57: Lightning Zaps
pub use nip57::{
    Nip57Error, ZapReceipt, ZapRequest, ZAP_RECEIPT_KIND, ZAP_REQUEST_KIND,
};

// NIP-89: Application Handlers
pub use nip89::{
    HandlerInfo, HandlerMetadata, HandlerRecommendation, HandlerType, KIND_HANDLER_INFO,
    KIND_HANDLER_RECOMMENDATION, Nip89Error, PricingInfo, SocialTrustScore,
    is_handler_info_kind, is_handler_recommendation_kind, is_nip89_kind,
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

// Identity types for marketplace (base types always available)
pub use identity::{AgentIdentity, IdentityError, NostrIdentity, ReputationScore, WalletInfo};

// Identity types that require full feature (use chrono::DateTime)
#[cfg(feature = "full")]
pub use identity::CreatorProfile;

// Lightning payment types (require full feature for DateTime)
#[cfg(feature = "full")]
pub use payments::{
    CoalitionPayment, InvoiceStatus, LightningInvoice, PaymentDestination, PaymentError,
    PaymentRequest, PaymentResult, PaymentSplit,
};

// Compute provider types for marketplace
pub use provider::{
    ComputeCapabilities, ComputePricing, ComputeProvider, ProviderError, ProviderReputation,
    Region, ReputationTier,
};

// Compute job types for marketplace
pub use compute_job::{
    ComputeJobError, ComputeJobRequest, ComputeJobResult, InferenceParams, JobRequirements,
    JobStatus as ComputeJobStatus, SelectionMode, TokenUsage, select_provider,
};
