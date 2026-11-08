# Shared actors, work in progress (#12992)

`phases.coax` compiles an authored task graph into an immutable execution plan.
It is the first part of the actor runtime; no playable consumer uses it yet.

The six ordered phases are accepted results/lifecycle, sensed knowledge,
action selection, movement, pose/sensor finalization and presentation. All
tasks in a lower phase precede tasks in a higher phase. Within a phase,
explicit dependencies resolve **stable task IDs**, independently of authoring
array order. A cycle, missing/duplicate dependency or unordered read/write
conflict refuses the complete plan. Ready tasks use increasing ID priority.
The runtime will consume the flattened task records, without dependency lookup.

Version 1 admits 1–16 tasks, 0–16 dependencies per task and positive signed
64-bit task IDs. Read/write masks name facts (bit 0), sensed knowledge (1),
intent (2), accepted transforms (3), poses (4), sensors (5), presentation (6)
and ready resources (7). Lifecycle may write bits 0–3; sense writes 1;
selection writes 2; movement writes 3; finalization writes 4–5; presentation
writes 6. Ready resources are read-only throughout this plan. These masks
describe execution dependencies, not a language effect proof. Native task
implementations and their actual effects still need qualification.

Use only a successful compiler result (`status == ok` and `plan.valid`). A
refusal returns an empty invalid plan and identifies the relevant authored
task/dependency where available. A typed `ActorPlan` copied from an untrusted
source is not itself evidence of admission. No durable plan wire format or
public ABI is introduced here.

```sh
coax/bin/coax test aethyr/actors/phases.coax --library
coax/bin/coax mutate aethyr/actors/phases.coax --library --root-only --jobs 8
coax/bin/coax run aethyr/actors/tests/phase-orders.coax > /tmp/actor-orders.txt
python3 aethyr/actors/tests/phase_oracle.py /tmp/actor-orders.txt
```

The [phase qualification record](../../docs/verification/aethyr-actors-12992/phases.md)
retains the exact tested scope. Remaining issue work includes compiled
archetypes/default inheritance, typed state/service/resource bindings and pool
admission; the bounded behavior/schedule/perception runtime; current-owner
navigation/model result admission; common player/AI rider input; guard and
villager playable adoption; and fixed-step native/Wasm replays, captures and
the residual-adapter inventory. This directory does not complete #12992.
