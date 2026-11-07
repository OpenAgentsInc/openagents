# Provisional streamed walk and native CPU replay — November 7, 2026

**Historical stage; #12990 remains open.** The later
[coordinator/replacement qualification](walk-coordinator-recovery.md)
supersedes this stage's failing authority gate and SDK implementation.
These observations exercise a real browser consumer
and a native CPU replay. The coordinator's mutation gate is not passing;
neither a native GPU consumer nor device recovery is qualified. The first
observations below are historical; the later section covers the current
role-table coordinator, cancellation fix and both desktop visual tiers.
Artifact identities distinguish the actual executed builds.

## Consumer and original content

`aethyr/world/stream-walk.coax` composes the existing resource stream, cell
controller, entity store, semantic query adapter, walking/camera clock and
navigation grid. `aethyr/host/tests/stream-walk-face.c` performs concrete
calloc/free after the original grants, and projects scalar SDK calls.
The browser adapter uses the existing pack host, audio-memory owner,
decoded-image owner and renderer ledger. Coax produces the draw plan,
camera matrix and shader. The host creates real WebGPU buffers and full
mip textures, waits for actual queue completion, and retains immutable
resource borrows until the final draw completes.

The authored CC0 corridor is 12 cells of 64 m, or 768 m end to end. Its
64 resources require 62,106 retained model bytes at browser tier 0;
the chosen dynamic resident cap is 32,768 bytes. Seven category caps are
8,192/512/4,096/2,048/8,192/4,096/2,048 bytes, in the documented category
order. Four active actors and one prepared placement are distinct limits.
There are at most four unfinished resource loads and two collected voices.
Audio is decoded into retained PCM; playback is not qualified here.

The v2 scene preserves the source bytes from the v1 aligned corridor but
changes dependency metadata: geometry needs indices, placement bodies need
collision, and light/texture collection is independent. A prepared neighbor
does not acquire spawn authority. It has these immutable identities:

| Artifact | SHA-256 |
| --- | --- |
| `corridor-v2/scene.json` | `ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810` |
| Browser tier 0 RCM1 | `1c386700b972ee0d1a31aa2f47656961f108fdb2a6b359c62ef98a31534af738` |
| Executed browser Wasm | `7bd8f7ad1c52c8fba351a8cdfc702d16362e73e059fe7655409b6cf7eb3a08bd` |
| Browser physical route JSON | `1b13e1a2d7235707df21f3b8b1e4bf5ef4e87e5ca8fef34a356f611abcd4c959` |

## Actual browser route

Host: coderos-4080, Linux x86_64, NVIDIA GeForce RTX 4080, driver 595.71.05,
muted headless Chrome 152, adapter vendor `nvidia`, architecture `lovelace`.
Compiler: Coax 0.52.0, executable SHA-256
`efaeeae664be88349d3aea9106867fcb2a8d33dc06d2679bac89b2809aeb55b3`.

Six one-step warm-up frames precede 1,600 four-step frames in each
direction. The route advances 12,806 paid ticks over 3,206 frames, from
z=56 to z=767.9873046875, then back to z=18.3974609375. All twelve durable
IDs survive, and the first actor's update counter advances again on return.
Coax semantic queries combine ground boxes before actor capsules in the
kernel's shape order. A held cell edge stops planar velocity so the first
reverse input can move back into admitted content.

| Observation | Value |
| --- | ---: |
| Peak active streamed actors | 4 |
| Peak declared dynamic resident bytes | 18,171 |
| Actual requested CPU allocation peak | 14,132 |
| Physical allocation/free events | 680 |
| Load attempts, eventual physical retirements | 114 / 114 |
| Actual draw commands | 16,274 |
| Declared lighting fallback draws | 3 |
| Load failures / GPU validation errors | 0 / 0 |
| Frame work p50 / p95 / p99 | 0.7 / 2.1 / 3.4 ms |

Those frame times measure this accelerated input route, including its
awaited GPU work, while mutation workers were active. They are not a real
RAF cadence, uncontended benchmark, mobile-device measurement or general
engine frame guarantee. Per-load allocation/source/decode/upload/total
timings are retained separately with source and request identities.

The independent Python oracle reads the actual RCM1 costs and SDK events.
It reconciles each attempt's encoded, decoded, plan-object, decoder-work and
upload allocation once and each actual free once, checks every category
cap, and verifies that all 64 rows were exercised. Its independent binary32
flat-corridor integrator matches every observed position exactly. It checks
all accepted source hashes and the complete success/ready/destruction set.
Cleanup reaches zero dynamic CPU, audio, decoded-image, inbox and renderer
owners. The shared pack host retains its existing 8 MiB master.

