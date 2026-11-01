# 7. Field arithmetic mod p

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

Every point on secp256k1 has coordinates in the field of integers mod `p =
2^256 - 2^32 - 977`. This chapter builds that field on the limbs of chapter 2,
including the reduction that uses the special form of `p`, the inverse by
Fermat's little theorem and the square root that lifts an x-only key to a
point.

Part III, Elliptic curves. Result: Addition, multiplication, squaring,
inversion and square roots in F_p for p = 2^256 − 2^32 − 977, with canonical
outputs and exact addition chains for inverse and root.

## What this chapter teaches

- Reduction using `2^256 ≡ 2^32 + 977 (mod p)`.
- Normalization to a canonical value below `p`.
- Inversion as `a^(p-2)` with a fixed addition chain, so its time does not
  depend on `a`.
- Square root as `a^((p+1)/4)`, valid because `p ≡ 3 (mod 4)`, and the check
  that the result squares back.
- Edge cases: 0, `p - 1`, values with maximum magnitude.

## What exists today

- `coax/lib/crypto/secp_field.coax` implements the field with declared
  budgets; `secp_field32.coax` is the 32-bit form.
- Records:
  [coax-secp-field-inv-2026-10-28.md](../../docs/verification/coax-secp-field-inv-2026-10-28.md)
  and
  [2026-10-29-secp-field-guard-proof.md](../../docs/verification/2026-10-29-secp-field-guard-proof.md).

## Dependencies

- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)

## Done means

- Every operation matches the big-integer oracle on edge and random values.
- Inverse and square root use fixed chains, checked by the constant-time
  harness.
- Zero mutation survivors in `secp_field.coax`.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
