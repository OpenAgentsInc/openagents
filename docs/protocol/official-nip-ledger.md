# Official NIP ledger

The source lane and the inventory in
[`crates/nostr/src/lane.rs`](../../crates/nostr/src/lane.rs) name
`b82211e96c6dad616ed2ea43034c1c621256b745`: 99 specifications plus the upstream
README, including A3. The source-inventory guard checks that pin and the complete
file list. It does not certify protocol behavior. `PREVIOUS_BEHAVIOR_COMMIT`
retains `c53877571f96eb423661fc23c620d629d37b8f19` for the existing fixture
baseline; `CURRENT_CLIENT_REVIEWS` names the narrower updated client roles.
Historical `configured-and-proven` labels below describe that retained fixture
coverage, not every behavior in a NIP or every newly synced recommendation.

The [September 26 assessment](2026-09-26-upstream-nip-sync.md) and
[official implementation review](2026-09-26-upstream-nip-sync-official.md)
record the source changes and original gaps. This ledger records the subsequent
implementation. The pure `nostr` suite passes 290 tests. Strict all-target
Clippy and live PostgreSQL acceptance cover the new relay privacy, concurrent
management, snapshot, and restart fixtures. The [verification record](verification/2026-09-26-nips/README.md)
binds those results to a code revision and records the limits of the full gate.

| Current-source change | Implemented role | Boundary |
| --- | --- | --- |
| NIP-02 | Traversal-order petname display and offline resolution | NIP-05 roots require caller-verified bindings; no network resolver |
| NIP-22 | Comments can name kind-1 roots and parents | Existing scope and author checks still apply |
| NIP-30 | Event-local emoji tokens, including kind-1111 comments | No image fetch or product renderer integration |
| NIP-42 and NIP-67 | Typed `auth` EOSE hint, independent of current-view completeness | The relay does not emit this optional hint or automatically authenticate a client |
| NIP-43 | Signed relay declarations and fresh join/leave request parsing | No claim issuer, claim redemption, or complete membership service |
| NIP-78 | Author-authenticated publication; author-only history, live delivery, and counts; no content search | Opaque storage is not encryption; ownership is the signing key |
| NIP-84 | Structured `i` and arbitrary-text `r` sources; optional attribution | No source fetching or provenance verification |
| NIP-86 | Serialized allow/ban mutations remove the opposite list entry atomically | Optional administrative host roles still require configuration |
| NIP-A3 | Typed targets and escaped, inert payment URIs | No address validation, wallet dispatch, or payment authority |

## What a row proves

NIP-02's kind `3` follow list is replaceable. Each `p` tag names a
32-byte hex key, an optional relay, and an optional petname.
`parse_follow_list`, `append_follow`, `displayed_petname`, and
`resolve_petname` live in
[`domain/follow.rs`](../../crates/nostr/src/domain/follow.rs). The display
helper keeps direct names and emits indirect names in traversal order, such as
`~/erin/david/frank`. The resolver walks caller-supplied published lists from
the viewer, an explicit `npub`, or a caller-verified NIP-05 root. Missing data
returns no resolution; malformed components and ambiguous aliases refuse.
Display names outside the ASCII path grammar remain displayable but cannot
become an indirect path. The helper neither fetches lists nor verifies NIP-05
ownership. A newer list replaces its author's previous list through ordinary
store replacement. Fixtures cover traversal, cycles bounded by the supplied
path, unknown roots, missing lists, and ambiguity; the retained replacement
fixture is `lane::tests::nip02_follow_lists_replace_and_petnames_chain`.

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

