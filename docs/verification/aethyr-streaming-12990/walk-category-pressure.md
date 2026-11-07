# Actual native texture-category refusal — November 7, 2026

Starting from `505962f336`, the actual native corridor now qualifies the
category-pressure acceptance item of #12990. The issue remains open for
physical/transport/transient cost planning, remaining failed/superseded load
ownership paths and the final adoption/teaching audit.

## The real request boundary

After the usual 32-frame warmup, `--category-pressure` reads the scene's
original 91-byte 8×8 PNG. Coax builds and reads a content-addressed one-entry
RCM1 companion, then checks the derived plan against those source bytes.
It invokes `resource_budget_admit` with the live `SwWalk` budget, inbox and
renderer registry. This is the same shared admission function used by
`resource_stream_request`, with the same actual retained owners. No copied
budget, private allocator, reduced cap or invented author estimate is used.

This is an additional candidate companion, independent of the corridor's
64 authored resource rows. The existing world manifest and its two tier
identities stay unchanged. The gate refuses before the candidate can create
an owner, allocate its planned spans, upload or publish. Successful publication
of an additional companion is outside this refusal probe. An unexpected grant
fails qualification and retires its speculative tickets before returning.

| Derived request cost | Bytes |
| --- | ---: |
| Encoded PNG | 91 |
| Decoded full mip chain | 340 |
| Logical GPU mip chain | 340 |
| Retained plan object | 168 |
| Total retained | **939** |
| Decoder scratch | 298 |
| Native upload scratch | 340 |
| Total scratch | **638** |

The canonical decoder supports this source at side 8 or 16. The original
corridor sides 1/2 do not downsample an 8×8 source; those plans refuse as
invalid. This probe requests side 8 explicitly. The companion SHA-256 is
`3a97fd81a997b1ef4459e85985db634ab749b9986cffeddff1f1b2027ffbc809`.

| Live host before request | Tier 0 | Tier 1 |
| --- | ---: | ---: |
| Texture category used / cap | 256 / 512 | 288 / 512 |
| Aggregate used / cap | 15,656 / 32,768 | 15,784 / 32,768 |
| Staging used / cap | 0 / 2,048 | 0 / 2,048 |
| Aggregate including requested retention and scratch | 17,233 | 17,361 |

The returned reason is `ResourceAdmissionStatus.category`. Scratch fits its
line, and the whole request fits the aggregate. The texture line alone
refuses. Existing resource requests/generations, inbox bytes/count/serial,
registry epoch/live count/words and concrete CPU/GPU owners remain unchanged.
No resource allocation/free event occurs at the refusal boundary.

The budget's refusal counter increments once, so its canonical state digest
changes. Normalizing only that counted diagnostic recovers the exact previous
canonical digest. The record preserves both real digests and this separate
comparison; replay applies the actual counted refusal.

## Rendering, replay and independent checks

Both original tiers run on the RTX 4080 with Khronos validation, and again
under ASan/UBSan/LSan on Lavapipe with two workers. Every run compares all
230,400 image channels before/after refusal, continues eight paid movement
steps, performs controlled device replacement with the existing full-image/
source/state checks, then drains all owners. No validation or sanitizer
messages occur. Software timings are not hardware performance claims.

Native observation **v4** adds the pressure record; older observations keep
their reader paths. CPU replay requires the pressure record only for v4,
rebuilds its candidate from the pinned source and reissues admission between
warmup and the next paid frame. The two hardware histories reproduce all
**278 paid-step hashes**, completion records and final actor identities.
CPU replay makes no GPU pixel or device-replacement claim.

The independent Python oracle uses its existing standard-library PNG
reference to decode every probe texel and derive scratch. It reconstructs
all 488 companion bytes, including format/model identity and every cost
lane. It independently checks texture/staging/aggregate pressure, unchanged
owner observations, every old-scene image channel, movement, allocation
lifetimes and final cleanup. It rejects **62 altered receipts per run**,
including all thirteen repaired-root cost edits: **248** across four runs.
The CPU oracle rejects **34 altered histories per tier**, including omitted
or downgraded pressure records, every evidence word, state/content identities
and changes to the bound tick/queue serial: **68** total.

The changed pure authorities kill **63 pressure-conformance mutants** and
**7 replay-profile mutants**, zero survivors. The first conformance trial
exposed redundant guards and missing exact-boundary tests; those were removed
or covered without equivalents. Earlier SDK/policy gates remain retained at
their own source commits. A current receiver also replays the original v3
steady-loss history and rejects its twelve existing altered inputs.

The changed C bridge is enabled only when the native pressure type is in the
generated header. Rebuilding the browser rescue carrier produces byte-for-byte
the same 143,578-byte Wasm as `505962f336`, SHA-256
`2da263774ae49149c76457516776683a5667654b412dd9035ea823b8c7bb0345`.
Its previously passing pending-load/failed-adapter browser observations are
reused; the unchanged binary was not rerun.

## Artifacts and limits

`category-runtime.json`, the compact oracles and source/artifact manifests
beside this record pin the exact observed profile, quantiles, category/owner
peaks, stage timings, binaries and compiler inputs. Private artifacts live at
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`: native builds
`category-native-v3` / `category-native-san-v1`, four
`category-pressure-{low,high}[-san]-v1` histories and the final
`category-cpu-v3-san` receiver. `category-evidence` retains exact sources,
logs and earlier failed invocations. Commands are individually bounded.

```sh
coax/web/tool.sh aethyr/tools/stream-native-build.coax NEW_BUILD [--sanitize]
NEW_BUILD/stream-native SCENE_DIR SCENE_SHA256 NEW_BUILD/stream-draw.spv NEW_OUTPUT TIER --category-pressure
python3 aethyr/tests/streaming_native_oracle.py SCENE_DIR NEW_OUTPUT NEW_BUILD/stream-draw.spv --fault-checks
coax/web/tool.sh aethyr/tools/stream-walk-native.coax SCENE_DIR NEW_OUTPUT/receipt.json NEW_CPU_BUILD --sanitize
python3 aethyr/tests/streaming_native_oracle.py --cpu-replay SCENE_DIR NEW_OUTPUT NEW_CPU_BUILD/replay.json
```

This closes the measured native logical-category refusal gap. RCM1 still
uses logical GPU payload costs here. Explicit Vulkan backing remains measured
separately and needs full planning/admission before #12990 closes. The source/
report buffers are bounded qualifier storage, separate from candidate retention.
This stage does not qualify phone hardware, browser category pressure, native
factory/upload failures or superseded callbacks. Chapter status is unchanged.
