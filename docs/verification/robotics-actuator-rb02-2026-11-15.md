# RB-02 — illustrative scalar servo and complete in-memory state

Source base: `a8210ae767`. Profile: `scalar-servo-play-f64-v1`, CPU F64.
The [frozen contract](../../psionic/docs/robotics/actuator-contract-rb02.md)
defines equations, parameter origin, finite boxes, integration order and
the complete state. Coefficients are illustrative. No measured motor,
firmware match, mechanical gearbox, biped dynamics or hardware is qualified.

## Independent evidence

The Fraction oracle was written and its immutable outputs generated before
the Coax implementation. It retains 56 rows × 17 columns as exact rational
values and independently generates the rounded Coax reference. All numerical
outputs compare at the predeclared `2e-12 + 2e-12*abs(reference)` tolerance;
clipping flags, cursor and accepted-step counts compare exactly.

The three frozen cases exercise capped torque, positive/negative commands,
two-update delay, observation play, negative initial velocity, derivative
feedback, viscous/Coulomb losses and half available effort. The initial
hand calculation yields 0.20 requested/0.15 delivered torque, 15 acceleration,
0.03 rate and 0.00006 position at 0.002 seconds. The play output does not
feed back or apply load reaction. Its physical interpretation remains narrow.

## Observed gates

The registered complete entry is:

```shell
coax/remote/remote.sh run -- bash psionic/check.sh --only robotics/actuator
```

This slice was qualified in bounded passes, retaining unchanged passing
results rather than repeating them after a harness-only correction:

- Actuator: **16 checks passed** in debug and release; **137 root mutants,
  137 killed, zero survivors or equivalents**. Checks include all parameter
  endpoints/adjacent outside words, delay 0–16 and repeated wrap, nonempty
  queue restore, complete output preservation, exhausted counter and a
  derived-state failure after queue staging.
- The independent oracle matched all frozen fields. Two deliberately broken
  typed programs fail with the required **E0317** target-unit and **E0327**
  inertia-assignment diagnostics and nonzero compiler exits.
- Public API and independent trace observer passed actual native debug and
  release, each plain and with **ASan/UBSan**, on remote host `oa-build-1`.
  Five nonfinite IEEE classes poison every parameter, every state/queue slot,
  an incoming target and each force input. Observers check every output field
  after failure. In/out snapshot/step aliasing is exercised through value inputs.
- The action-map-to-servo example's full transcript passes debug/release,
  including delayed restore and a negative-inertia query refusal.

The first run exposed an incorrectly uncleared inactive test slot. The first
mutation pass then found four genuine test gaps: queue slot zero, the first
inactive slot, and nonzero initial motor/output positions. Those observations
were added; no product code was weakened or site marked equivalent. Native
harness bring-up also corrected the expected assignment diagnostic and a
Coax variable-name collision. Failed runs were not reported as qualification.

## Shared primitive and carried profiles

The finite clamp moved from Panda's private math into `coax/lib/clip.coax`.
Panda retains `PaClip`/`pa_clip` as a delegating wrapper, with unchanged value
and strict clipping semantics. The experiment action map uses the same
primitive without importing fixed Panda dynamics.

- Shared clamp: **6/6 root mutants killed**, no survivors or equivalents.
- Panda actuation math: **47 checks passed** in debug/release; **14/14 root
  mutants killed**. This is portable arithmetic regression, not a new full
  fixed-source SDK/device receipt.
- Experiment query: **35 checks passed** in debug/release; **197 mutants,
  196 killed, zero survivors, one carried reviewed equivalent**. Its native
  table/nonfinite checks pass all four modes and its unchanged transcript
  passes both evaluator modes. The extracted shared transcript runner was
  exercised again with only the example phase.

## Remaining boundaries

Snapshots are complete in-memory values. A durable experiment checkpoint
still requires the RB-06 registered companion with expected identity and
configuration binding. There is no struct-dump format or implicit serializer.
The generic query has no device effect, hidden randomization or motor
authorization. Engine generalized-force integration, biped/contact support,
identification and physical timing remain their own planned work packages.
