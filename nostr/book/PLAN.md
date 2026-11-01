# Plan and prerequisites

This plan gives the order of the book, what you have at the end of each part,
and, for every chapter, its dependencies, what "done" means and the spec files
it implements. Spec files are in [`nostr/nips/`](../nips/README.md), pinned by
[`manifest.json`](../nips/manifest.json): the official NIPs from
`nostr-protocol/nips`, the OpenAgents NIPs, and the Block NIPs from
`block/buzz`.

## What you need first

- The Coax language: [Coax book](../../coax/book/README.md) Parts I–III,
  especially [chapter 18, The protocol boundary](../../coax/book/ch18-protocol.md),
  [chapter 23, What verification establishes](../../coax/book/ch23-verification-boundaries.md)
  and [chapter 24, A complete boundary](../../coax/book/ch24-a-complete-boundary.md).
- A built compiler: `coax/build.sh`.
- No outside cryptography or Nostr library. Reference implementations appear
  only in the tests of Part XI, never in the code the book builds.

## The order

| Part | Chapters | What you have at the end |
| --- | --- | --- |
| I. Foundations | 1–3 | Byte views, strict hex and base64, 256-bit numbers in limbs, and the constant-time rules. |
| II. Hashes and MACs | 4–6 | SHA-256, HMAC-SHA256 and HKDF-SHA256 that pass NIST and RFC vectors. |
| III. Elliptic curves | 7–14 | secp256k1 and BIP-340: sign and verify that pass all 19 official vectors, keys from host randomness, and x-only ECDH. |
| IV. Encodings | 15–17 | Bech32, every NIP-19 entity, `nostr:` URIs and the exact event serialization. |
| V. Encryption | 18–20 | ChaCha20, NIP-44 v2 passing its vector file, and a written account of NIP-04's weaknesses. |
| VI. The event model | 21–25 | Events you can verify and sign, kinds and tags, filters, every relay message and gift wrap. |
| VII. Transport | 26–28 | A WebSocket client and server in Coax over the host's socket and TLS. |
| VIII. A client | 29–33 | A client that subscribes, publishes, follows the outbox model and survives restarts. |
| IX. A relay | 34–44 | A relay with durable storage, indexes, NIP-11, NIP-42, deletion, counts, search, protected events, negentropy and limits. |
| X. Every NIP | 45–92 | Every pinned NIP, grouped by subject: the official set, the OpenAgents set and the Block set. |
| XI. Interop and verification | 93–97 | Pinned vectors, differential tests, fuzzing, stated properties and a conformance report. |
| XII. Nostr in OpenAgents | 98–104 | Projects, sign-in, Pylons, the Verse, agents, the production relay and agent treasuries on this stack. |

Parts I–VI form one chain: each chapter uses only earlier ones. Part VII needs
Part I and SHA-1 for the handshake. Part VIII builds the client and Part IX the
relay; both need Parts VI and VII, and the relay's NIP-42 chapter also uses the
client connection of chapter 29. The chapters of Part X each name their
own dependencies; most need only Part VI, and the encrypted ones need Part V.

## Part X: which NIP sets

- **Official NIPs** (`nostr/nips/official/`): every file pinned from
  `nostr-protocol/nips`. Specs that the upstream index marks
  `unrecommended` (03, 04, 06, 08, 15, 26, 28, 31, 72, 90, 96, BE and EE) are
  described with their status. NIPs merged into NIP-01 (12, 16, 20 and 33) are
  covered in Part VI.
- **OpenAgents NIPs** (`nostr/nips/openagents/`): all 36 specs and the shared
  contracts, in chapters 75–86.
- **Block NIPs** (`nostr/nips/block/`): the 20 Buzz extension specs from
  `block/buzz` (`docs/nips`), in chapters 87–92.
- **Payments.** Lightning zaps (NIP-57), zap goals (NIP-75), Wallet Connect
  (NIP-47), payment targets (NIP-A3) and x402 over Lightning are built. Cashu
  (NIP-60, NIP-61) and Fedimint (NIP-87) are excluded by the repository's
  Bitcoin-only rule; [chapter 74](ch074-cashu-and-fedimint-are-excluded.md)
  states this.

## Every chapter

### Part I. Foundations

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md) — implemented | coax:5, coax:6 | Every RFC 4648 test vector for base64 passes, and every non-canonical form refuses. Hex and base64 round trips hold for every length up to the profile limit in a `#forall` check. `coax mutate` leaves zero survivors on the codec files. | — |
| [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md) — implemented | 1, coax:13 | Limb operations match the big-integer oracle on edge values: 0, 1, `p - 1`, `2^256 - 1` and values at each magnitude bound. A `#forall` check confirms no intermediate exceeds its declared bound. Native and wasm builds give the same bytes. | — |
| [3. Constant-time rules and secrets](ch003-constant-time-rules.md) — implemented | 1, 2, coax:23 | A leaky branch fixture and a leaky index fixture are each detected by the timing harness. The constant-time checker refuses a seeded secret-dependent branch with its diagnostic. Every secret buffer in Parts II–V is erased on success and on every failure path. | — |

