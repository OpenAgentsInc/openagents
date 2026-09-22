//! Nostr protocol primitives.
//!
//! These types implement the pinned NIP specifications in `nips/`. They do
//! not perform storage or network I/O, which keeps protocol decisions
//! deterministic and fixture-testable.

mod agent;
mod alt;
mod app_data;
mod article;
mod assertion;
mod badge;
mod block;
mod blossom;
mod bookmark;
mod calendar;
mod channel;
mod chat;
mod comment;
mod community;
mod content_warning;
mod deletion;
mod draft;
mod ecash;
mod error;
mod event;
mod expanded;
mod external_id;
mod extras;
mod file;
mod filter;
mod follow;
mod geocache;
mod gift_wrap;
mod goal;
mod handler;
mod hex;
mod highlight;
mod imeta;
mod label;
mod listing;
mod live;
mod market;
mod mentions;
mod mls;
mod monitor;
mod note;
mod nsite;
mod nutzap;
mod ots;
mod peer_order;
mod picture;
mod podcast;
mod poll;
mod pow;
mod profile_link;
mod proxy;
mod public_message;
mod reaction;
mod references;
mod relay_list;
mod remote_sign;
mod replacement;
mod report;
mod repost;
mod snippet;
mod status;
mod storage;
mod subject;
mod thread;
mod timestamp;
mod torrent;
mod vending;
mod video;
mod voice;
mod wallet;
mod wallet_connect;
mod wiki;
mod zap;

