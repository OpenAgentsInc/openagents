# 259. Rollouts from independent worlds

> **Status: unimplemented — design draft.** This chapter designs a bounded
> collector and normalization lifecycle. Existing scalar GAE/PPO profiles are
> reused; no new batched robot physics or training result is claimed.

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
