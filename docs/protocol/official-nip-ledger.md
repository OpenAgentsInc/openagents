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

NIP-65 is `configured-and-proven`. Kind `10002` is a replaceable
list of `r` tags. Each value is a `ws://` or `wss://` URL. A missing
marker means the relay is both read and write. `read` and `write` narrow
that. `write_relays` and `read_relays` split the list. `publish_relays`
names the author's write relays and then each mentioned user's read
relays, without repeats. Admission rejects an empty list, a non-relay
URL, and any marker other than `read` or `write`. A newer list from the
same author replaces the older one. NIP-11 lists `65` with no extra
setting. The relay does not publish to those URLs. The suggestion to
keep two to four relays is not enforced, and indexer discovery is not
implemented. Acceptance is
`domain::relay_list::tests::a_relay_list_splits_read_and_write_and_replaces`.

NIP-84 is `configured-and-proven`. Kind `9802` is a highlight.
The content is the highlighted text, and it may be empty for non-text
media. The source is an `a` tag, an `e` tag, or an `r` tag marked
`source`. An `r` tag marked `mention` is a URL inside the commentary,
not the source. `p` tags credit pubkeys as `author` or `editor`. A
mention uses `mention` so it is not read as an author. A `context` tag
keeps the surrounding paragraph. A `comment` tag makes the event a
quote highlight and keeps that commentary. `clean_source_url` drops
tracker query parameters before a client publishes the source URL. The
relay stores the URL as tagged and does not fetch it. Kind `9802` is a
regular event, so a newer highlight does not replace an older one, and
it is not added to the NIP-11 list. Admission rejects a highlight with
no source, an unknown role, or an `r` tag that is not `source` or
`mention`. Acceptance is
`domain::highlight::tests::a_highlight_names_its_source_and_a_comment_quotes_it`.

NIP-89 is `configured-and-proven`. Kind `31989` recommends
applications for one event kind. Its `d` tag is that kind, and each `a`
tag points at a kind `31990` handler, with an optional relay hint and
platform. Kind `31990` names the kinds it supports in `k` tags. Empty
content means the client's kind `0` profile carries the display name.
Non-empty content is a JSON object. `latest` and `next` point at a site
manifest. A platform tag such as `web` or `ios` carries a URL with the
literal `bech32`. `handler_url` replaces that placeholder with a NIP-19
token the caller supplies. A tag without a NIP-19 type is the generic
handler for that platform. A `client` tag on another event names the
handler that published it. Leaving the tag off is the opt-out. Both
kinds are addressable, so a newer event with the same `d` tag replaces
the older one. A filter on `d` or `k` selects them. Neither kind is
added to the NIP-11 list. The relay does not fetch the application or
its kind `0` profile. This crate encodes `npub` and `nsec` only.
Acceptance is
`domain::handler::tests::a_recommendation_points_at_a_handler_and_the_url_receives_the_entity`.

NIP-98 is `configured-and-proven`. Kind `27235` is an ephemeral
HTTP authorization. The event has one absolute `u` URL and one `method`.
`parse_http_authorization_claim` checks the kind, the signature, a 60
second window, the URL, and the method before the server reads a body.
When the caller has a body, `parse_http_authorization` also checks the
lowercase SHA-256 in the `payload` tag. The `Authorization` scheme is
`Nostr` followed by the base64 event. Kind `27235` is not stored. NIP-11
lists `98` when a management pubkey or media storage is configured.
Content may be non-empty. The pinned example's event id does not match
the NIP-01 preimage, so that header is refused. Acceptance is
`domain::expanded::tests::a_nostr_authorization_matches_the_url_method_and_body`.