NIP-22's kind `1111` comment names one uppercase root scope (`A`, `E`,
or `I`) and one lowercase parent scope (`a`, `e`, or `i`). `K` and `k` are
required; a Nostr scope also requires its author. An addressable parent carries
its event ID in `e`. External scopes use NIP-73 identifiers with matching kind
tags. Kind `1` roots and parents are accepted, including replies to a comment
beneath a kind-1 root. A top-level comment uses the same root and parent.
Admission rejects malformed scopes and missing authors. Content remains text;
kind `1111` is regular storage and is not advertised as a separate relay role.
Fixture: `domain::comment::tests::a_comment_scopes_to_the_root_including_kind_1`.

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

NIP-84's kind `9802` highlight retains the selected text, or empty content
for non-text media. Sources include Nostr `a` and `e` references, structured
NIP-73 `i` identifiers, and arbitrary `r` text such as a book description.
Attribution is recommended by the spec and is not mandatory admission policy.
An ordinary highlight can omit the `r` marker; a quote highlight's HTTP(S) URLs
require `source` or `mention`. `p` roles distinguish authors, editors, and
mentions; `context` retains surrounding text, and `comment` supplies quote
commentary. `clean_source_url` is an optional local utility, not a protocol
requirement or relay rewrite. The parser does not fetch a source or prove that
a quotation is accurate. Fixtures in
[`domain/highlight.rs`](../../crates/nostr/src/domain/highlight.rs) cover mixed
sources, text-only references, source-less media, structured-source errors,
and quote URL markers. The event remains regular stored data.

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

NIP-78 uses addressable kind `30078` for an application coordinate and
regular kind `78` for multiple records. Content and unrelated tags stay opaque;
a `d` tag groups regular rows and identifies replacement for `30078`. The relay
now adopts the spec's authentication and author-only serving recommendations
as its local policy: publication requires NIP-42 authentication as the signing
author, and history, live delivery, HTTP query results, and counts disclose
these kinds only to that author. Another authenticated key, including an agent
owner distinct from the signer, gains no access. Both kinds are excluded from
content search. Opaque application `h` data is not a Block group membership
claim. The relay does not decrypt or interpret application content.

The pure replacement fixture remains
`domain::app_data::tests::an_application_record_replaces_on_its_identifier_and_a_plain_event_does_not`.
The new `current_private_gateway_contract` in
[`gateway_postgres.rs`](../../crates/nostr-relay/tests/gateway_postgres.rs)
checks cross-process history, live delivery, counts, and search for authors,
other readers, and unauthenticated readers. Live PostgreSQL acceptance passed;
see the [verification record](verification/2026-09-26-nips/README.md).
NIP-RS provides a separately configured complete
own-author `30078` cut; that Block extension is not a NIP-78 merge engine.

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

NIP-A4 is `configured-and-proven`. `open_public_message` in
`crates/nostr/src/domain/public_message.rs` reads a kind `24` event: at
least one `p` tag naming a 32-byte hex receiver with an optional
`ws://` or `wss://` relay hint, no `e` tag — the pinned text has no
threads — and a well-formed `q` tag when one cites an event. An
`expiration` tag is recommended and read through `Event::expiration`.
`targets_public_message` checks the `k` tag `24` that reactions and
zaps aimed at the kind must carry, and `link_kind` reads the kind a
`nevent1` or `naddr1` link declares so a client can honor the rule that
a natively rendered public-message link names kind `24`. Admission
refuses a threaded or receiverless message. The kind carries no
privacy and stores like any regular note. NIP-A4 is a draft, so kind
`24` stays off the NIP-11 list. Acceptance is
`domain::public_message::tests::a_public_message_names_its_receivers_and_has_no_thread`.

NIP-C7 is `configured-and-proven`. `open_chat` in
`crates/nostr/src/domain/chat.rs` reads a kind `9` event and validates
each `q` tag: an event id or address target, an optional relay URL,
and an optional 32-byte hex author key. `is_chat_reply` tells a reply
from a bare message, and `chat_filter` returns the `kinds=[9]` filter a
chat view must fetch so implementations keep each other's context.
Admission refuses a malformed quote. The relay stores kind `9` like
any regular event, and NIP-C7 is a draft, so it stays off the NIP-11
list. Acceptance is
`domain::chat::tests::a_chat_reply_quotes_its_parent_and_the_stream_fetches_kind_9`.

