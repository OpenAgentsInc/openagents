# The Panda adapter consumes runtime clocks

`panda-runtime-clock-500-20-v1` connects the nominal runtime clock precursor to
the genuine Panda SDK adapter used by [chapter 170](../../book/ch170-close-the-simulated-control-loop.md).
It preserves the existing `PlState` command supervisor, controller arithmetic
and `PTR1` trace contract. A new Coax sidecar gates effects and checks that the
action-row and physical-step views stay aligned.

This is runtime simulation-clock consumption. It is not compiler R13 clock
syntax, a general R18 sensor policy or a new full-policy task evaluation.

## Keep the two counters honest

`PkState` pairs the existing supervisor with `ScSchedule`, the last successful
explicit forward generation, a fresh-sample flag and the admitted SDK initial
seconds. `PlState.tick` still counts 20Hz action rows; `ScSimInstant.step` counts
500Hz physical integrations. The initial SDK time may be nonzero while the
new epoch's simulation step is zero.

Request/accept/refuse/select wrappers call the existing `pl_*` procedures and
the clock procedures together. Replies still undergo all 112-word command
checks. Clock admission happens before `pl_accept` can publish commands, and
an ordinary refusal preserves both cursors and public outputs. Tags bind the
original epoch/serial/profile/action-row identity as well as the simulation
origin. Untrusted tag ticks are compared by the existing supervisor; only its
admitted origin is multiplied by 25.

After the last physical integration in a row, the clock has advanced 25 steps
while the supervisor still names the old action row. Encode that row's trace
first, then call `pk_commit`. This explicit boundary preserves every `PTR1`
tick/substep/serial field. No fields or versions were changed in place.
Reset and early finish coordinate both sides; finishing early does not imply
that the full 18,000-step horizon was consumed.

## Track effects rather than reconstructing stamps

| Event | Admission |
| --- | --- |
| Capture request input | Use the tracked last-forward generation and current committed qpos generation before the next forward |
| SDK forward receipt | `pk_forward` records a successful effect at the current simulation step |
| SDK sample receipt | `pk_sample` requires that fresh forward and consumes its sample flag |
| Control intent | `pk_control` checks the actual substep; the existing controller still runs every step and updates goals only at substep zero |
| SDK integration receipt | `pk_receipt` admits exactly the next step and checks the separate F64 SDK-time adapter |
| Row completion | Trace first, then commit the unchanged action-row supervisor |

The adapter never creates a successful forward from `tick*25`. Calling a
forward early, sampling without it, duplicating it or reporting a wrong-phase
integration fails closed. The generic tag means **last explicit forward**, not
the physical timestamp of every possible solver's internal caches. The pinned
Euler [source audit](../../fixtures/robotics/libero-panda/integrator-source-profile.json)
separately supports the one-step cached geometry/current-qpos interpretation
for the qualified scenes.

Unexpected SDK, source callback or controller failures latch both states as
halted. Adopt the returned state even when its status is non-ok. Physical
effects already performed cannot be rolled back. The host aborts without a
completed case ledger on unexpected failures; full cleanup/bounded-control
proofs remain separate. `pl_status` and `clock_status` are diagnostics: the
new top-level status decides whether a transition admitted.

## What was executed

The [qualification](../../fixtures/robotics/libero-panda/panda-clock-qualification.json)
separates three kinds of evidence:

| Evidence | Executed scope |
| --- | --- |
| Coax/new ABI | New authority checks, zero surviving root mutants; debug/release/ASan+UBSan paired-state ABI checks |
| Actual shared host header | Normal/delayed paths and seven effect faults in each native mode, using explicitly synthetic SDK/controller/source callbacks |
| Genuine MuJoCo 3.3.1 | Two new 10-row/250-step prefixes with authenticated recorded actuator controls, one normal and one delayed-first-reply |

The real prefixes use demo 001 and demo 000 respectively, with each demo's
own scene and reset state. Every post-step 45-F64 state and every 512-byte
trace record matches its original recorded prefix exactly. Requests occur at
0/200 for the normal case and 0/25/225 for the delayed case. Independent SDK
**forward-only** checks rebuild cached/fresh observation poses from the saved
states; all sampled F64 words and request-observation F32 bytes match exactly.
No second physics/reference rollout, controller arithmetic or neural provider
was initialized for those comparisons.
Trace reconstruction uses the new actual header/selected command/SDK state;
actuator words and clipping flags deliberately come from the recorded fixture.
Exact record equality does not claim a fresh controller calculation.

The command inputs are explicitly named **recorded-command fixtures**. They
copy selected action rows from `PTR1` and declare the unused test tail zero.
`PTR1` does not preserve all words of later model replies, so these fixtures
are never represented as authentic later neural output. Actual actuator words
are replayed unchanged. Prefixes are not task-success trials, and the two
different scenes are not a paired policy-outcome comparison.

The Metal and source-framework loop hooks compile against the additive
aggregate and execute through the same sidecar APIs in the synthetic host
test. A new whole-NN run through those hooks was deliberately not performed.
Camera effect order was preserved in the hook and checked by synthetic
callbacks; no new pixel/renderer qualification is claimed. Existing original
model, controller and scene evidence is retained rather than relabeled.

```shell
coax/bin/coax mutate psionic/lib/robotics/panda-clock-adapter.coax --root-only
COAX=coax/bin/coax psionic/tests/robotics/check-panda-clock.sh /tmp/new-paired-clock
COAX=coax/bin/coax psionic/tests/robotics/check-panda-clock-host.sh /tmp/new-clock-host
COAX=coax/bin/coax psionic/tests/robotics/check-panda-clock-prefix.sh SCENE_XML INIT_F64 ORIGINAL_PTR1 SCENARIO /tmp/new-clock-prefix
```

The SDK check is opt-in and authenticates the frozen scene/reset/trace/ledger
identities before loading the scene. Historical private artifact and dated
receipt names retain their original chapter numbers. This work adds no
steady-time deadline enforcement, inference cancellation, civil-time mapping,
generic maximum sensor age, Verse migration, complete snapshot or compiler
`#bounded` proof. Existing wall-latency statistics remain separate from the
simulation clock.
