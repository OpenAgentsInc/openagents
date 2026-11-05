# Interactive Metal host (#12868): qualification in progress

The opt-in `--world metal` AppKit host executes the shared Coax renderer and
Everglade producer. The desktop default remains WebKit. This record does not
close #12868 or qualify the three-host default change in #12869.

The durable raw observations are under
`~/.openagents/aethyrforge/metal-host-12868/`. Tests use a separate app bundle,
data directory and native window, with audio muted. The ordinary installed
Aethyrforge app is not replaced by these runs.

## Boundary

`MetalRealm.m` binds Metal objects, reads packages and images through SDKs,
copies admitted buffers, submits the Coax-authored pass plan and observes GPU
completion. The Coax geometry, bindings, package, ledger, surface, transport,
HTTP route and lifecycle authorities decide admission. The face uses native
`uintptr_t` addresses and the same game payloads as its Wasm build. Metal
contains no replacement gameplay, HUD or world simulation.

`MetalWorld.swift` presents a `CAMetalLayer` with `CADisplayLink`, translates
AppKit input and releases held input on blur and hiding. `MetalNetwork.swift`
executes URLSession effects after shared Coax slot/generation decisions.
`MetalAudio.swift` passes canonical decoded PCM and commands to the shared
Coax mixer; CoreAudio supplies the device callback. The native setup, status,
Coder and voice bridge remain desktop services around that world.

## Observed sections

| Section | Retained observation | Result and scope |
| --- | --- | --- |
| Core dispatch | `conformance-current/metal-core.json` | Common Coax checker accepts the actual fixture defaults, assets, two frames and input/message event words. |
| Actual AppKit event projection | `appkit-green/appkit.json` | Real NSEvents pass through the production ForgeMetalWorld view to the recording face: Control-A key 97/modifiers 2, precise wheel travel 32 and -8, nonprecise wheel travel 80 CSS pixels, Shift down/up and final blur/held zero. Option/Command flag-only events complete without accessing their unavailable character payload. The original wrong key scalar and wheel signs are retained in `appkit-red/`. |
| Socket/HTTP ownership | `conformance-current/metal-transport.json` | Actual loopback workload passes all 79 common rows, including slot exhaustion, UTF-8, overflow, cancellation and stale-owner fencing. |
| HTTP normalization | `conformance-http/metal-http-target.json` | All 30 physical peer targets pass the common checker. |
| Lifecycle | `conformance-current-hidden/metal-lifecycle.json` | Ten actual rows include a held key followed by hidden release. Negative, backward and hidden frame offers do not advance the producer. Final lifecycle is `[1,1,1,1,4015625]`, held input is zero. |
| Drawable affordability | `conformance-drawable-after/metal-lifecycle.json` | A CAMetalLayer SDK subclass returns no drawable. The host calls it once, leaves producer frames at zero and its clock at -1; the same ten lifecycle rows then pass. This check failed before moving drawable acquisition ahead of `htl_offer` and the producer. |
| GPU recovery | `replay-qualified-half/` | Same frozen inputs before/after resource recreation produce identical bytes; two frames complete, pending is zero, old and closed ledger live counts are zero. |
| Sanitized interpreter | `replay-asan/` and `asan-sdk/` | AddressSanitizer instruments the Objective-C interpreter, generated shared renderer/texture C and Swift effects driver. Frozen frame/recovery remain byte-identical, strict browser comparison has zero outliers, and API/GPU validation report no errors. The separately loaded producer image is not instrumented by this run. |
| Frame image | `frame-jpeg/` | Actual 320×180 BGRA readback encodes through the SDK; JPEG/base64 bytes round-trip through the shared Coax HTTP object attachment, preserving the other JSON fields. API and GPU validation are enabled. This run sends no provider request. |
| Canonical audio bank | `audio-canonical-bank/` | Actual current native producer WAV bytes and canonical decoded PCM are retained. The independent browser bridge compares all 48 assets bit-for-bit. |
| Rendered audio | `audio-fixtures-canonical/` | The actual browser AudioWorklet matches all eleven native ranges at the unchanged `2^-20` sample bound, without alignment or masks; nine ranges are bit-exact. See the [common audio record](../host-contract-2026-11-05/audio-canonical.md). |
| CoreAudio device | `audio-canonical-device/audio.json` | Actual muted device callbacks remain finite, hold during pause and resume; the authored room impulse uses 66 partitions. |
| Visible native window | `window-timing-60/` | Prior staged producer: 356 frames in 5.966595 s, 59.497921 FPS at an explicit 60 Hz cap on a 120 Hz display. Synthetic SDK key-call to GPU-completion p95 is 27.071 ms over 20 events; movement and resize are observed. Covered rendering/audio stop while native room messages continue near 30 Hz. Final ledger is zero. This software proxy excludes the physical keyboard/AppKit event queue and is not an optical measurement. |
| Display-rate native window | `window-current-120/` | Updated producer/SDK: 728 frames in 6.058315 s, 120.00036 FPS on a 120 Hz display at 1280×720 physical pixels (640×360 CSS, DPR 2). P95 interval is 8.34 ms; the same software input proxy is 14.547 ms over 20 events. Movement and resize pass. Covered frames/completions hold at 958, audio callbacks at 757, while room messages advance 280→310. Close releases the ledger. |
| Source-joined display-rate window | `window-qualified-current/` | Product pin `a0f3f8b518`: 719 frames in 5.98336 s, 119.999465 FPS at 120 Hz, 1280×720 physical pixels. P95 interval is 8.343 ms; SDK key-call to GPU-completion p95 is 14.735 ms over 20 events. Movement, resize and room continuity pass. Covered frames/completions hold at 948, audio callbacks at 753, while messages advance 281→311. Pending is zero while covered; close releases the ledger. The exact executable, loaded producer, SDK objects and shell sources are frozen before launch in `sdk/`. |
| Physical AppKit flows | `ui-*.json` | Actual Command-M minimizes: producer/completion counters hold at 1136 with pending zero while room messages advance 631→881. Raising restores drawing, and Command-R replaces the engine and reconnects (net 2). Native Coder accepts a Unicode draft, which is cleared without sending; status returns to ordinary play. The isolated bundle has no microphone permission, so this observation does not qualify capture. |

