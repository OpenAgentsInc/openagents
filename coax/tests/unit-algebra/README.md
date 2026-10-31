# Exact unit metadata algebra

The separately qualified producer is
[`unit-algebra.coax`](../../lib/compiler/unit-algebra.coax). It loads the
unchanged first quantity policy and reuses `Q_Unit` and `Q_Status`.
It does not enable new source operators or change type identity.

| API | Successful metadata |
| --- | --- |
| `UA_normalize(u)` | Original dimensions, positive coprime scale |
| `UA_product(a,b)` | Added dimensions, exact multiplied scale |
| `UA_quotient(a,b)` | Subtracted dimensions, exact divided scale |
| `UA_ratio(source,target)` | Equal original dimensions required; dimensionless source/target scale |

Each returns `UA_Result { status, unit }`. Inputs must have all four exponents
in **−8..8** and numerator/denominator in **1..1,048,576**. Invalid input yields
`Q_Status.metadata` before arithmetic. Otherwise, out-of-range result dimensions
yield `dimension`, followed by `scale` if the reduced numerator or denominator
exceeds 1,048,576. Refusal returns the all-zero, invalid `Q_Unit`; it never
publishes a partial dimension or scale. Only `ok` carries an admitted unit.

The product of two admitted scale fields is at most 2^40, which fits signed
64-bit arithmetic. Reduction uses positive-integer Euclid: every nonzero
remainder is smaller than its preceding divisor. There is no artificial GCD
iteration limit, floating-point approximation, truncation or saturation.
Reduction happens before the final scale bound, so large common factors cancel.

For example, force metadata `(mass=1,length=1,time=-2,angle=0,scale=1)` times
half-metre metadata produces `(1,2,-2,0,1/2)`. The ratio of centimetre metadata
to metre metadata is exactly the dimensionless metadata `1/100`. These are
metadata calculations; they do not perform value conversion.

The frozen M1 conversion policy accepts only binary scale ratios and can
represent a transient coefficient of 2^40. This producer accepts arbitrary
exact positive rational metadata, but a reduced metadata scale of 2^40 exceeds
the existing `Q_Unit` bound and is refused. The two APIs have different output
contracts. Operations form a partial algebra: a refused intermediate cannot be
used just because a larger expression could eventually cancel it.

Canonical physical metadata does not merge nominal unit declarations or
quantity families. Future compiler integration must define that identity and
derived-unit interning explicitly. Quantity products/quotients, coordinate
frames, affine points, operator lowering, device ABI and the full R12 consumer
gate remain separate work. This does not establish compiler `#bounded`, a wall
deadline or whole-compiler correctness.

## Qualification

Write checks before product code; the module retains nine authored check
families. Its changed-root mutation gate kills **69/69** mutants. The first
mutation attempt exposed three missing final-scale boundary observations;
accepted denominator 2^20 and irreducible products `17*61681 = 2^20+1` close
those cases. No survivor was waived.

The independent standard-library Python oracle uses arbitrary-precision
`fractions.Fraction`, rather than calling Coax or importing its arithmetic.
It generates **114,192** cases with fixed seed `0x55414c4745425241`: exhaustive
small rational pairs, each dimension boundary, both operand positions for bad
metadata, final scale limits, cancellation, long Euclidean inputs, failure
precedence and fixed random/cancellation groups. Exact record counts and native
byte lengths are asserted before comparison.

The native C harness transports inputs, calls exported Coax functions and
serializes all seven result words. It contains no second unit implementation.
Debug, optimized release and ASan/UBSan each match all **799,344** independently
expected words exactly, including every refused payload. Integer transcripts
are identical across these three modes on the recorded Darwin arm64 host.
This is evidence for the named host/profile, not a cross-architecture claim.
The sanitizer build uses `-fno-sanitize-recover=all`, and native runs require
empty stderr. Adding this stricter harness gate reran only the sanitizer phase
against the existing generated C, inputs and Fraction answers; the unchanged
debug/release, oracle and mutation evidence is explicitly reused.

[`qualification.json`](qualification.json) binds the final source, oracle,
harness, compiler and output digests. Large generated inputs and transcripts
stay outside the repository.

```shell
coax/bin/coax test coax/lib/compiler/unit-algebra.coax
coax/bin/coax mutate coax/lib/compiler/unit-algebra.coax --root-only --jobs 8
python3 coax/tests/unit_algebra.py
```

The native tool accepts `COAX_TEST_COMPILER` and
`COAX_UNIT_ALGEBRA_RECORD_DIR`; neither Python nor this harness is needed by
product code. Reuse the existing passing record when its sources are unchanged.
