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

1. Finish R12 quantities, points/vectors and frame-chain transforms. A scalar
quantity compiler milestone is under construction; it does not complete frame
algebra or Panda migration. Preserve Verse’s left-handed view conversion and
Panda’s distinct grip-site/EEF frames, arm radians and finger metres. Measure
physical carriers and native code against the untyped baseline.
2. Add R13 simulation/steady/civil clock distinctions and declared multirate
sample-and-hold. Preserve the SDK profile’s 500Hz physics/20Hz control schedule,
with eight executed rows per model request. Simulation currently pauses during
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