The earlier 60 Hz window timing uses an earlier staged producer. It is retained as evidence
of the host path, and is not a claim that final current-source packaging has
passed. Its raw JSON contains every observed frame and event clock.
The newer 120 Hz result is scoped to its measured surface; the later
1800×1200 resize is a functional check, not a claim of 120 FPS there.
The older `window-current-120` executable was overwritten by the subsequent
Window-menu fix before it was frozen; it has no exact binary join. The new
`window-qualified-current` run includes that fix and retains its exact binary.
The native display-link preference now starts from AppKit's actual display
maximum. A standard Window/Minimize menu consumes Command-M before it can
reach the realm's map key.

## Complete frozen GPU frame

The source-qualified browser capture and genuine historical CPU compiler
receipt are described in the [common frame record](../host-contract-2026-11-05/qualified-shared-frame.md).
These runs use that exact RFI, package, buffers, texture mip chains and media;
they do not rerun or synthesize the producer. The retained graphics compiler
is the separately qualified #12867 iteration image, not a newly released
compiler with a fabricated provenance receipt.

The unchanged channel rule is
`abs(value-reference) <= .002 * max(1,abs(reference))`. The inherited G5 live
frame allows 2,000 colour channels beyond that rule. Nonfinite values and
masked pixels remain zero-required. Supplemental strict comparator failures
are retained rather than reclassified as strict passes.

| Actual Metal target | Channels outside the rule | Maximum absolute error | Nonfinite/masked | Existing live gate |
| --- | ---: | ---: | ---: | --- |
| RGBA16Float | 0 / 230,400 | 0.001220703125 | 0 / 0 | Pass |
| BGRA8Unorm | 8 / 230,400 | approximately 1/255 | 0 / 0 | Pass; strict supplemental comparator fails |

Both runs issue 136 actual draw calls and release all ledger resources.
`replay-qualified-half/sdk/` and `replay-qualified-bgra8/sdk/` freeze their
respective SDK executables, source, generated C/headers and objects. Shared
runtime package/assets and producer image are frozen separately at
`replay-qualified-shared-resources/metal/`.

The same qualified browser input currently fails Vulkan's BGRA8/RGBA8 live
comparison with 7,020 colour channels outside the rule, exceeding 2,000.
Its half-float comparison has 53 and passes the existing allowance. The
common record retains both. This is unresolved three-host evidence; the
desktop default remains web.

`rgba8-float-diagnostic/` additionally renders the *same RGBA8 input RFI* into
half-float and applies the published `rr_unorm8` once after the full frame.
That differs from browser BGRA8 in 5,811 channels, Metal BGRA8 in 5,805 and
Vulkan RGBA8 in 3,223 (maximum 0.1294117645 for Vulkan). It changes current
per-pass rounding and is diagnostic only. No production target, shader,
comparison rule or allowance was changed by this experiment.

