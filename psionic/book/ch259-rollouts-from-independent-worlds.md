# 259. Rollouts from independent worlds

> **Status: partly implemented.** The CPU `running-statistics-f64-v1`
> component and a bounded F64 transition collector on the oscillator plant are
> runnable and checked. Actor/normalizer version fencing across PPO epochs,
> declared counter ranges for reset and randomization, delayed actions,
> mid-rollout checkpoint and resume, and a row codec remain to implement. No
> robot or new learning result is qualified.

Training needs more than arrays containing observations and rewards. It needs
the observation that produced an action, the exact old policy distribution,
what the plant received and the state at which an episode ended. An auto-reset
can erase the final observation before the critic sees it. Shared randomness
can make one robot's trajectory depend on how often another falls.

We first build the collector on an existing owned analytic plant. This closes
the data, reset and checkpoint contracts before a costly biped run. The same
collector later connects to articulated/contact environments through an
explicit adapter; the analytic run is never labeled walking.

## A truncation is not a terminal state

Take a continuing-task transition with reward 1, current value 0.5, final value
2 and discount 0.9. An external collector cutoff keeps the final-state value:

```text
delta = reward + gamma * final_value - current_value
      = 1 + 0.9 * 2 - 0.5 = 2.3.
```

If the robot instead reached a true terminal state, delta is 1 − 0.5 = 0.5.
If auto-reset returned a new state whose value is 100 and we used it as the
final value, delta would be 90.5. The wrong result can look like a successful
learning signal. Store the pre-reset final observation and its value explicitly.

Reuse [`rga_returns`](../lib/robotics/locomotion/gae.coax) for bootstrap/trace
semantics. Do not implement another recurrence in the collector. A finite
task deadline can be a true terminal condition; label it from task meaning,
rather than treating every clock cutoff as truncation.

## One row and one publication boundary

A logical row retains observation, actor/normalizer identity, sampled latent
action, limited physical target, old joint-action log probability, current
value, reward, final/next value, termination flags, episode and step identity.
The serialized shape is registered before its codec is written.

Choose world-major storage for the first CPU profile. Begin with four worlds
and eight steps: 32 rows, within current GAE admission. For 61 F32 observations
and 14 F32 latent actions, those two arrays alone use 32 × 75 × 4 = 9,600 bytes.
At 4,096 worlds and 32 steps they use 39,321,600 bytes, or 37.5 MiB. Neither
calculation includes values, applied actions, optimizer, physics or scratch.
Admit the complete memory plan before allocation or collection.

The current GAE helper admits at most 16 worlds, horizon 128 and 2,048 rows.
Larger batches must use checked independent slices or a newly qualified
extension. Preserve each episode's horizon semantics and stage a complete
result; a partial slice is not a completed rollout.

## The shared statistics component now runs

[`running-stats.coax`](../lib/running-stats.coax) implements bounded F64
statistics for 1–64 observation channels and 0–2,048 incoming rows per call,
with at most one billion samples. Its [contract](../docs/robotics/running-statistics-rb03.md)
fixes row-major input order and all numerical boxes. Each snapshot holds the
active channel count and every channel's count, mean and summed squared
residual `M2`; inactive slots are zero and receive admission too.

For the three rows `[1,2,3]`, `[2,4,4]`, `[3,6,5]`, the means are `[2,4,4]`
and the summed squared residuals are `[2,8,2]`. Population variance is therefore
`[2/3,8/3,2/3]`. With epsilon `1e-8`, the first channel of observation `[4,0,-1]`
becomes `(4-2)/sqrt(2/3+1e-8)`, approximately 2.4494897. These are independent
two-pass calculations, rather than another copy of the incremental recurrence.

The Welford arithmetic is shared with chapter 34's comparator; its sample
variance convention stays unchanged there. The observation transform here uses
population variance with a fixed epsilon. An empty snapshot explicitly applies
the identity transform. It never divides by zero or silently treats one sample
as an uninitialized state.

<!-- source: psionic/lib/running-stats.coax running-stats-push -->
```jai
  stage:=previous;
  for row:0..<rows {for channel:0..<previous.channels {
    stage.moments[channel]=sm_push_owned(stage.moments[channel],values[row*previous.channels+channel]);
  }}
  status=rn_check(stage);if status!=RnStatus.ok {return status;}
  output[0]=stage;return RnStatus.ok;
```

Every incoming value is admitted before this stage begins. A numerical state
can lie exactly at its M2 ceiling; F64 rounding on the next update can put the
candidate just outside that box. Final admission is necessary even after finite
input checks. Such a failure preserves the whole caller snapshot.

<!-- source: psionic/lib/running-stats.coax running-stats-apply -->
```jai
  stage:[RN_CHANNELS]f64;
  for channel:0..<s.channels {
    m:=s.moments[channel];stage[channel]=values[channel];
    if m.count>0 {stage[channel]=(values[channel]-m.mean)/sqrt(m.m2/f64(m.count)+1e-8);}
  }
  for channel:0..<s.channels {output[channel]=stage[channel];}return RnStatus.ok;
```

