# Checked state changes

Coax 0.52 adds typed handles, enforced procedure effects, recoverable in-memory
transactions and published-surface compatibility. Everglade uses them for
references that survive a tick, shared page/room rules and character checkpoints.
These mechanisms do not establish general memory safety: view regions, a checked
native boundary and concurrency remain separate work.

## Handles

`#load "coax/lib/handle.coax"` provides `Handle(T)` and `Store(T)`. A handle is
an `@id` record with a `u64 value`: its low 32 bits are the slot, its high 32
bits the generation. Zero means absent; live generations start at one.
Different `T` specializations are different nominal domains. Integer conversion
and serialization require explicit extraction and construction.

A store borrows caller-owned value, generation, liveness, free-list and counter
arrays of equal capacity. The caller initializes them to zero and keeps them
alive and exclusive for the store's lifetime. Copies of a store borrow the same
backing. A handle is local to one store; matching `T` does not establish which
store owns it. Region checking is still planned.

`store_put` returns an optional handle. `store_get` returns `([]T)?`; an absent,
stale or retired handle returns `none`. `store_get_live` asserts liveness and
returns a view for code that already established it. Removal advances the
generation. The last generation retires the slot instead of wrapping.

The OpenAgents Metaverse Engine uses `Handle(Entity)` while preserving its ordered flush, stable
`EntityId` column and existing 4,096-owner retirement limit. `entity_get` returns
an optional entity value; components stay in the realm's columns. Dead Gears'
callers also carry the typed handle. Saves and messages use stable IDs.

Everglade keeps a character's seat lifetime separate from its socket generation.
A reconnect preserves it; replacement advances it. Room casts, queued strikes, accepted world impacts and
ability owners capture that lifetime. Town, placed-object, forest, grass and
debris fires capture their starter's handle. Contacts, spread, falling trees
and forest replication resolve it before assigning blame. Fire may continue
after its owner leaves. Shouts carry the speaker's lifetime and
clear when the replicated slot is replaced. Exhausted lifetimes retire.

## Effects

```coax
step :: (state: []State) #effects(reads, writes, traps) {
  state[0].tick += 1;
}
clock_now :: () -> int #foreign #effects(foreign, clock);
```

`#effects(...)` bounds the transitive effects of a procedure, including concrete
generic calls and deferred cleanup. The compiler refuses an undeclared effect
and reports a shortest call path to its source. Declarations are required on
`#export_c`, `#foreign`, and procedures in `#authority` units. Other procedures
are inferred. Entry `main` and generated check procedures are exempt from the
presence requirement. An explicit bound is checked everywhere.

| Capability | Meaning |
| --- | --- |
| `computes` | Empty set |
| `reads`, `writes` | Read or write borrowed storage, including string reads |
| `traps` | Bounds, arithmetic, assertion, tag and other fatal checks |
| `io`, `foreign` | Source I/O and native calls |
| `unsafe` | Uninitialized or otherwise unmodeled unsafe storage access |
| `clock` | A declared host clock or nondeterministic source |
| `suspends`, `allocates` | Reserved capabilities for future language operations |
| `all` | Every capability in this table |

`#pure` is the empty set. It does not permit traps. A fatal check's diagnostic
is not source I/O. Unknown operations conservatively require every capability.
Foreign declarations are trusted summaries and always carry `foreign`; the
compiler cannot inspect the native implementation. Native regions and unsafe
blocks are not implemented by this effect vocabulary. Coax still has no
first-class procedure values or callback type syntax; their admission must
preserve these bounds when that language feature arrives.

The repository's existing boundary and authority procedures declare `all` unless
they already had a narrower contract. Everglade's shared movement and rules
procedures explicitly permit only reads, writes and traps. Adding a clock,
foreign call or I/O operation to their transitive closure fails compilation.
The conservative `coax.effects.v1` resource report retains its existing schema;
source effect admission uses the separate policy in `effect-contract.coax`.

## Transactions

```coax
change :: (state: []State) -> bool? #effects(reads, writes, traps) {
  #transaction {
    state[0].amount += 1;
    if state[0].amount > 10 { return null; }
  }
  return some(true);
}
```

A normal block exit commits. Returning `Result.err` or optional `none`, directly
or through `try`, restores the block's state before the refusal leaves. A caught
failure that does not escape the block does not abort it. Nested blocks compose;
inner cleanup runs before undo, outer cleanup after undo. Traps remain fatal and
do not promise rollback.

