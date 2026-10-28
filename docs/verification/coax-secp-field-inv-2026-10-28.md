# Fixed-chain field inversion and square root

Task `secp-field-inv-sqrt` (#6076) extends field multiplication shipped in
`594f746ec1`. The implementations are in `coax/lib/crypto/secp_field.coax` and
keep its explicit magnitude <=8 runtime input precondition. Outputs are
canonical fields. No scalar/group/signing implementation or production-key use
is added by this slice.

## API and refusals

All three operations return `SecpFieldResult { value, ok }`:

| Entry point | Contract |
| --- | --- |
| `sf_inverse` | `a^(p-2)` for nonzero a; zero modulo p returns `ok=false,value=0` |
| `sf_inverse_vartime` | Explicit public-input name; currently calls the same fixed chain |
| `sf_sqrt` | Canonical **even** root when one exists; non-residue returns `ok=false,value=0`; zero succeeds with root zero |

Inputs are field values modulo p, so the zero refusal also covers a raw
representation of p. Use the strict byte decoder when parsing a canonical wire
coordinate; these functions do not validate points or scalars.

Every input takes the full public chain. Inversion tests the computed result's
zero mask only after all arithmetic; a positive exponent already maps zero to
zero, so there is no redundant clearing loop. Square root verifies `r*r=a`,
selects between r and -r with a parity mask, then masks the refused value to zero.
The result flag is an explicit API outcome, not an early exit or partial-value
release. Secret candidate values are not logged or sent to host effects.

The source uses fixed operation counts, public metadata/indices and masked
choices. This does **not** establish target-wide constant time, compiler
soundness, secure erasure or a secret-taint system. Checked arithmetic guards,
optimized objects, copies and JIT behavior remain subject to the independent
security gates in the Nostr plan. The public-input alias has no variable-time
optimization yet, and its name alone enforces no taint restriction.

## Shared addition chain and exact exponents

`SF_FIELD_CHAIN` is a typed public table of twelve `(left, squares, right)`
steps. State 0 is normalized a; each step squares its selected predecessor a
fixed number of times and multiplies by another preceding state. The states
include powers `2^2-1`, `2^3-1`, `2^6-1`, `2^9-1`, `2^11-1`, `2^22-1`,
`2^44-1`, `2^88-1`, `2^176-1`, `2^220-1`, `2^223-1`, then

`t = a^E`, where `E = (2^223-1)*2^23 + (2^22-1) = 2^246-2^22-1`.

Inversion computes `a^9=(a^3)^2*a^3`, then `a^45=(a^9)^4*a^9` and
`t^(2^10)*a^45`. Its exponent is exactly

`2^10*(2^246-2^22-1)+45 = 2^256-2^32-979 = p-2`.

Square root computes `(t^(2^6)*a^3)^(2^2)`, with exponent

`4*(64*(2^246-2^22-1)+3) = 2^254-2^30-244 = (p+1)/4`.

Because p is 3 modulo 4, this produces a square root precisely when the
subsequent square check succeeds. The public prefix costs 245 squarings and
12 multiplications. Inversion totals 258 squarings and 15 multiplications;
square root totals 253 squarings and 13 multiplications plus its one validation
square. The full chain storage is thirteen fields, 1040 explicit bytes, with
no heap allocation. This does not bound hidden compiler/ABI copies or promise
erasure of their stack storage.

The same table drives an **exact 256-bit integer exponent interpreter** using
eight little-endian `u32` words and `u64` carry intermediates. It neither reduces
the exponent modulo a small number nor relies on a fingerprint. A shifted or
added word is below `2^33`; each discarded bit-256 carry must be zero. `#run`
constructs the public prefix once and memoizes it; inverse/root certificates
reuse it. Top-level `#assert` checks the complete expected exponents:

```text
p-2:       fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2d
(p+1)/4:   3fffffffffffffffffffffffffffffffffffffffffffffffffffffffbfffff0c
```

Public-step admission rejects out-of-range dependencies or square counts.
The checks pin every intermediate prefix state and every exponent byte, test
carry propagation across all word boundaries and exercise exact public count
limits. Both the runtime and exponent interpreter read the same public steps.

## Budgets and verification

```sh
coax/bin/coax test coax/lib/crypto/secp_field.coax
coax/bin/coax mutate coax/lib/crypto/secp_field.coax
COAX_FIELD_INV_REPORT=/tmp/field-inv.json python3 coax/tests/nostr/secp_field_inv.py
```

The compiler remains Coax 0.15.0 for this slice. The word model and memoized
prefix fit the default **100,000 aggregate compile-time steps**; the limit was
not raised. All **36 source checks pass** at the default 20-million-step budget
per check; the largest observed one remains about 9.2 million steps.
**678/678 reachable mutants are killed**, including the existing multiplication
and byte-helper closure. No mutation filter or equivalent exemption is used.

The explicit native driver defaults to **100,000 nonzero seeded inputs per
compiler mode**. For each it compares the actual inverse against the pinned
independent bigint harness and verifies `a*inv(a)=1`. It also computes
`sqrt(a*a)` and checks its exact canonical even value against ±a using independent
integer arithmetic. Checked and release C both compile at `-O2`; their results
must match. A smaller `COAX_FIELD_INV_COUNT` is a development run only and is
recorded as such.

Inputs and results stream through files; each input is 32 bytes and the result
64 bytes. The driver caps 100,000 accepted records, has a 180-second wall limit
per mode, and the complete fixture directory is limited to 32 MiB. Hashing is
streamed. No production app/compiler path acquires a Python dependency.

Additional tests cover raw magnitude 8, zero aliases, non-residues and the
public-input alias. Roots match the full even y coordinate of 36 points per
mode serialized through the pinned libsecp256k1 v0.8.0 **public API**, including
G and -G. Inverse uses the independent bigint reference; libsecp does not expose
its private field-inverse API. The existing inspected oracle artifact is reused
and fingerprint-checked; tests do not download source or run build scripts.

Actual wasm32/V8 runs inverse/root fixtures and refusals in both compiler
profiles. An ASan/UBSan emitted program exercises 256 raw-field cases. Negative
compile fixtures overflow the full 256-bit exponent by shifting or adding,
and must refuse in both modes. Three independently altered emitted kernels
change the inverse exponent, break even-root selection, or release a candidate
on non-residue refusal; all three faults are detected.

`coax/tests/nostr/data/secp-field-inv-2026-10-28.json` records the full run,
compiler/toolchain, exact counts, source/input/output/artifact hashes, native
oracle identity and budgets. This is correctness evidence for this slice,
not a one-hour fuzz, iOS-device, cross-browser timing or independent security
approval. No unrelated compiler bootstrap or whole-app retest was repeated.
