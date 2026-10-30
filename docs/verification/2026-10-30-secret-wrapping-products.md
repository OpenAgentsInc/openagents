# Secret products without checked u64 multiplication

Part of #4828. Follows [profile v3](2026-10-30-secret-audit-ranges.md).

## The defect

A checked Coax `u64` multiply emits
`r = a * b; if (a != 0 && r / a != b) trap`. Even when overflow is
impossible, that check branches on whether `a` is zero and divides by `a`.
Division takes data-dependent time. Applied to secret limbs or bits, it makes
the checked native and WASI builds non-constant-time. Release builds already
emit a wrapping multiply and are not affected.

The large audit profile refuses every secret checked `u64 *`. On the ECDH
path that refusal first appeared in `cb_mask`, then in every limb product.

## Changes

Each changed product provably fits in 64 bits, so `mul_wrap` returns the same
value as `*`, without the check:

- **`bytes.coax`, `cb_mask`:** `(bit & 1) * CB_MAX64` becomes
  `mul_wrap(bit & 1, CB_MAX64)`. The operand is 0 or 1.
- **`ecdh_fast.coax`, `secp_field32.coax`, `secp_field32_total.coax`,
  `secp_scalar.coax`:**
  - Schoolbook limb products: 32-bit × 32-bit plus two 32-bit terms is at
    most 2⁶⁴ − 1. This is the `schoolbook_wide_and_carry` lemma in the
    field-guard proof models.
  - Folds by 977.
  - Montgomery `factor * N[j]` and `t[i] * N0_INV`: both operands are below
    2³².
- **Open-coded masks:** `value_barrier((c & 1) * CB_MAX64)` in
  `ecdh_fast.coax` and `secp_field32_total.coax` becomes `cb_mask(c)`, which
  is identical by definition.
- **`ecdh.coax`:** the result code `(valid ^ 1) * EC_SCALAR` becomes
  `mul_wrap(valid ^ 1, EC_SCALAR)`. The operand is 0 or 1.
- **`secp_group32.coax`, `sg32_from_field`:** each limb is masked to 26 bits
  before the 8×32 conversion, so the 32-bit pieces fit by construction.
  Before, a checked `cast(u32)` trapped on any limb above 26 bits, which was
  a branch on secret limb bits. Every caller passes normalized field values
  (table picks and field results), and the old code was already wrong for
  any other input.

The large profile now gives `mul_wrap` and `add_wrap` the exact range whenever
the operand bounds prove they cannot wrap. Graph checks cover both cases:
`mul_wrap(x>>32,x>>32)` fits and `mul_wrap(x>>31,x>>32)` can wrap;
`add_wrap(x>>63,x>>63)` fits and `add_wrap(x>>0,x>>63)` can wrap.

## Checks

On compiler `coax0 0.23.0` SHA-256
`03498a6cf5a3df42c9ea7ed734efd89de6a36dbd1971eae5856d13c7d476ab93`:

| File | Coax checks | Mutation (killed / total) |
| --- | --- | --- |
| `bytes.coax` | 13/13 | 145/145 |
| `ecdh.coax` | 110/110 | 1,825/1,825 |
| `ecdh_fast.coax` | 74/74 | 1,404/1,404 |
| `secp_field32.coax` | 18/18 | 249/249 |
| `secp_field32_total.coax` | 25/25 | 384/384 |
| `secp_scalar.coax` | 26/26 | 394/394 |
| `secp_group32.coax` | 66/66 | 1,174/1,174 |
| large `aggregate.coax` | 210/210 | 3,268/3,268 |

All mutation runs had 0 survivors. The checks include each file's known-answer
vectors and edge cases. The large profile's public-view (8/8), large-closure,
correspondence, float/enum and contained-cast suites pass.

`schnorr_sign.coax`'s own checks stop with "compile-time evaluation exceeded
100000 steps" at `secp_field.coax:237`. They stop the same way on unchanged
`origin/main` with this compiler, so this was not caused by these changes.
The signer's vectors were not rerun through that file.

The z3 proof scripts in `tools/coax-nostr/field-guards/`, `total-field/` and
`ecdh/` pin earlier source hashes, and by design they refuse changed
sources. They were not rerun because z3 is not installed on this machine.
Their lemmas are about integer values; the products they bound are the ones
rewritten here.

## Audit result

`ecdh_stage` under the large profile, with a classified 32-byte secret view,
a public peer and a public workspace view:

- The analysis now completes the entry's dataflow: 61 procedures and 180,978
  visited nodes, within the 262,144 budget.
- No secret branch, secret index, unsupported construct or unproved trap
  remains.
- The result is `guard-obligations` (error 14): the two authored public
  guards at lines 239 and 492. Their origin mask is 0; they are public
  assertions handled by the separate guard-binding records.
- The return value depends on the secret: the call's result code reports
  whether the key is valid.

This is a source-level analysis under the stated premises. It is not target
timing, independent review of v3, or production approval. The Schnorr
signing entry itself is not yet audited.
