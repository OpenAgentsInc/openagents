# 170. Close the simulated control loop

The original GR00T policy now has a complete route to a simulated Panda:
raw cameras and arm state enter Coax, Coax executes the original learned
network on Metal, and a Coax operational-space controller turns its selected
pose commands into seven torques and two finger targets. MuJoCo 3.3.1 advances
the source LIBERO scene. The next observation comes from that changed scene.

We first qualify the actual LIBERO/Panda loop with MuJoCo as an evaluation
backend. Chapters 107, 109 and 111 now provide independent function-transform,
scalar-solver and differentiable-solver foundations. Later sections qualify
fixed-source robot queries and an unconstrained Euler map. Their integration
into a matching owned scene/constraint/rendering backend, and compiler
enforcement of `#bounded`, remain pending. The
controller uses fixed arrays, checked loop limits and no allocation or effects;
that is a reviewed implementation property, not an implemented compiler
annotation or proof. This chapter does not qualify real hardware.

[Chapter 169](ch169-execute-a-groot-policy.md) established a prepared 140-token
learned-network replay. Here the source book-and-caddy instruction creates
**152 tokens** and the images come from a genuine source scene. A separate
[portable example](../examples/panda-supervisor.coax) teaches supervisor
transitions without weights, a GPU or a simulator. It never stands in for the
original-model run.

## Bind the scene before sending an action

