# 17. JSON you can sign

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

An event ID is the SHA-256 of one exact JSON array. Any difference in
whitespace or escaping changes the ID. This chapter builds a bounded JSON
reader and writer and the serialization NIP-01 specifies, and explains why JCS
is not the same thing.

Part IV, Encodings. Result: A bounded JSON reader and writer that keep
original bytes, and the exact NIP-01 serialization `[0, pubkey, created_at,
kind, tags, content]` with its escape rules, distinct from RFC 8785 JCS.

## What this chapter teaches

- The serialization: no whitespace, UTF-8, and the escapes NIP-01 lists: `\n`,
  `\"`, `\\`, `\r`, `\t`, `\b`, `\f`; other characters are written as they
  are.
- Bounded parsing: depth, string length and token limits; duplicate keys
  refuse.
- Integers above `2^53 - 1` refuse for `created_at` and `kind`.
- Original bytes are kept, so a signed event is never re-serialized for
  storage.
- RFC 8785 JCS for OpenAgents envelopes, and why it is never an event
  preimage.

## What exists today

- `coax/lib/nostr/json.coax` reads and writes bounded JSON (commit
  `8f23a77f35`); `adapters/json-admission.coax` refuses key ambiguity and
  unsafe integers.
- `coax/lib/nostr/jcs.coax` holds the integer-only RFC 8785 subset.
- Records:
  [coax-json-decoder-checks-2026-10-28.md](../../docs/verification/coax-json-decoder-checks-2026-10-28.md),
  [coax-nostr-json-admission-2026-10-28.md](../../docs/verification/coax-nostr-json-admission-2026-10-28.md).

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [Coax chapter 18, The protocol boundary](../../coax/book/ch18-protocol.md)

## Done means

- The JSONTestSuite cases pinned in `coax/tests/vectors/upstream/json/` give
  their expected results.
- Serializations of events with every escape class match an independent
  implementation byte for byte.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- [NIP-01](../nips/official/01.md)
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
