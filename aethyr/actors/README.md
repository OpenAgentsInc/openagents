# Shared actors, work in progress (#12992)

`definitions.coax` flattens typed authored archetypes, and `phases.coax`
compiles their service graph into an immutable execution plan. The definition
and phase compilers are implemented; no actor execution or playable consumer
uses them yet.

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
coax/bin/coax test aethyr/actors/definitions.coax --library
coax/bin/coax mutate aethyr/actors/definitions.coax --library --root-only --jobs 8
coax/bin/coax run aethyr/actors/tests/definition-cases.coax > /tmp/actor-definitions.txt
python3 aethyr/actors/tests/definition_oracle.py /tmp/actor-definitions.txt
```

The [phase qualification record](../../docs/verification/aethyr-actors-12992/phases.md)
and [definition record](../../docs/verification/aethyr-actors-12992/definitions.md)
retain the exact tested scopes. Remaining issue work includes aggregate
pool/resource admission and instance lifecycle hooks; the bounded
behavior/schedule/perception runtime; current-owner
navigation/model result admission; common player/AI rider input; guard and
villager playable adoption; and fixed-step native/Wasm replays, captures and
the residual-adapter inventory. This directory does not complete #12992.
