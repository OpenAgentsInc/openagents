# 8. Scalars mod n

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Private keys, nonces and the `s` half of a signature are scalars mod the group
order `n`. This chapter builds scalar arithmetic on eight 32-bit limbs and the
range checks that BIP-340 requires.

Part III, Elliptic curves. Result: Scalars mod the group order n: reduction of
32-byte and 64-byte inputs, addition, multiplication and negation, with the s
< n and key-range checks BIP-340 needs.

## What this chapter teaches

- `n = 0xFFFFFFFF FFFFFFFF FFFFFFFF FFFFFFFE BAAEDCE6 AF48A03B BFD25E8C
  D0364141`.
- Reducing a 32-byte hash output mod `n`, as BIP-340 does for the challenge
  `e`.
- Refusing a private key of 0 or one at least `n`.
- Refusing a signature whose `s` is at least `n`.

## What exists today

- `coax/lib/crypto/secp_scalar.coax` with checks in
  `coax/tests/nostr/scalar.py`.
- Notes: `coax/tests/nostr/SCALAR.md`.

## Dependencies

- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Done means

- Scalar operations match the big-integer oracle.
- Out-of-range keys and `s` values refuse, each with its own status.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
