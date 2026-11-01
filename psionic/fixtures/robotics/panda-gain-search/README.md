# Frozen owned-plant gain search: no qualified family

**All nine profiles are valid, but none is feasible.** No winner or runner-up
was selected. No held-out SDK validation, expanded grid, changed horizon,
relaxed goal or best-available controller followed this result.

The experiment froze Kp scales `[1,2,4]` and Kd scales `[0.5,1,2]` in row-major
order. It keeps all four original hold/track/pulse/brake scenarios unchanged.
The brake's original Kd7 is 20 versus 2 in ordinary scenarios, so one scale
pair selects a **scenario-conditioned gain family**, not a universal vector.
All new dynamics use the owned source-bound Panda plant, with no SDK or NN.

| Profile | Kp × | Kd × | Worst terminal ratio | Total IAE (rad·s) | Pjp-clipped coordinates | Feasible |
| --- | --- | --- | --- | --- | --- | --- |
| 0 | 1 | 0.5 | 3.879737 | 0.266187 | 14 | No |
| 1 (carried) | 1 | 1 | 5.157346 | 0.229669 | 31 | No |
| 2 | 1 | 2 | 6.202255 | 0.198056 | 41 | No |
| 3 | 2 | 0.5 | 1.843086 | 0.206789 | 16 | No |
| 4 | 2 | 1 | 2.854085 | 0.190134 | 32 | No |
| 5 | 2 | 2 | 4.643709 | 0.173539 | 42 | No |
| 6 | 4 | 0.5 | 1.730989 | 0.152021 | 18 | No |
| 7 | 4 | 1 | 1.355279 | 0.143858 | 35 | No |
| 8 | 4 | 2 | 2.587362 | 0.139504 | 43 | No |

Displayed values are rounded; exact cached scorecards determine policy.
A terminal ratio at most one is required. Profile 7's diagnostic minimum
1.35528 still misses and is **not selected**. Profiles 5/8 meet moving-case
bands but miss braking; profiles 6/7 meet braking but miss moving cases.
Every profile meets the hold-drift and nominal-SSE improvement goals. No Pa
control or force clipping was observed in any profile. This
tradeoff describes this fixed grid/horizon, not all possible controllers.

## New work and carried work

The canonical release/O2 grid executes only the eight new gain pairs:
32 rollouts and exactly 24,000 new owned steps. Four historical profile-1
fresh-feedback trajectories are reused by exact byte, original source,
reference, gain and numerical-profile identities. Their recorded-command
shadow is not a new baseline. `pgs_step(1,...)` refuses stage7/code2 before
model arithmetic. Profile-1 gain metadata and scoring remain allowed.

`baseline-carry.json` binds the original qualified full trace SHA256
`2f4c1631b80479238761535526ab430232252e801fd778b4c180a8c2c443ffc7`.
The compact cache extracts post-state bytes and integer metadata, preserving
exact previous-state lineage. No original trajectory, oracle or unchanged
primitive gate is rerun. Existing pnt/Pjp/Pa/Peu/Pqt sources stay unchanged.

## Objective and admission

All four cases must satisfy both position error ≤0.01 rad and rate error
≤0.05 rad/s at **every state626..750**. Hold also requires full-run drift
≤0.005 rad. Only nominal tracking must beat its analytic stationary-position
SSE; equal sample counts give the same strict RMS ordering. A temporary pulse
window does not meet the stronger terminal-band requirement.

Coax derives reference `k+1` for post-state `k+1`, checks all 27 input floating
words before arithmetic, exact previous-rate lineage, numerical state boxes
and all nine source joint ranges, including the fingers. Shape/index API
refusals preserve caller output. Faulted/partial traces instead publish a
finite invalid scorecard retaining only their admitted prefix, with explicit
reason/goal flags; no infinity sentinel hides missing evidence.

