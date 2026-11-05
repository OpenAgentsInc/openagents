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

## Observed Origin and target-selected cookies

The raw socket SDK now consumes `relay_conn_open_context` and the shared
`ws_context.coax` writer. The world supplies its explicitly bound Origin;
the actual ephemeral URLSession cookie jar selects cookies for the concrete
target, using HTTP for `ws` and HTTPS for `wss`. Coax first admits measured
UTF-8 extents, then byte values, before the worker retains copies. Each job
owns its URI/Origin/Cookie bytes until final retirement, independently of
later host configuration or cookie changes. Empty context retains the old
relay request bytes. There is no TLS trust or redirect change.

`socket-context/` and `socket-context-asan/` observe the actual physical
loopback upgrade bytes and26 delivered recording-realm rows. Before seeding,
the peer refuses the upgrade. A real HTTP response sets an HttpOnly auth
cookie, a different-path cookie, a Secure cookie and an expired cookie. The
`/auth` upgrade includes only `auth=one`; `/other` includes only `other=two`;
the target spelled `localhost` receives no `127.0.0.1` cookies. The world
Origin stays explicit, including on that different target spelling. Changing
the world configuration's Origin makes the peer refuse despite the target's
correct cookie. These are synthetic fixture credentials, with no production
access. The unchanged TLS gate remains separate; this plaintext workload
does not establish a fresh authenticated remote TLS session.

The field authority kills77 root mutants and the actual receipt authority
kills82, with zero equivalents. An initial mutation run exposed a shared
input/expectation constructor; the reducer now tests independently authored
input rows from a separate fixture file. Actual SDK/peer records are separate
from both. `socket-context-transport/` and its ASan counterpart pass the common
79-row regression. Because job storage changed, `socket-context-pressure-asan/`
also repeats the changed ownership path:512 held payload bytes,64-command
overflow,27 FIFO frames,1,000 replacements and zero final owners after the
physical peer reports flush backpressure.

ASan instruments the raw SDK/platform effects, MF reader, generated renderer/
texture C and Swift driver. The recording producer remains uninstrumented;
leak detection is disabled. `socket-context-sdk/compiled-source-observation.json`
joins all17 actual emitted inputs with zero differences. The original598
receipt verifies before/after its genuine historical root plus the recorded
product overlays. The frozen C/header, SDK sources, objects and programs are
joined by `socket-context-sdk/artifacts.sha256`. No compiler release or
experimental graphics admission is inferred from these CPU/service gates.

`socket-context-window/` observes the same newly linked/signed isolated app
joining its actual local room with API/GPU validation. While covered, frames
and completions stay7/7, pending reaches0 and callbacks rise51→83. Returning
to play reaches12 frames, then closes with ledger0/0. This includes the
updated sound packet reader and its additive physical face export. It is a
continuity check under load, not a replacement for the frozen display-rate
measurement. The canonical app and production services remain untouched.

## Shared audio-session ownership

The CoreAudio source now consumes `audio-session.coax` under the same lock
as commands and PCM blocks. Gesture, cover/uncover and terminal retirement
come from the shared Coax offer. Inactive or retired callbacks clear the
actual supplied device planes before returning and do not advance PCM.
Commands capture gesture state when offered. Retirement detaches the device
node before freeing PCM; retained callbacks keep their session allocation
until their last source reference. Canonical decode and mixer arithmetic
are unchanged. A refused bank entry reports failure while independently
admitted assets remain usable.

The initial actual pre-gesture callback in `audio-session-red/` advanced
1,024 Coax frames and17 device frames. The corrected callback holds both at
zero and clears a deliberately nonzero supplied buffer. The full real-device
trace in `audio-session-product-second/` and `audio-session-product-asan/`
passes the same32-word `hpa_accept` authority used by Linux. Its published
93-mutant gate and the session's33-mutant gate are unchanged from
`e2c4a49a34`; neither is repeated here. The actual receipts are copied as
[normal](audio-session.json) and [instrumented](audio-session-asan.json).
They cover controls before gesture, refused stale cue, gesture while covered,
exact pause holds, resume, exactly-once drains, terminal callbacks, malformed
metadata, one refused source with another usable source, and three hostile
packet spans while unrelated producer frames continue.

The retained first full trace, `audio-session-product/`, caught missing
partial-bank failure status and a harness observation taken after only one
512-frame CoreAudio callback. That callback consumed the remaining half of
an existing1,024-frame Coax block. The corrected harness waits for both an
actual callback and block-clock advancement; it does not alter audio pacing
or weaken the common receipt. Normal and instrumented final traces observe
pause at1,024 Coax frames/one callback and resume at2,048/three callbacks.
Device-buffer nonzero samples remain zero. ASan instruments the MF reader,
Swift device adapter, generated renderer/texture C and recording producer;
leak detection is disabled on this Mac. An initial sanitizer staging attempt
omitted `RenderTexture.c`; its build failure is retained, then corrected by
copying the already frozen source without repeating the successful renderer
compilation.