The selected task asks the Panda to pick up a book and put it in the back
compartment of a desk caddy. The
[contract](../fixtures/robotics/libero-panda/contract.json) pins the
[LIBERO source](https://github.com/Lifelong-Robot-Learning/LIBERO/tree/8f1084e3132a39270c3a13ebe37270a43ece2a01),
[robosuite controller](https://github.com/ARISE-Initiative/robosuite/tree/fbee5844ff5632f5b5698e204ec5357ca50be0df),
and the original dataset revision. The
[acquisition receipt](../fixtures/robotics/libero-panda/acquisition.json)
records the authenticated HDF5 and scene assets. Neural weights and processor
remain the category 2 `GR00T-N1.7-LIBERO/libero_10` artifact of chapter 169.

A demo's XML and its initial state form a pair. All 50 source XMLs and caddy
placements differ. Reusing demo 0's scene for another demo's state silently
changes the evaluation. Our fixed profile restores each selected original
45-F64 state into its own scene and performs a fresh SDK forward. This is an
explicit reset distribution, distinct from NVIDIA's ordinary seeded reset.

HDF5 does not contain all historical controller or solver state. We therefore
start a fresh Coax controller at the restored grip-site pose, with the pinned
nominal Panda nullspace home and zero normalized finger targets. The home is
`[0, pi/16, 0, -pi/2-pi/3, 0, pi-.2, pi/4]`; it is not inferred from the
restored joint positions. A fresh-controller episode is reproducible under
this named profile. It does not recreate unrecorded demonstration controller
goals or qacc warm-start history.

The [evaluation profile](../fixtures/robotics/libero-panda/evaluation-profile.json)
freezes demo/state pairs **1, 10, 20, 30, 40, 49** before observing policy outcomes.
Controller and neural implementation fixtures use demo 0. These six pairs are
held out from implementation work; the checkpoint was fine-tuned on LIBERO 10,
and its exposure to these demonstrations is unknown. Their results cannot be
presented as checkpoint-training holdout accuracy or a generalization estimate.

The flow sampler takes an explicit 5,280-word F32 noise tensor, shaped
`[1,40,132]`. This profile generated it once with NumPy PCG64(169), narrowed it
to F32 and froze its hash. The runtime reuses those bytes on each request.
There is no cross-runtime RNG-equivalence claim and no resampling hidden in
the control loop. Changing the noise defines a different experiment.

## Know which observation the model sees

The two raw 256-square RGB cameras are `agentview` followed by
`robot0_eye_in_hand`. The SDK host reads an offscreen OpenGL framebuffer; it
opens no window. Coax applies the source wrapper's two-axis flip, 243-square
centre crop, qualified resize and source fused F32 normalization. It packs the
camera patches in the learned order, normalizes the eight physical state
coordinates with the original statistics, and tokenizes the actual instruction
with the original tokenizer. Camera preparation is no longer a hand-authored
network input.

Two rotation quantities must stay distinct:

- The model observation uses EEF grip-site XYZ, the `robot0_right_hand` BODY
  quaternion converted into axis-angle, and two measured finger positions.
- The operational-space controller uses the `gripper0_grip_site` SITE rotation
  matrix, world-frame site velocity and site Jacobians.

MuJoCo supplies quaternion order wxyz. The source observation conversion uses
xyzw, preserves quaternion sign, clamps the scalar to [-1, 1] and converts with
`xyz * 2 * acos(w) / sqrt(1-w*w)`, returning zero when the denominator is exactly
zero. Canonicalizing to positive w changes valid angles above pi. Calling
these axis-angle coordinates roll/pitch/yaw does not make them Euler angles.
The [Coax observation boundary](../lib/robotics/panda-env.coax) reproduces these
semantics and stages its complete eight-value result before publication.

Timing is part of this observation's meaning. Before every controller substep,
the host calls `mj_forward`, then samples fresh site pose, velocity, Jacobians,
arm mass and bias. It sets motor controls and calls `mj_step`. The source does
not do another forward before its observation. Site/body/camera derived data
therefore lag the newly integrated qpos by one **2 ms** step, while the finger
qpos is current. Reset observations have no such lag. We preserve and label
this mixed observation rather than quietly refreshing it into a different
model input. Independent source snapshots check this interpretation.

## Give the chunk a logical clock

The source action path has three horizons: the model generates 40 padded rows,
the adapter decodes 16 × 7, and the environment executes the first **eight**
actions before requesting a new chunk. Each of those actions lasts 25 physics
steps of .002 s: one .05 s logical control tick, or 20 Hz in simulation time.

Inference is synchronous and **simulation pauses during inference**. Twenty
hertz does not describe wall-clock throughput. The ledger records actual
policy latency and GPU command time separately. A future asynchronous or
real-time deployment needs a different admission and deadline contract.

The Coax session also checks the SDK clock after every physical step against
`initial_time + (25*tick + substep)*.002`, within an inclusive 2^-26 seconds
(about 15 ns). Initial SDK time must lie in [0,100] seconds; the nominal elapsed
horizon is at most 36 seconds. A reset or jump cannot silently become another
admitted tick. These are checked F64 values and integer counters, rather than
the compiler clock types planned in R13. The first two recorded evaluations
predate this live guard: the [clock receipt](../fixtures/robotics/libero-panda/clock-qualification.json)
checks all their 9,700 saved clocks through the same compiled native function.
The remaining four evaluations execute the guard live.

The [supervisor](../lib/robotics/panda-loop.coax) owns reset epochs, request
serials, chunk origins, action indices and exact tick receipts. A model reply
must match the waiting epoch, serial, profile and origin tick. It validates
all 112 decoded coordinates, including the unused last eight actions, before
publishing the first eight. Pose coordinates must be finite within ±1 and the
gripper must be exactly −1, 0 or 1. Private scratch may change during refusal;
public actions and the header do not.

Run the portable protocol example from the repository root:

```shell
coax/bin/coax run psionic/examples/panda-supervisor.coax
coax/bin/coax run psionic/examples/panda-supervisor.coax --release
```

Its first section admits owned commands and supplies eight authored 25-step
receipts. These are protocol inputs; **no physics runs in this example**.

<!-- source: psionic/examples/panda-supervisor.coax chunk -->
```jai
    reset:=pl_reset_done(pl_new(),0);
    #assert reset.status==PlStatus.ok;
    request:=pl_request(reset.state);
    #assert request.status==PlStatus.ok;
    words:[112]u32;actions:[8]PlAction;scratch:[8]PlAction;
    for row:0..<16 {words[row*7]=f32_bits(0.125f32);}
    admitted:=pl_accept(request.state,request.tag,0,words[..],actions[..],scratch[..],false);
    #assert admitted.status==PlStatus.ok;
    header:=admitted.state;
    for tick:0..<8 {
        selected:=pl_choose(header,tick);
        #assert selected.status==PlStatus.ok && selected.slot==tick && !selected.fallback;
        #assert actions[selected.slot].v[0]==0.125f32;
        // Authored receipt for this protocol demonstration, not a physics run.
        committed:=pl_commit(selected.state,tick,25,0);
        #assert committed.status==PlStatus.ok;
        header=committed.state;
    }
    #assert header.tick==8 && header.phase==PlPhase.empty;
```

After eight receipts the header returns to empty at tick 8. The genuine SDK
runner reaches that state only after 200 actual physics steps. The supervisor
requires exactly 25 completed substeps and a successful effect receipt before
advancing each tick. An interrupted effect halts the episode; it cannot be
reported as a completed action.

Missing input or a waiting reply can select the declared zero-pose fallback.
That command still passes through the OSC controller: it is **not zero motor
torque**. Its zero gripper command holds the internal finger target. If a reply
arrives after a fallback advanced the clock, its old origin is stale. Refuse it,
issue a fresh request, and bind the replacement to the current tick.

<!-- source: psionic/examples/panda-supervisor.coax recover -->
```jai
    delayed:=pl_request(header);
    held:=pl_hold(delayed.state,8);
    #assert held.status==PlStatus.ok && held.fallback;
    header=pl_commit(held.state,8,25,0).state;
    late:=pl_accept(header,delayed.tag,9,words[..],actions[..],scratch[..],false);
    #assert late.status==PlStatus.stale && late.state.phase==PlPhase.waiting;
    refused:=pl_refuse(header);
    #assert refused.status==PlStatus.ok && refused.state.refusals==1;
    fresh:=pl_request(refused.state);
    admitted=pl_accept(fresh.state,fresh.tag,9,words[..],actions[..],scratch[..],false);
    #assert admitted.status==PlStatus.ok && admitted.state.fallbacks==1;
```

Reset increments the epoch so that an old reply cannot enter a new episode.
The example also demonstrates a 24-step receipt halting without advancing time.

<!-- source: psionic/examples/panda-supervisor.coax reset -->
```jai
    restarted:=pl_reset(admitted.state);
    #assert restarted.status==PlStatus.ok && restarted.state.epoch==2;
    ready:=pl_reset_done(restarted.state,0);
    current:=pl_request(ready.state);
    old:=pl_accept(current.state,request.tag,0,words[..],actions[..],scratch[..],false);
    #assert old.status==PlStatus.stale;
    admitted=pl_accept(current.state,current.tag,0,words[..],actions[..],scratch[..],false);
    #assert admitted.status==PlStatus.ok;
    selected:=pl_choose(admitted.state,0);
    incomplete:=pl_commit(selected.state,0,24,0);
    #assert incomplete.status==PlStatus.effect && incomplete.state.phase==PlPhase.halted;
```

## Turn a pose command into motor controls

The decoded first six coordinates are normalized delta commands, not
velocities or absolute joint targets. On the first physics substep of a new
action, the Coax controller clips them to ±1, scales XYZ by .05 m and rotation by
.5 rad, and reanchors position at the current world site position. A nonzero
world axis-angle increment left-multiplies the current site rotation. Exactly
zero rotation retains the previous orientation goal; replacing it with the
current rotation would change the source controller.

For every physics substep, the controller recomputes task-space inertia from
the fresh 7 × 7 mass and 6 × 7 Jacobian. Translation and rotation use separate 3 × 3
inertias, task gain 150 and critically damped gain 2 sqrt(150). The full 6 × 6
pseudoinverse defines the dynamically consistent nullspace projector. A
mass-weighted posture term pulls toward the explicit nominal home, and the
source bias compensation is added before clipping to the actual actuator
limits: ±80 Nm for the first five joints and ±12 Nm for the last two.

The [matrix helpers](../lib/robotics/panda-matrix.coax) use F64 Cholesky mass
inversion and bounded one-sided Jacobi SVD. The pseudoinverse preserves
NumPy's source-default cutoff: retain singular values strictly above
**1e-15 times the largest**, not 1e-5. Nonfinite values, a nonphysical mass
matrix, unsupported numeric scale or exhausted convergence refuse. The
fixed-array implementation allocates nothing; it does not promise uniform
error near every singular configuration.

The controller's axis-angle-to-rotation path preserves another source detail:
its quaternion begins in F64 but the source `quat2mat` explicitly narrows it,
its normalization and outer product to F32. The observed scalar expressions
then follow NumPy 1.26 promotion. This is a named source-formula profile, not
Numba-JIT bit identity. It is also separate from the learned network's F32
activations and wide-statistics Metal profile.

The seventh command advances two persistent normalized finger targets by
`[-1,+1] * .01 * sign(command)` on **every substep**, with clipping to ±1.
Targets map to [0, .04] m and [-.04, 0] m. Moving the update outside the 25-step loop
would slow it 25-fold. Zero command holds this internal target, even when the
measured finger position differs.

[`pc_reset` and `pc_step`](../lib/robotics/panda-control.coax) validate the full
sample, state, action and pinned actuator ranges. They stage goals and finger
state privately and publish only after all matrix operations and motor outputs
succeed. Refusal leaves every controller-state field untouched. An invalid
result's zero-filled motor arrays are not admitted motor commands.

The [controller receipt](../tests/robotics/panda-control-qualification.json)
records 297 matrix-root and 462 controller-root mutants, all killed. The native
harness passes 963 cases each in debug, release and ASan+UBSan, with
bit-identical outputs across builds. An independent NumPy 1.26.4 reference
uses seven frozen original telemetry rows and eight owned action perturbations,
including 25-substep fixed-sample replays: 112 controller cases plus 80 matrix
cases per mode. Maximum torque difference is 3.410605131648481e-13 Nm under
frozen 2e-5 + 2e-6 |reference| limits; goal rotation differs by at most
1.1102230246251565e-16, and other state fields and clipping counts match.
This is numerical controller evidence. It does not itself show task completion.

## Replay controls without running the policy again

The [SDK driver](../tests/robotics/panda_loop_metal.c) connects the Coax
session, resident Metal policy and MuJoCo effects. Each completed physics step
emits a canonical 512-byte record with epoch/serial/tick/substep, selected
seven-coordinate action, the nine applied motor controls and resulting 45-F64
integration state. Coax owns the record encoding and SHA256 chain.

The chain starts from hashes of the selected local scene, initial state,
explicit noise and profile. After the episode, the replay resets the same
scene and state and applies the recorded controls, forwarding before each
step. It checks every resulting state byte and the final chain. No model
inference is needed for this controls-only replay.

Exact replay means exact recorded states under this SDK build and numerical
profile. It is not cross-platform determinism, proof that repeated neural
inferences are bit-identical, or reconstruction of hidden historical source
controller state. The chain detects accidental changes; it is not a signature,
remote attestation or authentication of arbitrary future model/scene files.
The caller still owns authenticated immutable inputs and honest effect receipts.

The wrapper requires a fresh output directory, and the driver creates each case
directory exclusively. It refuses to overwrite a prior ledger. Unexpected
controller, SDK, clock or file/device failures stop the experiment; a partial
trace without a completed footer and result is not a qualified episode. The
current driver does not promise a durable completed failure ledger or automatic
recovery from arbitrary platform failures. The declared fault experiments below
exercise the request supervisor with otherwise successful effects.

Task success also has a precise source meaning. Coax reproduces LIBERO's
strict world-axis-aligned containment test for the book body's origin in the
caddy region, with the source lower-z allowance. This is not complete book
volume containment, and this source predicate does not require contact.
Synthetic last-row demonstration reward/done labels are not an online oracle.

## Keep numerical parity separate from task outcomes

The genuine 152-token frame audit uses raw source-scene cameras and the actual
instruction/state, with original learned weights throughout. It preserves
frozen budgets and reports these questions separately:

| Boundary | Current result |
| --- | --- |
| Prepared pixels, state, IDs and explicit noise | Exact independent preparation handoffs |
| Four public visual outputs: ordinary merger plus three deepstack taps | Pass |
| Text endpoint and all 31 global action-head boundaries | Pass |
| Same-input independent conditional action head | All 31 boundaries pass |
| Adapter on actual Coax normalized output | All 112 decoded values exact |
| Gripper against the original independent model endpoint | All 16 decisions exact |
| All 34 global visual hidden boundaries | **False: four late boundaries fail** |

The failed visual boundaries are blocks 21, 22, 23 and the ordinary merger input.
They remain in the diagnostic record at their original bounds. Public visual
features pass after the merger, but that does not relabel their input or make
a claim of uniform hidden-state parity. No source-default BF16 execution
identity is claimed. The composed product uses actual Coax visual/text
features, never captured reference features substituted into the policy.

The initial genuine demo 0 smoke run completed 16 logical ticks, two model calls
and 400 physics steps, with exact controls-only replay. That establishes a
working feedback path for the declared interval. It is not a success-rate
measurement.

The [closed-loop receipt](../fixtures/robotics/libero-panda/closed-loop-qualification.json)
records the six frozen evaluations. Each runs until source-predicate success
or the 720-tick limit; no case was retried or tuned after seeing its outcome.

| Demo | Ticks | Model requests | Outcome | Clipped torque channels | Wall seconds | Exact physics replay |
| --- | ---: | ---: | --- | ---: | ---: | --- |
| 1 | 200 | 25 | Success | 0 | 30.520 | Yes |
| 10 | 188 | 24 | Success | 0 | 23.381 | Yes |
| 20 | 720 | 90 | Limit reached | 1 | 83.520 | Yes |
| 30 | 720 | 90 | Limit reached | 2 | 84.975 | Yes |
| 40 | 720 | 90 | Limit reached | 2 | 89.049 | Yes |
| 49 | 185 | 24 | Success | 0 | 24.046 | Yes |

That is **3/6 successes**, 343 genuine model requests, 2,733 control ticks and
68,325 physics steps. All resulting 45-word integration states replay exactly.
The outcomes measure this fixed recorded-scene, nominal-home, fixed-noise SDK
profile. They are neither NVIDIA's benchmark score nor a held-out-training
accuracy claim. Across the 343 requests, mean model wall time was 0.9475 s
and mean GPU time 0.6410 s; six episode loops took 335.49 s for 136.65 s of
simulated action time, confirming this is a paused-simulation profile.
Three limits reached remain failures of the task outcome; exact
replay does not turn them into task successes.

The four additional experiments run **16-tick prefixes**, rather than full task
evaluations. All resume admitted model control and replay their 400 physics
steps exactly:

| Injected condition | Model requests | Pose-hold ticks | Refused replies/chunks | Clipped torque channels |
| --- | ---: | ---: | ---: | ---: |
| Missing initial model observation | 2 | 1 | 0 | 0 |
| One-tick delayed reply | 3 | 1 | 1 | 0 |
| Decoded coordinate 1.5 | 3 | 1 | 1 | 0 |
| One observed 500 Nm bias spike | 2 | 0 | 0 | 7 |

These prefixes measure bounded command handling and recovery of the request
schedule. They do not measure task completion after a fault. All torque controls
remain within the pinned seven motor limits, and finger controls remain within
the two source ranges.


The declared fault scenarios are a missing initial observation, a one-tick
late reply, an out-of-limit decoded coordinate, and a finite 500 Nm bias spike
in the first controller sample. They exercise supervised hold, stale reply
refusal, atomic command admission and motor clipping. A fault scenario is an
owned experiment input; it is not a claim that arbitrary sensor failures or
hardware disturbances are handled.

A source-framework closed-loop comparison now covers demos 20, 30 and 40
under the same reset, noise and controller profile. All three reference episodes
reach the 720-tick limit, with exact controls-only replay. The
[reference receipt](../fixtures/robotics/libero-panda/framework-baseline-qualification.json)
records all 270 actual MPS forwards. It uses the pinned official Transformers
backbone and independently authored source-equation head under the F32 profile,
with the same Coax preparation, decoding and controller; it is development
tooling, not a Python product fallback or NVIDIA BF16 benchmark. The first
genuine reference frame matches all 5,280 frozen normalized words exactly.
The reference also times out under this controller/reset profile. Matching
outcomes do not establish numerical identity or locate a failure cause. The four failed visual
hidden diagnostics also remain a concrete numerical improvement target. The owned physical solver, direct parameter recovery and function transforms
now have [their own](ch109-learn-a-physical-system.md)
[qualified](ch111-recover-the-physics-from-the-motion.md)
[chapters](ch107-transform-functions-not-just-tensors.md). A matching own robot
backend and its compiler/control guarantees remain [roadmap work](PLAN.md). A real deployment
also needs a measured asynchronous deadline profile, hardware-specific state
and safety contracts, and qualified recovery beyond this simulator experiment.

## Measure state and force sensitivities on the owned plant

The [new runnable example](../examples/own-plant-sensitivities.coax) connects
chapter 107’s generic function calculus to chapter 109’s RK4 field. It augments
position/velocity with a force coordinate whose derivative is zero, so the
action stays held through all four stages. A 29-node permanent graph can then
supply the step, its 3×3 state/action Jacobian and a scalar control cost’s three
gradient and nine Hessian components. JVP/VJP/HVP and shared-context `vmap`
use the same generic execution machinery.

This is an explicitly qualified F32 algebraic realization. Formal partials
are F64 at stored F32 primals; matrix/time/target/weights stay fixed while q/v/u
vary. Coordinate scales 1m, 1m/s and 1N make the weighted error dimensionless.
[The API/scope](../docs/physics/oscillator-functions.md) specifies exact packed
input spans and the independent polynomial comparison. Chapter 111’s recovered
`OsParams` are compatible input context; parameter recovery remains a separate
qualified F64 path.

```shell
coax/bin/coax run psionic/examples/own-plant-sensitivities.coax
coax/bin/coax run psionic/examples/own-plant-sensitivities.coax --release
psionic/check.sh --only physics/oscillator-functions
```

Across 45 frozen points and 18 batches, 180 F32 primal values match the independent
forward calculation exactly; 1,485 formal values agree within 4.45e-16. The F32
step differs from the physical F64 state by at most 5.57e-8 and its Jacobian by
1.08e-8. All 202 root mutants die and each native host build mode passes 2,592
assertions with identical result bytes. The [receipt](../fixtures/physics/oscillator-functions/qualification.json)
retains the frozen budgets, force finite differences and mode distinctions.

Actual physical execution retains `os_step`’s next-state admission. This graph
models the smooth planning interior; bounds refusal, clipping and contacts
are outside its derivative contract. These scalar sensitivities do not qualify
a GR00T embodiment or the matching own robot backend. The [remaining sequence](../docs/physics/owned-control-loop.md)
keeps that integration separate from these completed numerical building blocks.

## Checkpoint the owned smooth plant

The [owned-oscillator replay component](../docs/physics/oscillator-replay.md)
now saves a completed RK4 boundary, its parameters, dt, global tick and only
forces already accepted. Acceptance has a bounded FIFO and commits atomically:

<!-- source: psionic/lib/physics/oscillator-replay.coax accept -->
```jai
or_accept :: (s:OrSnapshot,forces:[]f64)->OrResult #export_c {
  status:=or_snapshot_status(s);if status!=OrStatus.ok {return OrResult.{status=status,snapshot=s};}
  if forces.count<0 || forces.count>128-s.pending {return OrResult.{status=OrStatus.capacity,snapshot=s};}
  if forces.count>OR_TICK_MAX-s.tick-s.pending {return OrResult.{status=OrStatus.range,snapshot=s};}
  for force:forces {if !os_finite(force) {return OrResult.{status=OrStatus.number,snapshot=s};}if abs(force)>2 {return OrResult.{status=OrStatus.force,snapshot=s};}}
  next:=s;for i:0..<forces.count {next.forces[s.pending+i]=forces[i];}next.pending+=forces.count;return OrResult.{snapshot=next};
}
```

A 1,312-byte versioned record includes solver/numeric/target/build/run identities,
canonical unused lanes and a tagged checksum. Restore requires the expected
context and publishes one complete admitted snapshot. Later input acceptance
remains external: the demonstration restores mid-run, resupplies the recorded
later accepts, and reproduces the uninterrupted physical state exactly.

The [qualification](../fixtures/physics/oscillator-replay-qualification.json)
records 24 checkpoints across 980 transitions, including a 720-tick episode and
near-limit global ticks. All three native modes pass 20,733 assertions each;
1,312 byte corruptions and 61 semantic faults refuse atomically. State/control
traces match between modes; their distinct executable hashes intentionally
produce distinct snapshots, and all six cross-build restores refuse. First
divergence reports tick, field and source operation, with applied force compared
before state so signed-zero input changes stay observable.

```shell
psionic/check.sh --only physics/replay
psionic/check.sh --only physics/replay-native
```

The first command is the portable Coax unit gate; the second explicitly runs
the qualified Darwin-arm64 native profile. This component owns one smooth
plant only. It does not snapshot the robot, policy, external producer or Verse,
and supplies no typed-clock, compiler-bounded or cross-architecture guarantee.

## Give the owned plant a clock and a schedule

The [step-clock example](../examples/step-clock.coax) now runs the owned
oscillator under a nominally typed runtime profile. Simulation instants,
step durations, steady instants/durations and civil labels are distinct
Coax types. Integer simulation steps drive event order: physics/control
intent at 500Hz and goal updates every 25 steps at 20Hz. Explicit adapters
supply rounded F64 seconds and the existing SDK time check.

Model demand follows eight consumed rows. Normal request origins are
0,200,400; one deferred reply followed by a hold row reanchors them to
0,25,225. Paused inference can outlast a supplied steady deadline while its
simulation origin stays unchanged. The example preserves capture before
fresh sample and requires control/gripper intent on every physical step.
The scalar plant supplies no actual gripper or pretrained robot semantics.

The [API and scope](../docs/physics/step-clock.md) specify the phases and
clock domains. Ordinary admission refuses atomically; a bad integration
receipt returns a halted cursor even with a non-ok status. Callers must
adopt that returned state. Five compile-fail fixtures reject domain mixing.
Across four 18,000-step scenarios, 72,000 event records match an independent
integer/Fraction oracle. Each debug, release and sanitizer build passes
1,808,302 native assertions; all 329 root mutants die. The
[receipt](../fixtures/physics/step-clock-qualification.json) binds the source,
proof and retained original oscillator evidence.

```shell
coax/bin/coax run psionic/examples/step-clock.coax
psionic/check.sh --only physics/step-clock
psionic/check.sh --only physics/step-clock-native
```

The first physics gate is portable Coax checks plus the example; the second
explicitly adds the qualified native profile. This foundation does not
implement compiler clock syntax, R14 bounds, a wall watchdog, generic sensor
maximum ages, complete snapshots or the Panda/Verse migration.

## Gate the genuine adapter with paired clocks

The [new Panda sidecar](../docs/physics/panda-runtime-clock.md) pairs the runtime
clock with the existing chunk supervisor. Its own last-forward stamp advances
only after an actual forward effect receipt. Capture cannot silently borrow
a stamp manufactured from the current tick. Request, acceptance, selection,
fresh sample, control intent, one SDK step and row commit are cross-checked.
The old action tick stays unchanged until its 25 trace records have been encoded;
physical steps are a separate counter. Partial effects halt both cursors.

The [qualification](../fixtures/robotics/libero-panda/panda-clock-qualification.json)
covers two 250-step SDK prefixes: normal demo 001 and delayed-first-reply demo 000.
All 22,500 newly measured post-step F64 words and 500 PTR1 records match the
original recorded traces exactly; 250 cached/fresh observation words and 40 request
F32 inputs are exact. The 90 reset words in its artifact are authenticated source
input bytes, not another initial-state measurement. These runs use recorded
actuator controls and explicitly named command fixtures, with declared unused
synthetic tails; later complete NN replies cannot be recovered from PTR1.
They do not initialize the neural provider or recompute controller arithmetic.

Eight new source checks and 251 root mutants pass. The portable paired ABI
passes 1,061 checks per native mode. The actual modified host header also runs
normal/delayed and seven fault cases in debug, release and sanitizer modes;
failed effects produce no completed ledger. Native state/controller/trace
proofs outside the changed path are retained.

```shell
psionic/check.sh --only robotics/panda-clock
```

The provider build wrappers now use the new aggregate, and their hooked C path
compiles. The original full learned-policy episode records remain historical
with their original artifact hashes. A fresh whole-NN run through these hooks,
new RGB comparison, actual wall watchdog, full snapshots, compiler R13/R14 and
generic R18 supervision remain separate qualification work. The direct SDK
prefix command and exact audited inputs are documented in the sidecar notes.

## Give positions and displacements different types

The initial [Coax spatial profile](../../coax/SPATIAL.md) now provides
`Point3(Frame,Unit,R)` and `Vector3(Frame,Unit,R)` on native C. A position minus
a position yields a displacement; a position plus a displacement yields a
position. Mixing nominal frames, units or carriers is refused. A frame that
requires F64 cannot construct an F32 point or vector.

The first small world consumer is the floating-origin camera. Its public
`SpaceCam` and `D3` layouts stay the same. At the existing world-metres boundary,
it explicitly constructs two F64 world points and subtracts them before
returning raw coordinates. Nothing narrows the astronomical positions first.

<!-- source: verse/spacecam.coax spacecam-relative -->
```jai
spacecam_relative :: (c: SpaceCam, p: D3) -> D3 {
  world: Point3(SpaceCamWorld, spacecam_units.Metre, f64);
  world.x = p.x; world.y = p.y; world.z = p.z;
  origin: Point3(SpaceCamWorld, spacecam_units.Metre, f64);
  origin.x = c.center.x; origin.y = c.center.y; origin.z = c.center.z;
  displacement := world - origin;
  return d3(displacement.x, displacement.y, displacement.z);
}
```

The changed camera checks keep the five-AU hull visible and retain displacement
in all three coordinates. A separate native oracle checks 1,536 coordinates
per debug, release and sanitizer mode against direct F64 subtraction, with
exact non-NaN bits and NaN classification. It also checks the unchanged public
layouts and the actual world's F32 refusal. The
[consumer receipt](../../coax/tests/spatial/spacecam-qualification.json) keeps
this evidence separate from the compiler's aggregate/operator qualification.

This is an explicit assertion at a raw boundary. It does not prove incoming
coordinate provenance or migrate Panda's poses, add frame transforms or bind a
point to a clock/epoch. Components remain explicit raw escapes. Generated
operator helpers can remain as debug calls; the spatial profile records
measured release code and its relocation limits. Mutation traversal retains
authored input expressions. The later binary source-token profile below also
qualifies a bounded set of type-preserving operator changes. Full R12 remains
open.

## Check the source operator after lowering

The compiler implements point/vector arithmetic by generating ordinary private
procedures. A source `point + displacement` therefore becomes a call before
mutation checks run. Retaining the authored token lets Coax choose a valid
alternative: point-minus-displacement has the same result type. The actual
mutator redirects that one source call and preserves its operands. Other source
calls that share the ordinary helper keep their own meaning.

The [token witness](../../coax/tests/spatial/operator-native.md) qualifies
`+/-` for point/vector and vector/vector pairs, and `*/` for vector/raw pairs,
in both carriers. All twelve supported fixture mutants are killed; independent
native execution checks 2,340 coordinates across O0, O2 and fail-fast ASan/UBSan.
Apply changes one callee; undo restores the emitted C/header exactly. Separate
worker, shard, selective-owner, weak-check and operand-order checks exercise
the actual mutation runner.

Point differences, vector-plus-point and scalar-left multiplication have no
paired alternate with the same result type in this profile. Unary negation
is unsupported. Those sites have separate inventory, not successful kills or
equivalence waivers. Alternate preparation runs only for mutation; exhausting
the shared instance pool refuses before a partial score. This closes a specific
coverage gap without claiming every spatial operation, frame transformation
or compiler-bounded control is qualified.

An [actual Panda consumer witness](../../coax/tests/spatial/panda-shift-token.md)
then changes the unchanged `pfk_shift` source expression from point-plus-vector
to point-minus-vector. Only the new endpoint check runs, and it kills that one
new mutant. Native execution separately observes all thirteen poses: translated
positions negate from the zero root while attitudes stay identical. The old
281-site FK corpus is retained with its historical scope and is not rerun.

## Compute the Panda chain in Coax

A small owned forward-kinematics component now computes the twelve-body Panda
chain through the hand and grip site from seven joint coordinates. Its fixed
transforms and seven local-Z hinges are manually bound to the authenticated
demo0 source model. This advances our own backend without replacing the
simulator's dynamics, contact solver, mass matrix or bias forces.

The [runnable example](../examples/panda-kinematics.coax) declares radian-valued
coordinates and receives world positions as `Point3(PfkWorld,Metre,f64)`.
Quaternion and rotation arrays remain explicit untyped attitude boundaries;
these are not compiler-proven frame-chain transforms.

<!-- source: psionic/examples/panda-kinematics.coax coordinates -->
```jai
    q:[7]pfku.Quantity(pfku.Radian,f64); // Explicit radian zero coordinates.
    poses:[1]PfkOutput;
    #assert pfk_forward(q[..],poses[..])==PfkStatus.ok;
    hand:=poses[0].links[9];grip:=poses[0].grip;
    print("hand BODY xyz (um): % % %\n",panda_example_um(hand.position.x),panda_example_um(hand.position.y),panda_example_um(hand.position.z));
    print("grip SITE xyz (um): % % %\n",panda_example_um(grip.position.x),panda_example_um(grip.position.y),panda_example_um(grip.position.z));
    separation:=hand.position-grip.position;
    #assert abs(separation.z-0.097)<2e-12;
    print("hand-to-grip world Z separation (um): %\n",panda_example_um(separation.z));
```

The zero-angle query makes the geometry easy to audit. The hand height is
`0.912 + 0.333 + 0.316 + 0.384 - 0.1065 = 1.8385` metres. The grip site extends
another `0.097` metres down, to `1.7415`; both have world X `-0.75 + 0.088 =
-0.662`. The example rounds only its display to integer micrometres. Its
[golden transcript](../fixtures/panda-kinematics.out) was written from these
relations before execution. Joint4 at zero lies outside its source range
`[-3.0718,-0.0698]`; this is a kinematic coordinate example, not an actuator-valid
home or a change to the controller's declared home.

The hand BODY orientation and grip SITE orientation also differ. The fixed
gripper attachment rotates their X axes by a quarter turn. A row-major matrix's
first column expresses that local X axis in world coordinates:

<!-- source: psionic/examples/panda-kinematics.coax attitudes -->
```jai
    // Column zero is each local +X axis expressed in world coordinates.
    cosine:=hand.rotation[0]*grip.rotation[0]+hand.rotation[3]*grip.rotation[3]+hand.rotation[6]*grip.rotation[6];
    #assert abs(cosine)<2e-12;
    print("BODY/SITE X-axis cosine (ppm): %; attitudes differ\n",cast(int)floor(cosine*1000000+0.5));
```

The FK API admits exactly seven finite radian values with absolute magnitude
at most 100 and one output record. These conservative numerical bounds are
separate from simulator joint-limit dynamics. Failure publishes no pose:

<!-- source: psionic/examples/panda-kinematics.coax refusal -->
```jai
    before:=poses[0];q[0].value=101;
    #assert pfk_forward(q[..],poses[..])==PfkStatus.range;
    for i:0..<12 {#assert panda_example_same_pose(poses[0].links[i],before.links[i]);}
    #assert panda_example_same_pose(poses[0].grip,before.grip);
    print("101 rad: range refusal; all 13 poses unchanged\n");
```

The helper compares all 208 public F64 words, including signed zero, across the
twelve link poses and grip pose. Source-level checks also refuse metre-valued
joint inputs and positions from another nominal world. Native C erases the
radian wrapper and stores each position as three F64 values; a native caller
still declares the physical meaning and supplies valid backing storage.

```shell
coax/bin/coax run psionic/examples/panda-kinematics.coax
coax/bin/coax run psionic/examples/panda-kinematics.coax --release
```

The [source profile](../fixtures/robotics/panda-fk/source-profile.json) and
[qualification receipt](../fixtures/robotics/panda-fk/qualification.json) keep
source constants, reference scope and native compilation distinct. Eleven
Coax check groups pass, and all 281 candidate-emitted mutation sites in the
three new FK roots are killed. That historical count predates source spatial
token mutation and is retained with its original scope. The new token evidence
below is separate. Authored operand traversal is another compiler check.
Independent analytic and numerical checks exercise the spatial operations
themselves.

The native corpus has 158 configurations and 1,307 ABI/admission assertions per
mode. An independent stdlib homogeneous-matrix formulation checks 24,648 values;
a genuinely new seven-configuration forward-only MuJoCo 3.3.1 recording checks
1,428 body/site values; seven retained fresh controller samples check 84 grip
endpoint values. The frozen absolute budget is 2e-12. The largest SDK error is
4.33e-15. No old SDK episode, learned model or controller gate is rerun.

The qualified Darwin-arm64 host profile uses separate scalar libm `sin` and
`cos` calls, with `-fno-builtin-sin -fno-builtin-cos -fno-fast-math
-ffp-contract=off`. Its complete outputs agree byte for byte across debug,
release and ASan/UBSan. The default optimized build fused those intrinsics
into `___sincos_stret`: every numerical comparison passed, but 792 words differed
from debug by at most 1.56e-15. The
[failed strict-byte record](../fixtures/robotics/panda-fk/default-fused-diagnostics.json)
remains unchanged. The example's rounded, independently known geometry is not
a claim of default-build bit parity, R15 or cross-target determinism.

This component supplies a pure pose query, with no simulation clock or sensor
freshness stamp. A cached SDK pose and a newly forwarded pose must still be
compared with the joint coordinates of their actual generation. Robot import, articulated dynamics, full R12 transforms and compiler bounded
control remain subsequent slices. The following sections add the owned geometric
Jacobian and a separate moving-finger extension.

## Differentiate the grip-site geometry

The owned FK query now has a geometric Jacobian: six world-coordinate rows
for seven joint coordinates. The top three rows describe grip-site translation
per radian; the bottom three describe world angular rotation per joint radian.
For joint axis `a`, joint anchor `p` and grip position `g`:

```jai
J_linear[:,i]  = a[i] cross (g - p[i]) // numeric derivative per radian
J_angular[:,i] = a[i]                 // radian/radian ratio
```

`pj_forward` evaluates its own FK snapshot before projecting those columns.
It reuses FK's typed radians, finite/range admission and status; refusal
preserves the caller's Jacobian. Singular configurations are valid geometry:
this API neither inverts the matrix nor admits a motor command. Its exported
native entries accept coordinates, not caller-supplied pose records. The
non-exported projection helper performs arithmetic and establishes no input
provenance guarantee.

[`PjOutput`](../lib/robotics/panda-jacobian-types.coax) keeps separate
21-value blocks: `Quantity(Metre/Radian,f64)` for translation and
`Quantity(UnitOne,f64)` for the angular ratio. Raw attitude matrices remain
convention-labelled boundaries; this does not complete frame-chain typing.

[The qualification](../fixtures/robotics/panda-jacobian/qualification.json)
records 47/47 killed new-root mutants and 62 native configurations, with
770 ABI/admission assertions per debug, release and fail-fast sanitizer mode.
All 2,604 output words per mode agree byte for byte under the explicit
scalar-libm profile. All 294 retained SDK Jacobian values pass the frozen
2e-12 absolute budget; maximum difference is 1.11e-15.

An independent homogeneous-matrix FK supplies central position differences
and a world-left SO3 rotation difference. The preselected step is 1e-5
radians, with absolute budget 3e-8; largest linear/angular errors are
3.27e-11 and 1.57e-11. The full 1e-3 through 1e-7 sweep is retained: the
coarser 1e-3 result exceeds the primary budget and does not replace the chosen
step. These are derivatives of the smooth kinematic map, not of floating
rounding, refusal or source soft-limit dynamics. The reference loader explicitly
rejects nonfinite expected values before computing maxima; five new guard
controls and all 10,206 cached expected values pass without regenerating the
numerical corpus. No SDK calls or old controller gates run for this slice.

```shell
psionic/check.sh --only robotics/panda-jacobian
```

## A joint rate becomes a world speed

The zero-angle posture makes one derivative easy to predict. The grip site is
0.088 metres from joint 1's world Z axis in world X. A positive joint-1 rate of
0.5 radians per second therefore gives world Y speed `0.088 × 0.5 = 0.044`
metres per second. The angular world Z speed is 0.5 radians per second.

The quantities matter. `linear[7]` is row Y, column joint 1, with unit
Metre/Radian. Multiplying it by Radian/Second cancels the same physical Radian
factor and produces Metre/Second. `angular[14]` is the world Z/joint-1 angular
ratio `UnitOne`; multiplying it preserves the rate's Radian/Second unit. There
is no global rule that silently erases angles.

<!-- source: psionic/examples/panda-jacobian.coax rates -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);jacobian:[1]PjOutput;
 #assert pj_forward(q[..],jacobian[..])==PfkStatus.ok;
 joint_rate:pfku.Quantity(UnitQuotient(pfku.Radian,pfku.Second),f64);joint_rate.value=0.5;
 // Joint1 is column0: world Y is row1, world angular Z is row2.
 grip_y:pfku.Quantity(UnitQuotient(pfku.Metre,pfku.Second),f64)=jacobian[0].linear[7]*joint_rate;
 world_z:pfku.Quantity(UnitQuotient(pfku.Radian,pfku.Second),f64)=jacobian[0].angular[14]*joint_rate;
 #assert panda_jacobian_micro(grip_y.value)==44000 && panda_jacobian_micro(world_z.value)==500000;
 print("joint 1 rate (urad/s): %\n",panda_jacobian_micro(joint_rate.value));
 print("grip SITE world Y (um/s): %\n",panda_jacobian_micro(grip_y.value));
 print("world angular Z (urad/s): %\n",panda_jacobian_micro(world_z.value));
```

The display helper rounds only for presentation. The program prints 500000
`urad/s` for joint 1, 44000 `um/s` for grip-site world Y, and 500000 `urad/s`
for world angular Z. The prewritten golden follows the lever arm above;
these are not numbers inferred from the program after running it.

```shell
coax/bin/coax test psionic/examples/panda-jacobian.coax --only 'Panda Jacobian example'
coax/bin/coax run psionic/examples/panda-jacobian.coax
coax/bin/coax run psionic/examples/panda-jacobian.coax --release
```

This is a local geometric calculation, not a motor command. At the all-zero
posture joint 4 is outside its source physical soft range
`[-3.0718,-0.0698]` radians. A geometric derivative can exist there without that
posture being safe or admitted for control. The target is the grip SITE in the
world basis; the angular block is not a derivative of the model's hand-BODY
axis-angle observation coordinates.

The example's two own checks and debug/release transcripts pass on the actual
merged 0.45 compiler. Their stdout matches `psionic/fixtures/panda-jacobian.out`
exactly. The unchanged Jacobian mutation/native/FD/SDK corpora were not rerun.

## Extend the tree through the moving fingers

The arm query now has a separate four-body extension. `PffOutput` contains the
unchanged `PfkOutput` and four finger poses, ordered left finger, left tip,
right finger and right tip. The two slide inputs are metre-valued quantities;
the seven arm coordinates remain radians. A source-level attempt to swap
those units fails before native code is generated.

Both finger BODY attachments use the original decimal quaternion
`(.707107,0,0,.707107)`, normalized by the owned FK math. That fixed quarter
turn comes before each local +Y slide, so positive slide displacement moves
along parent -X. The right finger has a separately rotated GEOM; copying that
rotation as its BODY pose would give the wrong tree.

<!-- source: psionic/examples/panda-fingers.coax coordinates -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64); // Coordinate example, not an actuator home.
 slides:[2]pfku.Quantity(pfku.Metre,f64);slides[0].value=0.013;slides[1].value= -0.027;
 poses:[1]PffOutput;#assert pff_forward(q[..],slides[..],poses[..])==PfkStatus.ok;
 print("slides (um): % %\n",pff_example_um(slides[0].value),pff_example_um(slides[1].value));
 print("tip-to-tip distance (um): %\n",pff_example_um(pff_example_distance(poses[0].fingers[1],poses[0].fingers[3])));
```

The tip offsets add .0085 metres on each side. For the source coordinate
ranges, the tip-to-tip distance is `q_left - q_right + .017` metres. The first
query therefore gives .057 metres. This coordinate relation was used to write
the golden transcript before execution; the program only rounds its display.

Changing the left slide leaves the arm and grip site unchanged. The second
query gives `.04 - (-.027) + .017 = .084` metres:

<!-- source: psionic/examples/panda-fingers.coax branches -->
```jai
 original_arm:=poses[0].arm;slides[0].value=0.04;
 #assert pff_forward(q[..],slides[..],poses[..])==PfkStatus.ok;
 pfk_same_output(original_arm,poses[0].arm);
 print("left slide to 40000 um: tip distance % um; arm unchanged\n",pff_example_um(pff_example_distance(poses[0].fingers[1],poses[0].fingers[3])));
```

The numerical profile admits exactly seven finite arm coordinates within
+/-100 radians, exactly two finite slide coordinates within +/-1 metre, and
one output record. It checks shape first, arm admission second and slides in
index order last. These conservative numerical bounds do not enforce source
soft limits or translate a normalized gripper command. All-zero arm coordinates
are an explanatory posture; joint4 lies outside its source soft range.

A rejected slide preserves all 272 F64 words across the seventeen public poses:

<!-- source: psionic/examples/panda-fingers.coax refusal -->
```jai
 before:=poses[0];slides[1].value=1.0000000000000002;
 #assert pff_forward(q[..],slides[..],poses[..])==PfkStatus.range;pff_same(poses[0],before);
 print("outside +/-1m: range refusal; all 17 poses unchanged\n");
```

```shell
coax/bin/coax run psionic/examples/panda-fingers.coax
coax/bin/coax run psionic/examples/panda-fingers.coax --release
psionic/check.sh --only robotics/panda-fingers
```

The [source profile](../fixtures/robotics/panda-fingers/source-profile.json)
and [qualification](../fixtures/robotics/panda-fingers/qualification.json) bind
the fixed tree and numerical evidence. Both new authority roots have zero
survivors: 45 profile mutants and 61 branch/admission mutants. The native
corpus has 153 configurations, including four inclusive slide boundaries,
with 1,753 ABI/admission/I/O assertions per debug, release and ASan/UBSan mode.
All complete outputs agree byte for byte under the previously declared
Darwin-arm64 scalar-libm profile. This does not establish default fused-libm
or cross-target bit identity, or complete lowered spatial-token mutation.

Independent homogeneous matrices check 31,212 values; literal parent-relative
motion and attitude relations check 1,836 and 5,508 more. Five genuinely new
MuJoCo 3.3.1 reset/forward queries check 1,340 all-body/grip values; maximum
SDK error is 1.78e-15 against the frozen 2e-12 absolute budget. All 112 compiled
local-pose words match their source-derived normalized values exactly. The
collector performs no integration, rendering, controller or learned inference.
The default gate uses the committed data and needs no SDK installation.

Compiled mass, COM, inertia-frame and armature facts were saved in that same
capture, along with a separately versioned nine-joint mass/bias reference.
Those are inputs for a future owned CRBA/RNEA slice, not an owned dynamics
qualification. The moving fingers and tips contribute .22 kg in this particular
simulation model, so even the arm mass block depends on their slide positions.
These source-file parameters are not manufacturer measurements. The pure FK
query also supplies no sensor generation or clock stamp; reference poses and
coordinates must describe the same fresh state.

## What mass means for nine different coordinates

We can now ask how much kinetic energy the owned Panda geometry assigns to
joint motion. Seven coordinates are radians and two are metres. That makes
the answer a generalized mass matrix rather than one weight in kilograms.
The implementation is a direct sum over the sixteen source robot bodies:
their COM translation, rotation about their own inertial frames, and the nine
source armature coefficients. It computes a matrix and gravitational
potential gradient; it does not advance physics.

The exact original simulation inertials matter. The fixed gripper has an
anisotropic inertia whose principal axes have a different orientation from
its BODY frame. Each finger and fixed tip moves mass with its slide. Omitting
those descendants would change even the seven-by-seven arm block. The source
numbers are deliberately simple simulation-file parameters, not measurements
of a physical Franka.

For body b, let its world COM be c_b, its COM linear Jacobian Jv_b and its
angular Jacobian Jw_b. Rotate the principal inertia from its declared inertial
frame into the world. The coefficient calculation is:

```jai
M[i,j] = sum_b(mass_b * dot(Jv_b[i], Jv_b[j])
             + dot(Jw_b[i], I_world_b * Jw_b[j]));
M[i,i] += armature[i];
gravity[i] = -sum_b(mass_b * dot(Jv_b[i], world_gravity));
```

These equations use an explicit one-radian generator convention. Actual
Coax quantities keep radians as an angle dimension: the hinge/hinge block is
kg·m²/rad², hinge/slide and slide/hinge are kg·m/rad, and slide/slide is kg.
Hinge gravity is energy per radian; slide gravity is force. The four typed
blocks materialize all 81 entries. Their names do not silently turn angles
into dimensionless numbers or claim that raw attitude arrays implement a
generic frame system.

The pure public query takes seven radian coordinates, two metre coordinates
and one output. It calls owned finger FK internally, constructs the entire
metric privately and publishes once. Bad shapes or numbers preserve the
caller's output. It inherits the numerical ±100rad/±1m domain; that is a
mathematical query domain, not permission to send such coordinates to motors.
The zero pose used below has joint4 outside its physical source soft range.
`PmProfile` and `pm_sum_owned` remain source-visible low-level Coax helpers
with trusted valid-pose/profile preconditions. They do not admit or authenticate
arbitrary caller PODs. The exported fixed-source `pm_forward` constructs its
own profile; omitting a helper from the C header does not seal the Coax module.

For a simple independent calculation, move only the left slide at 20mm/s.
Its effective diagonal is .1kg finger +.01kg tip +1kg source armature:
1.11kg. Half that times the squared speed is .000222 joules. The right slide
belongs to another branch, so its direct cross-slide metric entry is zero.

<!-- source: psionic/examples/panda-inertia.coax kinetic -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);slides:[2]pfku.Quantity(pfku.Metre,f64);metric:[1]PmOutput;
 #assert pm_forward(q[..],slides[..],metric[..])==PfkStatus.ok;
 // Only the left slide moves: .1kg finger +.01kg tip +1kg armature.
 speed:pfku.Quantity(UnitQuotient(pfku.Metre,pfku.Second),f64);speed.value=0.02;
 energy:=convert_unit(PmJoule,metric[0].slide_slide[0]*speed*speed)*0.5;
 #assert panda_inertia_micro(metric[0].slide_slide[0].value)==1110000 && panda_inertia_micro(energy.value)==222;
 print("left slide effective mass (microkg): %\n",panda_inertia_micro(metric[0].slide_slide[0].value));
 print("left slide kinetic energy at 20mm/s (microjoules): %\n",panda_inertia_micro(energy.value));
 print("right-to-left slide coupling (microkg): %\n",panda_inertia_micro(metric[0].slide_slide[2].value));