NIP-99 is `configured-and-proven`. Kind `30402` is an addressable
classified listing. Kind `30403` uses the same tags and saves a draft,
so it does not replace the published listing. The content is Markdown
and is kept as text, including `nostr:` references. One `d` tag
identifies the listing. `title`, `summary`, `published_at`, `location`,
and `price` are optional structured tags. A price is an amount, a
three-letter currency, and an optional lowercase frequency such as
`month`. `status` is `active` or `sold`. `image` tags carry an `http://`
or `https://` URL and optional `widthxheight` dimensions. `t` tags are
topics and `g` is a geohash. `e` and `a` tags name related events.
Admission rejects a missing identifier, a malformed price, or any other
status. A newer listing with the same `d` tag replaces the older one.
Neither kind is added to the NIP-11 list. The relay does not fetch
images. Acceptance is
`domain::listing::tests::a_listing_keeps_its_price_and_a_draft_does_not_replace_it`.

NIP-B0 is `configured-and-proven`. Kind `39701` is an addressable
web bookmark. The `d` tag is the URI. `bookmark_identifier` omits the
characters before the hostname when the scheme is `https`, so
`https://alice.blog/post` is stored as `alice.blog/post`. An `http` URI
keeps its scheme. Userinfo on an `https` URI is omitted with the scheme.
The content is a description and may be empty. `title`, `published_at`,
and `t` are optional. Admission rejects a `d` tag that still begins with
`https://`. A newer bookmark with the same `d` tag replaces the older
one. Clients query that tag directly. A reply is a kind `1111` comment
with `K` and `k` equal to `39701`. A kind `1` note is not a reply. Kind
`39701` is not added to the NIP-11 list. The relay does not fetch the
page. Acceptance is
`domain::bookmark::tests::a_bookmark_drops_the_https_scheme_and_a_comment_replies`.

NIP-C0 is `configured-and-proven`. Kind `1337` stores a code
snippet in `content`. `l` is the programming language and must be
lowercase, such as `javascript` or `rust`. That same `l` tag is also a
NIP-32 self-label in the `ugc` namespace. `name` is a filename,
`extension` has no leading dot, and `description` and `runtime` are
optional. `license` is an SPDX identifier, may repeat, and may include
an `http://` or `https://` URL of the license text. `dep` names a
dependency. `repo` is an HTTP URL or a kind `30617` repository address
with an optional `wss://` relay hint. Kind `1337` is a regular event, so
a newer snippet does not replace an older one. It is not added to the
NIP-11 list. The relay does not run the snippet or fetch the repository.
SPDX identifiers are not checked against the SPDX list. Acceptance is
`domain::snippet::tests::a_snippet_keeps_its_source_and_does_not_replace`.

NIP-CC is `configured-and-proven`. Kind `37516` is an addressable
geocache. It requires one `d` identifier, one `name`, one or more `g`
geohashes, difficulty `D`, terrain `T`, and size `S`. Kind `7516` is a
found log that names the cache. Kind `7517` is a verification whose
content is `Geocache verification for` plus the finder `npub`, and
`confirm_find` checks that the cache verification key signed it and
that it names the log author. `exclusive_finder` attributes a
`first-to-find` cache to the `F` tag when one is present, and otherwise
to the earliest verified log. Kind `37517` is an addressable curation
list. A non-found log is a kind `1111` comment. A newer cache with the
same `d` tag replaces the older one. These kinds are not added to the
NIP-11 list. The 8-character geohash guidance is not enforced, because
the pinned example includes shorter prefixes. The verification `naddr`
is not decoded. The pinned DNF example omits the NIP-22 `e` version
tag, so a stored comment includes it. Images are not fetched. Unknown
`n` modifiers are kept. Found-log image tags are not checked. Acceptance
is `domain::geocache::tests::a_geocache_is_found_verified_and_collected`.

