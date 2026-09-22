//! Nostr protocol primitives.
//!
//! These types implement the pinned NIP specifications in `nips/`. They do
//! not perform storage or network I/O, which keeps protocol decisions
//! deterministic and fixture-testable.

mod agent;
mod article;
mod badge;
mod block;
mod comment;
mod deletion;
mod draft;
mod error;
mod event;
mod expanded;
mod filter;
mod follow;
mod gift_wrap;
mod hex;
mod highlight;
mod label;
mod market;
mod ots;
mod profile_link;
mod relay_list;
mod remote_sign;
mod replacement;
mod report;
mod timestamp;
mod torrent;

pub use agent::{
    AGENT_OBSERVER_KIND, AGENT_TURN_METRIC_KIND, AgentObserverDirection, AgentObserverRoute,
    OwnerAttestation, agent_observer_route, agent_turn_metric_owner, validate_nip44_v2_content,
    verify_agent_auth_attestation, verify_owner_attestation, verify_owner_binding,
};
pub use article::{Article, is_article_reply, open_article};
pub use badge::{
    AwardRecipient, BadgeAward, BadgeDefinition, BadgeImage, BadgeSet, ProfileBadge, ProfileBadges,
    open_badge_award, open_badge_definition, open_badge_set, open_profile_badges,
};
pub use block::{
    AGENT_ENGRAM_KIND, AGENT_PERSONA_KIND, BLOCK_GLOBAL_ONLY_KINDS, DM_HIDE_KIND, DM_OPEN_KIND,
    DM_VISIBILITY_KIND, EVENT_REMINDER_KIND, IDENTITY_ARCHIVE_LIST_KIND,
    IDENTITY_ARCHIVE_REQUEST_KIND, IDENTITY_ARCHIVED_KIND, IDENTITY_UNARCHIVE_REQUEST_KIND,
    IDENTITY_UNARCHIVED_KIND, IdentityArchiveRequest, MAX_REMINDER_HORIZON_SECONDS, PROJECT_KIND,
    PUSH_LEASE_KIND, READ_STATE_KIND, RELAY_ONLY_BLOCK_KINDS, TEAM_CATALOG_KIND,
    THREAD_SUMMARY_KIND, WINDOW_BOUNDS_KIND, WORKSPACE_PROFILE_KIND, dm_visibility_channel,
    parse_identity_archive_request, validate_block_ingest, workspace_icon,
};
pub use comment::{Comment, CommentScope, is_top_level, open_comment};
pub use deletion::{DeletionRequest, DeletionTombstone};
pub use draft::{
    Checkpoint, DraftWrap, PrivateRelayList, RECOMMENDED_DRAFT_TTL_SECONDS, UnsignedDraft,
    open_checkpoint, open_draft_wrap, open_private_relays, seal_checkpoint, seal_draft,
    seal_private_relays, validate_checkpoint, validate_draft_wrap, validate_private_relays,
};
pub use error::DomainError;
pub use event::{EXTENDED_INDEXED_TAG_NAMES, Event, Tag, is_indexed_tag_name};
pub use expanded::{
    GroupAction, GroupMetadata, HttpAuth, HttpAuthClaim, RelaySigner, parent_would_cycle,
    parse_http_authorization, parse_http_authorization_claim, parse_http_authorization_hash,
    reorder_children,
};
pub use filter::{
    Filter, SEARCH_EXCLUDED_KINDS, matches_any, search_excludes_kind, search_matches, search_terms,
};
pub use follow::{Follow, append_follow, displayed_petname, parse_follow_list};
pub use gift_wrap::{
    RANDOMIZE_WINDOW_SECONDS, Rumor, open_wrap, randomized_timestamp, recipient_removed_wrap,
    seal_rumor, validate_seal, validate_wrap, wrap_seal,
};
pub use highlight::{
    Attribution, Highlight, HighlightRole, HighlightSource, clean_source_url, open_highlight,
};
pub use label::{Label, LabelTarget, Labeling, open_labeling};
pub use market::{
    Auction, Bid, BidConfirmation, BidStatus, Checkout, MarketplacePage, Order, OrderStatus,
    PaymentRequest, Product, ShippingZone, Stall, auction_end, bid_confirmation_matches,
    open_auction, open_bid, open_bid_confirmation, open_checkout, open_marketplace, open_product,
    open_stall, product_shipping_extra, shipping_cost, shipping_zone, validate_marketplace,
};
pub use ots::{BitcoinAttestation, open_attestation};
pub use profile_link::{
    IdentityClaim, ProfileLinks, author_npub, expected_statement, open_profile_links, proof_url,
};
pub use relay_list::{
    ListedRelay, RelayList, RelayMarker, open_relay_list, publish_relays, read_relays, write_relays,
};
pub use remote_sign::{
    BunkerUrl, NostrConnectUrl, Permission, RemoteRequest, RemoteResponse, RemoteSigner,
    auth_challenge_url, open_response, parse_bunker, parse_nostrconnect, seal_request,
    validate_remote_signing,
};
pub use replacement::{
    EventClass, ReplacementAddress, ReplacementDecision, compare_replacement,
    compare_replacement_order,
};
pub use report::{REPORT_TYPES, Report, ReportTarget, open_report};
pub use timestamp::TimestampPolicy;
pub use torrent::{
    CatalogId, Torrent, TorrentComment, TorrentFile, magnet_uri, open_torrent, open_torrent_comment,
};
