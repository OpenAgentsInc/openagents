# Common scheduled PD: original owned cases qualify

**The new common law meets all four original-case goal sets.** Overall worst
terminal ratio is 0.8379269323 (required ≤1), with total IAE
0.1049563997 rad·s. This is owned-model implementation evidence on the original
cases. Prospective C/D assessment and SDK validation are **not run** here.
The earlier nine-pair Part-A result remains **NONE** without reclassification.

## One law, no legacy brake override

Common Kp is `[240,240,240,240,160,80,40]` J/rad². The first six Kd values are
`[16,16,16,16,10,6]` J·s/rad². Joint 7 is always overwritten by the same law:

- `e = reference_rate7 - actual_rate7`, with `s=abs(e)` in rad/s.
- At `s <= 0.1`, Kd7=4; at `s >= 0.5`, Kd7=20.
- Otherwise evaluate `4 + 16*((s-0.1)/(0.5-0.1))` in the frozen F64 order,
  with contraction disabled and the stored RN64 denominator.

The old case 3 Kd7 override is never read. Regimes 0/1/2 is determined by error
thresholds, not rounded gain. The signed diagnostic retains subtraction's
signed zero; the absolute diagnostic clears its sign bit, including −0.
Both rates must be finite before either ±20 rad/s numerical-bound check.
Every view shape is checked before indexing; scheduled-force admission then
precedes unchanged Pjp admission. Outputs publish completely or remain
unchanged, including spare entries. Public command/step wrappers snapshot both
raw q9/v9 inputs before publication.

The gain cap of 20 is not an incremental-slope cap of 20. For the underlying
real-valued interpolation, derivative force `D(e)=Kd(abs(e))*e` has slope
`Kd(s)+s*Kd'(s)`: approximately `80*s` in the declared coefficient basis, approaching
40 J·s/rad² from below. The mathematical knots have one-sided slopes. This is
not a derivative of the rounded staircase map; stored F64 knots, operation
order and rounding are normative. Nonnegative scalar damping is not a
whole-plant passivity or stability certificate under moving references,
coupling, saturation and discrete integration.

## Exact carry before one new brake

Before a brake update, 2,250 newly computed complete ordinary-case commands
match all 712 authenticated old profile-8 command bytes, and the corresponding
1,040-byte original records remain exact. All those samples use low regime,
Kd7=4. The old inputs, shared physics, references, numerical execution profile
and prior evidence remain bound. Therefore cases 0..2 carry without physics
re-simulation. A command mismatch would stop this phase; there is no fallback.

Only the brake supplies a new measured trajectory: 750 release-mode updates,
202,535 assertions. All steps succeed. Three later cached native contexts each
pass 381,267 assertions and produce byte-identical metric/assessment outputs,
with zero plant steps. The old three metric proofs are carried with only the
explicit scheduled-v1/local ID 0 remap. That ID is neither old grid profile 0
nor profile 8, and this experiment performs no nine-candidate selection.

| Original scenario | Worst terminal normalized error | Goal set |
| --- | --- | --- |
| Hold, exact carry | 0.0001732912 | Pass |
| Nominal, exact carry | 0.8280275411 | Pass |
| Pulse, exact carry | 0.8379269323 | Pass |
| New brake | 0.2424512135 | Pass |

Every joint/post-state 626..750 must satisfy both 0.01 rad position and
0.05 rad/s rate error. Hold also meets full-run drift ≤0.005 rad; nominal SSE
beats the analytic stationary-position comparator. Final brake maximum errors
are 0.00126465 rad and 0.0106499 rad/s; complete last 125-state peaks are
0.00242451 rad and 0.0106920 rad/s. The first 25-state band 363..387 is an observed
window, not a checked never-leaves suffix or permanent settling claim.
The last 125 window is the actual acceptance criterion.

There are 34 Pjp-clipped coordinates and zero Pa control/force clips. All 3,000
observed schedule tuples are independently exact: low 2,714 / ramp 253 / high 33
(the ordinary carry contributes 2,250 low samples). Reference limits do not
bound actual speeds/accelerations. Source coordinate/finite/status validity
and diagnostic peaks remain separately recorded.

## Evidence and limits

- `experiment.json`, prospective descriptor/seal and `scalar input` facts
  freeze the common vectors, operation order, original cases and prospective
  C/D boundary before new braking observations. C/D is not executed.
- `source-qualification.json`: eleven new groups and 117+116=233 actual emitted
  root sites killed; initial observational and depth-budget failures retained.
  Only new load placement was flattened; no compiler budget or old source
  changed. Counts do not close full Quantity-token mutation provenance.
- `unit-refusals.json`, four unit logs: first E0317 witnesses for wrong
  rate, damping, force and error dimensions.
- `preparation-qualification.json`: three first native host contexts each
  pass 688 ABI assertions and 62 scalar probes against the sealed independent
  table, before model execution.
- `carry-qualification.json`, `collection-lineage.json`: exact complete
  command/record carry before the single new brake, authenticated against
  original bytes. No old plant or SDK execution.
- `native-qualification.json`: the first 750-step brake and three cached
  metric contexts; raw 1,072-byte rows and all statuses/clip fields preserved.
- `independent-qualification.json`, `metric-carry.json`, scalar-reference facts:
  exact 3,000 schedule tuples and 2,250 carries; three old metric proofs reused.
  Fresh 32 brake metric coefficients differ by at most 4.22e-15, and 26,250 PD
  fields by at most 4.62e-14. Twenty-three reader poisons are refused.
- `new-brake-observations.json`: final, last 125 and observed 25-window values.
- `example-qualification.json`: first qualified scalar-only example check
  and debug/release match the prewritten golden. The initial compile-only
  bool-display mistake is preserved, with no producer/law/golden change.

Helpers and returned PODs are source-visible and unsealed, not trajectory,
goal-proof or control-authority certificates. Trusted law inputs are finite
with |error|≤40 rad/s; reference/apply/reduction helpers require their fixed
profile, admitted inputs and coherent prior state. Public APIs preserve their
stated shape and numeric admission. Existing Pjp/Pnt/Pqt/Pa/Peu/Pgs code stays
unchanged; no standalone old gate or model corpus is repeated.

## Pure example and cached gate

The example executes scalar schedule/gain fixtures only, not force,
integration, SDK or NN. Its literals are separate from measured trajectories:

```shell
coax/bin/coax run psionic/examples/panda-scheduled-pd.coax
```

The default Darwin arm64 gate consumes self-contained committed observations
and performs cached checks only, with no collection or SDK dependency:

```shell
COAX=coax/bin/coax psionic/tests/robotics/check-panda-scheduled-pd.sh /tmp/panda-scheduled-cached-new
```

The additive wrapper/adapter is syntax/plan/stub-routing checked here; its
numerical path is not rerun. The measured first evidence above carries
unchanged. It requires no historical private binaries or duplicate carry file.

This is a successful original-case owned-model experiment, not prospective
C/D validation, SDK agreement for the new brake, general nonlinear stability,
contact/collision handling, hardware safety, finite nullspace integration,
full chapter completion or compiler `#bounded`/real-time qualification.