```

Run the checked example and the new focused native query with a qualified
compiler:

```shell
coax/bin/coax run psionic/examples/panda-inertia.coax
coax/bin/coax test psionic/tests/robotics/panda_inertia_unit_checks.coax --only "Panda inertia typed blocks"
COAX=coax/bin/coax bash psionic/tests/robotics/panda_inertia_check.sh /tmp/new-panda-inertia-proof
```

The same five original SDK forward captures already supplied a nine-by-nine
mass block and nine bias words each. Their velocities were all zero, making
their bias the gravity target for this pinned model. We reuse all 405 mass
and 45 gravity coefficients: maximum errors are 4.44e-15 and 7.11e-15 under
the bounds fixed before comparison. No new SDK, renderer or neural inference
runs here.

A separate homogeneous-matrix reference differentiates COM positions and
world-left rotations, then evaluates kinetic energy in principal coordinates
and gravitational potential differences. It does not copy the product's
cross-product Jacobian or world-inertia implementation. Its preselected
h=1e-5 passes the frozen mass/gravity difference budgets. The complete step
sweep stays in the record, including larger/smaller steps that miss those
primary budgets. These derivatives describe the smooth geometric model;
they do not differentiate a rounded floating-point program or admission.

Nine new check groups, 338 focused mutants with zero survivors, four source
unit refusals and three native modes qualify this query. The native 57-case
outputs match byte for byte under the stated Darwin-arm64 scalar-libm profile.
The first sanitizer attempt found a pointer bug in the new C test harness;
its failed record is retained, and the repaired harness passes. Numeric
references were reused by exact identity rather than recomputed.

This mass-and-gravity slice still lacks velocity-dependent bias, passive and
actuator force, contact, constraints and integration. The controller's existing
nonzero-velocity bias snapshots are not gravity-only targets. Combining these
pieces into a replacement simulator requires those later checks, synchronized
finger state and a new whole-loop qualification. See the
[qualification receipt](../fixtures/robotics/panda-inertia/qualification.json) for exact source,
reference, compiler, numerical-profile and failure-history records.

## Forces for a requested acceleration

A mass matrix tells us how acceleration enters the force balance. A moving
robot also needs velocity-dependent and gravitational terms. Our next pure
query computes all three for the fixed source model:

```jai
required_force(q, v, a) = M(q)*a + velocity_bias(q, v) + gravity(q);
```

The requested acceleration is an input. The query does not integrate the arm,
choose that acceleration, resolve contact or send torques to hardware.

The source robot has 16 selected bodies. Base and link 0 are fixed before the
nine coordinates. The 14 remaining descendants move with the arm or slides;
this includes the rigid hand/gripper and all four finger bodies, not just the
nine bodies with joints. Massless frames still carry geometry but add no
wrench. Exact simulation inertials and their principal-frame orientations
remain source constants rather than manufacturer identification.

We propagate physical world velocity and acceleration from a parent's BODY
origin to the next origin. If r is the current world offset, the fixed-part
motion is:

```jai
v_child = v_parent + omega_parent cross r;
a_child = a_parent + alpha_parent cross r
         + omega_parent cross (omega_parent cross r);
