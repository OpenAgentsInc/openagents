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

- Concrete shared/dependency owners, last borrowers and real asynchronous
  failure/cancel/stale/superseded completion receipts.
- Named interest/update/draw/audio/prefetch profiles bound to the existing
  cells, independent texture/lighting lifetimes and persistent services.
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

The next owner boundary must distinguish wanted state from retained
references. A cancelled parent can retain a dependency through a pending
effect or final borrower without wanting that dependency published. Count
each concrete parent's dependency edge once, retain it until actual parent
destruction, and compute wanted closure from live claims. Old-epoch effects
still need cleanup through their original records; replacement cannot reset
or overwrite those records early. Fixed-tick publication additionally needs
the complete dependency closure ready. These requirements are not yet
implemented by the admission component alone.

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
