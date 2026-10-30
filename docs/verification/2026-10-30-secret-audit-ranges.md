# Large source-audit profile v3: unsigned ranges for checked operations

Part of #4828 (`crypto-secret-audit-large-closure`). This builds on
[v2](2026-10-30-secret-audit-contained-cast.md). It does not complete #4828,
and it is not a signer or production approval.

## What changed

The large profile (`tools/coax-nostr/large-source-audit/`) is now
`large-aggregate-source-v3`. Before this, every secret checked `+`, `-`,
`*` or narrowing cast refused. The signer's 32-bit limb carry chains
(`sc_subtract_n`, `sc_add`, `sf32_add`) are made of exactly these operations.

Each analyzed value can now carry a sound unsigned range `[rlo, rhi]`. A
value without one has its type's whole range: u8, u32, u64 and bool are
bounded, and int has no bound. Ranges come from three places:

- **Integer literals:** the literal's value.
- **Operations:** `+`, `-`, `*` (only where the bound can't overflow), `&`,
  `|`, `^`, and shifts by a public known amount.
- **Casts:** a cast whose source range fits the target keeps the range.

A range survives a binding, parameter or return only into the same unsigned
type, or from an int literal. Field reads, index reads, builtins (including
rotates), unary operators and truncation that may change the value all drop
it. Joins at public `ifx` choices use `lo = a.lo & b.lo` and
`hi = fill(a.hi | b.hi)`, where `fill(x)` is the smallest all-ones value at
or above `x`. Ranges never decide a branch, an index, a loop bound or a
public interval; they are used only to rule out traps.

**Admitted only when the range proves the trap can't fire:**

- **Secret checked `+` and `-` on u8, u32 and u64:** `+` requires
  `a.hi + b.hi <= max`, and `-` requires `a.lo >= b.hi`.
- **Secret checked `*` on u8 and u32**, when `a.hi * b.hi <= max`. The
  emitted helper multiplies in int64 and compares; it never divides.
- **Secret checked casts to a different unsigned type** (not bool), when the
  source range fits the target.

**Still refused:**

- **Secret checked `u64 *`, always.** Its emitted check is
  `if (a != 0 && r / a != b)`. That branches on whether the secret operand
  is zero and divides by it, and division time depends on the value, even
  when overflow is impossible.
- `/`, `%`, unary `-`, compound arithmetic assignments, identity casts,
  bool targets, and anything involving an unbounded int.

**Loops.** As before, loop analysis first removes facts from every scalar
the loop assigns. Then:

1. Assigned scalars start the first pass with the range they had on entry.
2. If a pass leaves every binding's range inside the one it assumed, that
   assumption is an inductive invariant and the pass stands.
3. Otherwise each range widens to `[lo & e.lo, fill(hi | e.hi)]` and the
   body is analyzed again.
4. After four passes without an invariant, a final pass runs with type
   ranges, which is the v2 behavior.

Every pass is a complete analysis. A narrower range only ever admits more,
so a refusal in any pass is also a refusal under the final ranges. The other
side effects of earlier passes (taint, dirty regions, guard records, memo
entries keyed by argument ranges) only over-approximate. Memo keys include
ranges, so a call analyzed under narrow argument ranges is never reused for
wider ones.

## Checks

On compiler `coax0 0.23.0` SHA-256
`03498a6cf5a3df42c9ea7ed734efd89de6a36dbd1971eae5856d13c7d476ab93`:

- **`aggregate.coax`:** 210 of 210 Coax checks pass. They include
  source-shaped graphs in `coax/tests/nostr/secret_range_cases.coax`:
  - loops that converge on the 2nd and 4th passes;
  - one that would need a 5th pass and refuses;
  - a growing sum, which refuses;
  - the carried scalar being the first binding;
  - ranged and full-range loop entries;
  - literal, binding, default-declaration, cast, truncation and
    public-`ifx` cases.
- **Mutation:** an ordinary run in 20 shards killed **3,258 of 3,258
  mutants, 0 survived** ([log](data/secret-range-audit-2026-10-30/large-mutation.log)).
  It ran on `aggregate.coax` SHA-256
  `468d7841a91e2fc140e3f28a2c986f58b546ef53071ecaee963bd38aa0c25579` and
  `secret_range_cases.coax` SHA-256
  `973d2c9faa78979a7c9210e64bf79d8b42ac41603c58878297346b469dd9d745`.
  - Earlier runs left survivors where two comparisons give the same result
    on ties (`min`/`max`), in an unreachable loop bound, and in a redundant
    whole-binding restore.
  - Those were restructured: bitwise lower bounds, a branch-free `fill`,
    and value-only restores. None were marked equivalent.
  - Fixture helpers were moved into the check bodies.
  - A full reference-mode run was not done.
- **`test_public_views.py`:** 8 of 8 tests pass, with these
  [cases](data/secret-range-audit-2026-10-30/large-range-cases.json):
  - **Accepted:** carry and borrow chains, `u32 *` within range, masked and
    shifted narrowing, and a safe subtraction.
  - **Refused:** a growing sum, `u64 *`, `u32 *` overflow, `x+1`, a possible
    underflow, and an escaping carry.
- **`test_large_closure.py`** (expects v3), **`test_correspondence.py`** and
  **`secret_checked_cast_types.py`:** pass. The correspondence check now
  applies an exact, ordered block list (`large-delta.json`) to the pinned
  public-view source. The legacy and public-view profiles are unchanged.

## Signer path progress

The real `ecdh_stage` entry now proves the scalar carry chains, including
`sc_subtract_n`'s loop-carried borrow, and reaches 33 procedures. It refuses
at `bytes.coax:179:73`.

That refusal is a **constant-time defect in checked C builds**, not only an
analysis limit. `cb_mask` computes `(bit & 1) * CB_MAX64` as a checked `u64`
multiply, and its emitted overflow check branches on the secret bit. Release
builds use a wrapping multiply and are not affected.

## Dependencies and limits

- **Inherited premises:** the v2 rule's dependency on Coax never widening
  integers implicitly applies here too. So do the trusted compiler, parser,
  bridge and stable-source premises.
- **Emitted checks still run:** the admitted operations' helpers still
  compare the secret against bounds. The analysis shows only that the trap
  branch's direction cannot depend on the secret. At `-O0` the comparisons
  and branches remain in the object code.
- **Not covered:** target timing, WASI and AArch64 objects, and independent
  review of this version.