```

A hinge adds `axis*joint_rate` to angular velocity. Its angular acceleration
also needs `axis*joint_acceleration` and the moving-axis contribution
`(omega_parent cross axis)*joint_rate`. A slide adds axis times slide rate to
linear velocity, axis times slide acceleration to linear acceleration, and
the Coriolis term `2*(omega_parent cross axis)*slide_rate`. Omitting that factor
or using a BODY axis as though it were already world-oriented changes bias.

Each COM then adds its rotated offset's angular-acceleration and centripetal
terms. Its required force is mass times COM acceleration minus gravity; its
torque about the COM is `I_world*alpha + omega cross (I_world*omega)`. We project
these wrenches through the world COM columns and sum every descendant. The
nine source armatures add `armature*joint_acceleration` once. This is world
Newton–Euler body arithmetic with Jacobian projection; we are not claiming a
generic importer or complete CRBA/RNEA simulator.

The public typed query accepts seven radian positions and two metre positions,
the corresponding seven/two rate and acceleration spans, and one output.
All shapes are checked first; positions, rates and accelerations then follow
their documented order, each coordinate checking number before range. Finite
numerical domains are ±100rad/±1m,±20rad/s/±2m/s and±200rad/s²/±20m/s². Those
bounds are query admission, not hardware or source soft limits. Refusal leaves
the output intact. Seven energy-per-radian and two force quantities preserve
mixed coordinate dimensions and turn force times rate into watts under the
explicit one-radian generator convention.

Low-level source-visible motion/profile helpers require trusted valid poses
and source constants. Their PODs are writable and unsealed; absence from the
C header does not make them provenance certificates. The exported fixed-source
query constructs its own profile and performs its own admission.

The independent SDK target is its nine-by-nine mass block times the explicitly
chosen acceleration plus qfrc_bias. This is separate from passive damping,
friction/fluid, actuator and contact/constraint forces. Public `mj_rne(...,1)`
returns body inertial force without joint armature acceleration force. The
separately recorded raw RNE therefore needs `armature*a` exactly once; using
qfrc_inverse would introduce different passive/constraint contracts.

Before observing results we froze 24 input rows,15 SDK contexts, tolerances and
the finite-difference sweep. The 15 contexts have 12 nonzero-velocity states and
three zero-rate baselines. One baseline carries previously observed mass/
gravity subfields while providing a NEW matched site Jacobian/velocity/limit
observation. All new measurements use the same forward generation: no physics
step, renderer or neural inference occurs. Fresh PcSample 144 records remain
independent raw reference facts for a later owned-controller adapter.

The second oracle propagates analytic derivatives of full-angle homogeneous
matrices, computes kinetic metrics in principal coordinates, then differences
only that independent metric for Christoffel terms. It does not reuse the
product's quaternion/cross recursion or its owned mass matrix as an oracle.
The primary h=1e-5 passes; the complete 1e-3…1e-7 sweep remains visible, including
the coarse step outside the primary budget. These differentiate the smooth
geometric model, not floating-point rounding or refused inputs.

All 135 SDK force coefficients agree within 2.49e-14 under the predeclared 5e-10
absolute bound. All 216 independent-primary force coefficients agree within
2.97e-10 under 3e-7. Seventeen new Coax groups,250 focused mutants with zero
survivors, source unit checks and three native modes qualify the query. The
native modes serialize the same 216 force words bit for bit under the stated
Darwin-arm 64 scalar-libm profile; that is not cross-target determinism.

A separate cache-only power witness checks
`v dot velocity_bias = .5*v transpose*Mdot*v` using a predeclared directional
time difference. Its maximum discrepancy is 6.80e-10W under 2e-6W, with no repeat
SDK/native/coordinate-FD execution. This connects the gyroscopic and moving-axis
terms to kinetic energy rather than checking dimensions alone.

The [runnable example](../examples/panda-inverse-dynamics.coax) uses the frozen
SDK dense/dense-acceleration state. Its display golden was written from the
independently captured SDK force row before the Coax example. Values are
rounded only for display; the query returns full F64 generalized forces.

<!-- source: psionic/examples/panda-inverse-dynamics.coax inverse -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);w:[2]pfku.Quantity(PidLinearRate,f64);
 a:[7]pfku.Quantity(PidAngularAcceleration,f64);b:[2]pfku.Quantity(PidLinearAcceleration,f64);
 coordinates:=[9]f64.[0.3,-0.4,0.5,-1.6,-0.7,1.2,0.9,0.017,-0.029];
 rates:=[9]f64.[0.7,-0.4,0.3,-0.6,0.5,-0.2,0.1,0.03,-0.02];
 accelerations:=[9]f64.[1.3,-2.1,0.7,-1.1,0.9,0.4,-0.8,0.2,-0.15];
 for i:0..<7 {q[i].value=coordinates[i];v[i].value=rates[i];a[i].value=accelerations[i];}
 for i:0..<2 {s[i].value=coordinates[7+i];w[i].value=rates[7+i];b[i].value=accelerations[7+i];}
 force:[1]PidOutput;#assert pid_inverse(q[..],s[..],v[..],w[..],a[..],b[..],force[..])==PfkStatus.ok;
 for i:0..<7 {print("hinge % (uJ/rad): %\n",i+1,panda_inverse_micro(force[0].hinge[i].value));}
 for i:0..<2 {print("slide % (uN): %\n",i+1,panda_inverse_micro(force[0].slide[i].value));}
```

