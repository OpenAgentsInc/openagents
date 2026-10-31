# 170. Close the simulated control loop

The original GR00T policy now has a complete route to a simulated Panda:
raw cameras and arm state enter Coax, Coax executes the original learned
network on Metal, and a Coax operational-space controller turns its selected
pose commands into seven torques and two finger targets. MuJoCo 3.3.1 advances
the source LIBERO scene. The next observation comes from that changed scene.

We first qualify the actual LIBERO/Panda loop with MuJoCo as an evaluation
backend. The owned solver
in chapter 109, function transformations in chapter 107, differentiable simulator
in chapter 111, and compiler enforcement of `#bounded` remain pending. The
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
