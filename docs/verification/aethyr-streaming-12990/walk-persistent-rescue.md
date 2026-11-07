# Streamed browser rescue — November 7, 2026

**#12990 remains open.** This stage starts at local checkpoint `6d385aeb3a`.
It adds actual persistent rescue for a separate exact-build streamed browser
carrier. The ordinary host-v1 state-region save is unchanged. This carrier
owns pointers into C statics and allocated source/decoded spans, so copying
only a gameplay struct would leave its restored ownership incomplete.

## Saved boundary

`stream-walk-rescue.coax` loads the preceding device/HUD carrier and the new
`host/rescue-policy.coax` authority. Before each ordinary frame the SDK holds
the entire imported Wasm32 memory, including C statics, malloc metadata,
source/decoded bytes, immutable pending facts and their views. The actual
qualified memory extent is **2,097,152 bytes**. It has a separate retained
transport ticket; it is additional to the eight-MiB engine reservation and
eight-MiB page transport service, not part of the 32-KiB streamed-resource
budget. Close releases the checkpoint ticket too.

A trap or failed GPU replacement saves that held image to IndexedDB
`coaxial-rescue`, store `saves`, under the separate
`aethyr.stream-walk.<slot>` key. The stopped overlay has one reload action;
it becomes available after the save transaction settles. Failed or suppressed
saving offers start-over. The qualifier inspects that action; its reload is
issued through CDP rather than an owner click.

One read/delete transaction consumes a record before granting restoration.
The SDK hashes the actual engine and image bytes with WebCrypto. Coax checks
all eight words of each engine/scene/manifest identity, image checksum,
layout version, matching tier and exact page-aligned extent. This is local
checkpoint integrity/compatibility, not authentication of hostile records.
A checksum, build, tier or declared-size mismatch starts fresh. No raw image
from another build is written over the running memory.

The new instance uses the same immutable module and memory-address layout.
It initializes before putting back the admitted memory. CPU allocations and
leases retain their saved addresses; GPU generations are replaced through
the existing checked `sw_recover_gpu` boundary. Successful retained sources
upload without a simulation step. Unfinished source effects restart against
those original allocations and complete through the ordinary immutable-fact
queue. The old document's SDK callbacks cannot enter the new instance.

Queued facts remain pending inputs until the existing paid boundary. The
checkpoint observer temporarily excludes their count while asking the v1
accepted-state serializer, then restores it. It neither applies nor drops
those facts. The complete saved image retains their actual payload/views;
the accepted-state digest alone does not claim to serialize pending SDK
work. Fresh native CPU replay below verifies the resulting accepted history.

The loss wall clock is initialized anew on reload; a former document's
`performance.now()` origin is not carried into the new policy. Simulation
clock, actor IDs, sleeping state, inbox tickets and resource generations
remain retained. A stop before **300 frame starts** following a restoration
suppresses another save; the exact threshold is checked in Coax. The actual
crash-loop probe stops on the first subsequent frame and reloads fresh.

## Actual hardware and replay

All probes use muted headless Chrome 152 on coderos-4080's RTX 4080, tier 0
at 320×180. The original scene and RCM1 identities remain unchanged. Before
normal failures, 160 frames have paid 640 ticks and walked away from spawn.
The pending case instead holds source fetches across two four-step frames.

| Mode | Actual observation | Rescue negatives rejected |
| --- | --- | ---: |
| `trap` | A real Wasm `unreachable` saves and restores the held accepted state, actors and all 230,400 GPU channels | 9 |
| `adapter-failure` | A real device-loss callback reaches a refused replacement adapter; reload restores the checkpoint and full image | 9 |
| `pending` | Four unfinished source allocations survive the image; fresh SDK effects finish them and continued play drains them | 9 |
| `once` | Successful restore/close followed by another reload starts at tick zero | 10 |
| `loop` | An immediate second real module trap suppresses saving; the next reload starts fresh | 10 |
| `engine` | Altered saved engine identity refuses in Coax with identity status 6 | 7 |
| `corrupt` | A changed retained image byte fails the freshly recomputed checksum, status 6 | 7 |
| `extent` | Altered declared byte extent refuses, status 4 | 7 |
| `tier` | Altered saved tier refuses, status 5 | 7 |

The independent Python oracle checks each actual image's complete SHA-256,
engine/content pins, full GPU readbacks, actor values, held/restored tick and
digest, 128 subsequent paid steps, single use, crash-loop outcome and final
owners. It rejects **75 altered receipts**. The original two-MiB images and
readbacks remain private artifacts, not committed payloads.

Every resulting accepted history also runs through the unchanged preceding
ASan/UBSan/LSan CPU receiver. It compares **3,107 paid steps across nine
histories**, including continued play and cleanup. All positive runs finish
with leak detection enabled and no sanitizer output. The independent CPU
oracle reconstructs every observed state/event/position byte and rejects six
altered histories each: **54 rejections**. Fresh-start cases replay only the
new session. Restored cases replay exactly the prefix retained by the image
plus the recorded continuation; rejected/rolled-back frames are not inserted.
These are recorded-input replays, not assertions about equal live completion
timing or recreation of GPU effects.

