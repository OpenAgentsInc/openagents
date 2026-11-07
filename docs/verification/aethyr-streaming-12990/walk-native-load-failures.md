# Native allocation, source and decode failures — November 7, 2026

Starting from `6cf7b2b5af`, failed native load effects now complete through
the existing shared owner, retire their partial physical state and retry
only after destruction is accepted. **#12990 remains open** for checked
Vulkan factory/upload failures, actual stale/superseded callbacks, complete
physical/transport/transient planning and the final adoption/teaching audit.

## Changed behavior

The earlier `swn_effects` returned immediately after a failed allocation,
source or decode call. The concrete attempt and reservations remained owned,
but no failed completion could drive their normal retirement. Injecting a
failure at allocation field 2 reproduced `native warmup failed`, exit 1 and
no complete receipt in the retained red trial.

The caller now preserves the first failed phase, skips later effects and
offers a typed completion for that same row/generation/epoch. Partial
allocations keep their source/decoded/plan pointers and image/audio cache
reservations until physical retirement. Allocation stops at its first failed
field. Scratch is freed after the synchronous effect finishes; its fixed
transfer grant releases afterward. The shared controller still decides
when completion applies, which attempts retire and when retry may begin.

`native-load-policy.coax` supplies checked outcome precedence and exact
qualifier-mode/effect admission. The additive native `sw_host_finish_code`
preserves existing `ResourceCostStatus` values. The original boolean
`sw_host_finish` facade is unchanged for older callers and the browser.
Neither ABI changes existing manifests, content IDs or accepted-history
state serialization.

## Actual failure matrix

Each case targets the first texture attempt at row 0, before warmup, and
consumes its input once. This is explicit effect-input qualification; no
physical OS exhaustion is induced.

| Mode | Actual observed failure | Successful allocation prefix |
| --- | --- | --- |
| `--load-alloc-0` | Allocator returns NULL with ENOMEM at encoded source | none |
| `--load-alloc-1` | Same at decoded pixels | source |
| `--load-alloc-2` | Same at retained plan | source, decoded |
| `--load-alloc-3` | Same at decoder work | source, decoded, plan |
| `--load-alloc-4` | Same at native transfer span | source, decoded, plan, work |
| `--load-source` | Real `fs_read` of the absent fixture path returns -1 | all five fields |
| `--load-decode` | Actual canonical decoder receives one byte less scratch and returns capacity | all five fields |

Allocation inputs replace the selected effect with a NULL/ENOMEM result;
they do not allocate until the machine is exhausted. Decoder refusal comes
from the real canonical decoder and retains the original authenticated
source. The source case skips publication after the failed real file read.
Allocation/decode completion uses capacity (3), and source failure invalid
(2). The record labels the effect input separately from its observed result.

Both original visual tiers run on the RTX 4080 with Khronos validation and
again under ASan/UBSan/LSan on two-worker Lavapipe: **28 GPU histories**.
Every history contains exactly one failed completion, its destruction and
a later successful generation. The failed key never publishes readiness or
creates an uploaded render resource. All physical partial fields and cache/ledger
owners drain exactly once. The subsequent successful attempt renders,
continues eight paid movement steps, survives controlled device replacement
with complete image/source/state equality, and closes with zero owners.
Validation and sanitizer observations are clean.

## Replay and independent verification

Native observation **v5** adds mandatory `loadFailure` metadata. Earlier
schemas retain their reader paths. The instrumented CPU receiver replays
all fourteen hardware histories: **1,946 paid steps**, exact completion
records and final actor identities. It reproduces the selected partial
allocation prefix or decoder scratch refusal. Source failure replays the
recorded read outcome without retrying an external missing-file effect.
CPU replay makes no GPU recovery or pixel claim.

The independent oracle reconstructs source/container identity, each
allocation/free extent, dependency publication, binary32 walking and complete
recovery images. Stage records identify even a source-allocation failure
with no allocation event. Failed attempts must contain exactly their allowed
allocation prefix, never a ready event, and destruction before retry. The
recorded source is empty for field 0 failure, the retained zeroed span for
later allocation/read failures, and the original source for decoder refusal.
Failure bytes do not become an admitted resource.

Each GPU history rejects **47 altered receipts**, including effect identity,
phase/result, removed stage/destruction, source hash and false readiness:
**1,316** rejections. Each CPU history rejects **27 altered inputs**, including
missing/downgraded metadata, every effect word and a changed typed completion:
**378** rejections. The authorities kill **102 load-policy mutants** and
**9 replay-profile mutants**, zero survivors. Earlier passing gates on
unchanged policy sources are retained at their own commits.

The current healthy full route also passes the reported-loss oracle and
matches the earlier `505962f336` route at every one of **12,931 paid-step
hashes** across **3,238 frames**. Its accepted motion, events and actor
identities remain exact. The current CPU receiver also accepts the existing
v4 pressure history and rejects its 34 altered inputs.

The browser's allocator shares the stop-at-first-failure change, while
native fault inputs and typed completion remain outside its ABI. The
rebuilt browser rescue carrier passes actual pending-load and failed-adapter
reloads, full state/image restoration and final cleanup. Their independent
oracle rejects nine altered receipts each. Browser allocator exhaustion is
not inferred from those regressions.

## Artifacts, timings and remaining scope

`load-runtime.json`, compact oracles and source/artifact manifests beside
this record pin the observed allocation/category/owner peaks, stage timings,
frame quantiles, source graph, generated C and exact executables. Artifacts
live under `~/.openagents/aethyrforge/streaming-12990/2026-11-07/`:
`load-native-v2`, `load-native-san-v2`, `load-cpu-v1-san`, the
`load-NAME-tier{0,1}[-san]-v1` cases and `load-evidence`. Red/precursor builds
remain separate. The raw runtime images and generated binaries stay private.

```sh
coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_BUILD [--sanitize]
NEW_BUILD/stream-native SCENE_DIR SCENE_SHA256 NEW_BUILD/stream-draw.spv NEW_OUTPUT TIER --load-alloc-2
python3 aethyr/tests/streaming_native_oracle.py SCENE_DIR NEW_OUTPUT NEW_BUILD/stream-draw.spv --fault-checks
coax/web/tool.sh aethyr/tools/stream-walk-native.coax SCENE_DIR NEW_OUTPUT/receipt.json NEW_CPU_BUILD --sanitize
python3 aethyr/tests/streaming_native_oracle.py --cpu-replay SCENE_DIR NEW_OUTPUT NEW_CPU_BUILD/replay.json
```

All commands are individually bounded below fifteen minutes. Rendering is
headless and muted; no system changes or owner actions are needed. Software
timings are not hardware performance claims. RCM1 still uses the existing
logical GPU cost model; explicit Vulkan backing is measured separately.
Vulkan resource factories and upload helpers retain fatal SDK paths and
require checked cleanup/retention before their failure surface is qualified.
Asynchronous native file workers and actual superseded callbacks are separate
exits. This stage does not promote chapter status or close the issue.