The disabled `presentation-experiment/` path consumes the shared Coax
`RenderPresentation` descriptor, draws the complete graph into RGBA16Float
and uses existing media entry 13 with the descriptor's nearest/clamp sampler
to render once into an actual BGRA8Unorm target. Against the matched new
browser actual display readback, 5/230,400 channels differ beyond the rule
(maximum one UNORM step), with zero nonfinite values or masks. It passes the
inherited live allowance and retains the strict failure. API and GPU
validation are enabled with no reported errors; pending and closed ledger
counts are zero. `sdk/` and `artifacts.sha256` retain the exact interpreter,
generated units and executable. This is a private experiment, with no
production target/default change.

## Actual producer and asset isolation

`cpu-current-capacity/` supplies the matched 90-frame workload to the native
and actual Wasm faces. All 128 scalar probe results agree; this includes
the probe's unused sentinel indices and does not imply every simulation
word is identical. The measured store/particle allocation capacities agree.
The tighter CPU byte diagnostic fails on draw transform S1 word 3184.
`state-fields.txt` maps arena differences through 211 actual compiler field
offsets: floats, heights, ground store, objects, sprite/draw storage and the
two native/Wasm pointer-bearing views differ. It does not waive a simulation
difference by classifying it as drawing.

With the same frozen texture/media bytes, the two actual producer handoffs
render through the same MF interpreter with 27/230,400 channels beyond the
unchanged rule, zero nonfinite/masked pixels, within the existing live
allowance. Using the actual native asset preparation instead gives 15,581
channels and fails; native-versus-native frozen asset isolation accounts
for 15,554. These actual failures remain in `handoff-current-capacity/`.
The existing native layer tool makes alpha-weighted box mipmaps, whereas
the historical browser asks its image SDK for high quality resizing. A
canonical Coax decode/mip API is published separately in
`aethyr/render/host-layers.coax`. The subsequent actual browser bridge and
GPU readback compare all 53,127,800 logical layer/mip bytes exactly against
the native bank. Those component observations do not erase the old asset
failures or enable the ordinary pre-v1 web path.

The matched new canonical browser capture is
`host-contract-12869/layers-bridge/canonical-frame/half/frame.config`.
`replay-canonical-half/` executes it through the real Metal interpreter:
zero of 230,400 channels exceed the unchanged rule, maximum absolute error
is 0.0013427734375, and nonfinite/masked counts are zero. Two frames complete
with byte-exact recovery, pending zero and old/closed ledger live counts zero.
API/GPU validation is enabled with no reported errors. Its exact SDK is
frozen with `artifacts.sha256`. The corresponding Vulkan half-float run has
27 outliers, within the inherited 2,000-channel live allowance.

`cpu-next/` runs the same 90-frame workload against the freshly emitted
native and Wasm producer at public product pin `a0f3f8b518`. Actual allocation
capacities and all 128 scalar probe outputs agree. The tight CPU diagnostic
still fails at S1 word 3184; `state-fields.txt` preserves the field-level
differences, including terrain and guide fields, without claiming complete
simulation byte identity.

`handoff-canonical-current/` substitutes the actual observed producer bytes
and measured capacities into the canonical RFI. Its complete comparisons are:

| Inputs through the same Metal interpreter | Channels outside the rule | Maximum absolute error |
| --- | ---: | ---: |
| Native versus Wasm producer, identical frozen assets | 27 | 0.1846923828125 |
| Actual native versus frozen canonical assets, identical native producer | 1,471 | 0.00885009765625 |
| Actual native producer/assets versus matched Wasm handoff | 1,498 | 0.1846923828125 |

All three have zero nonfinite values and masks and pass the inherited live
allowance; all three strict supplemental comparisons fail and are retained.
The canonical layer bank is identical. Remaining asset differences in this
run come from the SDK media decode/resize path. This result is scoped to the
matched 90-frame input, rather than every possible world or media asset.

## Build status

The earlier conditional native-v1/web-v0 literal-table shared release Wasm is 2,176,847 bytes, within the
existing 2,280,000-byte budget recorded by the room/party changes. An older
packed-table snapshot measured 2,377,513 bytes shared versus 2,253,731 bytes
producer-only: the shared renderer increment was 123,782 bytes and the old
producer baseline excess was separate. This work raises no Wasm budget.

