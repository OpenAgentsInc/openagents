# Wide multiply: 2026-10-28

Coax0.16.0 adds `mul_wide(a,b)` for two `u64` operands. The exact unsigned
128-bit product is a compiler-owned `CoaxU64Product` record, fields `hi:u64`
and `lo:u64` in that order. This adds neither general128-bit arithmetic nor
tuple returns. Literals use normal u64 admission; invalid typed operands,
arity, conflicting declarations and Metal use refuse.

The parser's Coax implementation injects the ordinary record only when the
intrinsic or type is used. The Coax checker enforces operands and returns that
record. The existing C++ evaluator represents the two halves as an aggregate;
C emission uses `unsigned __int128` natively where available and four32-bit
partial products on wasm32 or the portable fallback. Both operands run once,
left to right, in compile-time, checked and wrapping profiles. The result
allocates no target heap storage and its16-byte C layout is not a wire format.

For the fallback, four products are at most(2^32−1)^2. The middle carry is at
most3*(2^32−1), which fits u64; the computed high word is bounded by the exact
product's high word≤2^64−2. The low-word shift deliberately keeps its lower64
bits in unsigned C. No helper or result declaration is emitted when unused.

## Validation

The full Python-free bootstrap passed: modified Coax parser/checker checks and
zero-survivor mutation gates, all existing bootstrap gates, seed/header equality,
final compiler fixed point and final canonical release/binary drift checks.
The parser/checker suites have162 and556 checks respectively. New frontend
checks cover normal parsing, direct schema injection, stale spare nodes,
operand hints, types, arity and missing internal schema. No equivalents were
introduced. The pre-existing unused C++ debug parameter warning remains.

Six focused tests in `coax/tests/mul_wide.py` pass with the final compiler:

- 16484 native product comparisons against independent Python integers across
 checked/wrapping emission and native128/forced portable lowering.
- 442 compile-time products, both halves checked, plus named layout/reflection.
- 20000 native ASan/UBSan comparisons with an independent C `__int128` oracle.
- 2242 actual wasm32/Bun product cases; preprocessing confirms the split path.
- Once-only left-to-right side effects, precise negative diagnostics, conflicting
 declarations, Metal refusal and no unused helper/schema.

The existing full compiler suite passed (89 fixture/example cases and every
component command). New pass/fail/example fixtures were then checked separately
in their applicable modes, and the new six-test group was registered in the
runner. Only registration changed afterward; shell syntax was checked, without
repeating unchanged compiler suites. The release generator's10 tests pass.
The demo prints `hi=1 lo=18446744073709551614` in both modes.

Byte-for-byte emitted-C comparisons against the trusted0.15.0 compiler, using
the same absolute sources, matched for:

| Source | Emitted C SHA-256 |
| --- | --- |
| crypto/bytes.coax | c3a35fd752a329869828229e4f0234a2e40d1f4f8f378cdd7ec551f9ab572475 |
| crypto/sha256.coax | 73c3fc28fbb9e86ad71e6194338ea9ef0e5e7706d27b93343a5f4fbba1a5b581 |
| nostr/json.coax | 3b62f3df5e20a8b20e9e2b8ca3b8cc33639d6e2d17dc8355360a9d4933c85077 |

These are scoped compatibility observations, not a proof for every program,
physical-device performance, constant-time lowering or a crypto adoption gate.
The current10x26 field implementation uses bounded u64 arithmetic and does not
need this primitive; a future5x52 implementation can use it explicitly.

```sh
coax/build.sh
coax/tests/run.sh
python3 coax/tests/mul_wide.py
python3 coax/tests/release.py
coax/bin/coax run coax/examples/mul-wide.coax
scripts/coax-release.sh --check --binary coax/bin/coax
```
