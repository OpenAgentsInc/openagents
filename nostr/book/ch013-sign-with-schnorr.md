# 13. Sign with Schnorr

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Signing uses a private key and a nonce. A repeated or biased nonce reveals the
key. This chapter builds BIP-340 signing with its nonce derivation, checks the
result against the official signing vectors and verifies each signature before
it leaves the signer.

Part III, Elliptic curves. Result: BIP-340 signing with auxiliary randomness,
a nonce that never repeats for different messages, and fixed-work
self-verification before a signature is released.

## What this chapter teaches

- Key negation when `d*G` has odd `y`.
- Nonce: `t = d xor hash_aux(a)`, `k0 = int(hash_nonce(t || P || m)) mod n`,
  refuse `k0 = 0`.
- `s = (k + e*d) mod n` and the output `R.x || s`.
- Self-verification with fixed work: nothing branches on the candidate
  signature before it is released.
- Resumable stages that stay under the interpreter's check budget.

## What exists today

- `coax/lib/crypto/schnorr_sign.coax` and `schnorr_self_verify.coax`, with
  stage workspaces.
- Records:
  [2026-10-30-schnorr-sign.md](../../docs/verification/2026-10-30-schnorr-sign.md),
  [2026-10-29-schnorr-self-verify.md](../../docs/verification/2026-10-29-schnorr-self-verify.md),
  [2026-10-30-signer-independent.md](../../docs/verification/2026-10-30-signer-independent.md).

## Dependencies

- [11. Verify a Schnorr signature](ch011-verify-a-schnorr-signature.md)
- [12. Keys and randomness](ch012-keys-and-randomness.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Done means

- The BIP-340 signing rows (secret key, aux and message given) produce the
  expected signatures byte for byte.
- A secret-probe run finds no secret-dependent branch in the emitted signer.
- An independent library verifies 10,000 signatures from the Coax signer.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
