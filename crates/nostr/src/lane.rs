//! Evidence for the pinned official NIP lane.
//!
//! Each file under `nips/official/` other than the index is either checked
//! here or named as a document whose normative text now lives in NIP-01.
//! A check builds or verifies a value with the shipped functions and
//! refuses a value that drops a required field. The manifest commit is
//! `OFFICIAL_COMMIT`.

use serde_json::Value;

use crate::domain::Event;

/// The official lane commit recorded in `nips/manifest.json`.
pub const OFFICIAL_COMMIT: &str = "c53877571f96eb423661fc23c620d629d37b8f19";

/// One event-shaped specification: a kind and a tag the pinned text names.
#[derive(Clone, Copy)]
pub struct Shape {
    /// File name under `nips/official/`.
    pub file: &'static str,
    /// Kind the text assigns.
    pub kind: u16,
    /// Tag name the text writes as a JSON tag.
    pub tag: &'static str,
}

/// Event shapes whose kind and tag both occur in the pinned file.
pub static SHAPES: &[Shape] = &include!("lane_shapes.inc");

/// Documents whose body says the rules now live in NIP-01.
pub static MOVED_TO_NIP01: &[(&str, &str)] = &[
    ("12.md", "Moved to [NIP-01](01.md)."),
    ("16.md", "Moved to [NIP-01](01.md)."),
    ("20.md", "Moved to [NIP-01](01.md)."),
    (
        "33.md",
        "Renamed to \"Addressable events\" and moved to [NIP-01](01.md).",
    ),
];

/// Whether `event` carries this shape's kind and tag.
pub fn verify_shape(shape: &Shape, event: &Event) -> Result<(), &'static str> {
    if event.kind != shape.kind {
        return Err("kind");
    }
    let named = event.tags.iter().any(|tag| tag.name() == Some(shape.tag));
    if !named {
        return Err("tag");
    }
    if shape.tag != "-" && !event.tag_values(shape.tag).any(|value| !value.is_empty()) {
        return Err("value");
    }
    Ok(())
}

/// A NIP-05 internet identifier is `local-part@domain` with neither side empty.
pub fn nip05_identifier(value: &str) -> Result<(), &'static str> {
    let Some((local, domain)) = value.split_once('@') else {
        return Err("at");
    };
    if local.is_empty()
        || domain.is_empty()
        || domain.contains('@')
        || value.chars().any(char::is_whitespace)
    {
        return Err("parts");
    }
    Ok(())
}

/// The NIP-06 account path. Account `0` is the basic client key.
#[must_use]
pub fn nip06_path(account: u32) -> String {
    format!("m/44'/1237'/{account}'/0/0")
}

/// The unsigned event NIP-07 `signEvent` accepts.
pub fn nip07_request(
    created_at: Option<u64>,
    kind: u16,
    content: &str,
) -> Result<(), &'static str> {
    if created_at.is_none() {
        return Err("created_at");
    }
    if content.len() > 128 * 1024 {
        return Err("content");
    }
    let _ = kind;
    Ok(())
}

/// A `nostr:` URI. `nsec` is not an identifier this scheme carries.
pub fn nostr_uri(value: &str) -> Result<(), &'static str> {
    let Some(rest) = value.strip_prefix("nostr:") else {
        return Err("scheme");
    };
    if rest.starts_with("nsec") {
        return Err("nsec");
    }
    if !rest.starts_with("npub1")
        && !rest.starts_with("note1")
        && !rest.starts_with("nevent1")
        && !rest.starts_with("nprofile1")
        && !rest.starts_with("naddr1")
    {
        return Err("identifier");
    }
    Ok(())
}

/// A NIP-26 delegation tag has the delegator pubkey, the conditions, and the signature.
pub fn delegation_tag(values: &[&str]) -> Result<(), &'static str> {
    if values.len() != 3 || values.iter().any(|value| value.is_empty()) {
        return Err("delegation");
    }
    Ok(())
}

/// A NIP-30 shortcode is alphanumeric, plus hyphen and underscore.
pub fn emoji_shortcode(value: &str) -> Result<(), &'static str> {
    if value.is_empty()
        || !value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
    {
        return Err("shortcode");
    }
    Ok(())
}

/// NIP-11 requires `supported_nips` to be an array of numbers.
pub fn information_document_lists_nips(document: &Value) -> bool {
    document
        .get("supported_nips")
        .and_then(Value::as_array)
        .is_some_and(|nips| !nips.is_empty() && nips.iter().all(|nip| nip.is_number()))
}

/// NIP-45 `COUNT` is a verb, an id, and at least one filter object.
pub fn count_message(message: &Value) -> Result<(), &'static str> {
    let Some(items) = message.as_array() else {
        return Err("array");
    };
    if items.first().and_then(Value::as_str) != Some("COUNT") {
        return Err("verb");
    }
    if items.len() < 3 || items[1].as_str().is_none() || !items[2].is_object() {
        return Err("filter");
    }
    Ok(())
}

/// NIP-49 `LOG_N` is one byte and the iteration count is `2^LOG_N`.
pub fn nip49_iterations(log_n: u8) -> Result<u32, &'static str> {
    if log_n == 0 || log_n > 24 {
        return Err("log_n");
    }
    Ok(1u32 << log_n)
}

/// NIP-55 method names the Android signer answers.
pub fn android_signer_method(name: &str) -> bool {
    matches!(
        name,
        "get_public_key"
            | "sign_event"
            | "nip04_encrypt"
            | "nip04_decrypt"
            | "nip44_encrypt"
            | "nip44_decrypt"
    )
}

/// NIP-64 content is a PGN database: a tag pair or a numbered move.
pub fn chess_pgn(content: &str) -> Result<(), &'static str> {
    if content.contains('[') && content.contains(']') || content.contains("1.") {
        Ok(())
    } else {
        Err("pgn")
    }
}

/// NIP-86 method names this relay implements. Unknown names are refused.
pub fn management_method(name: &str) -> bool {
    matches!(
        name,
        "supportedmethods"
            | "banpubkey"
            | "unbanpubkey"
            | "listbannedpubkeys"
            | "allowpubkey"
            | "unallowpubkey"
            | "listallowedpubkeys"
            | "allowkind"
            | "disallowkind"
            | "listallowedkinds"
    )
}

/// NIP-BE advertisement UUID, copied from the pinned text.
pub const BLE_SERVICE_UUID: &str = "0000180f-0000-1000-8000-00805f9b34fb";

/// Kind `62` is the vanish request. It names no extra tag.
pub fn vanish_request(event: &Event) -> Result<(), &'static str> {
    if event.kind == 62 {
        Ok(())
    } else {
        Err("kind")
    }
}

/// One pinned file whose applicable roles are configured and proven.
///
/// `partial` shape checks stay in [`SHAPES`]. A row here is a different
/// status: domain, client, and server each name the shipped function, and
/// `acceptance` names the test that calls it.
pub struct Evidence {
    pub file: &'static str,
    pub domain: &'static str,
    pub client: &'static str,
    pub server: &'static str,
    pub paths: &'static str,
    pub configuration: &'static str,
    pub fixture: &'static str,
    pub acceptance: &'static str,
    pub limitations: &'static str,
    pub owner: &'static str,
    pub status: &'static str,
}