```shell
coax/bin/coax run psionic/examples/panda-inverse-dynamics.coax
coax/bin/coax run psionic/examples/panda-inverse-dynamics.coax --release
psionic/check.sh --only robotics/panda-inverse-dynamics
```

The [qualification](../fixtures/robotics/panda-inverse-dynamics/qualification.json)
keeps source, input, SDK, compiler and numerical-profile identities distinct.
Forward integration, passive/actuator/contact forces and a whole owned-backend
controller episode remain later work.

## Turn a net force into an acceleration

The owned metric can now answer a new question: given an already assembled
net generalized force, which angular and linear accelerations does it imply?
The new fixed-source query solves `M(q) * a = F_net`. It does not decide what
an actuator command means or subtract gravity and velocity bias on our behalf.

Seven coordinates use radians and two use metres. We first give every
coordinate an explicit basis: one Radian or one Metre. Multiplying the mixed
mass blocks on both sides by those bases gives coefficients in kg·m²;
multiplying the force entries by the same bases gives joules:

```jai
D       = diag(1 Radian, ..., 1 Radian, 1 Metre, 1 Metre);
M_basis = transpose(D) * M * D;       // kg*m²
F_basis = transpose(D) * F_net;       // joules
M_basis * a_basis = F_basis;          // a_basis has Second^-2
acceleration = D * a_basis;
```

Coax owns a new nine-by-nine Cholesky factorization and both triangular
substitutions. Quantity arithmetic performs the basis changes; only the
normalized scalar coefficients cross into raw F64 factorization. The result
crosses an explicit inverse-second-squared boundary and regains its radian or
metre basis. There is no hidden angle erasure or square root of a quantity.

Choose an acceleration of 2m/s² for just the left slide. Its effective diagonal
is .1kg finger +.01kg tip +1kg armature =1.11kg, so the left slide needs 2.22N.
To keep every other acceleration zero, apply the **whole** corresponding mass
column, including its coupled arm forces:

<!-- source: psionic/examples/panda-mass-solve.coax column -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);slides:[2]pfku.Quantity(pfku.Metre,f64);metric:[1]PmOutput;
 #assert pm_forward(q[..],slides[..],metric[..])==PfkStatus.ok;
 wanted:pfku.Quantity(UnitQuotient(pfku.Metre,UnitProduct(pfku.Second,pfku.Second)),f64);wanted.value=2;
 h:[7]pfku.Quantity(PmHingeGravity,f64);f:[2]pfku.Quantity(PmSlideGravity,f64);
 // Prescribe zero arm/right-slide acceleration by applying the WHOLE mass column.
 for i:0..<7 {h[i]=convert_unit(PmHingeGravity,metric[0].hinge_slide[i*2]*wanted);}
 for i:0..<2 {f[i]=convert_unit(PmSlideGravity,metric[0].slide_slide[i*2]*wanted);}
 out:[1]PmsOutput;#assert pms_forward(q[..],slides[..],h[..],f[..],out[..])==PmsStatus.ok;
 print("chosen left slide acceleration (um/s^2): %\n",pms_example_micro(wanted.value));
 print("required left slide NET force (microN): %\n",pms_example_micro(f[0].value));
 print("solved left/right slide acceleration (um/s^2): % %\n",pms_example_micro(out[0].slide[0].value),pms_example_micro(out[0].slide[1].value));
```

The golden transcript was written from that chosen solution and the 1.11kg
source coefficient before execution. The example rounds display values only.
Applying just 2.22N at the slide would generally move the coupled arm as well.

Zero net force means zero inertial acceleration in this API:

<!-- source: psionic/examples/panda-mass-solve.coax net -->
```jai
 for i:0..<7 {h[i].value=0;}for i:0..<2 {f[i].value=0;}
 #assert pms_forward(q[..],slides[..],h[..],f[..],out[..])==PmsStatus.ok;
 for a:out[0].hinge {#assert a.value==0;}for a:out[0].slide {#assert a.value==0;}
 print("zero NET force -> zero acceleration (all 9 coordinates)\n");
```

That does not mean zero actuator force holds a physical robot still. A caller
assembling total dynamics must account for gravity, velocity-dependent bias,
passive and applied forces first. The next component can compose those
contracts explicitly; this query does not infer them from a force's unit name.

The public boundary accepts seven finite radian positions, two finite metre
positions, seven net energy-per-radian forces and two net Newton forces. It
keeps the position query's numerical ±100rad/±1m box and an inclusive force
magnitude limit of 10000 per coordinate. Shape checks precede position and
force admission. Refusal preserves all nine acceleration words:

<!-- source: psionic/examples/panda-mass-solve.coax refusal -->
```jai
 before:=out[0];f[0].value=10000.000000000002;
 #assert pms_forward(q[..],slides[..],h[..],f[..],out[..])==PmsStatus.range;pms_same(out[0],before);
 print("force above 10000: range refusal; all 9 outputs unchanged\n");
```

The raw normalized matrix profile has inclusive 1e8 coefficient and 1e4 force
limits, exact numerical symmetry and Cholesky Schur pivots at least .25.
The bound source model's minimum armature is .714286 plus a positive
semidefinite body metric; the .25 floor is conservative for this model, not
a promise to accept every positive-definite matrix. Source-visible helpers
still have trusted basis/force/profile preconditions and forgeable PODs. Only
the fixed-source forward entry owns its admitted model/force path.

```shell
coax/bin/coax run psionic/examples/panda-mass-solve.coax
coax/bin/coax run psionic/examples/panda-mass-solve.coax --release
COAX=coax/bin/coax psionic/tests/robotics/panda_mass_solve_check.sh /tmp/panda-mass-solve-new
```

The [experiment](../fixtures/robotics/panda-mass-solve/experiment.json) and
[qualification](../fixtures/robotics/panda-mass-solve/qualification.json)
keep earlier component references distinct from the new solver proof.
Two forces at each of 57 previously qualified metric configurations give 114
new contexts. Five retained original SDK matrices times three independently
chosen accelerations add 15; seven analytic raw profiles exercise the numerical
helper. No previous M9/SDK/model gate is rerun.

All 213 new-root mutants are killed. Debug, release and ASan/UBSan each pass
10,028 ABI/refusal assertions and publish the same 1,224 F64 words byte for byte
under the declared Darwin-arm64 scalar-libm profile. Independent row-pivoted
Gaussian elimination differs by at most 3.64e-12; equation residuals reach
9.10e-12. The fixed per-scalar allowances are `2e-10 + 2e-12*abs(reference)`
for acceleration and `2e-9 + 2e-12*abs(force)` for residuals, in the explicit
coordinate basis. All decoded actual/reference words are finite and complete
before comparison. The failed first native fault setup and its repaired test
harness remain recorded; the product and numerical references were unchanged.

These outputs use formal Radian/Second² and Metre/Second² types. A future
inverse-dynamics assembly using nominal acceleration units needs explicit
`convert_unit`, with no loss of physical angle identity. Contacts, force
assembly, integration, compiler bounded control and real hardware remain
separate qualifications. Zero joint angles here are a numerical query, not
an actuator-valid home.

## An applied force becomes acceleration

The net-force solver answers a deliberately narrow question: which acceleration
would a supplied net force produce? A caller normally has an applied generalized
force instead. Gravity and motion already contribute force, so we first subtract
the owned bias:

```jai
M(q) * acceleration = applied - bias(q, rate)
```

The [new public query](../lib/robotics/panda-forward-dynamics.coax) evaluates one
fresh fixed-source FK, builds the full nine-coordinate metric, and evaluates the
owned inverse query at zero acceleration. Its result is the gravity-plus-velocity
bias. Both operands of the subtraction carry the same physical force unit:
energy/radian for a hinge, newtons for a slide. The existing one-radian/one-metre
basis and mass solver then restore angular and linear acceleration quantities.

Applied force and net force each have an inclusive magnitude-10000 numerical
limit. A legal applied endpoint can produce an illegal net force when the bias
opposes it; the query refuses and preserves caller output. It never clips the
force into the admitted box. Those large mathematical limits are separate from
motor command limits. In particular, the controller's finger-position targets
are metres, while this query requires newtons. The separate actuator map below
converts those targets; this forward query does not perform that conversion.

The [runnable example](../examples/panda-forward-dynamics.coax) supplies the
original SDK force from retained context13. The original acceleration was
chosen before any of these queries, so its golden display is independent of the
new forward implementation.

<!-- source: psionic/examples/panda-forward-dynamics.coax pfd-query -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);sv:[2]pfku.Quantity(PidLinearRate,f64);
 h:[7]pfku.Quantity(PmHingeGravity,f64);f:[2]pfku.Quantity(PmSlideGravity,f64);
 coordinates:=[9]f64.[0.3,-0.4,0.5,-1.6,-0.7,1.2,0.9,0.017,-0.029];
 rates:=[9]f64.[0.7,-0.4,0.3,-0.6,0.5,-0.2,0.1,0.03,-0.02];
 // Retained original SDK M*a+bias, experiment context13; not motor targets.
 applied:=[9]f64.[10.97794424270289, -19.859397263891573, 0.9544918198938825, 16.59200521881249, -1.1636592449850924, 2.503706256966014, -0.8899579105213928, 0.9824336240405068, 0.59784382475265];
 for i:0..<7 {q[i].value=coordinates[i];v[i].value=rates[i];h[i].value=applied[i];}
 for i:0..<2 {s[i].value=coordinates[7+i];sv[i].value=rates[7+i];f[i].value=applied[7+i];}
 acceleration:[1]PmsOutput;
 #assert pfd_forward(q[..],s[..],v[..],sv[..],h[..],f[..],acceleration[..])==PmsStatus.ok;
 for i:0..<7 {print("hinge % (micro-rad/s^2): %\n",i+1,pfd_example_micro(acceleration[0].hinge[i].value));}
 for i:0..<2 {print("slide % (micro-m/s^2): %\n",i+1,pfd_example_micro(acceleration[0].slide[i].value));}
```

```shell
coax/bin/coax run psionic/examples/panda-forward-dynamics.coax
coax/bin/coax run psionic/examples/panda-forward-dynamics.coax --release
COAX=coax/bin/coax psionic/tests/robotics/panda_forward_dynamics_check.sh /tmp/panda-forward-new
```

The [frozen experiment](../fixtures/robotics/panda-forward-dynamics/experiment.json)
contains 15 original `M*a+bias` contexts, three zero-applied/zero-rate gravity
contexts, and 18 signed applied-cap contexts. Independent pivoted Gaussian solves
and original-SDK equation residuals compare 297 admitted acceleration words;
33 records admit and 3 refuse at the separate net-force cap. All statuses and 324
output words match byte for byte across debug, release and fail-fast ASan/UBSan.
Each native run passes 4,028 assertions. The 135 original prescribed acceleration
words differ by at most 7.627e-15. Eight new Coax groups/125 cases pass and all
85 actual emitted root mutants are killed; the existing quantity-token lowering limitation remains
explicit, so that score does not claim every authored quantity operator token.

Native evidence uses Darwin arm64 F64 scalar libm, with fast math and contraction
disabled. Unsupported targets are refused rather than counted as skipped passes.
The frozen scalar budgets remain those of the mass solver. Actual and expected
arrays must be complete and finite before maxima. The native witness also
checks all input IEEE classes, count extremes, raw/typed identity, admission
precedence and output overlap with all input views. Source-visible low-level
helpers remain trusted and unsealed.

This is an unconstrained acceleration query. Zero applied force gives
gravity-driven joint acceleration; it does not turn gravity off. Passive forces,
damping, contact, actuator conversion and integration are still absent. It is
not a comparison with the SDK's fully assembled `mj_forward.qacc`, a simulated
step, a controller rollout, compiler-bounded control or hardware execution.

## A controller target becomes an actuator force

The forward query consumes generalized force, but our controller returns two
different kinds of control. Its seven arm values are joint forces; its two
finger values are signed position targets in metres. Passing those metres to
the newton inputs would change their meaning. The [actuator query](../lib/robotics/panda-actuation.coax)
keeps the distinction in its typed API.

For the admitted original source profile, arm controls first clamp to five
±80 and two ±12 ranges. Finger targets clamp to `[0,.04]` and `[-.04,0]`
metres, then their fixed-gain/affine-bias actuator forces clamp to ±20N. The
right target stays signed; there is no extra normalized gripper-command map.
The two products remain separate in source order:

```jai
finger_force = 1000 * clipped_target + (-1000 * joint_position)
```

The result keeps actuator force and JOINT-passive force separate. Source joint
springs are exactly zero, so the latter is viscous damping. The servo's 1000
coefficient belongs to actuation, not that passive spring model. One can inspect
the forces without taking a physics step or issuing a command.

<!-- source: psionic/examples/panda-actuation.coax pa-query -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);sv:[2]pfku.Quantity(PidLinearRate,f64);
 u:[7]pfku.Quantity(PmHingeGravity,f64);target:[2]pfku.Quantity(pfku.Metre,f64);
 coordinates:=[9]f64.[0,0.19634954084936207,0,-2.617993877991494,0,2.941592653589793,0.7853981633974483,0.01,-0.01];rates:=[9]f64.[0.7,-0.4,0.3,-0.6,0.5,-0.2,0.1,0.03,-0.02];controls:=[9]f64.[100,-100,100,-100,100,-20,20,0.05,-0.05];
 for i:0..<7 {q[i].value=coordinates[i];v[i].value=rates[i];u[i].value=controls[i];}
 for i:0..<2 {s[i].value=coordinates[7+i];sv[i].value=rates[7+i];target[i].value=controls[7+i];}
 forces:[1]PaOutput;#assert pa_forward(q[..],s[..],v[..],sv[..],u[..],target[..],forces[..])==PfkStatus.ok;
 print("clipped controls: %, forces: %\n",forces[0].control_clipped,forces[0].force_clipped);
 for i:0..<7 {print("hinge % actuator/joint (micro-J/rad): % %\n",i+1,pa_example_micro(forces[0].actuator.hinge[i].value),pa_example_micro(forces[0].joint_passive.hinge[i].value));}
 for i:0..<2 {print("slide % actuator/joint (micro-N): % %\n",i+1,pa_example_micro(forces[0].actuator.slide[i].value),pa_example_micro(forces[0].joint_passive.slide[i].value));}
