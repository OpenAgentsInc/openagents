# secp256k1 field multiplication and square

Task `secp-field-mul` (#51cb) extends the representation slice shipped in
`44e24c812e`. `sf_multiply` and `sf_square` in `coax/lib/crypto/secp_field.coax`
accept the same magnitude <=8 digit precondition and return canonical values.
Square uses the multiplication kernel with identical operands. There is no
variable-time path, native-wide-integer language dependency, group operation,
signing implementation or production-key use in this slice.

This is a checked interval certificate for this exact carry/fold schedule, plus
independent correctness tests. It is not a formal proof of the compiler or a
constant-time/security approval. The explicit diagnostic representation and
runtime preconditions remain distinct as described in the representation note.

## Schedule and accumulator bounds

Write `B=2^26`, `T=2^22`, `S=2^256`, `c=2^32+977` and `p=S-c`.
Normalize both admitted operands before multiplying. The existing normalization
schedule admits magnitude <=8; its initial lower/top propagation bounds are
`m*B-1` and `m*T-1`. Its first folds add at most `(m-1)*977` and `(m-1)*64`.
The certificate checks these preconditions as well as the product schedule.

1. Accumulate the 100 ordinary `u64` digit products into a 20-digit array.
   Each product is at most `(B-1)^2 = 4503599493152769`. A coefficient has at
   most ten terms: `A=45035994931527690`. An inductive carry budget is
   `C=10*B=671088640`. `A+C=45035995602616330 < 2^56`, and
   `(A+C)>>26 = 671088630 <= C`. Thus every product, accumulated sum and carry
   fits `u64` in checked and release modes without accidental wrapping.
2. Carry the array into radix B. The last coefficient receives only the carry
   from digit 18, whose own product is bounded by `(T-1)^2`. The conservative
   top bound before that carry is `(T-1)^2+C=17592848744449`. All twenty output
   digits fit 26 bits. Both carry passes use the same independently checked
   helper; it checks public span length and retains the final digit's full carry.
3. Split at bit 256. The low ten digits use 22 top bits; high digit i is
   `(product[i+9]>>22) | ((product[i+10]&MASK22)<<4)`. Its first nine digits
   are <=`B-1`; the conservative final high-digit bound is 4194463 (<`2^23`,
   and also <`B`). Retain `low + high*c` in twelve digits by adding
   `high[i]*977` at i and `high[i]*64` at i+1.
4. Every first-fold coefficient is at most
   `(1+977+64)*(B-1)=69927435246`. The inductive carry budget 2048 is safe:
   `(69927435246+2048)>>26=1042 <= 2048`; the accumulated word remains below
   `2^37`. The final digit's more precise numerator is
   `4194463*64+2048=268447680`, giving a top carry <=4. Carry all twelve digits
   using that helper before splitting again.
5. The second high part has just two digits: q0 <=`B-1` and
   q1 <=`15+4*16=79`. Its second-fold digit 0 is at most 65632468014,
   giving carry 977. Digit 1 including that carry is at most 4362154255,
   giving carry 65. Digit 2 including its carry is at most 67113984, giving
   carry <=1. All following carries, including bit 256, are therefore <=1.
6. Fold that last carry with c once. The first two digits are bounded by
   `(B-1)+977=67109840` and `(B-1)+64=67108927`, both within magnitude 2.
   The existing magnitude <=8 normalization routine finishes canonical
   reduction. No field result or partial wire output is released before it.

Loop counts and digit addresses depend on public indices. Public-length
refusals and checked integer traps cannot occur for admitted inputs under these
bounds. No radix validation branch depends on secret digit contents. This is
not a claim that arbitrary input digits are admitted, or that a target's
optimized branches/JIT have been independently audited for secret use.

## Executable certificate

`SF_MUL_BOUNDS` is produced by `#run sf_mul_bounds(8)` and a top-level `#assert`
requires its budget admission before C emission. Every arithmetic bound uses
overflow-refusing `sf_bound_product/sum`. The certificate records the following
facts; `SF_MUL_LIMITS` gives inclusive declared budgets. Slots 24 and 25 must also
be <= the selected inductive carry budgets in slots 6 and 11.

| Slots | Facts |
| --- | --- |
| 0–3 | Raw normalization low/top propagation and first two folded low digits |
| 4–8 | Product, ten-term coefficient, carry budget, coefficient+carry, top coefficient+carry |
| 9–14 | Final high digit, first-fold coefficient, selected carry, coefficient+carry, top carry, second high digit |
| 15–20 | Second-fold digit 0, carry 0, digit 1+carry, carry 1, digit 2+carry, final carry |
| 21–23 | Last folded low two digits and admitted magnitude |
| 24–26 | Observed convolution carry bound, observed first-fold carry bound, first-fold top numerator |

The checks pin every computed fact independently, exercise every limit exactly
and one past it, and test both inductive comparisons. This distinguishes the
measured schedule from its reusable admission policy. Negative compilation
fixtures attempt `2^32 * 2^32` (which cannot fit `u64`) and a certificate whose
accumulator exceeds the declared budget. Both refuse in both compiler modes.
No conclusion depends on assuming magnitude 9 necessarily overflows.

## Focused verification and budgets

```sh
coax/bin/coax test coax/lib/crypto/secp_field.coax
coax/bin/coax mutate coax/lib/crypto/secp_field.coax
python3 coax/tests/nostr/secp_field.py
COAX_FIELD_MUL_REPORT=/tmp/field-mul.json python3 coax/tests/nostr/secp_field_mul.py
```

The source checks pass at the default 20-million-step budget per check; the
largest observed check uses about 9.2 million steps. All **25 checks pass** and
**557/557 reachable mutants are killed**, including the loaded byte library.
There are no equivalent-mutant exemptions or ownership filters.

The product driver defaults to a hard maximum of **1,000,000 random products
per compiler mode**, plus a square every 256 cases. Input records are 64 bytes;
output records are 32 bytes. Inputs and results stream through temporary files,
with streaming SHA-256 fingerprints and a 160 MiB total fixture-file budget.
Each native driver has a 120-second wall limit. Python holds one comparison at
a time; it does not retain a million integers or expected-result objects.
`COAX_FIELD_MUL_COUNT` may select a smaller development run, which the report
records explicitly and which does not satisfy the million-product gate.

The retained report `coax/tests/nostr/data/secp-field-mul-2026-10-28.json` records
the full run, pinned independent bigint oracle, seed, compiler/toolchain,
source/artifact/input/output hashes and actual counts. Both native C profiles
run at `-O2`; actual wasm32 executes product, square and raw magnitude fixtures
in V8/WASI for both profiles. Raw edges include magnitude 8 and the rare final
carry: `(-1)*(-(2^32+977))` has a second fold equal to `2^256`.

An ASan/UBSan emitted program exercises 8192 raw magnitude-8 cases. Separate
actual emitted kernels lose a product carry and corrupt the final 977 fold;
the independent oracle detects both. The representation suite also passes on
the expanded module. Libsecp's public API does not expose its field internals;
no libsecp-field comparison or one-hour fuzz milestone is claimed. Coax 0.15.0
was kept stable through this run; an unrelated compiler update was not used as
a reason to rerun it. This supplies no iOS-device, cross-browser timing,
secret-erasure or independent arithmetic/security approval.