NIP-10 is `configured-and-proven`. Kind `1` is a regular plaintext
note. A marked `e` tag of `root` names the thread root, and `reply`
names the direct parent. A direct reply to the root carries only the
`root` marker, and `is_direct_reply` reports that case. Unmarked `e`
tags keep the deprecated positional form: one tag is a direct reply,
and two or more put the root first and the parent last. Tags between
them are mentions. A `q` tag cites an event id or a replacement
address. `reply_participants` places the replied-to author first and
then the other pubkeys, dropping duplicates. A newer note does not
replace an older one. Kind `1` threading is not added to the NIP-11
list. The relay does not fetch the referenced event, so it does not
prove that an `e` tag points at kind `1`. Markup in the content is
kept. `e` tags are not required to appear in root-to-parent order. A
reply's `p` tags are not checked against the parent note. Acceptance
is `domain::note::tests::a_note_threads_from_the_root_to_its_parent`.

NIP-18 is `configured-and-proven`. Kind `6` reposts a kind `1`
note. Kind `16` reposts any other kind. The `e` tag is the event id
and its third value is a `ws://` or `wss://` relay. The content is
the JSON of that event, or empty. A non-empty content must be a
signed event whose id matches the `e` tag. A protected event, one
with a `["-"]` tag, is not embedded. Kind `16` refuses a kind `1`
body. When a `k` or `a` tag is present it must match the embedded
event. A `q` tag cites another event. A newer repost does not replace
an older one. Kinds `6` and `16` are not added to the NIP-11 list.
An empty repost does not prove the target kind, because the relay
does not fetch it. `nostr:` mentions are not rewritten into `q` tags.
A `p` tag and a `k` tag are not required when the content is empty.
Acceptance is
`domain::repost::tests::a_repost_embeds_the_note_and_a_generic_repost_names_its_kind`.

NIP-52 is `configured-and-proven`. Kind `31922` is a date-based
calendar event. `start` is an inclusive `YYYY-MM-DD` date and `end`,
when present, is exclusive and later. Kind `31923` is a time-based
event. `start` and `end` are unix seconds, `start_tzid` names the
start zone, and an omitted `end_tzid` uses that same zone. `D` is
`day_stamp`, `floor(unix seconds / 86400)`, and must include the
start day. Kind `31924` is a calendar list of kind `31922` and
`31923` addresses. Kind `31925` is an RSVP whose `status` is
`accepted`, `declined`, or `tentative`. A declined RSVP ignores
`fb`. A `name` tag supplies the title only when `title` is absent.
An `a` tag on a calendar event requests inclusion in a kind `31924`
calendar. A newer event with the same `d` tag replaces the older
one. These kinds are not added to the NIP-11 list. Time zone names
are not checked against the IANA database. `D` tags are not required
to list every day through the exclusive end. Images and links are
not fetched. Recurring events are not expanded. The relay does not
decide who may attend. Acceptance is
`domain::calendar::tests::a_calendar_event_keeps_its_span_and_an_rsvp_names_it`.

NIP-57 is `configured-and-proven`. Kind `9734` is a zap request.
It names one recipient, the relays that should receive the receipt,
and an optional millisatoshi `amount`, `lnurl`, event id, address,
and target kind. Kind `9735` is a zap receipt. Its `description` is
that signed request. Its `bolt11` invoice amount matches the request
amount when both name one, and `SHA256(description)` matches the
invoice description hash when the invoice carries one. A `preimage`
must hash to the invoice payment hash. `zap_callback_query` builds
the LNURL callback query and does not send it. `zap_split` divides
an amount across `zap` tags: missing weights split it equally, and a
missing weight beside a present weight is zero. A newer zap does not
replace an older one. Kinds `9734` and `9735` are not added to the
NIP-11 list. The relay does not call LNURL or pay invoices. The
bolt11 signature and expiry are not checked. The receipt pubkey is
not compared with a provider `nostrPubkey`, because that profile is
not fetched. A published kind `9734` is stored. Acceptance is
`domain::zap::tests::a_zap_receipt_matches_the_request_amount_and_description`.

