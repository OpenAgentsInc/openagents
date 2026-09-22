# Official NIP ledger

`crates/nostr/src/lane.rs` is the ledger for the pinned official lane. The
commit it names is the `official` entry in `nips/manifest.json`. A test
fails when a file under `nips/official/` other than `README.md` has no check.

## What a row proves

NIP-02 is `configured-and-proven`. The domain role is a kind `3` replaceable
list whose `p` tags carry a 32-byte hex key, an optional `ws://` or `wss://`
relay, and an optional petname. The client helpers are `parse_follow_list`,
`append_follow`, and `displayed_petname` in `crates/nostr/src/domain/follow.rs`.
The server role is the ordinary replacement head: `EventClass::from_kind(3)`
is `Replaceable`, so a newer list from the same author deletes the previous
one in `crates/nostr-relay/src/store/mod.rs`. There is no separate setting.
The fixture is the pinned `p` tag triple. Live acceptance is
`lane::tests::nip02_follow_lists_replace_and_petnames_chain`. Content is
ignored, which is what the pinned text says. The `nostr` and `nostr-relay`
crates own the row.

NIP-03 is `configured-and-proven`. `open_attestation` in
`crates/nostr/src/domain/ots.rs` reads a kind `1040` event: one `e` tag, one
`k` tag, and a base64 `.ots` body. The proof digest must be that event id,
and the proof must reach exactly one Bitcoin block-height attestation.
Admission uses the same check. NIP-03 stays off the NIP-11 list because the
pinned text marks it unrecommended. The height is not compared to a Bitcoin
block header. Acceptance is
`domain::ots::tests::a_bitcoin_proof_binds_the_event_id_and_one_height`.

NIP-04 is `configured-and-proven`. `nip04::encrypt` and `nip04::decrypt`
use AES-256-CBC. The key is the X coordinate of the ECDH point and is not
hashed. The content form is `base64(ciphertext)?iv=base64(iv)`. Admission
checks the single `p` tag and that form, and does not decrypt. NIP-04 stays
off the NIP-11 list because the pinned text marks it unrecommended. There is
no MAC. Acceptance is
`nip04::tests::a_direct_message_round_trips_and_keeps_a_mention_as_text`.

NIP-15 is `configured-and-proven`. Stalls (`30017`), products (`30018`),
marketplace pages (`30019`), and auctions (`30020`) are addressable. A
stall, product, or auction `d` tag must equal its content `id`. A kind
`1021` bid is a non-negative integer and names one auction event id. A
kind `1022` confirmation is valid for that version only when the auction
author signs it. `shipping_cost` adds the stall zone cost to the product
extra times the number of units. Checkout types 0, 1, and 2 are plaintext
JSON. NIP-15 stays off the NIP-11 list because the pinned text marks it
unrecommended. The relay does not read checkout JSON inside kind `4`.
It does not settle a payment. A later auction edit is still stored.
Acceptance is
`domain::market::tests::a_stall_product_and_bid_follow_the_pinned_marketplace_events`.

NIP-17 is `configured-and-proven`. A kind `14` rumor is unsigned plain
text. Its `p` tags name the receivers, and the chat room is that set plus
the author. Kind `15` is a file rumor: the algorithm is `aes-gcm`, and `x`
and `ox` are SHA-256 hashes. The author seals the rumor as kind `13` with
NIP-44 and gift-wraps that seal as kind `1059` once per participant,
including a copy for the author. The reader decrypts both layers and
refuses the rumor when its pubkey differs from the seal. Kind `10050` is
the replaceable inbox list. The relay does not decrypt a wrap. It requires
one `p` tag, and with `NOSTR_RELAY_URL` set it serves kind `1059` only to
that authenticated reader and lists 17 in NIP-11. A wrap timestamp may sit
at most two days before the rumor. File bytes are not downloaded. The
caller supplies the one-time wrapper key. Kind `21059` belongs to NIP-59.
Acceptance is
`nip17::tests::a_private_message_round_trips_and_rejects_an_impersonated_rumor`.