The [example](../examples/running-statistics.coax) keeps separate active and
staged snapshots, checks the independent normalization vector, and refuses a
late invalid observation:

```shell
coax/bin/coax run psionic/examples/running-statistics.coax
```

```text
staged: 3 samples, means 2 4 4, M2 2 8 2
active: 0 samples, unchanged; staged transform matches independent reference
late observation: status 3 (range), staged snapshot unchanged
```

The example exits zero after requiring the query's exact `RnStatus.range`
refusal. Its two snapshots illustrate caller ownership; they do not implement
the actor/epoch lifecycle described below. The future collector must enforce
which snapshot remains frozen while old log probabilities are reused.

A separate Fraction/70-digit Decimal oracle checks the reference moments and
normalization. Debug/release and native sanitizer checks cover all public
outputs, batch-versus-stream bit equality, empty/one-sample transforms, maximum
shape, exhausted counters, real nonfinite active/inactive values and aliasing.
The [receipt](../../docs/verification/robotics-running-stats-rb03-2026-11-15.md)
records the observed checks and remaining collector boundary. No persistent
snapshot codec or F32 neural normalization claim follows from this F64 component.

## The oscillator collector now runs

[`world-rollout.coax`](../lib/robotics/world-rollout.coax) collects 1–16
oscillator worlds for 1–64 steps, at most 512 rows. It reuses `os_step`,
`rgp_sample`/`rgp_density`, `rn_apply`/`rn_push`, `rga_returns_raw` and
tagged SHA-256; it adds no second plant, sampler or GAE recurrence. Each world
owns its counter-RNG stream, episode cursor and reset state. The caller's
snapshot normalizes every step and stays frozen; the raw observations are
pushed into a separate staged snapshot that the result returns. Each row keeps
the pre-reset next state, the bootstrap value (zero only on a true terminal),
the old log probability and the GAE outputs. A truncated row keeps its
final-state value; the next row starts from the reset state.

The collector admits the configuration, every world and the snapshot before
it steps any world, and publishes worlds, rows and digests only after GAE and
the statistics push succeed. Each refusal names the fault and leaves every
output untouched. The checks run a three-world batch and each world alone and
require identical rows and per-world digests. They move one world so that it
terminates early and require the other worlds' rows and digests to stay
unchanged. They also cover the exact admission bounds.

An independent Python oracle,
[`world_rollout_oracle.py`](../tests/robotics/world_rollout_oracle.py),
recomputes the trajectory, GAE, Welford update and digests, and wrote the
frozen vectors in
[`world-rollout-golden.coax`](../lib/robotics/world-rollout-golden.coax).
Python's `log`, `cos` and `exp` come from host libm, while Coax uses its fixed
fdlibm profile, so floats are compared within 8 units in the last place; on
the reference Mac the largest gap is 3. Flags, counters and digest bytes match
exactly.

```shell
psionic/tests/robotics/check.sh world-rollout
```

Only the action-noise draws use the counter stream; resets copy a fixed
state. This is an analytic plant with a linear actor and a quadratic critic,
not a learning result.

## Frozen normalization and independent randomness

Freeze a normalizer snapshot before collection and retain it through every
PPO epoch on those rows. New statistics can be staged from collected raw
observations and activated for the next rollout. Old log probabilities must
never be compared with a new policy evaluated through a silently changed
normalizer. A checkpoint includes both active and staged statistics.

Use the existing [counter RNG](../lib/counter-rng.coax). Declare counter ranges
for reset, physical randomization, action noise and training shuffle, keyed
by logical environment/episode/step. Reject wrap/exhaustion. Do not reseed all
worlds when one episode ends. Worker scheduling is not a random key.

The environment adapter returns a completed transition or an explicit failure.
If an external simulator has already stepped when a receipt fails, mark the
run incomplete; publication rollback does not roll the physical effect back.

## A reset-isolation example

Run environments A and B for eight steps using fixed commands. Repeat while
resetting only A after step three. B must retain the same observations,
random draws and state digests in the deterministic profile. Then run B alone
with the same logical identity. Its trajectory must still match.

Exercise a nonempty delayed-action queue, active/staged normalizers and a
collector tail in a mid-rollout checkpoint. Restoring only model weights
cannot satisfy this test.

## What the implementation must prove

Checks cover the hand delta calculation, final-versus-reset observation,
duplicate/reordered rows, inconsistent identities, buffer exhaustion and
nonfinite masked data. Refusals preserve published outputs. Independent
table traces check row order, while the existing GAE oracle checks targets.
Snapshots must preserve plant state, pending actions, counters, episode cursors,
normalizers and any policy state.

[RB-03](../docs/robotics/microduck-implementation-roadmap.md#rb-03-bounded-collector-and-normalizer)
starts on the owned oscillator. Engine chapter 31 owns actual batched worlds;
this collector must not grow a second dynamics engine to fill that gap.
[Chapter 260](ch260-train-a-neural-walking-policy.md) uses these frozen rows.