NIP-94 is `configured-and-proven`. Kind `1063` describes a shared
file. `url` is an `http://` or `https://` download address. `m` is
one lowercase MIME type. `x` is the SHA-256 of the file. `ox` is the
hash before a server transformation and may be omitted. `size` is a
byte count. `dim` is `<width>x<height>`. `magnet` starts with
`magnet:?`. `i` is a 40- or 64-character lowercase infohash.
`blurhash`, `thumb`, `image`, `summary`, `alt`, `fallback`, and
`service` are optional. Kind `1063` is a regular event, so a newer
description does not replace an older one. It is not added to the
NIP-11 list. The relay does not download the file or recompute the
hash. A blurhash is not decoded into pixels. A magnet URI is not
matched against the infohash. Acceptance is
`domain::file::tests::a_file_keeps_its_hash_and_does_not_replace`.

NIP-78 is `configured-and-proven`. Kind `30078` is an addressable
application record. Its `d` tag names the app and the context, or any
other string of 1 to 1024 characters. Kind `78` is a regular event for
many rows of the same type. A `d` tag on kind `78` groups those rows
and does not replace them. Content and the other tags stay opaque.
A newer kind `30078` record with the same `d` tag replaces the older
one. Kind `30078` stays out of search. Kinds `78` and `30078` are not
added to the NIP-11 list. The relay does not decrypt the content or
decide which app owns an identifier. Kind `78` content remains
searchable. Acceptance is
`domain::app_data::tests::an_application_record_replaces_on_its_identifier_and_a_plain_event_does_not`.

NIP-88 is `configured-and-proven`. Kind `1068` is a poll.
`content` is the label. Each `option` tag is an alphanumeric id and a
label. `relay` tags are `ws://` or `wss://` URLs. `polltype` is
`singlechoice` or `multiplechoice`; a missing type is single choice.
`endsAt` is the unix second when voting stops. Kind `1018` is a
response: one `e` tag names the poll, and each `response` tag names an
option. `tally` keeps one vote per pubkey, the latest `created_at`
that is not after `endsAt`. An equal timestamp keeps the greater event
id. A single-choice vote counts its first response tag. A
multiple-choice vote counts the first tag for each option id. A newer
poll does not replace an older one. Kinds `1068` and `1018` are not
added to the NIP-11 list. The relay does not fetch the poll's relays.
Kind `5` deletions of votes are still honored. Follow sets, proof of
work, and web of trust are not applied. Acceptance is
`domain::poll::tests::a_poll_counts_one_vote_per_pubkey_inside_its_window`.

NIP-90 is `configured-and-proven`. Kinds `5000` through `5999`
are job requests. The result kind is that number plus `1000`. Kind
`7000` is feedback. An `i` tag is `url`, `event`, `job`, or `text`.
`output` is a lowercase MIME type. `bid` and `amount` are
millisatoshis. A result embeds the signed request in `request`, and
its `e` tag and kind must name that request. Feedback `status` is
`payment-required`, `processing`, `error`, `success`, or `partial`.
A newer job event does not replace an older one. NIP-90 is
unrecommended, so these kinds are not added to the NIP-11 list. The
relay does not run the job, fetch inputs, or pay invoices. An
`encrypted` payload is checked as NIP-04 framing and is not decrypted.
Kind `5` deletion still follows NIP-09. Acceptance is
`domain::vending::tests::a_job_result_uses_the_request_kind_plus_one_thousand`.

NIP-96 is `configured-and-proven`. Kind `10096` is a replaceable
list of `https://` file servers. `parse_storage_document` reads
`/.well-known/nostr/nip96.json`. A normal document requires `api_url`.
A delegated document has an empty `api_url`, a `delegated_to_url`, and
no other fields. The free plan defaults `is_nip98_required` to true.
`parse_upload_response` requires `url` and `ox` on a successful upload.
`ox` is the SHA-256 of the original file. `parse_processing_status`
reads a delayed job whose percentage is an integer from 0 to 100. A
newer server list replaces the older one. NIP-96 is unrecommended, so
kind `10096` is not added to the NIP-11 list. The relay does not
upload, download, or delete files. HTTP status codes and a NIP-98
payload hash are not checked. A blurred or resized file is not fetched.
Acceptance is
`domain::storage::tests::a_file_server_list_replaces_and_an_upload_keeps_the_original_hash`.

