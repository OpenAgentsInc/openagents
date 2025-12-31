//! Nostr protocol implementation for OpenAgents.
//!
//! This crate provides a comprehensive implementation of the Nostr protocol and related
//! specifications (NIPs) for decentralized social networking and data exchange. It forms
//! the foundation for OpenAgents' marketplace, agent communication, and data vending machine
//! capabilities.
//!
//! # Architecture
//!
//! The crate is organized around several key components:
//!
//! - **Core Protocol** (NIP-01): Event structure, signing, verification, and serialization
//! - **Identity Management** (NIP-06): BIP39 mnemonic-based key derivation
//! - **Encryption** (NIP-04, NIP-44): Encrypted direct messages with versioned encryption
//! - **Social Features** (NIP-02, NIP-25, NIP-28, NIP-72): Following, reactions, chat, communities
//! - **Content Types** (NIP-23, NIP-71, NIP-84): Long-form, video, highlights
//! - **Lightning Integration** (NIP-47, NIP-57, NIP-61): Wallet Connect, zaps, Cashu payments
//! - **Marketplace** (NIP-15, NIP-69, NIP-99): Products, orders, classified listings
//! - **Data Vending** (NIP-90): Compute job requests and results
//! - **Agent Protocol** (NIP-SA, NIP-89): Autonomous agents with sovereign identity
//!
//! # Protocol Compliance
//!
//! This implementation follows the Nostr protocol specification as defined at
//! <https://github.com/nostr-protocol/nips>. All NIPs are implemented according to their
//! current specifications, with appropriate feature flags for cryptographic operations.
//!
//! # Features
//!
//! - `full` (default): Full crypto support (signing, encryption, key derivation)
//! - `minimal`: Event types and serialization only (for WASM/relay use)
//!
//! # Usage
//!
//! ## Creating and Signing Events
//!
//! ```
//! use nostr::{generate_secret_key, EventTemplate, finalize_event, KIND_SHORT_TEXT_NOTE};
//!
//! // Generate a new keypair
//! let secret_key = generate_secret_key();
//!
//! // Create an event template
//! let template = EventTemplate {
//!     kind: KIND_SHORT_TEXT_NOTE,
//!     tags: vec![],
//!     content: "Hello Nostr!".to_string(),
//!     created_at: std::time::SystemTime::now()
//!         .duration_since(std::time::UNIX_EPOCH)
//!         .unwrap()
//!         .as_secs(),
//! };
//!
//! // Sign and finalize the event
//! let event = finalize_event(&template, &secret_key).unwrap();
//! # Ok::<(), nostr::Nip01Error>(())
//! ```
//!
//! ## Event Verification
//!
//! ```
//! use nostr::{generate_secret_key, EventTemplate, finalize_event, verify_event, KIND_SHORT_TEXT_NOTE};
//!
//! // Create and sign an event
//! let secret_key = generate_secret_key();
//! let template = EventTemplate {
//!     kind: KIND_SHORT_TEXT_NOTE,
//!     tags: vec![],
//!     content: "Test".to_string(),
//!     created_at: 1234567890,
//! };
//! let event = finalize_event(&template, &secret_key).unwrap();
//!
//! // Verify the event signature
//! let is_valid = verify_event(&event).unwrap();
//! assert!(is_valid);
//! # Ok::<(), nostr::Nip01Error>(())
//! ```
//!
//! ## Working with Tags
//!
//! ```
//! use nostr::{generate_secret_key, EventTemplate, finalize_event};
//!
//! let secret_key = generate_secret_key();
//!
//! // Create event with tags
//! let template = EventTemplate {
//!     kind: 1,
//!     tags: vec![
//!         vec!["e".to_string(), "event_id_here".to_string()],
//!         vec!["p".to_string(), "pubkey_here".to_string()],
//!     ],
//!     content: "Reply to an event".to_string(),
//!     created_at: 1234567890,
//! };
//!
//! let event = finalize_event(&template, &secret_key).unwrap();
//! assert_eq!(event.tags.len(), 2);
//! # Ok::<(), nostr::Nip01Error>(())
//! ```
//!
//! # NIP Coverage
//!
//! This crate implements:
//! - NIP-01: Basic protocol (events, signing, verification)
//! - NIP-02: Follow List (Contact List and Petnames)
//! - NIP-03: OpenTimestamps Attestations for Events
//! - NIP-04: Encrypted Direct Messages (requires `full` feature)
//! - NIP-05: Mapping Nostr Keys to DNS-based Internet Identifiers
//! - NIP-06: Key derivation from BIP39 mnemonic seed phrases (requires `full` feature)
//! - NIP-07: window.nostr capability for web browsers
//! - NIP-08: Handling Mentions (deprecated, use NIP-27)
//! - NIP-09: Event Deletion Request
//! - NIP-10: Text Notes and Threads
//! - NIP-11: Relay Information Document
//! - NIP-12: Generic Tag Queries (deprecated, moved to NIP-01)
//! - NIP-13: Proof of Work
//! - NIP-14: Subject Tag in Text Events (email-style subject lines)
//! - NIP-15: Nostr Marketplace (merchant stalls, products, auctions)
//! - NIP-16: Event Treatment (deprecated, moved to NIP-01)
//! - NIP-17: Private Direct Messages (requires `full` feature)
//! - NIP-18: Reposts
//! - NIP-19: bech32-encoded entities
//! - NIP-20: Command Results (deprecated, moved to NIP-01)
//! - NIP-21: nostr: URI scheme
//! - NIP-22: Comment
//! - NIP-23: Long-form Content
//! - NIP-24: Extra Metadata Fields and Tags
//! - NIP-25: Reactions
//! - NIP-26: Delegated Event Signing
//! - NIP-27: Text Note References
//! - NIP-28: Public Chat (channels, messages, moderation)
//! - NIP-29: Relay-based Groups (group chat with member management)
//! - NIP-30: Custom Emoji (emoji tags with shortcodes and image URLs)
//! - NIP-31: Alt Tag for Unknown Events (graceful degradation)
//! - NIP-32: Labeling (content moderation and categorization)
//! - NIP-33: Parameterized Replaceable Events (deprecated, moved to NIP-01)
//! - NIP-35: Torrents (BitTorrent file sharing index)
//! - NIP-36: Sensitive Content / Content Warning
//! - NIP-37: Draft Wraps (encrypted draft storage and private relay lists)
//! - NIP-38: User Statuses (live status updates)
//! - NIP-39: External Identities in Profiles
//! - NIP-40: Expiration Timestamp
//! - NIP-42: Authentication of Clients to Relays
//! - NIP-43: Relay Access Metadata and Requests (membership lists, join/leave requests)
//! - NIP-44: Versioned Encryption (requires `full` feature)
//! - NIP-45: Event Counts (COUNT verb for relays)
//! - NIP-46: Nostr Remote Signing (requires `full` feature)
//! - NIP-47: Nostr Wallet Connect (requires `full` feature)
//! - NIP-48: Proxy Tags (bridging from other protocols)
//! - NIP-49: Private Key Encryption (requires `full` feature)
//! - NIP-50: Search Capability
//! - NIP-51: Lists (mute lists, pin lists, bookmarks, etc.)
//! - NIP-52: Calendar Events (date-based, time-based, calendars, RSVPs)
//! - NIP-53: Live Activities (live streaming, meetings, presence)
//! - NIP-54: Wiki (collaborative wiki articles)
//! - NIP-55: Android Signer Application (Android intent-based signing)
//! - NIP-56: Reporting
//! - NIP-57: Lightning Zaps (tipping with Lightning payments)
//! - NIP-58: Badges (badge definitions, awards, and profile display)
//! - NIP-59: Gift Wrap (encapsulation and metadata obscuring, requires `full` feature)
//! - NIP-60: Cashu Wallets (ecash wallet state management)
//! - NIP-61: Nutzaps (Cashu-based zaps with P2PK tokens)
//! - NIP-62: Request to Vanish (GDPR-compliant complete data deletion)
//! - NIP-64: Chess (Portable Game Notation)
//! - NIP-65: Relay List Metadata (user's preferred read/write relays)
//! - NIP-66: Relay Discovery and Liveness Monitoring (relay characteristics and monitors)
//! - NIP-68: Picture-first Feeds (Instagram-style image posts)
//! - NIP-69: Peer-to-peer Order Events (decentralized marketplace orders)
//! - NIP-70: Protected Events (author-only publishing with authentication)
//! - NIP-71: Video Events (video content with metadata)
//! - NIP-72: Moderated Communities (Reddit-style communities)
//! - NIP-73: External Content IDs (ISBN, podcast GUID, ISAN, blockchain references)
//! - NIP-75: Zap Goals (fundraising goals with zap tracking)
//! - NIP-77: Negentropy Syncing (efficient event set reconciliation)
//! - NIP-78: Application-specific Data (arbitrary app data storage)
//! - NIP-84: Highlights (highlighting valuable content)
//! - NIP-86: Relay Management API (HTTP API for relay administration)
//! - NIP-88: Polls (decentralized polls with single/multiple choice)
//! - NIP-89: Application Handlers (social discovery of skills/agents)
//! - NIP-90: Data Vending Machine (DVM) job requests/results/feedback
//! - NIP-92: Media Attachments (inline media metadata)
//! - NIP-94: File Metadata
//! - NIP-95: File Storage on Relays (deprecated, use NIP-96)
//! - NIP-96: HTTP File Storage Integration (deprecated, use NIP-B7)
//! - NIP-98: HTTP Auth (requires `full` feature)
//! - NIP-99: Classified Listings (marketplace ads)
//! - NIP-C7: Chats (simple chat protocol with quote replies)
//! - NIP-SA: Sovereign Agents (autonomous agents with their own identity)
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
mod nip03;
#[cfg(feature = "full")]
mod nip04;
mod nip05;
#[cfg(feature = "full")]
mod nip06;
mod nip07;
mod nip08;
mod nip09;
mod nip10;
mod nip11;
mod nip12;
mod nip13;
mod nip14;
mod nip15;
mod nip16;
#[cfg(feature = "full")]
mod nip17;
mod nip18;
mod nip19;
mod nip20;
mod nip21;
mod nip22;
mod nip23;
mod nip24;
mod nip25;
mod nip26;
mod nip27;
mod nip28;
mod nip29;
mod nip30;
mod nip31;
pub mod nip32;
mod nip33;
mod nip34;
mod nip35;
mod nip36;
mod nip37;
mod nip38;
mod nip39;
mod nip40;
mod nip42;
mod nip43;
#[cfg(feature = "full")]
mod nip44;
mod nip45;
#[cfg(feature = "full")]
mod nip46;
mod nip47;
mod nip48;
#[cfg(feature = "full")]
mod nip49;
mod nip50;
mod nip51;
mod nip52;
mod nip53;
mod nip54;
mod nip55;
mod nip56;
mod nip57;
mod nip58;
#[cfg(feature = "full")]
mod nip59;
mod nip60;
mod nip61;
mod nip62;
mod nip64;
mod nip65;
mod nip66;
mod nip68;
mod nip69;
mod nip70;
mod nip71;
mod nip72;
mod nip73;
mod nip75;
pub mod nip77;
mod nip78;
mod nip84;
mod nip86;
mod nip87;
mod nip88;
mod nip89;
pub mod nip90;
mod nip92;
mod nip94;
mod nip95;
mod nip96;
mod nip99;
#[cfg(feature = "full")]
mod nip98;
mod nipc7;
pub mod nip_sa;
#[cfg(feature = "full")]
mod payments;
mod provider;