NIP-22 is `configured-and-proven`. A kind `1111` comment names one
uppercase root scope (`A`, `E`, or `I`) and one lowercase parent scope
(`a`, `e`, or `i`). `K` and `k` are required. A nostr scope requires its
author tag. An addressable parent also carries the parent event id in `e`.
External scopes use the NIP-73 identifier types, and the kind tag has to
match that type. Kind `1` is refused. A top-level comment uses the same
scope for the root and the parent. Admission rejects a malformed comment.
Kind `1111` stays a regular stored event and is not added to the NIP-11
list. Content is kept as text. A URL with a fragment is refused and is not
rewritten. Acceptance is
`domain::comment::tests::a_comment_scopes_to_the_root_and_refuses_a_kind_1_reply`.

NIP-23 is `configured-and-proven`. Kind `30023` is an addressable
article with one `d` tag. Optional metadata is `title`, `image`,
`summary`, `published_at`, and `t` topics. Content is Markdown: a
paragraph is a single line, HTML tags are refused, and each `nostr:`
reference must be a NIP-19 identifier other than `nsec`. `e` and `a`
tags, when present, are event ids and addresses. Kind `30024` is refused
because the pinned text moves drafts to NIP-37. A reply is a kind `1111`
comment whose `K` or `k` tag is `30023`, not a kind `1` note. The newest
event for the `d` tag replaces the older one. Kind `30023` is not added
to the NIP-11 list. Markup inside a code fence is not treated as HTML.
Acceptance is
`domain::article::tests::an_article_replaces_on_its_identifier_and_keeps_markdown_paragraphs`.

NIP-32 is `configured-and-proven`. Kind `1985` attaches one or more
`l` labels to `e`, `p`, `a`, `r`, or `t` targets. When an `L` namespace
is present, each `l` mark must name one of those namespaces. With no `L`
tag and no mark, the namespace is `ugc`. A namespace that starts with `#`
records the tag that value should be associated with. On any other kind,
`l` and `L` describe that event. Admission rejects a kind `1985` event
with no target or a mark that does not match. Kind `1985` is a regular
event, so a newer label does not replace an older one. It is not added to
the NIP-11 list. The relay does not rewrite the labeled target. A
correction is a new event plus a NIP-09 deletion. Acceptance is
`domain::label::tests::a_label_attaches_a_namespace_to_its_targets`.

NIP-35 is `configured-and-proven`. Kind `2003` is a torrent index:
one v1 info hash in `x` (40 hex characters or 32 base32 characters), at
least one `file` path and size, and optional `tracker`, `title`, `t`, and
`i` catalog tags. `magnet_uri` builds `magnet:?xt=urn:btih:` and adds each
tracker as `tr`. Catalog ids are `tcat`, `newznab`, `imdb`, `tmdb`,
`ttvdb`, `mal`, and `anilist`. Kind `2004` is a comment. It uses NIP-10
`root` and `reply` markers, and still accepts the deprecated positional
`e` tags. Admission rejects a torrent without a hash and a file, and a
comment that names no event. Both kinds are regular stored events and are
not added to the NIP-11 list. The info hash is not checked against torrent
bytes, and trackers are not contacted. Acceptance is
`domain::torrent::tests::a_torrent_builds_a_magnet_and_a_comment_names_it`.

NIP-37 is `configured-and-proven`. Kind `31234` stores an unsigned
draft encrypted with NIP-44 to the author's own key. One `d` tag
identifies it and one `k` tag names the draft kind. Empty content
deletes that addressable draft. A newer wrap with the same `d` tag
replaces the older one. Kind `1234` is a checkpoint whose `a` tag is
`31234:<pubkey>:<identifier>`. Kind `10013` is the replaceable list of
`wss://` or `ws://` relays, encrypted in the content, with no public
`relay` tag. Admission checks those tags and NIP-44 framing and does not
decrypt. None of these kinds are added to the NIP-11 list. The recommended
expiration is 90 days. This module does not require authentication on a
private-storage relay. Acceptance is
`domain::draft::tests::a_draft_wrap_round_trips_and_a_blank_content_deletes_it`.

