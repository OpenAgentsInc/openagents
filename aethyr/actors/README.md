# Shared actors, work in progress (#12992)

`definitions.coax` flattens typed authored archetypes, and `phases.coax`
compiles their service graph into an immutable execution plan. The definition
and phase compilers are implemented. `behavior-plan.coax` compiles bounded
native intent programs and `behavior.coax` selects their commands on paid
fixed steps. `resources.coax` and `instances.coax` add root claims, bounded
instance admission and an entity lifecycle. Physical execution and playable
adoption remain.

Load these as **source parts**, without `as`: `#load "instances.coax";`.
They share the consumer's logical owner with the existing state/stream source
parts, so a host can compose actors with State Lab. The independent phase
compiler remains a named module. The former actor module aliases were internal
to these fixtures; their removal changes no saved/protocol/asset format or C ABI.

## Compiled definitions

Version 1 authors `ActorDefinition` values in Coax. The language rejects
unknown struct fields; parameter rows additionally reject unknown numeric
keys, wrong value kinds, noncanonical values and duplicate overrides. A
library has up to 16 definitions, each with a positive stable ID, revision
and optional parent ID. Parents may appear anywhere. All declared references
and the whole library's parent graph pass admission before a result escapes.

Defaults come from one typed parameter schema. Child rows override individual
parameters and service roles. Field/root groups use `-1` for inheritance and
zero to replace with an empty group. Group replacement clears old tails.
Compilation produces an owned record with root-to-leaf revision provenance,
20 fixed parameter slots, six resolved native-service records and their exact
input slots, a flat task order, up to eight typed fact bindings and eight
resource roots. It retains no authoring strings or views into input tables.

Fact references bind the existing state field's ID, kind, scope and owner;
compiled bindings keep its slot and the complete schema SHA-256. Resource
references bind complete content identity **and** `ResourceSpec`, then keep
RCM1 owner indices and a deduplicated closure over all 64 possible owners.
Compilation reserializes and hashes the complete canonical RCM1 metadata,
including costs and dependency identities, against its retained manifest pin.
This does not authenticate source bytes: the actual stream's `resource_matches`
and decoder remain responsible for those bytes and physical admission.

The parameter schema declares controller, animation and physics selections;
radius/speed; discover/alert/track bands, horizontal cosine, vertical and cull
distance; eye height; forget/lost/cooldown ticks; sense/search/action work,
instance capacity and repeat suppression. See `actor_parameter_schema` for
exact defaults, types and limits. Bands must satisfy discover ≤ alert ≤ track
≤ cull. The initial definition profile permits stationary, route or rider
controllers with kinematic physics and absent/in-place animation; a rider
requires in-place animation. Rigid and root-driven combinations refuse.
This matrix admits definitions; it does not qualify physical actor execution.

Services are supplied by a trusted typed native catalog (up to 32 entries),
one of each pipeline role. Each declares version, implementation, input roles
and read/write channels. Version 1 maps each role to its corresponding native
implementation. Cyclic or unsafe service declarations refuse through the
shared phase compiler. Instance creation, lifecycle hooks, entity ownership,
actual resource reservations and aggregate pool/work admission still need
the execution boundary below. A compiled definition is never a spawn grant.

## Phase order

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

## Instance admission and lifetime

`actor_pool_open` owns 16 fixed slots with a configured capacity of 1–16.
`actor_pool_request` reserves pending-instance capacity, per-archetype capacity
across revisions, and aggregate declared sense/search/action work. Pending,
born, live and retiring slots all retain their quotas until removal. Admission
checks placement bounds, program/archetype/schema linkage and program identity
before taking every resource root as one atomic set. A failed claim spends no
stream serial or partial reference. Existing stream budgets still govern actual
decode/upload allocations; actor work reservations are not GPU-memory grants.

An actor has an authored durable ID and archetype revision, a runtime key
(pool slot, actor ID, generation, owner, world epoch), a placement, a behavior
cursor and bounded sensed memory. The entity store separately supplies its
session-local `EntityId` and checked handle. No durable document stores a handle.
Duplicate actor IDs refuse, including IDs still loading or retiring. Generations
and owner counters refuse at their caps instead of wrapping. Pool/stream and
entity storage belong to one world lifetime; a replacement world must receive
a new epoch, retaining the old stream until its effects/borrowers retire.

`actor_pool_begin` runs once per paid step. A loading actor queues an entity
only after its full retained root set is ready. It becomes available to actor
systems on the step after `entity_flush`, with its cursor, placement and memory
already initialized. Birth, activation, retirement, removal and pressure are
typed lifecycle events, at most one per configured slot per step. Entity-capacity
pressure keeps the actor loading, counts the refusal and permits a later retry.
The realm still flushes its common entity store at the end of each paid step.

`actor_pool_current` checks the complete key, live phase, actual entity handle
and ID, and current root readiness. `actor_pool_select` uses it before committing
any behavior cursor. Stop immediately revokes actor admission and queues the
shared despawn. Quotas/claims release only after that entity is gone. Corrupt
retained receipts produce counted pressure without releasing another claim.
Whole-stream retirement revokes all claims atomically; after entity retirement
the pool can then free its quotas, while concrete resource owners retain their
allocations until physical destruction and final borrower release.

`actor_pool_transfer` rotates the owner key, cancels an active action into its
cooldown and preserves random counters and sensed memory. It records a control
source (AI or human); controller adapters must route that source. This boundary
does not yet implement rider input, human/AI routing, attachment or dismount.
Nav/model callbacks must use the current-instance gate in their eventual
adapters; this checkpoint gates behavior commands, not those callbacks yet.