pub use agent::{
    AGENT_OBSERVER_KIND, AGENT_TURN_METRIC_KIND, AgentObserverDirection, AgentObserverRoute,
    OwnerAttestation, agent_observer_route, agent_turn_metric_owner, validate_nip44_v2_content,
    verify_agent_auth_attestation, verify_owner_attestation, verify_owner_binding,
};
pub use alt::{alt_text, fallback_summary, needs_fallback};
pub use app_data::{AppData, AppTag, open_app_data};
pub use article::{Article, is_article_reply, open_article};
pub use assertion::{
    Metric, MetricValue, ProviderSource, TrustedAssertion, TrustedProviders,
    open_trusted_assertion, open_trusted_providers, parse_provider_list,
};
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
pub use blossom::{
    MediaReference, ServerList, media_reference, open_server_list, recovery_url, verifies_media,
};
pub use bookmark::{Bookmark, bookmark_identifier, is_bookmark_reply, open_bookmark};
pub use calendar::{
    Attendance, Availability, Calendar, CalendarEvent, CalendarRef, CalendarSpan, Participant,
    Rsvp, day_stamp, open_calendar, open_calendar_event, open_rsvp,
};
pub use channel::{
    Channel, ChannelMessage, ChannelMetadata, channel_metadata, metadata_updates_channel,
    open_channel, open_channel_message, open_channel_metadata, open_hide_message, open_mute_user,
};
pub use chat::{Chat, ChatQuote, chat_filter, is_chat_reply, open_chat};
pub use comment::{Comment, CommentScope, is_top_level, open_comment};
pub use community::{
    Community, CommunityApproval, CommunityImage, CommunityModerator, CommunityPost,
    CommunityRelay, CommunityRelayRole, is_community_moderator, open_community,
    open_community_approval, open_community_post, open_community_repost,
};
pub use content_warning::{ContentWarning, content_warning};
pub use deletion::{DeletionRequest, DeletionTombstone};
pub use draft::{
    Checkpoint, DraftWrap, PrivateRelayList, RECOMMENDED_DRAFT_TTL_SECONDS, UnsignedDraft,
    open_checkpoint, open_draft_wrap, open_private_relays, seal_checkpoint, seal_draft,
    seal_private_relays, validate_checkpoint, validate_draft_wrap, validate_private_relays,
};
pub use ecash::{
    EcashNetwork, MintAnnouncement, MintEndpoint, MintKind, MintPointer, MintRecommendation,
    open_cashu_mint, open_fedimint, open_mint_recommendation,
};
pub use error::DomainError;
pub use event::{EXTENDED_INDEXED_TAG_NAMES, Event, Tag, is_indexed_tag_name};
pub use expanded::{
    GroupAction, GroupMetadata, HttpAuth, HttpAuthClaim, RelaySigner, parent_would_cycle,
    parse_http_authorization, parse_http_authorization_claim, parse_http_authorization_hash,
    reorder_children,
};
pub use external_id::{ExternalId, external_id_kind, open_external_ids};
pub use extras::{Birthday, ProfileExtras, lowercase_hashtags, open_profile_extras};
pub use file::{FileImage, FileMetadata, open_file_metadata};
pub use filter::{
    Filter, SEARCH_EXCLUDED_KINDS, matches_any, search_excludes_kind, search_matches, search_terms,
};
pub use follow::{Follow, append_follow, displayed_petname, parse_follow_list};
pub use geocache::{
    CacheLogType, CacheSize, CurationList, FoundLog, Geocache, Verification, cache_log_type,
    confirm_find, exclusive_finder, open_curation, open_found_log, open_geocache,
    open_verification, rot13,
};
pub use gift_wrap::{
    RANDOMIZE_WINDOW_SECONDS, Rumor, open_wrap, randomized_timestamp, recipient_removed_wrap,
    seal_rumor, validate_seal, validate_wrap, wrap_seal,
};
pub use goal::{
    GoalReference, GoalSubject, ZapGoal, goal_progress, open_goal_references, open_zap_goal,
    zap_counts_toward_goal, zap_request_covers_goal,
};
pub use handler::{
    AppHandler, ClientAttribution, HandlerLink, HandlerRecommendation, ManifestRef,
    RecommendedHandler, handler_url, link_for, open_client_tag, open_handler, open_recommendation,
};
pub use highlight::{
    Attribution, Highlight, HighlightRole, HighlightSource, clean_source_url, open_highlight,
};
pub use imeta::{Imeta, has_duplicate_urls, imeta_for, open_imetas, parse_imeta};
pub use label::{Label, LabelTarget, Labeling, open_labeling};
pub use listing::{Listing, ListingImage, ListingStatus, Price, open_listing};
pub use live::{
    LiveChat, LiveParticipant, LiveStatus, LiveStream, Meeting, MeetingRoom, Presence, RoomStatus,
    live_status_is_stale, open_live_chat, open_live_stream, open_meeting, open_meeting_room,
    open_presence, participation_proof, presence_is_fresh,
};
pub use market::{
    Auction, Bid, BidConfirmation, BidStatus, Checkout, MarketplacePage, Order, OrderStatus,
    PaymentRequest, Product, ShippingZone, Stall, auction_end, bid_confirmation_matches,
    open_auction, open_bid, open_bid_confirmation, open_checkout, open_marketplace, open_product,
    open_stall, product_shipping_extra, shipping_cost, shipping_zone, validate_marketplace,
};
pub use mentions::{IndexMention, note_mentions, resolve_mentions};
pub use mls::{
    KeyPackage, KeyPackageRelays, Welcome, commit_wins, exporter_conversation_key,
    inner_event_hides_the_group, open_group_message, open_key_package, open_key_package_relays,
    open_welcome,
};
pub use monitor::{
    MonitorAnnouncement, MonitorTimeout, RelayDiscovery, RelayNetwork, RelayRequirement,
    normalize_relay_url, open_monitor_announcement, open_relay_discovery,
};
pub use note::{NoteQuote, NoteRef, TextNote, is_direct_reply, open_note, reply_participants};
pub use nsite::{
    SiteFile, SiteManifest, open_site, open_site_snapshot, resolve_site_path, site_aggregate,
};
pub use nutzap::{
    Nutzap, NutzapInfo, NutzapMint, NutzapProof, nutzap_inbox, nutzap_matches, open_nutzap,
    open_nutzap_info,
};
pub use ots::{BitcoinAttestation, open_attestation};
pub use peer_order::{MakerRating, OrderSide, PeerOrder, PeerOrderStatus, open_peer_order};
pub use picture::{AnnotatedUser, PICTURE_MEDIA_TYPES, Picture, annotated_user, open_picture};
pub use podcast::{
    AuthoredPodcasts, PodcastAudio, PodcastCredit, PodcastEpisode, PodcastRole, PodcastShow,
    confirmed_hosts, open_authored_podcasts, open_podcast_episode, open_podcast_show,
};
pub use poll::{Poll, PollOption, PollResponse, PollType, open_poll, open_poll_response, tally};
pub use pow::{meets_committed_target, nonce_commitment, pow_difficulty};
pub use profile_link::{
    IdentityClaim, ProfileLinks, author_npub, expected_statement, open_profile_links, proof_url,
};
pub use proxy::{ProxySource, is_bridged, known_protocol, proxy_sources};
pub use public_message::{
    MessageReceiver, PublicMessage, link_kind, open_public_message, targets_public_message,
};
pub use reaction::{
    Reaction, ReactionVerdict, open_external_reaction, open_reaction, reaction_verdict,
};
pub use references::{TextReference, decode_reference, text_references};
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
pub use repost::{Repost, open_repost};
pub use snippet::{Snippet, SnippetLicense, SnippetRepo, open_snippet};
pub use status::{StatusType, UserStatus, is_live, open_user_status};
pub use storage::{
    FileServers, ProcessingStatus, StorageDocument, StoragePlan, UploadResponse, UploadedFile,
    open_file_servers, parse_processing_status, parse_storage_document, parse_upload_response,
};
pub use subject::{SUBJECT_BOUND, reply_subject, subject, subject_fits};
pub use thread::{ForumThread, is_thread_reply, open_thread};
pub use timestamp::TimestampPolicy;
pub use torrent::{
    CatalogId, Torrent, TorrentComment, TorrentFile, magnet_uri, open_torrent, open_torrent_comment,
};
pub use vending::{
    JobFeedback, JobInput, JobInputKind, JobParam, JobRequest, JobResult, JobStatus,
    job_result_kind, open_job_feedback, open_job_request, open_job_result,
};
pub use video::{ADDRESSABLE_VIDEO_KINDS, VIDEO_KINDS, Video, is_stream_type, open_video};
pub use voice::{VoiceMessage, open_voice_message, open_voice_reply};
pub use wallet::{
    CashuProof, CashuToken, CashuWallet, HistoryDirection, MintQuote, SpendHistory, TokenRef,
    TokenRole, WalletSecrets, open_history_event, open_mint_quote, open_quote_event,
    open_spend_history, open_token, open_token_deletion, open_token_event, open_wallet,
    open_wallet_secrets, read_wallet_content, roll_over_token, seal_wallet_content,
    token_plaintext,
};
pub use wallet_connect::{
    WalletCommand, WalletConnection, WalletEncryption, WalletError, WalletInfo, WalletReply,
    WalletRequest, WalletResponse, encryption_supported, format_connection, negotiated_encryption,
    open_wallet_info, open_wallet_request, open_wallet_response, parse_connection,
    read_wallet_command, read_wallet_message, read_wallet_reply, request_is_current,
    seal_wallet_message,
};
pub use wiki::{
    WikiArticle, WikiLink, WikiMerge, WikiRedirect, WikiTarget, WikiVersion,
    normalize_wiki_identifier, open_wiki_article, open_wiki_merge, open_wiki_redirect,
};
pub use zap::{
    ZapReceipt, ZapRequest, ZapShare, bolt11_amount_msat, decode_lnurl, open_zap_receipt,
    open_zap_request, zap_callback_query, zap_split,
};
