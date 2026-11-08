# Compiled actor profile 1 — #12992

The engine-owned `/worlds/actor-lab` adopts three offline compiled archetypes
through one shared actor pool and scheduler: a guard, a scheduled villager and
a mount-like controlled walking body. `aethyr/actors/content.coax` contains their
rules and instance overrides; `lab.coax` supplies the world adapter and controls.
Changing the guard's data changes its speed and sight range; replacing the
villager's search budget with one cell produces typed exhaustion. No private
per-archetype brain or scheduler was added to the realm.

This delivery starts from `87d9210c3a` (#12991), rebased with the incoming
actor compiler deliveries `331caba6f2` and `a70321f380`. Their definitions and phase
APIs are preserved. The native-rule execution profile is `profile.coax`, distinct
from the general authored archetype compiler. Actor Lab consumes the latter's
resolved parameter slots, field/root bindings and phase plan offline. The commit containing this record
is the source revision. The receipt manifest pins source, authored content,
compiler and output bytes; no published protocol or asset format was changed.
Unique module names were added to four existing state units so multiple actor
units can share the same declarations. Their function bodies are unchanged. The general archetype compiler now imports
resource types through their shared state/variant owner, avoiding a second logical
owner for RCM1. Its assets alias and native oracle driver are explicit; its
admission function bodies are unchanged.

## Contract and scope

Profile 1 admits at most 8 archetypes, 8 native rules, 8 schedule rows, 6 ordered
phases, 16 entity/actor slots and 128 navigation cells. Full SHA-256 identities
pin flat definitions and their world schema. Typed Coax authoring rejects unknown
fields/types; the compiler also rejects duplicate/missing IDs, inheritance and
phase cycles, unsupported body profiles and unresolved state/anchor references.
Instances carry stable entity IDs, generation handles, revisions, placement,
normalized overrides, scoped FST1 facts, knowledge, body and owned resource claims.

The six phases run on State Lab's existing 64 Hz paid clock: lifecycle/external
results, senses, choice, movement, finalized procedural poses, presentation. All
scoped state transitions preflight before any actor phase publishes. Resource
claims and all work reservations precede deferred entity publication. Refusals
preserve owned state or select the explicit conservative fallback; partial resource
holds roll back. Despawn clears retired components and cancels old model ownership.

Sensing uses the admitted semantic-query snapshot and bounded `Memory`. Priority,
weighted choices, cooldown, interruption, culling, grace, forgetting, schedule
predicates and direct time jumps are typed data. Navigation reuses shared grid
flow/downhill rules and reports pending/success/unreachable/exhausted/invalidated/
stuck, with bounded retries. Keys bind actor generation, control owner, world/nav
revisions, movement profile/radius and request serial. A wider actor than the
admitted grid clearance falls back explicitly. Model replies use existing
ReplaySession/DecideTable tickets and catalogues, not another external-result wire.

Movement uses one shared walker for human and AI input. Handoff cancels competing
actor authority while preserving knowledge. Read-only attributes and deduplicable
fear/interest notices are observations; INTERACT grants no conversation permission.

## Numerical and playable evidence

The scheduler's checks compose the actual lab in `checks.coax`. The qualified
journey hashes are checked at each of its 30 paid cuts, so a changed body or
knowledge field fails at the first affected cut. Separate fixtures cover actual
solid/sensor/self query selection, eye/height/culling endpoints, a grid's cell-zero
and distance horizon, one-axis blockage, request exhaustion, scoped clock skew,
unpublished/dying components, model expiry and editor ownership cancellation.
The shared walker is also checked against independently specified control tuples
and actual collision geometry. These are integration checks, not a second engine.

The independent Python oracle imports no engine implementation. It specifies
canonical binary hashes, a 128-step weighted state machine and a 128-step sight/
memory sequence, including cooldown, interruption, seed/counter and lost-target
boundaries. It also validates actual native/Wasm receipts against the public lab
script, lifecycle outcomes, data edits, work ceilings and control records.

The four schedules are 30, 60 and 144 FPS plus repeated 1/23/7/48/11 ms frames.
Each reaches 640 paid steps in 10 simulated seconds, with no dropped time.
All 30 recorded cuts and the final simulation digest match across native and
Wasm. Frame counters and native/Wasm storage layouts are deliberately excluded
from numerical comparison. Their actual values remain in the receipts.

Both independent control adapters produce all 64 identical world-axis control
records, including a hop at tick 120; their complete actor/world hashes agree.
Visual time with observation disabled preserves the final digest. Real canvas
pointer and keyboard input moves the player, hands control to the mount and moves
that body. The former player may coast under the existing walker's deceleration;
its held input is suppressed. Desktop 960×640 and phone 390×844 were captured in
headless muted Chrome and inspected. No window or audio was opened.

The accepted core contract hash is
`6f50c69f640eb1b3069e8c146ea61f838a2145895611835bd5c567cbeab7ebb3`.
Raw host receipts, control tuples, counters and oracle results are in `receipts/`.
Native ASan/UBSan completes the real 640-step replay and the independent core
probe. Actor Lab uses 16 MiB imported Wasm memory, a declared 4 MiB stack and a
400,000-byte module ceiling. Its fixed instance and frame scratch are not a
claim of dynamic crowd scaling. The larger stack is required by nested bounded
catalogue/instance drafts; the browser test caught an overflow with 1 MiB.

The original State Lab additionally matches #12991 at all 19 cuts and final state
on all four frame schedules after the shared module/host edits. The regression
hashes are recorded separately. Unchanged state function mutation evidence is
reused from #12991; unchanged navigation, query and walker gates retain their
existing qualification.

## Commands

```sh
coax/bin/coax test aethyr/actors/checks.coax
coax/bin/coax mutate aethyr/actors/profile.coax --root-only
coax/bin/coax mutate aethyr/actors/choices.coax --root-only
coax/bin/coax mutate aethyr/actors/perception.coax --root-only
coax/bin/coax mutate aethyr/actors/navigation.coax --root-only
coax/bin/coax mutate aethyr/actors/pool.coax --root-only --shard 1/2
coax/bin/coax mutate aethyr/actors/pool.coax --root-only --shard 2/2
coax/bin/coax mutate aethyr/actors/model.coax --root-only
# Run I=1..32, at most six commands at once under the shared worker budget:
coax/bin/coax mutate aethyr/actors/checks.coax --unit aethyr/actors/runtime.coax --shard I/32 --jobs 1
# Merge those unchanged-source records using the same --unit scope.
coax/bin/coax mutate aethyr/actors/signals.coax --root-only
COAXIAL_RUN_STEPS=250000000 coax/web/wasm.sh aethyr/actors/lab.coax apps/openagents-web/actor-lab-face.c apps/openagents-web/build/actor-lab.wasm
coax/web/tool.sh aethyr/actors/tests/actor-run.coax 60
coax/web/tool.sh aethyr/actors/tests/actor-browser.coax http://127.0.0.1:9379/worlds/actor-lab /tmp/actor-browser
python3 aethyr/actors/tests/oracle.py /tmp/actor-browser /tmp/actor-native
coax/bin/coax c aethyr/actors/lab.coax --run-steps 250000000 --library --header /tmp/actors.h > /tmp/actors.c
clang -std=c11 -O1 -g -fsanitize=address,undefined -ffp-contract=off -fno-fast-math -I/tmp /tmp/actors.c aethyr/actors/tests/sanitized.c -lm -o /tmp/actor-asan
/tmp/actor-asan
coax/web/tool.sh tools-check
python3 psionic/tests/book.py --book aethyr
```

Use separate output files for each native FPS and profile 1 for the AI receipt.
Mutation records must merge with the same `--root-only` or selected `--unit` scope and unchanged
source/compiler; use additional shards if needed to keep each command under
15 minutes. Passing unchanged gates are retained, not rerun for confirmation.

The chapter registry passes independently. The full book command is blocked by
an existing reviewed capability claim: `coax.wgsl.current` names changed evidence
in `verse/everglade/gpu.bindings`. This delivery leaves that unrelated claim and
chapter statuses unchanged and records the failure verbatim.

## Residual adapters

| Consumer/boundary | Remaining work and reason |
| --- | --- |
| General actor behavior/loading pool | The independently qualified `BehaviorPlan` and loading `ActorPool` source parts retain their wider intent/admission API; arbitrary programs need a physical action adapter. Actor Lab uses the smaller native-rule `choices.coax` and `ActorExecPool` profile. |
| Everglade watch, guard and routines | Private realm/room/replication rules remain; a production migration needs its own behavior/replay receipts. |
| Dead Gears rules adapter | Shared grid costs are reused; its gameplay adapter is not silently replaced. |
| Path graphs and followers | Existing `follower.coax` consumers remain; this profile supports grid walking only, without crowd avoidance. |
| Character/animation/physics profiles | Procedural walking body only; rigs, attack/damage verbs and other profiles refuse before spawn. |
| Mount seats and safe dismount | Attachment/action pipeline belongs to #12993; this delivery establishes shared input and control ownership. |
| Conversation/services | Existing talk and permissions remain separate; notices do not grant actions. |
| Models | Existing expiring intent gate/replay is composed; lab transport is deterministic, with no hosted account call. |
| Forge import/undo/UI | Typed authored records and lifecycle API exist; generic document translation remains an explicit adapter. |
| Complete actor save archive | Scoped facts use FST1. Lab save/load controls save the world facts; arbitrary actor/body/knowledge persistence and migration need a separate archive adapter. |
| Production asset import | Shared resource closures are held through #12990, but no new rig/asset importer is claimed. |

## Retained mutation evidence

| Unit | Killed | Survivors |
| --- | ---: | ---: |
| Execution profile compilation | 419 | 0 |
| Weighted behavior | 192 | 0 |
| Perception and knowledge | 292 | 0 |
| Navigation ticket gates | 185 | 0 |
| Instance/resource/control lifecycle | 474 | 0 |
| Model intent wrapper | 134 | 0 |
| Attributes and notices | 74 | 0 |
| Integrated paid scheduler | 570 | 0 |
| **Total** | **2,340** | **0** |

The six unchanged compilation/behavior/perception/navigation/model/notice units
retain their earlier passing runs. Their only source changes are collision-free
type/constant names and the execution-profile filename/module; `reused-gates.json`
checks exact renamed source equality against the integration basis. The new
`ActorOverrides` type added no procedure to that earlier compilation boundary.
The general archetype (316 mutants) and phase (137) compiler function bodies are
unchanged; their incoming qualification is retained under
`docs/verification/aethyr-actors-12992/`. Its 64-case independent native oracle
also passes after resolving the shared resource module owner.

## Final main integration

The final identifier-only integration preserves all 40 qualified execution/check
sources exactly after the recorded renames (`receipts/final-integration.json`).
This avoids colliding with the wider behavior/instance APIs delivered concurrently
on main. No algorithm, branch, bound or test expectation changed. The existing
instance/State Lab composition check still passes. The 64-case general definition
oracle is byte-identical. A fresh native replay matches every earlier byte; a fresh
Wasm build and browser replay pass all schedules, cuts, controls and real input.
ASan/UBSan passes the integrated 640-tick replay. Unchanged passing mutation
evidence is retained rather than rerun. `manifest.json` pins the final sources,
module, compiler, configuration, timings and measured peaks.