NIP-39 is `configured-and-proven`. Kind `10011` is a replaceable
list of `i` tags. Each tag has `platform:identity` and a proof. The
identity is stored in lowercase. Platform names use `a-z`, digits, and
`._-/`. GitHub, Twitter, Mastodon, and Telegram check the identity and
proof shapes. `proof_url` and `expected_statement` name the public page
and the sentence that page should contain, including the author's `npub`.
Extra values after the proof are kept. Admission rejects a list with no
`i` tag, a missing proof, or a known platform whose identity does not
match. A newer list from the same author replaces the older one. Kind
`10011` is not added to the NIP-11 list. The relay does not fetch the
proof. Acceptance is
`domain::profile_link::tests::a_profile_link_list_names_each_platform_and_replaces`.

NIP-46 is `configured-and-proven`. Kind `24133` is an ephemeral request
or response. The content is NIP-44 ciphertext, and the event has one `p`
tag naming the other party. `parse_bunker` and `parse_nostrconnect` read
the connection tokens, including percent-encoded relays and a permission
list. `RemoteSigner::handle` decrypts a request and answers `connect`,
`ping`, `get_public_key`, `sign_event`, the NIP-04 and NIP-44 helpers,
`switch_relays`, and `logout`, then seals the response. A bunker secret
works for one connection. A permission list limits `sign_event` and the
cipher methods. Admission checks the `p` tag and the NIP-44 framing and
does not decrypt. The kind is ephemeral, so the relay fans the event out
and does not store it. Kind `24133` is not added to the NIP-11 list. The
relay does not fetch a NIP-89 announcement or a `nostr.json` document.
Acceptance is
`domain::remote_sign::tests::a_client_connects_and_the_signer_returns_a_signed_event`.

NIP-56 is `configured-and-proven`. Kind `1984` names a user in a `p`
tag, a note in an `e` tag, or a blob hash in an `x` tag. The third value
of the tag being reported is `nudity`, `malware`, `profanity`, `illegal`,
`spam`, `impersonation`, or `other`. A blob report also names the event
that carries the blob. The pinned blob example has no `p` tag, so that
form may omit the user. A `server` tag, when present, is an `http://` or
`https://` URL. `l` and `L` tags follow NIP-32 and describe the report
itself. Admission rejects a report with no target, no type, an unknown
type, or a blob hash without an event. Kind `1984` is a regular stored
event. The relay does not delete or hide the referenced event, and the
kind is not added to the NIP-11 list. The relay does not fetch the blob
or the server URL. Acceptance is
`domain::report::tests::a_report_names_the_user_or_the_blob_and_does_not_delete_it`.

NIP-58 is `configured-and-proven`. Kind `30009` defines a badge with
one `d` identifier and optional `name`, `description`, `image`, and
`thumb` tags. An image URL is `http://` or `https://`. Dimensions, when
present, are `widthxheight`. Kind `8` awards that definition to one or
more pubkeys. The award is a regular event, so it is not replaced or
transferred. Kind `10008` is the replaceable profile list: consecutive
`a` and `e` pairs, plus `a` tags that name a kind `30008` set. An `a`
tag without the following `e` tag is ignored, and so is an `e` tag
without a preceding `a` tag. Kind `30008` with any other `d` value is a
labeled set of those pairs. Kind `30008` with `d` equal to
`profile_badges` is the deprecated profile list. None of these kinds are
added to the NIP-11 list. The relay does not fetch images and does not
check that the award event repeats the definition address. Recommended
1024x1024 dimensions are not required. Acceptance is
`domain::badge::tests::a_badge_definition_is_awarded_and_the_profile_lists_it`.