### Part II. Hashes and MACs

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [4. SHA-256](ch004-sha-256.md) — implemented | 1 | Every NIST CAVS 11.0 SHA256ShortMsg and SHA256LongMsg vector passes. Every split of a 1,000-byte message gives the same digest. The compression kernel moves out of the compiler-fingerprint module without changing a digest. | — |
| [5. HMAC-SHA256](ch005-hmac-sha256.md) — implemented | 4 | RFC 4231 SHA-256 cases 1–7 pass. A tag that differs in one bit is refused by the constant-time comparison. | — |
| [6. HKDF](ch006-hkdf.md) — implemented | 5 | RFC 5869 SHA-256 cases 1–3 pass. An expand request of `255 * 32 + 1` bytes refuses. | — |

### Part III. Elliptic curves

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [7. Field arithmetic mod p](ch007-field-arithmetic-mod-p.md) — implemented | 2, 3 | Every operation matches the big-integer oracle on edge and random values. Inverse and square root use fixed chains, checked by the constant-time harness. Zero mutation survivors in `secp_field.coax`. | — |
| [8. Scalars mod n](ch008-scalars-mod-n.md) — implemented | 2, 3 | Scalar operations match the big-integer oracle. Out-of-range keys and `s` values refuse, each with its own status. | — |
| [9. The secp256k1 group law](ch009-the-secp256k1-group-law.md) — implemented | 7 | Group laws hold on random points: associativity, `P + (-P) = O`, `2P = P + P`. `lift_x` refuses every x whose `x^3 + 7` is not a square. | — |
| [10. Scalar multiplication](ch010-scalar-multiplication.md) — implemented | 8, 9, 3 | All three paths give the oracle's point for random and edge scalars (0, 1, `n - 1`). The constant-time paths pass the timing harness with leaky controls detected. | — |
| [11. Verify a Schnorr signature](ch011-verify-a-schnorr-signature.md) — implemented | 4, 8, 10 | All 19 BIP-340 vectors give their expected result. Fast and reference paths agree on every vector and on random signatures. | — |
| [12. Keys and randomness](ch012-keys-and-randomness.md) — implemented | 8, 3 | A key drawn on each host is a valid scalar, and a forced out-of-range draw is retried. A signer reply with the wrong ID, pubkey or body is refused. | — |
| [13. Sign with Schnorr](ch013-sign-with-schnorr.md) — implemented | 11, 12, 3 | The BIP-340 signing rows (secret key, aux and message given) produce the expected signatures byte for byte. A secret-probe run finds no secret-dependent branch in the emitted signer. An independent library verifies 10,000 signatures from the Coax signer. | — |
| [14. Shared secrets with ECDH](ch014-shared-secrets-with-ecdh.md) — implemented | 10, 12 | The NIP-44 `get_conversation_key` vectors give the expected keys. Every invalid Wycheproof public key refuses whose x-only form names no curve point (all 16 such cases; the chapter accounts for the rest). | — |

### Part IV. Encodings

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [15. Bech32](ch015-bech32.md) — implemented | 1 | Every valid and invalid BIP-173 Bech32 vector gives its expected result. A Bech32m string refuses. | — |
| [16. NIP-19 entities and nostr: URIs](ch016-nip-19-entities-and-nostr-uris.md) — implemented | 15 | The NIP-19 examples in `nostr/nips/official/19.md` decode to their stated values. A `nostr:nsec1…` URI refuses. | [NIP-19](../nips/official/19.md), [NIP-21](../nips/official/21.md) |
| [17. JSON you can sign](ch017-json-you-can-sign.md) — implemented | 1, 4, coax:18 | The JSONTestSuite cases pinned in `coax/tests/vectors/upstream/json/` give their expected results. Serializations of events with every escape class match an independent implementation byte for byte. | [NIP-01](../nips/official/01.md) |

### Part V. Encryption

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [18. ChaCha20](ch018-chacha20.md) — implemented | 1 | RFC 8439 sections 2.3.2 and 2.4.2 vectors pass. A request that would pass counter `2^32 - 1` refuses. | — |
| [19. NIP-04 and why it is retired](ch019-nip-04-and-why-it-is-retired.md) — design draft | 14, 1 | The chapter states every weakness above with a reference to the pinned text. If a reader is built: FIPS-197 and NIST CBC vectors pass, and the reader refuses to encrypt. | [NIP-04](../nips/official/04.md) |
| [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md) — implemented | 6, 14, 18, 5, 1 | Every `valid` and `invalid` entry in `nip44.vectors.json` gives its expected result. Lengths 65,535, 65,536 and 65,537 behave as the active profile states. A ciphertext with a wrong MAC never produces plaintext output. | [NIP-44](../nips/official/44.md) |

