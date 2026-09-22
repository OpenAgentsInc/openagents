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
