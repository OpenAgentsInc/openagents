# 3. Constant-time rules and secrets

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

A signing key, a nonce or a conversation key must not leak through time or
memory. This chapter states the rules every later chapter follows when it
touches a secret, the Coax features that enforce part of them, and the
measurements that check the rest.

Part I, Foundations. Result: The rules for secret data: no branch, index or
variable-time call on a secret, explicit erasure, value barriers, and timing
measurements that are evidence, not proof.

## What this chapter teaches

- The three rules: a secret never decides a branch, never selects a memory
  address and never reaches a variable-time routine.
- Constant-time equality and select in `bytes.coax`, and `value_barrier` for
  `u32` and `u64` (commit `b9a5f216f0`).
- Explicit erasure with `zeroize.coax`, and why a compiler copy of a by-value
  secret is a separate gate.
- Timing measurements with deliberately leaky fixtures, and why no detection
  is scoped evidence.
- What this chapter does not claim: target-code timing on every CPU and
  production-secret approval.

## What exists today

- `coax/lib/crypto/zeroize.coax`, `secret-flow.coax` and `secret-types.coax`
  hold the erasure contract and secret-typed values.
- `coax/tests/constant_time.py` checks the emitted C for secret-dependent
  branches; `tools/coax-nostr/timing/` measures native and browser timing with
  leaky controls ([README](../../tools/coax-nostr/timing/README.md)).
- Record:
  [coax-nostr-ct-view-2026-10-28.md](../../docs/verification/coax-nostr-ct-view-2026-10-28.md).

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [Coax chapter 23, What verification
  establishes](../../coax/book/ch23-verification-boundaries.md)

## Done means

- A leaky branch fixture and a leaky index fixture are each detected by the
  timing harness.
- The constant-time checker refuses a seeded secret-dependent branch with its
  diagnostic.
- Every secret buffer in Parts II–V is erased on success and on every failure
  path.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