NIP-59 is `configured-and-proven`. A rumor is an unsigned event of
any kind. `seal_rumor` encrypts it to the recipient as kind `13` with
NIP-44. The seal has no tags, or one `expiration` tag, and no `p` tag.
`wrap_seal` encrypts that seal under a one-time key as kind `1059` or
kind `21059`, with one `p` tag. Timestamps on the seal and the wrap may
move at most two days earlier than the rumor. Kind `1059` is a regular
stored event and is served only to the authenticated reader named by the
`p` tag. Kind `21059` is ephemeral, so the relay does not store it.
Admission checks that framing and does not decrypt. A kind `5` from the
reader removes a wrap addressed to that reader in `DeletionRequest::deletes`.
The Postgres tombstone table still records only same-author `e` and `a`
tags, so a live deletion does not yet remove stored wraps by `p` tag.
NIP-11 lists `59` when `NOSTR_RELAY_URL` is set. Proof of work is not
required. Acceptance is
`domain::gift_wrap::tests::a_rumor_is_sealed_and_the_ephemeral_wrap_is_not_stored`.

NIP-09 is `configured-and-proven`. `DeletionRequest::from_event` reads a
kind `5` event. An `e` tag must be a 32-byte lowercase hex id. An `a` tag
must name a replacement address whose pubkey is the request author. The
client hides an event only when `deletes` matches that author, and an
address version only when its `created_at` is less than or equal to the
request. The server stores the request, writes the same tombstones in
`apply_deletion`, and rejects a later event that matches one. NIP-11 lists
`9` with no extra setting. `k` tags are optional and do not choose targets.
A request that names another kind `5` event deletes nothing. Acceptance is
`lane::tests::nip09_deletion_requests_hide_the_authors_events_through_the_request_time`.
The live store path is `deletion_before_event` in
`crates/nostr-relay/tests/store_postgres.rs`.

NIP-29 is `configured-and-proven`. The domain role is `GroupMetadata` and
`GroupAction` in `crates/nostr/src/domain/expanded.rs`. The server role is
admission, query filtering, and metadata regeneration in
`crates/nostr-relay/src/store/mod.rs`. Configuration is
`NOSTR_RELAY_RELAY_SECRET_KEY`; NIP-11 then advertises
`nip29.subgroups: true`. The fixture is the pinned metadata event: `private`,
`hidden`, `restricted`, one `parent`, and the child list. Live acceptance is
`domain::expanded::tests::private_hidden_and_subgroup_fields_follow_the_pinned_metadata_event`
and the relay query filter that hides private timeline events and hidden
metadata from non-members. The limitation is that kinds 9003, 9004, 9006, and
9011–9020 have no row in the pinned moderation table, and kind 39004 stays
empty because this process does not run LiveKit. The `nostr` and `nostr-relay`
crates own the row. Its status is distinct from a kind-and-tag check.

Event-shaped files other than the rows above are still a `Shape`: the kind
and one tag that occur in the pinned text. That check is partial. It signs
an event with that kind and tag, accepts it, and refuses the same event
with the tag removed. It does not re-state every optional field, and it is
not configured-and-proven.

Files whose body says the rules moved to NIP-01 are not a second protocol.
The check requires that sentence and still verifies a signed NIP-01 event.

The other files call the function that implements them: identifiers,
bech32, encryption, search, expiration, delegation, sync, management method
names, and the browser and Android signer method lists.

## Advertisement

NIP-11 lists a numeric NIP only when the relay runs that path. 77 is listed
because `NEG-OPEN` is handled. OpenAgents profile names stay off that list
until `NOSTR_RELAY_OPENAGENTS_PROFILES` is set.

## Owner

The `nostr` and `nostr-relay` crates own these checks. Live Postgres
acceptance of the gateway remains `crates/nostr-relay/tests/gateway_postgres.rs`
and runs only against a disposable database.
