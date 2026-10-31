# Owned oscillator snapshots: an R16 precursor

`oscillator-replay-f64-v1` checkpoints the bounded, owned one-axis oscillator
and replays its accepted forces. It is a small consumer of the requirements in
[ROADMAP R16](../../../coax/ROADMAP.md#19-r16--deterministic-replay-and-snapshots),
not completion of R13 typed clocks, R15 deterministic jobs/math or R16 robot
and Verse replay. Chapter 109's solver and chapter 111's inverse-physics code and
records are unchanged.

```shell
coax/bin/coax test psionic/lib/physics/oscillator-replay-codec.coax
coax/bin/coax mutate psionic/lib/physics/oscillator-replay.coax --root-only
coax/bin/coax mutate psionic/lib/physics/oscillator-replay-codec.coax --root-only
psionic/tests/physics/oscillator-replay-check.sh /tmp/oscillator-replay-native-new
```

The wrapper is for reproduction in a fresh directory. Initial qualification
ran its debug, release and sanitized modes separately, then ran the new mode
comparison/cross-build gate. Passing unchanged gates were not repeated.

## The API owns state changes and ordered acceptance

Product logic is Coax. Native C in the test harness only supplies platform
observations, fixture inputs, effects and independent assertions. Python is
development tooling, never a runtime dependency.

| Procedure | Contract |
| --- | --- |
| `or_begin(params,state,dt,ids)` | Creates an admitted snapshot at completed tick 0 with no pending inputs. |
| `or_accept(snapshot,forces)` | Validates the entire incoming batch, then appends it in order; refusal returns the original snapshot. |
| `or_advance(snapshot)` | Consumes the first accepted force and runs the exact original 109 `os_step`; returns the next snapshot and one completed-transition record. |
| `or_encode(snapshot,out,overlap)` | Stages the complete canonical record before writing; preserves any destination tail. |
| `or_restore(bytes,expected_ids,out,overlap)` | Requires one destination snapshot; validates the whole record/context before one final assignment. Every refusal preserves the destination. |
| `or_replay(snapshot,out,overlap)` | Stages up to128 queued transitions and publishes all or none. It supplies no new external acceptance events. |
| `or_first(left_ids,right_ids,left,right)` | Refuses different identity contexts or inadmissible traces; reports the first exact differing recorded field. |

The queue holds at most 128 forces and each replay call at most 128 transitions.
The completed global tick may reach 2^31−1; checked remaining clock headroom
prevents append/advance overflow. A 720-tick episode therefore fits across
multiple acceptance/replay calls. These are ordinary integers and F64 step
values, with no typed-clock or wall-time guarantee.

The snapshot stores only forces already accepted. A force's application tick
is the current completed tick plus its FIFO ordinal. Later acceptance calls
and their timing remain external. Restoring a checkpoint does not manufacture
those inputs or restore an external producer's generator/cursor.

## Inventory at a completed RK4 boundary

| State | Treatment |
| --- | --- |
| Mass, spring, damping | Included; admitted under the original fixed-mass solver profile. |
| Current position and velocity | Included as exact binary64 words. |
| Completed tick, fixed dt, accepted horizon | Included; `accepted_end = tick + pending`. |
| Accepted pending forces | Included in FIFO order, including active signed-zero words. |
| Solver-source, numeric, target, build and run identities | Included as five nonzero 32-byte digests; must equal the caller's expected context. |
| Worker/integration inventory | Fixed serial worker 1 and completed-step history kind 0. |
| RK stage scratch or warmstart | Absent by design: this RK4 step is Markov in position/velocity, parameters and its force. There is no persisted integration history. |
| RNG or learned-model/optimizer state | Absent: none participates in this plant/replay component. An external force generator or future policy would need its own inventory. |
| Entity IDs/generations, structural commands, jobs | Absent: one implicit oscillator, no entity system, structural queue or parallel scheduler. This does not implement R5/R15 state identities/schedules. |
| Future inputs not yet accepted | External; re-supply the same recorded events in order and at the same logical boundaries. |
| Identity manifests, actual executable and FP environment verification | External trusted context, not reconstructed or attested by the codec. |
| Files, network/GPU/device handles, addresses and allocation state | External resources; reconstruct them. No host pointer is serialized. |

A snapshot is meaningful only at the declared completed-step boundary. There
is no representation for an interrupted RK stage. Pure value updates allow
rollback here; committed external effects, robot warmstarts, Verse resources,
policy caches, model RNG and transactional migration are separate work.

The fixture captures after acceptance events at a logical boundary and before
its next integration step. On resume those already accepted events are not
sent twice; later external events are supplied again. This is an explicit
fixture convention, not a general event-log/session protocol.

## OSRP v1: canonical bytes

The [profile contract](../../fixtures/physics/oscillator-replay-profile.json)
specifies a fixed 1,312-byte little-endian record. It is never a C struct dump.

| Byte offset | Contents |
| ---: | --- |
|0|`OSRP` magic; u16 version 1; u16inventory mask 31; u32 size 1312; u32 reserved 0 |
|16,48,80,112,144|32-byte solver-source, numeric, target, exact-build and run identities |
|176|u64 completed tick |
|184|F64 dt |
|192,200,208|F64 mass, spring, damping |
|216,224|F64 position, velocity |
|232,240|u64 pending count and accepted end tick |
|248,252|u32 worker count 1 and integration history kind 0 |
|256|128 F64 force words; inactive lanes must be canonical positive zero |
|1280|32-byte tagged SHA-256 of all 1,280 preceding bytes |

The tag is `psionic.oscillator.snapshot.v1`. Its digest is:

```jai
tag_hash = SHA256("psionic.oscillator.snapshot.v1")
digest = SHA256(tag_hash || tag_hash || body[0:1280])
```

The decoder rejects different length/version/inventory, reserved-byte changes,
unsupported workers/history, stale checksums, invalid clock headroom,
nonfinite/out-of-profile values, noncanonical inactive lanes and mismatched
identities. It checks raw floating words before conversion. A valid active
`-0` is preserved; inactive `-0` is rejected as a second encoding of an unused
lane.

Tagged hashing detects changed bytes; it does not authenticate a forged
record whose hash has been recomputed. Likewise, identity equality is not
machine attestation. The caller must authenticate expected context and input
records. Deliberately re-encoded but physically valid changed state is a new
record; the reference trace reveals its divergence.

Only this snapshot is a versioned wire format. `OrFrame` is a diagnostic ABI
object; the harness's local trace file is a qualification artifact, not a
published R16 event-log format. Crash-safe file publication, fsync/rename and
positive process-restart lifecycle are not qualified by this primitive.

## Exact builds and numeric scope

The native harness hashes the actual completed executable after compilation
and supplies that hash as the build identity. It separately records the
original 109 source manifest, target/architecture, FE_TONEAREST and observed
ARM64 FP control register. The driver checks that control state stays fixed.
The same expectations are supplied to Coax; Coax compares them before restore.
The library does not inspect the machine or change/restore FP control state.

The reproduction wrapper refuses targets other than Darwin ARM64. The
qualified host uses serial binary64 RK4 with contraction disabled and
`-fno-fast-math`. The actual source has no transcendental dynamics. Debug,
release and ASan+UBSan executions have different executable hashes. Their
state/input/tick traces match exactly, while their snapshots intentionally
differ in build identity and checksum. Each of the six ordered cross-build
restore attempts refuses with `build`, leaving the destination unchanged.

This evidence is for the qualified host and its observed environment. It is
not cross-architecture determinism, host-libm equivalence, a general strict
math implementation, a scheduler proof or a full R15 profile.

## Mid-run restore and useful first differences

Five frozen scenarios produce 24 checkpoints and 980 original transitions:

| Scenario | Checkpoint/continuation evidence |
| --- | --- |
| Incremental 128-step run | Accept 32 forces at ticks 0,16,48,80; checkpoint at 43 contains 21 accepted pending inputs. Later events must be re-supplied. |
|720-step run | Accept batches of at most 32; checkpoint before/after batch boundaries and in the middle of the run. |
| Clock-headroom fixture | Start a manufactured admitted state at global tick 2^31−1−128 and finish 128 steps exactly at the cap. This does not simulate billions of historical steps. |
| Signed-zero inputs | Preserve `-0` and `+0` applied-input words and detect their difference even when resulting state words coincide. |
| Empty queue | Encode/restore an empty accepted horizon; replay writes zero records and advance reports `empty`. |

Every checkpoint restores within its exact executable and continues to the
uninterrupted endpoint, re-supplying the recorded later acceptance events.
The original unchanged109 solver independently checks every original state;
a separate Python implementation constructs all expected canonical bytes and
all 980 complete input/state/tick records. Every word and byte matches exactly.

Each `OrFrame.tick` names the completed state boundary. Its force was applied
at `tick-1`. The comparator checks tick, then applied force, position and
velocity, so a changed input cannot hide behind coincident states. Source IDs
name the differing recorded output's definition, not necessarily the earliest
internal cause.

| Deliberate change | First difference |
| --- | --- |
| Replace the checkpoint's current state | Tick 44, position, `os_step:next.position` |
| Reset its completed clock to zero | Tick 44, completed tick |
| Change its admitted dt | Tick 44, position, `os_step:next.position` |
| Explicitly empty its accepted pending queue | Tick 44, trace length |
| Omit the later unaccepted event at 48 | Tick 65, trace length |
| Reverse the later accepted batch at 48 | Tick 65, applied force, `accepted_force_order` |
| Change applied `+0` to `-0` with identical state | Tick 1, applied force |

Unknown versions and missing inventory categories refuse rather than default.
Valid re-encoded changes are compared explicitly; later inputs declared
external are not silently recovered.

## Qualification

Thirteen authored checks kill 463/463 changed-root mutants with zero survivors.
Each native mode passes 20,733 assertions, 24 checkpoint restores, 980 original
transitions, all 1,312 single-byte corruptions and 61 independently constructed
semantic faults. All refusals tested preserve the destination. Seven controlled
divergence cases have closed expected tick/field/source coverage. Independent
parsing also rejects each semantic fault fixture.

The full evidence, real build hashes, FP context, trace/snapshot hashes and
limits are in [the qualification receipt](../../fixtures/physics/oscillator-replay-qualification.json).
No passing unchanged 109/111, model, mutation or native gate was repeated.

R16 still needs the complete robot and Verse consumers, stable entity/state
identities, recorded typed clocks, qualified jobs/math, model/controller state,
external effect commits and transactional rollback. This primitive supplies
a small owned-solver checkpoint with a deliberately inspectable boundary.