Each tick sums seven absolute position errors, multiplies that row by 0.002 s
and adds it to IAE. SSE is tick-major/joint-major. Combined cards reduce cases
0..3. Feasible candidates compare exactly by worst terminal normalized error,
then total IAE, Pjp-clipped coordinates, then fixed profile ID. No epsilon is
used. No feasible candidate yields winner=runner=-1, feasible_count=0 and
canonical zero gaps. With a runner, gap fields are runner minus winner,
including signed later-key gaps.

`position_square` and `stationary_square` have rad² coefficients, `iae` rad·s,
`terminal` is dimensionless and `hold_peak` radians. Diagnostic arrays retain
explicit raw-coordinate conventions: seven angular and two linear speeds,
effective accelerations `(post_v-previous_v)/0.002`, and post-state source
margins. Reference kinematic limits are not actual-motion limits.

The helpers and all case/candidate PODs are source-visible and **unsealed**.
Numeric/order domain checks are not trajectory or goal-proof certificates.
The reducer requires admitted finite bounded samples and coherent prior
metrics; the reference helper assumes the unchanged fixed case/edge profile.
The cache manifest binds actual profile/case/tick and source provenance.

## Evidence

- `experiment.json`, `profiles.f64`, `heldout-unscaled.f64`, `input-freeze.json`:
  original nine-pair, four-scenario and two-held-out-start freeze.
- `source-qualification.json`: 25 new groups and 578 actual emitted root sites
  killed (73 control, 219 scorer, 286 selector), preserving initial compile and
  observational failures. These are actual sites, not full Quantity-token
  provenance or a compiler `#bounded` claim.
- `preparation-qualification.json`: first debug/release/fail-fast sanitizer
  scorer guards, 1,338 assertions each, with score/combine/select overlaps;
  all 36 resulting gain vectors match the independent frozen input bits.
  `header-failure-v1/` retains the first header-basename build failure and
  its tooling-only followup; successful C/header/object work was reused.
- `native-qualification.json`, `lineage.json`, `grid-progress.jsonl`:
  all 32 new rollouts, 24,000 steps once, zero new baseline steps. All 36 case
  cards are valid/complete. Three cached scorer/selector profiles each pass
  515,393 assertions and produce byte-identical cards/selection, with zero
  plant calls during scoring.
- `independent-objective.json`, `scorecards.json`: independent direct-array
  reductions check 1,152 case metric coefficients (max error 5.77e-14,
  0.9324% of the frozen 2e-12 absolute +2e-12 relative budget), all counters,
  goal flags and exact none-selection. Fourteen cache poisons, fourteen
  score-reader poisons and six lexicographic policy witnesses pass.
- `example-qualification.json`: first pure metadata/policy example executions
  match an independently prewritten golden; no rollout is executed.
- `cached-gate-routing-qualification.json`: syntax/plan/stub routing of the
  additive cached gate. That new adapter's numerical path is not repeated
  here; underlying measurements above are carried unchanged.

The public case/candidate/selection binaries, cache and independent binaries
are numerical observations and expected tables. They are not new model
weights, original vendor code, control authority or hardware certificates.

## Run without SDK or fresh physics

The pure Coax example shows metadata, carry refusal and absent-evidence policy
fixtures. It does not label those literals as measured model outputs:

```shell
coax/bin/coax run psionic/examples/panda-pd-gain-search.coax
```

The default Darwin arm64 gate consumes committed cached observations and runs
only current-source scorer/selector checks, including native/sanitizer modes:

```shell
COAX=coax/bin/coax psionic/tests/robotics/check-panda-pd-gain-search.sh /tmp/panda-gain-cached-new
```

It never executes the canonical grid, loads the SDK, simulates the baseline or
needs historical private binaries. The acquisition driver is separate and
one-shot. The cached gate does not claim a new physics run or held-out result.

This is a negative bounded search result. There is no qualified gain family
to validate. The predeclared held-outs remain unused; another hypothesis needs
a separate freeze. Global optimum, general tracking/stability, contact,
hardware safety, finite nullspace integration and compiler real-time bounds
remain outside scope.
