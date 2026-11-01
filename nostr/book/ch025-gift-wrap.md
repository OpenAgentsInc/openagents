# 25. Gift wrap

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Gift wrap hides who sent a message and when. A rumor (an unsigned event) is
sealed by its author and wrapped by a one-time key. This chapter builds the
three layers and the checks that unwrap them.

Part VI, The event model. Result: NIP-59 rumors, kind-13 seals and kind-1059
gift wraps from one-time keys, with randomized timestamps, checked in both
directions against an independent implementation.

## What this chapter teaches

- Rumor: an event with an ID but no signature, so it cannot be published as
  is.
- Seal: kind 13, empty tags, content the NIP-44 encryption of the rumor to the
  recipient.
- Wrap: kind 1059 (or 21059) from a fresh key, a `p` tag for the recipient,
  content the encrypted seal.
- Timestamps up to two days in the past, drawn from host randomness.
- Unwrapping: verify the wrap, decrypt, verify the seal, check that the
  rumor's pubkey equals the seal's.

## What exists today

- `coax/lib/nostr/nip59.coax` builds and opens all three layers;
  `coax/tests/nostr/nip59_interop.ts` is the independent direction.

## Dependencies

- [20. NIP-44 encrypted payloads](ch020-nip-44-encrypted-payloads.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)
- [13. Sign with Schnorr](ch013-sign-with-schnorr.md)

## Done means

- Wraps from Coax open in an independent implementation and the reverse.
- A seal whose pubkey differs from the rumor's refuses.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-59](../nips/official/59.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
