# RB-03a — shared running observation statistics

Source base: `a1757c76c7`. This delivers `running-statistics-f64-v1`, a bounded
CPU statistics/normalization component. It does not finish RB-03's transition
adapter, collector, frozen actor/normalizer lifecycle, counters or resume.
Chapter 259 is therefore only partly implemented.

The [contract](../../psionic/docs/robotics/running-statistics-rb03.md) freezes
all ranges, population variance, epsilon, empty-state behavior and input order.
Its independent two-pass Fraction/70-digit Decimal oracle was written and
frozen before implementation. It does not use the incremental recurrence to
compute expected moments. Generated vectors are checked against every frozen
reference field with a predeclared `2e-13 + 2e-13*abs(reference)` budget.

## Observed gates

```shell
coax/remote/remote.sh run -- bash psionic/check.sh --only running-stats
```

- Shared Welford arithmetic: **one check passed** in debug/release;
  **11/11 root mutants killed**, no survivors or equivalents.
- Running statistics: **nine checks passed** in debug/release, including
  retained primitive checks; **108/108 root mutants killed**, no survivors
  or equivalents. Tests cover 64 channels/2,048 rows, exact input endpoints,
  one billion samples, row/count exhaustion, empty/one-sample transforms,
  bit-identical batch/stream updates and complete late-refusal preservation.
- Independent Fraction/Decimal reference matched. Actual native debug and
  release each passed plain and **ASan/UBSan**, on `oa-build-1`. Five IEEE
  poison classes exercise all 128 active/inactive snapshot float fields,
  all nine input-row positions and three transform positions. Every refused
  query preserves every output field; value-buffer and snapshot aliasing pass.
- The public active/staged example's exact transcript passed both evaluator
  modes. These are caller-owned snapshots, not enforced rollout/epoch fencing.

The first mutation round found six test gaps. One apparent redundancy proved
to be a real F64 boundary: an admitted snapshot with count 999,990,000,
mean −1,000,000 and M2 3.99996e21, updated with +1,000,000, rounds to
3.9999600040000005e21. Its next declared ceiling is 3.999960004e21. Final
admission correctly refuses without incrementing published state. That exact
case now kills removal of the candidate check; no equivalent was invented.

Other added observations cover both raw finite words, exact maximum row shape,
single-channel counter exhaustion and one-sample/endcap normalization. Harness
bring-up corrected an intrinsic-name collision. A broad generic dispatcher
attempt stopped at unrelated Unicode-17 Python provisioning; the targeted
route now invokes the existing Coax qualifier before that unrelated setup.
Those failed attempts are not qualification results.

## Shared owners and carried regressions

`stats-moments.coax` extracts the original comparator's exact Welford operation
order. The comparator still divides M2 by `n-1`; observation normalization
divides by `n`. Its complete debug/release suite passes **eight checks**,
including 420 independent integer-moment cases, and **116/116 root mutants**
are killed. No benchmark winning/timing result was repeated or broadened.

`coax/lib/ieee.coax` extracts finite-word admission. Its exhaustive exponent/
sign check covers 4,096 cases, each with three fraction patterns; **4/4 root
mutants are killed**. Existing function-layout, Gaussian and oscillator names
delegate to it. Their changed-procedure mutation commands report **zero sites**
in those pure forwarding wrappers, rather than a fictitious killed count.
Their named raw-finite/profile checks pass separately. These targeted runs
do not claim full authority coverage or new function/physics/device profiles.

No stored snapshot format, authenticated history, collector, neural F32
integration or learning result is supplied by this component. The next slice
must consume these snapshots in the owned oscillator collector, preserve
final-before-reset values, compose existing GAE and enforce version/lifetime
and counter allocation before comparing uninterrupted/resumed collection.