```

```shell
coax/bin/coax run psionic/examples/panda-actuation.coax
coax/bin/coax run psionic/examples/panda-actuation.coax --release
COAX=coax/bin/coax psionic/tests/robotics/panda_actuation_check.sh /tmp/panda-actuation-new
```

The [frozen experiment](../fixtures/robotics/panda-actuation/experiment.json)
uses 18 new q/rate/control contexts. The independent SDK acquisition first
admits the compiled actuator profile, then performs exactly 18 forwards with
no steps, rendering or model inference. It retains separate spring, damper,
gravity-compensation, fluid and full-passive vectors, actuator-space and
generalized forces, transmission/control data, and full-system acceleration/
constraint diagnostics. The latter are future evidence, not owned results.

Debug, release and fail-fast ASan/UBSan each pass 3,960 native assertions. All 162
actuator, 162 generalized-actuator and 162 joint-passive coefficients agree
numerically exactly with the SDK and an independent scalar oracle. The frozen
comparison bound remains 5e-11 absolute, with zero relative tolerance. Exact
clipping counts come from the independent source-profile oracle; force/count
output bytes agree across the three owned profiles. Complete finite guards
precede every maximum, and failed calls preserve caller output even when it
overlaps inputs. Eight Coax groups cover 135 cases. The record names the unchanged typed-entry body and its carried initial kills,
the math 20/20 scope and raw-only 25/25 follow-up, while retaining the original
97/98 missing-observation failure. It makes no aggregate final-rerun score;
quantity operator-token coverage remains an explicit compiler limitation.

Joint passive is not full passive. The original model enables fluid: retained
hinge-fluid coefficients reach 4.701012241 energy/radian, and slide-fluid
coefficients reach 0.859280139N. The owned fluid query below now supplies this
fixed-source calculation. `dof_frictionloss` belongs to the constraint solver, so we do
not invent a Coulomb term in the damping output. Contacts, constraint forces,
integration, full SDK acceleration parity, compiler-bounded control and hardware
remain separate milestones.

## Fluid resistance completes this fixed robot's passive force

The scene has air density 1.2 kg/m³, viscosity 2e−5 kg/(m·s), and zero wind. Its
robot geoms all have the fluid-interaction coefficient zero. MuJoCo uses the
inertia-box fallback in this case. Positive-mass bodies participate even with
no geometry, including the hand. The artificial fingertip inertias remain the
original simulator coefficients; replacing them with mesh-sized boxes would
change the model.

Each body's linear velocity is measured at its inertial center of mass. Its
local axes are the principal-inertial axes: world-from-BODY multiplied by the
normalized BODY-from-inertial attitude. Pinned MuJoCo `mj_objectVelocity` for a
BODY/local request uses `xipos`/`ximat`. We form the COM linear/angular
Jacobians, multiply by the nine joint rates, rotate into those principal axes,
apply viscous and quadratic box resistance, rotate the wrench into world,
then project it through those same COM Jacobians.

Angles stay physical dimensions. An explicit one-radian basis converts angular
rate to 1/s for drag. Dividing the physical moment in joules by one radian
restores the hinge generalized force in joule/radian. A hinge force times its
radian/s rate and a slide force times its metre/s rate both produce watts.
The output keeps three vectors: fluid, joint viscous damping, and complete
passive. Zero spring, gravity-compensation, tendon, flex, plugin and callback
facts authorize that last sum for this source model. Frictionloss remains a
constraint term.

The public boundary accepts the existing ±100 rad/±1 m positions and ±20 rad/s/
±2 m/s rates, with inclusive finite admission. It validates shapes, positions,
then rates and publishes one 216-byte record atomically. Overlap is supported
because inputs are read before publication. Force clipping and a guarantee of
fitting the later solver's force cap are outside this query. Coefficient
helpers remain trusted and source-visible; public construction supplies their
poses, Jacobians and inertials.

<!-- source: psionic/examples/panda-fluid.coax pfl-query -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);sv:[2]pfku.Quantity(PidLinearRate,f64);
 coordinates:=[9]f64.[0,0.19634954084936207,0,-2.617993877991494,0,2.941592653589793,0.7853981633974483,0.01,-0.01];
 rates:=[9]f64.[0.7,-0.4,0.3,-0.6,0.5,-0.2,0.1,0.03,-0.02];
 for i:0..<7 {q[i].value=coordinates[i];v[i].value=rates[i];}for i:0..<2 {s[i].value=coordinates[i+7];sv[i].value=rates[i+7];}
 forces:[1]PflOutput;#assert pfl_forward(q[..],s[..],v[..],sv[..],forces[..])==PfkStatus.ok;
 fluid_power:pfku.Quantity(PflExamplePower,f64);passive_power:pfku.Quantity(PflExamplePower,f64);
 for i:0..<7 {fluid_power=fluid_power+convert_unit(PflExamplePower,forces[0].fluid.hinge[i]*v[i]);passive_power=passive_power+convert_unit(PflExamplePower,forces[0].passive.hinge[i]*v[i]);print("hinge % fluid/joint/passive (micro-J/rad): % % %\n",i+1,pfl_example_micro(forces[0].fluid.hinge[i].value),pfl_example_micro(forces[0].joint_passive.hinge[i].value),pfl_example_micro(forces[0].passive.hinge[i].value));}
 for i:0..<2 {fluid_power=fluid_power+convert_unit(PflExamplePower,forces[0].fluid.slide[i]*sv[i]);passive_power=passive_power+convert_unit(PflExamplePower,forces[0].passive.slide[i]*sv[i]);print("slide % fluid/joint/passive (micro-N): % % %\n",i+1,pfl_example_micro(forces[0].fluid.slide[i].value),pfl_example_micro(forces[0].joint_passive.slide[i].value),pfl_example_micro(forces[0].passive.slide[i].value));}
 print("fluid/passive power (micro-W): % %\n",pfl_example_micro(fluid_power.value),pfl_example_micro(passive_power.value));
```
```shell
coax/bin/coax run psionic/examples/panda-fluid.coax
COAX=/path/to/current/coax psionic/tests/robotics/panda_fluid_check.sh /tmp/panda-fluid-proof-new
```

The experiment was frozen before implementation/comparison. Its 18 cached SDK
rows contain 7 unique position/rate pairs, only 2 moving. Another 24 owned states
are checked against independent homogeneous-matrix product-rule tangents and
scalar resistance. No new SDK calls, rendering, inference or integration run.
The independent oracle's first metadata preflight found fixed mount bodies
sharing root 2. The original experiment/failure remain recorded. A versioned
support addendum proves each omitted body's entire ancestor chain has no
robot DOF 0..8, including fixed mounts 18–22. Cases, numerical formulas and
budgets stayed frozen.

The 17 new Coax groups/74 cases pass; actual emitted mutation scopes are 130/130
for math, 12/12 for body, and 53/53 for the public boundary, all zero survivors without equivalent waivers.
Scalar Quantity operator-token coverage remains incomplete. Independent field
checks and typed unit refusals supplement that stated emitted-site proof.

Debug, release and fail-fast ASan+UBSan each pass 3,752 native assertions over
42 × 27 force coefficients, 90 IEEE input poisons, 36 inclusive boundaries, exact
raw/typed ABI, four overlap cases, refusal/canary/spare/input snapshots and a
16 body metadata sidecar. All owned status/force streams are byte-identical.
The cached SDK comparison is numerical: hinge-fluid error 1.69e−14 J/rad and
slide-fluid 5.00e−15 N; the independent matrix maximum is 1.16e−12 J/rad and
4.00e−14 N. Each uses the original 5e−10 absolute / 0 relative budget. Virtual-work
error 4.95e−12 W passes the original 5e−9 W budget; fluid/passive power is
nonpositive. Source metadata matches every mass, COM, principal moment,
ancestor mask and normalized inertial quaternion.

The record retains a source-refusal tooling correction and Darwin sanitizer
startup failure (`detect_leaks=1` is unsupported). Only the failed sanitizer
startup was retried with leak detection disabled; passed debug/release proofs
were carried by exact artifacts. Reader-only followup rejects late actual and
expected NaN/±Inf and wrong extents without rerunning numerical code.

This is a Darwin arm64 F64/libm profile. Cross-target/R15, arbitrary models,
nonzero wind, ellipsoid fluid, contact/constraint solving, integration,
compiler bounded-control proof and hardware remain pending. The scene runtime
continues using SDK integration. Evidence and exact identities are in
[`panda-fluid/README.md`](../fixtures/robotics/panda-fluid/README.md),
[`qualification.json`](../fixtures/robotics/panda-fluid/qualification.json),
and the linked checks, oracle, support, example and reader-guard receipts.

## Assemble the smooth acceleration

The controller, actuator and passive-force queries now meet at one equation.
For the same admitted positions and rates, the [smooth query](../lib/robotics/panda-smooth.coax)
constructs actuator force, full passive force and the caller's generalized
external force, then asks the applied-force solver for acceleration:

```jai
applied = (actuator + external) + passive
mass * acceleration = applied - bias
```

Here `bias` is the owned inverse-dynamics result at zero acceleration: gravity
and velocity-dependent terms. Full passive already includes joint damping and
fluid drag. Adding the actuator query's separate damping output again would
count it twice. The seven hinge forces are energy per radian; two slide forces
are newtons. External input is generalized force in those coordinates, not a
Cartesian world force or another motor command.

All span shapes are checked before numerical admission. External force is finite and within the
inclusive +/-10,000 numerical box; the query then admits positions/rates,
constructs ambient force and admits the actual controls. The existing applied
and bias-subtracted net-force caps remain in force. Refusal preserves the
complete caller output. Nothing clips external or net force into that box.
Actuator clipping counters still describe actual actuator controls, even when
external force changes the resulting acceleration.

<!-- source: psionic/examples/panda-smooth.coax psf-query -->
```jai
 x:=psf_fixture();q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);sv:[2]pfku.Quantity(PidLinearRate,f64);
 control:[7]pfku.Quantity(PmHingeGravity,f64);target:[2]pfku.Quantity(pfku.Metre,f64);
 external_h:[7]pfku.Quantity(PmHingeGravity,f64);external_s:[2]pfku.Quantity(PmSlideGravity,f64);
 for i:0..<7 {q[i].value=x.q[i];v[i].value=x.v[i];control[i].value=x.control[i];}
 for i:0..<2 {s[i].value=x.q[7+i];sv[i].value=x.v[7+i];target[i].value=x.control[7+i];}
 out:[1]PsfOutput;#assert psf_forward(q[..],s[..],v[..],sv[..],control[..],target[..],external_h[..],external_s[..],out[..])==PmsStatus.ok;
 print("smooth acceleration (micro-rad/s2, micro-m/s2)\n");
 for i:0..<7 {print("hinge %: %\n",i+1,psf_example_micro(out[0].acceleration.hinge[i].value));}
 for i:0..<2 {print("slide %: %\n",i+1,psf_example_micro(out[0].acceleration.slide[i].value));}
 print("actuator clips: % control, % force\n",out[0].control_clipped,out[0].force_clipped);
```

```shell
coax/bin/coax run psionic/examples/panda-smooth.coax
coax/bin/coax run psionic/examples/panda-smooth.coax --release
COAX=coax/bin/coax psionic/tests/robotics/panda_smooth_check.sh /tmp/panda-smooth-new
```

The [frozen experiment](../fixtures/robotics/panda-smooth/experiment.json)
uses 18 previously captured SDK rows with zero generalized external force.
Their retained target is `qacc_smooth`, which solves the unconstrained force
system before contact/constraint corrections. The SDK performed no new calls.
Two additional targets use the same retained rows: at exactly identical q/v,
replace a recorded actuator-force change with its generalized external-force
difference. Total force, mass and bias stay the same, so the already recorded
acceleration is the target. These are derived equal-force comparisons, not two
new external-force simulator experiments.

Across the 20 contexts, debug, release and fail-fast ASan/UBSan each pass 4,406
native assertions. Applied-force error is at most 1.78e-14 against the frozen
5e-10 absolute bound; acceleration error is at most 4.27e-14 against 5e-9, both
with zero relative tolerance. Status and actuator counters are exact; all three
owned output/status/count streams agree byte for byte. Native cases inject 180
nonfinite IEEE payloads, probe numerical endpoints and combined/net caps, and
exercise all four input/output overlap roles. Every numerical comparison
admits finite actual and reference values first.

Six Coax groups cover 67 cases, and the new authority root kills 88/88 actual
emitted mutants with no equivalence waivers. Four static type refusals keep
position targets, external hinge forces, rates and angular acceleration from
silently changing units. The example's transcript was written from the retained
SDK target before execution and passes in both native modes. The record retains
initial import/test-fixture failures and the compile-time VM's refusal to create
raw nonfinite F64 values; those payloads belong to the native boundary checks.
Typed addition token provenance remains the earlier explicit compiler gap.

This is still a force/acceleration query. It does not reproduce the SDK's
constrained `qacc`, contact/frictionloss solve, discrete implicit damping or
position/velocity integration. Camera geometry, policy execution, episode
success, wall-time control and hardware remain separate. The next physical
step must state and check those integration and constraint contracts.

## Build the controller sample from one owned state

The source controller consumes one published `PcSample`: 144 F64 words holding
the grip SITE pose, arm coordinates/rates, arm mass block, both site Jacobians,
world linear/angular velocity, nonzero-rate bias and source torque limits. The
new `pcs_forward` constructs that existing record from seven radian positions,
two metre slides and their matching rates. It does not change the ABI.

One admitted owned finger/FK result feeds every calculation. The arm mass
block includes the moving fingers; the bias uses both finger rates even though
the grip site's seven-column velocity uses only arm rates. Zero-rate gravity
cannot replace that bias. Position/rotation belong to the grip SITE, not the
model observation's hand BODY attitude.

The example uses matched SDK capture 13, whose rounded golden fields were
fixed before the program was run:

<!-- source: psionic/examples/panda-owned-sample.coax sample -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);w:[2]pfku.Quantity(PidLinearRate,f64);
 coordinates:=[9]f64.[0.3,-0.4,0.5,-1.6,-0.7,1.2,0.9,0.017,-0.029];
 rates:=[9]f64.[0.7,-0.4,0.3,-0.6,0.5,-0.2,0.1,0.03,-0.02];
 for i:0..<7 {q[i].value=coordinates[i];v[i].value=rates[i];}
 for i:0..<2 {s[i].value=coordinates[7+i];w[i].value=rates[7+i];}
 sample:[1]PcSample;#assert pcs_forward(q[..],s[..],v[..],w[..],sample[..])==PcStatus.ok;
 #assert pc_sample(sample[0])==PcStatus.ok;
 for i:0..<3 {print("position % (um): %\n",i,panda_sample_micro(sample[0].position[i]));}
 for i:0..<3 {print("linear velocity % (um/s): %\n",i,panda_sample_micro(sample[0].velocity[i]));}
 for i:0..<3 {print("angular velocity % (urad/s): %\n",i,panda_sample_micro(sample[0].velocity[3+i]));}
 print("hinge 2 bias (uJ/rad): %\n",panda_sample_micro(sample[0].bias[1]));
