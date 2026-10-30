# Large source-audit profile v2: range-contained checked casts

Part of #4828 (`crypto-secret-audit-large-closure`). This is one refinement of
the separately identified large profile, not completion of the task and not
a signer or production approval.

## What changed

The [checked-cast correction](2026-10-29-secret-audit-checked-cast.md) made
every aggregate profile refuse any checked cast of a secret, including
widening casts that cannot trap. On the real ECDH and signer graphs, this
was the first refusal after type admission: `bytes.coax:108:28` widens a
secret byte, and `sc_subtract_n` widens secret `u32` limbs.

The large profile (`tools/coax-nostr/large-source-audit/`) is now
`large-aggregate-source-v2`. It admits a secret checked cast only when the
cast is an unsigned strict widening: `u8 -> u32`, `u8 -> u64` or
`u32 -> u64`. The operand's dependency mask is kept. Every other checked
cast of a secret still refuses: identity casts, narrowing, any `int` source
or target, `bool`, floats, enums and anything that is not one of the five
primitive types. The target is read from the cast's type node without side
effects (`AG_cast_primitive`). A name that is not a primitive resolves to 0,
which no rule contains. The legacy (`secret-check`) and public-view
(`public-view-check`) profiles and the shared Coax tests are unchanged, and
`test_correspondence.py` pins the exact source delta from the reviewed v2
public-view profile.

## Why it is sound for the native C target

`coax0` emits a checked unsigned cast as `coax_cast_uu((uint64_t)x, hi, line)`,
which traps when `x > hi`. For a strict unsigned widening, `hi` is the
target's maximum and every source value is at most the source's maximum,
which is smaller. The trap condition cannot hold for any value, so the
direction of the check does not depend on the secret. With the helper
inlined at `-O2`, the check disappears from the object code entirely.

`coax/tests/nostr/secret_contained_cast.py` checks this on the host target.
It compiles the fixture's checked and release C at `-O2`. Each widening
function must contain no conditional branch, conditional select, call or
trap. The narrowing control (`u64 -> u32`) must keep its branch. Every `u8`
value and ten `u32` boundary values must widen exactly. On x86-64 with
gcc 15.2.0, each widening compiles to one `movzbl` or `mov` plus `ret`. The
narrowing control keeps `shr; jne` to the trap.
[Result](data/secret-contained-cast-2026-10-30/target-native.json).

Not covered: WASI and AArch64 objects, `-O0` builds (where the helper call
and its never-taken branch remain), other C compilers and flags, and
hardware timing.

## Checks

On compiler `coax0 0.23.0` SHA-256
`03498a6cf5a3df42c9ea7ed734efd89de6a36dbd1971eae5856d13c7d476ab93` (built
from compiler sources identical to `origin/main` at the time):

- **`aggregate.coax`:** 202 of 202 Coax checks pass. The new checks cover
  every primitive pair and the target reader, including node 0, non-primitive
  nodes and out-of-range ids.
- **Mutation:** an ordinary run in 20 shards killed 2,981 of 2,981 mutants,
  with 0 survivors, on source SHA-256
  `cd2e901eb7988f96e65469a79fcf993416fa229db45dfdfe28e9158d0b4d22aa`
  ([log](data/secret-contained-cast-2026-10-30/large-mutation.log)). The
  first run left two survivors at `AG_cast_primitive`'s `id<0` boundary.
  A check that node 0 resolves as it does in `AG_type` kills both. A full
  reference-mode mutation run was not done.
- **`test_correspondence.py`:** passes, so the public-view source still
  matches its pin and the large profile differs from it by exactly the
  stated substitutions.
- **`test_large_closure.py`:** passes. It now expects
  `large-aggregate-source-v2`.
- **`test_public_views.py`:** 8 of 8 tests pass, with these
  [cases](data/secret-contained-cast-2026-10-30/large-cast-cases.json):
  - **Accepted, return mask 1:** a secret view byte widened to `u64`;
    `u8 -> u32`, `u8 -> u64` and `u32 -> u64` from a truncated secret
    `u64`.
  - **Accepted, return mask 2:** an aliased write followed by a widening.
  - **Refused as `unsupported`:** `u64 -> u32`, `u64 -> u8`, `u32 -> u8`,
    `u64 -> int`, `u8 -> int`, `u32 -> int`, identity `u64 -> u64`, and a
    widening later narrowed through a shift (scalar and view).
  - **Accepted:** a public narrowing.
- **`secret_checked_cast_types.py`:** float and enum casts still refuse in
  all three profiles.
- **Not run:** `secret_checked_cast.py`. It needs the Mac's Homebrew WASI
  toolchain. Its `checked_wide` expectation for the large profile is updated
  to `accepted`.

## Independent review

A separate read-only reviewer, not shown this record's reasoning, looked for
a false acceptance and found no concrete bypass.

- **Implicit widening:** probes that try to widen implicitly or mix widths
  are compile errors in Coax. These include `ifx` mixing `u8` and `u64`,
  `b8 | x64`, `0x100000000 | b8`, `y:u64 = b8` and `rotl(b8,3)`.
- **Retyping:** the auditor retypes values at returns, parameters,
  assignments, fields and view elements, and a literal in `AG_binary`
  takes the other operand's type.
- **Primitive names:** primitives cannot be shadowed by a struct, and type
  aliases don't compile. `uint`, `u16`, enum and `#distinct` targets
  resolve to 0 and refuse.
- **Emitted C:** the trap condition for the three admitted pairs is
  unsatisfiable in both helper variants.

The reviewer raised two dependencies, recorded here:

- **Constant-direction branch at `-O0`:** at `-O0`, and anywhere the helper
  is not inlined, the secret is still compared against a constant and a
  conditional jump remains. Its direction never changes, but the
  instructions exist. Only the `-O2` host objects above are shown
  branch-free.
- **No implicit widening in Coax:** the rule depends on Coax never widening
  an integer implicitly. `AG_union` and `AG_binary` take the first
  operand's type. If the language ever admits mixed widths (for example,
  `ifx` taking the wider branch, or literals defaulting to `u64`), this
  rule must be revisited, because it could then admit a narrowing.

## Signer path progress

The real `ecdh_stage` entry under the v2 profile no longer stops at
`bytes.coax:108:28`. It reaches 30 procedures and refuses at
`secp_scalar.coax:99:18`, the checked addition in
`SC_BASE+cast(u64)a[i]-...`. That is the secret checked-arithmetic rule from
[the arithmetic correction](2026-10-29-secret-audit-checked-arithmetic.md).
Admitting it needs interval facts about the limbs (`a[i] < 2^32`,
`SC_BASE = 2^32`), not a cast rule. This is still a refusal, and it is
neither an ECDH nor a signer audit result. The two earlier public guard
obligations (lines 239 and 492) are unchanged.

## Still open in #4828

- Secret checked arithmetic with provable bounds, starting at
  `secp_scalar.coax:99`.
- The shifted `u64 -> u32` narrowing at `secp_group32.coax:36`, which needs
  caller-specific field bounds.
- Type admission of the classified 32-byte signer input in the committed
  profile.
- Deeper types, effectful calls, public-length loops and the full 27-unit
  signer graph.
- Full reference mutation, independent review, and target timing.

Historical source-audit acceptances remain suspended, as recorded earlier.