### Part VI. The event model

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md) — implemented | 11, 13, 17, coax:18 | A captured real event verifies; altered copies (ID, signature, kind, one content byte) each refuse with their own status. Events signed in Coax verify in an independent library. | [NIP-01](../nips/official/01.md) |
| [22. Kinds and tags](ch022-kinds-and-tags.md) — implemented | 21 | Every range boundary (9999, 10000, 19999, 20000, 29999, 30000, 39999, 40000) classifies correctly. A malformed `a` tag refuses. | [NIP-01](../nips/official/01.md), [NIP-16](../nips/official/16.md), [NIP-33](../nips/official/33.md) |
| [23. Filters](ch023-filters.md) — partly implemented | 22 | Matching agrees with an independent implementation on a generated corpus of events and filters. Over-limit filters refuse instead of truncating. | [NIP-01](../nips/official/01.md), [NIP-12](../nips/official/12.md) |
| [24. Relay messages](ch024-relay-messages.md) — partly implemented | 21, 23 | Every frame written by one side is read back exactly by the other. Captured relay sessions parse without loss. | [NIP-01](../nips/official/01.md), [NIP-20](../nips/official/20.md) |
| [25. Gift wrap](ch025-gift-wrap.md) — implemented | 20, 21, 13 | Wraps from Coax open in an independent implementation and the reverse. A seal whose pubkey differs from the rumor's refuses. | [NIP-59](../nips/official/59.md) |

### Part VII. Transport

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [26. WebSocket frames](ch026-websocket-frames.md) — implemented | 4, 1 | RFC 6455 section 1.3's example key gives `s3pPLMBiTxaQ9kYGzzhZRbK+xOo=`. A fuzzed frame stream never reads past its buffer under ASan. | — |
| [27. A WebSocket server](ch027-a-websocket-server.md) — implemented | 26 | Every malformed upgrade refuses with its HTTP status. A session from the Coax client passes under ASan and UBSan. | — |
| [28. Sockets, TLS and relay URLs](ch028-sockets-tls-and-relay-urls.md) — implemented | 26, coax:20 | `coax/tests/net/tls.sh` passes against local TLS endpoints under ASan and UBSan. A certificate for the wrong name refuses the connection. | — |

### Part VIII. A client

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [29. A relay connection](ch029-a-relay-connection.md) — partly implemented | 24, 28 | A scripted fixture drops, refuses and stalls the relay, and the machine reaches the expected state each time. Zero mutation survivors in `relay.coax`. | [NIP-01](../nips/official/01.md) |
| [30. Subscriptions and EOSE](ch030-subscriptions-and-eose.md) — partly implemented | 29, 23 | A capped relay (37 events per page) still yields the full set by pagination. A missing NIP-67 hint is never shown as complete. | [NIP-01](../nips/official/01.md), [NIP-67](../nips/official/67.md) |
| [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md) — partly implemented | 29, 21 | Each outcome is reached in a scripted session. A publish is never reported as stored without an `OK true` or `duplicate:`. | [NIP-01](../nips/official/01.md) |
| [32. Many relays and the outbox model](ch032-many-relays-and-the-outbox-model.md) — design draft | 30, 31 | Selection for a fixture of 50 authors matches a hand-computed table. Events from several relays merge to one copy per ID. | [NIP-65](../nips/official/65.md) |
| [33. A client that survives restarts](ch033-a-client-that-survives-restarts.md) — design draft | 32, 31 | A crash at each step of a publish leaves exactly one copy delivered after restart. A deleted event does not return after reconnecting to a relay that still has it. | [NIP-01](../nips/official/01.md), [NIP-09](../nips/official/09.md) |

