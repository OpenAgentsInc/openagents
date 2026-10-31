# Clock domains and a fixed multirate runtime

`step-clock-500-20-v1` is a small runtime precursor for R13. It gives the owned
oscillator an integer simulation clock, a fixed sample-and-hold schedule and
explicit receipt failures. It does not implement compiler clock syntax or a
general robot scheduler. The runnable example is
[`examples/step-clock.coax`](../../examples/step-clock.coax).

## Separate the domains

| Record | Meaning | Operations |
| --- | --- | --- |
| `ScSimInstant {epoch, step}` | Committed physical-step counter within one local reset epoch | Add `ScSimDelta`, compute same-epoch age |
| `ScSimDelta {steps}` | Nonnegative count of physical steps | Explicit F64-second boundary |
| `ScSteadyInstant {session, ns}` | Supplied monotonic-clock observation within one session | Elapsed, deadline, expiry |
| `ScSteadyDelta {ns}` | Nonnegative steady nanoseconds | Checked deadline addition |
| `ScCivilInstant {source, seconds}` | Supplied finite civil label with an opaque source identity | Admission only; no civil/simulation map |

These are distinct Coax struct types. Five compile-fail fixtures reject mixed
domains and using an instant as a duration. Their integer fields remain ordinary
integers: constructors and host observations must be honest, and the checked
functions admit their bounds. Epoch/session/source identifiers are local labels,
not authentication or globally unique identities.

Simulation is limited to 18,000 steps, identifiers and serials to 1–1,000,000
(serial zero means no request yet), and steady values to 0–9×10^18 nanoseconds.
Admission checks headroom before addition. Backwards or foreign-domain elapsed
requests refuse; a deadline expires inclusively at equality. No function reads
a hidden host clock. Civil labels do not drive the schedule.
`sc_sim_age` only computes an age; it does not admit a maximum sensor age,
light-time or a complete R18 sensor policy.

The exact simulation period is the integer ratio **1/500 second**. Twenty-five
steps make **1/20 second**. No repeatedly accumulated float drives event order.
`sc_seconds` explicitly rounds a step count divided by 500 to F64. The separate
`sc_sdk_time` adapter preserves the existing SDK expression
`initial + f64(step)*0.002`, with inclusive 2^-26-second tolerance and initial
time in [0,100]. It does not replace the integer clock.

The scalar quantity compiler milestone has F32/F64 carriers, not integer
nanosecond quantities; its binary unit-scale conversion cannot express 1/500.
This precursor uses nominal integer records and explicit F64 boundaries rather
than claiming those missing language capabilities.

## Follow consumed rows

| Event | Fixed profile |
| --- | --- |
| Physical integration and control/gripper intent | Every step, 500Hz |
| New action goal | Substep zero of each 25-step row, 20Hz |
| Request | After eight **consumed** action rows; nominally 2.5Hz |
| Hold | One complete 25-step row; does not consume an action slot |

A request ticket binds epoch, serial, profile and base step. A reply can replace
the empty chunk only while the simulation still has that base step. An arbitrary
steady-time wait does not advance simulation. The example intentionally shows
an expired supplied steady deadline alongside a valid unchanged simulation
origin: applying a real timeout/cancellation policy is future work.

Normal request origins are 0,200,400. A deferred or refused first reply followed
by one hold gives 0,25,225; a missing first observation gives 25,225,425.
Scheduling requests by `step % 200` would get recovery wrong.

## Preserve the order and the generation labels

At a request boundary, capture the cached observation **before** the next explicit
forward/sample. `ScObservation.last_forward` names the last explicit forward
generation, `max(0,k-1)`; `qpos` names the current committed generation `k`.
Reset labels both zero. These generic labels do not claim a physical timestamp
for every external integrator's internal caches. The pinned Euler SDK inference
has a separate [source audit](../../fixtures/robotics/libero-panda/integrator-source-profile.json).

The admitted order is capture/request/accept (or hold), row selection, fresh
sample at `k`, control intent, integration, then next-step admission and trace.
`ScPhase` checks prevent moving straight from selection to integration. The
goal flag must be true only at substep zero; the gripper-intent flag is required
on every step. This models the event order, not an actual gripper actuator.

Ordinary admission refusals preserve the cursor. For a valid cursor, a wrong-phase, duplicate,
skipped, foreign-epoch or failed integration receipt latches `halted` without
publishing a new step; recovery requires an explicit epoch reset. A failed
oscillator integration preserves its plant as well. Public records are
caller-owned values: structural validation is not provenance for a fabricated
history or rollback of external physical effects.
Callers must adopt `sc_receipt(...).state` even when its status is non-ok: that
returned state carries the fail-stop latch. Dropping it and retaining the old
cursor would discard the failure. `sco_advance` already propagates the latch.

## An owned consumer and focused evidence

`oscillator-clock.coax` applies a held force to the existing owned RK4 solver at
F64 .002 seconds. It admits a new force only on a row boundary and commits plant
and time together. The force pattern is a declared experiment input, not a
learned controller or an assessment of task success. The unchanged
[oscillator numerical qualification](../../fixtures/physics/oscillator/qualification.json)
is reused; its arithmetic gates and all SDK/model runs are not repeated here.

```shell
coax/bin/coax run psionic/examples/step-clock.coax
COAX=coax/bin/coax psionic/tests/physics/check-step-clock.sh /tmp/new-clock-check
```

The new [qualification](../../fixtures/physics/step-clock-qualification.json)
binds three authority roots with zero surviving root mutants. Debug, release
and ASan/UBSan native modes cover four 18,000-step/720-row scenarios, including
paused fake wall time, hold reanchoring, bounds and receipt faults. An independent
stdlib integer/Fraction oracle checks every event field, exact periods and
request origins. New native builds can reuse that proof only after matching all
event/summary bytes and binding the original oracle/profile/header identities;
changed/false proof metadata refuses. The example has a prewritten golden
transcript, identical in both native modes.

General rates/phases, civil mappings, actual steady-clock effects and watchdogs,
inference cancellation, complete runtime snapshots, Verse migration, compiler
R13 syntax and R14 `#bounded` remain pending. No genuine robot integration or
real-time performance has been qualified by this runtime precursor.