NIP-70 is `configured-and-proven`. `Event::is_protected` reads the lone
`-` tag, and the gateway refuses the event with `auth-required:` unless
the connection has completed NIP-42 `AUTH` as the event's own author —
the pinned alternative to rejecting every protected event outright.
`Event::embeds_protected_repost` catches a kind `6` or `16` whose content
embeds a protected event, and the gateway refuses that too. Live
acceptance is `protected_and_private_contract` in
`crates/nostr-relay/tests/gateway_postgres.rs`: the author's own event is
accepted after AUTH, another key's copy is refused, and both repost
forms are refused. The tag cannot stop a reader republishing the
content elsewhere — it only keeps this relay from facilitating it, as
the pinned text says.

NIP-14 is `configured-and-proven`. `subject` reads the `subject` tag on
a kind `1` event, `subject_fits` applies the recommended 80-character
bound, and `reply_subject` produces the `Re:`-adorned subject a reply
SHOULD replicate — without doubling the prefix on a deeper reply. The
relay stores kind `1` either way, and NIP-14 is a draft, so the tag
stays off the NIP-11 list. Acceptance is
`domain::subject::tests::a_reply_replicates_the_subject_with_a_re_prefix`.

NIP-25 is `configured-and-proven`. `open_reaction` reads a kind `7`
event: the last `e` tag names the reacted event, the last `p` tag names
its author, an `a` tag carries the `kind:pubkey:d` address of an
addressable target, and a `k` tag is the reacted kind as a string.
`reaction_verdict` reads `+` or empty as a like, `-` as a dislike, and
anything else as custom; a `:shortcode:` content requires exactly one
`emoji` tag naming that code with a URL. `open_external_reaction`
checks kind `17`, which must carry the NIP-73 `i` and `k` tags that
identify external content. Admission refuses a targetless or malformed
reaction. NIP-25 is a draft, so the kinds stay off the NIP-11 list.
Acceptance is
`domain::reaction::tests::a_reaction_names_its_target_as_the_last_e_tag`
and `domain::reaction::tests::an_external_reaction_carries_i_and_k_tags`.

NIP-24 is `configured-and-proven`. `open_profile_extras` reads the
kind `0` extras — `display_name`, `website`, `banner`, `bot`, and
`birthday` — and ignores the deprecated `displayName` and `username`
spellings entirely rather than folding them into the new names. The
deprecated kind `3` relay read/write object is likewise ignored; the
follow parser already reads `p` tags only. `lowercase_hashtags`
enforces the `t`-tag MUST — admission refuses an uppercase or empty
hashtag on any kind. NIP-24 is a draft, so the fields stay off the
NIP-11 list. Acceptance is
`domain::extras::tests::extra_profile_fields_parse_and_deprecated_ones_are_ignored`
and `domain::extras::tests::hashtags_are_lowercase`.

NIP-31 is `configured-and-proven`. `fallback_summary` hands a
`kind:1`-only client the `alt` tag's human-readable summary for any
kind it does not render natively, and leaves kind `1` alone — its
content is already text. NIP-31 is unrecommended and a draft, so the
tag stays off the NIP-11 list and admission does not require it.
Acceptance is
`domain::alt::tests::an_alt_tag_summarizes_a_kind_a_text_client_does_not_render`.

NIP-36 is `configured-and-proven`. `content_warning` reports whether an
event carries the `content-warning` tag and the optional reason the
author attached; a reader hides the content until its user acts. The
NIP binds client display only — the relay stores the tag — and as a
draft it stays off the NIP-11 list. Acceptance is
`domain::content_warning::tests::a_content_warning_hides_the_content_until_the_reader_acts`.