NIP-A0 is `configured-and-proven`. Kind `1222` is a root voice message
and kind `1244` is a reply. `content` is an `http://` or `https://` URL
of the audio file. A reply uses the NIP-22 root and parent tags, read
by `comment_scopes`. An optional `imeta` tag carries a `waveform` of
whole-number amplitudes and a `duration` in seconds. A newer message
does not replace an older one. NIP-A0 is a draft, so these kinds are
not added to the NIP-11 list. The relay does not download the audio,
so it does not check the codec or the 60-second guidance. A duration
over 60 seconds is kept. The suggestion of fewer than 100 waveform
amplitudes is not enforced. Acceptance is
`domain::voice::tests::a_voice_message_keeps_its_audio_url_and_a_reply_threads_to_it`.

NIP-87 is `configured-and-proven`. Kind `38172` announces a Cashu mint,
kind `38173` announces a Fedimint, and kind `38000` recommends one of
them. A mint names one `d` identifier, one or more `u` endpoints, and
a network of `mainnet`, `testnet`, `signet`, or `regtest`. A Cashu
mint lists `nuts`. A Fedimint lists `modules`. A Cashu `d` tag is a
32-byte pubkey. Empty mint content is kept, and non-empty content must
be a JSON object. A recommendation's `k` and `d` name that mint, and
each `a` tag repeats the same address. A newer announcement or
recommendation with the same identifier replaces the older one. NIP-87
is a draft, so these kinds are not added to the NIP-11 list. The relay
does not contact a mint, decode an invite code, or read a kind `0`
profile when content is empty. Acceptance is
`domain::ecash::tests::a_mint_announcement_replaces_and_a_recommendation_names_it`.

NIP-75 is `configured-and-proven`. Kind `9041` is a fundraising goal.
`amount` is the target in millisats and `relays` lists where zaps are
sent and tallied. `closed_at` is the last second that still counts. A
zap request covers the goal when it includes every goal relay.
`goal_progress` adds those amounts and skips a later zap. Optional
`image`, `summary`, `r`, and `a` tags are kept. `zap` tags name
beneficiaries and the split sums to the target. A `goal` tag on
another event stores the goal id and an optional relay. A newer goal
does not replace an older one. NIP-75 is a draft, so kind `9041` is
not added to the NIP-11 list. The relay does not send or tally
Lightning payments, and it does not fetch the goal when a zap request
arrives. Acceptance is
`domain::goal::tests::a_zap_goal_tallies_until_it_closes_and_a_request_lists_its_relays`.

NIP-60 is `configured-and-proven`. Kind `17375` is a replaceable Cashu
wallet. Public `mint` tags name its mints. The content is NIP-44
ciphertext to the author. After decryption it holds a wallet private
key, which is not the Nostr key, and the same mint URLs. Kind `7375`
is one set of unspent proofs. Spending a proof rolls the unspent
proofs into a new token whose `del` list names the old event, and the
kind `5` deletion includes `k` `7375`. Kind `7376` records direction,
amount, and the created and destroyed token ids. Kind `7374` keeps a
mint quote. Its expiration is after the event and at most 14 days
later. A newer wallet replaces the older one. Token, history, and
quote events do not replace. NIP-60 is a draft, so these kinds are
not added to the NIP-11 list. The relay does not decrypt the content,
talk to a mint, or check a proof signature. Acceptance is
`domain::wallet::tests::a_wallet_replaces_and_a_spent_proof_rolls_into_a_new_token`.

