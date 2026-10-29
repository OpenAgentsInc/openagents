# Affine and Jacobian group operations

Task `secp-group` (#8ee9). `coax/lib/crypto/secp_group.coax` adds point
representations, validation, add/double/negate/mixed operations, public lifts and
compressed encoding, and a full-scan table selector. It loads the existing field
module; it does not load/change scalar arithmetic or the compiler. No scalar
multiplication, signature, key-custody or production-provider claim is added.

## Representations and admission

`SgPoint` is Jacobian `(X,Y,Z)` with affine `x=X/Z²,y=Y/Z³` and curve equation
`Y²=X³+7Z⁶`. Its canonical infinity is **(1,1,0)**. `SgAffine` contains x, y and
an all-zero/all-one `u64` infinity mask; affine infinity is `(0,0,MAX64)`.
Other infinity masks or Jacobian zero-Z coordinate pairs are invalid. Physical
field aliases modulo p are allowed under the existing magnitude contract and
are normalized on output. The all-zero default point is invalid, not infinity.

All field-valued inputs, including unselected table entries and validation
arguments, must satisfy the field module's **magnitude <=8** precondition.
Malformed curve values within that representation are handled as refusals;
arbitrary out-of-bound limbs are programmer errors requiring the diagnostic
field wrappers. Public byte constructors admit bounded field values directly.
The host still owns physical pointer/view capacity at the C/Wasm boundary.

Point results contain a canonical value and `ok`. Refusal returns canonical
Jacobian infinity with `ok=false`. Affine refusal returns canonical affine
infinity. Accepted infinity has `ok=true`, so callers must inspect the flag.
The explicit C layouts are 240-byte points and 168-byte affine points; these
struct memory images are not wire formats.

| API | Behavior |
| --- | --- |
| `sg_generator`, `sg_infinity` | Canonical G and infinity values |
| `sg_valid_mask`, `sg_equal_mask` | All-one/zero validation or equality masks; equality rejects invalid operands |
| `sg_add`, `sg_double`, `sg_negate` | Validate bounded inputs, execute fixed-work arithmetic and return canonical point/status |
| `sg_from_affine`, `sg_to_affine` | Checked conversions; the inverse-of-zero value handles infinity without an early exit |
| `sg_mixed_add` | Convert the affine operand and share the complete add kernel; no specialized mixed shortcut yet |
| `sg_pick` | Scan every entry, select with masks, validate the selected point, and return canonical point/status |
| `sg_lift_x_public` | Exactly 32 canonical x bytes to the even-y finite point; non-residue/noncanonical x refuses |
| `sg_decode_public` | Compressed 33-byte prefix 2/3 or one-byte infinity 00 |
| `sg_encode_public` | Fixed 33-byte container, length 33/1 and status; invalid input has length 0 and all-zero bytes |

The one-byte infinity format is for general group tooling. **Nostr public keys
must be finite**; the future NIP adapter must apply that identity policy. The
x-only lift already returns a finite point on success. Uncompressed 65-byte SEC
encoding is not a supported public wire input in this slice.

## Complete exceptional handling

The arithmetic follows the a=0 Jacobian `add-2007-bl` and `dbl-2009-l` formulas
in the [Explicit-Formulas Database](https://www.hyperelliptic.org/EFD/g1p/auto-shortw-jacobian-0.html).
The generic addition formulas alone are incomplete. Our kernel evaluates both
generic addition and doubling unconditionally, then handles exceptional inputs
with masks. For the scaled coordinates, `H=U2-U1` and equality of `S1,S2`
determine the same-point case. The final infinity is canonicalized by its Z mask.

| Inputs | Selected result |
| --- | --- |
| Finite, H nonzero | Generic addition, including equal-y/r=0 cases |
| Finite, H zero and S1=S2 | Doubling of the first point |
| Finite, H zero and S1!=S2 | Generic result has Z=0, canonicalized to infinity |
| First infinity | Second point |
| Second infinity | First point |
| Both infinity | Canonical infinity |
| Either invalid bounded operand | Full arithmetic still executes; checked result refuses with infinity |

There is no field division in add/double. The discarded generic same-point
candidate may be zero, but remains defined field arithmetic before selection.
Canonical infinity also makes projective cross-product equality work for both
finite points and infinity. Public equality additionally masks both operand
validations; noncanonical infinity sentinels cannot compare as valid points.

Each field expression uses the already bounded field add/subtract/multiply/
square routines. Their canonical outputs satisfy magnitude 1; selection masks
are always zero or all-one and preserve complete field values. No new widened
or overflowing field accumulator is introduced. The only new non-field secret
operations are XOR/OR/bounded shifts for word equality, and masked selection.
The complete 64-bit index is tested; it is not truncated to 32 bits.

## Public boundaries and table contract

Only public format lengths and prefixes select decoder branches. A 32-byte x
is reduced for arithmetic and independently compared with its canonical
serialization, so `x>=p` cannot become an accepted reduced key. The root is
verified and combined with that canonicality mask. Odd parity on a refused
lift still returns canonical infinity, not a modified sentinel.

Table length is public and must be 1–256. Every entry is read using the public
loop index; the secret requested index participates only in fixed-work word
equality and masks. Missing indices (including high-bit indices) refuse after
the whole scan. Only the selected entry is curve-validated: an unselected
malformed point does not poison a valid selection. All entries retain the
field magnitude precondition. Empty/oversized views refuse before any read.

## Focused verification

```sh
coax/bin/coax test coax/lib/crypto/secp_group.coax
coax/bin/coax mutate coax/lib/crypto/secp_group.coax
COAX_GROUP_REPORT=/tmp/group.json python3 coax/tests/nostr/secp_group.py
```

The Coax checks include G, independent 2G/3G coordinates, scaled equal points,
field aliases, infinity/equal/opposite inputs, noncanonical sentinels, every
index bit, exact public table capacity, invalid selected records, public key
parities, canonical x limits and refusal bytes. They run under the default
20-million-step budget per check and the existing aggregate 100k compile-time
budget. The largest observed check uses about 11.8 million steps. Coax 0.15.0
remains fixed for this run; no toolchain update triggered a repeat.
All **47 checks pass** and **847/847 reachable mutants are killed**, including
the loaded field/byte closure. No ownership filter or equivalent exemption was
used. Unneeded infinity equality gates were removed; missing parity/refusal
cases were added to the checks instead of suppressing survivors.

The native driver uses 1024 random pairs plus six special seeds in both C
profiles, with independent random nonzero Jacobian scales. Its 12360 API
comparisons cover add/double/mixed/negation/equality. Independent affine math
and 1030 pinned public libsecp-combine cases agree, including the uncommon
H!=0/equal-y case constructed with a nontrivial cube root of unity. Native
round trips contribute 678 comparisons and test every one of 256 prefixes per
mode. Actual wasm32/V8 runs 170 operation/decoder/table/encoder fixtures per
mode and independently projects Jacobian outputs using BigInt arithmetic.

An ASan/UBSan emitted program covers 128 repeated exceptional cases and invalid
public spans. Three actual altered kernels omit the equal-point alternative,
omit first-infinity handling, or mis-handle opposite points; all are detected.
The bounded fixture directory is below 8 MiB. The retained report at
`coax/tests/nostr/data/secp-group-2026-10-28.json` pins source, compiler,
generated C/native/Wasm artifacts and the existing inspected libsecp oracle.
There are no downloads or oracle rebuilds in the tests or product build.

A scoped structural check rejects explicit C conditionals in the direct
add/double/equality/curve kernels and verifies that the table element load uses
the public loop variable. It detects an inserted conditional in the actual
emitted function. This is not general taint analysis, an audit of every callee,
or proof of machine/JIT behavior. The source does not branch/index on secret
exception flags, and independent target timing/codegen/erasure/security review
remains a separate gate. No iOS-device, scalar-multiply, one-hour fuzz or
production cryptographic approval is inferred from this slice.
