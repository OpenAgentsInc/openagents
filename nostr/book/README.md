# Nostr From Scratch

Building Nostr in [Coax](../../coax/book/README.md).

This book builds a complete Nostr implementation in Coax with no outside
libraries. It starts with bytes and big integers, builds SHA-256, HMAC, HKDF,
secp256k1, BIP-340 Schnorr signatures, Bech32, ChaCha20 and NIP-44 from their
specifications, and then builds the event model, a WebSocket transport, a
client and a relay on them. Part X covers every NIP pinned in
[`nostr/nips/`](../nips/README.md): the official set, the OpenAgents set and
Block's Buzz set. Part XI checks the stack against published vectors and
other implementations, and Part XII shows where OpenAgents uses it.

The C host may link one TLS library so native clients reach `wss://` relays.
Every protocol decision above the socket, including WebSocket framing, Nostr
messages, NIP-42 and NIP-44, is Coax code ([AGENTS.md](../../AGENTS.md)).
Money follows the repository's Bitcoin-only rule: Lightning and on-chain
Bitcoin are built; Cashu and Fedimint are described and excluded
([chapter 74](ch074-cashu-and-fedimint-are-excluded.md)).

You need the Coax language (its book's Parts I–III). The
[plan](PLAN.md) gives the order and each chapter's dependencies, done
criteria and spec files. The [contents](SUMMARY.md) list every chapter.
The work to implement every chapter is tracked in [issue #12612](https://github.com/OpenAgentsInc/openagents/issues/12612).

## Status

Most of the cryptography and the core event, client and relay code already
exists in Coax under `coax/lib/crypto/`, `coax/lib/nostr/` and
`coax/lib/net/`, with checks and verification records in
`docs/verification/`. The chapters that cover that code are marked
*Partly implemented*: the code and its checks exist, and the chapter's
teaching text, worked trace and runnable example are not written yet. Their
first line under the title is a quoted `**Status: partly implemented.**`
marker.

A chapter whose status is *Unimplemented: design draft* is written ahead of
its code. Its first line under the title is a quoted
`**Status: unimplemented — design draft.**` marker. It says what the chapter
teaches, with the kinds, tags and messages from the NIP text, what exists
today, its dependencies, what "done" means and its spec files. It has no
source-linked listings or examples.

Of the 104 chapters, 36 are implemented, 20 are partly implemented and
48 are design drafts.

The [chapter registry](chapters.json) records each chapter's status (one of
*implemented*, *partly implemented*, *design draft* or *planned*), its
profile, its dependencies and three separate facts: its implemented source
(`source`), the targets with recorded evidence for its profile
(`qualified_targets`), and the code outside the book that uses it today
(`adopted_by`). The status table below, the contents and each chapter's
status marker must agree with it; `python3 psionic/tests/book.py --book nostr`
fails when they don't.

## Promoting a draft to a teaching chapter

A partly implemented or design draft chapter becomes a teaching chapter only
when it has all four of the following:

1. **A worked calculation or trace.** A small input whose result the chapter
   works out by hand, step by step, before the program runs.
2. **Runnable source.** The code and an example in `nostr/examples/`, listed
   in [examples.json](examples.json), with an exact transcript that the
   book's checks compare in debug and release.
3. **A deliberately broken case and its diagnostic.** An input or an edit
   that the code refuses, with the exact error or refusal shown and its exit
   status checked.
4. **An independent check.** A published test vector, a reference
   implementation or a hand-derived value that the checks compare with the
   code's result.

When all four exist, update the chapter's status in the same commit in four
places: the chapter's status marker, its entry in [chapters.json](chapters.json),
its label in the [contents](SUMMARY.md) and its row in the table below. The
registry check fails when they disagree.

To find a route through the four books, see
[Paths through the books](../../docs/books/paths.md).

## Writing style

Write chapters to the
[Google developer documentation style guide](https://developers.google.com/style):

- Use short, plain sentences in active voice and present tense.
- Address the reader as "you".
- Say what the code does and what was measured. Don't use metaphors,
  personification, or literary phrasing.
- Use numbered lists for steps and bulleted lists for related items.
- Use code font for code, file names, and identifiers.

## Checking the book

Commands run from the repository root.

```shell
python3 psionic/tests/book.py --book nostr
python3 psionic/tests/book.py --book nostr --coax coax/bin/coax
```

The first checks structure: chapter files and headings, the contents, local
links, status markers, the chapter registry and the example inventory in
[examples.json](examples.json). The second also runs every example in
`nostr/examples/` in debug and release and compares its output and exit
status. It is the same checker as the Psionic and Aethyr books'; build the
compiler once with `coax/build.sh`.

Coax listings, equations and short illustrative blocks use `jai` fences;
commands use `shell`.

## Chapters

### Part I. Foundations (chapters 1–3)

Bytes in fixed buffers, hex and base64, big integers in fixed limbs, and the
constant-time rules every later chapter follows.

| Chapter | Status | Result |
| --- | --- | --- |
| [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md) | Runnable: native C | Caller-owned byte views, big- and little-endian loads and stores, strict lowercase hex and canonical padded base64, with every decoder refusing bad input instead of guessing. |
| [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md) | Runnable: native C and wasm | 256-bit numbers as ten 26-bit limbs in `u64` and as eight 32-bit limbs, with carry schedules that never overflow and an independent big-integer oracle. |
| [3. Constant-time rules and secrets](ch003-constant-time-rules.md) | Runnable: native C and wasm | The rules for secret data: no branch, index or variable-time call on a secret, explicit erasure, value barriers, and timing measurements that are evidence, not proof. |

### Part II. Hashes and MACs (chapters 4–6)

SHA-256, HMAC-SHA256 and HKDF-SHA256, each checked against its published
vectors.

| Chapter | Status | Result |
| --- | --- | --- |
| [4. SHA-256](ch004-sha-256.md) | Runnable: native C and wasm | Streaming SHA-256 over caller-owned state, checked against every NIST CAVS ShortMsg and LongMsg vector, with split-stream and padding-boundary checks, plus BIP-340 tagged hashes. |
| [5. HMAC-SHA256](ch005-hmac-sha256.md) | Runnable: native C and wasm | HMAC-SHA256 with keys longer than a block hashed first, checked against RFC 4231 cases 1–7 including the truncated case. |
| [6. HKDF](ch006-hkdf.md) | Runnable: native C and wasm | HKDF-SHA256 extract and expand with output bounds, checked against RFC 5869 cases 1–3, and the exact NIP-44 v2 use: extract with salt `nip44-v2`, expand to 76 bytes. |

### Part III. Elliptic curves (chapters 7–14)

secp256k1 from field arithmetic to BIP-340 verification checked against the 19
official vectors, keys from host randomness, signing, and x-only ECDH.

| Chapter | Status | Result |
| --- | --- | --- |
| [7. Field arithmetic mod p](ch007-field-arithmetic-mod-p.md) | Runnable: native C and wasm | Addition, multiplication, squaring, inversion and square roots in F_p for p = 2^256 − 2^32 − 977, with canonical outputs and exact addition chains for inverse and root. |
| [8. Scalars mod n](ch008-scalars-mod-n.md) | Runnable: native C and wasm | Scalars mod the group order n: reduction of 32-byte inputs, addition, multiplication and negation, with the s < n and key-range checks BIP-340 needs. |
| [9. The secp256k1 group law](ch009-the-secp256k1-group-law.md) | Runnable: native C and wasm | Points on y² = x³ + 7 in affine and Jacobian coordinates, doubling and complete addition with the point at infinity and the P + (−P) case handled. |
| [10. Scalar multiplication](ch010-scalar-multiplication.md) | Runnable: native C and wasm | Three multiplications: a variable-time public path for verification, a fixed-base table for k·G, and a constant-time variable-base path for ECDH, each checked against an oracle. |
| [11. Verify a Schnorr signature](ch011-verify-a-schnorr-signature.md) | Runnable: native C and wasm | BIP-340 verification over byte-view messages, checked against all 19 rows of the official `test-vectors.csv`, including messages of 0, 1, 17 and 100 bytes, with a Nostr wrapper that requires 32-byte event IDs. |
| [12. Keys and randomness](ch012-keys-and-randomness.md) | Runnable: native C and wasm | Private keys from host entropy with rejection of 0 and values at least n, the entropy contract per host, and one signer-request contract for a local key, a NIP-07 browser signer and a phone over NIP-46. |
| [13. Sign with Schnorr](ch013-sign-with-schnorr.md) | Runnable: native C and wasm | BIP-340 signing with auxiliary randomness, a nonce that never repeats for different messages, and fixed-work self-verification before a signature is released. |
| [14. Shared secrets with ECDH](ch014-shared-secrets-with-ecdh.md) | Runnable: native C and wasm | x-only ECDH that validates the peer key and returns the raw shared x, checked against labelled Wycheproof secp256k1 inputs, for NIP-44 conversation keys. |

### Part IV. Encodings (chapters 15–17)

Bech32, NIP-19 and `nostr:` URIs, and the exact JSON that a Nostr event ID
hashes.

| Chapter | Status | Result |
| --- | --- | --- |
| [15. Bech32](ch015-bech32.md) | Runnable: native C | BIP-173 Bech32 (checksum constant 1, never Bech32m) with the pinned vectors and canonical lowercase re-encoding. |
| [16. NIP-19 entities and nostr: URIs](ch016-nip-19-entities-and-nostr-uris.md) | Runnable: native C | `npub`, `nsec` and `note`, and the TLV forms `nprofile`, `nevent` and `naddr` with relay hints, plus the `nostr:` URI scheme of NIP-21. |
| [17. JSON you can sign](ch017-json-you-can-sign.md) | Runnable: native C | A bounded JSON reader and writer that keep original bytes, and the exact NIP-01 serialization `[0, pubkey, created_at, kind, tags, content]` with its escape rules, distinct from RFC 8785 JCS. |

### Part V. Encryption (chapters 18–20)

ChaCha20, the retired NIP-04 scheme and its weaknesses, and NIP-44 v2 checked
against its official vectors.

| Chapter | Status | Result |
| --- | --- | --- |
| [18. ChaCha20](ch018-chacha20.md) | Runnable: native C and wasm | RFC 8439 ChaCha20 with caller-supplied nonce and counter, counter exhaustion refused, and HChaCha20 for NIP-49's XChaCha20. |
| [19. NIP-04 and why it is retired](ch019-nip-04-and-why-it-is-retired.md) | Unimplemented: design draft | NIP-04's AES-256-CBC with the shared x as key and its `?iv=` suffix, its weaknesses (no authentication, metadata in the clear, the key used as is), and the decision to read it only where a consumer needs it. |
| [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md) | Runnable: native C | NIP-44 version 2: conversation keys, padding, ChaCha20, HMAC over nonce‖ciphertext checked before decryption, and every valid and invalid vector in `nip44.vectors.json`. |

### Part VI. The event model (chapters 21–25)

NIP-01 events, IDs and signatures, kinds and tags, filters, the relay messages
that carry them, and NIP-59 gift wrap built on all of them.

| Chapter | Status | Result |
| --- | --- | --- |
| [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md) | Runnable: native C and wasm | The NIP-01 event: `id`, `pubkey`, `created_at`, `kind`, `tags`, `content`, `sig`; recomputing the ID, verifying the signature, and building signed events separately from reading them. |
| [22. Kinds and tags](ch022-kinds-and-tags.md) | Runnable: native C | Kind ranges (regular, replaceable 0, 3 and 10000–19999, ephemeral 20000–29999, addressable 30000–39999 by `d` tag) and the standard tags `e`, `p`, `a` and `d`. |
| [23. Filters](ch023-filters.md) | Runnable: native C and wasm | NIP-01 filters: `ids`, `authors`, `kinds`, `#<letter>` tags, `since`, `until` and `limit`; AND across fields, OR within a list, inclusive times, and absent lists distinct from empty ones. |
| [24. Relay messages](ch024-relay-messages.md) | Runnable: native C | Client frames `EVENT`, `REQ`, `CLOSE`, `AUTH` and `COUNT`, relay frames `EVENT`, `OK`, `EOSE`, `CLOSED`, `NOTICE` and `AUTH`, and the `OK`/`CLOSED` reason prefixes NIP-20 introduced. |
| [25. Gift wrap](ch025-gift-wrap.md) | Runnable: native C | NIP-59 rumors, kind-13 seals and kind-1059 gift wraps from one-time keys, with randomized timestamps, checked in both directions against an independent implementation. |

### Part VII. Transport (chapters 26–28)

WebSocket framing as a client and a server in Coax, over the host's socket and
TLS.

| Chapter | Status | Result |
| --- | --- | --- |
| [26. WebSocket frames](ch026-websocket-frames.md) | Runnable: native C | RFC 6455 as a client: the opening handshake with `Sec-WebSocket-Accept` from SHA-1, masked client frames, fragmentation, ping, pong and close with status codes. |
| [27. A WebSocket server](ch027-a-websocket-server.md) | Runnable: native C | The server side of RFC 6455: accept the upgrade, refuse unmasked client frames, write unmasked frames, enforce frame and message limits and close cleanly. |
| [28. Sockets, TLS and relay URLs](ch028-sockets-tls-and-relay-urls.md) | Runnable: native C | The host boundary: the C host opens sockets and does TLS only; Coax parses `ws://` and `wss://` URLs, decides every frame and owns every protocol decision above the socket. |

### Part VIII. A client (chapters 29–33)

A relay connection with generations and backoff, subscriptions and EOSE,
publishing with acknowledgements, the outbox model and a client that survives
restarts.

| Chapter | Status | Result |
| --- | --- | --- |
| [29. A relay connection](ch029-a-relay-connection.md) | Runnable: native C | The pure client state machine: OPEN, CONNECTING, CLOSED and BACKOFF, connection generations that make stale reports harmless, and capped jittered backoff. |
| [30. Subscriptions and EOSE](ch030-subscriptions-and-eose.md) | Runnable: native C | Subscription IDs that name a slot and its serial, EOSE as the end of stored events and never a completeness claim, NIP-67 completeness hints, and paginated catch-up. |
| [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md) | Runnable: native C | Publishing with acknowledgement deadlines, the distinction between published, acknowledged, refused and unknown, and requeue after AUTH. |
| [32. Many relays and the outbox model](ch032-many-relays-and-the-outbox-model.md) | Unimplemented: design draft | NIP-65 relay lists (kind 10002 with `r` tags marked `read` or `write`), reading from an author's write relays, writing to your write relays and the tagged users' read relays. |
| [33. A client that survives restarts](ch033-a-client-that-survives-restarts.md) | Unimplemented: design draft | A durable client store of verified originals, an offline outbox that sends each event once after a restart, and deletions that are never resurrected on reconnect. |

### Part IX. A relay (chapters 34–44)

Admission, durable storage, filter matching with indexes, NIP-11, NIP-42,
deletion and expiration, NIP-45 counts, NIP-50 search, NIP-70, NIP-77
negentropy, and limits.

| Chapter | Status | Result |
| --- | --- | --- |
| [34. Admit an event](ch034-admit-an-event.md) | Runnable: native C | The relay's EVENT admission: verify ID and signature, check size, tag and time limits, answer `OK` with the standard prefixes, and route each frame from one connection. |
| [35. Store events](ch035-store-events.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Verified originals with one copy per ID, replaceable and addressable replacement with the same-second lowest-ID rule, an fsync before `OK`, and crash recovery that cuts only a partial last write. |
| [36. Match filters with indexes](ch036-match-filters-with-indexes.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Answering REQ from the store newest first with query cost limits, then indexes by ID, author, kind and tag that never change which events match. |
| [37. NIP-11 relay information](ch037-nip-11-relay-information.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | The relay information document served on `Accept: application/nostr+json`: name, pubkey, `supported_nips`, software, version and `limitation` fields that match what the relay enforces. |
| [38. NIP-42 authentication](ch038-nip-42-authentication.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Challenges bound to one connection and one relay URL, kind 22242 events with `relay` and `challenge` tags checked within a time window, and AUTH on both the client and the relay. |
| [39. Delete, expire and vanish](ch039-delete-expire-and-vanish.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | NIP-40 `expiration` tags enforced at admission and query, NIP-09 kind-5 deletion requests from the same author with durable tombstones, and NIP-62 kind-62 requests to vanish. |
| [40. Count events](ch040-count-events.md) | Unimplemented: design draft | NIP-45 `COUNT` requests and responses, `approximate` counts, and HyperLogLog with 256 registers whose offset comes from the filter. |
| [41. Search](ch041-search.md) | Unimplemented: design draft | NIP-50's `search` filter field, results ordered by relevance, and the `key:value` extensions (`include:spam`, `domain:`, `language:`, `sentiment:`, `nsfw:`). |
| [42. Protected events](ch042-protected-events.md) | Unimplemented: design draft | Events tagged `["-"]` accepted only from their authenticated author, so a third party cannot republish them to this relay. |
| [43. Negentropy sync](ch043-negentropy-sync.md) | Unimplemented: design draft | NIP-77's `NEG-OPEN`, `NEG-MSG`, `NEG-CLOSE` and `NEG-ERR`, and Negentropy protocol V1: range-based set reconciliation over `(created_at, id)` with fingerprints. |
| [44. Limits, policy and floods](ch044-limits-policy-and-floods.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Per-connection and global limits, rate limits with `rate-limited:` reasons, storage caps, slowloris timeouts and a write policy that is declared, not implicit. |

### Part X. Every NIP (chapters 45–92)

Every other official NIP pinned in `nostr/nips/official/`, every OpenAgents
NIP and every Block (Buzz) NIP, grouped by subject and ordered by dependency.

| Chapter | Status | Result |
| --- | --- | --- |
| [45. Profiles, follows and names](ch045-profiles-follows-and-names.md) | Runnable: native C | Kind 0 profiles and their `nip05` value, kind 3 follow lists with relay hints and petnames, the newest list replacing older ones in the store, and NIP-05 identifiers checked offline against a `/.well-known/nostr.json` response, refusing redirects and other keys. NIP-24's extra fields and NIP-39 claims are not read yet. |
| [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md) | Runnable: native C | BIP-39 words to seed with PBKDF2-HMAC-SHA512, BIP-32 derivation at m/44'/1237'/0'/0/0, and the NIP-06 key and its npub. |
| [47. Encrypted key backup](ch047-encrypted-key-backup.md) | Runnable: native C and wasm | NIP-49 `ncryptsec`: NFKC-normalized passwords, scrypt with `log_n` from the payload, XChaCha20-Poly1305 with the key-security byte as associated data, and bounded KDF memory. |
| [48. Signers outside the app](ch048-signers-outside-the-app.md) | Unimplemented: design draft | NIP-07's `window.nostr`, NIP-46 remote signing over kind 24133 with `bunker://` and `nostrconnect://` URIs, and NIP-55's Android signer intents. |
| [49. HTTP auth](ch049-http-auth.md) | Runnable: native C and wasm | NIP-98 kind 27235 events in `Authorization: Nostr <base64>`, binding the exact URL, method and payload hash within 60 seconds, with single-use event IDs. |
| [50. Notes, threads and subjects](ch050-notes-threads-and-subjects.md) | Runnable: native C | Kind 1 notes, NIP-10 marked `e` tags (`root`, `reply`, `mention`) with the deprecated positional form, `p` tags for participants, NIP-14 `subject` and NIP-36 `content-warning`. |
| [51. References inside content](ch051-references-inside-content.md) | Unimplemented: design draft | NIP-27 `nostr:` references in content, the deprecated NIP-08 `#[index]` mentions, and NIP-30 `:shortcode:` custom emoji with `emoji` tags and kind 30030 sets. |
| [52. Reposts, reactions and comments](ch052-reposts-reactions-and-comments.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Kind 6 and 16 reposts, kind 7 reactions (`+`, `-`, emoji) and kind 17 website reactions, and NIP-22 kind 1111 comments with uppercase root and lowercase parent scope tags. |
| [53. Long-form, drafts and wiki](ch053-long-form-drafts-and-wiki.md) | Unimplemented: design draft | Kind 30023 articles and 30024 drafts in Markdown, NIP-37 kind 31234 draft wraps encrypted with NIP-44, and NIP-54 kind 30818 wiki articles with `d`-tag normalization. |
| [54. Lists, sets and bookmarks](ch054-lists-sets-and-bookmarks.md) | Unimplemented: design draft | NIP-51 standard lists (10000 mutes, 10001 pins, 10003 bookmarks and others) and sets (30000 follow sets and others) with public tags and private items encrypted to self, and NIP-B0 kind 39701 web bookmarks. |
| [55. Labels, reports and trusted assertions](ch055-labels-reports-and-trusted-assertions.md) | Unimplemented: design draft | NIP-32 kind 1985 labels in `L` namespaces, NIP-56 kind 1984 reports with report types, and NIP-85 kinds 30382–30385 assertions from declared providers (kind 10040). |
| [56. Proof of work](ch056-proof-of-work.md) | Unimplemented: design draft | NIP-13 difficulty as the number of leading zero bits of the event ID, the `nonce` tag with its target, and relay admission by `pow:` reason. |
| [57. Media and files](ch057-media-and-files.md) | Unimplemented: design draft | `imeta` tags (NIP-92), kind 1063 file metadata (NIP-94), picture (20) and video (21, 22, 34235, 34236) events, voice messages (1222, 1244), Blossom servers (kind 10063) and the deprecated NIP-96. |
| [58. Calendars, polls and highlights](ch058-calendars-polls-and-highlights.md) | Unimplemented: design draft | NIP-52 date (31922) and time (31923) events, calendars (31924) and RSVPs (31925); NIP-88 polls (1068) and responses (1018); NIP-84 highlights (9802). |
| [59. Groups and communities](ch059-groups-and-communities.md) | Unimplemented: design draft | NIP-29 relay-based groups with `h` tags, moderation kinds 9000–9022 and relay-signed metadata 39000–39005; NIP-7D forum threads (kind 11); NIP-C7 chats (kind 9); and the unrecommended NIP-72 and NIP-28. |
| [60. Private messages](ch060-private-messages.md) | Unimplemented: design draft | NIP-17 kind 14 chat and kind 15 file messages sent as gift wraps, kind 10050 DM relay lists, NIP-A4 kind 24 public messages, and the unrecommended NIP-EE MLS groups. |
| [61. Live activities](ch061-live-activities.md) | Unimplemented: design draft | NIP-53 kind 30311 live events with `streaming`, `status` and participant roles, kind 1311 live chat, and kind 30312/30313 meeting spaces and rooms with kind 10312 presence. |
| [62. Badges and statuses](ch062-badges-and-statuses.md) | Unimplemented: design draft | NIP-58 badge definitions (30009), awards (8) and profile badges (10008, formerly 30008), and NIP-38 kind 30315 user statuses with `expiration`. |
| [63. Git on Nostr](ch063-git-on-nostr.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | NIP-34 repository announcements (30617) and state (30618), patches (1617), pull requests (1618, 1619), issues (1621) and statuses (1630–1633), and NIP-C0 code snippets (1337). |
| [64. App data and handlers](ch064-app-data-and-handlers.md) | Unimplemented: design draft | NIP-78 kind 30078 application data, NIP-89 handler recommendations (31989) and announcements (31990), NIP-5A static websites (35128 and 15128 manifests), and NIP-CD slash commands (31992, 10992). |
| [65. Data vending machines](ch065-data-vending-machines.md) | Unimplemented: design draft | NIP-90 job requests (5000–5999), results (6000–6999) and feedback (7000), job chaining, and why the pinned text marks it `unrecommended`. |
| [66. Markets and orders](ch066-markets-and-orders.md) | Unimplemented: design draft | NIP-99 classified listings (30402 and 30403 drafts) with `price` in a currency and frequency, NIP-69 peer-to-peer order events (38383), and the unrecommended NIP-15 marketplace. |
| [67. Relay discovery, access and management](ch067-relay-discovery-access-and-management.md) | Unimplemented: design draft | NIP-66 relay discovery (30166) and monitor announcements (10166), NIP-43 membership lists (13534) and join and leave requests (28934–28936), and the NIP-86 JSON-RPC management API over HTTP with NIP-98 auth. |
| [68. Bridges, external IDs and timestamps](ch068-bridges-external-ids-and-timestamps.md) | Unimplemented: design draft | NIP-48 `proxy` tags for bridged events, NIP-73 `i` and `k` tags for external content IDs (URLs, ISBNs, podcasts, geohashes), and NIP-03 kind 1040 OpenTimestamps attestations. |
| [69. Small content kinds](ch069-small-content-kinds.md) | Unimplemented: design draft | NIP-35 torrents (2003), NIP-64 chess games in PGN (64), NIP-CC geocaches (37516) and logs (7516, 7517), and NIP-F4 podcasts. |
| [70. Retired and experimental NIPs](ch070-retired-and-experimental-nips.md) | Unimplemented: design draft | NIP-26 delegation tokens, the NIP-31 `alt` tag and NIP-BE Bluetooth LE transport, each marked `unrecommended` in the pinned text, with what this stack reads and what it never builds. |
| [71. Lightning zaps and goals](ch071-lightning-zaps-and-goals.md) | Unimplemented: design draft | NIP-57 zap requests (9734) sent to an LNURL server and zap receipts (9735) whose bolt11 description hash commits to the request, and NIP-75 zap goals (9041). |
| [72. Wallet Connect](ch072-wallet-connect.md) | Unimplemented: design draft | NIP-47 Nostr Wallet Connect: `nostr+walletconnect://` URIs, info (13194), requests (23194) and responses (23195) encrypted with NIP-44, and the commands `pay_invoice`, `make_invoice`, `get_balance` and others. |
| [73. Payment targets](ch073-payment-targets.md) | Unimplemented: design draft | NIP-A3 kind 10133 `payto` tags, with only the Bitcoin and Lightning target types acted on under the repository rule. |
| [74. Cashu and Fedimint are excluded](ch074-cashu-and-fedimint-are-excluded.md) | Unimplemented: design draft | NIP-60 Cashu wallets, NIP-61 nutzaps and NIP-87 mint discovery, described for reading only; the repository's Bitcoin-only rule excludes Cashu and Fedimint from implementation. |
| [75. Hosts and reachability](ch075-hosts-and-reachability.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | NIP-HOST invitations, enrollment, grants with epochs and revocation bound to the original host-signed envelope, and NIP-REACH host directories, presence, hints and direct channels. |
| [76. Work, coordination and control](ch076-work-coordination-and-control.md) | Unimplemented: design draft | NIP-WORK tracked items and revisions, NIP-COORD claims with fencing tokens and leases, NIP-CTRL client pairing and commands, and NIP-CTX task frames and context receipts. |
| [77. Capabilities, policy and extensions](ch077-capabilities-policy-and-extensions.md) | Unimplemented: design draft | NIP-CAP execution descriptions and bindings, NIP-POL inspectable host policy and approvals, NIP-EXT signed component packages, and NIP-REG curated plugin registries. |
| [78. Runs, sessions, terminals and trajectories](ch078-runs-sessions-terminals-and-trajectories.md) | Unimplemented: design draft | NIP-RUN durable run journals with controller handoff, NIP-SESS engine sessions, NIP-TERM terminal sessions, and NIP-ATIF agent trajectories as manifests and chunks. |
| [79. Agents, skills and automation](ch079-agents-skills-and-automation.md) | Unimplemented: design draft | NIP-AGENT saved agents (30187), NIP-SKILL published `SKILL.md` listings (30188), NIP-SOV sovereign agents with local custody, and NIP-AUTO scheduled and triggered work. |
| [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md) | Unimplemented: design draft | NIP-CJ encrypted conversation, typed-decision and execution jobs, NIP-LAB agent labor terms and delivery, and NIP-MKT offerings, negotiated orders and Bitcoin settlement. |
| [81. Decisions, evaluations, optimization and programs](ch081-decisions-evaluations-optimization-and-programs.md) | Unimplemented: design draft | NIP-DEC decision requests and answers (25910), NIP-EVAL suites and reports, NIP-OPT optimization studies, and NIP-PRG typed programs (30182, 30183). |
| [82. Pylons, environments and deploys](ch082-pylons-environments-and-deploys.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | NIP-PYLON beacons (30200), service receipts (3201) and pool aggregates (30201), NIP-ENV environment leases, and NIP-DEPLOY public deploy status (3205, 30440). |
| [83. Attested work and sealed data](ch083-attested-work-and-sealed-data.md) | Unimplemented: design draft | NIP-ATT attested workload releases (3202), release heads (30202), rebuilds (3203) and endpoints, and NIP-VAULT sealed personal data with wraps and key slots. |
| [84. Knowledge and workspaces](ch084-knowledge-and-workspaces.md) | Unimplemented: design draft | NIP-KB knowledge entries (3190), heads (30190), withdrawals (3191) and evidence, and NIP-WS workspaces with documents, ranges, worktrees and checkpoints. |
| [85. Live media, worlds and experience](ch085-live-media-worlds-and-experience.md) | Unimplemented: design draft | NIP-LIVE voice, camera and screen sessions, NIP-MV 3D-world presence (world 33300, entity 33301, pose frames 23300), and NIP-XP quests (30193), awards (3193) and revocations (3194). |
| [86. Lightning payments for requests](ch086-lightning-payments-for-requests.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | NIP-X402: x402 v2 over HTTP with the three base64 JSON headers, Bitcoin mainnet Lightning with a bolt11 invoice in millisats, and the `openagents:spark-receiver:1` settlement profile. |
| [87. Owner attestation and agent authentication](ch087-owner-attestation-and-agent-authentication.md) | Unimplemented: design draft | Block's NIP-OA `auth` tag by which an owner key authorizes an agent key, and NIP-AA agent authentication to relays with virtual member privileges and revocation. |
| [88. Federated identity](ch088-federated-identity.md) | Partly implemented: native C and wasm (code and checks exist; the teaching text is not written) | NIP-FI stateless assertions with bounded compact JWS, issuer, audience, community and token-class binding, and NIP-FA browser login, code exchange and relay assertion. |
| [89. Channels](ch089-channels.md) | Unimplemented: design draft | NIP-CL channel identity and discovery (44100, 44101), NIP-DV DM visibility, NIP-CW channel and thread windows with composite cursors, and NIP-WP workspace profiles. |
| [90. Artifacts, archives and reminders](ch090-artifacts-archives-and-reminders.md) | Unimplemented: design draft | NIP-AR channel artifacts (45010) with compare-and-swap revisions, NIP-IA identity archival with consent, and NIP-ER event reminders (30300). |
| [91. Git signing, projects, leases and read state](ch091-git-signing-projects-leases-and-read-state.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | NIP-GS Git object signing with Nostr keys and its test vectors, NIP-MP multi-repository projects (30621), NIP-PL push leases (30350, 30390) and NIP-RS cross-device read state. |
| [92. Private agent state](ch092-private-agent-state.md) | Unimplemented: design draft | NIP-AE engrams (30174), NIP-AP personas (30175–30178), NIP-AO ephemeral observation (24200), NIP-AM turn metrics (44200) and the NIP-PMA private managed-agent aggregate reservation. |

### Part XI. Interop and verification (chapters 93–97)

Pinned vectors, differential tests against reference implementations, fuzzing,
formal properties and a conformance report.

| Chapter | Status | Result |
| --- | --- | --- |
| [93. Test vectors and their provenance](ch093-test-vectors-and-their-provenance.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Every vector file pinned by source URL, revision, license, retrieval date and SHA-256, with derived adapters labelled and their input hashes recorded. |
| [94. Differential tests](ch094-differential-tests.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Running the same inputs through Coax and a reference implementation: the Rust relay from history in a network-less sandbox, and independent libraries for signatures, NIP-44 and NIP-59. |
| [95. Fuzzing](ch095-fuzzing.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Coverage-guided fuzzing of every parser (JSON, frames, filters, NIP-19, NIP-44 payloads) under ASan and UBSan, with pinned seeds and minimized regressions kept as tests. |
| [96. Formal properties](ch096-formal-properties.md) | Unimplemented: design draft | Properties stated and checked with `#forall` and models: signature validity matches BIP-340's definition, filter matching is AND across fields and OR within lists, and indexes never change results. |
| [97. A conformance report](ch097-a-conformance-report.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | A machine-readable support matrix per profile (`core-read`, `core-write`, `phone-signer`, `openagents-host`, `block-identity`, `block-channel`, `agent-private`, `key-backup`) that refuses to advertise a NIP whose runtime gate is missing. |

### Part XII. Nostr in OpenAgents (chapters 98–104)

Where projects, the website, Pylons, the Verse, the wallet and agents use the
stack, and the cutover of relay.openagents.com.

| Chapter | Status | Result |
| --- | --- | --- |
| [98. Projects on Nostr](ch098-projects-on-nostr.md) | Partly implemented: native C and wasm (code and checks exist; the teaching text is not written) | OpenAgents' project pages fold NIP-34 repositories, issues and statuses and NIP-22 comments from the relay, verify each event in the browser, and replace a snapshot only with a complete bounded capture. |
| [99. The website and sign-in](ch099-the-website-and-sign-in.md) | Partly implemented: native C and wasm (code and checks exist; the teaching text is not written) | Logging in to openagents.com with a NIP-98 signed request from a words account (NIP-06) or a browser signer (NIP-07), and the cutover of the C verifier to Coax. |
| [100. Pylons and the mesh](ch100-pylons-and-the-mesh.md) | Partly implemented: native C (code and checks exist; the teaching text is not written) | Pylons authenticate to the relay with NIP-42, announce beacons, and answer sealed NIP-44 decision requests; the mesh opens encrypted channels with this book's primitives. |
| [101. The Verse and the wallet](ch101-the-verse-and-the-wallet.md) | Unimplemented: design draft | Presence in realms with NIP-MV pose frames, Verse identities as Nostr keys, and zaps and Wallet Connect over the Spark wallet's existing payment rules. |
| [102. Agents on Nostr](ch102-agents-on-nostr.md) | Unimplemented: design draft | Hosted project agents and Coder publishing with this book's signer and relay client, replacing the C runner's own signing path. |
| [103. Replace relay.openagents.com](ch103-replace-relay-openagents-com.md) | Unimplemented: design draft | Exporting the Rust relay's Postgres read-only, importing with re-verification of every event, a shadow period comparing both relays, and DNS cutover only with owner sign-off. |
| [104. Agent treasuries and a network of keys](ch104-agent-treasuries-and-a-network-of-keys.md) | Unimplemented: design draft | NIP-KEYS multisig wallets across independent key agents: descriptors and composition rules, enrollment with possession proofs, PSBT signing requests and receipts, an attested co-signer enforcing a SOV treasury policy, timelocked recovery, rotation sweeps and a hot/cold treasury split. |