#[cfg(test)]
mod tests;

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

// NIP-03: OpenTimestamps Attestations for Events
pub use nip03::{
    KIND_OTS_ATTESTATION, Nip03Error, OpenTimestampsAttestation, TARGET_EVENT_TAG,
    TARGET_KIND_TAG, create_attestation_tags, decode_ots_content, encode_ots_content,
    get_target_event_id, get_target_event_kind, get_target_relay_url, is_ots_attestation,
    parse_attestation,
};

// NIP-04: Encrypted Direct Messages (requires full feature)
#[cfg(feature = "full")]
pub use nip04::{ENCRYPTED_DM_KIND, Nip04Error, decrypt, encrypt};

// NIP-05: Mapping Nostr Keys to DNS-based Internet Identifiers
pub use nip05::{Nip05Error, Nip05Identifier, Nip05Response};

// NIP-06: Key derivation from mnemonic (requires full feature)
#[cfg(feature = "full")]
pub use nip06::{
    Keypair, Nip06Error, derive_agent_keypair, derive_keypair, derive_keypair_full,
    derive_keypair_with_account, mnemonic_to_seed, npub_to_public_key, nsec_to_private_key,
    private_key_to_nsec, public_key_to_npub,
};

// NIP-07: window.nostr capability for web browsers
pub use nip07::{
    Nip07Error, SignEventTemplate, WindowNostr, WindowNostrNip04, WindowNostrNip44,
    WindowNostrProvider, is_available as nip07_is_available,
};