`audio-session-window/` opens the newly linked isolated ordinary app, actual
local room and muted CoreAudio device with API/GPU validation enabled.
Cover holds renderer frames/completions at8/8 and audio at49,152 Coax frames/
96 callbacks while room messages increase34→64. Returning to play reaches
14 renderer frames and145 audio callbacks; the final Coax block clock is
74,752, with74,240 consumed device frames. The retained half-block explains
that difference. Actual device planes remain zero, terminal audio is
inactive/retired, and closed ledger objects/words are0/0. This is a continuity
gate on the frozen producer/room, not a new display-rate or current-room
qualification. The canonical app is untouched and owned children exit.

`audio-session-sdk/artifacts.sha256` joins SDK/fixture sources, generated C/
headers, objects, both test executables and the signed isolated app. Actual
emission source tables join79 renderer inputs and14 fixture inputs with zero
differences. The original598 receipt verifies before and after emission in
the genuine historical root plus separately recorded product overlays. The
fixture includes the public pointer-width-aware hostile-span export from
`3a4ef4a0b8`. Existing sample gates were not repeated. These CPU/device checks
do not admit the experimental presentation product, qualify current primary
packaging, close either issue, or change the desktop default.

## Local TLS opening composition

`socket-tls/` and `socket-tls-asan/` exercise the same raw worker and MF/Swift
adapter over actual Network.framework TLS. A real ephemeral cookie jar is
seeded through its SDK with a plain cookie, Secure cookie and different-path
cookie. Both trusted `/auth` openings physically carry only
`plain=one; secure=two` and the explicit world Origin
`https://bound.invalid`; the actual negotiated version isTLS1.3. An untrusted
self-signed certificate and a trusted certificate for the wrong hostname
produce the exact deferred error/close rows. Those two peers observe no
application header bytes. A later trusted opening succeeds, so refusal does
not poison the next socket owner. The full18-row callback receipt and two
physical openings are copied as [client](socket-tls.json) and
[peer](socket-tls-peer.json).

The admission/observation authority kills68 root mutants without equivalent
annotations. Literal reference input rows are separate from the expected
row constructor. The peer is an independent Python `ssl` wire oracle; Coax
owns process/certificate orchestration and the exact receipt gate. It runs
only on loopback, ends within120 seconds and retains raw observations.
Private generated keys are deleted after both runs; public fixture
certificates/configurations remain for the record. No Keychain or system
trust-store entry is added.

The explicit `socket-test-trust` build task recompiles only the TLS effect
with the pre-existing `CXTLS_TEST_TRUST` fixture hook, then relinks a separate
SDK directory. Its sole anchor is the generated fixture certificate.
Ordinary `socket-library` never enters that task; production trust code and
the packaged app are unchanged. ASan instruments the portable worker,
platform effects, generated network/renderer/texture C, MF reader and Swift
driver. The recording fixture is uninstrumented and leak detection is
disabled. This is synthetic local credential/TLS composition, not a claim
about a production authenticated session. The unchanged17-source networking
emission join is reused from `cbf057b592`; no compiler or GPU gate is repeated.
`socket-tls-sdk/artifacts.sha256` joins both exact executables/libraries,
sources and public fixture identities. This check opens no GPU or audio
device and changes no host default or graphics admission.

## Room371403 integration and visible-window precondition

The [actual window receipt](room-371403-window.json) joins the existing
frozen native producer/SDK with the separately qualified room at
`3714032827`. Its binary SHA256 is
`682c794700a6db58fc5be2414d22a80b4067baa8be05277b2dd80e98de8813ab`;
the room's own six-unit manifest and explicit older `cdace` compiler profile
are in [its source record](../room-deaths-2026-11-05/sources.txt).
That room/compiler receipt is not renderer/compiler admission. Subsequent
room/session changes on main are outside this integration pin.

`room-371403-diagnostic-window/` observes a real visible, unhidden, active
AppKit window on the active Space, with API/GPU validation enabled. The room
connects as net2/seat0. Coverage holds renderer frames/completions at9/9 and
audio at44,032 Coax frames/85 callbacks while messages increase34→64.
Returning to play reaches16 renderer frames,134 audio callbacks and68,608
audio frames. Device-buffer nonzero samples remain zero; terminal audio is
retired/inactive and closed ledger objects/words are0/0. All owned children
exit. `room-371403-sdk/artifacts.sha256` freezes shell sources, linked
objects, executable, loaded libraries, room identity and actual plan/result.

