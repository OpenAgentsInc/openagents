# Resource-cost and streaming work — #12990

Issue #12990 remains open. The [actual callback record](walk-callback-retirement.md)
qualifies unwanted held image/buffer responses, retention through real effect
join, retirement before newer generations, old-key/wrong-epoch refusals and
unchanged current storage/pixels. Both browser tiers replay every accepted
paid step through CPU codecs. Complete physical/transport/transient planning
and the final adoption/teaching audit remain open.
The [integrated native SDK record](walk-native-sdk-failures.md)
adds 136 actual scoped staging/destination/submission/poll error histories,
including upload deadline stops, full private retirement and exact CPU replay
on two tiers and both hardware/software backends. Inputs are explicitly
injected over real effects; physical OOM/reset is not qualified.
The [owned native upload record](walk-native-transfers.md)
adds checked buffer/image transfers, retained pending views, early-release
guards and actual held-observation probes on both tiers/backends. Exact
route histories and CPU replay are retained. The [checked native factory record](walk-native-factories.md)
adds atomic buffer/image creation and partial-effect cleanup, with poisoned
SDK outputs, actual hardware/software factory prefixes and successful
two-tier consumer adoption. The integrated SDK record extends these boundaries
through actual failed-load cleanup and stopped-frame/drain observations.
The [native load-failure record](walk-native-load-failures.md)
now qualifies partial allocation, source-read and decoder-refusal cleanup,
retirement/retry and exact CPU replay on both native tiers. The callback
record extends the separate browser path through supersession and redelivery.
The [actual category-pressure record](walk-category-pressure.md)
now qualifies texture-line refusal against the native host's live budget,
with staging/aggregate room, unchanged pixels/owners, continued movement,
device replacement and exact refusal-boundary CPU replay. The [native draw-result record](walk-native-loss.md)
adds owned pending submissions, reported-loss recovery, rate/deadline/failure
stops and zero-step interruption replay. It qualifies actual Vulkan effects
with explicit SDK-result injection, not a physical GPU reset or the complete
native factory/upload/rescue surface. The [automatic browser-loss record](walk-automatic-loss.md)
adds unsolicited callbacks, repeated/rebuilding loss, a real ten-second
deadline, late-device destruction and checked frame admission. The additive
[persistent browser rescue](walk-persistent-rescue.md) now covers exact-build
whole-reactor save/reload, unfinished source effects, single use and crash
loops. Native persistent rescue and broader memory/failure profiles remain
unqualified. The
[native-history CPU replay](walk-native-cpu-replay.md)
now checks every paid step in the actual Vulkan histories against the CPU
consumer, including twelve altered-history refusals per tier. The original
native manifest/backend is retained; GPU effects are not replayed. The
[persistent HUD record](walk-persistent-hud.md)
adds actual native/browser rendering before any area resources, through
unload/reload and physical GPU replacement. Both native tiers preserve
every pre-HUD canonical paid-step digest. All-pixel HUD references stay
within one channel level under the fixed bound of two. This does not
independently qualify world shading or persistent rescue.
The earlier [native Vulkan
record](walk-native-gpu.md) adds actual two-tier native loads, draws,
retirement and controlled device replacement, with full readback equality
and sanitizer observations. Physical Vulkan cost planning remains separate
from logical RCM1 budgets. The [receive-fault replay
record](walk-receive-fault-replay.md) qualifies actual corrupt, oversized,
cancelled, reordered and bad-container histories with retained source
observations and exact native state at every paid step. The [canonical
paid-step replay record](walk-state-replay.md) adds a versioned state profile, independent
byte vectors and actual native/browser comparison at every paid tick for
both successful route histories. A separate [actual upload-failure
record](walk-upload-failure-replay.md) now qualifies every paid step through
GPU refusal, physical retirement and retry for that one failure history.
The preceding [coordinator and controlled
GPU replacement record](walk-coordinator-recovery.md) retains its mutation,
real WebGPU replacement and exact-owner upload-failure evidence. The
[earlier browser/replay record](walk-browser-cpu-replay.md) preserves its
historical failing gates and artifacts. Physical backend and transport/transient
budget planning and the final adoption/teaching audit still prevent issue
completion.

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

- Complete transport/transient and physical backend allocation planning.
  Current traces reconcile logical resource costs; the native Vulkan
  backing extents and the browser's engine-owned transport arenas require
  explicit treatment alongside the existing aggregate owners.
- Replay the additional load-failure histories as they are recorded. Current
  successful and draw-fault native GPU histories already replay every paid
  step through CPU allocation/codecs in their original backend/cost profile.
- Final consumer/residual-adapter inventory, supported-platform limits and
  complete teaching/registry audit. Existing source and qualified profile
  evidence must be retained when reconciling chapter status.

The native draw-result record qualifies labelled SDK observations over real
Vulkan submissions, including repeated/rebuilding loss and deadline stops.
The two-MiB browser rescue carrier has actual trap, unfinished-load and
failed-adapter reloads. Neither result qualifies native persistent rescue,
physical GPU resets, arbitrary browser rescue extents or an independent
world-shader reference. Those remain explicit profile limitations; they are
not additional prerequisites invented for this resource-streaming issue.

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

## Packed vertices and prepared corridor assets

