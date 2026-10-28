# secp256k1 field representation

Task `secp-field-repr` (#5684). This slice implements representation and basic
arithmetic in `coax/lib/crypto/secp_field.coax`. It does not implement field
multiplication/inversion, group arithmetic, signing or a complete Nostr stack.

## API and preconditions

`SecpField` contains ten little-endian `u64` digits, weighing `2^(26*i)`.
Digits 0–8 have a 26-bit mask; digit 9 has a 22-bit mask. The value is interpreted
modulo `p = 2^256 - 2^32 - 977`. Its C layout is 80 bytes, alignment 8.

Runtime arithmetic admits each digit up to **8 times its own mask**. This is a
conservative, explicit admission limit, not the largest magnitude that might fit
`u64`. A caller must maintain this invariant. Outputs are canonical (`0 <= x < p`)
and have magnitude 1. This initial slice favors simple canonical boundaries over
lazy-normalization throughput; no performance milestone is claimed.

| Entry point | Contract |
| --- | --- |
| `sf_load` | Exactly 32 big-endian bytes, reduced modulo p; wrong length refuses with zero value |
| `sf_load_strict` | Exactly 32 canonical big-endian bytes; `>=p` refuses and clears the returned value |
| `sf_store` | Normalizes an admitted field value into exactly 32 big-endian output bytes; wrong length leaves output untouched |
| `sf_normalize` | Admitted magnitude <=8 to canonical magnitude 1 |
| `sf_add`, `sf_subtract`, `sf_negate` | Normalize admitted operands, calculate modulo p, return canonical output |
| `sf_equal_mask` | Normalize both inputs; return all-one `u64` for equality, zero otherwise |

Use `sf_load_strict` when parsing a canonical field coordinate. Reducing an
arbitrary 32-byte string is not a substitute for validating a point or scalar.
These field functions impose no scalar/key validity policy.

The separate **`SecpFieldDebug`** representation is always present: value,
declared magnitude, and normalized flag (96-byte C layout). The compiler has no
conditional struct fields here. `sf_debug_valid` checks actual digit bounds;
when normalized it also requires magnitude 1 and canonical digits.
`sf_debug_normalize/add/negate` assert input validity before arithmetic and return
canonical metadata. These are explicitly selected diagnostic operations; the
plain runtime type does not automatically carry metadata or run these checks.
The diagnostic operations retain their assertions in both compiler modes.

## Carry and reduction bounds

Let `B=2^26`, `T=2^22`, `c=2^32+977`, and `m<=8`. Initially each low digit is at
most `m*(B-1)` and the top at most `m*(T-1)`.

1. A low carry is at most 7. Every initial propagated digit is at most
   `8*(B-1)+7=2^29-1`, well within `u64`. The propagated top is at most
   `8*(T-1)+7=2^25-1`, so the discarded bit-256 carry is at most 7.
2. Replace that carry `q*2^256` with `q*c`: add at most `7*977` to digit 0 and
   `7*64` to digit 1. The low accumulators are below `2^27`. The next propagated
   carry is at most 1 at every digit, including the top.
3. Fold the next top carry once more. If that carry is 1, the preceding masked
   top is zero; the final carry pass can raise it to at most 1, which cannot
   overflow `T`. If it is zero, the prior pass already normalized every digit.
   The final bit-256 carry is therefore zero; the implementation asserts this
   invariant. Every intermediate accumulator in this schedule is below `2^30`.
4. The result is below `2^256`. Adding `c` overflows bit 256 exactly when the
   value is at least `p`. Its low 256 bits then equal `x-p`. A barrier-backed mask
   selects those digits or the original. Both carry alternatives are 0 or 1.

Canonical addition has each digit below twice its mask, so it remains inside
the normalization admission bound. Negation constructs `p_i+p_i-x_i` per digit:
each sum is at least the largest possible canonical digit, so subtraction never
underflows. This represents `2p-x`, whose digits have magnitude <=2, and reduces
to `-x mod p`. No accidental unsigned wrapping is relied on in either compiler
mode. Equality ORs canonical digit XORs (at most `2^26-1`); adding that mask fits
below `2^27`, and bit 26 distinguishes zero from nonzero without subtraction.

Load/store offsets and every loop/address depend only on public byte/digit
indices. The next digit wraps modulo ten: for the last byte, its contribution
is zero (load) or entirely above bit 7 and masked away (store). Shifts are
bounded 2–26, and the largest packing intermediate is below `2^52`.

Runtime loops are fixed length. Source branches are public length checks and
invariant guards that cannot fail for admitted inputs. Debug validation can
branch on digit contents and is diagnostic instrumentation, not a secret entry
point. Checked integer guards and the final carry assertion remain relevant in
generated C. These bounds do **not** establish target constant time: independent
arithmetic review, secret-path enforcement, optimized object/JIT inspection and
leakage probes are separate gates in the Nostr plan. No secure-erasure or
cross-browser/iOS-device security assurance is inferred from this slice.

## Focused verification

```sh
coax/bin/coax test coax/lib/crypto/secp_field.coax --steps 1000000000
coax/bin/coax mutate coax/lib/crypto/secp_field.coax
COAX_FIELD_REPORT=/tmp/coax-field.json python3 coax/tests/nostr/secp_field.py
```

The Coax checks include zero, one, `p-1`, `p`, `2^256-1 mod p`, every bit position,
every admitted magnitude, exact limb-bound violations and output refusal. The
loaded byte-helper checks remain in the mutation closure. No source-owned
mutator filter, equivalent-mutant exemption or compiler change was used.

The explicit test driver uses the existing independent Python bigint oracle in
`tools/coax-nostr/fuzz/oracle.py`. Libsecp's public API does not expose its field
internals, so no libsecp-field comparison is claimed. The retained report at
`coax/tests/nostr/data/secp-field-2026-10-28.json` identifies actual compiler,
sources, generated C and Wasm artifacts. Both checked and release C profiles
run at native `-O2`; actual wasm32 executes in V8/WASI with independent expected
bytes. The driver also compiles an ASan/UBSan native probe and modifies actual
emitted reduction `977` to `976` to demonstrate a detected fault.

The negative processes exceed a real digit bound at magnitude 8 for each of ten
digits, in both compiler modes. They do not claim magnitude 9 necessarily
overflows a machine word. No compiler bootstrap, unrelated app tests, iOS build,
network oracle fetch or one-hour milestone run is part of this focused slice.