### Part IX. A relay

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [34. Admit an event](ch034-admit-an-event.md) — partly implemented | 21, 24, 27, coax:24 | Forged and malformed events refuse with the expected reason. The differential sessions against the Rust relay match or have a recorded explanation. | [NIP-01](../nips/official/01.md) |
| [35. Store events](ch035-store-events.md) — partly implemented | 34, 22 | A SIGKILL at any write leaves a store that restarts with every acknowledged event. Replacement order matches the NIP-01 rules on tied timestamps. | [NIP-01](../nips/official/01.md) |
| [36. Match filters with indexes](ch036-match-filters-with-indexes.md) — partly implemented | 23, 35 | A `#forall` property: for generated stores and filters, indexed and scanned results are equal. The five project REQs match relay.openagents.com's recording. | [NIP-01](../nips/official/01.md) |
| [37. NIP-11 relay information](ch037-nip-11-relay-information.md) — partly implemented | 34 | Every advertised limit has a test that hits it. `supported_nips` equals the set of NIP chapters marked done for the relay. | [NIP-11](../nips/official/11.md) |
| [38. NIP-42 authentication](ch038-nip-42-authentication.md) — partly implemented | 21, 29, 34 | Each refusal case above has a test. The Psionic chapter 24 Pylon authenticates to the Coax relay over the tailnet (52/52 sessions). | [NIP-42](../nips/official/42.md) |
| [39. Delete, expire and vanish](ch039-delete-expire-and-vanish.md) — partly implemented | 35 | A deletion by another pubkey is ignored. A deleted event cannot be re-published after a restart. An expired event is never returned to a REQ. | [NIP-09](../nips/official/09.md), [NIP-40](../nips/official/40.md), [NIP-62](../nips/official/62.md) |
| [40. Count events](ch040-count-events.md) — design draft | 36 | Exact counts equal the length of the scanned result. HLL sketches built on two relays merge to the sketch of the union. | [NIP-45](../nips/official/45.md) |
| [41. Search](ch041-search.md) — design draft | 36 | Search results on a fixture corpus match the Rust relay's, or the difference is recorded. A filter with an unsupported extension still runs. | [NIP-50](../nips/official/50.md) |
| [42. Protected events](ch042-protected-events.md) — design draft | 38 | A protected event from an unauthenticated connection refuses; after AUTH as the author it is accepted. A protected event from another authenticated key refuses. | [NIP-70](../nips/official/70.md) |
| [43. Negentropy sync](ch043-negentropy-sync.md) — design draft | 36, 4 | The reconciliation of two random sets finds exactly the symmetric difference. Messages interoperate with an independent Negentropy implementation. | [NIP-77](../nips/official/77.md) |
| [44. Limits, policy and floods](ch044-limits-policy-and-floods.md) — partly implemented | 34, 36 | Every limit has a flood case that receives its refusal. Rate limits are configurable and shown in NIP-11. | [NIP-01](../nips/official/01.md), [NIP-11](../nips/official/11.md) |