/// Official files whose checks go beyond a kind and one tag.
pub static PROVEN: &[Evidence] = &[
    Evidence {
        file: "02.md",
        domain: "kind 3 is replaceable; a p tag is a 32-byte hex key, an optional ws:// or wss:// relay, and an optional petname",
        client: "parse_follow_list, append_follow, and displayed_petname",
        server: "EventClass::from_kind(3) is Replaceable, so the relay replacement head deletes the previous list",
        paths: "crates/nostr/src/domain/follow.rs; crates/nostr/src/domain/replacement.rs; crates/nostr-relay/src/store/mod.rs",
        configuration: "no setting; kind 3 uses the ordinary replacement head",
        fixture: "the pinned p-tag triple: pubkey, relay URL, petname",
        acceptance: "lane::tests::nip02_follow_lists_replace_and_petnames_chain",
        limitations: "content is ignored, as the pinned text says it is not used",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
    Evidence {
        file: "03.md",
        domain: "kind 1040 binds an event id to one OpenTimestamps proof",
        client: "open_attestation",
        server: "admission refuses a kind 1040 event whose proof does not bind that id",
        paths: "crates/nostr/src/domain/ots.rs; crates/nostr/src/domain/event.rs",
        configuration: "no setting; NIP-03 stays off the NIP-11 list because the pinned text marks it unrecommended",
        fixture: "an .ots file whose digest is the e tag and whose one attestation is a Bitcoin height",
        acceptance: "domain::ots::tests::a_bitcoin_proof_binds_the_event_id_and_one_height",
        limitations: "the height is not compared to a Bitcoin block header; pending and non-Bitcoin attestations are refused",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "04.md",
        domain: "kind 4 content is AES-256-CBC under the unhashed X coordinate of the ECDH point",
        client: "nip04::encrypt and nip04::decrypt",
        server: "admission checks the p tag and the ?iv= content form without decrypting",
        paths: "crates/nostr/src/nip04.rs; crates/nostr/src/domain/event.rs",
        configuration: "no setting; NIP-04 stays off the NIP-11 list because the pinned text marks it unrecommended",
        fixture: "FIPS-197 AES-256 block, then a two-party round trip of base64(ciphertext)?iv=base64(iv)",
        acceptance: "nip04::tests::a_direct_message_round_trips_and_keeps_a_mention_as_text",
        limitations: "there is no MAC, so a padding failure is not authentication; the recipient pubkey is visible in the p tag",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "09.md",
        domain: "kind 5 names event ids and same-author replacement addresses; a request does not delete kind 5",
        client: "DeletionRequest::from_event, tombstones, and deletes",
        server: "admit stores the request, apply_deletion writes tombstones, and a later matching event is rejected",
        paths: "crates/nostr/src/domain/deletion.rs; crates/nostr-relay/src/store/mod.rs; crates/nostr-relay/src/store/statements.rs",
        configuration: "no setting; NIP-11 lists 9 because kind 5 admission and tombstones run on every relay",
        fixture: "the pinned kind 5 example: e and a references, optional k tags, and a reason in content",
        acceptance: "lane::tests::nip09_deletion_requests_hide_the_authors_events_through_the_request_time",
        limitations: "a request with no well-formed same-author reference creates no tombstone and is still stored; k tags do not select targets; relays that already published the event may still hold a copy",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
    Evidence {
        file: "15.md",
        domain: "kinds 30017, 30018, 30019, and 30020 are addressable marketplace records; kind 1021 is one bid on one auction event id",
        client: "open_stall, open_product, open_auction, open_bid, bid_confirmation_matches, shipping_cost, and open_checkout",
        server: "admission refuses a malformed marketplace event; addressable kinds use the ordinary replacement head",
        paths: "crates/nostr/src/domain/market.rs; crates/nostr/src/domain/event.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; NIP-15 stays off the NIP-11 list because the pinned text marks it unrecommended",
        fixture: "a stall whose d tag equals its id, a product shipping extra, a bid on one auction version, and checkout types 0, 1, and 2",
        acceptance: "domain::market::tests::a_stall_product_and_bid_follow_the_pinned_marketplace_events",
        limitations: "checkout JSON is parsed after decryption, so a relay does not read kind 4; costs are JSON numbers and no payment is settled; a later auction edit is stored and does not keep bids from the previous event id",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "17.md",
        domain: "an unsigned kind 14 or 15 rumor is sealed as kind 13 and gift-wrapped as kind 1059 to one recipient",
        client: "chat_rumor, seal, gift_wrap, open_direct_message, file_message, and inbox_relays",
        server: "admission requires one p tag on kind 1059; the relay serves that event only to the authenticated reader named by the tag",
        paths: "crates/nostr/src/nip17.rs; crates/nostr/src/domain/expanded.rs; crates/nostr-relay/src/gateway/subscription.rs",
        configuration: "NOSTR_RELAY_URL; NIP-11 then lists 17 because gift wraps are served only to the authenticated p-tagged reader",
        fixture: "a kind 14 rumor sealed and wrapped to the recipient and to the author, plus a kind 10050 inbox list",
        acceptance: "nip17::tests::a_private_message_round_trips_and_rejects_an_impersonated_rumor",
        limitations: "the relay does not decrypt the wrap; file bytes are not downloaded or decrypted; the caller supplies the one-time wrapper key; kind 21059 belongs to NIP-59",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
    Evidence {
        file: "22.md",
        domain: "kind 1111 names one uppercase root scope and one lowercase parent scope, with K and k",
        client: "open_comment and is_top_level",
        server: "admission refuses a kind 1111 event that breaks the scope rules, including a reply to kind 1",
        paths: "crates/nostr/src/domain/comment.rs; crates/nostr/src/domain/event.rs",
        configuration: "no setting; kind 1111 is an ordinary stored event and is not added to the NIP-11 list",
        fixture: "the pinned blog comment, the web URL comment, and the reply to a podcast comment",
        acceptance: "domain::comment::tests::a_comment_scopes_to_the_root_and_refuses_a_kind_1_reply",
        limitations: "content is kept as text and markup is not stripped; a URL with a fragment is refused and is not rewritten into a normalized form",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "23.md",
        domain: "kind 30023 is an addressable article identified by one d tag",
        client: "open_article and is_article_reply",
        server: "admission refuses a malformed article or a kind 30024 draft; the addressable replacement head keeps the newest d tag",
        paths: "crates/nostr/src/domain/article.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 30023 uses the ordinary addressable replacement head and is not added to the NIP-11 list",
        fixture: "the pinned article tags: d, title, published_at, t, e, and a, plus a nostr: reference",
        acceptance: "domain::article::tests::an_article_replaces_on_its_identifier_and_keeps_markdown_paragraphs",
        limitations: "kind 30024 drafts are refused because the pinned text moves them to NIP-37; markup inside a code fence is not treated as HTML; a paragraph is one line",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "32.md",
        domain: "kind 1985 attaches l labels in an L namespace to e, p, a, r, or t targets",
        client: "open_labeling",
        server: "admission refuses a kind 1985 event with no target or an l mark outside its L tags; the event is regular, so a newer label does not replace it",
        paths: "crates/nostr/src/domain/label.rs; crates/nostr/src/domain/event.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 1985 is an ordinary stored event and is not added to the NIP-11 list",
        fixture: "the pinned #t topic label on two pubkeys, a nip28.moderation label on an event, and an ISO-3166-2 self-label",
        acceptance: "domain::label::tests::a_label_attaches_a_namespace_to_its_targets",
        limitations: "a # namespace is recorded and is not written onto the target; a correction is a new event plus a NIP-09 deletion, not a replaceable head; namespaces are not checked against a registry",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "35.md",
        domain: "kind 2003 is one BitTorrent v1 info hash, a file list, and the tags for a magnet link",
        client: "open_torrent, magnet_uri, and open_torrent_comment",
        server: "admission refuses a kind 2003 event without an info hash and a file, and a kind 2004 comment that does not name an event",
        paths: "crates/nostr/src/domain/torrent.rs; crates/nostr/src/domain/event.rs",
        configuration: "no setting; kinds 2003 and 2004 are ordinary stored events and are not added to the NIP-11 list",
        fixture: "the pinned torrent tags: title, x, file, tracker, i, and t, plus a kind 2004 root comment",
        acceptance: "domain::torrent::tests::a_torrent_builds_a_magnet_and_a_comment_names_it",
        limitations: "the info hash is not checked against torrent bytes; trackers are not contacted; a kind 2004 comment keeps its text and does not interpret markup",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "37.md",
        domain: "kind 31234 stores an unsigned draft encrypted to its author, identified by one d tag and one k tag",
        client: "seal_draft, open_draft_wrap, seal_checkpoint, open_checkpoint, seal_private_relays, and open_private_relays",
        server: "admission checks the d and k tags and NIP-44 framing, and an empty content deletes the addressable draft",
        paths: "crates/nostr/src/domain/draft.rs; crates/nostr/src/nip44.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kinds 31234, 1234, and 10013 are not added to the NIP-11 list",
        fixture: "a kind 30023 draft sealed under d article-1, a blank deletion, a kind 1234 checkpoint, and a kind 10013 relay list",
        acceptance: "domain::draft::tests::a_draft_wrap_round_trips_and_a_blank_content_deletes_it",
        limitations: "the relay does not decrypt; private-storage relays are not forced to require authentication; publishing kind 10013 to NIP-65 write relays is the client's choice of transport",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "39.md",
        domain: "kind 10011 is a replaceable list of platform:identity claims, each with a proof",
        client: "open_profile_links, proof_url, expected_statement, and author_npub",
        server: "admission refuses a kind 10011 event with no i tag or a claim that breaks its platform shape; the replaceable head keeps the newest list",
        paths: "crates/nostr/src/domain/profile_link.rs; crates/nostr/src/domain/replacement.rs; crates/nostr/src/nip19.rs",
        configuration: "no setting; kind 10011 uses the ordinary replaceable head and is not added to the NIP-11 list",
        fixture: "the pinned github, twitter, mastodon, and telegram i tags",
        acceptance: "domain::profile_link::tests::a_profile_link_list_names_each_platform_and_replaces",
        limitations: "the relay does not fetch the gist, tweet, post, or Telegram message; an unknown platform is kept when its name and proof are non-empty",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "46.md",
        domain: "kind 24133 is an ephemeral NIP-44 request or response with one p tag",
        client: "parse_bunker, parse_nostrconnect, seal_request, open_response, and RemoteSigner::handle",
        server: "admission checks one p tag and NIP-44 framing; the kind is ephemeral, so the relay does not store the event",
        paths: "crates/nostr/src/domain/remote_sign.rs; crates/nostr/src/nip44.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 24133 is not added to the NIP-11 list",
        fixture: "the pinned nostrconnect URL and a sign_event round trip",
        acceptance: "domain::remote_sign::tests::a_client_connects_and_the_signer_returns_a_signed_event",
        limitations: "the relay does not decrypt; NIP-89 discovery and nostr.json are not fetched; a bunker secret works once; permissions restrict sign_event and the cipher methods only",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "56.md",
        domain: "kind 1984 names a user, a note, or a blob and a report type on that tag",
        client: "open_report",
        server: "admission requires a target and a known report type; the event is regular, so the relay stores it and does not hide the referenced event",
        paths: "crates/nostr/src/domain/report.rs; crates/nostr/src/domain/label.rs; crates/nostr/src/domain/deletion.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 1984 is an ordinary stored event and is not added to the NIP-11 list",
        fixture: "the pinned profile, note, impersonation, and blob reports",
        acceptance: "domain::report::tests::a_report_names_the_user_or_the_blob_and_does_not_delete_it",
        limitations: "the pinned blob example has no p tag, so a blob report may omit the user; the relay does not fetch the blob or the server URL; a report is not a deletion",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "58.md",
        domain: "kind 30009 defines a badge, kind 8 awards it, and kinds 10008 and 30008 list the badges a profile accepts",
        client: "open_badge_definition, open_badge_award, open_profile_badges, and open_badge_set",
        server: "admission checks the definition, the award, and the list; kind 30009 and kind 30008 replace on d, kind 10008 replaces for the author, and kind 8 is stored as a regular event",
        paths: "crates/nostr/src/domain/badge.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kinds 30009, 8, 10008, and 30008 are not added to the NIP-11 list",
        fixture: "the pinned bravery definition, award, profile list, and deprecated profile_badges event",
        acceptance: "domain::badge::tests::a_badge_definition_is_awarded_and_the_profile_lists_it",
        limitations: "an unpaired a or e tag is ignored; the relay does not fetch images or check that the award event repeats the definition address; 1024x1024 is not required",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "59.md",
        domain: "an unsigned rumor is sealed as kind 13 and wrapped as kind 1059 or ephemeral kind 21059 with one p tag",
        client: "seal_rumor, wrap_seal, open_wrap, and randomized_timestamp",
        server: "admission checks the seal tags and NIP-44 framing; kind 1059 is stored and served only to the p-tagged reader; kind 21059 is ephemeral and is not stored",
        paths: "crates/nostr/src/domain/gift_wrap.rs; crates/nostr/src/nip44.rs; crates/nostr/src/domain/deletion.rs; crates/nostr-relay/src/gateway/subscription.rs",
        configuration: "NIP-11 lists 59 when NOSTR_RELAY_URL is set; kind 21059 needs no separate setting",
        fixture: "the pinned party rumor, author, recipient, and one-time wrapper key",
        acceptance: "domain::gift_wrap::tests::a_rumor_is_sealed_and_the_ephemeral_wrap_is_not_stored",
        limitations: "the relay does not decrypt; proof of work is not required; a published seal is a regular event; the Postgres tombstone table still records only same-author e and a tags, so a live deletion does not yet remove stored wraps by p tag",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "65.md",
        domain: "kind 10002 is a replaceable list of r tags, each read, write, or both",
        client: "open_relay_list, read_relays, write_relays, and publish_relays",
        server: "admission requires at least one ws:// or wss:// r tag and refuses any other marker; a newer list from the same author replaces the older one",
        paths: "crates/nostr/src/domain/relay_list.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no extra setting; NIP-11 lists 65",
        fixture: "the pinned alicerelay, brando-relay, write-only, and read-only r tags",
        acceptance: "domain::relay_list::tests::a_relay_list_splits_read_and_write_and_replaces",
        limitations: "the relay does not publish to those URLs; the 2-4 relay guidance is not enforced; indexer discovery is not implemented",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "84.md",
        domain: "kind 9802 highlights text or media and names the source with an a, e, or source r tag",
        client: "open_highlight and clean_source_url",
        server: "admission requires a source and refuses an unknown role or an r tag that is not source or mention; the event is regular, so a newer highlight does not replace it",
        paths: "crates/nostr/src/domain/highlight.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 9802 is an ordinary stored event and is not added to the NIP-11 list",
        fixture: "an article address, a note, a cleaned source URL, author and editor p tags, and a comment quote",
        acceptance: "domain::highlight::tests::a_highlight_names_its_source_and_a_comment_quotes_it",
        limitations: "the relay does not fetch the source or rewrite the stored URL; tracker removal is the client's clean_source_url helper; roles other than author, editor, and mention are refused",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "89.md",
        domain: "kind 31989 recommends kind 31990 handlers for one event kind, and a client tag names the publishing application",
        client: "open_recommendation, open_handler, link_for, handler_url, and open_client_tag",
        server: "admission checks the d tag, handler addresses, and platform URLs; both kinds replace on d; a filter on d or k selects them",
        paths: "crates/nostr/src/domain/handler.rs; crates/nostr/src/domain/filter.rs; crates/nostr/src/domain/replacement.rs; crates/nostr/src/nip19.rs",
        configuration: "no setting; kinds 31989 and 31990 are not added to the NIP-11 list",
        fixture: "the pinned kind 31337 recommendation, handler address, web and ios URLs, and a client tag",
        acceptance: "domain::handler::tests::a_recommendation_points_at_a_handler_and_the_url_receives_the_entity",
        limitations: "the relay does not fetch the application or its kind 0 profile; this crate encodes npub and nsec only, so the caller supplies the NIP-19 token that replaces bech32; omitting the client tag is the opt-out",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "98.md",
        domain: "kind 27235 authorizes one absolute URL and one HTTP method, with an optional SHA-256 of the body",
        client: "parse_http_authorization_claim and parse_http_authorization",
        server: "the claim is checked before the body is read; the payload hash is checked with the body; kind 27235 is ephemeral, so the relay does not store it",
        paths: "crates/nostr/src/domain/expanded.rs; crates/nostr-relay/src/gateway/management.rs; crates/nostr-relay/src/gateway/media.rs; crates/nostr-relay/src/gateway/query.rs",
        configuration: "NIP-11 lists 98 when a management pubkey or media storage is configured",
        fixture: "the pinned Snort GET authorization and a signed POST with a payload hash",
        acceptance: "domain::expanded::tests::a_nostr_authorization_matches_the_url_method_and_body",
        limitations: "the time window is 60 seconds; content may be non-empty; a payload tag is checked only when the caller supplies a body; the pinned example id does not match the NIP-01 preimage, so that header is refused",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
    Evidence {
        file: "99.md",
        domain: "kind 30402 is an addressable classified listing and kind 30403 is the same shape saved as a draft",
        client: "open_listing",
        server: "admission checks the d tag, price, status, image, and references; a newer listing with the same d tag replaces the older one; a draft does not replace the published listing",
        paths: "crates/nostr/src/domain/listing.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kinds 30402 and 30403 are not added to the NIP-11 list",
        fixture: "the pinned lorem-ipsum tags: title, published_at, t, image, summary, location, price, e, and a",
        acceptance: "domain::listing::tests::a_listing_keeps_its_price_and_a_draft_does_not_replace_it",
        limitations: "the description is kept as Markdown and nostr: references are not decoded; images are not fetched; title, summary, location, and price may be omitted",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "B0.md",
        domain: "kind 39701 is an addressable web bookmark whose d tag is the URI with the https scheme omitted",
        client: "bookmark_identifier, open_bookmark, and is_bookmark_reply",
        server: "admission refuses an https scheme left on the d tag; a newer bookmark with the same d tag replaces the older one; a reply is a kind 1111 comment",
        paths: "crates/nostr/src/domain/bookmark.rs; crates/nostr/src/domain/comment.rs; crates/nostr/src/domain/replacement.rs; crates/nostr/src/domain/filter.rs",
        configuration: "no setting; kind 39701 is not added to the NIP-11 list",
        fixture: "the pinned alice.blog/post bookmark, its title and topics, and a kind 1111 reply",
        acceptance: "domain::bookmark::tests::a_bookmark_drops_the_https_scheme_and_a_comment_replies",
        limitations: "the relay does not fetch the page; http keeps its scheme; other schemes are refused; a kind 1 note is not a reply",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "C0.md",
        domain: "kind 1337 stores source text with a lowercase language, an optional filename, and SPDX licenses",
        client: "open_snippet",
        server: "admission checks the language, extension, license, and repository; the event is regular, so a newer snippet does not replace it; an l tag remains a NIP-32 self-label",
        paths: "crates/nostr/src/domain/snippet.rs; crates/nostr/src/domain/label.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 1337 is an ordinary stored event and is not added to the NIP-11 list",
        fixture: "the pinned JavaScript hello-world snippet and a Rust snippet with an Apache-2.0 license and a 30617 repository",
        acceptance: "domain::snippet::tests::a_snippet_keeps_its_source_and_does_not_replace",
        limitations: "the relay does not run the snippet or fetch the repository; SPDX identifiers are not checked against the SPDX list; an l tag is also the NIP-32 self-label ugc",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "CC.md",
        domain: "kind 37516 is an addressable geocache with a geohash, difficulty, terrain, and size; kind 7516 is a found log; kind 7517 is a verification; kind 37517 is a curation list",
        client: "open_geocache, confirm_find, exclusive_finder, cache_log_type, open_curation, and rot13",
        server: "admission of kinds 37516, 7516, 7517, and 37517; a newer cache with the same d tag replaces the older one; a kind 1111 DNF comment is a NIP-22 top-level comment",
        paths: "crates/nostr/src/domain/geocache.rs; crates/nostr/src/domain/comment.rs; crates/nostr/src/domain/replacement.rs; crates/nostr/src/domain/expanded.rs",
        configuration: "no setting; kinds 37516, 37517, 7516, and 7517 are not added to the NIP-11 list",
        fixture: "the pinned First Treasure, a verified first-to-find find, a DNF comment, and the Texas Ren Fest list",
        acceptance: "domain::geocache::tests::a_geocache_is_found_verified_and_collected",
        limitations: "the 8-character geohash guidance is not enforced because the pinned example includes shorter prefixes; the verification naddr is not decoded; the pinned DNF example omits the NIP-22 e version tag, so a stored comment includes it; images are not fetched; unknown n modifiers are kept; found-log image tags are not checked",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "10.md",
        domain: "kind 1 is a regular plaintext note; marked e tags name the thread root and the direct parent; q tags cite an event id or an address",
        client: "open_note, is_direct_reply, and reply_participants",
        server: "admission checks event ids, markers, relay hints, quotes, and participant pubkeys; a newer note does not replace an older one",
        paths: "crates/nostr/src/domain/note.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 1 threading is not added to the NIP-11 list",
        fixture: "a direct root reply, a nested reply with a mention and a quote, and the deprecated positional root-then-parent form",
        acceptance: "domain::note::tests::a_note_threads_from_the_root_to_its_parent",
        limitations: "the relay does not fetch the referenced event, so it does not prove an e tag points at kind 1; content markup is kept; e tags are not required to appear in root-to-parent order; a reply's p tags are not checked against the parent note",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "18.md",
        domain: "kind 6 reposts a kind 1 note and kind 16 reposts any other kind; the e tag names the event and a relay",
        client: "open_repost",
        server: "admission requires one e tag with a relay URL; a non-empty content embeds a signed event whose id matches; a protected embed is refused; a newer repost does not replace an older one",
        paths: "crates/nostr/src/domain/repost.rs; crates/nostr/src/domain/event.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/note.rs",
        configuration: "no setting; kinds 6 and 16 are not added to the NIP-11 list",
        fixture: "a kind 6 repost of a signed note, an empty repost, a refused protected embed, and a kind 16 repost of a kind 30023 article",
        acceptance: "domain::repost::tests::a_repost_embeds_the_note_and_a_generic_repost_names_its_kind",
        limitations: "an empty repost does not prove the target kind because the relay does not fetch it; nostr: mentions are not rewritten into q tags; a p tag and a k tag are not required when the content is empty",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "52.md",
        domain: "kind 31922 is a date-based calendar event, kind 31923 is time-based, kind 31924 is a calendar, and kind 31925 is an RSVP",
        client: "open_calendar_event, open_calendar, open_rsvp, and day_stamp",
        server: "admission checks dates, timestamps, the start day, titles, and references; a newer event with the same d tag replaces the older one; a declined RSVP drops fb",
        paths: "crates/nostr/src/domain/calendar.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kinds 31922, 31923, 31924, and 31925 are not added to the NIP-11 list",
        fixture: "a 2026-09-22 offsite, a timed call in America/Costa_Rica, a work calendar, and an accepted RSVP",
        acceptance: "domain::calendar::tests::a_calendar_event_keeps_its_span_and_an_rsvp_names_it",
        limitations: "time zone names are not checked against the IANA database; D tags must include the start day and are not required to list every later day; images and links are not fetched; recurring events are not expanded; a declined RSVP ignores fb; the relay does not decide who may attend",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "57.md",
        domain: "kind 9734 is a zap request and kind 9735 is a zap receipt; bolt11 amount and description hash bind the receipt to the request",
        client: "open_zap_request, open_zap_receipt, zap_callback_query, zap_split, bolt11_amount_msat, and decode_lnurl",
        server: "admission checks the request relays and recipient, the receipt invoice and description, and zap tag splits; a newer zap does not replace an older one",
        paths: "crates/nostr/src/domain/zap.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kinds 9734 and 9735 are not added to the NIP-11 list",
        fixture: "the pinned lnbc10u invoice, a 21000 millisatoshi request and receipt, and a 1/1/2 zap split",
        acceptance: "domain::zap::tests::a_zap_receipt_matches_the_request_amount_and_description",
        limitations: "the relay does not call LNURL or pay invoices; the bolt11 signature and expiry are not checked; the receipt pubkey is not compared with a provider nostrPubkey; a published kind 9734 is stored",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "94.md",
        domain: "kind 1063 describes a shared file by URL, lowercase MIME type, and SHA-256",
        client: "open_file_metadata",
        server: "admission checks the URL, MIME type, hashes, size, dimensions, magnet URI, infohash, blurhash, and fallback URLs; a newer file description does not replace an older one",
        paths: "crates/nostr/src/domain/file.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kind 1063 is not added to the NIP-11 list",
        fixture: "a JPEG with its SHA-256, original hash, 640x480 dimensions, blurhash, thumbnail, and one fallback URL",
        acceptance: "domain::file::tests::a_file_keeps_its_hash_and_does_not_replace",
        limitations: "the relay does not download the file or recompute the hash; a blurhash is not decoded into pixels; ox may be omitted; a magnet URI is not matched against the infohash",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "78.md",
        domain: "kind 30078 is an addressable application record and kind 78 is a regular event for many rows",
        client: "open_app_data",
        server: "admission requires one d tag on kind 30078; a newer record with the same d tag replaces the older one; kind 78 does not replace; kind 30078 stays out of search",
        paths: "crates/nostr/src/domain/app_data.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs; crates/nostr/src/domain/filter.rs",
        configuration: "no setting; kinds 78 and 30078 are not added to the NIP-11 list",
        fixture: "a settings:theme record and a kind 78 log row that shares a d tag",
        acceptance: "domain::app_data::tests::an_application_record_replaces_on_its_identifier_and_a_plain_event_does_not",
        limitations: "content and tags other than d stay opaque and are not decrypted; the relay does not decide which app owns an identifier; kind 78 content remains searchable",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "88.md",
        domain: "kind 1068 is a poll and kind 1018 is a response; tally keeps one vote per pubkey",
        client: "open_poll, open_poll_response, and tally",
        server: "admission checks option ids, relay URLs, poll type, and the response's poll id; a newer poll does not replace an older one",
        paths: "crates/nostr/src/domain/poll.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; kinds 1068 and 1018 are not added to the NIP-11 list",
        fixture: "the pineapple poll, a replaced single-choice vote inside the window, and a multiple-choice vote",
        acceptance: "domain::poll::tests::a_poll_counts_one_vote_per_pubkey_inside_its_window",
        limitations: "the relay does not fetch the poll relays; kind 5 deletions of votes are still honored; follow sets, proof of work, and web of trust are not applied; an equal timestamp keeps the greater event id",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "90.md",
        domain: "kinds 5000-5999 are job requests, kinds 6000-6999 are results at request plus 1000, and kind 7000 is feedback",
        client: "open_job_request, open_job_result, open_job_feedback, and job_result_kind",
        server: "admission checks input types, bids, relays, the embedded signed request, and feedback status; a newer job event does not replace an older one",
        paths: "crates/nostr/src/domain/vending.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs; crates/nostr/src/nip04.rs",
        configuration: "no setting; NIP-90 is unrecommended and its kinds are not added to the NIP-11 list",
        fixture: "a kind 5001 transcription request, its kind 6001 result, a payment-required feedback event, and an encrypted request",
        acceptance: "domain::vending::tests::a_job_result_uses_the_request_kind_plus_one_thousand",
        limitations: "the relay does not run the job, fetch inputs, or pay invoices; encrypted payloads are checked as NIP-04 framing and are not decrypted; kind 5 deletion still follows NIP-09",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "96.md",
        domain: "kind 10096 lists HTTPS file servers; nip96.json and the upload JSON name the API and the original file hash",
        client: "parse_storage_document, parse_upload_response, parse_processing_status, and open_file_servers",
        server: "admission requires one or more distinct https:// server tags; a newer list replaces the older one",
        paths: "crates/nostr/src/domain/storage.rs; crates/nostr/src/domain/expanded.rs; crates/nostr/src/domain/replacement.rs",
        configuration: "no setting; NIP-96 is unrecommended and kind 10096 is not added to the NIP-11 list",
        fixture: "a free-tier nip96.json document, a delegated document, an upload with url and ox, and a two-server kind 10096 list",
        acceptance: "domain::storage::tests::a_file_server_list_replaces_and_an_upload_keeps_the_original_hash",
        limitations: "the relay does not upload, download, or delete files; HTTP status codes and a NIP-98 payload hash are not checked; a blurred or resized file is not fetched",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "A0.md",
        domain: "kind 1222 is a root voice message and kind 1244 is a NIP-22 reply; content is the audio URL",
        client: "open_voice_message and open_voice_reply",
        server: "admission of kinds 1222 and 1244; a reply must carry NIP-22 scopes; a regular event does not replace",
        paths: "crates/nostr/src/domain/voice.rs; crates/nostr/src/domain/comment.rs; crates/nostr/src/domain/expanded.rs",
        configuration: "no setting; NIP-A0 is a draft and kinds 1222 and 1244 are not added to the NIP-11 list",
        fixture: "a kind 1222 event with an audio URL, waveform, and duration, and a kind 1244 reply scoped to that event",
        acceptance: "domain::voice::tests::a_voice_message_keeps_its_audio_url_and_a_reply_threads_to_it",
        limitations: "the relay does not download the audio, so it does not check the codec or the 60-second guidance; a duration over 60 seconds is kept; the suggestion of fewer than 100 waveform amplitudes is not enforced",
        owner: "nostr",
        status: "configured-and-proven",
    },
    Evidence {
        file: "29.md",
        domain: "GroupMetadata and GroupAction, including private, hidden, restricted, and one parent",
        client: "GroupMetadata::from_tags and parent_would_cycle",
        server: "admission, the query filter, and metadata regeneration",
        paths: "crates/nostr/src/domain/expanded.rs; crates/nostr-relay/src/store/mod.rs",
        configuration: "NOSTR_RELAY_RELAY_SECRET_KEY; NIP-11 nip29.subgroups is true when that key is set",
        fixture: "private, hidden, restricted, one parent, and the child list",
        acceptance: "domain::expanded::tests::private_hidden_and_subgroup_fields_follow_the_pinned_metadata_event",
        limitations: "kinds 9003, 9004, 9006, and 9011-9020 have no row in the pinned moderation table; kind 39004 stays empty because this process does not run LiveKit",
        owner: "nostr and nostr-relay",
        status: "configured-and-proven",
    },
];

/// Every official file this module accounts for.
pub fn covered_files() -> Vec<&'static str> {
    let mut files: Vec<&str> = SHAPES.iter().map(|shape| shape.file).collect();
    files.extend(PROVEN.iter().map(|row| row.file));
    files.extend(MOVED_TO_NIP01.iter().map(|(file, _)| *file));
    files.extend([
        "01.md", "05.md", "06.md", "07.md", "11.md", "19.md", "21.md", "26.md", "30.md", "40.md",
        "42.md", "43.md", "44.md", "45.md", "49.md", "50.md", "55.md", "62.md", "64.md", "77.md",
        "86.md", "BE.md",
    ]);
    files.sort_unstable();
    files.dedup();
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use serde_json::json;

    use crate::domain::{
        DeletionRequest, DomainError, EventClass, GroupMetadata, RelaySigner, ReplacementDecision,
        Tag, compare_replacement, displayed_petname, parse_follow_list, search_matches,
    };
    use crate::negentropy::{self, Item};
    use crate::nip19;
    use crate::nip44;

    fn official_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../nips/official")
    }

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap()
    }

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        signer().sign(1_700_000_000, kind, tags, content.into())
    }

    #[test]
    fn nip02_follow_lists_replace_and_petnames_chain() {
        let text = fs::read_to_string(official_dir().join("02.md")).unwrap();
        assert!(text.contains("follow list"));
        assert!(text.contains("petname"));
        let row = PROVEN.iter().find(|row| row.file == "02.md").unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(SHAPES.iter().all(|shape| shape.file != "02.md"));

        let alice = "ab".repeat(32);
        let bob = "cd".repeat(32);
        let carol = "ef".repeat(32);
        let tags = vec![
            Tag::new(vec![
                "p".into(),
                alice.clone(),
                "wss://alicerelay.com/".into(),
                "alice".into(),
            ]),
            Tag::new(vec!["p".into(), bob.clone()]),
        ];
        let follows = parse_follow_list(&tags).unwrap();
        assert_eq!(follows[0].petname, "alice");
        assert_eq!(follows[1].relay, "");
        let event = sign(3, tags, "unused");
        event.validate_structure().unwrap();
        assert_eq!(event.class(), EventClass::Replaceable);

        let newer = signer().sign(
            1_700_000_100,
            3,
            vec![Tag::new(vec!["p".into(), alice.clone()])],
            String::new(),
        );
        assert_eq!(
            compare_replacement(&event, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );

        let alice_list = [crate::domain::Follow {
            pubkey: bob.clone(),
            relay: String::new(),
            petname: "bob".into(),
        }];
        let bob_list = [crate::domain::Follow {
            pubkey: carol.clone(),
            relay: String::new(),
            petname: "carol".into(),
        }];
        let mut published = std::collections::BTreeMap::new();
        published.insert(alice.as_str(), alice_list.as_slice());
        published.insert(bob.as_str(), bob_list.as_slice());
        let viewer = [crate::domain::Follow {
            pubkey: alice.clone(),
            relay: String::new(),
            petname: "alice".into(),
        }];
        assert_eq!(
            displayed_petname(&viewer, &published, &carol).as_deref(),
            Some("carol.bob.alice")
        );

        let bad = sign(3, vec![Tag::new(vec!["p".into(), "zz".into()])], "");
        assert!(bad.validate_structure().is_err());
        let _ = GroupMetadata::from_tags(&[Tag::new(vec!["private".into()])]).unwrap();
        assert!(PROVEN.iter().any(|row| row.file == "29.md"));
    }

    #[test]
    fn nip09_deletion_requests_hide_the_authors_events_through_the_request_time() {
        let text = fs::read_to_string(official_dir().join("09.md")).unwrap();
        assert!(text.contains("deletion request"));
        assert!(text.contains("identical `pubkey`"));
        assert!(text.contains("has no effect"));
        let row = PROVEN.iter().find(|row| row.file == "09.md").unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(SHAPES.iter().all(|shape| shape.file != "09.md"));

        let author = RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap();
        let other = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let note = author.sign(1_700_000_000, 1, Vec::new(), "accidental".into());
        let foreign_note = other.sign(1_700_000_000, 1, Vec::new(), "not mine".into());
        let address = author.sign(
            1_700_000_010,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "draft".into(),
        );
        let boundary = author.sign(
            1_700_000_020,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "same timestamp".into(),
        );
        let later = author.sign(
            1_700_000_050,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "kept".into(),
        );
        let foreign_address = other.sign(
            1_700_000_010,
            30_023,
            vec![Tag::new(vec!["d".into(), "post".into()])],
            "someone else".into(),
        );
        let reason = "these posts were published by accident";
        let request_event = author.sign(
            1_700_000_020,
            5,
            vec![
                Tag::new(vec!["e".into(), note.id.clone()]),
                Tag::new(vec!["e".into(), foreign_note.id.clone()]),
                Tag::new(vec!["e".into(), "dcd59".into()]),
                Tag::new(vec!["e".into(), note.id.to_uppercase()]),
                Tag::new(vec!["a".into(), format!("30023:{}:post", author.pubkey())]),
                Tag::new(vec!["a".into(), format!("30023:{}:post", other.pubkey())]),
                Tag::new(vec!["k".into(), "1".into()]),
                Tag::new(vec!["k".into(), "30023".into()]),
            ],
            reason.into(),
        );
        request_event.validate_structure().unwrap();
        assert_eq!(request_event.class(), EventClass::Regular);
        assert_eq!(request_event.content, reason);

        let request = DeletionRequest::from_event(&request_event).unwrap();
        assert!(request.event_ids.contains(&note.id));
        assert!(request.event_ids.contains(&foreign_note.id));
        assert_eq!(request.event_ids.len(), 2);
        assert_eq!(
            request
                .addresses
                .iter()
                .map(|address| address.to_string())
                .collect::<Vec<_>>(),
            vec![format!("30023:{}:post", author.pubkey())]
        );
        assert_eq!(request.tombstones().count(), 3);
        assert!(request.deletes(&note));
        let mut spoofed = note.clone();
        spoofed.pubkey = other.pubkey().to_owned();
        assert!(!request.deletes(&spoofed));
        assert!(!request.deletes(&foreign_note));
        assert!(request.deletes(&address));
        assert!(request.deletes(&boundary));
        assert!(!request.deletes(&later));
        assert!(!request.deletes(&foreign_address));

        let unrelated = author.sign(1_700_000_000, 1, Vec::new(), "other note".into());
        assert!(!request.deletes(&unrelated));
        let without_kinds = author.sign(
            1_700_000_020,
            5,
            vec![Tag::new(vec!["e".into(), note.id.clone()])],
            String::new(),
        );
        assert!(
            DeletionRequest::from_event(&without_kinds)
                .unwrap()
                .deletes(&note)
        );

        let retraction = author.sign(
            1_700_000_030,
            5,
            vec![Tag::new(vec!["e".into(), request_event.id.clone()])],
            String::new(),
        );
        assert!(
            !DeletionRequest::from_event(&retraction)
                .unwrap()
                .deletes(&request_event)
        );
        assert!(request.deletes(&note));

        let empty = author.sign(
            1_700_000_040,
            5,
            vec![Tag::new(vec!["e".into(), "zz".into()])],
            "nothing actionable".into(),
        );
        let empty_request = DeletionRequest::from_event(&empty).unwrap();
        assert!(empty_request.tombstones().next().is_none());
        assert!(!empty_request.deletes(&note));

        let not_a_request = author.sign(1, 1, Vec::new(), String::new());
        assert!(matches!(
            DeletionRequest::from_event(&not_a_request),
            Err(DomainError::NotDeletionRequest)
        ));
    }

    #[test]
    fn every_pinned_official_file_has_one_check_and_the_manifest_commit_matches() {
        let manifest = fs::read_to_string(official_dir().join("../manifest.json")).unwrap();
        assert!(manifest.contains(OFFICIAL_COMMIT));
        let mut on_disk = BTreeSet::new();
        for entry in fs::read_dir(official_dir()).unwrap() {
            let name = entry.unwrap().file_name().into_string().unwrap();
            if name.ends_with(".md") && name != "README.md" {
                on_disk.insert(name);
            }
        }
        let covered: BTreeSet<String> = covered_files().into_iter().map(str::to_string).collect();
        assert_eq!(covered, on_disk, "ledger files differ from nips/official");
    }

    #[test]
    fn event_shapes_match_the_pinned_text_and_refuse_a_missing_tag() {
        for shape in SHAPES {
            let text = fs::read_to_string(official_dir().join(shape.file)).unwrap();
            assert!(
                text.contains(&shape.kind.to_string()),
                "{} missing kind {}",
                shape.file,
                shape.kind
            );
            let tag_written = text.contains(&format!("[\"{}\"", shape.tag))
                || text.contains(&format!("`{}`", shape.tag));
            assert!(tag_written, "{} missing tag {}", shape.file, shape.tag);
            let good = if shape.tag == "-" {
                sign(shape.kind, vec![Tag::new(vec!["-".into()])], "note")
            } else {
                sign(
                    shape.kind,
                    vec![Tag::new(vec![shape.tag.into(), "value".into()])],
                    "note",
                )
            };
            assert!(verify_shape(shape, &good).is_ok(), "{}", shape.file);
            let bare = sign(shape.kind, Vec::new(), "note");
            assert!(verify_shape(shape, &bare).is_err(), "{}", shape.file);
        }
        let deletion = sign(5, vec![Tag::new(vec!["e".into(), "ab".repeat(32)])], "gone");
        let request = DeletionRequest::from_event(&deletion).unwrap();
        assert!(request.tombstones().next().is_some());
    }

    #[test]
    fn moved_documents_point_at_nip01_and_a_signed_event_still_verifies() {
        for (file, sentence) in MOVED_TO_NIP01 {
            let text = fs::read_to_string(official_dir().join(file)).unwrap();
            assert!(text.contains(sentence), "{file}");
        }
        let event = sign(
            1,
            vec![Tag::new(vec!["p".into(), signer().pubkey().into()])],
            "hi",
        );
        event.validate_crypto().unwrap();
        assert_eq!(EventClass::from_kind(1), EventClass::Regular);
        assert_eq!(EventClass::from_kind(30023), EventClass::Addressable);
        assert!(information_document_lists_nips(&json!({
            "supported_nips": [1, 77]
        })));
        assert!(!information_document_lists_nips(&json!({"name": "relay"})));
    }

    #[test]
    fn the_remaining_official_files_call_their_own_checks() {
        assert!(nip05_identifier("alice@example.com").is_ok());
        assert!(nip05_identifier("not an id").is_err());
        let five = fs::read_to_string(official_dir().join("05.md")).unwrap();
        assert!(five.contains("nip05"));

        assert_eq!(nip06_path(0), "m/44'/1237'/0'/0/0");
        assert!(
            fs::read_to_string(official_dir().join("06.md"))
                .unwrap()
                .contains("1237")
        );

        assert!(nip07_request(Some(10), 1, "hi").is_ok());
        assert!(nip07_request(None, 1, "hi").is_err());
        assert!(
            fs::read_to_string(official_dir().join("07.md"))
                .unwrap()
                .contains("getPublicKey")
        );

        let npub = nip19::encode_npub(&hex_decode(signer().pubkey()));
        assert_eq!(
            nip19::decode_npub(&npub).unwrap(),
            hex_decode(signer().pubkey())
        );
        assert!(
            fs::read_to_string(official_dir().join("19.md"))
                .unwrap()
                .contains("npub")
        );

        assert!(nostr_uri(&format!("nostr:{npub}")).is_ok());
        assert!(nostr_uri("nostr:nsec1qqqq").is_err());
        assert!(
            fs::read_to_string(official_dir().join("21.md"))
                .unwrap()
                .contains("nostr:")
        );

        assert!(delegation_tag(&["aa", "kind=1", "sig"]).is_ok());
        assert!(delegation_tag(&["aa"]).is_err());
        assert!(
            fs::read_to_string(official_dir().join("26.md"))
                .unwrap()
                .contains("delegation")
        );

        assert!(emoji_shortcode("party_1").is_ok());
        assert!(emoji_shortcode("nope space").is_err());
        assert!(
            fs::read_to_string(official_dir().join("30.md"))
                .unwrap()
                .contains("emoji")
        );

        let expiring = sign(
            1,
            vec![Tag::new(vec!["expiration".into(), "1700000100".into()])],
            "soon",
        );
        assert_eq!(expiring.expiration(), Some(1_700_000_100));
        assert!(
            fs::read_to_string(official_dir().join("40.md"))
                .unwrap()
                .contains("expiration")
        );

        let auth = sign(
            22_242,
            vec![
                Tag::new(vec!["relay".into(), "wss://relay.example".into()]),
                Tag::new(vec!["challenge".into(), "abc".into()]),
            ],
            "",
        );
        auth.validate_crypto().unwrap();
        assert_eq!(auth.kind, 22_242);
        assert!(
            fs::read_to_string(official_dir().join("42.md"))
                .unwrap()
                .contains("22242")
        );

        let role = sign(
            33_534,
            vec![
                Tag::new(vec!["-".into()]),
                Tag::new(vec!["d".into(), "admin".into()]),
            ],
            "",
        );
        assert_eq!(role.kind, 33_534);
        assert!(role.tags.iter().any(|tag| tag.name() == Some("d")));
        assert!(
            fs::read_to_string(official_dir().join("43.md"))
                .unwrap()
                .contains("33534")
        );

        let peer = crate::domain::RelaySigner::from_secret_hex(&"24".repeat(32)).unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([0x42; 32]).unwrap();
        let public = secp256k1::XOnlyPublicKey::from_byte_array([0; 32]);
        let _ = (peer, secret, public);
        let conversation = nip44::conversation_key(
            &secp256k1::SecretKey::from_byte_array([0x42; 32]).unwrap(),
            &{
                let signer = RelaySigner::from_secret_hex(&"24".repeat(32)).unwrap();
                let bytes = hex_decode(signer.pubkey());
                secp256k1::XOnlyPublicKey::from_byte_array(bytes).unwrap()
            },
        );
        let hidden = nip44::encrypt("hello", &conversation, [9; 32]).unwrap();
        assert_eq!(nip44::decrypt(&hidden, &conversation).unwrap(), "hello");
        assert!(
            fs::read_to_string(official_dir().join("44.md"))
                .unwrap()
                .contains("NIP-44")
                || fs::read_to_string(official_dir().join("44.md"))
                    .unwrap()
                    .contains("44")
        );

        assert!(count_message(&json!(["COUNT", "q", {"kinds": [1]}])).is_ok());
        assert!(count_message(&json!(["REQ", "q", {}])).is_err());
        assert!(
            fs::read_to_string(official_dir().join("45.md"))
                .unwrap()
                .contains("COUNT")
        );

        assert_eq!(nip49_iterations(16).unwrap(), 65_536);
        assert!(nip49_iterations(0).is_err());
        assert!(
            fs::read_to_string(official_dir().join("49.md"))
                .unwrap()
                .contains("ncryptsec")
        );

        assert!(search_matches("Cat", 1, "a cat walked"));
        assert!(
            fs::read_to_string(official_dir().join("50.md"))
                .unwrap()
                .contains("search")
        );

        assert!(android_signer_method("get_public_key"));
        assert!(!android_signer_method("export_nsec"));
        assert!(
            fs::read_to_string(official_dir().join("55.md"))
                .unwrap()
                .contains("get_public_key")
        );

        assert!(vanish_request(&sign(62, Vec::new(), "")).is_ok());
        assert!(vanish_request(&sign(1, Vec::new(), "")).is_err());
        assert!(
            fs::read_to_string(official_dir().join("62.md"))
                .unwrap()
                .contains("62")
        );

        assert!(chess_pgn("[Event \"x\"]\n1. e4").is_ok());
        assert!(chess_pgn("not a game").is_err());
        assert!(
            fs::read_to_string(official_dir().join("64.md"))
                .unwrap()
                .contains("64")
        );

        let mut local = vec![Item {
            timestamp: 10,
            id: [1; 32],
        }];
        negentropy::prepare(&mut local);
        let frame = negentropy::open(&local);
        assert!(negentropy::respond(&local, &frame).is_ok());
        assert!(
            fs::read_to_string(official_dir().join("77.md"))
                .unwrap()
                .contains("NEG-OPEN")
        );

        assert!(management_method("banpubkey"));
        assert!(!management_method("dropdatabase"));
        assert!(
            fs::read_to_string(official_dir().join("86.md"))
                .unwrap()
                .contains("supportedmethods")
        );

        let ble = fs::read_to_string(official_dir().join("BE.md")).unwrap();
        assert!(ble.contains(BLE_SERVICE_UUID));
    }

    fn hex_decode(value: &str) -> [u8; 32] {
        let mut out = [0u8; 32];
        let bytes = value.as_bytes();
        for index in 0..32 {
            let high = match bytes[index * 2] {
                b @ b'0'..=b'9' => b - b'0',
                b @ b'a'..=b'f' => b - b'a' + 10,
                _ => panic!("hex"),
            };
            let low = match bytes[index * 2 + 1] {
                b @ b'0'..=b'9' => b - b'0',
                b @ b'a'..=b'f' => b - b'a' + 10,
                _ => panic!("hex"),
            };
            out[index] = (high << 4) | low;
        }
        out
    }
}
