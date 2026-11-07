# Diagnostic pressure, capture costs and adapter inventory — #12990

This checkpoint starts at `ebc40a8e47` in the reused worktree. It closes the
auxiliary-browser work named in the [HTP1 adoption record](walk-transport-binding.md):
diagnostic overflow, capture/encoding temporaries, compiler-input extents
and an explicit [remaining-adapter inventory](../../../aethyr/host/stream-adoption.md).
Older records retain their original artifacts and scope. The issue's
requirement audit is separate from these component results.

## Bounded diagnostics preserve product cleanup

`diagnostic-trace.coax` admits 1–2,048 24-word records and derives exact
array bytes plus the existing page-rounded reservation. On overflow it
returns no observation index and counts dropped events without throwing
through resource effects. Closing still permits final drain, but never
claims a complete trace. The drop counter saturates at signed-int64 maximum.
Observation v2 carries `complete`, `exhausted`, `dropped` and
`traceReservedBytes`; historical v1 remains readable under its original
complete-history contract.

Actual browser probes hold a source with bytes delivered but EOF pending.
They exhaust the recorder through real loading/travel, start close and hold
the actual cancellation completion. The original source pointer/owner stays
live until that completion is released. The trace and world memories then
zero, every resource drains and only the original pack master remains.

| Probe | Frames / attempts | Recorded / dropped | Held resource |
| --- | ---: | ---: | --- |
| Diagnostic-only build, capacity 32 | 2 / 5 | 32 / 11 | Raw source owner 5 |
| Diagnostic-only build, capacity 2,048 | 9,771 / 320 | 2,048 / 12 | Packed texture owner 4 |

The independent transport oracle reconstructs only each retained prefix.
It reports still-live prefix operations separately from final cleanup,
and proves that its ordinary complete-history verifier rejects both inputs.
Missing releases are never inferred from final zero counters. Sanitized CPU
replay compares every accepted step, source span, record and position:
**39,087 paid ticks** for the long case and **10** for the small case.
The small case rejects **18** altered histories. The long case is explicitly
positive-only and reuses existing negative-policy evidence; it does not
claim new long-history negative runs.

The long run exposed the replay observer's former one-MiB output sink.
That diagnostic sink is now four MiB, matching the existing receipt bound;
the input parser and product runtime were not enlarged. The original
frame-3,516 observation failure is retained. No passing unchanged long run
was repeated after the independent capture policy was added.

The final integrated 156,237-byte world also passes a separate capacity-32
probe (`trace-capture-held-32-v2`), with the same held-owner/drain/zero checks,
explicit incomplete-prefix verification and sanitized accepted-history replay.
Its short-history oracle rejects 18 alterations. This composes both new
policies with the final browser cleanup adapter without repeating the passing
9,771-frame stress history.

## Capture and compiler input

`capture-cost.coax` admits dimensions through 4,096 with at most four MiB of
256-byte-padded readback. It derives raw copy, complete base64 extent and
logical UTF-16 peak. The existing renderer readback grant and two existing
storage reservations precede GPU buffer creation. The 32 KiB string chunk
capacity is prepaid even for tiny images. Raw copies zero in `finally`;
original reservations stay held through the GPU completion observation,
including failed encoding. A returned base64 value is explicitly the
caller's diagnostic output, outside the engine after return.

The final actual 320×180 fixture passes **seven named observations**:
world/reader compiler inputs zeroed; owners retained across held GPU
completion; encoder failure cleanup; text-budget pressure refused before
any GPU buffer; retry pixels unchanged; exact source extents; final cleanup.
The independent oracle reconstructs row padding, base64, text bound, page
reservations, actual storage pressure and program/shader file extents.

| Measured/planned field | Bytes or units |
| --- | ---: |
| Padded raw readback and copy | 230,400 B each |
| Raw-copy reservation | 262,144 B |
| Base64 result | 307,200 code units |
| Logical UTF-16 temporary peak / budget | 1,075,200 B |
| Text reservation | 1,114,112 B |
| Caller-owned result logical UTF-16 | 614,400 B |
| Actual world compiler input | 156,237 B, subsequently zeroed |
| Actual pack-reader compiler input | 36,403 B, subsequently zeroed |
| Three retained WGSL inputs | 7,691 code units / 15,382 logical UTF-16 B |

The program-input inventory does not reinterpret static programs as dynamic
world resources. Compiled programs, JS object/string representation,
serialization, GC, driver storage and RSS remain separately unmeasured.
Zeroing and explicit release do not assert immediate physical frees.

## Integrated two-tier profile