The following stage starts at local commit `0597a6abd0`, following the
profile qualification above. `packed-source-sha256.txt` pins the exact
changed component, example, tool and independent-oracle sources. These are
local #12990 changes, not a completion or a production deployment.

`forge/vertices.coax` admits the published 48-byte VPK static record. It
preflights all eight floating-point lanes by IEEE exponent before conversion,
then applies the existing spatial finite/bounds predicate. Position, normal
and UV failures report the row/lane. Packed color, layer and spare words
remain opaque bytes, including all-one bit patterns. The appended resource
format 8 uses model version 2. All old ordinals, the 168-byte plan layout
and the old `RESOURCE_MODEL_REVISION` remain unchanged. RCM1 selects the
separate exact `RESOURCE_MODEL_PACKED_REVISION` only when a packed resource
is present; a mismatched old/new header refuses even with a correct root.

| Changed boundary | Root mutation result |
| --- | --- |
| Static packed-vertex admission | 27 killed, zero survivors |
| Decoder cost planning | 291 killed, zero survivors |
| Canonical manifest/model admission | 374 killed, zero survivors |
| Original ticket/ledger admission | 232 killed, zero survivors |

The cost run first exposed a redundant assignment of alignment 4, already
set by the common plan initialization. It was removed and that changed gate
was rerun. No equivalent exemption was added. The earlier passing manifest
and admission root results are retained; their source roots did not change.

The codec fixture now has 40 words. Its first 32 exactly preserve the
preceding fixture, including every v1 manifest SHA-256 word. The appended
eight cover 96 encoded/GPU/native-upload bytes, two vertices, version 2,
the new manifest/model digest words and exact cleanup. Every attribute lane
refuses NaN and both infinity signs. Native, release, ASan/UBSan/LSan and
actual browser Wasm agree on all 40 fixture words. Release and sanitizer
stderr are empty. The executed 67,580-byte Wasm SHA-256 is
`e19a586685e7506a59873f581ae74abb335099c3207301215ad3ca82685057cb`.
Generated C, release/sanitized executables and this same Wasm are retained
as `packed-costs*` in the shared artifact directory named above.

`tools/streaming-assets.coax` creates original CC0 fixture data in a **new**
output directory. Its 12 adjacent 64 m cells cover 768 m. Floor vertices,
ground collision and cell coordinates share exact edges. It produces:

- 13 whole VPK1 packs admitted through the shared writer/reader: one common
  texture/cube/index pack and twelve floor/lighting/PCM packs.
- 24 raw collision/body sources with stable body IDs 1–12.
- 64 resource rows, including shared dependency references, exact source
  hashes and admitted container spans in `corridor-scene.json`.
- Four manifests with the same source identities: browser/Vulkan transfer
  models at image sides 1 and 2. Logical whole-world retention is 62,106
  bytes at side 1 and 62,522 at side 2. Summed scratch is 366 bytes for the
  browser model and 5,770/5,978 for the native model. These scratch sums are
  **not** a claim that every decode/upload runs concurrently.
- A real 8-by-8 PNG probe whose declared texture retention exceeds a
  512-byte category cap. Actual category-refusal host exercise is pending.

The retained prepared directory is
`~/.openagents/aethyrforge/streaming-12990/2026-11-06/corridor-v1-aligned/`.
A second generation in `corridor-v1-reproduced/` compares byte-for-byte
equal (`diff -r`, exit 0), including the complete index and all manifests.
Both scene roots are
`61163431b0125715413087af2897dca8bd39b5361911c904c1f534612d8ec133`.
An attempt to reuse the first directory exits 2 before writing. The earlier
`corridor-v1/` directory is a diagnostic precursor with an incorrect X span;
it is not this scene's qualified source. It remains retained rather than
being overwritten or substituted.

The independent Python oracle uses only standard-library SHA-256, struct,
wave, CRC and zlib operations. It verifies source/container identity, VPK
layout/catalog/ranges, exact floor/body placement, every PCM sample and
decoded PNG pixel, all 13 cost lanes and dependency/model digests in all
four manifests. It also rejects three deliberately broken fixtures after
their outer identities are repaired: a forged GPU cost, changed body
placement and changed audio sample. It does not import or call the Coax
implementation. Its successful record is `corridor-oracle.log`.

Commands:

```sh
coax/web/tool.sh aethyr/tools/streaming-assets.coax NEW_OUTPUT_DIR
python3 aethyr/tests/streaming_assets_oracle.py OUTPUT_DIR --fault-checks
coax/web/tool.sh aethyr/tools/resource-cost-hosts.coax
```

This completes preparation of real source assets for the next host stage.
It does **not** adopt the profiles, measure actual allocations or establish
the 32 KiB resident cap: 32 KiB is the intended consumer budget against
which the 62 KiB world must stream. Actual walking, asynchronous failures,
same-history replay, native/browser GPU recovery, route churn and timing
tails remain required. No issue closure is justified by these receipts.

All six book registries pass with unchanged status counts
(`packed-book-registries.log`). The tracked tools gate passes: 1,850 files,
zero new files without a firm reason (`packed-tools-check.log`). Chapter 16
describes the appended exact fixture; no chapter is promoted. The previously
recorded unrelated chapter-87 listing drift is unchanged and was not rerun.

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