NIP-08 is `configured-and-proven` for compatibility.
`resolve_mentions` walks a kind `1` content for `#[index]`
placeholders and returns those that name an `e` or `p` tag at that
tags position; an out-of-range index or a placeholder pointing at any
other tag is normal text and is never replaced, as the pinned MUST NOT
requires. NIP-08 is unrecommended — NIP-27 `nostr:` links supersede
it — so it stays off the NIP-11 list. Acceptance is
`domain::mentions::tests::an_index_mention_resolves_to_its_e_or_p_tag_only`.

NIP-38 is `configured-and-proven`. `open_user_status` reads a kind
`30315` addressable event: the `d` tag names `general`, `music`, or a
client-defined type; `r`, `p`, `e`, and `a` tags link a URL, profile,
note, or address; `expiration` ends it; empty content clears it.
`is_live` applies that rule against a timestamp. Admission requires
the `d` identifier and validates link values for their kind. NIP-38 is
a draft, so the kind stays off the NIP-11 list. Acceptance is
`domain::status::tests::a_status_names_its_type_and_may_link_and_expire`.

NIP-7D is `configured-and-proven`. `open_thread` reads a kind `11`
thread and its recommended `title`; `is_thread_reply` accepts only a
kind `1111` comment whose root `E` scope names the thread and whose
`K` hint is `11`, so reply hierarchies never nest. NIP-7D is a draft,
so the kind stays off the NIP-11 list. Acceptance is
`domain::thread::tests::a_thread_carries_its_title_and_a_reply_points_at_the_root`.

NIP-13 is `configured-and-proven`. `pow_difficulty` counts an id's
leading zero bits — the pinned 36-bit example checks — and
`nonce_commitment` reads the `nonce` tag's declared target, so
`meets_committed_target` can refuse a lucky low-target id; a missing
commitment reports `None` for caller policy, as the pinned MAY allows.
The relay does not mine or require work, and NIP-13 is a draft, so it
stays off the NIP-11 list. Acceptance is
`domain::pow::tests::difficulty_counts_leading_zero_bits_and_the_commitment_gates`.

NIP-48 is `configured-and-proven`. `proxy_sources` reads each `proxy`
tag's id and protocol, `known_protocol` names the defined set —
`activitypub`, `atproto`, `rss`, `web` — and `is_bridged` reports the
marking; unknown protocols parse because the list may extend. The tag
is stored data binding client reconciliation only. Acceptance is
`domain::proxy::tests::a_proxy_tag_links_the_bridged_source`.

NIP-27 is `configured-and-proven`. `decode_reference` turns a `nostr:`
entity into its typed reference — `npub`/`nprofile` author,
`note`/`nevent` id with relay hints, author, and kind, or `naddr`
reassembled into its `kind:author:d` address — and `text_references`
finds each decodable code in content while leaving undecodable tokens
as text. Acceptance is
`domain::references::tests::nostr_references_decode_to_their_entity`.

NIP-73 is `configured-and-proven`. `external_id_kind` classifies an
`i` tag's value into every defined kind — `web`, `isbn`, `geo`,
`iso3166` including subdivisions, `isan`, `doi`, `#`, the three
podcast guid forms, and `<chain>:tx`/`<chain>:address` — and
`open_external_ids` requires each `i` to pair with a `k` declaring
that kind and validates the optional URL hint. The kind `17` reaction
admission uses the pairing; it is not enforced on every kind because
NIP-39 `i` tags mean identity claims. NIP-22 comment external scopes
share the classifier. NIP-73 is a draft, so the tags stay off the
NIP-11 list. Acceptance is
`domain::external_id::tests::every_i_tag_classifies_and_pairs_with_its_k`
and `domain::external_id::tests::malformed_identifiers_and_missing_kinds_are_refused`.

