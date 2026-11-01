# 9. The secp256k1 group law

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

The secp256k1 group is the set of points on `y^2 = x^3 + 7` over `F_p` plus a
point at infinity. This chapter builds point addition and doubling in Jacobian
coordinates, which avoid an inverse per step, and handles every exceptional
case.

Part III, Elliptic curves. Result: Points on y² = x³ + 7 in affine and
Jacobian coordinates, doubling and complete addition with the point at
infinity and the P + (−P) case handled.

## What this chapter teaches

- The curve equation and the generator `G`.
- Jacobian coordinates `(X, Y, Z)` for `(X/Z^2, Y/Z^3)`.
- Doubling, mixed addition, and the cases where the formulas fail: equal
  points, opposite points and infinity.
- Lifting an x-only public key to the point with even `y` (BIP-340 `lift_x`),
  and refusing an x that is not on the curve.

## What exists today

- `coax/lib/crypto/secp_group.coax` with the secret path's complete
  exceptional handling.
- Record:
  [coax-secp-group-2026-10-28.md](../../docs/verification/coax-secp-group-2026-10-28.md).

## Dependencies

- [7. Field arithmetic mod p](ch007-field-arithmetic-mod-p.md)

## Done means

- Group laws hold on random points: associativity, `P + (-P) = O`, `2P = P +
  P`.
- `lift_x` refuses every x whose `x^3 + 7` is not a square.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