### Part X. Every NIP

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [45. Profiles, follows and names](ch045-profiles-follows-and-names.md) — partly implemented | 22, 28 | NIP-05 lookups refuse redirects and non-matching keys. A newer kind 3 replaces an older one in the store. | [NIP-02](../nips/official/02.md), [NIP-05](../nips/official/05.md), [NIP-24](../nips/official/24.md), [NIP-39](../nips/official/39.md) |
| [46. Keys from a seed phrase](ch046-keys-from-a-seed-phrase.md) — implemented | 13, 5 | Both NIP-06 vectors produce their stated private and public keys. An invalid checksum refuses. | [NIP-06](../nips/official/06.md) |
| [47. Encrypted key backup](ch047-encrypted-key-backup.md) — partly implemented | 18, 5, 15 | The NIP-49 test vector decrypts to its stated key. A `log_n` of 22 refuses on a profile limited to 20. | [NIP-49](../nips/official/49.md) |
| [48. Signers outside the app](ch048-signers-outside-the-app.md) — design draft | 12, 20, 31 | A Coax client signs through a Coax bunker and an independent bunker. A reused connect secret refuses. | [NIP-07](../nips/official/07.md), [NIP-46](../nips/official/46.md), [NIP-55](../nips/official/55.md) |
| [49. HTTP auth](ch049-http-auth.md) — partly implemented | 21, 4 | Each server check has a refusing test. The same event ID used twice refuses the second time. | [NIP-98](../nips/official/98.md) |
| [50. Notes, threads and subjects](ch050-notes-threads-and-subjects.md) — partly implemented | 22 | Thread reconstruction on a fixture matches a hand-built tree. A positional-only event is read with the documented rule. | [NIP-10](../nips/official/10.md), [NIP-14](../nips/official/14.md), [NIP-36](../nips/official/36.md) |
| [51. References inside content](ch051-references-inside-content.md) — design draft | 16, 50 | Every reference in the NIP-27 examples is found with its byte range. A shortcode with an invalid character is not replaced. | [NIP-27](../nips/official/27.md), [NIP-08](../nips/official/08.md), [NIP-30](../nips/official/30.md) |
| [52. Reposts, reactions and comments](ch052-reposts-reactions-and-comments.md) — partly implemented | 22, 50 | A comment whose root and parent scopes disagree in kind refuses. Reactions aggregate per target with one per author. | [NIP-18](../nips/official/18.md), [NIP-25](../nips/official/25.md), [NIP-22](../nips/official/22.md) |
| [53. Long-form, drafts and wiki](ch053-long-form-drafts-and-wiki.md) — design draft | 22, 20 | NIP-54's normalization examples give the stated `d` values. A NIP-37 draft decrypts only for its author. | [NIP-23](../nips/official/23.md), [NIP-37](../nips/official/37.md), [NIP-54](../nips/official/54.md) |
| [54. Lists, sets and bookmarks](ch054-lists-sets-and-bookmarks.md) — design draft | 22, 20 | Private items round-trip only with the owner's key. A NIP-04-encrypted private section is read and re-encrypted with NIP-44 on the next write. | [NIP-51](../nips/official/51.md), [NIP-B0](../nips/official/B0.md) |
| [55. Labels, reports and trusted assertions](ch055-labels-reports-and-trusted-assertions.md) — design draft | 22 | Labels are grouped by namespace and signer. An assertion from a provider the user did not name is not shown as trusted. | [NIP-32](../nips/official/32.md), [NIP-56](../nips/official/56.md), [NIP-85](../nips/official/85.md) |
| [56. Proof of work](ch056-proof-of-work.md) — design draft | 21 | The NIP-13 example ID has difficulty 21 as stated. An event whose target is below the relay minimum refuses even if its ID has more work. | [NIP-13](../nips/official/13.md) |
| [57. Media and files](ch057-media-and-files.md) — design draft | 22, 49 | Downloaded media whose SHA-256 differs from `x` is refused. Every NIP-92 example parses to its stated fields. | [NIP-92](../nips/official/92.md), [NIP-94](../nips/official/94.md), [NIP-68](../nips/official/68.md), [NIP-71](../nips/official/71.md), [NIP-A0](../nips/official/A0.md), [NIP-B7](../nips/official/B7.md), [NIP-96](../nips/official/96.md) |
| [58. Calendars, polls and highlights](ch058-calendars-polls-and-highlights.md) — design draft | 22 | Poll tallies on a fixture with repeated votes match the NIP's latest-response rule. A date event with `end` before `start` refuses. | [NIP-52](../nips/official/52.md), [NIP-88](../nips/official/88.md), [NIP-84](../nips/official/84.md) |
| [59. Groups and communities](ch059-groups-and-communities.md) — design draft | 38, 34, 22 | A non-member's event to a closed group refuses. Group metadata is accepted only when signed by the relay's key. | [NIP-29](../nips/official/29.md), [NIP-7D](../nips/official/7D.md), [NIP-C7](../nips/official/C7.md), [NIP-72](../nips/official/72.md), [NIP-28](../nips/official/28.md) |
| [60. Private messages](ch060-private-messages.md) — design draft | 25, 32 | A message from Coax opens in an independent NIP-17 client and the reverse. A recipient without kind 10050 receives nothing. | [NIP-17](../nips/official/17.md), [NIP-A4](../nips/official/A4.md), [NIP-EE](../nips/official/EE.md) |
| [61. Live activities](ch061-live-activities.md) — design draft | 22 | An activity with `status` `live` and no update for an hour is shown as ended, as the NIP recommends. Chat events reference the activity by its `a` address. | [NIP-53](../nips/official/53.md) |
| [62. Badges and statuses](ch062-badges-and-statuses.md) — design draft | 22 | An award by someone other than the definition's author is not shown. An expired status is not shown. | [NIP-58](../nips/official/58.md), [NIP-38](../nips/official/38.md) |
| [63. Git on Nostr](ch063-git-on-nostr.md) — partly implemented | 22, 52 | A status from someone who is neither the author nor a maintainer is ignored. The project fold over a captured relay matches its recorded tables. | [NIP-34](../nips/official/34.md), [NIP-C0](../nips/official/C0.md) |
| [64. App data and handlers](ch064-app-data-and-handlers.md) — design draft | 22 | A NIP-5A file whose hash differs from the manifest is refused. Handler lookup for a kind returns announcements from the user's follows first. | [NIP-78](../nips/official/78.md), [NIP-89](../nips/official/89.md), [NIP-5A](../nips/official/5A.md), [NIP-CD](../nips/official/CD.md) |
| [65. Data vending machines](ch065-data-vending-machines.md) — design draft | 22, 20 | A result whose `request` tag does not match the request's ID refuses. The record [gptoss-cloud-nip90-2026-10-30.md](../../docs/verification/gptoss-cloud-nip90-2026-10-30.md) is reproduced or retired. | [NIP-90](../nips/official/90.md) |
| [66. Markets and orders](ch066-markets-and-orders.md) — design draft | 22 | Listings parse with every `price` form in the NIP. An order with `layer` other than Bitcoin, Lightning or Spark is shown but never settled. | [NIP-99](../nips/official/99.md), [NIP-69](../nips/official/69.md), [NIP-15](../nips/official/15.md) |
| [67. Relay discovery, access and management](ch067-relay-discovery-access-and-management.md) — design draft | 37, 38, 49 | Management calls without a valid NIP-98 event from an admin key refuse. A member list not signed by the relay's NIP-11 `self` key is ignored. | [NIP-66](../nips/official/66.md), [NIP-43](../nips/official/43.md), [NIP-86](../nips/official/86.md) |
| [68. Bridges, external IDs and timestamps](ch068-bridges-external-ids-and-timestamps.md) — design draft | 22, 4 | NIP-73 examples normalize to their stated forms. An OTS proof that does not commit to the event ID refuses. | [NIP-48](../nips/official/48.md), [NIP-73](../nips/official/73.md), [NIP-03](../nips/official/03.md) |
| [69. Small content kinds](ch069-small-content-kinds.md) — design draft | 22 | Each NIP's example event parses and round-trips. A kind 64 event with invalid PGN is shown as invalid. | [NIP-35](../nips/official/35.md), [NIP-64](../nips/official/64.md), [NIP-CC](../nips/official/CC.md), [NIP-F4](../nips/official/F4.md) |
| [70. Retired and experimental NIPs](ch070-retired-and-experimental-nips.md) — design draft | 21 | An event with a NIP-26 delegation tag is attributed to its signer, not the delegator. `alt` text is shown for an unknown kind. | [NIP-26](../nips/official/26.md), [NIP-31](../nips/official/31.md), [NIP-BE](../nips/official/BE.md) |
| [71. Lightning zaps and goals](ch071-lightning-zaps-and-goals.md) — design draft | 21, 45 | A receipt whose description hash does not match refuses. Goal progress sums only valid receipts. | [NIP-57](../nips/official/57.md), [NIP-75](../nips/official/75.md) |
| [72. Wallet Connect](ch072-wallet-connect.md) — design draft | 20, 31 | Every command's request and response round-trips with an independent client. A payment over the connection's budget refuses with `QUOTA_EXCEEDED`. | [NIP-47](../nips/official/47.md) |
| [73. Payment targets](ch073-payment-targets.md) — design draft | 22 | Every example in the pinned text parses. A non-Bitcoin target is never offered as a payment option. | [NIP-A3](../nips/official/A3.md) |
| [74. Cashu and Fedimint are excluded](ch074-cashu-and-fedimint-are-excluded.md) — design draft | — | No Coax file implements these kinds, and a check fails if one appears without an owner decision. The chapter lists every kind above with its source line. | [NIP-60](../nips/official/60.md), [NIP-61](../nips/official/61.md), [NIP-87](../nips/official/87.md) |
| [75. Hosts and reachability](ch075-hosts-and-reachability.md) — partly implemented | 20, 21, 49 | A revoked, expired or replayed grant refuses. A stale route generation is not used. | [OpenAgents NIP-HOST](../nips/openagents/NIP-HOST.md), [OpenAgents NIP-REACH](../nips/openagents/NIP-REACH.md), [OpenAgents shared contracts](../nips/openagents/contracts.md) |
| [76. Work, coordination and control](ch076-work-coordination-and-control.md) — design draft | 75 | Two workers claiming one task: exactly one wins, and the other's writes are fenced. A command from an unpaired client refuses. | [OpenAgents NIP-WORK](../nips/openagents/NIP-WORK.md), [OpenAgents NIP-COORD](../nips/openagents/NIP-COORD.md), [OpenAgents NIP-CTRL](../nips/openagents/NIP-CTRL.md), [OpenAgents NIP-CTX](../nips/openagents/NIP-CTX.md) |
| [77. Capabilities, policy and extensions](ch077-capabilities-policy-and-extensions.md) — design draft | 75 | A capability without a matching binding is refused at dispatch. A revoked extension release does not install. | [OpenAgents NIP-CAP](../nips/openagents/NIP-CAP.md), [OpenAgents NIP-POL](../nips/openagents/NIP-POL.md), [OpenAgents NIP-EXT](../nips/openagents/NIP-EXT.md), [OpenAgents NIP-REG](../nips/openagents/NIP-REG.md) |
| [78. Runs, sessions, terminals and trajectories](ch078-runs-sessions-terminals-and-trajectories.md) — design draft | 76, 20 | A run journal replays to the same head after a controller handoff. A trajectory chunk missing from its manifest is reported, not ignored. | [OpenAgents NIP-RUN](../nips/openagents/NIP-RUN.md), [OpenAgents NIP-SESS](../nips/openagents/NIP-SESS.md), [OpenAgents NIP-TERM](../nips/openagents/NIP-TERM.md), [OpenAgents NIP-ATIF](../nips/openagents/NIP-ATIF.md) |
| [79. Agents, skills and automation](ch079-agents-skills-and-automation.md) — design draft | 76, 77 | A saved agent round-trips through its private carriage. A missed scheduled occurrence is recorded once, not run twice. | [OpenAgents NIP-AGENT](../nips/openagents/NIP-AGENT.md), [OpenAgents NIP-SKILL](../nips/openagents/NIP-SKILL.md), [OpenAgents NIP-SOV](../nips/openagents/NIP-SOV.md), [OpenAgents NIP-AUTO](../nips/openagents/NIP-AUTO.md) |
| [80. Jobs, labor and markets](ch080-jobs-labor-and-markets.md) — design draft | 20, 76 | A job result from a key other than the admitted provider refuses. An order settles only with a Lightning or on-chain proof. | [OpenAgents NIP-CJ](../nips/openagents/NIP-CJ.md), [OpenAgents NIP-LAB](../nips/openagents/NIP-LAB.md), [OpenAgents NIP-MKT](../nips/openagents/NIP-MKT.md) |
| [81. Decisions, evaluations, optimization and programs](ch081-decisions-evaluations-optimization-and-programs.md) — design draft | 80 | A decision answer that does not match its request's question set refuses. An evaluation report with an unknown suite digest is shown as unverifiable. | [OpenAgents NIP-DEC](../nips/openagents/NIP-DEC.md), [OpenAgents NIP-EVAL](../nips/openagents/NIP-EVAL.md), [OpenAgents NIP-OPT](../nips/openagents/NIP-OPT.md), [OpenAgents NIP-PRG](../nips/openagents/NIP-PRG.md) |
| [82. Pylons, environments and deploys](ch082-pylons-environments-and-deploys.md) — partly implemented | 80, 75 | An expired beacon contributes no capacity. A receipt signed by a key other than the beacon's refuses. | [OpenAgents NIP-PYLON](../nips/openagents/NIP-PYLON.md), [OpenAgents NIP-ENV](../nips/openagents/NIP-ENV.md), [OpenAgents NIP-DEPLOY](../nips/openagents/NIP-DEPLOY.md) |
| [83. Attested work and sealed data](ch083-attested-work-and-sealed-data.md) — design draft | 20, 82 | An endpoint whose attestation does not match a release head is refused. A vault object opens only with a valid key slot. | [OpenAgents NIP-ATT](../nips/openagents/NIP-ATT.md), [OpenAgents NIP-VAULT](../nips/openagents/NIP-VAULT.md) |
| [84. Knowledge and workspaces](ch084-knowledge-and-workspaces.md) — design draft | 76 | A withdrawn entry is not served as current. A conditional change against a stale version refuses. | [OpenAgents NIP-KB](../nips/openagents/NIP-KB.md), [OpenAgents NIP-WS](../nips/openagents/NIP-WS.md) |
| [85. Live media, worlds and experience](ch085-live-media-worlds-and-experience.md) — design draft | 61, 20 | A pose frame outside its world's bounds refuses. A revoked award is removed from the derived ledger. | [OpenAgents NIP-LIVE](../nips/openagents/NIP-LIVE.md), [OpenAgents NIP-MV](../nips/openagents/NIP-MV.md), [OpenAgents NIP-XP](../nips/openagents/NIP-XP.md) |
| [86. Lightning payments for requests](ch086-lightning-payments-for-requests.md) — partly implemented | 49, 71 | Any other scheme or network refuses. A preimage that does not hash to the payment hash refuses. | [OpenAgents NIP-X402](../nips/openagents/NIP-X402.md) |
| [87. Owner attestation and agent authentication](ch087-owner-attestation-and-agent-authentication.md) — design draft | 38 | An attestation with altered conditions refuses. A revoked agent loses its virtual membership on the next AUTH. | [Block NIP-OA](../nips/block/NIP-OA.md), [Block NIP-AA](../nips/block/NIP-AA.md) |
| [88. Federated identity](ch088-federated-identity.md) — partly implemented | 38, 1 | `none`, symmetric and confused algorithms refuse. A JWS with a wrong audience or community refuses. | [Block NIP-FI](../nips/block/NIP-FI.md), [Block NIP-FA](../nips/block/NIP-FA.md) |
| [89. Channels](ch089-channels.md) — design draft | 59 | A missing page never shows an empty channel. A hidden DM is not returned to a non-participant. | [Block NIP-CL](../nips/block/NIP-CL.md), [Block NIP-DV](../nips/block/NIP-DV.md), [Block NIP-CW](../nips/block/NIP-CW.md), [Block NIP-WP](../nips/block/NIP-WP.md) |
| [90. Artifacts, archives and reminders](ch090-artifacts-archives-and-reminders.md) — design draft | 89 | A revision against a stale version refuses. A reminder is delivered once at its time. | [Block NIP-AR](../nips/block/NIP-AR.md), [Block NIP-IA](../nips/block/NIP-IA.md), [Block NIP-ER](../nips/block/NIP-ER.md) |
| [91. Git signing, projects, leases and read state](ch091-git-signing-projects-leases-and-read-state.md) — partly implemented | 63, 13 | NIP-GS and NIP-RS test vectors pass. The NIP-MP fixtures in `nostr/nips/block/NIP-MP.fixtures.json` fold to their stated results. | [Block NIP-GS](../nips/block/NIP-GS.md), [Block NIP-MP](../nips/block/NIP-MP.md), [Block NIP-PL](../nips/block/NIP-PL.md), [Block NIP-RS](../nips/block/NIP-RS.md) |
| [92. Private agent state](ch092-private-agent-state.md) — design draft | 20, 87 | No AO frame is written to durable storage. Decrypted bodies never appear in public snapshots or telemetry, checked by a canary. | [Block NIP-AE](../nips/block/NIP-AE.md), [Block NIP-AP](../nips/block/NIP-AP.md), [Block NIP-AO](../nips/block/NIP-AO.md), [Block NIP-AM](../nips/block/NIP-AM.md), [Block NIP-PMA](../nips/block/NIP-PMA.md) |

