# Automatic streamed WebGPU loss — November 7, 2026

**#12990 remains open.** Starting from local checkpoint `ad3a303a05`,
the actual streamed browser consumer now handles unsolicited device loss
through checked Coax policy. It preserves the ordinary host's existing
three-losses-per-minute and absolute ten-second recovery contract. Failed
replacement stops paid frames and retains CPU/source owners for cleanup;
**persistent rescue is not implemented by this stage**.

## Boundary and ownership

`aethyr/render/loss-policy.coax` owns monotonic wall-clock admission,
window pruning, deadline, attempt serial, stale completion and failure
outcomes. Attempt fencing passes the original i64 token as BigInt through
the SDK; the exact decimal `generation` also drives live timestamp reset.
The numeric serial remains diagnostic. A loss during rebuilding spends another serial without extending
the original deadline. Exactly minute-old events expire; the fourth loss
strictly inside the window stops. The diagnostic retains the three stored
timestamps, so the fourth-loss receipt has three retained entries and four
observed loss events. Exhausted serials stop rather than wrap. This policy
has no simulation clock or area owner.

The new `stream-walk-device.coax` composes the original walker, retained
GPU-handle replacement, HUD and canonical serializer. Its SDK initializes
the policy with compiler defaults. The C face asks `gl_frame_allowed`
before calling the walker: rebuilding admits no paid frame, and stopped
cleanup needs the session's actual `closing` flag. A caller's optional
JavaScript cleanup argument cannot grant that permission.

Browser callbacks report loss and execute admitted effects. Old loads,
render borrows and error scopes quiesce before replacement. GPU objects
are destroyed before the existing atomic handle replacement; sources stay
under their original CPU tickets. Each asynchronous device/pipeline/upload
stage checks its attempt serial and the Coax deadline. Superseded builders
unwind before another builder spends their grants. An overdue real device
is destroyed when its actual request completes and cannot publish a new
surface. A timeout stops new frames while outstanding effects retain their
ownership; cleanup waits for those effects to finish.

Normal and stopped close both retire the actual resource owners and fixed
display grants. The page-wide transport reactor remains a separately owned
**8 MiB** service; the consumer's additional 8 MiB engine reservation is
released. Final resource/display/operation counts are zero, not the entire
browser process footprint. Wasm GC/physical commitment and complete
transport/transient planning remain outside this qualification.

## Actual hardware probes

All probes use muted headless Chrome on coderos-4080's RTX 4080, tier 0 at
320×180. Device loss comes from real `GPUDevice.destroy()` and its actual
`lost` callback. The retained source scene and browser RCM1 companion are
unchanged; scene identity is
`ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810`.

| Probe | Observed outcome |
| --- | --- |
| Steady loss | Original sources, complete canonical digest and all 230,400 RGBA channels preserved; continued paid step and cleanup pass |
| Repeated loss | Three recoveries, then a fourth-loss stop; ordinary and cleanup-argument calls cannot step the stopped session |
| Loss while building | A real replacement is destroyed during shader compilation; a second attempt succeeds under the original deadline, with exact state/image |
| Adapter failure | Actual replacement API returns no adapter; the session stops, preserves CPU/source owners and drains on close |
| Deadline | Actual adapter promise is held through the unchanged ten-second deadline; active/stopped face calls refuse and late completion cannot restart it |
| Late real device | A real hardware device is created but its return is held past the deadline; it is physically destroyed after release, without publishing a surface |
| In-frame loss | Loss during queue completion does not omit or duplicate the already accepted four-step frame; subsequent play and cleanup pass |
| Out-and-back route | Two unsolicited losses while traveling; both preserve the complete current digest, persistent HUD and paid tick; 768 m outward and back retain actor IDs and saved progress |

These are **13 actual loss events across eight histories**. The full route
has 3,233 ordinary frames plus cleanup, **12,932 paid ticks and 114
attempts**. Active/drawn/updated peaks are **4/3/4**; the final observation
reports 324 blocked steps, zero audio deferrals, 53 resource refusals and
1,105 cell refusals. These are reported pressure counts, not evidence of
the still-required controlled texture-category refusal with prior content
usable. The first area unloads with the HUD live, and returning advances
its saved actor state.

The route's measured dynamic CPU peak is **14,132 bytes**, and reported
fixed core/face metadata is **872,385 bytes** in this Wasm32 carrier.
Logical fixed GPU storage is 1,024,256 bytes (uniforms, HUD vertices,
depth, display and FP16 target), separately from dynamic resources. Actual
SDK/driver backing is not inferred from those logical extents. Unpaced
frame p50/p95/p99 are **1.7/4.1/10.7 ms**; the probe overlapped other headless
loss probes, and these are not interactive FPS or phone measurements.
Per-case timing and counters are retained in the compact oracle records.

The independent loss oracle checks full images, canonical equality where
no input step was offered, unchanged ticks across each replacement,
absolute deadline, replay/stop counts, upload closure and final owners. It
rejects **72 altered receipts** across the eight cases. Active frame-gate
sidecars additionally observe refused SDK calls while actual replacement
is pending, after its loss callback has arrived.

