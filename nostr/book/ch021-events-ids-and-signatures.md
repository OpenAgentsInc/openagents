# 21. Events, IDs and signatures

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

The event is the only object in Nostr. This chapter reads an event, recomputes
its ID from chapter 17's serialization, verifies its signature with chapter
11, and builds new signed events with chapter 13.

Part VI, The event model. Result: The NIP-01 event: `id`, `pubkey`,
`created_at`, `kind`, `tags`, `content`, `sig`; recomputing the ID, verifying
the signature, and building signed events separately from reading them.

## What this chapter teaches

- The seven fields and their exact types: 32-byte lowercase hex, integer
  seconds, integer kind 0–65535, array of string arrays.
- Verification order: shape, ID recomputation, signature. A parsed event is
  not a verified one.
- Signing: build the serialization, hash, sign, and check the result before
  release.
- Read and sign are separate files, so read-only consumers never link a
  signer.

## What exists today

- `coax/lib/nostr/event.coax` reads and verifies; `event_sign.coax` builds
  signed events.
- Records:
  [coax-nostr-event-2026-10-28.md](../../docs/verification/coax-nostr-event-2026-10-28.md),
  [2026-10-30-event-sign.md](../../docs/verification/2026-10-30-event-sign.md),
  [2026-10-29-nip01-page.md](../../docs/verification/2026-10-29-nip01-page.md).

## Dependencies

- [11. Verify a Schnorr signature](ch011-verify-a-schnorr-signature.md)
- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)
- [17. JSON you can sign](ch017-json-you-can-sign.md)
- [Coax chapter 18, The protocol boundary](../../coax/book/ch18-protocol.md)

## Done means

- A captured real event verifies; altered copies (ID, signature, kind, one
  content byte) each refuse with their own status.
- Events signed in Coax verify in an independent library.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