Pools, archetypes and resource receipts are owned runtime records, not public
decoded wire structs. This unit initializes the common cursor/memory/placement
columns; movement, physics, animation, sensors and actual service-hook dispatch
still need the adopted execution path. Per-instance parameter/fact overrides
and persistence remain part of that integration.

Use only a successful compiler result (`status == ok` and `plan.valid`). A
refusal returns an empty invalid plan and identifies the relevant authored
task/dependency where available. A typed `ActorPlan` copied from an untrusted
source is not itself evidence of admission. No durable plan wire format or
public ABI is introduced here.

## Bounded behavior selection

`behavior_compile` binds an admitted archetype and its complete state-schema
identity to an owned program: up to eight priority sequences, four conjunctive
guards and four actions per sequence, eight world-time schedule rows, and
16 stable anchors. The declared action-work budget must cover all rows.
Guards compare declared typed facts, supplied sensed/path flags or the current
schedule. They contain no runtime expressions or display-text dispatch.

Native verbs are HOLD, GO, CHASE, FLEE, INTERACT, SAY and CONTROLLED. GO uses
a fixed admitted anchor or the current schedule; CONTROLLED requires a rider
archetype. Stationary controllers cannot compile movement verbs. This is
typed intent, not a claim that a movement or speech body has executed it.

`behavior_project` copies only declared facts from an admitted `StateStore`.
`behavior_open` binds a cursor to program identity, actor ID, generation,
owner, world epoch and seed. `behavior_step` requires the exact next already
paid `FrameClock` step. It adds no clock. A refusal returns the old cursor
and no command/events; the caller must discard stale feedback and complete
that same paid step with admitted input before publishing.

Lower priority numbers win. Rows are compiled in priority/full-ID order.
Weighted choice consumes exactly one existing counter-based random draw per
fresh choice, even for a sole option; continuation and idle consume none.
Repeat suppression excludes the last choice only when an alternate in the
same winning priority group is eligible. Cooldowns use authoritative ticks.
HOLD/SAY complete at their duration; other verbs fail at timeout unless
matching body feedback completes them. Action serials bind body feedback to
the full actor/generation/owner/world-epoch key. A next action with unavailable
inputs fails without minting a serial. An interruptible action can yield to
guards or a higher priority. Lost/replaced targets force cancellation even
for a noninterruptible action. A time jump selects only the current schedule
row, cancelling and retargeting a scheduled GO without visiting skipped rows.

Each successful step reports bounded work and at most two events. Selection
evaluates at most 36 guards, eight sequences, one draw and one command;
counters refuse at their declared cap without changing the old cursor.
The program fingerprint binds all active fields, guards, actions and schedule
data. Programs and projected facts are trusted internal admitted snapshots,
not an untrusted wire format. In particular, body feedback correlation does
not prove that an entity or owner is still current in the live world.

```sh
coax/bin/coax test aethyr/actors/phases.coax --library
coax/bin/coax mutate aethyr/actors/phases.coax --library --root-only --jobs 8
coax/bin/coax run aethyr/actors/tests/phase-orders.coax > /tmp/actor-orders.txt
python3 aethyr/actors/tests/phase_oracle.py /tmp/actor-orders.txt
coax/bin/coax test aethyr/actors/definitions.coax --library
coax/bin/coax mutate aethyr/actors/definitions.coax --library --root-only --jobs 8
coax/bin/coax run aethyr/actors/tests/definition-cases.coax > /tmp/actor-definitions.txt
python3 aethyr/actors/tests/definition_oracle.py /tmp/actor-definitions.txt
coax/bin/coax test aethyr/actors/behavior.coax --library
coax/bin/coax mutate aethyr/actors/behavior-plan.coax --library --root-only --jobs 8
coax/bin/coax mutate aethyr/actors/behavior.coax --library --root-only --shard 1/2 --jobs 8 > /tmp/behavior-1.rec
coax/bin/coax mutate aethyr/actors/behavior.coax --library --root-only --shard 2/2 --jobs 8 > /tmp/behavior-2.rec
coax/bin/coax mutate aethyr/actors/behavior.coax --library --root-only --merge /tmp/behavior-1.rec --merge /tmp/behavior-2.rec
coax/bin/coax test aethyr/actors/instances.coax --library
coax/bin/coax mutate aethyr/actors/resources.coax --library --root-only --jobs 8
coax/bin/coax mutate aethyr/actors/instances.coax --library --root-only --shard 1/2 --jobs 8 > /tmp/instances-1.rec
coax/bin/coax mutate aethyr/actors/instances.coax --library --root-only --shard 2/2 --jobs 8 > /tmp/instances-2.rec
coax/bin/coax mutate aethyr/actors/instances.coax --library --root-only --merge /tmp/instances-1.rec --merge /tmp/instances-2.rec
```

The [phase qualification record](../../docs/verification/aethyr-actors-12992/phases.md)
and [definition record](../../docs/verification/aethyr-actors-12992/definitions.md),
plus the [behavior record](../../docs/verification/aethyr-actors-12992/behavior.md),
retain the exact tested scopes. The behavior trial checks selection from
scripted observations/body outcomes at 30/60/144 FPS and uneven frames in
native C and actual browser Wasm. The
[instance record](../../docs/verification/aethyr-actors-12992/instances.md)
qualifies bounded admission, the actual shared entity/stream lifecycle and
native/browser lifecycle-selection replay. Neither trial draws or moves actors.

Remaining issue work includes actual native service-hook dispatch, per-instance
overrides/persistence and physical pool allocation traces; perception/query
integration with discover/alert/track,
culling and lost-target counters; current-owner navigation/model admission and
failure/retry modes; common player/AI rider input; guard and villager playable
adoption plus a mount-like fixture; resource retirement; actual adopted
native/Wasm replays and captures; and the residual-adapter inventory. This
directory does not complete #12992.