Every accepted history also runs through the unchanged preceding
ASan/UBSan/LSan CPU receiver. Every paid-step digest, event word and final
actor value matches. The full route compares all **12,932** paid steps;
the smaller cases compare their complete 131/132/136-step histories. Each
CPU oracle rejects six altered histories: **48 refusals** in total. This
replays accepted simulation history, not GPU effects or wall-clock loss
notifications. Different live completion schedules remain distinct inputs.

## Live clock and compatibility

A separate real `?play=1` RAF probe holds a real replacement device's return
for one second. Initially the following frame was offered **1,099.9 ms**,
including **1,094.1 ms** of rebuilding time; the fixed-step cap still paid
eight unintended ticks. The live driver now resets its wall timestamp when
the Coax recovery serial changes or replacement is pending. The corrected
held frame is offered **zero ns**, followed by **14,400,000 ns**; independently
reconstructed excess is **−0.2 ms**, with about **1,197.6 ms** actual GPU
recovery. The offered pause contains no simulation time; the following
frame resumes ordinary fixed-step accounting.
The raw failed and corrected observations are retained. This timing probe
is separate from deterministic fixed-input CPU replay.

The previous 141,838-byte HUD Wasm also runs the current SDK's actual full
route and explicit replacement successfully. Its absence of automatic-loss
exports selects the existing explicit path. The new 142,136-byte device
Wasm and the explicit older profile do not change published host-v1,
VPK/RCM1 or canonical-state identities. The final authority build is
byte-identical to the guarded binary used by these probes.

## Checks and preserved failures

Complete root mutation gates kill **135 mutants, zero survivors**: 127 for
loss/frame admission and eight for canonical device observation. The latter
includes the independently specified `b6924e…` true-bit wire vector at
ordinal zero. No equivalents or tolerance changes were introduced. The
unchanged core/serializer/HUD/component results remain those of their
preceding pinned commits. Tools-reason and book-registry results accompany
this record; chapter status does not change.

Earlier gaps and failures remain visible:

- Five initial loss-policy mutants exposed exact-deadline and reversed-poll
  coverage gaps; one observer mutant exposed the fixed ordinal. Exact
  boundary checks and the independent wire vector kill them.
- The first SDK build lacked explicit C exports; the compile failure is
  retained. Final compilation and actual Wasm execution exercise them.
- A new probe reproduced `SDK allowed a stopped paid frame` before frame
  admission moved into Coax. The corrected face denies the same call and
  still permits actual closing cleanup.
- An active-pause probe initially attempted its call before the actual loss
  callback arrived. It now waits for admitted recovery state; this was an
  observer ordering error, not evidence that an unobserved loss had already
  paused simulation.
- Early resumed traces captured final actor values before their last paid
  continuation. CPU replay refused them. The observer now records actor
  values after that step. Its short-trace negative checker now selects an
  actual four-step frame for interior-step corruption, retaining all six
  negative cases and the first-difference check.

## Provenance, scope and remaining work

Artifact root is `~/.openagents/aethyrforge/streaming-12990/2026-11-07/`.
Final histories are `browser-loss-MODE-complete` for all eight modes.
The exact Wasm, source/SDK dependencies, raw receipts, captures, sidecars,
CPU replay outputs and earlier failed/prototype artifacts are pinned in
the accompanying manifests. The live clock records are
`live-loss-{red,final}.json`; the old-carrier compatibility route is
`browser-loss-legacy-complete`. CPU reports remain beside the original immutable
`native-history-replay-v1-san/replay` executable.

Build with `coax/web/wasm.sh aethyr/world/stream-walk-device.coax
aethyr/host/tests/stream-walk-device-face.c NEW.wasm` and serve the existing
three WGSL files and original scene. Invoke `coax/web/tool.sh
aethyr/tools/stream-loss-browser.coax BASE SCENE_JSON NEW_OUTPUT MODE`;
the eight modes are listed in its usage guard. The live probe is
`aethyr/tools/stream-live-loss.coax BASE SCENE_JSON NEW_REPORT`.
Independently inspect receipts with `python3
aethyr/tests/streaming_loss_oracle.py OUTPUT --fault-checks`, or use
`--live REPORT` for the RAF record. All commands are bounded below the
repository limit: 60–180 s for probes/builds/oracles and 890 s for mutation.

This is scoped browser fixture adoption. The ordinary `coax/web/host.js`
still has its existing legacy recovery implementation and remains a named
residual adapter; no default host migration is claimed. Native automatic
loss and mobile/tier-1 automatic-loss hardware qualification remain open.
Existing native controlled replacement has its separate receipt. Persistent
failed-replacement rescue, additional interrupted/stale/superseded load and
allocator/SDK failure histories, controlled category pressure, complete
transport/transient/physical backend planning, independent world-shader
reference and the final consumer/residual-adapter audit still prevent
#12990 completion.