```

The output retains F64 precision; only display values round to microunits.
Shapes precede all values. Positions retain their numerical ±100rad/±1m box;
rates admit inclusive ±20rad/s and ±2m/s. Refusal preserves the entire caller
record. The source-visible packing/pose helpers still trust their inputs and
do not turn writable PODs into provenance certificates. This pure query adds
no sensor clock stamp or compiler runtime-bound proof.

The [native qualification](../fixtures/robotics/panda-owned-sample/native-qualification.json)
compares all 2,160 fields from fifteen matched SDK contexts and passes 9,811
assertions in each debug, release and ASan/UBSan profile. Some contexts repeat
q/v while acceleration changes; these are not fifteen independent states.
Complete outputs agree byte for byte under the named Darwin-arm64 scalar-libm
profile. Three source-unit refusals check the typed boundary.

The initial mutation result was 100 emitted sites: 97 killed and 3 survivors,
all in the constructor. A redundant delegated shape guard was removed and
independent expected public q/qd checks strengthened. The exact changed
`pcs_forward` scope then kills 29/29 sites. Five unchanged procedure bodies
carry their initial zero-survivor evidence; their bodies and checks are bound
explicitly. We do not invent an aggregate current count. These are actual
compiler-emitted sites, not complete Quantity `+`/`*` token-provenance coverage;
that lowering gap remains separate from the spatial-token work. Literal typed
velocity checks and the 2,160 SDK-field comparisons qualify the arithmetic
independently of that mutation limitation.

```shell
coax/bin/coax run psionic/examples/panda-owned-sample.coax
psionic/check.sh --only robotics/panda-owned-sample
```

The focused gate checks the one constructor root and, when mutation is
requested, materializes only the changed procedure while retaining every
original check. It runs the sample native gate once, then uses verified byte
aliases for the separately qualified paired-controller gate. The alias cache
keeps the exact historical execution receipt required by that paired profile;
it never relabels the old record as fresh execution. Newly generated C/header
and owned sample bytes must match the frozen identities, or the gate refuses.
No private cache is required, no second sample generation runs, and a future
compiler change is not silently accepted under this old profile.

## Does the owned sample preserve controller behavior?

Matching sample fields is necessary, but the controller combines Jacobians,
mass and bias through inverses, pseudoinverses and clipping. Small field errors
could therefore change its output or decisions. We test that boundary directly
with the unchanged Coax controller, using the original SDK samples on one side
and the already qualified owned samples on the other.

The experiment is frozen before execution: 15 paired sample contexts, three
actions and 25 held calls per pair. Both sides reset to the exact source default
home before each action. The controller sample stays fixed, and only call 0 creates
a new pose goal. Subsequent calls keep the goal while the controller's internal
gripper target still updates once per call. These are controller calls, not
elapsed simulation time or physical jaw motion.

The three actions are zero, a dense vector and a vector combining inclusive
pose-command endpoints with two saturated components. Status, readiness and
both clipping counts must agree exactly. We predeclare absolute bounds of
2e-8 for torque, 2e-12 for gripper output and 1e-10 for all 21 floating-point state
fields. Relative tolerance is zero. We compare logical fields after normal
calls; padding bytes do not define state equivalence.

All 45 paired resets and 1,125 paired held controller calls return success.
The largest torque discrepancy is 6.54e-13, gripper outputs agree exactly, and
the largest state-field discrepancy is 2.00e-15. Every readiness and clipping
decision agrees. This success histogram is explicit rather than inferred from
two sides returning the same refusal. The selected contexts include repeated
q/v inputs and are not 45 independent physical states.

Separate refused siblings exercise bad mass in reset and step, a NaN action,
a short action span and a not-ready state. Their exact diagnoses agree, output
torque/gripper/count values are zero, and all 150 failed actual calls preserve
their initialized state byte snapshots. The main event ledger contains every
reset, held call and refused sibling: 2,490 events per native mode. All ordinary sample/expected values and decoded comparison fields are finite,
and extents are complete before numerical maxima; poison checks also target the final field to catch partial scans.

The NEW harness passes debug, release and fail-fast ASan/UBSan with 86,679
assertions per profile. Complete serialized event transcripts match byte for
byte under the named Darwin-arm64 scalar-libm profile. It reuses the qualified
Coax C/header and cached sample bytes; it does not rerun the sample generator,
SDK, learned model, old controller gate or any physics step. Exact frozen
source/input hashes and the ledger schema are recorded in
[the paired receipt](../fixtures/robotics/panda-owned-controller/qualification.json).

This establishes static-state compatibility of the owned controller sample
boundary for the declared corpus. A new closed-loop episode still needs the
owned or SDK integration backend, fresh observation timing, actuator/contact
semantics and its own replay/task-result qualification. Held calls alone make
no stability, real-time, hardware-safety or complete-simulator claim.


## Choosing the owned query source without changing the plant

The control loop can now choose the sample computed by our fixed Coax Panda
model. A new versioned platform effect copies all nine named coordinates and
rates. Coax owns the source choice, finite/shape/effect admission and sample
construction. The two modes are explicit; an owned refusal stops the path
instead of quietly using SDK geometry.

Both SDK telemetry and the additive `PandaRobotStateV1` effect remain required.
All 173 telemetry words and 18 coordinate/rate words must be finite; only the
selected sample then receives mathematical admission. SDK `pc_sample` checks
number bounds, rotation and actuator limits; it does not prove mass positive
definiteness. Matrix inversion is checked later by controller reset/step.
The published `PandaTelemetry` layout stays unchanged.

The first profile deliberately retains the SDK as integrator and shadow
query provider. It collects cached request observations before forwarding,
then obtains fresh control data after the explicit forward. Keeping those
stages preserves the existing mixed-age observation contract. This first
integration does not remove the SDK geometry queries.

The teaching example queries a geometric zero pose. The first hinge turns
at 0.5 rad/s, with a 0.088 m world-X grip lever arm: world-Y speed is
0.044 m/s and world-Z angular rate is 0.5 rad/s.
Joint4=0 is outside its physical soft range. This is geometry, not a motor
command or a hardware-valid configuration.

<!-- source: psionic/examples/panda-query-source.coax select -->
```jai
 // Version1transport: seven arm radians, two finger metres; matching rates.
 sdk:[173]u64;q:[9]u64;v:[9]u64;v[0]=f64_bits(0.5);sample:[1]PcSample;
 // Zero SDK telemetry is finite but its zero rotation is invalid. Owned
 // selection admits only its own mathematical sample after common checks.
 r:=pqs_select(1,sdk[..],q[..],v[..],0,0,sample[..]);#assert r.code==PqsCode.ok;
 #assert abs(sample[0].velocity[1]-0.044)<0.000000000000002 && abs(sample[0].velocity[5]-0.5)<0.000000000000002;
 print("grip SITE world Y (um/s): %\n",panda_query_micro(sample[0].velocity[1]));
 print("world angular Z (urad/s): %\n",panda_query_micro(sample[0].velocity[5]));
 r=pqs_select(0,sdk[..],q[..],v[..],0,0,sample[..]);#assert r.code==PqsCode.sample;
 print("SDK invalid rotation: refused\n");
 r=pqs_select(1,sdk[..],q[..],v[..],0,1,sample[..]);#assert r.code==PqsCode.effect;
 print("failed robot effect: refused\n");
```

```shell
coax/bin/coax run psionic/examples/panda-query-source.coax
psionic/check.sh --only robotics/panda-query-source
```

A frozen, independently prewritten transcript checks those rates. New native
selector checks matched 4,320 SDK/owned field words in each of debug, release and fail-fast
ASan+UBSan builds, with 5,595 assertions per mode; all 97 actual emitted new root
mutants were killed. The native numerical profile is Darwin arm64 scalar libm,
with no fast math or contraction; unsupported targets refuse rather than skip.
The original sample and controller gates were carried from their existing
receipts rather than rerun.

The meaningful integration check uses the same authenticated reset and
recorded actions twice, recomputing all controller outputs. One new 250-step
prefix uses SDK samples; the other uses owned samples. At each actual state,
the opposite-source controller runs as an independent shadow. All 1,000
selected/shadow calls returned OK. The independent trajectories agreed to
1.87e-12 in applied arm torque, 1.34e-15 across qpos words and 7.25e-14 across
qvel words; gripper commands and request F32 observations were bit-identical.
The predeclared budgets were 1e-6, 1e-8 and 1e-7 respectively. Clock/action/flag
records, source stamps, ready state and clip counts matched exactly.

Those 500 SDK steps qualified the recorded-action prefix without NN, rendering,
task success or hardware evidence. The following section separately qualifies
the explicit owned-query route in the genuine neural host. SDK integration/
contact and geometry effects remain;
removing those needs separate owned-backend work. Source-visible low-level
helpers still have trusted preconditions; only the exported selector supplies
this admission contract. Full records and frozen budgets are in
[the query-source fixtures](../fixtures/robotics/panda-query-source/README.md).

The SDK prefix is an explicit opt-in gate, never part of the default selector
check. Supply the exact authenticated scene/reset pair, recorded action trace
and a new persistent output directory. Preparation freezes source/C/header/binary
identities before the execution phase. The current proof pins its qualified
compiler and MuJoCo 3.3.1 SDK identities; a changed profile needs new qualification.

```shell
PSIONIC_PANDA_SCENE="$PANDA_SCENE" \
PSIONIC_PANDA_INITIAL_STATE="$PANDA_INITIAL" \
PSIONIC_PANDA_QUERY_TRACE="$PANDA_TRACE" \
PSIONIC_PANDA_QUERY_OUTPUT=/tmp/NEW-panda-query-prefix \
  psionic/check.sh --only robotics/panda-query-source-sdk
```

## An explicit owned-query route in the neural runner

The neural runner now has a build option that selects our fixed Panda model
for the controller sample. Coax owns its admission and construction; the host
copies state and performs effects. The default SDK route keeps its existing
profile and numeric recipe.

An opted-in binary first authenticates the exact qualified scene, reset,
loaded SDK library file and all 85 referenced assets. Directory labels and
matching dimensions are insufficient. This is a fixed-model implementation,
not a model importer, and its raw state records are not provenance proofs.
Unknown mode/model or an owned refusal stops the route without SDK fallback.

The request stage remains unchanged: cached SDK observations and camera
captures occur before the next forward. Fresh owned control samples are
selected after the explicit forward and before the clock commits its sample
receipt. The existing fault overlay follows selection. This keeps the
right-hand BODY/grip SITE and observation-age distinctions intact. SDK
integration/contact and its geometry queries still remain.

```shell
PANDA_QUERY_SOURCE=owned-shadow-v1 PANDA_BUILD_ONLY=1 \
  PANDA_SHADER_CACHE="$QUALIFIED_SHADERS" \
  psionic/tests/robotics/run-panda-loop-metal.sh \
  "$GROOT_SHARDS" "$COSMOS_PST" "$NOISE_F32" "$PANDA_PAIRS" \
  /tmp/new-owned-query-build 1 0 000