The evaluator and C emitter retain before-images of initialized visible values
and their reachable mutable view backing at entry. This coarse write log covers
writes made through helpers and aliased views. Covered spans share saved bytes;
view traversal tracks cycles. It uses host allocation and admits at most 256
levels of traversal. Exceeding that bound or exhausting allocation traps. It is
not a fixed-memory or real-time guarantee.

Transaction bodies permit only reads, writes and traps. I/O, foreign, clock,
suspension, allocation and unsafe effects refuse, including through callees.
Procedures with uninitialized local declarations conservatively refuse capture.
Refusal payloads own ordinary data: a transactional `Result` error type cannot
contain views, strings, nominal IDs or handles. This prevents an aborted
allocation from publishing a reference whose generation has also been undone.
Successful results may carry handles. Device backends refuse transactions.

Everglade wraps shared multi-entity actions, repair release and batched fire
damage. A refused third target restores earlier targets, dice and hit events.
A full repair outbox restores its cooldown and cast phase. The in-memory ledger
restores item ownership and versions when its outbox append refuses. Saved
commits, trades and checkpoints keep their service operation IDs and revisions;
this language block does not make durable writes atomic across a crash.

## Persisted values and compatibility

`Record :: struct #persisted { ... }` admits owned scalars, enums, nominal IDs,
records, fixed arrays and tagged sums of admitted values. It recursively refuses
views, strings and process-local `Handle(T)` values. Generic records are checked
when instantiated. A saved reference uses its stable ID and is resolved on load.
The marker does not generate a serializer or change an existing wire version.
Everglade's input/cast/interaction records, owned hello value and character
capture carry the marker. The older borrowed hello codec remains a boundary
adapter; active page/room code uses the owned value and preserves its wire bytes.

```
coax compat OLD.coax NEW.coax
coax compat --emit MODULE.coax > MODULE.surface
coax compat OLD.surface NEW.coax --baseline PRODUCTION_REMEASURED.surface
```

The initial manifest profile is `coax.compat.v1`, native LP64, development build
and checked arithmetic. It records published procedures and types, reachable
C ABI layouts, actual inferred effects, persisted fields and evaluated defaults.
A sealed module uses its `#export` list; an unsealed unit publishes its own
declarations. Marked persisted records are included even when private.

The check refuses removed or renamed declarations, changed signatures, added
effects and changes to an existing C ABI. Nominal parameter and result types must match. View parameters may widen from
mutable to read-only, and view results may narrow from read-only to mutable;
the reverse changes refuse. Arity and calling convention must remain fixed.
Persisted records may append explicitly defaulted fields; existing order, names,
types and default values must remain compatible. Nested persisted values follow
the same rule. Changing union alternatives refuses. Published generic templates
require a concrete facade; unsupported or malformed surfaces fail explicitly.
This does not compare arbitrary serializer implementations or protobuf schemas;
those retain their existing protocol checks and versioning rules.

`--baseline` suppresses only incompatibilities already present when production's
source is measured with the candidate compiler. It compares the properties still
failing; fixing one fault does not make an unchanged fault block the release.
A new failure, a different bad value or a newly removed declaration blocks it.

`release.sh` compares the message and checkpoint facades in `coax/web/compat/`
with production's `PASSED` commit. The same reviewed facade loads each revision's
source. It exposes the message values and saved character; marked persisted
records are included automatically. Receive histories, delta tables and mailbox
scratch can grow independently. Versioned codec checks still qualify the text
bytes. The general `coax compat` command compares a module's full published
surface. Room `deploy.sh` checks every revision receiving traffic before reading
ticket keys or draining rooms. Revision labels retain the source commit, with
existing image tags as the migration path. Each qualified build retains its
manifests under `coax/web/compat/`; releases archive them and room images include
them. Before this profile existed, the retained production source supplies the
baseline. The gate never executes scripts from the old checkout.

## Verification

The component mutations, native/evaluator regressions, production-surface
comparison and Everglade qualification are recorded in
[`docs/verification/coax-checked-state-2026-11-06`](../docs/verification/coax-checked-state-2026-11-06/README.md).
`coax/tests/run.sh --only handles effect_contracts transactions compat` runs the
focused language/library gates. For tick qualification, `everglade-drift
--tick-us 600` computes the nearest-rank p95 from every completed worker cost
batch collected during the script. It retains the raw samples, rounds each
duration up to microseconds and refuses missing, malformed or overflowing
observations. `--offline native --tick-us 840` checks a separate native local
room's reported p95. Both gates refuse an observation above their explicit limit.
