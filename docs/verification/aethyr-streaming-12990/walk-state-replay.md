# Canonical paid-step streaming replay — November 7, 2026

**#12990 remains open.** The actual corridor now records a versioned
canonical SHA-256 after every paid step, including the steps between a
frame's start and endpoint. Native CPU/codec replay compares each digest
before consuming that frame's effect commands. This qualifies the two
recorded successful route histories, not every failed/cancelled history,
automatic device-loss handling, rescue or native GPU rendering/recovery.

The source starts from local checkpoint `97e8224a97`, rebased over remote
`4522812690`. Source and artifact identities accompany this record. The
original coordinator, codec, owner, profile and explicit replacement
receipts remain in [the preceding record](walk-coordinator-recovery.md).
Unchanged passing component gates are reused.

| Current gate | Result |
| --- | --- |
| Coordinator loaded baseline | 645 checks pass |
| Coordinator root mutation | 958 killed, zero survivors |
| Canonical serializer unit mutation | 185 killed, zero survivors |
| Current two-tier actual WebGPU routes | Both pass |
| Current two-tier instrumented native CPU replay | Both pass, including leak detection |
| Tools language/reason check | 1,855 tracked tools, zero new without a reason |
| Book registries | Pass; no chapter status changes |

The full book listing check still reports the previously recorded Chapter
87 `shared-step` listing drift. That chapter/source pair is unchanged by
this work; no full-book pass is claimed. The two-survivor interim
coordinator result is retained as well: it led to an exact pending-clock
snapshot check for each observation's paid ordinal.

## Declared state profile

`aethyr.stream-walk-state.v1` serializes explicit LE64 words, including
IEEE float bits and Boolean 0/1. It never hashes struct padding, addresses
or the SDK allocator layout. Its order is:

1. Magic/version, lifecycle, tick, paid ordinal, query epoch and counters.
2. The pending accepted frame schedule and complete player/camera/input
   state, including interpolation, collision diagnostics and every press
   counter. The player's committed clock still describes the prior frame;
   the separately serialized schedule/ordinal bind the pending work.
3. The named interest/work/pool profile and physical surfaces.
4. Logical cell manifests, states, sleeping bodies and entity-store fields;
   active actor records and retained decoded collision/body values.
5. Manifest identity, stream lifecycle, load generations, claims, owner
   admission leases, budget totals/category peaks and peak owners.
6. Inbox entries, prefetch reference and current load/retirement/event
   outboxes, including every content/source SHA word.

Facts have all been applied at the observation boundary. Pending SDK
callbacks do not mutate this state before their recorded paid step. The
immutable manifest identity binds admitted source formats and cost models.
Dead array capacity is excluded where its logical length is explicit.

The renderer registry, GPU handles, display-borrow serials, overwritten
query/navigation scratch and observer storage are outside this simulation
profile. Actual GPU/CPU/transport ownership retains its separate traces.
Digest comparison does not replace physical allocation, source/decoder,
SDK error or GPU validation checks. Different live completion schedules
can still produce different states.

The observer is optional for other engine callers. The actual recording
face enables it. It retains eight SHA-256 slots in fixed controller storage
and uses a bounded 128 KiB local serialization buffer inside the existing
8 MiB reserved reactor arena. Neither is charged to the 32 KiB dynamic
resource cap. The reported known fixed C metadata is now 773,933 bytes;
the actual Wasm extent remains 2,162,688 bytes under its 8 MiB ceiling.

## Checks and independent vectors

The serializer's full loaded gate passes; its 185 root-unit mutants are
all killed, with zero survivors. The earlier run killed 91 and left 94;
that failure is preserved. Most exposed a weak comparison that could
share the same encoding mistake on both sides.

The independent Python `state_wire_vector` defines a 27,928-byte wire
fixture with zero scalar values, one cell/sleeping body, one retained
collision/body pair and one item in each outbox. Every Boolean is set to
zero or one in separate vectors. These are raw serialization fixtures,
not claims of world admission. Exact SHA-256 values are:

- False: `f817b3610a618dc57e8eb94235deb5eb38117924b4b2ebf42049a7d02a3569ad`.
- True: `b6924eb23d1c0967da2516f174709cd244fe0a821d911c5ff7dd3768cb820645`.

Coax checks those independently specified bytes, high/signed bits, negative
zero, every body/key/lease/clock lane, changes in each retained state
domain, observer isolation, multi-step boundaries and zero-step frames.
Record-field-count checks require schema review when retained shapes grow.
No mutant is marked equivalent. These checks do not prove collision
resistance beyond the unchanged SHA-256 implementation's existing scope.

## Actual two-tier route and native replay

Both muted headless Chrome 152 routes on coderos-4080's RTX 4080 use the
same original CC0 corridor-v2 sources and the existing low/high RCM1
manifests. Each runs 3,206 frames, 12,806 walking ticks and three cleanup
ticks. They reach z=767.9873046875 and return to z=18.3974609375. Gameplay
frame tuples and all twelve final actor identities/counters match across
tiers; their resource layouts and complete state hashes intentionally
remain tier-specific.

Each trace has 114 attempts and final physical retirements, 680 exact
allocation/free events, no upload/codec errors and final zero dynamic
CPU/audio/image/inbox/GPU owners. Actual requested CPU peaks are
14,132/14,212 bytes; declared resident peaks are 18,171/18,299 bytes.
Frame-work p50/p95/p99 with the observer enabled are low 2.9/5.6/8.0 ms and
high 2.9/5.7/7.7 ms. These accelerated-input headless routes run concurrently;
they do not qualify uncontended RAF or phones.
Driver/SDK backing, allocator overhead and process RSS remain outside the
requested-payload and logical-texel numbers.

One unchanged instrumented native executable replays both histories using
the same Coax core, real source authentication/decoders and malloc/free.
Positive routes pass ASan/UBSan and leak detection. All 12,809 paid-step
SHA-256 values match for each history, alongside the existing exact
completion records, frame positions and final cell records. Native CPU
peaks are 14,819/14,883 bytes; its eager preparation remains within the
original outstanding grants. The native observer makes no GPU objects.

Independent transcript reserialization gives observation SHA-256:

- Low: `02883242549fe8b35399e1e48f436416fb04df7c34a66b7024ba501cb25f016f`.
- High: `886aa462318e5d1cc8f796dbe298bcf85adf8cef9b68a02d6a58f7068a5952e0`.

Six altered histories per tier are rejected: changed frame position,
generation or missing completion, plus an edited intermediate digest,
omitted step and reordered steps. The intermediate edit changes no input,
completion, frame endpoint or final actor record; native replay identifies
the exact affected paid tick. Negative observers deliberately stop with
live test allocations, so only those child processes disable leak checking;
they retain memory-error detection, require exit 1/no output report and
reject unexpected diagnostics.

## Artifacts and remaining exits

Shared artifacts live under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`:
`stream-walk-state-route{,-high}.json`, their allocation/stage/PNG companions,
`stream-walk-state.wasm`, and `native-replay-state-san/` (source, header,
instrumented executable and two reports). No fixture source is committed.
The replay report is version 2 and requires the explicit state-profile
identity; old version-1 records remain historical, never silently upgraded.

The final executed browser artifact is `stream-walk-state-final.wasm`,
129,784 bytes, SHA-256
`b44e82ed556bfa47608c952764dc6068335dd09a58932c06c5c9949b0fdecbed`.
Its receipts are `stream-walk-state-qualified{,-high}.json` and the matching
`replay-qualified{,-high}.json` native reports. The earlier `state-route`
receipts used the same runtime before independent vector/schema checks
were added; those artifacts are preserved. The unchanged native executable
is reused for the final browser histories. Exact source/artifact pins and
compact oracle/mutation records accompany this document.

Remaining work includes actual failed/cancelled/stale/superseded history
replay, recording the actual retained payload when it differs from the
immutable source, automatic/repeated GPU loss and failure rescue, native
GPU recovery, persistent HUD adoption, the remaining host fault matrix,
shader qualification and complete transport/transient owner accounting.
No issue or chapter is promoted by this stage.