The streamed reactor reserves an additional 8 MiB ceiling through the same
pack storage owner; its actual linear extent grows from 2 MiB to 2,162,688
bytes. Creating memory at the maximum initially left libc unable to grow
after a linker heap-tail change; the fix permits growth within the unchanged
reserved ceiling. The fixed CellWorld/controller/observer metadata is about
774 KiB and is separate from the small dynamic-resource cap. The uniform
buffer and two 640x360 GPU targets are separately retained ledger owners.
SDK objects, allocator overhead, GC, driver backing and process RSS are not
measured as zero or included in the logical texel-byte totals.

Linux's compositor screenshot omitted the WebGPU canvas. The `.png` receipt
therefore comes from the actual owned color target through a separately
granted GPU readback buffer and retained CPU copy, normalized and PNG-encoded
in Coax. The oracle rejects a flat image. The image is evidence of this
fixture, not visual parity for a material or lighting pipeline.

## Native CPU replay

`aethyr/tools/stream-walk-native.coax` builds the Coax replay driver and the
same C allocation adapter, then reads the browser's recorded inputs and
completion/destruction history. It authenticates all fixture containers and
source spans and executes the actual Coax decoders. Effects are prepared
when requested and completed/destroyed only at their recorded boundaries.
The native observer makes no GPU SDK objects and uses the browser cost model
for this cross-host CPU comparison.

All 3,206 frame positions match in binary32, all 23 words of each emitted
record match, and active/drawn/updated/resident counts match per frame.
All five final cell fields, including entity IDs and update counters, match.
The three closing frames also match their recorded events and position;
the final tick is 12,809 and all 114 physical attempts retire. Native CPU
allocation peaks at 14,819 bytes because scratch is prepared synchronously
before the recorded completion. Actual bytes remain covered by the original
inbox tickets. GPU layout/performance is not inferred from that comparison.

The independent oracle reserializes event words and frame-position bits,
obtaining observation SHA-256
`164312fdd2709f6e0212bf3e9bc49aa23b586fd7d85e01dc9c847684cee02c60`.
It also changes one expected position, changes a completion generation, and
removes one completion; all three altered histories are rejected with no
successful output receipt. The complete native route passes ASan/UBSan with
leak detection enabled and no sanitizer output.

| Executed artifact | SHA-256 |
| --- | --- |
| `native-replay-cells/replay.c` | `11fc755ffe032df9bb6917fdbc49166d2aa1a723abc39ae5aab8b5c5a3140a59` |
| `native-replay-cells/replay` | `ae428cf9af26f0553dfa1e22941ef7f75ea28364b2c986416f1b8b9d45305390` |
| `native-replay-san/replay` | `1fb2cae07d7b38e97de96cb57bf95bdf384c626865563fe5b11d323e00de1844` |

This replay compares the recorded frame boundaries and event ticks; it does
not yet record and compare a complete state digest after every individual
paid step. The retained route contains successful loads. Failure, stale,
supersession and cancellation histories still need actual-host replay
qualification, as do different visual tiers and device replacement.

## Reproduction and artifacts

