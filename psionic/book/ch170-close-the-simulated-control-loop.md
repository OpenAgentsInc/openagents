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
