# Actor instance and resource lifetime (#12992, partial delivery)

Qualified source base: `8f76175c8d`, plus the change containing this record.
The [source inventory](instances-source-hashes.txt) pins the actor sources and
fixtures. Rebase onto `e38a1b3480` changes only unrelated SRD/research files.
This checkpoint adds common instance admission and lifecycle. It does not
complete #12992 or establish playable guard/villager/mount adoption.

## Boundary and composition

`resources.coax` takes an archetype's complete root set through the existing
RCM1 stream. Speculative stream copies make a failed multi-root claim atomic:
no consumed serial, prefix claim or partially initialized owner escapes.
Readiness requires every retained live claim and its ready concrete owner.
Release keeps the stream's dependency and borrower rules; cancellation is
never permission to free a physical allocation.

`instances.coax` owns a fixed 16-slot pool, with configured capacity 1–16 and
aggregate declared sense/search/action caps. Pending, born, live and retiring
actors all retain their instance/work reservations. Per-archetype capacity
counts all revisions of the same authored ID. Admission refuses duplicate
actor IDs, exhausted generation counters, incompatible program/schema/revision
links, invalid placements and root/aggregate pressure before publication.
The placement profile is f32 metres within ±1e9 on each axis and yaw within
±1e6 radians, inclusive, rejecting NaN and infinities.

The authored actor ID is separate from the entity store's session `EntityId`.
Runtime keys bind slot, full actor ID, generation, owner and world epoch.
`actor_pool_current` also checks the actual live entity handle/ID and retained
resource readiness. A behavior cursor commits only under that current key.
World replacement requires a new epoch and retention of the old stream until
its effects/borrowers finish. These are owned runtime records, not decoded
wire structs or remote capability tokens.

On a paid step, loading actors queue entities only after all roots are ready.
Common cursor, bounded memory and placement already exist before the entity
becomes visible. Activation waits for the shared `entity_flush` and sets the
new cursor to the preceding paid tick, without creating another clock.
Stop immediately revokes actor command admission and queues the despawn.
Removal releases quotas and root claims only after the entity is gone.
Typed events report birth, activation, retirement, removal or pressure;
each configured slot emits at most one per step. Pressure is counted and
keeps retained state available for a later retry or repair.

Whole-stream `resource_stream_retire` clears every claim atomically. The
instance boundary recognizes that revocation and, after entity retirement,
can release its own quotas without waiting for an already revoked claim.
The resource stream still holds concrete allocations through actual
destruction and final borrower release. A closing diagnostic record that
still has live claims uses normal exact claim release.

Control transfer rotates ownership, cancels an active action into cooldown,
and preserves random counters and sensed memory. It records the selected
input source. Human/AI input routing, common rider input and attachment are
not implemented by this boundary; the eventual controller adapter must use
that source. These tests establish owner-key invalidation and cancellation,
not full mount/control parity.

The actor compiler/selector/instance units are now **source parts** loaded
without aliases. They join the consumer's logical owner, as the existing
state/stream units require. Their former named modules claimed those units
under a different owner and could not compose with State Lab (E0607).
The independent phase compiler keeps its named module. No public asset,
save, protocol or C ABI changes. All in-tree uses of the former actor aliases
were these fixtures and have been updated.

`instance-compose.coax` loads the actual State Lab fixture and instance API
together. Final Coax C emission and GCC compilation both pass. Generated C
SHA-256: `a8b9a2662413bceb4687c9365bd8ea0fd079bd29fade8f419c84855bf57ee7fa`.
This proves source/C composition, not playable adoption or rendered State Lab
integration.

## Checks and mutation

The selected actor baseline passes 40/40 checks among the lifecycle entry's
450 loaded checks; its [log](instances-selected-checks.txt) explicitly says
that `--only` skips authority coverage. Final mutation runs use the complete
baseline and authority execution coverage, with entry-source mutation scope:

| Unit | Killed | Survivors | Evidence |
| --- | ---: | ---: | --- |
| Definitions after composition change | 316 | 0 | [report](definitions-composed-mutate.txt) |
| Behavior compiler after composition change | 391 | 0 | [report](behavior-plan-composed-mutate.txt) |
| Selection after composition change | 765 | 0 | [report](behavior-composed-mutate.txt), [1](behavior-composed-1.rec), [2](behavior-composed-2.rec) |
| Root leases | 82 | 0 | [report](resources-mutate.txt) |
| Instance lifecycle | 395 | 0 | [report](instances-mutate.txt), [1](instances-1.rec), [2](instances-2.rec) |

These repeat the earlier definition/behavior gates because their source
ownership and qualified names changed. The phase source/checks did not
change; its 137/137 result at `a70321f380` is reused. No whole-engine mutation
claim is made.

The tests cover exact pool/work bounds and peaks; malformed metadata and
program linkage; pending-instance capacity; duplicate IDs; resource-claim
atomicity; delayed/withheld flushes; cancelled births; live entity death and
ID replacement; all runtime-key lanes; entity pressure and repair; retained
bad receipts; world shutdown; both owner-transfer directions, idle and active
transfer, and exhaustion. Root tests additionally exercise eight independently
compiled roots and a real dependency closure with outstanding borrower
retirement. No mutant is waived or marked equivalent.

Initial root mutation retained 13 survivors, then one. Instance mutation
retained 126, then 11; direct boundary/diagnostic cases were added. Redundant
root-index checks were removed because stream admission already owns them.
Retirement queues the despawn exactly on entry, avoiding a repeated no-op
despawn call. Peak arithmetic is exact within the bounded capacities and
keeps no redundant equal-value branch.

