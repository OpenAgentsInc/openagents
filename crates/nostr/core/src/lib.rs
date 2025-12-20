//! Nostr protocol implementation for OpenAgents.
//!
//! This crate provides:
//! - NIP-01: Basic protocol (events, signing, verification)
//! - NIP-06: Key derivation from BIP39 mnemonic seed phrases (requires `full` feature)
//! - NIP-28: Public Chat (channels, messages, moderation)
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
#[cfg(feature = "full")]
mod nip04;
#[cfg(feature = "full")]
mod nip06;
mod nip28;
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

// NIP-04: Encrypted Direct Messages (requires full feature)
#[cfg(feature = "full")]
pub use nip04::{ENCRYPTED_DM_KIND, Nip04Error, decrypt, encrypt};

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