NIP-47 is `configured-and-proven`. `parse_connection` reads a
`nostr+walletconnect://` URI: the wallet pubkey, one or more relays,
the client secret, and an optional `lud16`. Kind `13194` lists the
methods and the encryption schemes. `nip44_v2` wins when it is
offered. A missing encryption tag means NIP-04. Kind `23194` is the
client request. Kind `23195` is the wallet response and names the
request id. A `pay_invoice` command keeps the bolt11 invoice, and the
optional amount must match the invoice. The response carries the
payment preimage or a typed error. A newer info event replaces the
older one. Request and response events are ephemeral. NIP-47 is a
draft, so these kinds are not added to the NIP-11 list. The relay
does not pay the invoice. Acceptance is
`domain::wallet_connect::tests::a_pay_invoice_round_trips_and_an_info_event_replaces`.

NIP-53 is `configured-and-proven`. Kind `30311` is an addressable live
stream. A `p` tag may carry a signature of the activity address
`30311:<pubkey>:<d>`, and that signature is checked. A participant
without a signature stays unmarked. Kind `1311` is a chat message and
must name the stream with an `a` tag. Kind `30312` is a meeting room
and needs one `Host`. Kind `30313` is a meeting in that room. Kind
`10312` is presence in one room at a time, because it is replaceable.
`live_status_is_stale` reports a `live` stream that has had no update
for more than one hour. A newer stream with the same `d` tag replaces
the older one. NIP-53 is a draft, so these kinds are not added to the
NIP-11 list. The relay does not open the stream or the meeting
service, and it does not rewrite a stale `live` status. Acceptance is
`domain::live::tests::a_live_stream_replaces_and_a_chat_names_it`.

NIP-54 is `configured-and-proven`. Kind `30818` is an addressable
wiki article. `normalize_wiki_identifier` lowercases letters, turns
whitespace into `-`, drops punctuation, and keeps numbers and
non-ASCII letters. The `d` tag must already be that form. Djot
reference links with no definition become wikilinks. A defined
reference such as `nostr:npub...` stays a URI. A `fork` or `defer`
marker names both the article address and the event id. Kind `818` is
a merge request: one `a` tag, one `p` tag for that author, and one
`e` tag marked `source`. Kind `30819` redirects a normalized name to
an article. A newer article or redirect with the same `d` tag
replaces the older one. A merge request does not replace. NIP-54 is
a draft, so these kinds are not added to the NIP-11 list. Djot is
not rendered, and the merge source event is not fetched. Acceptance
is
`domain::wiki::tests::a_wiki_article_normalizes_its_name_and_a_merge_names_the_fork`.

NIP-72 is `configured-and-proven`. Kind `34550` is an addressable
community. `p` tags marked `moderator` are the moderators, and the
event author is also a moderator. `relay` tags use the markers
`author`, `requests`, and `approvals`. A kind `1111` post whose `K`
tag is `34550` is rooted at that community. A top-level post includes
the NIP-22 `e` version tag. Kind `4550` approves a post. An approval
that names an `e` tag carries that event as JSON. `is_community_moderator`
reports whether the approval pubkey is the founder or a listed
moderator. A kind `6` or `16` repost into the community embeds the
original event, not an approval. A newer community with the same `d`
tag replaces the older one. An approval does not replace. NIP-72 is
unrecommended, so these kinds are not added to the NIP-11 list. The
relay does not fetch posts or rank approvals. Acceptance is
`domain::community::tests::a_community_lists_moderators_and_a_moderator_approves_a_post`.

NIP-85 is `configured-and-proven`. Kinds `30382`, `30383`,
`30384`, and `30385` are addressable trusted assertions. The `d` tag
is a pubkey, an event id, an address, or a NIP-73 identifier. `rank`
is an integer from 0 to 100. The other declared counts are
non-negative integers. A kind `30385` assertion also carries the
NIP-73 `k` tag. Kind `10040` lists provider keys and relays. Its
content is empty or a NIP-44 ciphertext of more provider rows.
`parse_provider_list` reads that JSON after decryption. A newer
assertion with the same `d` tag replaces the older one. A newer
provider list replaces the older one. NIP-85 is a draft, so these
kinds are not added to the NIP-11 list. The relay does not compute
the scores or decrypt the provider list during admission. Acceptance
is
`domain::assertion::tests::a_trusted_rank_replaces_and_a_provider_list_keeps_a_private_source`.