The independent initial native trace exposed the shutdown quota leak at paid
tick 50: two actor slots remained after global root revocation. The direct
shutdown check and final model now require zero. The compiler rejects direct
nonfinite bit construction in checks, so nonfinite placements are generated
by floating arithmetic. The just-outside f32 position case is 1,000,000,128;
1,000,000,001 rounds onto the admitted endpoint and was corrected as a test
error. Fixed-case compile budgets retain their compiler near-limit notes;
commands stay below the repository's duration cap.

## Native and browser replay

`instance-trial.coax` drives the actual pool, stream and entity store on 64
paid steps at each of 30/60/144 FPS and uneven 5/27/14/31 ms frames. It loads
two real four-byte index sources under one dependency edge, then stages,
activates, selects, stops, replaces and transfers actors; finally it retires
the world and completes retained resource destruction. Every scenario and
input-generation loop stays in Coax. The C face projects the ABI only;
browser JavaScript reads observation words only.

The [independent Python model](../../../aethyr/actors/tests/instance_oracle.py)
models actor admission/lifecycle, quotas, reusable entity storage, full entity
digest, owner transfer and a single HOLD program using absolute completion
and availability deadlines. It imports no Coax implementation or generated
expectations. Each of 104 words per step is compared, with the complete row
set required. Observations include lifecycle events, instance keys, handles,
entity IDs, control/cursor counters, quota peaks and resource ownership/costs.
This is a scripted lifecycle/intent trial, not a physics or perception trial.

Each cadence admits eight actor requests and counts 14 refusals, reaches three
reserved slots and at most two lifecycle events per step, completes eight
births/activations/removals, performs one owner transfer and refuses 19
wrong-generation selection probes. The latter are local command-context
checks, not nav/model callback qualification. Declared work peaks are 24
sense, 384 search and 24 action units. These reserve future service work;
they are not measurements of executed sense/search work.

The [native observations](instances-native.txt) contain 256 rows, 68,600 bytes,
SHA-256 `0bfb7f6f38a6bc2b2446a293f297327068693282b75f40df1c5aa2c4d9cb29a7`.
The same normal C emitted for Wasm is compiled with GCC 15.2.0,
`-O1 -g -fsanitize=address,undefined -fno-sanitize-recover=all
-fno-omit-frame-pointer -fno-fast-math -ffp-contract=off` and called through
the same thin ABI face. ASan/UBSan with leak detection reports zero diagnostics.
Native and actual muted headless-Chrome Wasm match every model word at all
four cadences; [oracle output](instances-oracle.txt) retains the per-target
hashes. Native stack remains the default 8 MiB limit.

The new source-part versions of the old 1,024-row behavior trial and 64-case
definition trial are also emitted/executed natively. Both outputs are
byte-identical to their previously independently verified retained observations
at `8f76175c8d` and `331caba6f2`. Canonical schema/RCM1/program identities and
the weighted-selection trace survive the module-composition change.

Final Wasm: 100,249 bytes, SHA-256
`e99e755ce132b5a33fa1a0561200cac72dea2433c7d8d29d2d6ee41d13fbaced`.
Build: 11.6 s reported, including 9.5 s Coax emission, 1.1 s clang, 0.5 s
link and 0.3 s wasm-opt, plus setup. Ordinary Wasm settings remain a 1 MiB
stack and 64 MiB imported memory; successful execution does not measure exact
stack peaks. The CDP tool's separate arena setting is not the actor's stack.

Generated C SHA-256:
`323c2b3caf12573309f63a980a4698c7dd2ee6b06ff0d37d7d8c15a143db32b3`;
header:
`1e0c10dda4e8c081865e12a8bb20c82d00ac5535830050e15318b7d5c0cad888`.
Compiler: Coax 0.52.0, SHA-256
`bd13739490a65b8969c6fb4298ca8f94264416e41ad1a146ca8170fdec8acacc`.
Artifacts are retained under
`~/.openagents/aethyrforge/actors-12992/instances-v2/`.

## Allocation scope and remaining work

[Compiler LP64 layout metadata](instances-sizes.txt): `ActorPool` 17,032 bytes
(all 16 fixed slots, even when capacity is three), `ActorSlot` 1,056,
`ActorResources` 312 and `BehaviorState` 264. The trial additionally retains
64×104×8 = 53,248 bytes of observation words plus its count. No runtime heap
allocation is performed by these actor procedures. These sizes are not an
observed peak host allocation or stack trace.

The trial's two index owners retain 352 named budget bytes: 344 CPU ticket
bytes and eight GPU ledger bytes. These remain through world revocation and
actor removal until explicit destruction. This exercises the shared
accounting/ownership boundary; it does not create actual GPU buffers or
measure driver allocations. Root tests also keep a borrower alive across
claim release and refuse premature destruction.

The [module README](../../../aethyr/actors/README.md) retains the complete
remaining issue scope: actual service-hook dispatch and physical bodies,
per-instance overrides/persistence, real perception/query bands and memory,
current-owner nav/model callback adapters, failure/retry modes, human/AI rider
input and attachment/dismount, two editable playable archetypes and a mount
fixture, adopted native/Wasm replay/captures, physical allocation/work peaks,
and the exact residual-adapter inventory. No private realm scheduler is
migrated in this checkpoint. Chapter 75 remains partly implemented. The
inherited book capability-pin failure recorded in [behavior-book.txt](behavior-book.txt)
is unchanged; no book pass or chapter promotion is claimed.