// NIP-08: Handling Mentions (deprecated, use NIP-27)
pub use nip08::{
    Mention, Nip08Error, create_tags_from_mentions, extract_mentions, extract_mentions_detailed,
    format_mention, get_mention_value, parse_mention, replace_mentions, validate_mention,
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

// NIP-14: Subject Tag in Text Events
pub use nip14::{
    RECOMMENDED_MAX_LENGTH, REPLY_PREFIX, SUBJECT_TAG, add_subject, create_reply_subject,
    get_subject, has_subject, is_subject_too_long, truncate_subject, truncate_subject_recommended,
};

// NIP-15: Nostr Marketplace
pub use nip15::{
    AuctionProduct, BidConfirmation, BidStatus, KIND_AUCTION, KIND_BID, KIND_BID_CONFIRMATION,
    KIND_MARKETPLACE_UI, KIND_PRODUCT, KIND_STALL, MarketplaceUI, MerchantStall, Nip15Error,
    Product, ProductShipping, ProductSpec, ShippingZone, UIConfig, is_marketplace_kind,
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

// NIP-22: Comment
pub use nip22::{
    COMMENT_KIND, Nip22Error, create_comment_tags, get_parent_address, get_parent_event_id,
    get_parent_kind, get_root_address, get_root_event_id, get_root_kind, is_comment,
    validate_comment,
};

// NIP-23: Long-form Content
pub use nip23::{
    ARTICLE_KIND, Article, DRAFT_ARTICLE_KIND, Nip23Error, is_article_kind,
};

// NIP-24: Extra Metadata Fields and Tags
pub use nip24::{
    Birthday, ExtraMetadata, Nip24Error, normalize_hashtag, remove_deprecated_fields,
    validate_hashtag,
};

// NIP-25: Reactions
pub use nip25::{
    EXTERNAL_REACTION_KIND, Nip25Error, REACTION_KIND, Reaction, ReactionType, is_reaction_kind,
};

// NIP-26: Delegated Event Signing
pub use nip26::{
    Condition, Nip26Error, check_delegation_conditions, conditions_to_string,
    create_delegation_string, create_delegation_token, parse_conditions, validate_delegation,
    verify_delegation_token,
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

// NIP-29: Relay-based Groups
pub use nip29::{
    Admin, GROUP_D_TAG, GROUP_TAG, GroupAdmins, GroupMembers, GroupMetadata, GroupRoles,
    JoinRequest, KIND_GROUP_ADMINS, KIND_GROUP_MEMBERS, KIND_GROUP_METADATA, KIND_GROUP_ROLES,
    KIND_JOIN_REQUEST, KIND_LEAVE_REQUEST, KIND_MOD_CREATE_GROUP, KIND_MOD_CREATE_INVITE,
    KIND_MOD_DELETE_EVENT, KIND_MOD_DELETE_GROUP, KIND_MOD_EDIT_METADATA, KIND_MOD_PUT_USER,
    KIND_MOD_REMOVE_USER, LeaveRequest, ModerationAction, Nip29Error, PREVIOUS_TAG, Role,
    format_group_identifier, is_group_kind, is_group_metadata_kind, is_group_moderation_kind,
    parse_group_identifier, validate_group_id,
};

// NIP-30: Custom Emoji
pub use nip30::{
    CustomEmoji, EMOJI_TAG, Nip30Error, add_emoji_tag, contains_shortcodes, emojify,
    emojify_with, extract_shortcodes, get_emoji, get_emoji_tags, has_emoji,
    remove_all_emoji_tags, remove_emoji_tag, validate_shortcode,
};

// NIP-31: Alt Tag for Unknown Events
pub use nip31::{
    ALT_TAG, Nip31Error, add_alt_tag, create_default_alt, get_alt_tag, has_alt_tag,
    remove_alt_tag, set_alt_tag, validate_alt_summary,
};

// NIP-32: Labeling
pub use nip32::{
    KIND_LABEL, Label, LabelEvent, LabelTarget, Nip32Error, SelfLabel, UGC_NAMESPACE,
    is_label_kind,
};

// NIP-34: Git Stuff
pub use nip34::{
    CommitterInfo, Issue, Patch, PullRequest, RefState, RepositoryAnnouncement, RepositoryState,
    Status, StatusUpdate, KIND_ISSUE, KIND_PATCH, KIND_PULL_REQUEST, KIND_PULL_REQUEST_UPDATE,
    KIND_REPOSITORY_ANNOUNCEMENT, KIND_REPOSITORY_STATE, KIND_STATUS_APPLIED, KIND_STATUS_CLOSED,
    KIND_STATUS_DRAFT, KIND_STATUS_OPEN, KIND_USER_GRASP_LIST,
};

// NIP-33: Parameterized Replaceable Events (deprecated, moved to NIP-01)
pub use nip33::{
    ADDRESSABLE_KIND_MAX, ADDRESSABLE_KIND_MIN, D_TAG, Nip33Error, create_a_tag, create_address,
    get_a_tags, get_d_tag, get_event_address, parse_address, set_d_tag,
    validate_addressable_event,
};

// NIP-35: Torrents
pub use nip35::{
    FILE_TAG, INFO_HASH_TAG, Nip35Error, TITLE_TAG, TORRENT_COMMENT_KIND, TORRENT_KIND,
    TRACKER_TAG, Torrent, TorrentFile, create_info_hash_tag, create_title_tag,
    create_tracker_tag, get_external_ids, is_nip35_kind, is_torrent_comment_kind,
    is_torrent_kind,
};

// NIP-36: Sensitive Content / Content Warning
pub use nip36::{
    CONTENT_WARNING_TAG, Nip36Error, add_content_warning, get_content_warning,
    has_content_warning, reasons, remove_content_warning,
};

// NIP-37: Draft Wraps
pub use nip37::{
    DRAFT_WRAP_KIND, DraftWrap, Nip37Error, PRIVATE_CONTENT_RELAY_LIST_KIND,
    PrivateContentRelayList, is_draft_wrap_kind, is_nip37_kind,
    is_private_content_relay_list_kind,
};

// NIP-38: User Statuses
pub use nip38::{
    KIND_USER_STATUS, Nip38Error, STATUS_GENERAL, STATUS_MUSIC, StatusLink, StatusType,
    UserStatus, is_user_status_kind,
};

// NIP-39: External Identities in Profiles
pub use nip39::{
    ExternalIdentity, GenericIdentity, GitHubIdentity, MastodonIdentity, Nip39Error,
    TelegramIdentity, TwitterIdentity, normalize_identity, validate_platform_name,
};

// NIP-40: Expiration Timestamp
pub use nip40::{
    EXPIRATION_TAG, Nip40Error, get_expiration, has_expiration, is_expired, set_expiration,
    time_until_expiration, validate_expiration,
};

// NIP-42: Authentication of Clients to Relays
pub use nip42::{
    AUTH_KIND, AUTH_REQUIRED_PREFIX, CHALLENGE_TAG, MAX_TIME_DIFF, Nip42Error, RELAY_TAG,
    RESTRICTED_PREFIX, create_auth_event_tags, create_auth_required_message,
    create_restricted_message, get_challenge, get_relay_url, is_auth_event,
    is_auth_required_error, is_restricted_error, normalize_relay_url, validate_auth_event,
};

// NIP-43: Relay Access Metadata and Requests
pub use nip43::{
    ADD_USER_KIND, AddUserEvent, CLAIM_TAG, INVITE_REQUEST_KIND, InviteRequestEvent,
    JOIN_REQUEST_KIND, JoinRequestEvent, LEAVE_REQUEST_KIND, LeaveRequestEvent, MEMBER_TAG,
    MEMBERSHIP_LIST_KIND, MembershipListEvent, Nip43Error, REMOVE_USER_KIND,
    RemoveUserEvent, validate_add_user, validate_join_request, validate_leave_request,
    validate_membership_list, validate_remove_user,
};

// NIP-44: Versioned Encryption (requires full feature)
#[cfg(feature = "full")]
pub use nip44::{
    CHACHA_KEY_SIZE, CHACHA_NONCE_SIZE, HMAC_KEY_SIZE, MAC_SIZE, MAX_PLAINTEXT_LEN,
    MIN_PADDED_LEN, MIN_PLAINTEXT_LEN, NONCE_SIZE, Nip44Error, VERSION,
    decrypt as decrypt_v2, encrypt as encrypt_v2,
};

// NIP-45: Event Counts
pub use nip45::{CountRequest, CountResponse, Nip45Error};

// NIP-47: Nostr Wallet Connect (requires full feature)

// NIP-46: Nostr Remote Signing (requires full feature)
#[cfg(feature = "full")]
pub use nip46::{
    BunkerUrl, KIND_NOSTR_CONNECT, Nip46Error, NostrConnectMethod, NostrConnectRequest,
    NostrConnectResponse, NostrConnectUrl, generate_request_id, is_nostr_connect_event,
};
#[cfg(feature = "full")]
pub use nip47::{
    BalanceResult, ErrorCode, ErrorResponse, GetBalanceParams, GetInfoParams, InfoResult,
    Invoice, InvoiceState, ListTransactionsParams, ListTransactionsResult, LookupInvoiceParams,
    MakeInvoiceParams, Method, MultiPayInvoiceItem, MultiPayInvoiceParams, MultiPayKeysendItem,
    MultiPayKeysendParams, Network, Nip47Error, NostrWalletConnectUrl, Notification, NotificationType,
    PayInvoiceParams, PayInvoiceResult, PayKeysendParams, Request, RequestParams, Response,
    ResponseResult, TlvRecord, Transaction, TransactionType, INFO_EVENT_KIND,
    NOTIFICATION_KIND_NIP04, NOTIFICATION_KIND_NIP44, REQUEST_KIND, RESPONSE_KIND,
};

// NIP-48: Proxy Tags
pub use nip48::{
    Nip48Error, PROXY_TAG, ProxyProtocol, ProxyTag, add_proxy_tag, get_proxy_tag,
    get_proxy_tags, has_proxy_tag, is_bridged_event,
};

// NIP-49: Private Key Encryption (requires full feature)
#[cfg(feature = "full")]
pub use nip49::{
    KeySecurity, Nip49Error, ENCRYPTED_SIZE, PRIVATE_KEY_SIZE, SALT_SIZE, TAG_SIZE,
    decrypt as nip49_decrypt, derive_key as nip49_derive_key, encrypt as nip49_encrypt,
    normalize_password,
    NONCE_SIZE as NIP49_NONCE_SIZE, VERSION as NIP49_VERSION,
};

// NIP-50: Search Capability
pub use nip50::{
    Nip50Error, SearchExtensions, SearchQuery, Sentiment, validate_query,
};

// NIP-51: Lists
pub use nip51::{
    KIND_APP_CURATION, KIND_BLOCKED_RELAYS, KIND_BOOKMARK_SETS, KIND_BOOKMARKS, KIND_CALENDAR,
    KIND_COMMUNITIES, KIND_CURATION_SETS, KIND_DM_RELAYS, KIND_EMOJI_SETS, KIND_EMOJIS,
    KIND_FOLLOW_LIST, KIND_FOLLOW_SETS, KIND_INTERESTS, KIND_INTEREST_SETS, KIND_KIND_MUTE_SETS,
    KIND_MEDIA_FOLLOWS, KIND_MEDIA_STARTER_PACKS, KIND_MUTE_LIST, KIND_PINNED_NOTES,
    KIND_PUBLIC_CHATS, KIND_RELAY_FEEDS, KIND_RELAY_LIST, KIND_RELAY_SETS, KIND_RELEASE_ARTIFACTS,
    KIND_SEARCH_RELAYS, KIND_SIMPLE_GROUPS, KIND_STARTER_PACKS, KIND_VIDEO_CURATION,
    KIND_WIKI_AUTHORS, KIND_WIKI_RELAYS, ListType, Nip51Error, create_metadata_tags,
    create_set_identifier_tag, get_description, get_image, get_list_type, get_public_items,
    get_set_identifier, get_title, is_list_event, is_nip04_encryption,
};

// NIP-52: Calendar Events
pub use nip52::{
    Calendar, CalendarEventRef, CalendarEventRsvp, DateBasedCalendarEvent, FreeBusyStatus,
    KIND_CALENDAR_EVENT_RSVP, KIND_DATE_BASED_CALENDAR_EVENT,
    KIND_TIME_BASED_CALENDAR_EVENT, Nip52Error, Participant, RsvpStatus, TimeBasedCalendarEvent,
    is_calendar_event_kind, is_nip52_kind,
};
// Note: KIND_CALENDAR (31924) is already exported from nip51

// NIP-53: Live Activities
pub use nip53::{
    KIND_LIVE_CHAT_MESSAGE, KIND_LIVE_STREAMING, KIND_MEETING_ROOM_EVENT, KIND_MEETING_SPACE,
    KIND_ROOM_PRESENCE, LiveChatMessage, LiveParticipant, LiveStatus, LiveStreamingEvent,
    MeetingRoomEvent, MeetingSpace, Nip53Error, RoomPresence, SpaceStatus, is_nip53_kind,
};

// NIP-54: Wiki
pub use nip54::{
    KIND_WIKI_ARTICLE, KIND_WIKI_MERGE_REQUEST, KIND_WIKI_REDIRECT, Nip54Error, WikiArticle,
    WikiMergeRequest, WikiRedirect, WikiReference, is_nip54_kind, normalize_d_tag,
};

// NIP-55: Android Signer Application
pub use nip55::{
    CompressionType, ContentResolverUri, Nip55Error, Permission, ReturnType, SignerRequest,
    SignerRequestType, SignerResponse, WebSignerRequest, NOSTRSIGNER_SCHEME,
};

// NIP-56: Reporting
pub use nip56::{
    Nip56Error, Report, ReportTarget, ReportType, REPORT_KIND, is_report_kind,
};

// NIP-57: Lightning Zaps
pub use nip57::{
    count_zaps_for_event, Nip57Error, ZapReceipt, ZapRequest, ZapSettings, ZAP_RECEIPT_KIND,
    ZAP_REQUEST_KIND,
};

// NIP-58: Badges
pub use nip58::{
    BadgeAward, BadgeDefinition, BadgeThumbnail, ImageDimensions, KIND_BADGE_AWARD,
    KIND_BADGE_DEFINITION, KIND_PROFILE_BADGES, Nip58Error, PROFILE_BADGES_D_TAG,
    ProfileBadgePair, ProfileBadges, is_badge_award_kind, is_badge_definition_kind,
    is_nip58_kind, is_profile_badges_kind,
};

// NIP-59: Gift Wrap (requires full feature)
#[cfg(feature = "full")]
pub use nip59::{
    KIND_GIFT_WRAP, KIND_SEAL, Nip59Error, Rumor, create_gift_wrap, create_seal, gift_wrap,
    random_timestamp, unwrap_gift_wrap, unwrap_gift_wrap_full, unwrap_seal,
};

// NIP-60: Cashu Wallets
pub use nip60::{
    CashuProof, EventMarker, Nip60Error, QuoteEvent, SpendingHistoryEvent, TokenContent,
    TokenEvent, TransactionDirection, WalletEvent, QUOTE_KIND, SPENDING_HISTORY_KIND, TOKEN_KIND,
    WALLET_KIND, is_nip60_kind, is_quote_kind, is_spending_history_kind, is_token_kind,
    is_wallet_kind,
};

// NIP-61: Nutzaps
pub use nip61::{
    MintInfo, Nip61Error, Nutzap, NutzapInfo, NutzapProof, NUTZAP_INFO_KIND, NUTZAP_KIND,
    create_mint_tag, create_proof_tag, create_pubkey_tag, create_relay_tag, create_u_tag,
    create_unit_tag, is_nip61_kind, is_nutzap_info_kind, is_nutzap_kind,
};

// NIP-62: Request to Vanish
pub use nip62::{
    ALL_RELAYS, Nip62Error, REQUEST_TO_VANISH_KIND, RELAY_TAG as NIP62_RELAY_TAG,
    RequestToVanish, get_target_relays, is_global_request, is_request_to_vanish,
    validate_request_to_vanish,
};

// NIP-64: Chess (Portable Game Notation)
pub use nip64::{
    CHESS_GAME_KIND, ChessGame, GameResult, Nip64Error, create_alt_description, create_chess_game,
    create_chess_game_with_tags, is_chess_game_kind,
};

// NIP-65: Relay List Metadata
pub use nip65::{
    Nip65Error, READ_MARKER, RELAY_LIST_METADATA_KIND, RelayEntry, RelayListMetadata, RelayMarker,
    WRITE_MARKER, get_relay_entries, is_relay_list_metadata_kind,
};

// NIP-66: Relay Discovery and Liveness Monitoring
pub use nip66::{
    CHECK_TYPE_TAG, CheckTimeout, FREQUENCY_TAG, GEOHASH_TAG as NIP66_GEOHASH_TAG,
    KIND_TAG as NIP66_KIND_TAG, KindPolicy, NETWORK_TYPE_TAG, NIP_SUPPORT_TAG, Nip66Error,
    NetworkType, RELAY_DISCOVERY_KIND, RELAY_MONITOR_ANNOUNCEMENT_KIND, RELAY_TYPE_TAG,
    REQUIREMENT_TAG, RTT_OPEN_TAG, RTT_READ_TAG, RTT_WRITE_TAG, RelayDiscovery,
    RelayMonitorAnnouncement, Requirement, RttMetrics, TIMEOUT_TAG, TOPIC_TAG as NIP66_TOPIC_TAG,
    is_relay_discovery, is_relay_monitor_announcement, validate_relay_discovery,
    validate_relay_monitor_announcement,
};

// NIP-68: Picture-first Feeds
pub use nip68::{
    ALLOWED_MEDIA_TYPES, Nip68Error, PICTURE_KIND, PictureEvent, UserAnnotation,
    create_content_warning_tag, create_geohash_tag, create_language_tags, create_location_tag,
    is_allowed_media_type, is_picture_kind,
};

// NIP-69: Peer-to-peer Order Events
pub use nip69::{
    BitcoinLayer, Nip69Error, OrderStatus, OrderType, P2POrder, P2P_ORDER_KIND, Rating,
    DOCUMENT_TYPE, is_p2p_order_kind,
};

// NIP-17: Private Direct Messages (requires full feature)
#[cfg(feature = "full")]
pub use nip17::{
    ChatMessage, DmRelayList, FileMessage, KIND_CHAT_MESSAGE, KIND_DM_RELAY_LIST,
    KIND_FILE_MESSAGE, Nip17Error, QuotedEvent, receive_chat_message, receive_file_message,
    send_chat_message, send_file_message,
};

// NIP-70: Protected Events
pub use nip70::{
    Nip70Error, PROTECTED_TAG, add_protected_tag, get_protected_tag, is_protected,
    remove_protected_tag, validate_protected_event,
};

// NIP-71: Video Events
pub use nip71::{
    KIND_SHORT_VIDEO, KIND_VIDEO, Nip71Error, TextTrack, VideoEvent, VideoSegment, VideoVariant,
    is_video_kind,
};

// NIP-72: Moderated Communities
pub use nip72::{
    Community, CommunityApproval, CommunityModerator, CommunityPost, CommunityRelay,
    KIND_COMMUNITY_APPROVAL, KIND_COMMUNITY_DEFINITION, KIND_COMMUNITY_POST, Nip72Error,
    is_nip72_kind,
};

// NIP-73: External Content IDs
pub use nip73::{
    EXTERNAL_ID_TAG, EXTERNAL_KIND_TAG, ExternalContent, ExternalContentType, Nip73Error,
    add_external_content, bitcoin_address, bitcoin_tx, doi, ethereum_address, ethereum_tx,
    geohash, get_external_content_refs, hashtag, isan, isbn, podcast_episode, podcast_feed,
    podcast_publisher, web,
};

// NIP-75: Zap Goals
pub use nip75::{
    AMOUNT_TAG, CLOSED_AT_TAG, GOAL_TAG, IMAGE_TAG, Nip75Error, RELAYS_TAG, SUMMARY_TAG,
    ZAP_GOAL_KIND, ZapGoal, add_goal_tag, create_amount_tag, create_closed_at_tag,
    create_image_tag, create_relays_tag, create_summary_tag, get_goal_reference,
    is_zap_goal_kind,
};

// NIP-77: Negentropy Syncing
pub use nip77::{
    Bound, EventId, NegClose, NegErr, NegMsg, NegOpen, NegentropyMessage, Nip77Error,
    PROTOCOL_VERSION_1, Range, RangeMode, RangePayload, Record, TIMESTAMP_INFINITY,
    calculate_fingerprint, decode_varint, encode_varint, sort_records,
};

// NIP-78: Application-specific Data
pub use nip78::{AppData, KIND_APP_DATA, Nip78Error, is_app_data_kind};

// NIP-84: Highlights
pub use nip84::{
    Attribution, Highlight, HighlightSource, KIND_HIGHLIGHT, Nip84Error, is_nip84_kind,
};

// NIP-86: Relay Management API
pub use nip86::{
    EventEntry, IpEntry, Nip86Error, PubkeyEntry,
    Method as RelayManagementMethod, Request as RelayManagementRequest,
    Response as RelayManagementResponse, create_allow_event_request, create_allow_kind_request,
    create_allow_pubkey_request, create_ban_event_request, create_ban_pubkey_request,
    create_block_ip_request, create_change_relay_description_request,
    create_change_relay_icon_request, create_change_relay_name_request,
    create_disallow_kind_request, create_list_allowed_kinds_request,
    create_list_allowed_pubkeys_request, create_list_banned_events_request,
    create_list_banned_pubkeys_request, create_list_blocked_ips_request,
    create_list_events_needing_moderation_request, create_supported_methods_request,
    create_unblock_ip_request, CONTENT_TYPE as RELAY_MANAGEMENT_CONTENT_TYPE,
};

// NIP-87: Ecash Mint Discoverability
pub use nip87::{
    D_TAG as NIP87_D_TAG, EVENT_REF_TAG as NIP87_EVENT_REF_TAG, CashuMintInfo, FedimintInfo,
    KIND_CASHU_MINT, KIND_FEDIMINT, KIND_MINT_RECOMMENDATION, KIND_TAG as NIP87_KIND_TAG,
    MODULES_TAG, MintNetwork, MintRecommendation, NETWORK_TAG as NIP87_NETWORK_TAG, Nip87Error,
    NUTS_TAG, URL_TAG as NIP87_URL_TAG, create_cashu_mint_tags, create_fedimint_tags,
    create_recommendation_tags, is_cashu_mint, is_fedimint, is_mint_recommendation, is_nip87_kind,
    parse_cashu_mint, parse_fedimint, parse_recommendation,
};

// NIP-88: Polls
pub use nip88::{
    ENDS_AT_TAG, KIND_POLL, KIND_POLL_RESPONSE, Nip88Error, OPTION_TAG, POLL_EVENT_TAG,
    POLL_TYPE_TAG, PollEvent, PollOption, PollResponse, PollType, RELAY_TAG as NIP88_RELAY_TAG,
    RESPONSE_TAG, create_poll_event, create_poll_tags, create_response_tags,
    get_effective_responses, is_poll, is_poll_ended, is_poll_response, one_vote_per_pubkey,
    parse_poll, parse_poll_ends_at, parse_poll_options, parse_poll_relays, parse_poll_type,
    parse_response,
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
    create_job_feedback_event, create_job_request_event, create_job_result_event,
    get_request_kind, get_result_kind, is_dvm_kind, is_job_feedback_kind, is_job_request_kind,
    is_job_result_kind,
};

// NIP-92: Media Attachments
pub use nip92::{
    IMETA_TAG, MediaAttachment, Nip92Error, add_media_attachment, get_media_attachments,
    has_media_attachments,
};

// NIP-94: File Metadata
pub use nip94::{
    create_file_metadata_event, Dimensions, FileImage, FileMetadata, FILE_METADATA_KIND,
    Nip94Error, is_file_metadata_kind,
};

// NIP-95: File Storage on Relays
pub use nip95::{
    FILE_CONTENT_KIND, FILE_HEADER_KIND, FileContent, FileHeader, Nip95Error,
    get_alt, get_block_size, get_content_events, get_hash, get_mime_type, get_size, get_summary,
    is_file_content_kind, is_file_header_kind, is_nip95_kind,
};

// NIP-96: HTTP File Storage Integration
pub use nip96::{
    DeleteResponse, FILE_SERVER_PREFERENCE_KIND, FileMetadata as Nip96FileMetadata,
    ListFilesResponse, MediaType, Nip94Event, Nip96Error, ProcessingStatus, ServerInfo,
    ServerPlan, UploadRequest, UploadResponse, UploadStatus, WELL_KNOWN_PATH,
    construct_delete_url, construct_download_url,
};

// NIP-99: Classified Listings
pub use nip99::{
    ClassifiedListing, DraftListing, ListingImage, ListingStatus, Nip99Error, Price,
    KIND_CLASSIFIED_LISTING, KIND_DRAFT_LISTING, is_classified_listing_kind,
    is_draft_listing_kind, is_nip99_kind,
};

// NIP-C7: Chats
pub use nipc7::{
    CHAT_KIND, NipC7Error, QUOTE_TAG, QuoteReference, get_quote_reference, has_quote_tag,
    is_chat_kind,
};

// NIP-98: HTTP Auth (requires full feature)
#[cfg(feature = "full")]
pub use nip98::{
    AUTH_SCHEME, DEFAULT_TIMESTAMP_WINDOW, HttpAuth, HttpMethod, KIND_HTTP_AUTH, Nip98Error,
    ValidationParams, decode_authorization_header, encode_authorization_header, hash_payload,
    validate_http_auth_event,
};

// NIP-SA: Sovereign Agents
pub use nip_sa::{
    // Profile (kind:39200)
    AgentProfile, AgentProfileContent, AutonomyLevel, ProfileError, ThresholdConfig,
    KIND_AGENT_PROFILE,
    // State (kind:39201)
    AgentState, AgentStateContent, Goal, GoalStatus, MemoryEntry, StateError, KIND_AGENT_STATE,
    STATE_VERSION,
    // Schedule (kind:39202)
    AgentSchedule, ScheduleError, TriggerType, KIND_AGENT_SCHEDULE,
    // Goals (kind:39203)
    PublicGoals, PublicGoalsContent, PublicGoalsError, KIND_PUBLIC_GOALS,
    // Tick (kinds:39210, 39211)
    TickAction, TickError, TickRequest, TickResult, TickResultContent, TickStatus, TickTrigger,
    KIND_TICK_REQUEST, KIND_TICK_RESULT,
    // Trajectory (kinds:39230, 39231)
    StepType, TrajectoryError, TrajectoryEvent, TrajectoryEventContent, TrajectorySession,
    TrajectorySessionContent, TrajectoryVisibility, KIND_TRAJECTORY_EVENT,
    KIND_TRAJECTORY_SESSION,
    // Skill (kinds:39220, 39221)
    SkillDelivery, SkillDeliveryContent, SkillError, SkillLicense, SkillLicenseContent,
    KIND_SKILL_DELIVERY, KIND_SKILL_LICENSE,
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