NIP-5A is `configured-and-proven`. Kind `15128` is one replaceable
root site per pubkey and has no `d` tag. Kind `35128` is an
addressable named site. Its `d` tag is 1 to 13 lowercase letters,
digits, or hyphens and does not end in a hyphen. Each `path` tag maps
an absolute file to a SHA-256. `site_aggregate` hashes the sorted
`<hash> <path>` lines. An `x` tag marked `aggregate` must equal that
digest. Kind `5128` is a regular snapshot: it copies the paths, repeats
the aggregate, and names the site in one `a` tag. A copied site names
its parent in `a` and the lineage origin in `A`. `resolve_site_path`
maps a directory request to `index.html` and a missing file to
`404.html` when that path is published. A newer root site replaces the
older one. A newer named site with the same `d` tag replaces the older
one. A snapshot does not replace. NIP-5A is a draft, so these kinds
are not added to the NIP-11 list. The relay does not serve HTTP or
fetch Blossom blobs. Kind `5128` sits in the NIP-90 numeric range, and
admission reads it as a snapshot. Acceptance is
`domain::nsite::tests::a_root_site_replaces_and_a_snapshot_keeps_the_aggregate`.

NIP-61 is `configured-and-proven`. Kind `10019` is a replaceable
receiving policy. `relay` tags are where a sender publishes the
nutzap. `mint` tags are the URLs the recipient accepts, with optional
unit markers such as `sat` and `usd`. `pubkey` is the P2PK key from
the NIP-60 wallet. It is a 32-byte key, or 33 bytes starting with `02`,
and it is not the user's Nostr key. Kind `9321` carries one or more
P2PK proofs, one mint in `u`, the recipient in `p`, and an optional
comment. `nutzap_matches` checks that the mint, the unit, and the lock
key come from that policy. `nutzap_inbox` is the kind `9321` filter
with `#p`, `#u`, and `since`. A newer policy replaces the older one. A
nutzap does not replace. A kind `7376` history row can mark the nutzap
`redeemed`. NIP-61 is a draft, so these kinds are not added to the
NIP-11 list. The relay does not talk to a mint, verify a DLEQ proof,
or swap a token. The pinned NIP-65 text does not define URL
normalization, so a mint matches only as written. Acceptance is
`domain::nutzap::tests::a_nutzap_uses_the_recipients_mint_and_lock_key`.

NIP-66 is `configured-and-proven`. Kind `30166` is an addressable
observation of one relay. The `d` tag is a normalized `ws://` or
`wss://` URL, or a 32-byte hex pubkey when the relay has no URL.
`normalize_relay_url` lowercases the scheme and host, drops the
default port, and uses `/` when the path is empty. The stored `d` tag
must already be in that form. Optional tags report the network, the
relay type, supported NIPs, requirements, topics, accepted and
rejected kinds, a geohash, and round-trip times. Non-empty content is
a JSON object, the monitor's copy of the NIP-11 document. Kind `10166`
is a replaceable announcement. It names a frequency in seconds, the
checks the monitor runs, and optional timeouts. The pinned example
puts the check name before the milliseconds. The prose puts the
milliseconds first. Both forms are accepted. A newer observation with
the same `d` tag replaces the older one. A newer announcement replaces
the older one. NIP-66 is a draft, so these kinds are not added to the
NIP-11 list. The relay does not open a socket, fetch a NIP-11
document, or measure round-trip time. Percent-encoded relay URLs are
refused. Acceptance is
`domain::monitor::tests::a_relay_observation_replaces_and_a_monitor_lists_its_checks`.