The preceding `room-371403-window/` and `room-371403-raised-window/` are
unqualified observations: AppKit reported not-visible, and producer/audio
correctly stayed paused. The test plan now explicitly raises/unhides its
actual window and requires visibility. Additional raw observations split
window visibility, occlusion, active Space and app activation. The final run
observed all visibility facts true; no product visibility rule was bypassed,
and no cause is assigned to the earlier external occlusion. The display was
observed awake and the console session unlocked. This is a scoped room/
device continuity check, not a new display-rate measurement or complete
current-main packaging qualification.

## Remaining complete-host gates

Current producer/face packaging still needs a successful bounded emission
under an admitted CPU compiler and an exact source/object join. Two sibling
current5E attempts hit their840-second bound without C/header output;
unchanged attempts are not repeated. The earlier137-input primary closure
differs from the `b6e12743cd` snapshot in15 product files; its frozen image
cannot be relabeled current.

The quantized FP16 presentation matrix remains experimental under its exact
frozen graphics iteration backend. No admitted current graphics compiler or
replacement build/load receipt has been published here. Existing shader
admission is preserved. Complete qualification needs the admitted product,
matching producer/assets and real display-rate/per-pass evidence joined to
one packaged snapshot. The passing frozen comparisons and earlier display
timing remain separately scoped. Shared browser/Vulkan service work and
HTTP redirect alignment continue under#12869; neither issue nor the desktop
default is changed by these component checkpoints.

The desktop builder also stamps the separate `services-carrier.coax` closure,
`services-face.c`, `build-services.coax`, Wasm wrapper and compiler, then
packages `build/host-services.wasm` beside the audio/layer/media reactors.
The actual dependency observation is retained in `services-desktop-deps.txt`.
The source-qualified18,883-byte reactor from `49d2173a9e` copies unchanged to
the isolated resource path in `services-desktop-bundle/`; its SHA256 remains
`f8d87f8a86f4deb7a71e1d122ac69f020323952fe55c25dba0722bac68bdb27d`.
The sibling's original598/c224 receipt and12-input exact source join qualify
that reactor. Coax checks the updated desktop build source. This leaf build/
copy hook does not claim a successful complete current desktop build or
repeat the already passing browser service observations.

The subsequent queue reactor at `6bc5ec9ab2` is 19,919 bytes, SHA256
`71834393855de2a1f5ff38a6c7eeda855ed1d74218fb86f514a7d01e428fc56c`.
It is copied unchanged to the separate scoped resource fixture
`services-queue-desktop-bundle/`, preserving the earlier artifact.
`services-queue-desktop-deps.txt` observes all 13 source paths, including
`aethyr/host/inbox-queue.coax`. The desktop stamp already follows the full
carrier load graph through `wasm-size sources`; it automatically includes
this new dependency without maintaining a second manual list. The sibling's
actual generated-source join is 13/13 exact. This is a resource-copy check,
not a runnable current-desktop or new browser conformance claim.

## Ordinary default audio scope

The 48 canonical WAV assets qualify the synthesized set and room impulse.
Ordinary Everglade selects `egs_mixed` unless its URL requests `?sfx=synth`.
Its `SFXB_PICK` variants emit `NAME-CAND` cues and fetches, backed by the
Opus/Ogg files described in [the authored sound record](../../../aethyr/audio/SOURCES.md).
The native adapter's SDK fallback therefore serves ordinary v1 audio.
It is preserved; restricting v1 to the 48 exported WAV names would omit
default selected sounds.

Actual AVAudioFile metadata observations in `audio-default-ogg-observation.txt`
read `fire-cast-e2.ogg` as 32,652 mono frames, `fire-hit-e2.ogg` as 42,240 mono
frames and `ambient-e1.ogg` as 960,000 stereo frames, all at 48 kHz. These reads
open no audio device and establish no decoded-sample parity. `voice.js`
replies use their separate Int16 PCM stream, rather than this asset codec.
Default compressed metadata/codec qualification and allocation-pressure
behavior remain distinct from the successful 48-bank sample comparisons.

The CPU primary blocker is specifically the missing bounded current 5E
emission, not lack of a usable admitted CPU image. Original598 remains
admitted against genuine `c224561457` compiler inputs plus separately pinned
pure product inputs. `metal-build` emits the native primary with `--library
--run-steps 128000 --header everglade5e.h` and the existing default aggregate/
string limits, then compiles natively with `-O2 -ffp-contract=off
-fno-fast-math`. The two 840-second failures publish no successful C/header
or emission profile. Existing `wasm.sh` caches C/headers by compiler SHA,
unit/header-base/options and every loaded source hash. A producer-only
cache with the exact admitted image/profile and frozen current source join
could feed `metal-build generated --generated` and `producer
--producer-object` without repeating emission. No eligible current entry
has been established here; source paths alone do not prove that join.
Shared-carrier C and the parent's classic room C are not substitutes for
the matching 5E producer closure. Graphics admission remains a separate gate.
