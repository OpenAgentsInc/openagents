# From the SDK loop to an owned robot backend

The genuine LIBERO/Panda SDK loop is delivered: original GR00T inference on
Metal, own Coax preparation/controller/supervisor, 3/6 frozen task successes and
exact recorded-control replay. The source-framework comparison also times out
on the three failed cases. Those outcomes do not identify their cause; four
global visual diagnostics remain failed. MuJoCo still supplies dynamics,
contacts and rendering for the genuine model evaluation.

| Building block | Delivered scope |
| --- | --- |
| [107](../../book/ch107-transform-functions-not-just-tensors.md) | CPU graph functions, formal first/second partials and batching |
| [109](../../book/ch109-learn-a-physical-system.md) | Owned one-axis F64 RK4 and a separately evaluated learned surrogate |
| [111](../../book/ch111-recover-the-physics-from-the-motion.md) | Exact-solver parameter recovery, noise/sampling and an unidentifiable equilibrium |
| [Force sensitivities](oscillator-functions.md) | Independently checked F32 planning realization, fixed-context q/v/u products |
| [Checkpoints](oscillator-replay.md) | Exact same-build one-axis state/accepted-force restore; later events external |
| [Effect inventory](../../../coax/EFFECTS.md) | Transitive may-effects with witness paths; borrowed/foreign/intrinsic unknowns explicit |

The next sequence follows the compiler [physical-world requirements](../../../coax/ROADMAP.md#15-r12--units-and-coordinate-frames):

1. Finish R12 quantities, points/vectors and frame-chain transforms. The [scalar quantity compiler milestone](../../../coax/QUANTITIES.md) now
qualifies native carriers, matching-unit arithmetic and binary scale conversion.
The separately checked exact metadata producers prepare unit products; source
product/type-interner integration, frame algebra and Panda migration remain
pending. Preserve Verse’s left-handed view conversion and
Panda’s distinct grip-site/EEF frames, arm radians and finger metres. Measure
physical carriers and native code against the untyped baseline.
2. Add R13 simulation/steady/civil clock distinctions and declared multirate
sample-and-hold. Preserve the SDK’s 500Hz physics and torque servo, 20Hz action-goal
updates and eight executed action rows per model request. Simulation currently pauses during
inference; a real steady-time deadline needs its own measurement and policy.
3. Close the R3 storage and R11 effect facts required by R14. Unknown foreign,
borrowed-region or intrinsic resources must remain refusals for `#bounded`.
Prove fixed scratch/stack and loop/call bounds, including error/cleanup paths,
and compare target stack bounds to native measurement.
4. Qualify the own smooth control loop, numeric profile and stamped command
supervision. Use the physical solver for publication, 107 for local sensitivity
checks, and 111 for parameter calibration. Admission/refusal and saturation are
explicit events, not silently differentiated. The existing one-axis checkpoint
is a precursor to complete controller, schedule and producer snapshots.
5. Build the matching articulated model and scene path under R17: admitted
links/joints/inertias/frames, kinematics, forward dynamics, contacts, gripper and
rendered observation conventions. Compare small components with the SDK before
running expensive whole episodes. A field layout alone is not embodiment parity.
6. Complete R18/R19 sensor age, command expiry/limits and a frozen fault corpus,
then full R16 restore/replay with complete state inventory. Preserve held-out
failures and compare uninterrupted/resumed episodes under the exact declared
build/worker/target profile. R15 cross-target math/jobs require separate actual
architecture/worker evidence.

A small gantry can qualify clocks, supervision and replay with its own
controller/policy profile. The selected original checkpoint is trained for the
LIBERO/Panda observation/action semantics; a toy gantry or replayed action tape
cannot stand in for genuine closed-loop qualification. Until a matching owned
backend exists, that original model’s task evaluation stays on the SDK path.
No physical robot is part of this sequence.

## The existing SDK schedule is the concrete R13 consumer

| Event | Nominal simulation schedule | Order |
| --- | --- | --- |
| Physics and torque servo | every 2ms, 500Hz | forward/sample, Coax torque computation, SDK integration, clock/trace check |
| Action-goal update | every 25 physical steps, 20Hz | update the controller goal at substep 0; hold that goal through substeps 1–24 |
| Learned request | nominally every 8 action rows, 2.5Hz | capture/infer/accept before executing the next selected row; simulation pauses during inference |
| Task success | after each 25-step row, 20Hz | commit the completed row, process a pending delayed reply, then sample success |

These rates are distinct. `ps_control` calls `pc_step` at every physics step;
only its goal-update flag is gated by `substep==0`. The period/integration
sources are `tests/robotics/panda-session.coax` and
`tests/robotics/panda_loop_effects.h`. Faults, refusals and early success make
actual request/episode counts observable outcomes rather than fixed totals.
A typed scheduler must preserve this order and the declared clock domains,
not infer wall-clock throughput from nominal simulation rates.
