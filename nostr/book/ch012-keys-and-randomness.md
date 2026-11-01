# 12. Keys and randomness

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A Nostr identity is a random scalar. This chapter defines where randomness
comes from on each host, how a key is drawn from it, and how the rest of the
stack asks a signer for a signature without ever holding the key.

Part III, Elliptic curves. Result: Private keys from host entropy with
rejection of 0 and values at least n, the entropy contract per host, and one
signer-request contract for a local key, a NIP-07 browser signer and a phone
over NIP-46.

## What this chapter teaches

- The entropy contract: the host fills a buffer or fails; there is no fallback
  to a weak source.
- Key generation: draw 32 bytes, refuse 0 and values at least `n`, draw again.
- Mac, iOS Simulator and browser evidence; the Linux branch has not run in the
  recorded environment.
- A signer request that fixes the NIP-01 ID the reply must have, so the reply
  is accepted only for that identity and body.

## What exists today

- `coax/lib/crypto/entropy.coax` with host implementations in
  `coax/platform/entropy-*.c`.
- `coax/lib/nostr/keys.coax` holds the signer-request contract (local slot,
  NIP-07, later NIP-46).
- Record:
  [coax-browser-random-2026-10-28.md](../../docs/verification/coax-browser-random-2026-10-28.md).

## Dependencies

- [8. Scalars mod n](ch008-scalars-mod-n.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Done means

- A key drawn on each host is a valid scalar, and a forced out-of-range draw
  is retried.
- A signer reply with the wrong ID, pubkey or body is refused.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
