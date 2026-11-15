# RB-03a — reusable observation statistics

Profile `running-statistics-f64-v1` supplies incremental statistics and a
normalization transform. It does not implement collection, episode/reset
semantics, actor identities, active/staged lifecycle, RNG allocation or PPO.
The collector will own those decisions while retaining complete snapshots.

The inventory found Welford's existing update in `psionic/lib/comparator.coax`.
Its exact arithmetic moved into `stats-moments.coax`; both comparator and
observation statistics call it. The comparator retains sample variance over
positive nanosecond timings. This component uses population variance over
bounded signed observations. Neither changes the other's convention.

The inventory also found raw IEEE finite admission in the function-layout,
Gaussian and oscillator owners. `coax/lib/ieee.coax` now owns that primitive;
those existing names delegate to it. Normalization need not import an
autodiff graph or a physics/robotics context merely to inspect an F64 word.

## State and arithmetic

`RnSnapshot` has a channel count and 64 `SmMoments` slots. Every slot holds
sample count, mean and M2. All active sample counts must match. Inactive
slots are zero. All 128 floating fields receive finite admission, including
inactive fields. Counts range from zero through one billion, means through
±1,000,000 and M2 through `[0,4e12*count]`. Empty active means and M2 are zero;
one-sample M2 is zero. These are numerical consistency checks, not proof
that an untrusted snapshot came from a genuine observation history.

For each row in declared row-major order, update each active channel with
the shared arithmetic:

```text
n_next = n + 1
delta = value - mean
mean_next = mean + delta / n_next
M2_next = M2 + delta * (value - mean_next)
```

The primitive trusts admitted finite arithmetic and an incrementable count.
The bounded observation adapter admits its whole input, updates a private
candidate and then validates every candidate field before publication.
It accepts 1–64 channels and 0–2,048 rows per call. Every observation is finite
and within ±1,000,000. A count that would exceed one billion refuses.
Zero incoming rows copy the admitted snapshot exactly. Splitting a stream
without changing row order preserves all bits. Worker-order-independent
collection must determine that order before calling this component.

For a nonempty channel, normalization is
`(value - mean) / sqrt(M2 / count + 1e-8)`. Empty channels use identity.
There is no clipping or implicit Bessel correction. Outputs are F64 and
bounded in magnitude by 2e10 under the admitted boxes. The active/staged
snapshots, versions, row-use lifetime and conversion to neural activation
precision belong to the later collector/learner integration.

Raw snapshots are in-memory values. A stored snapshot needs a registered
identity-bound companion; no Coax struct layout is a persistent schema.

## Independent references and route

The independent oracle computes exact two-pass moments with Fraction and
normalizes with 70-digit Decimal arithmetic. It was frozen before product
implementation and does not import the incremental product. Its comparison
budget is `2e-13 + 2e-13 * abs(reference)`.

```shell
coax/remote/remote.sh run -- bash psionic/check.sh --only running-stats
```

That targeted route checks the two new units, their mutation boundaries,
the independent reference, four native modes and the exact example transcript.
It avoids unrelated text/Unicode provisioning. The
[receipt](../../../docs/verification/robotics-running-stats-rb03-2026-11-15.md)
records actual outcomes and carried-component regressions. RB-03's bounded
transition adapter, GAE composition and interrupted/resumed collection remain
the next implementation slice.
