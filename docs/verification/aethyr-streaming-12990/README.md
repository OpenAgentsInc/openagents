# Resource-cost and streaming work — #12990

Issue #12990 remains open. This record currently qualifies component
contracts and a small native/browser codec fixture. It does not establish
the required walkable streamed host, allocation reconciliation, route
performance, tier comparison or GPU recovery.

Work starts from `df06d21dee90a9f54778143ffd6b8fe1c400e392`, in the reused
`/home/christopherdavid/oa-open-issues` worktree. Exact component source
and compiler identities are in `component-source-sha256.txt`. Hardware is
coderos-4080, Linux x86_64, Coax 0.52.0; the actual browser identifies itself
as muted headless Chrome 152 in `resource-cost-hosts.log`.

## Measured components

| Gate | Result |
| --- | --- |
| `resource-cost.coax --root-only` mutation | 278 killed, zero survivors |
| `resource-manifest.coax --root-only` mutation | 373 killed, zero survivors |
| `resource-admission.coax --root-only` mutation | 230 killed, zero survivors |
| Native/browser Wasm codec/admission fixture | All 32 transcript words equal |
| Release fixture | Same 32 words, zero diagnostic output |
| ASan/UBSan with leak detection | Same 32 words, zero sanitizer output |
| Book chapter registries | All registries pass; status counts unchanged |
| Tools language/reason gate | 1,848 tracked tools; zero new without a reason |
| `resource-stream.coax --root-only` mutation | 440 killed, zero survivors |
| Native/browser owner fixture | All 64 words equal across two actual codec/owner journeys |
| Owner release and ASan/UBSan fixtures | Same 64 words; zero sanitizer output |
| Changed `cells.coax --root-only` mutation | 601 killed, zero survivors |
| `stream-profile.coax --root-only` mutation | 115 killed, zero survivors |
| Cell module compatibility | Compatible with the preceding committed module; new profile surface is additive |

All mutation gates execute their complete loaded baseline checks. Earlier
survivors were addressed with exact capacity, lane, cost-forgery, closure,
concurrent scratch, atomic rollback and lifetime checks. No equivalent
annotations or exemptions were added. Passing unchanged gates are reused.

The 64,996-byte executed Wasm SHA-256 is
`6a77d8301c94afec7f2c01c815330ac73b8e35af8a807340cf5c91b2ceedaefe`.
Generated C, the native executable, the instrumented executable and a Wasm
copy with that same hash live under
`~/.openagents/aethyrforge/streaming-12990/2026-11-06/`.

Commands, each below 15 minutes:

```sh
coax/bin/coax mutate aethyr/world/resource-cost.coax --library --root-only --jobs 8
coax/bin/coax mutate aethyr/world/resource-manifest.coax --library --root-only --jobs 8
coax/bin/coax mutate aethyr/world/resource-admission.coax --library --root-only --jobs 8
coax/web/tool.sh aethyr/tools/resource-cost-hosts.coax
coax/bin/coax build aethyr/examples/resource-cost-parity.coax --opt 2 --keep-c -o /tmp/oa-12990-resource-admission
coax/bin/coax run aethyr/examples/resource-cost-parity.coax --release
cc -std=c11 -O1 -g -fsanitize=address,undefined -fno-omit-frame-pointer -ffp-contract=off -fno-fast-math /tmp/oa-12990-resource-admission.c -lm -o /tmp/oa-12990-resource-admission-san
ASAN_OPTIONS=detect_leaks=1:abort_on_error=1 UBSAN_OPTIONS=halt_on_error=1 /tmp/oa-12990-resource-admission-san
python3 coax/book/check-structure.py --registry
```

The actual transcript includes a 69-byte PNG decoded through the canonical
full mip decoder, a PCM16 WAV decoded into F32, every relevant 64-bit body
lane, canonical manifest SHA-256 words, a cut manifest, a late compressed
image decode failure and a forged GPU cost. It does not create GPU objects
or claim measured physical GPU bytes. The native upload scratch field is
the declared explicit-transfer model to be reconciled with the consumer.
The final eight words exercise the real admission policy and existing
ticket/ledger records in native and Wasm: first grant, category refusal,
261-byte peak, 245-byte settled retention, final cleanup, 72-byte concurrent
scratch, stale settlement refusal and final second cleanup. These records
are policy observations; they do not substitute for physical host allocation
traces. Historical 24-word codec-only observations remain in the `-24` logs.
The final cleanup word also requires the actual native/Wasm backends to
refuse raw IEEE NaN and both infinity signs in each geometry/body coordinate
lane. These bytes run in compiled code because the evaluator deliberately
rejects nonfinite compile-time bit conversions. The sanitizer run covers
that same hostile-input path.

