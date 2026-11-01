# 2. Big integers in fixed limbs

> **Status: partly implemented.** The code and its checks exist in Coax; the
> teaching text, worked trace and runnable example are not written yet. The
> registry lists the source files.

secp256k1 works on 256-bit numbers, and Coax has no 256-bit integer type. This
chapter shows how a 256-bit number becomes a fixed array of limbs, how
addition, multiplication and reduction keep every intermediate value inside
`u64`, and how a separate big-integer implementation checks the result.

Part I, Foundations. Result: 256-bit numbers as ten 26-bit limbs in `u64` and
as eight 32-bit limbs, with carry schedules that never overflow and an
independent big-integer oracle.

## What this chapter teaches

- Ten little-endian `u64` limbs: nine of 26 bits and one of 22 bits, so a
  52-bit product fits with room for carries.
- Magnitude: how many unreduced additions a limb can take before it must be
  normalized.
- Eight 32-bit limbs for scalars mod `n`, and why the two representations
  differ.
- Multiplication with widening products, without `u128`.
- Checking each operation against Python's integers in the test harness, never
  against the code's own output.

## What exists today

- `coax/lib/crypto/secp_field.coax` holds the 10x26 representation with
  declared magnitude budgets.
- `coax/lib/crypto/secp_scalar.coax` holds the 8x32 scalar representation with
  a proven carry schedule.
- Records:
  [coax-secp-field-2026-10-28.md](../../docs/verification/coax-secp-field-2026-10-28.md)
  and
  [coax-secp-field-mul-2026-10-28.md](../../docs/verification/coax-secp-field-mul-2026-10-28.md).

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [Coax chapter 13, Data-oriented layout](../../coax/book/ch13-data-layout.md)

## Done means

- Limb operations match the big-integer oracle on edge values: 0, 1, `p - 1`,
  `2^256 - 1` and values at each magnitude bound.
- A `#forall` check confirms no intermediate exceeds its declared bound.
- Native and wasm builds give the same bytes.
- The chapter meets the promotion standard in the [book
  README](README.md#promoting-a-draft-to-a-teaching-chapter).

## Sources

- The primary sources are named under "What this chapter teaches" and pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