Both integrated routes complete **3,206 frames / 114 resource attempts**,
with **740 complete v2 events**, **117 plans/releases** and zero final
operation tickets. Each transport oracle rejects ten altered histories.
The allocation oracle checks every one of **680 payload allocation/free
events** per route, exact per-owner/category peaks and stage timing fields.
The unchanged content/gameplay comparison matches all 3,206 frames and all
twelve final actor cells. Retained-model peaks are 18,171/18,299 bytes;
final dynamic GPU layouts are 2,484/2,532 bytes.

| Tier | Frame p50 / p95 / p99 (ms) | Payload CPU peak | Sanitized replay CPU peak |
| --- | --- | ---: | ---: |
| 0 | 3.225 / 5.740 / 7.850 | 14,132 B | 14,819 B |
| 1 | 3.085 / 5.715 / 8.390 | 14,212 B | 14,883 B |

These concurrent correctness runs qualify fixed-history behavior, not a
quiet-machine live-FPS benchmark. Each CPU replay matches **12,809 paid
steps**, including cleanup, and rejects six altered histories. Both routes
peak at **50,855,936 reserved transport bytes** including world/master/trace;
actual reader/raw backing peaks at **33,554,520 bytes**. The new carrier
statics are **10,608 bytes**, separate from the existing 872,385-byte base
inventory and inside the reserved world arena. Stage durations, activity,
draw/update counts, counted pressure/refusals and individual owner peaks
remain in the receipts; no total process-memory claim is made.

Both actual device replacements preserve every pixel and cell without source
requests and allow continued movement. These runs precede only the final
capture-finally completion-hold change; that exact changed boundary passes
the seven-fact final SDK fixture. The world Wasm and normal route source are
identical. Earlier failure/cancel/supersession/category/native-backend
qualifications remain scoped to their pinned records and are reused.

## Source, commands and retained evidence

The scene and HTP1 roots are unchanged from `ebc40a8e47`. Original scene SHA:
`ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810`.
Final integrated world SHA:
`9c3a69edc2d8fc19a6a9d0d25c57437aaa9e9294c34cf85391892e855b4cfa5f`.
Diagnostic-only 155,273-byte world SHA:
`fe0c778ea9b4593dc8817442c24e03c980828201f9ec7abe9ebd87050530a6e4`.
The reader retains its prior SHA
`1fdbeee527640ce26e66bffc5cea367d62908fc3693b4071a6b6e33466b3ad90`.
Coax 0.52, GCC 15.2, muted headless Chrome 152 and NVIDIA Lovelace supplied
the observations. Policy gates kill **66/66 diagnostic + 58/58 capture root
mutants**, zero survivors and no equivalent waivers. Initial surviving
mutations/compile failures are retained alongside final gates.

Artifacts live under `~/.openagents/aethyrforge/streaming-12990/2026-11-07/`:
`trace-capacity-held-{32,2048}-v1`, `trace-capacity-cpu-v1`,
`trace-capture-route-{low,high}.json` and their sidecars,
`trace-capture-{recover,recover-high}.json`, and `capture-cost-sdk-v2.json`.
`trace-capture-site-v1` supplies routes/recovery; `trace-capture-site-v2`
supplies the final held-GPU-completion fixture. The shared planned, legacy
public/private/crypto/borrow, and physical split/cut/TCP-cancel loader suites
pass with the final `pack-host.js`; their `capture-pack-*-v1` directories
retain exact observations. Compact oracle/policy receipts accompany this
document; full logs and source/artifact SHA inventories retain precise scope.
The inventories pin 246 source files and 220 retained artifacts. All six
chapter registries pass without status promotion. The tools gate passes
1,871 tracked tools with zero missing firm reasons. The broader unchanged
book-gate failures recorded at `ebc40a8e47` are not represented as passes.

Use the existing Coax tools:

```sh
coax/bin/coax mutate aethyr/host/diagnostic-trace.coax --root-only
coax/bin/coax mutate aethyr/host/capture-cost.coax --root-only
coax/web/tool.sh aethyr/tools/stream-trace-browser.coax BASE SCENE_JSON HTP_ROOT NEW_OUTPUT 32 5
coax/web/tool.sh aethyr/tools/stream-trace-browser.coax BASE SCENE_JSON HTP_ROOT NEW_OUTPUT 2048 4
coax/web/tool.sh aethyr/tools/stream-transport-boundary.coax BASE SCENE_JSON HTP_ROOT OUTPUT --capture
python3 aethyr/tests/streaming_transport_walk_oracle.py SCENE_DIR HTP_DIR HISTORY --incomplete
python3 aethyr/tests/streaming_capture_oracle.py SITE_ROOT CAPTURE_RECEIPT
```

The [adapter inventory](../../../aethyr/host/stream-adoption.md) is the named
residual inventory required by #12990. Book source registries and chapters
16/38/53 describe current bounded evidence without promoting unfinished
teaching. Broader realm migration, range delivery, audible playback, mobile
hardware, native durable rescue and physical OOM/reset are not qualified by
this delivery.