NIP-69 is `configured-and-proven`. Kind `38383` is an addressable
peer-to-peer order. `d` is the order id. `k` is `buy` or `sell`. `f`
is a three-letter currency code. `s` is `pending`, `canceled`,
`in-progress`, `success`, or `expired`. `amt` is the bitcoin amount in
satoshis. `0` means the taker learns the amount later. `fa` is one
fiat amount, or a minimum and a maximum. `pm` lists payment methods,
either as separate tag values or as a comma-separated value. `z` is
`order`. A rating is a JSON object with review and scale fields. The
pending deadline is `expires_at`, and `expiration` is the NIP-40 time
after which a relay can delete the event. A newer order with the same
`d` tag replaces the older one. NIP-69 is a draft, so this kind is not
added to the NIP-11 list. The relay does not fetch a bitcoin price,
visit the source URL, or settle the trade. A three-letter currency
code is not looked up in ISO 4217. An amount of `0` stays `0`.
Acceptance is
`domain::peer_order::tests::a_pending_sell_replaces_when_it_succeeds`.

NIP-F4 is `configured-and-proven`. Kind `10154` is one replaceable
show per podcast key. It has one `title`, and it may have an image, a
description, websites, and `p` tags. A `p` tag may name the role
`host`, `cohost`, or `editor`. Kind `54` is a regular episode with a
title, one or more `audio` URLs, and markdown content. Kind `10064`
lists the podcast keys a person authors. `confirmed_hosts` keeps a
person only when that person's list names the show. A newer show
replaces the older one. A newer author list replaces the older one.
An episode does not replace. The pinned prose says kind `10164` once.
The example and NIP-51 both say kind `10064`, so admission follows
`10064`. Kind `10054` stays with NIP-51. NIP-F4 is a draft, so these
kinds are not added to the NIP-11 list. The relay does not download
audio or images, and it does not read a kind `0` profile. Acceptance
is
`domain::podcast::tests::a_show_replaces_and_a_host_is_confirmed_by_their_list`.

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

NIP-B7 is `configured-and-proven`. `open_server_list` in
`crates/nostr/src/domain/blossom.rs` reads a kind `10063` event: at least
one `server` tag, each a valid `http://` or `https://` URL. The kind is
replaceable, so a newer list supersedes the older one. `media_reference`
reads the 64-character hex tail of a media URL with its optional file
extension, `recovery_url` builds `https://server/<hex>.<ext>` against each
listed server, and `verifies_media` checks the downloaded bytes against
that digest. Admission refuses an empty or malformed list. The relay does
not download media and does not hash file bytes. NIP-B7 is a draft, so
kind `10063` stays off the NIP-11 list. The separate NIP-96 server list is
kind `10096`. Acceptance is
`domain::blossom::tests::a_server_list_recovers_media_by_its_sha256`.

NIP-EE is `configured-and-proven`. `open_key_package` reads a kind `443`
event: one `mls_protocol_version` of `1.0`, one `0x`-hex `ciphersuite`,
optional `extensions` ids, and the hex `KeyPackageBundle` content.
`open_key_package_relays` reads a kind `10051` list of `relay` URIs.
`open_welcome` reads the unsigned kind `444` rumor — one `e` tag naming
the KeyPackage event id and a `relays` tag. `open_group_message` checks
kind `445`: one `h` tag naming the 32-byte Nostr group id and a NIP-44
payload as content. `exporter_conversation_key` derives the group key the
pinned text describes — the `exporter_secret` as sender secret and its
own public key as receiver — and a test round-trips a payload under it.
`commit_wins` applies the `Commit` with the lowest `created_at`, then the
lowest id. `inner_event_hides_the_group` refuses an application rumor
carrying `h`. The crate does not parse an `MLSMessage`, and a relay
cannot decrypt kind `445`. NIP-EE is marked unrecommended — superseded
by the Marmot protocol — so these kinds stay off the NIP-11 list.
Acceptance is
`domain::mls::tests::a_key_package_a_welcome_and_a_group_message_follow_the_pinned_envelopes`
and `domain::mls::tests::the_earliest_commit_wins_then_the_lowest_id`.

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