Native generated C now travels through the existing raw-byte process API.
The old line reader refused 386,979-byte rig-table lines. The retained runner
check observes all 70,001 bytes of a long line including its terminal LF;
the current native producer emission and host C compilation then complete.

The next full packaging slice was stopped at twelve minutes while emitting
the local room on the loaded machine, before its fourteen-minute deadline.
The incomplete output is not admitted. A separately source-qualified room
artifact from the room-interest work was compared across all 235 Coax inputs,
its native SDK closure and pack, with no difference from `fae22e7101` at this
host snapshot. The reused binary is SHA-256
`04339e37ddc7147c093b2aa64c34a56c89fd5fb3fbe6b434f912c6b9388d6140`;
`room-reused-fae22/` retains the successful `build-sdk-fix.log`, 545-check
suite and two-socket transport log. Subsequent room/travel changes on main
are separate from this pin. This does not qualify the renderer.

The native primary's actual emitted C table names 137 Coax sources. The
source-observation tool hashes each original path and the corresponding
prepared product snapshot; all 137 matched at the successful emission's
product pin. The C SHA-256 is
`56b670afd85498c45a472710d92838cddffdead54b3247692dc3cd5371e3c447`.
The original admitted 598 image/receipt verifies against the genuine
historical `c224561457` compiler root before and after product preparation.
`qualified-current/` retains that verification, the product tar, successful
C/header/object and explicit source observation. The observation is not a
new compiler provenance receipt or release.

A separate fresh historical-root native emission hit its bounded deadline;
the deps traversal was also stopped without a result. Neither is claimed
as successful. At that stage the already successful C remained the native primary. The
web-v0 preview uses the existing content-addressed C cache with its exact
prepared product closure and the current conditional face, and retains
the actual generated units under `qualified-current/web-v0-emission/`.

After rebasing the host, `rebased-source-observation.json` finds five
subsequent game-source changes: raid tactics, network, wire, raid preview
and sound. The recorded producer/window runs remain explicitly pinned to
the earlier successful image; they are not current-main producer receipts.
Those observations motivated the fresh public product pin recorded below.

The earlier tools checks reported unrelated Nostr relay Python and human
asset tools missing a `Firm reason` header; those raw outputs are retained.
After rebasing the canonical browser bridge, the current required check
passes: 1,782 tracked tools, zero new tools without a reason.

## Fresh public product pin

`qualified-next/` prepares the genuine historical `c224561457` compiler root
against separately pinned product `a0f3f8b51848293eb7ffd8d7288dbb0c53d4db93`.
The original 598 image/helper/receipt verify before and after preparation.
The actual native primary C table has 137 sources; the shared Wasm table
has 183. Every observed source hash matches its pinned product snapshot.
Relative source rows are resolved from the recorded emission root. These
source observations are not new compiler provenance receipts.

| Artifact | SHA-256 |
| --- | --- |
| Native primary C | `e1d158f7cdaca9c53252f8bd38d954ccaa3ce9137f4fde4582266e4a64a8f438` |
| Native producer library | `416861a3d6092b1f51b5a21ffb89e1c61d70b4494d32c80bb9e293f320743a1e` |
| Shared Wasm primary C | `1c6b9983d432771ea0b2bdbf14f9b03f4a58221add9ed39f1fbc82b0bcb14836` |
| Shared release Wasm | `9cb26215e7ce3fbe3df9b4e2071068c6ebaec4d27891690110e1bace64f21781` |

The fresh primary native emission and C compilation complete within their
bounded slice. Rules are rebuilt for this pin; unchanged Step, Grid, River
and Guide side units retain their exact source-closure comparisons and
objects. The shared release Wasm completes in 415 seconds (405 Coax, four
clang, six link/optimization), measuring 2,131,313 bytes against the unchanged
2,280,000-byte budget. The conditional face keeps native v1 and web v0.

The staged app now contains that native producer and web preview, plus the
actual canonical browser image reactor. The desktop build tool builds and
copies `build/host-layers.wasm` alongside the audio reactor. The room remains
the separately qualified `fae22e7101` pin above; this is not a current-room
packaging claim. The frozen SDK generated C remains its recorded earlier
renderer snapshot, with exact source/object hashes, rather than subsequent
shared resource-limit refactoring on main.

Ordinary Vulkan 8-bit parity, the complete common-host services and final
packaging remain open. The presentation experiment stays disabled and WK
stays the default. Neither #12868 nor #12869 is closed by this checkpoint.

## AppKit input and optional quantized presentation