## Format and accounting

RCM1 is a new companion, preserving VPK1/VPK2 and existing host/render ABIs.
Its 48-byte header contains magic, schema 1, count, 440-byte row size and
the exact hash of `RESOURCE_MODEL_REVISION`. Each row encodes the source
cost-model version/format/category/backend/side, source SHA-256, thirteen
64-bit cost fields and at most eight earlier-owner references with their
full dependency source digests. Unused fields are zero. The whole canonical
byte sequence has its own expected SHA-256. The builder calls the actual
loader plans and format validation; the reader never trusts a root supplied
by the bytes themselves. New layouts or loader costs require a model
revision. Per-source hashes in the record pin the implementation revision.
Digest fields encode the eight numeric `AssetId` words as little-endian
u32 values, rather than copying conventional hex text or its byte spelling.

The seven named lines are geometry, textures, collision/navigation, baked
lighting, audio, actor state and staging. A plan's persistent bytes are
encoded + decoded + its retained plan object + logical GPU storage. Scratch
is decoder work + explicitly retained upload storage. Simultaneous native
2-by-2 PNG loads reserve 36 bytes of scratch each, and a 72-byte staging cap
admits exactly two. A third refuses. No cancellation API releases scratch.
The caller must wait for actual completion and physical cleanup.

The real metadata layout is 168 bytes in both qualified compilers. Hosts
using this profile must allocate that separate object; manifest/controller
storage and platform arenas are additional retained owners. The fixed
inbox and renderer metadata remain separate existing host allocations.
SDK/driver private storage is not estimated as zero or included in logical
texel costs. Existing tickets and GPU generations remain the allocation
authorities. Named-budget refusal occurs before speculative ticket/ledger
copies are published, including a failure after three reservations.

## Work still required before closing #12990

- Real asynchronous host adoption of the checked shared/dependency owners,
  including failure/cancel/stale/superseded completion receipts.
- Actual adoption of the declared interest/update/draw/audio/prefetch
  profiles, independent texture/lighting lifetimes and persistent services.
- Actual shared-loader adoption in a walkable world larger than residency,
  semantic collision, navigation and stable IDs at its edges.
- Allocation traces for every supported format, concurrent scratch and
  shared dependencies, with platform/SDK memory separated.
- Actual host retention must also remain under the existing pack storage,
  decoded-image and audio-memory owners. The standalone inbox/renderer
  component receipt does not qualify those integrations or an extra pool
  outside their aggregate limits.
- Different visual tiers, actual native/browser load/unload/GPU recovery,
  accepted-event replay, route churn/caps, stage timing and frame tails.
- A final residual-adapter inventory and teaching/registry updates covering
  the completed consumer rather than these component contracts alone.

## Concrete dependency owners

`resource-stream.coax` now implements separate retained-reference and
wanted-closure records. Two roots sharing a texture hold one concrete
texture owner. A second claim on an existing parent adds no second edge to
that parent's dependencies. Dropping the final root claim prevents new
borrows immediately; existing borrows and pending effects keep physical
owners and their edges until actual destruction. Failed loads retire their
physical attempt before a new generation may be admitted. Closing retains
the old backing record and owner epoch for cleanup rather than resetting it
under late callbacks.

Readiness publication checks the retained manifest binding, a new fixed
tick, current generation/epoch/content identity, wanted state and complete
dependency readiness. Publication stages its whole output and refuses a
short event span atomically. There are 64 root claims, 64 immutable borrows
and at most four unfinished loads. Separate typed claim/borrow references
and independent borrow serials keep render frequency out of load generation
identity. Every new identifier refuses at exhaustion; cleanup stays usable.