```

That command links the new versioned route; it does not run inference or open
a simulator. The option belongs to the updated wrapper and its new binary,
not an already built legacy artifact. The owned build uses its declared
scalar-libm recipe and a distinct combined policy/query identity.

The file-level preflight found a real boundary: our one-shot SHA API accepts
at most 1 MiB per input. A new Coax file adapter now plans bounded streaming
updates, with a 16 MiB file cap. Ten independent hash vectors through that cap
and a positive 88-file preflight pass. The old cryptographic implementation is
unchanged; no old cryptographic gate was rerun.

The new authority roots kill 329/329 mutants. Original-metadata native checks
make 3,217 assertions per mode, and the separate mocked effect-order harness
makes 551 assertions per mode across 15 contexts. Debug, release and fail-fast
sanitizer profiles pass. Exact generated-code bindings retain the original
control/current helper behavior, constants, globals and header layouts; one
private generated spatial-helper name changes while its signature/body do
not. Old receipts remain historical rather than being rewritten as new runs.

A genuine neural request with the opted-in owned route remains the next
integration check. This checkpoint contains no new NN/simulator execution,
episode or task-success result. The [complete contracts and composed proof](../fixtures/robotics/panda-query-runtime/README.md)
retain each executed artifact and its scope.


## Owned controller samples in the genuine neural loop

The earlier 500-step recorded-action comparison qualified replacing controller samples while preserving the SDK plant. A separate owned-query build then linked that selector to the existing resident Coax GR00T policy. Its build, mocked ordering and file-only admission records deliberately reported zero neural and simulator execution. This first bounded run tests their actual composition; those historical records remain unchanged.

The profile admits one exact original `demo_000` scene/reset, the actually linked MuJoCo 3.3.1 library and all 85 referenced assets before opening the policy. Counts and a matching directory name cannot certify our fixed Panda coefficients. The new query profile has its own schema and trace identity; unknown profile names or an owned-query failure cannot fall back to SDK controller math. The unchanged default route remains a different profile.

The coupled path is now:

1. Capture the existing cached SDK request state and both offscreen cameras, before the next explicit forward. Request state retains the grip SITE position, right-hand BODY attitude and current finger coordinates in the established observation recipe.
2. Run the resident Coax GR00T vision, text and four-step action-head graph on Metal with the original weights, the existing wide-statistics/F32 numerical profile and explicit fixed noise. The unchanged adapter decodes physical actions, then the supervisor admits the action chunk.
3. For every physics substep, perform SDK forward and read the current named nine positions/rates. Coax constructs the selected controller sample using its owned finger/arm FK, world Jacobian, sixteen-body mass and velocity-dependent bias. SDK telemetry is still acquired and admitted; its geometry remains available for requests, cameras and success observation.
4. Give that sample to the original operational-space controller. A new action updates the pose goal only at the first of 25 servo substeps; the gripper's internal target still advances on every servo call. The unchanged controller reports saturation and supplies nine actuator commands.
5. Let MuJoCo perform the source scene's actuation, contacts and integration. Admit the returned clock/state receipt, chain the canonical physics record and commit the complete action row. After eight consumed rows, capture the next observation at the same cached SDK stage and issue a second genuine neural request.

The supervisor admits all 16 decoded seven-component action rows, then consumes the first eight before replanning. Each consumed row spans 25 physics steps at 0.002 seconds: 20 Hz action goals and a 500 Hz servo in simulation time. The ninth row therefore exercises the first row of the second genuine request, rather than extending an old chunk indefinitely. These are scheduling semantics, not wall-time throughput guarantees.

This is not the chapter167 scheduled-PD experiment or its owned Euler/no-contact profile. The neural runner retains the original scene defaults, SDK geometry, contacts, integration and rendering. “Owned query” identifies the selected controller mathematics, not an SDK-free robot simulator.

The first run completes the prescribed nine ticks and two genuine model calls, with 225 canonical forward-run physics records, zero fallbacks, zero refusals and zero torque-clipped coordinates. The mandatory 225-step replay matches all 45 F64 state words exactly. The sanitizer-profile process exits successfully with empty stderr. Task success is false: this experiment required a clean bounded integration prefix, not successful task completion. The saved first decoded hash matches the older frame0 value as a diagnostic only.

The first independent stdlib audit passes on the saved evidence without repeating physics or inference. It decodes all 225 canonical records, checks 1,575 F32 action words and 12,150 F64 control/state words for finiteness, reconstructs the seed and every SHA link/footer, and verifies serial1→2 plus the first eight decoded action rows. Maximum SDK-clock error is 3.331e−16 seconds, within the frozen 2⁻²⁶-second budget. Its new reader guards cover all numeric lanes, canonical metadata, malformed case records and poisoned hash chains.

| Timing observation | Seconds | Scope |
| --- | ---: | --- |
| Whole recorded invocation | 36.6552 | Includes entry, model open, live loop and replay |
| Model context open | 12.4638 | Reported separately before loop timing |
| Live loop wall | 10.3489 | Excludes initial model open and replay |
| Inference total | 8.9717 | Two model calls; maximum call4.7554 |
| GPU compute total | 2.0947 | Reported graph GPU work within inference |
| Simulated time | 0.4500 | Nine action rows, independent of wall time |

Replay is not individually timed in this record. Subtracting these differently scoped observations would not manufacture a separate replay measurement.

The frozen normal prefix is nine action ticks: 225 forward-run SDK steps and 225 mandatory recorded-control replay steps, 450 integrated steps total. Two successful model calls imply four camera capture calls in the bound source path; these are source-backed counts, not newly instrumented per-function telemetry. Initialization and replay resets also perform SDK forward, so integrated-step counts are not totals of every SDK forward call. No extra model or SDK verification run is scheduled after this attempt.

The trace binds the exact scene, reset, explicit noise and versioned profile. The independent reader checks the new PTR1 bytes, finite action/control/state fields, canonical metadata and live simulation clock, and reproduces the SHA chain through every 512-byte record and final digest. In a complete normal prefix, serial1 covers ticks0..7 and serial2 covers tick8. The first eight consumed actions can be compared to the saved first decoded output; the full second decoded array is not separately persisted. The successful second-call/source/serial transition establishes that later handoff without claiming an independent byte oracle for every hidden layer or the complete second output.

Native replay resets the SDK and applies the recorded actuator commands. Every one of the 45 F64 state words must match exactly at every integrated step, and the hash chain must match. Replay performs no controller recomputation, neural inference or rendering. Exact physics replay therefore does not claim exact image replay. The first decoded hash is a diagnostic comparison with an existing frame0 receipt, not a new vendor-framework execution or an acceptance requirement.

Acceptance requires the complete normal prefix with no fallback/refusal, or a separately identified authoritative source-success stop after a complete action row. A partial, refused or failed run is retained and is not an accepted integration. PTR success flags remain zero in this source path; an early success is carried by the final case record and the bound source predicate. A short prefix has no task-success objective or LIBERO benchmark threshold.

Model-open time, loop wall time, inference time and GPU compute time remain distinct observations; replay is included only in the whole invocation timing. Synchronous inference pauses simulation. Nine action ticks correspond to 0.45 simulated seconds; that does not prove a 20 Hz wall deadline, compiler-enforced bounded control, hardware validity or global policy reliability.

The [owned-runtime checkpoint and executed-prefix evidence](../fixtures/robotics/panda-query-runtime/README.md) retain source, binary, shader, model-input and trace identities. The older model endpoint/conditional-head proofs and their failed global hidden diagnostics remain carried with their original scope. No new Coax controller, example or arithmetic qualification is implied by executing this existing linked profile for the first time.

## Run the genuine experiment

The [acquisition and profile notes](../fixtures/robotics/libero-panda/README.md)
identify the authenticated original weights, tokenizer, explicit noise and
paired scenes. Set `GROOT_SHARDS`, `GROOT_PST`, `PANDA_NOISE` and `PANDA_PAIRS`
to those private local assets. `MUJOCO_ROOT` identifies the binary 3.3.1 SDK
folder containing `include/` and its dylib. The build wrapper can also locate
that installed SDK in the private reference environment. The compiled runtime
uses C, Coax-generated code, Metal and the SDK; it does not invoke Python,
PyTorch, LIBERO scripts or NVIDIA repository code.

Run the evaluation in two-case slices with fresh outputs:

```shell
psionic/tests/robotics/run-panda-loop-metal.sh \
  "$GROOT_SHARDS" "$GROOT_PST" "$PANDA_NOISE" "$PANDA_PAIRS" \
  /tmp/panda-evaluation-a 720 0 001 010
psionic/tests/robotics/run-panda-loop-metal.sh \
  "$GROOT_SHARDS" "$GROOT_PST" "$PANDA_NOISE" "$PANDA_PAIRS" \
  /tmp/panda-evaluation-b 720 0 020 030
psionic/tests/robotics/run-panda-loop-metal.sh \
  "$GROOT_SHARDS" "$GROOT_PST" "$PANDA_NOISE" "$PANDA_PAIRS" \
  /tmp/panda-evaluation-c 720 0 040 049
```

Scenario numbers 1–4 select the four declared faults. Use 16 ticks and demo
000 for those prefixes, with a new output directory per scenario. The Coax
session defines the injected conditions; the C driver transports their effects.
`PANDA_NATIVE_MODE=sanitize` instruments the changed native path for ASan/UBSan.
Private traces stay outside the repository; public receipts retain their hashes
and outcomes. The dated [verification record](../../docs/verification/psionic-ch169-2026-10-31.md)
distinguishes every gate and its actual scope.

## Advance the unconstrained robot by one owned step

The smooth query tells us the acceleration at the current state. Moving the
state forward also requires an integration rule. The source model uses a
0.002-second Euler step with implicit joint damping. The fingers have strong
joint damping, so we account for how that damping responds to the velocity
chosen for the end of the step.

Let `B` be the diagonal matrix of nine joint-damping coefficients and let `h`
be typed seconds. The mass already includes each rotor armature once. Full
passive includes joint damping plus fluid, evaluated at the current q/v. In
the existing one-radian/one-metre coefficient basis, the equation is `(M +
h*B) * a_eff = ((actuator + external) + full_passive) - bias`. We solve for an
effective acceleration, update velocity, then use that new velocity to update
position:

```jai
net = ((actuator + external) + full_passive) - bias
effective_mass = mass + h * B
effective_acceleration = solve(effective_mass, net)
next_velocity = velocity + h * effective_acceleration
next_position = position + h * next_velocity
```

Keep the current `-B*v` contribution inside full passive. The added `h*B`
accounts for joint damping at the new velocity; adding `B*v` back would change
this equation. Fluid drag, actuator gains, gravity and velocity bias remain
current-state forces. No fluid derivative is added to the solve matrix. The
hinge diagonal receives seconds times kg m²/(s rad²), preserving kg m²/rad².
The slide diagonal receives seconds times kg/s, preserving kilograms. Cross
blocks retain their original units. The qualified explicit one-radian/one-
metre basis handles the numerical factorization without treating radians as an
unlabelled scalar.

This pure fixed-source map accepts seven arm angles, two finger positions,
their angular and linear rates, arm torque controls, signed finger-position
targets and generalized external forces. It returns just the nine robot
coordinates/rates, effective accelerations and two actuator-clipping counts in
one 232-byte record. It has no clock or motor effect. Every shape is checked
before values. External force admission precedes q/v and control admission;
applied and net force each keep their separate magnitude-10000 numerical cap.
Publication follows the effective-mass solve and complete next-state
admission. The candidate state must stay in the same inclusive numerical
boxes: ±100 rad, ±1 m, ±20 rad/s and ±2 m/s. A refusal preserves the complete
output. These are numerical bounds, not hardware limits or physical joint soft
ranges.

The runnable example starts at the geometric zero pose, including joint 4 = 0,
which lies outside its source physical soft range. It compares all
q/v/effective acceleration fields to an independent Gaussian solve of retained
original SDK mass/bias and actuator/passive operands at exactly the same zero
q/v. Those are new algebraic targets, not observed SDK timestep endpoints. The
stdout was written from that independent reference before implementing the
example.

```shell
coax/bin/coax run psionic/examples/panda-euler.coax
coax/bin/coax run psionic/examples/panda-euler.coax --release
COAX=/path/to/current/coax psionic/tests/robotics/panda_euler_check.sh /tmp/panda-euler-new
```

<!-- source: psionic/examples/panda-euler.coax peu-step -->
```jai
 q:[7]pfku.Quantity(pfku.Radian,f64);s:[2]pfku.Quantity(pfku.Metre,f64);
 v:[7]pfku.Quantity(PidAngularRate,f64);sv:[2]pfku.Quantity(PidLinearRate,f64);
 arm:[7]pfku.Quantity(PmHingeGravity,f64);target:[2]pfku.Quantity(pfku.Metre,f64);
 external_h:[7]pfku.Quantity(PmHingeGravity,f64);external_s:[2]pfku.Quantity(PmSlideGravity,f64);
 target[0].value=0.02;target[1].value= -0.02;
 out:[1]PeuOutput;
 #assert peu_step(q[..],s[..],v[..],sv[..],arm[..],target[..],external_h[..],external_s[..],out[..])==PmsStatus.ok;
 // Independent retained-operand Gaussian oracle, not an observed SDK step.
 peu_cached_golden(out[0],0);
 print("owned unconstrained Euler: 2000 microseconds\n");
 for i:0..<7 {print("hinge %: q % urad, v % urad/s, a % urad/s2\n",i+1,peu_example_micro(out[0].angles[i].value),peu_example_micro(out[0].hinge_rates[i].value),peu_example_micro(out[0].effective.hinge[i].value));}
 for i:0..<2 {print("slide %: q % um, v % um/s, a % um/s2\n",i+1,peu_example_micro(out[0].slides[i].value),peu_example_micro(out[0].slide_rates[i].value),peu_example_micro(out[0].effective.slide[i].value));}
 print("actuator clips: % control, % force\n",out[0].control_clipped,out[0].force_clipped);
```

The 2000 microseconds label names the fixed mathematical step size.
The displayed micro-units round the F64 state only for reading:

```jai
owned unconstrained Euler: 2000 microseconds
hinge 1: q 0 urad, v 0 urad/s, a 16 urad/s2
hinge 2: q 4 urad, v 2246 urad/s, a 1122766 urad/s2
hinge 3: q 0 urad, v 0 urad/s, a 53 urad/s2
hinge 4: q 8 urad, v 4164 urad/s, a 2082240 urad/s2
hinge 5: q 0 urad, v 0 urad/s, a 116 urad/s2
hinge 6: q -8 urad, v -4173 urad/s, a -2086395 urad/s2
hinge 7: q 0 urad, v 0 urad/s, a 40 urad/s2
slide 1: q 61 um, v 30564 um/s, a 15282008 um/s2
slide 2: q -61 um, v -30505 um/s, a -15252343 um/s2
actuator clips: 0 control, 0 force
```

The frozen six-context experiment keeps two sources of evidence distinct.
Three contexts reconstruct the step algebraically from retained original PID
mass/bias row 0 and Pa force rows 12/13/14, using an independent Gaussian
solve of `M+h*B`. They probe the exact finger-force cap and its adjacent
floating-point values. The other three are new, directly observed single-step
SDK contexts: static gravity, moving with external force, and moving with
saturated controls.

Before any SDK loading, an asset-inventory preflight found 86 XML declarations
but 85 unique asset paths. Its initial declaration-count assumption failed.
The failed record and a frozen source addendum preserve that correction, every
unique file identity, and the original XML binding all declarations. The C
collector, cases, equations and numerical budgets stayed unchanged.

The separate three-context SDK experiment deliberately disables both
constraints and contact (flags 17), leaving the other used flags clear and
source coefficients unchanged. Contact-only disabling would leave
frictionloss/limits enabled. The original flags 0 remain recorded. The
`EULERDAMP` disable bit remains clear, so implicit joint damping stays
enabled. Each new context has one authored pre-forward and one original SDK
step, with no post-step forward, renderer or neural call. The two off-diagonal
robot/scene mass blocks (9×12 and 12×9) are exactly zero. Every SDK warning
counter and constraint-force coefficient remains zero under this profile.

The reference q/v values are read directly from the updated SDK state. Its
`d->qacc` still describes the pre-step instantaneous calculation; Euler uses a
scratch acceleration for the update. We derive the effective reference from
`(v_next-v)/h`. Its finger coefficients differ from the instantaneous values
by about 2.754, 0.364 and 7.312 m/s² in the three contexts. That difference is
the reason to qualify the actual step rather than assume the smooth
acceleration can be integrated unchanged. Frozen absolute budgets are 2e−12
for q, 2e−10 for v and 1e−8 for that diagnostic, with zero relative tolerance.

The new source checks have 12 groups/163 cases and kill all 151 new-root
mutants without waivers; four negative source fixtures retain precise type
refusals. The score covers actual emitted sites; the existing scalar Quantity
operator-token coverage limitation remains explicit. The new native consumer
passes 4,380 assertions in each debug, release and fail-fast ASan/UBSan
profile, with byte-identical owned results. It includes every nonfinite input
lane, shape/cap/order refusals, overlapping inputs/output and late next-state
refusals. Twenty new consistency checks first establish that the smooth query
admits those same inputs, so an earlier model/force refusal cannot masquerade
as a late Euler-state refusal. All actual and reference fields are finite and
complete before maxima. Among 72 initial numerical-box endpoint queries, 12
honestly refuse downstream rather than being described as successful steps.

Against the three new observed SDK steps, maximum absolute differences are
1.3553e−20 for q, 1.1102e−16 for v and 3.5528e−14 for the effective-
acceleration diagnostic, under the frozen budgets. The three independent
cached algebraic targets pass separately: maximum coefficient differences are
1.356e−20 for q, 6.939e−18 for v and 3.553e−15 for effective acceleration.
Position/rate/acceleration values retain their hinge and slide units; these
are elementwise coefficient budgets, not a mixed-unit physical norm. An
independent Gaussian reconstruction of the three new SDK observations also
passes: its maximum coefficient differences are 1.695e−21 for q, 6.939e−18 for
v and 2.843e−14 for the velocity-derived effective acceleration. Those are
cached-data checks after acquisition, with no extra SDK calls. The qualified
PFL/PSF force paths and their prerequisites are carried by identity. No old
component, neural or SDK gate was repeated.

Owned same-recipe bit identity and SDK tolerance parity are separate claims.
The owned force-sum order can round differently from the SDK's order. Source-
visible low-level helpers remain trusted and unsealed. This fixed robot block excludes contact, frictionloss/limits, coupled object
dynamics, model importing, typed clocks, compiler boundedness, and hardware.
It therefore does not replace the current LIBERO simulation backend.
