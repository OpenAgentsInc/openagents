# 10. Scalar multiplication

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Signing, verifying and ECDH each multiply a point by a scalar. Verification
works on public data and may run in variable time. Signing and ECDH use
secrets and must not. This chapter builds the three paths and states which one
each later chapter uses.

Part III, Elliptic curves. Result: Three multiplications: a variable-time
public path for verification, a fixed-base table for k·G, and a constant-time
variable-base path for ECDH, each checked against an oracle.

## What this chapter teaches

- Double-and-add with a window for public verification (`s*G - e*P`).
- A precomputed table for `k*G` with fixed-pattern access.
- A constant-time ladder for `k*P` with a secret `k`.
- Measured costs, and why GLV and larger tables wait for measured need.

## What exists today

- `coax/lib/crypto/secp_mult.coax`, `secp_mult_base.coax` (with native and web
  table builds) and `secp_mult_const.coax`.
- Records:
  [coax-secp-mult-verify-2026-10-28.md](../../docs/verification/coax-secp-mult-verify-2026-10-28.md),
  [2026-10-29-secp-fixed-base.md](../../docs/verification/2026-10-29-secp-fixed-base.md),
  [2026-10-29-secp-variable-base.md](../../docs/verification/2026-10-29-secp-variable-base.md).

## Dependencies

- [8. Scalars mod n](ch008-scalars-mod-n.md)
- [9. The secp256k1 group law](ch009-the-secp256k1-group-law.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Done means

- All three paths give the oracle's point for random and edge scalars (0, 1,
  `n - 1`).
- The constant-time paths pass the timing harness with leaky controls
  detected.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