Final dynamic resource, display, audio/image and unfinished-operation counts
are zero. The page transport service remains **8 MiB**. The original document
is retired by navigation; these checks do not measure browser GC, driver
private allocations or the physical release timing of the old document.

Normal trap-case offered-frame p50/p95/p99 are **5.11/8.50/13.46 ms**.
The probes overlap eight headless browsers and copy/hash checkpoints; these
are unpaced observations, not 60-FPS or mobile qualification. The final
adapter-failure run executed separately at **2.05/4.50/8.51 ms**. Per-frame
raw timing and private snapshot identity remain in each receipt.

## Checks and failed trials

- Rescue compatibility/crash-loop authority: **65 mutants killed, zero
  survivors**. The first run had two survivors: a redundant byte ceiling
  followed by page-alignment refusal, and an extra identity-loop iteration
  hidden by all-zero positive fixtures. Page bounds now directly express
  32–128 pages, and positive fixtures use distinct engine/content/image
  identities. No equivalent exemption was added.
- Pending-fact checkpoint observer: **15 mutants killed, zero survivors**.
  Its checks require exact spans and preservation of pending count, payload
  and tick. Imported baseline checks remain enabled in both root gates.
- The previous automatic-device Wasm runs the current SDK's real loss,
  exact-state/image recovery and cleanup; its independent oracle rejects
  nine altered receipts. The previous explicit HUD carrier runs current SDK
  replacement with equal pixels/state, no source requests and successful
  continuation/cleanup. Neither older carrier opts into rescue storage.
- Tools gate passes: **1,860 tracked files, zero new without a firm reason**.
  All six chapter registries pass with unchanged status counts. No chapter
  is promoted. The unrelated historical full-book listing drift is not rerun
  or claimed fixed.

The first real browser trial trapped before its intended injection: the v1
state observer correctly refuses a pending-fact count. The new checkpoint
observer resolves that by retaining pending inputs separately from accepted
state; it does not weaken the serializer's paid-step assertion.

The first adapter-failure image comparison correctly failed: the held
checkpoint preceded the most recently rendered moving frame. The observer
now offers a zero-nanosecond frame immediately before loss, so the held
checkpoint and pre-loss image name the same pose. The earlier failed trial
remains retained; its later image is not substituted into the new receipt.

## Reproduction and limits

Artifacts live under
`~/.openagents/aethyrforge/streaming-12990/2026-11-07/`.
Final directories are `browser-rescue-MODE-v3`, except
`browser-rescue-adapter-failure-v4`. `rescue-observer-v3.coax` retains the
original observer for the eight unchanged passing modes; the current tool's
only subsequent change is the failed-adapter zero-time alignment above.
CPU reports are retained both beside their input and/or the unchanged
`native-history-replay-v1-san/replay` receiver. Source and artifact manifests
pin all these identities, exact source snapshots, failed trials and reports.

The executed **143,459-byte** `stream-walk-rescue-v2.wasm` SHA-256 is
`eeb359db881b81861b3c9c8dd42bb7b90aa7c59f87a43a647a9d9f35edd28ed7`.
Build and observe with bounded commands:

```sh
coax/web/wasm.sh aethyr/world/stream-walk-rescue.coax aethyr/host/tests/stream-walk-rescue-face.c NEW.wasm
coax/web/tool.sh aethyr/tools/stream-rescue-browser.coax BASE SCENE_JSON NEW_OUTPUT MODE
python3 aethyr/tests/streaming_rescue_oracle.py NEW_OUTPUT MODE NEW.wasm --fault-checks --history
coax/bin/coax mutate aethyr/host/rescue-policy.coax --library --root-only --jobs 8
coax/bin/coax mutate aethyr/world/stream-walk-rescue.coax --library --root-only --jobs 8
```

The header reserves layout 1 and admits 2–8 MiB of page-aligned Wasm memory;
only the two-MiB profile has actual rescue evidence. Larger images use the
ordinary host's once-per-30-frames holding cadence, which can restore an older
frame, and need their own allocation/timing/age tests. This is not a portable
save format or a native pointer image. Native automatic loss/rescue, mobile
hardware, storage quota/denial and interrupted transactions, actual concurrent
save consumers, deep engine-stack traps and rescue after rate/deadline stops
remain unqualified. The normal host-v1 adapter remains a named residual.

Physical backend/transport/transient planning, controlled category pressure,
remaining stale/allocator/SDK histories, independent world-shader reference
and final consumer/book audit still prevent #12990 closure. No issue comment,
push, closure, publication or owner action is claimed by this local stage.
