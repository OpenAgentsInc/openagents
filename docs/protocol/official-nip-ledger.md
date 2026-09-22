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