The AppKit regression constructs actual NSEvents and calls the production
view's methods, rather than calling the scalar face directly. Control keys
use the printable value with Control removed; flag-only events never read
`characters`. AppKit's upward-positive travel is converted in the standalone
Coax `appkit-input.coax` authority. Precise deltas use pixel units; ordinary
Cocoa ticks use forty CSS pixels. This follows the native Mac projection in
[WebKit's event factory](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/mac/PlatformEventFactoryMac.mm),
[DOM wheel conversion](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/dom/WheelEvent.cpp)
and [line-step definition](https://github.com/WebKit/WebKit/blob/main/Source/WebCore/platform/Scrollbar.h).
Its two checks precede implementation; the root mutation gate kills 1/1
mutant with no equivalents. The actual event regression passes with no
dropped events or held keys. The separate input object is linked into the
desktop shell; the producer and its already passed GPU/audio gates are unchanged.

`presentation-probe/` retains a separate private shader package and a source
joined 76-input facade emitted by the admitted 598 image in a genuine c224
root with pinned 6f product sources and the frozen descriptor overlay. The
original compiler receipt verifies before and after. New media MSL comes
from the explicitly identified c056 graphics iteration image, not a newly
released compiler. The SDK accepts this MSL with safe/precise arithmetic.

The independent fixture covers every finite binary16 encoding across RGB
permutations (nonfinite encodings are supplied as zero). Actual Metal BGRA8
and RGBA8 targets each match all 262,144 oracle bytes exactly. The BGRA8 bytes
also exactly match the real browser canvas readback; RGBA8 exactly matches
Vulkan. Producer frames remain zero, one presentation completes and the
closed ledger is zero. API/GPU validation is enabled with no reported errors.
The legacy-descriptor Metal control also matches this fixture; no claim is
made that the new mode changes Apple's old conversion on these inputs.

`finite-probe-sdk/`, the immutable input files and manifests retain the exact
probe binary/source/object join. The `metal-build manifest` task hashes an
existing product set without recompiling unchanged shaders; its unchanged
package output is byte-identical to the earlier manifest.

`presentation-v1-full/` uses the new actual browser RFI, full canonical bank
and versioned quantized descriptor. It preserves both the complete FP16
graph readback and the actual presented BGRA8 output. Four of 230,400 channels
exceed the unchanged per-channel rule, maximum one UNORM step, with zero
nonfinite values or masks. Both outputs remain byte-identical after GPU
resource recovery; two submissions complete, pending is zero, and old/closed
ledger counts are zero. Validation is enabled with no reported errors.
The matched Vulkan output has 1,920 outliers against both browser and Metal.
All pass the inherited 2,000-channel live allowance, and all strict failures
are retained. This close-to-bound result is scoped to that exact RFI.

The current native producer's actual 48 WAVs and canonical decoded PCM are
also re-exported at `audio-qualified-current/`; their complete file contents
equal the previously qualified bank, so unchanged audio ranges are not rerun.
Actual native producer/assets through the new display policy fail their
separate composition check: `handoff-presentation-v1/actual-native-vs-wasm.json`
records 2,213 channels outside the unchanged rule, above the 2,000 allowance,
with zero nonfinite values or masks. The exact native producer, actual asset
path and matched Wasm handoff are retained, without adding outlier counts.
That receipt retains the caught blocker. The new `host-image.coax` canonical
PNG/JPEG base decoder kills all 47 root mutants. Its subsequent native asset
migration and matched composition observations are recorded below.
The policy and shader stay private/disabled; ordinary target selection and
the desktop default remain unchanged.

## Canonical native media boundary

`media-canonical/red/` reproduces 3,830 differing bytes out of 64,512 actual
GPU bytes across all seven authored 48×48 hotbar JPEGs when native uses
ImageIO. `media-canonical/green/` records zero differing bytes after v1 native
uploads use `htm_plan`/`htm_decode` from `host-image.coax`. It retains each
original compressed file, same-index `media.list`/`media.urls`, canonical
expected RGBA and actual GPU readback. Every byte also matches after resource
recovery. Three frames complete, pending is zero and the closed ledger is
zero. API/GPU validation is explicitly enabled with no reported errors.
Pre-contract faces keep their previous ImageIO path; the canonical path has
no platform-decoder fallback after a decode refusal.

The new `RenderTexture` facade was emitted with admitted 598 from the genuine
c224 compiler root plus separately pinned dbd product inputs and a retained
one-line `native-texture.coax` load overlay. Original provenance verification
passes before preparation, after preparation and after emission. All 23
emitted source rows match the retained snapshot plus that explicit overlay.
`media-canonical/sdk/` and `sdk-artifacts.sha256` freeze the actual interpreter,
generated C/header/object, executable and loaded producer join. This does not
promote the private iteration graphics products or publish a compiler release.

`cpu-media-v1/` captures the same actual native 90-frame producer with the
canonical layers/media and private versioned display policy. Its uniforms
and 128 scalar fields are byte-identical to the earlier native capture.
Against the existing matched Wasm-to-Metal frozen handoff, 27 of 230,400
presented channels exceed the original `.002 * max(1,abs(reference))` rule,
with zero nonfinite values and no mask. This is within the unchanged 2,000
live allowance; the strict comparator failure remains retained. The earlier
2,213-channel failure is preserved.

The new production-browser canonical-media RFI replays through Metal with
four outliers, zero nonfinite values or masks, and exact graph/display bytes
after recovery (`media-canonical/replay-browser/`). The complete native90 RFI
under `handoff-media-v1/native/` also replays byte-identically to the actual
native graph and display outputs. Direct actual native90 display versus
Vulkan rendering the matched Wasm90 RFI has 1,723 outliers; using the same
native RFI on Vulkan has 1,697. Both have zero nonfinite values or masks and
pass the original 2,000 allowance, with validation clean and owned cleanup
observed. The common records are in `host-contract-12869/media-bridge/`.
These are actual composed comparisons, without adding component counts.

`aethyr/host/media-notice.coax` owns the existing607 ready/failed row formatter
(131/131 root mutants killed). It atomically emits exact ready dimensions,
failed zero dimensions and no pending rows. Native dispatch occurs after
the producer and frame encoder return, and identical notices are not
repeated. Actual malformed metadata, missing file and late PNG decode
failure each leave that image failed/not-ready with zero dimensions while
the other six images remain ready at 48×48. Each workload completes six
world frames across recovery, delivers the complete expected notice and
closes with zero pending/old/closed resources. API/GPU validation is enabled
with no reported errors. Raw receipts are `media-canonical/refusal-{metadata,
missing,late}/`; the source/executable join is `notice-sdk/`.

ASan instruments the Objective-C interpreter, generated renderer and new
decoder/notice facade plus Swift driver. Its actual seven-image GPU/recovery
and late-failure workloads pass, without ASan reports; leak detection is
disabled and the loaded producer library is not instrumented. These are
`notice-asan-bank/` and `notice-asan-late/`. A fresh native90 run with notice
dispatch preserves the prior uniforms, FP16 graph and actual UNORM display
bytes exactly (`cpu-media-notice-v1/`), so the direct 1,723/1,697 composed
receipts remain applicable without rerunning unchanged Vulkan work.

Desktop packaging builds/stages `build/host-media.wasm` alongside the audio
and layer reactors. The new display shader products remain experimental
under the frozen iteration backend. These component receipts do not replace
compiler build/load admission or qualify a current compiler release; ordinary
presentation selection and the desktop default remain unchanged.

## Updated isolated bundle smoke

`window-media-smoke/` retains a caught Metal validation assertion in the
desktop transparent HUD: Coax correctly emitted zero vertices, but the SDK
requested a zero-length buffer. `ForgeHUD.draw` now encodes the clear/present
without a buffer or draw for that empty stream. No gameplay or realm HUD
geometry changed. The room/serve children orphaned by that assertion were
terminated by their exact isolated-bundle PIDs.

`window-media-smoke-fixed/` records the newly linked/signed isolated app with
the canonical decoder/notice SDK and staged 51,577-byte media reactor. Under
explicit API/GPU validation it renders, joins the local room, pauses behind
status, resumes and closes with zero live ledger objects/words. No validation
error is reported. The initial reducer incorrectly expected seat1; the actual
engine reports a valid seat0 (as in the earlier window record). The corrected
reducer requires an admitted nonnegative seat and reuses the same actual
receipt, without relaunching the passing app workload. This loaded-machine
startup/cover/resume check is not a new FPS qualification. The earlier display
rate/latency numbers remain scoped to their frozen binary and surface.

The exact updated executable and shell/renderer inputs are frozen under its
`sdk/`. The media reactor SHA256 is
`8a044816e38ac651fa9c29af9dfdcba5218dbbb637a3daa98b4261d3d20a6998`.
The canonical app was untouched; the isolated app and owned children are closed.

## Native audio allocation and bank-span admission

`audio-memory/receipt.json` and `audio-memory-asan/receipt.json` are identical
actual native observations. The Swift PCM owner reserves through
`ham_extent`/`ham_reserve` before allocating samples. Converter input and
output buffers hold separate reservations through conversion. A ticket stays
alive with its allocation, including assets retained across context replacement;
the engine stops borrowing before its decoded buffers are released.

The workload observes invalid/overflowing extents refusing, the complete
8,388,608-word budget refusing another sample, all 128 tickets filling and a
released ticket being reused. An actual interleaved SDK buffer copies all six
samples correctly and returns its reservation on release. Bank adapter calls
observe duplicate, malformed and out-of-allocation rows refusing with empty
returned pointers before any source copy. The first and final bytes of admitted
physical storage remain accessible. Its exact observation authority kills all
96 root mutants, including the earlier 63-mutant allocator/span component,
with no equivalents. The shared extent authority kills all
64 root mutants; the existing bank authority's 92-mutant gate is cited rather
than repeated.

Both executions start no GPU or audio device. ASan instruments the Objective-C
adapter, generated renderer/texture facade and Swift allocator fixture; leak
detection is disabled. The loaded producer in the bank workload is not ASan
instrumented.

`audio-bank-memory/` additionally observes all 48 real synthesized assets
through the new physical exports and native adapter. Its entire 10,193,456 WAV
and PCM bytes equal the independently frozen canonical bank. The 2,042,394
decoded words stay reserved while references survive context close, then drop
to zero. Filling the word budget first still admits the 48 physical WAV spans
but refuses every PCM allocation; a known unavailable cue stays unavailable.
A separate optional SDK converter returns 24,960 samples, releasing temporary
reservations and finally its retained allocation to zero. Its resampled PCM
does not claim canonical sample parity.

The producer is the exact previously qualified `a0f3f8b518` face and matching
generated headers/objects, relinked with only three additive physical
capacity/storage exports. Those source bytes and the narrow patch are retained
under `audio-bank-producer/`. This is explicitly a frozen producer plus new SDK
qualification. Sibling work's fresh current-source primary emission reached its
840-second child deadline with status137/timeouttrue and no admitted C/header;
that failure remains separate and is not replaced by these older source inputs.

The original admitted598 compiler emits the facade from genuine historical
`c224561457` plus pinned product `56117fe1aa` and the two explicit bank/memory
loads. Its original receipt verifies before and after preparation and after
the overlay. `audio-memory-sdk/compiled-source-observation.json` records all
78 actual emitted source inputs matching the separately checked working
snapshot, with zero differences. The C SHA256 is
`92b7c5bc1b0e03533846f102b8f98a6c6384553537e2504b8436caf99fe539ec`.
This source join does not publish a compiler release or admit experimental
shader products. The receive queue's 106-mutant authority is a separate landed
component; ordinary native receive wiring remains an outstanding SDK gate.

## Bounded raw socket SDK component

`socket-pressure/` and `socket-pressure-asan/` exercise the new native raw
socket effects directly, separately from ordinary realm wiring. The existing
Coax RFC6455/UTF-8/fragment reader runs on four persistent SDK workers. Each
logical slot retains at most one current and one replacement job. A concrete
engine context stays alive with its actual jobs/events, so repeated transport
IDs cannot redirect an older callback to a replacement context. Main-thread
close returns without joining connecting workers.

The workload stalls delivery for1.7seconds. Both observations retain exactly
six leased events and512 payload bytes: the open, one empty oversized-frame
error, and four complete128-byte text frames. A binary frame is drained without
delivery. The worker waits before another callback copy/read. The normal
physical peer reaches its100ms flush timeout after4,163 complete frames;
the instrumented run reaches101ms after4,152. These observations establish
actual peer backpressure, without attributing application counters to SDK or
kernel buffers.

While the reader is blocked,64 empty outgoing commands fill their shared
tickets. The next refuses and causes the Coax-selected error/close. Complete
received text stays byte-exact and in FIFO order (13 normal/8 instrumented
frames before that explicit close). The test then replaces1,000 engine
contexts using the same transport owner numbers. Normal maxima are three
contexts/two jobs; all four replacement workers open and echo correctly.
An old held event survives context close, then releases only its own owner.
Final contexts/jobs/workers/message-storage counters are all zero after
process teardown. A small fixed per-slot terminal table retains only control
observations when no payload allocation is available.

The shared queue/control authority kills35 root mutants; pressure payload and
semantic receipt authorities kill26 and125, with no equivalents. ASan covers
the C worker/queue, all generated Coax networking/entropy code, Apple socket/TLS
and entropy effects, and the Swift probe; leak detection is disabled. The test
uses private loopback ports9170/9171, starts no GPU/audio device, and terminates
its owned peers. TLS trust behavior is unchanged and its prior gate is cited,
not repeated by this plaintext fixture.

`socket-sdk/compiled-source-observation.json` joins all16 actual emitted Coax
inputs with zero differences. They use the original admitted598 CPU image and
unchanged protected historical compiler root; the separately pinned product
snapshot has only the recorded native-socket/socket-queue overlays. C SHA256:
`691fcdf953527a6e50151d28a686cbd6401afa3408319ef7f836819449409025`.
The SDK sources, objects, executable and hashes are retained alongside it.
The ordinary adapter's subsequent qualification is recorded below.

## Ordinary bounded socket adapter

The native realm now loads its packaged `libnetwork.dylib` and dispatches
socket imports directly through the bounded SDK. The library and four idle
workers are process-owned; concrete contexts retire independently and retain
their jobs/events through their last references. A main-run-loop timer polls
the same Coax drain budget independently of rendering. The SDK never queues
a payload-bearing main-thread closure. Shared transport ownership is checked
again at actual delivery, then its exact incoming lease is released.

`socket-wired-transport/` and `socket-wired-transport-asan/` both pass the
common79-row transport checker using the real MF/Swift adapter and unchanged
recording producer. ASan instruments Objective-C, Swift, generated renderer/
texture/networking C and Apple socket/TLS effects; the recording producer
dylib is uninstrumented and leak detection is disabled. The earlier raw
pressure gate remains the physical-backpressure/storage evidence and was not
repeated. HTTP still uses the existing URLSession adapter.

`socket-wired-window-second/` exercises the newly linked muted isolated app
and its actual local room with API/GPU validation. While status covers the
world, producer/completion counts remain fixed and room callbacks increase;
returning to play advances rendering, with a valid local seat and zero live
ledger objects/words on close. This is a continuity gate, not a new FPS or
latency measurement. The initial `socket-wired-window/` copied an incomplete
staging stub and trapped on the missing `Zeron.metal` resource before world
startup; its crash is retained. Correcting the staged resources required no
product-code change. The canonical app was untouched and owned children exit.

`socket-wired-sdk/artifacts.sha256` joins the exact source, C/header, objects,
normal/instrumented executables, packaged library and isolated app. The two
previous archived objects accidentally overwritten during staging were
rebuilt from their original source paths and verified against every original
manifest hash before new output moved into this separate directory.
Generated CPU-source joins remain the earlier78-input renderer and16-input
network receipts. This checkpoint does not qualify remote authenticated
upgrade headers/cookies or claim SDK/kernel buffers share application bounds.
The experimental presentation/compiler/default gates remain unchanged.

## Draining sound packet capacity

The MF reader now requires the producer's additive
`coaxial_sounds_capacity` observation and calls the unchanged shared
`htb_copy_span` authority before publishing its pointer/length to Swift.
Refusal returns no pointer and zero bytes. Reading the logical length still
drains exactly once. No packet parser or mixer arithmetic changes.

`audio-packet-red/` catches the prior reader returning lengths5 and
4,294,967,295 against a four-byte allocation, a logical length with a null
address, and a wrapping complete allocation. `audio-packet-final/` and
`audio-packet-final-asan/` pass the actual eight-case fault workload, including
valid empty/full packets and recovery after every refusal. The observer
authority kills68 root mutants without equivalents; an earlier mutation run
caught missing exact-capacity/prefix cases, which are now explicit checks.

`audio-packet-production/` and `audio-packet-production-asan/` observe the
real frozen producer's443-byte startup packet, preserve every byte including
`!room room`, fetch/cue/bus/bed commands, and observe an empty second drain.
`audio-packet-producer/` retains the exact matching `a0f3f8b518` face and unit
objects with only the earlier three bank observations plus this fourth
additive `sizeof` export. No new primary emission or header substitution is
used. The first test-face compile exposed its old header's missing optional
browser lifecycle declarations; the native-only fixture explicitly excludes
that unused browser wrapper. The caught compile log is retained.

`audio-packet-sdk/artifacts.sha256` joins the final reader/fixture/probe source,
objects, generated inputs and normal/instrumented programs. ASan covers the
reader, Swift probe and generated renderer/texture C; the frozen primary and
fixture Coax object are uninstrumented, and leak detection is disabled. These
checks open no GPU or audio device. Existing DSP/sample gates are unchanged
and were not repeated.