NIP-92 is `configured-and-proven`. `parse_imeta` enforces the tag's
MUSTs — a `url` and at least one other `key value` field — and
`open_imetas` applies them at admission wherever `imeta` appears.
`imeta_for` matches a tag to the content URL it describes and
`has_duplicate_urls` flags the one-per-URL SHOULD. NIP-92 is a draft,
so the tag stays off the NIP-11 list. Acceptance is
`domain::imeta::tests::an_imeta_tag_carries_a_url_and_at_least_one_field`
and `domain::imeta::tests::malformed_imetas_are_refused`.

NIP-68 is `configured-and-proven`. `open_picture` reads a kind `20`
post: the `title`, the content description, and at least one `imeta`
whose MIME type is one of the six accepted image types.
`annotated_user` parses the `pubkey:posX:posY` field. Admission
requires the title and an accepted image. NIP-68 is a draft, so the
kind stays off the NIP-11 list. Acceptance is
`domain::picture::tests::a_picture_post_carries_its_images_in_imeta_tags`.

NIP-71 is `configured-and-proven`. `open_video` reads kinds `21`,
`22`, `34235`, and `34236`: the `title`, a `d` identifier on the
addressable kinds, and at least one `imeta` stream — `video/*`,
`audio/*`, or HLS — with numeric `duration` and `bitrate` and an
integer `waveform`. Admission enforces each requirement. NIP-71 is a
draft, so the kinds stay off the NIP-11 list. Acceptance is
`domain::video::tests::a_video_event_carries_streams_and_its_address`.

NIP-28 is `configured-and-proven` for compatibility. `open_channel`
parses kind `40`'s JSON `name`, `about`, `picture`, and `relays` plus
`t` categories; `open_channel_metadata` requires the `e` `root` naming
the channel and `metadata_updates_channel` enforces the pinned rule
that only the channel's author updates it; `open_channel_message`
reads the `root` and `reply` markers; `open_hide_message` and
`open_mute_user` name their `e` and `p` targets with an optional
`reason`. Admission validates each kind's structure while all five
store as regular events. NIP-28 is unrecommended — NIP-29 supersedes
it — so the kinds stay off the NIP-11 list. Acceptance is
`domain::channel::tests::channels_open_update_and_take_messages` and
`domain::channel::tests::malformed_channel_events_are_refused`.

NIP-34 is `configured-and-proven`. `open_repository` reads a kind
`30617` announcement — the required `d`, the optional `name`,
`description`, `web`, `clone`, `relays`, `maintainers`, `t` labels,
and the `euc`-marked `r` commit that identifies the project across
forks — and `open_repository_state` reads `30618`'s `refs/heads` and
`refs/tags` commit ids plus the `HEAD` ref. `open_patch` (1617),
`open_pull_request` (1618 and the `1619` update form), and
`open_issue` (1621) require the `30617:pubkey:identifier` `a` tag;
the PR forms also require a `c` tip commit and at least one `clone`
URL. `open_patch_status` maps kinds `1630`–`1633` to open, applied,
closed, and draft against an `e` `root` target, and `applied_commits`
reads the merge or applied commit evidence. Admission validates every
required tag; all kinds store as regular or addressable events.
NIP-34 is a draft, so the kinds stay off the NIP-11 list. Acceptance
is `domain::git::tests::a_repository_announces_itself_and_its_state`
and `domain::git::tests::malformed_git_events_are_refused`.

NIP-51 is `configured-and-proven`. `open_list` reads both surfaces of
the pinned tables: standard replaceable lists (kind `3` and the
`10000`–`10102` family) and addressable sets (`30000` follow sets and
the `30002`–`39092` family), each set carrying its required `d` plus
optional `title`, `image`, and `description`. Public members parse as
typed `ListItem`s — `p` with relay hint and petname, `e`, `a`, `t`,
`word`, `relay`, `emoji`, `group`, `server`, `url`, `r`, and a carried
`Other` — and `private_items` decrypts the content's NIP-44 tag array
under the author's own conversation key, while
`private_items_encoding` detects and refuses the deprecated NIP-04
`?iv=` form. `deprecated_standard_list` maps the legacy `"mute"`,
`"pin"`, `"bookmark"`, and `"communities"` set shapes to their
standard kinds. Admission requires a non-empty `d` on every set kind
and a numeric `d` on the kind-`30007` mute set. NIP-51 is a draft, so
the kinds stay off the NIP-11 list. Acceptance is
`domain::lists::tests::private_items_round_trip_through_nip44`,
`lists_and_sets_open_with_typed_items`,
`deprecated_set_shapes_map_to_standard_lists`, and
`malformed_lists_are_refused`.

