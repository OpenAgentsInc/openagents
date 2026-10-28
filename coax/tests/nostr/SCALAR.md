# Coax secp256k1 scalars

`coax/lib/crypto/secp_scalar.coax` implements arithmetic modulo the secp256k1
group order **n**, not the curve's field prime **p**. It loads only the shared
byte helpers and test fixtures; it needs neither `mul_wide` nor a native
cryptographic library. Product computation and admission are Coax. Python is
explicit test tooling, absent from normal compilation and target execution.

## Representation and API

`ScScalar` is a value of eight little-endian 32-bit limbs. Borrowed byte inputs
and outputs are exactly 32-byte **big-endian** integers. There are no retained
views, allocations or native handles. Arithmetic accepts any 256-bit limb
value and normalizes it modulo n. All arithmetic results are canonical.

| Procedure | Behavior |
| --- | --- |
| `sc_load(bytes)` | Requires exactly 32 bytes and integer <n; returns `ok`, `overflow` and a value; errors have zero value |
| `sc_reduce_bytes(bytes)` | Requires exactly 32 bytes; reduces modulo n and reports whether the original was ≥n |
| `sc_store(value, out)` | Requires canonical value <n and bounded output capacity ≥32; refusal preserves output |
| `sc_normalize(value)` | Reduces any 256-bit value modulo n |
| `sc_overflow(value)` | Tests raw representation ≥n |
| `sc_is_zero(value)`, `sc_equal(a,b)` | Compare raw limbs; normalize first when comparing residues of arbitrary raw inputs |
| `sc_secret_valid(value)` | Requires raw integer 1..n−1; this admission check does not approve using a production secret |
| `sc_add`, `sc_sub`, `sc_mul`, `sc_negate` | Canonical modular arithmetic |
| `sc_inverse(value)` | Returns canonical inverse and `ok`; zero residue returns zero with `ok=false` |
| `sc_window(value, offset, width)` | Reads 1–8 bits at a public position; refuses out-of-range metadata |
| `sc_evaluate(op,a,b,out)` | Mathematical byte adapter: 0 normalize, 1 add, 2 subtract, 3 multiply, 4 negate, 5 inverse |

The byte adapter requires two exact 32-byte inputs, including a dummy second
input for unary operations. It copies both inputs before output, so partially
overlapping or identical input/output views are safe. It commits only the
first 32 bytes, preserves the remaining capacity, and refuses zero inverse
without changing output. Output views use the shared 1 MiB admission bound.
Native callers still own truthful pointers/counts, lifetime and synchronization.

`sc_store` is canonical serialization; it refuses rather than silently reducing
an invalid raw value. `sc_reduce_bytes` and `sc_normalize` are the explicit
normalization paths. Zero is a valid ordinary scalar, distinct from a valid
signing or ECDH secret scalar.

## Multiplication and bounds

The implementation uses word radix B=2^32 and R=B^8. A schoolbook product uses
17 words, including an extra word for reduction carry. Each column schedules
**one** 32×32 product, **one** existing word and **one** incoming carry. For
W=B−1, the executable compile-time checks establish:

| Intermediate | Maximum |
| --- | --- |
| Product + word + carry | W²+W+W = 2^64−1 |
| Reduction's first product + low word | W²+W ≤ 2^64−1 |
| Plain addition + carry | W+W+1 = 2^33−1 |
| Propagated reduction carry + word | W+W ≤ 2^33−1 |
| Subtraction with explicit radix | B+W = 2^33−1; minimum is zero |

After a product column, shifting by 32 yields a carry ≤W, preserving the next
column's bound. The first reduction word cancels modulo B; only its outgoing
carry is needed. Propagation visits every remaining public word, even after
the carry becomes zero. No accumulator sums two full 64-bit products.
Checked emission retains arithmetic traps; the independent native checks
exercise this exact schedule rather than relying on wrapping C overflow.

REDC uses the checked identity n[0]·1435021631 = −1 (mod B). With canonical
inputs, T<n²<nR, so REDC(T)<2n: one masked subtraction is sufficient. Ordinary
256-bit normalization similarly needs one subtraction because 2^256<2n.
R² mod n converts into Montgomery form; a second Montgomery multiplication
returns the ordinary canonical product. These reasoning assumptions remain
subject to independent arithmetic review; finite checks are not a proof.

Inverse uses the public exponent n−2 with exactly 256 rounds, computing both
multiply candidates and selecting with the existing barrier-backed mask.
Arithmetic/reduction loop counts and indices depend on public positions,
not scalar values. Canonical admission/refusal intentionally reports validity.
The fixed-loop source design is **not target-level constant-time approval**:
compiler optimization, generated instructions, timing and physical targets
still require their separate gates. There is no erasure, signing, Schnorr,
group arithmetic, ECDH or production-secret approval in this module.

## Focused checks and retained evidence

```sh
coax/bin/coax test coax/lib/crypto/secp_scalar.coax
coax/bin/coax mutate coax/lib/crypto/secp_scalar.coax
python3 coax/tests/nostr/scalar.py
```

On 2026-10-28, Coax 0.15.0 passed **23 combined checks** and killed
**390/390 mutants**, without equivalent-mutant waivers. Seven focused tests
cover:

- 0, 1, n−1, n, 2^256−1 and word/carry edges; exact canonical byte encoding,
  overflow/zero admission, every window position, partial aliases and guards;
- **1,000,000 independent deterministic requests**, compared in both checked
  and optimized native builds (2,000,000 candidate comparisons): 199,000
  normalizations, 200,000 each add/subtract/multiply/negate, and 1,000 inverses;
- a deliberately removed high-product carry in actual emitted C, which the
  independent n−1 multiplication comparison detects;
- native ASan/UBSan, negative/extreme metadata, partial input/output aliases,
  and 10,000 repeated arithmetic calls through the real C ABI;
- actual wasm32-wasip1 in Node/V8/WASI, checked and optimized, with 2,030
  independent random/edge arithmetic observations per build;
- the existing differential harness's scalar JSON request/reference contract
  through a real process adapter (`scalar_candidate.py`).

`data/scalar-2026-10-28.json` pins the million-request seed/corpus, compiler,
source/adapter bytes and native artifacts. Its elapsed time measures the test
loop, including Python and FFI; it is not an engine or crypto throughput claim.
The existing oracle uses independent arbitrary-precision integer arithmetic
for scalars, since libsecp's public API does not expose arbitrary scalar
internals. No libsecp scalar comparison, one-hour fuzz milestone, Safari,
JavaScriptCore, iPhone or app integration is claimed by these checks.

The source changed only for this scalar slice. The compiler, fingerprint
kernel, protocol schemas, app build and unrelated compiler suites are untouched.