### Part XI. Interop and verification

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [93. Test vectors and their provenance](ch093-test-vectors-and-their-provenance.md) — partly implemented | 4, 11, 20 | `python3 coax/tests/vectors/check_vectors.py` and `test_integrity.py` pass. Every chapter's "done means" names the vector file it uses. | — |
| [94. Differential tests](ch094-differential-tests.md) — partly implemented | 34, 93 | Every difference has a classification and a reason. A seeded fault in the Coax relay produces a new unexplained difference. | — |
| [95. Fuzzing](ch095-fuzzing.md) — partly implemented | 17, 23, 26, 93 | Each target runs for a fixed budget without a finding, recorded with its seed. A seeded out-of-bounds read is found by the fuzzer. | — |
| [96. Formal properties](ch096-formal-properties.md) — design draft | 11, 23, 36 | Each property has a check that a deliberate bug breaks. Counterexamples from any model become regression tests. | — |
| [97. A conformance report](ch097-a-conformance-report.md) — partly implemented | 93, 94 | The report's NIP list equals the set of done chapters. A NIP added to a profile without evidence fails the check. | — |

### Part XII. Nostr in OpenAgents

| Chapter | Depends on | Done means | Spec |
| --- | --- | --- | --- |
| [98. Projects on Nostr](ch098-projects-on-nostr.md) — partly implemented | 63, 30, 21 | A forged or out-of-scope event never enters the verified view. The fold of a captured relay matches its recorded tables. | [NIP-34](../nips/official/34.md), [NIP-22](../nips/official/22.md) |
| [99. The website and sign-in](ch099-the-website-and-sign-in.md) — partly implemented | 46, 49 | The Coax verifier and the C verifier agree on every recorded request. A login event reused after its window refuses. | [NIP-98](../nips/official/98.md), [NIP-06](../nips/official/06.md) |
| [100. Pylons and the mesh](ch100-pylons-and-the-mesh.md) — partly implemented | 82, 20, 38, psionic:24 | A Pylon serves a self-test decision through the Coax relay. A request sealed to another provider is not decrypted. | [OpenAgents NIP-PYLON](../nips/openagents/NIP-PYLON.md), [NIP-44](../nips/official/44.md) |
| [101. The Verse and the wallet](ch101-the-verse-and-the-wallet.md) — design draft | 85, 71, 72, aethyr:49 | Two players see each other's presence through a Coax relay within the latency budget. A zap request over the wallet's limits is refused by the wallet rules. | [OpenAgents NIP-MV](../nips/openagents/NIP-MV.md), [NIP-57](../nips/official/57.md), [NIP-47](../nips/official/47.md) |
| [102. Agents on Nostr](ch102-agents-on-nostr.md) — design draft | 79, 80, 13 | The runner publishes through the Coax stack in staging with the same identities and history. A rollback to the C path is rehearsed. | [OpenAgents NIP-AGENT](../nips/openagents/NIP-AGENT.md), [OpenAgents NIP-CJ](../nips/openagents/NIP-CJ.md) |
| [103. Replace relay.openagents.com](ch103-replace-relay-openagents-com.md) — design draft | 34, 35, 37, 38, 39, 40, 41, 42, 43, 44, 94 | Event-set equality between the two relays at cutover. A rollback rehearsal restores service from the Rust relay. | [NIP-11](../nips/official/11.md) |
| [104. Agent treasuries and a network of keys](ch104-agent-treasuries-and-a-network-of-keys.md) — design draft | 13, 46, 79, 83 | Three software key agents on signet sign a 2-of-3 spend from requests carried over a Coax relay, and a PSBT whose outputs differ from its request is refused by every signer. A coordinator outage is recovered from one key agent's copy of the descriptor with an independent wallet tool. | [OpenAgents NIP-KEYS](../nips/openagents/NIP-KEYS.md), [OpenAgents NIP-SOV](../nips/openagents/NIP-SOV.md), [OpenAgents NIP-ATT](../nips/openagents/NIP-ATT.md) |

Dependencies written as `book:N`, such as `coax:18`, are chapters in another
book.