NIP-67 separates a complete current authorized view from an opportunity to
authenticate for more. `domain::eose::open_eose` accepts legacy messages,
multiple known hints, unknown hints, and trailing fields. `EoseHint::Auth` and
`authentication_may_reveal_more()` expose the opportunity without changing
`complete()`: `finish` means `Some(true)`, `more` without finish means
`Some(false)`, and auth-only or unknown hints leave completeness unknown.
This also supplies the NIP-42 client-side hint role. The relay continues to
emit `finish` or `more`, not the optional `auth` hint. If emission is added,
NIP-42 requires an AUTH challenge before that EOSE.

`query_history` probes beyond per-filter and combined limits to distinguish a
truncated result. `wire::eose_message` sends the third element, and NIP-11
advertises `67`. These hints concern stored events; live delivery is separate,
and the recommendation to preserve boundary timestamp ties across pagination
is not implemented. Fixtures are
`gateway::subscription::tests::a_truncated_history_announces_more_at_eose` and
`domain::eose::tests::finish_and_more_answer_completeness`.

NIP-30's [`domain/emoji.rs`](../../crates/nostr/src/domain/emoji.rs) produces
text and emoji tokens for kinds `0`, `1`, `1111`, `7`, and `30315`. Profile
emojification is limited to `name` and `about`. Definitions are local to the
event; unknown shortcodes remain literal text. The helper validates shortcode,
HTTP(S) image reference, and optional emoji-set coordinates, rejects ambiguous
definitions, and performs no network or HTML rendering. Fixtures cover comment
and profile fields, unknown codes, duplicate definitions, and unsafe URLs.

NIP-43's [`domain/relay_access.rs`](../../crates/nostr/src/domain/relay_access.rs)
checks signatures, protection, and the expected relay self-key on membership
and role declarations. Its separate join/leave parser checks freshness using
the caller's clock and tolerance. The required opaque join `claim` stays private in
Debug output. Parsing a claim does not redeem it, grant membership, or
implement issuance. Removed kind `28935` has no claim-issuance role. Existing
relay membership handlers remain narrower than a complete NIP-43 service.

NIP-86's `Store::manage` holds the policy lock while an allow mutation removes
the banned entry, or a ban mutation removes the allowed entry, and posts the
selected entry in the same transaction. The
[`store_postgres.rs`](../../crates/nostr-relay/tests/store_postgres.rs) fixture
races opposing mutations from two connections and requires exactly one final
list entry. That race passed in live PostgreSQL acceptance. This
change does not turn unsigned calls into administrative authority.

NIP-A3's [`domain/payment_target.rs`](../../crates/nostr/src/domain/payment_target.rs)
opens replaceable kind `10133` lists of `payto` type/address pairs, preserves
unknown well-formed types, and constructs inert URIs with encoded components.
Known Bitcoin and Ethereum targets use their URI schemes; other types use
`payto://`. Malformed types and empty addresses refuse. The parser does not
validate network addresses or make a payment. Fixtures cover replacement,
unknown types, malformed fields, and query/fragment characters treated as data.

The historical `Shape` list is empty. A historical `configured-and-proven`
label still means the fixture-backed role described in its row, not complete
client, relay, application, or host behavior. The explicit current-source
review table adds narrower evidence instead of reclassifying all old rows.

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