Artifacts live below
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`: `corridor-v2/`,
`stream-walk-browser-physical.json`, its `.png`, `.allocations.json` and
`.stages.json` companions, and the two native replay directories above.
Generated C and headers remain with the native executables. No downloaded
or licensed source content is used by this fixture.

Commands, each bounded below fifteen minutes:

```sh
coax/web/tool.sh aethyr/tools/streaming-assets.coax NEW_SCENE_DIR
python3 aethyr/tests/streaming_assets_oracle.py NEW_SCENE_DIR
coax/web/tool.sh aethyr/tools/stream-walk-browser.coax BASE_URL SCENE_JSON ROUTE_JSON --route
coax/web/tool.sh aethyr/tools/stream-walk-native.coax SCENE_DIR ROUTE_JSON NEW_NATIVE_DIR
ASAN_OPTIONS=detect_leaks=1:abort_on_error=1 UBSAN_OPTIONS=halt_on_error=1 coax/web/tool.sh aethyr/tools/stream-walk-native.coax SCENE_DIR ROUTE_JSON NEW_SAN_DIR --sanitize
python3 aethyr/tests/streaming_walk_oracle.py SCENE_DIR ROUTE_JSON NATIVE_DIR/replay.json
coax/bin/coax test aethyr/world/stream-walk.coax
```

The current coordinator passes 628 loaded checks. The earlier full mutation
exploration had 378 survivors; an expanded-test shard subsequently had 70
survivors among its 284 sites. New lifecycle and collection checks and a
shared resource-role table address gaps and redundant guards. The complete
four-shard result is **978 mutants, 833 killed, 145 surviving**, retained in
`walk-mutation-merged.log`. This is a failing gate. No equivalent waivers
were used. Every shard completed within the command-duration limit.

Outstanding adoption includes the persistent HUD, a native GPU host,
device replacement, exact per-step replay, the full hostile-load matrix,
final residual-adapter inventory, and the
affected teaching/registry update. No issue closure or publication follows
from this provisional record.

## Updated coordinator, cancellation and visual tiers

The updated role-table coordinator passes the actual full low/high browser
routes and native CPU replay. `walk-source-sha256.txt` pins its sources.
The two independent summaries are `walk-low-oracle.json` and
`walk-high-oracle.json`; `walk-tier-comparison.json` compares their gameplay
frames and retained source identities. Shader and fixture source bytes are
unchanged. The selected manifest changes image layout from 1x1 to 2x2 plus
its full mip chain, with separate decoded-image and logical GPU costs.

| Measured desktop tier | Low | High |
| --- | ---: | ---: |
| Peak declared resident bytes | 18,171 | 18,299 |
| Actual requested CPU allocation peak | 14,132 | 14,212 |
| Final dynamic logical GPU bytes | 2,484 | 2,532 |
| Final decoded-image bytes | 12 | 60 |
| Native CPU replay allocation peak | 14,819 | 14,883 |
| Frame work p50 / p95 / p99, ms | 0.7 / 2.4 / 3.6 | 0.7 / 2.7 / 4.5 |

All 3,206 gameplay frame tuples (input, tick, position and active/drawn/
updated counts) agree between these two live observations. The full cell
state at both turnarounds agrees, including all twelve update counters and
durable IDs. Each route has 114 attempts and no GPU errors or load failures;
source identities and concrete attempt identities agree. Callback order is
not claimed to be identical between live schedules. Both native replays
compare their own accepted histories, including cleanup. The low native run
passes ordinary O2 and ASan/UBSan builds; the high run passes ASan/UBSan.
The high oracle also rejects all three deliberately altered histories with
exit 1, empty diagnostic output and no successful receipt. Negative observer
runs stop on a mismatch without draining their test allocations; only these
negative child processes disable leak checking. The complete positive
sanitized routes keep leak detection enabled.

Updated artifact identities, all under the same November 7 shared directory:

| Artifact | SHA-256 |
| --- | --- |
| `stream-walk-role.wasm`, 120,187 bytes | `ef2ad4a96d193e129561e40ee0c2b78576c98d0a7b6b93c89745c33b1653f490` |
| `stream-walk-browser-role.json` | `b20392ea5018cffcbfd5fe82c4b8b17fb88a0718424f878ff2184d7ac727e021` |
| `stream-walk-browser-high.json` | `afd6934e9773b2556ea8c894c2a3a05922cd29194d691379f17b6a23ba583487` |
| `native-replay-role/replay` | `3d5cbef29576686bd3a0b21852877236ff19fdda7d87230e8e905c53f4f53943` |
| `native-replay-role-san/replay` | `386736f565bb47a6ba04b480fc69c8e94e3de8d83ecda807d86d39b8ffea4217` |
| `native-replay-high-san/replay` | `9e26836872df7b74e3ee76135172e4bf70fba3520d8a682089f139c49764c906` |

The two JSON traces have `.png`, `.allocations.json` and `.stages.json`
companions. `--route-high` selects the high descriptor; the native replay
reads and checks the trace's pinned tier/manifest/scene identity. The legacy
low trace without that optional profile object remains readable.

The actual stalled-stream probe first downloaded the 24-byte collision
source over HTTP, then offered those complete bytes in a browser
ReadableStream that never ended. Before the fix, close exceeded its five
second observation deadline, with one live reader and no cancellation. This
is retained as `stream-walk-stall-held-before.json`. The earlier
`stream-walk-stall-before.json` did not install its injection and is an
invalid fault rehearsal, not evidence of a reproduced product failure.

The browser now signals the existing pack reader cancellation when a loading
resource becomes unwanted and when the world closes. It awaits the actual
pack retirement before offering completion or releasing reservations. The
same probe then observes cancellation, no ready collision prefix, twelve
attempts and twelve physical retirements, zero CPU/audio/image/GPU owners,
and the pack host's one existing 8 MiB master reservation. The successful
receipt is `stream-walk-stall-after.json`, SHA-256
`55a3a1e20fcfde17a9a63b11520305c598dca477f955b997316ac3007d94b7d3`.
That before/after probe uses the earlier compiled coordinator; cancellation
is a browser SDK ownership change. The subsequent current-coordinator low
and high full routes exercise its ordinary success/cleanup path.

Actual GPU objects are registered for retirement immediately after their SDK
creation, before uploads can throw. Error scopes always pop, and draw-borrow
acquisition is inside the return-on-failure region. More injected upload,
device-loss and late-generation cases remain required before qualifying the
complete failure matrix. No mobile hardware claim follows from either tier.
