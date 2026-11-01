# Four short nonlinear Panda experiments

This fixture freezes four new 750-step cases: gravity hold, quintic tracking,
a tracking pulse, and saturated braking. Their controller and reference
constants were fixed before any new trajectory or SDK call. Every case lasts
1.5 seconds at a 500 Hz numerical update rate. The tracking references take
270 ticks (0.54 seconds), then hold their endpoints.

The numerical comparison passes. **Two behavior goals miss:** nominal final
maximum error is 0.0157710 rad, above the 0.01 rad goal; the brake does not
recover within 1.5 seconds. No gains, cases, horizons or budgets were changed.

| Case/result | Measured SDK value | Frozen goal |
| --- | --- | --- |
| Hold peak arm drift | 0.0000162924 rad | ≤0.005 rad: pass |
| Tracking RMS | 0.00725912 rad | Better than analytic stationary RMS 0.0199807 rad |
| Tracking final maximum error | 0.0157710 rad | ≤0.01 rad: **miss** |
| Pulse final maximum error | 0.0159404 rad | ≤0.02 rad: pass |
| Pulse 25-state position/rate band | Entry state 477, confirmed at 501 | Observed temporary band |
| Brake | 31 controller-clipped ticks; final maximum error 0.0455202 rad | **Not recovered** |

Pulse-band entry is 0.454 seconds after the force interval ends; confirmation
is at 1.002 seconds from reset. The pulse trajectory later leaves the 0.01 rad
band, so this is not permanent settling. The brake starts at 2 rad/s. Its
peak effective step acceleration is 41.58735686700388 rad/s², calculated as
`(post_v - pre_v) / 0.002`, not SDK `qacc`. The reference limits of 0.12 rad/s
and 0.6 rad/s² constrain the reference, not the feedback-controlled plant.
No source joint coordinate excursions or actuator-stage clips were measured.
These are sample-grid observations, not continuous-time certificates.

## What is owned

The test-only `pnt_*` consumer chooses every case constant, target, gain,
reference tick, pulse and composition in Coax. Control at tick `k` uses the
reference at `min(k,N)`; post-state quality uses reference `k+1`. Case 2
applies +3 J/rad on joint 2 exactly for `200 <= k < 250`. Finger targets stay
at their initial positions; actual drift is recorded. The PD requested force,
controller-clipped command and actuator clipping stages remain distinct.
`pnt_step` feeds controls/external forces to the owned implicit-damping Euler
plant. Its separate actuator observation is never added again to the plant.

`pnt_initial`, `pnt_control` and `pnt_step` accept only case IDs 0..3. The latter
two accept ticks 0..749 and snapshot both raw current-state views before
publication. Outputs are complete and atomic: 144-byte initial state,
712-byte command and 944-byte command-plus-step; statuses retain separate
stage and underlying code words. `q9` means seven radians and two metres;
`v9` means seven rad/s and two m/s. Physical reference/controller/actuator
fields retain their quantity types.

The case/reference/control/apply helpers and their PODs are source-visible
and unsealed. To obtain the four-case policy, use the fixed case factory,
admitted tick and coherent generated command. The apply helper delegates
numerical state/control admission to the owned plant; it does not certify a
forged command's reference/controller metadata. Deliberately altered helper
configurations used in clipping/refusal checks are **diagnostic fixtures**,
not additional public experiment cases.

## Evidence

- `experiment.json`, `cases.f64`, `input-freeze-v1.json`: original input freeze.
- `analytic-reference.json`: two unique new analytic curves, published as all
  four cases' 751 frames, plus the exact external-force schedule.
- `source-qualification.json`: twelve new source groups; 119/119 control and
  75/75 public actual emitted root sites killed. Initial 6+1 observational
  survivors and their closure are retained; production bodies stayed fixed.
  These counts do not claim full quantity-operator token provenance.
- `sdk-reference.json`: one acquisition, four initial forwards and exactly
  3,000 new SDK steps, with no explicit post-step forward, rendering or NN.
  It pins the original model/assets, source, transport and progress ledger.
  SDK actuator forces are the pre-integration cache left by `mj_step`.
- `independent-controller-qualification.json`: all 105,000 PD fields and
  27,000 actuator fields audited by independent scalar and matrix-gradient
  calculations. Maximum PD error is 6.04e-14; actuator values are numerically
  exact. Fifteen late reader poisons are refused.
- `native-qualification.json`: first debug, release and fail-fast ASan/UBSan
  full profiles, 1,329,240 assertions each and exact complete mode bytes.
  Each profile has 3,000 fresh-feedback owned steps and 3,000 recorded-command
  shadow steps. Fresh hinge position/rate errors are at most 4.44e-16 rad and
  1.60e-15 rad/s; shadow errors are 3.33e-15 rad and 5.75e-15 rad/s. Separate
  first sanitizer ABI probes cover 89 refusals and four input/output overlaps;
  those 386 assertions are carried, not repeated in the three full runs.
  Forty-five replay-reader poisons are refused.
- `example-qualification.json`: the pure example's first new display check,
  debug and release executions match its independently prewritten golden.
- `offline-routing-qualification.json`: syntax, plan and stub routing checks
  for the additive offline adapter. Its new numerical execution path was not
  rerun here; underlying numerical evidence is carried unchanged.

Absolute parity limits were frozen before capture: hinge position 2e-8 rad,
slide position 2e-9 m, hinge rate 2e-7 rad/s, slide rate 2e-8 m/s and each
controller/actuator coefficient 2e-8 in its own unit. Status, clipping counts
and native mode bytes compare exactly. All actual/reference fields are
checked finite and complete before maxima. Behavior goals remain separate
from these numerical gates. The stationary-position baseline is analytic,
not an executed holding controller. Existing primitive proofs are carried;
no old standalone NN, SDK or primitive gate was repeated.

## Run

The pure example requires no SDK or Python at runtime:

```shell
coax/bin/coax run psionic/examples/panda-joint-pd-tracking.coax
```

The additive Darwin arm64 offline gate uses the committed SDK trace, input
and receipts. It regenerates current Coax C/headers and SDK-free binaries;
it needs no installed SDK, authenticated assets or historical private binary:

```shell
COAX=coax/bin/coax psionic/tests/robotics/check-panda-joint-pd-tracking.sh /tmp/panda-tracking-new
```

That adapter's syntax/routing qualification is separate from the measured
first full native runs above. The original `panda_joint_pd_tracking_native_reference.py`
remains an unchanged historical capture-directory driver. New SDK acquisition
is explicitly separate in `panda_joint_pd_tracking_sdk_reference.py`; it is
not part of the offline gate.

The profile is the pinned scene's fixed-source Panda with contact and
constraint processing disabled (flags 17), Euler integration and physical
joint damping enabled. Nonrobot state starts from compiled defaults. This is
not a whole-scene physics backend, general controller API, collision/contact
qualification, nonlinear stability proof, hardware result or compiler
`#bounded`/real-time guarantee. Finite nullspace integration and collision/RRT
planning remain separate work.