`owner-source-sha256.txt` pins this component qualification. The example
`aethyr/examples/resource-stream-parity.coax` executes the same canonical
PNG decoder and controller on native and browser Wasm. Both journeys retain
three concrete resources, an 847-byte declared peak, twelve initial CPU
tickets and eleven GPU-ledger words. One schedule publishes all three
resources at tick 1; the other publishes the dependency at tick 0 and its
parents at tick 1. Their accepted-history SHA-256 values differ as expected.
An extra renderer borrow changes its private borrow serial but both
journeys' next logical load generation remains 8. Cancelled loading keeps
sixteen scratch bytes through completion, then waits for the destruction
receipt. Both end with four actual controller retirements, zero CPU tickets,
zero ledger words and no active owners.

The executed 70,649-byte Wasm SHA-256 is
`27289aed7f6c6bba86246b900ccac586dcba82ffe98615fb4de88595fffeafbc`.
Its artifact, generated C and native/instrumented binaries are retained in
the same shared directory as the earlier cost fixture. Commands:

```sh
coax/bin/coax mutate aethyr/world/resource-stream.coax --library --root-only --jobs 8
coax/web/tool.sh aethyr/tools/resource-stream-hosts.coax
coax/bin/coax run aethyr/examples/resource-stream-parity.coax --release
```

These are controller/codec receipts. They do not allocate real GPU objects
or execute actual asynchronous SDK work. The actual host coordinator must
drain completion facts only on paid steps and retain every applied failure,
success and destruction outcome in its replay history. The full consumer
and recorded-history replay requirement remains open; running these two
live schedules is not a claim that different live timings produce identical
simulation or a substitute for replaying a retained host record.

## Named cell and interest profiles

The additive `CellRings`/`cell_step_profile` API uses the historical cell
controller rather than introducing a second simulator. The default wrapper
keeps its 96/160/256 m rings, 8 m hysteresis, 48 resident and four in-flight
limits. `CellWorld` and `CellStep` layouts are unchanged. Checked profile
admission rejects invalid rings, malformed manifests, out-of-range positions
and a cap reduction below already retained/in-flight work without mutating
the caller. The shared request loop now admits its last signed-64 generation
and refuses future asks without wrapping. The new unit gates cover exact
caps, refused transitions, legacy-default digest equality and the original
39 cell/replay/store/edge checks. `profile-source-sha256.txt` pins the changed
source. These profile changes follow the earlier owner fixture; its recorded
source hash intentionally identifies the preceding cell source version.
`cell-profile-compat.log` retains the compiler's direct old/new module
compatibility result. The temporary old source was read from the preceding
commit for this comparison and then removed.

Profile 1 declares 32/64/96 m draw/simulate/resident rings, 8 m hysteresis,
four resident cells and one new cell request per paid step. Texture,
baked-lighting and audio collection have separate 96/80/64 m radii. Update
cadence is one or four paid ticks according to the admitted cell level and
does not depend on visual tier. Both tiers preserve that policy and differ
only in declared image layout. Placement hints trigger within 8 m of a
neighbor edge and declare a 4 KiB/one-root quota. Direction, exact bounds,
missing/loaded neighbors and deterministic axis ties are checked; hints leave
cell state and entry permission unchanged. Pool declarations are four
streamed actors, one persistent actor, two voices and zero animation/sensor/
effect/dynamic-light slots, with three persistent player/camera/HUD services.
They are declarations pending actual host allocation/route measurements.

The next consumer must derive its real pack geometry cost from the shared
loader's actual layout. `verse/tools/pack_realm.py` writes packed 48-byte
vertices: the first 32 bytes contain position/normal/UV floats, followed by
packed color/layer/spare bytes. The existing `positions4` fixture model is
not a general VPK vertex model. A new supported cost-model revision must
preserve the existing companion/model identities rather than treating all
packed fields as floats or guessing the retained size. PNG asset generation
can reuse the existing Coax encoder in `coax/lib/image/pillow.coax`.

The historical #12847 cell/edge/save work is retained and was inspected.
Its closure did not qualify an activated large-world realm. The legacy
gameplay failures reproduced on remote main remain in the #12989 record.
Chapter 53's format/read/write/admit listings now match the existing VPK2
source without a runtime change. The full book structure check reaches a
different pre-existing drift in chapter 87's `shared-step` listing; both
that chapter and its source are unchanged from the starting commit. This
remaining failure is retained in `book-structure.log`. Every chapter registry
passes in `book-registries.log`; neither chapter 38 nor another draft was
promoted by the component work.
