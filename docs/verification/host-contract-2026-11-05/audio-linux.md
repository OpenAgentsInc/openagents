# Linux audio SDK component

The actual Linux SDK decodes all 48 frozen producer bank WAVs to byte-identical
canonical PCM. Eleven rendered ranges pass the unchanged absolute 2^-20
sample gate, with no alignment correction, masks or nonfinite samples.
Nine ranges are exact; maximum room error is 2.37064e-19 and loud error is
9.54662e-24. These results use the same complete immutable inputs as the
browser/native canonical records. They establish Linux decode/render parity
for those inputs, not current Linux producer synthesis or an ordinary world
window. `linux-audio/offline.json` retains every bank/range observation.

The actual allocator uses shared `ham_extent`/reservation admission before
allocating. It refuses invalid/overflowing extents, another sample after the
32 MiB budget, and a 129th ticket. Releasing and reusing a slot preserves its
measured total. A physical six-sample SDK copy retains its exact values.
The actual bank wrapper reads only admitted pointers: duplicate, before-base,
past-end, oversized, signed/zero-length, invalid-name and extra-word rows
return empty views and preserve the first row. The last exact source byte
remains admitted. The shared 20-memory/50-bank observer is the unchanged
96-mutant authority at `22bd2c3a5b`; it is not rerun for this adapter.

PCM references keep their exact six-word allocation through context-owner
release and one retained-reference release; the last release returns the
budget to zero. That separate typed observer kills 11/11 mutants. The
allocator/span/retention workload passes normally and under ASan/UBSan with
LeakSanitizer enabled. No suppressions or equivalent annotations are used.

The real muted PipeWire stream negotiates 48 kHz stereo F32 and delivers
actual callbacks. SDK frames and Coax's 1024-frame clock are observed
separately, including the possible partial-block remainder. Both hold across
pause, resume advances both, and Coax time holds after the concrete stream
is closed. The physical SDK output planes are inspected before each queue
operation and contain zero nonzero samples. The final device receipt
authority kills 59/59 mutants without equivalents. Normal and instrumented
device receipts retain exact counts and the actual assigned PipeWire node.
This measures a PipeWire stream, not DAC/optical latency.

Failures remain retained under
`~/.openagents/aethyrforge/host-contract-12869/linux-audio/`. The first
device observer sampled format before the inactive stream negotiated it;
the corrected observer samples the completed running interval. Initial
LeakSanitizer device runs reported 3,677 bytes from the dynamically unloaded
SDK module path. Balancing `pw_init`/`pw_deinit` and ordering stream teardown
did not alone clear that report. A diagnostic with SDK module unloading
disabled was retained separately and is not the acceptance run. The adapter
uses a locked, non-realtime callback, and now explicitly skips its unused
realtime/RTKit scheduling module. Actual normal and instrumented device runs
use ordinary module unloading and leak detection, with no sanitizer report.
The [PipeWire thread-loop contract](https://docs.pipewire.org/page_thread_loop.html)
specifies the loop lock and stop ordering; the installed SDK is 1.6.6.

`NativeAudio.c` was emitted by the original admitted 598 compiler from the
genuine c224 compiler root with separately pinned product inputs. Its actual
20-source table joins current pure-Coax audio inputs with zero differences.
The existing original receipt verifies before/after that preparation; this
does not create a current compiler release or change compiler admission.
Generated C, headers, profiles, exact SDK source and executable hashes remain
in the durable artifact directory and build manifests. The compiler, helper
and original receipt identities remain those in `qualified-shared-frame.md`.

A fresh current world emission reached its 840-second bound cleanly, with
status 137, timeout true and zero output; no C/header was admitted. Its log
and separately pinned e35 product preparation are retained. The frozen bank
component and its additive storage exports are not mislabeled current realm
or full-host qualification.

Reproduce from the repo using retained emissions and installed SDK paths:

```sh
nice -n10 coax/web/tool.sh aethyr/host/build-linux-audio.coax \
  ROOT EMISSION NEW_SDK PIPEWIRE_DEV PIPEWIRE_LIB [--sanitize] [--memory]
NEW_SDK/check-linux-audio BANK.json WAV_AND_PCM_DIR PLAN.json RENDER_DIR \
  NEW_REPORT ROOT
NEW_SDK/check-linux-audio BANK.json WAV_AND_PCM_DIR PLAN.json RENDER_DIR \
  NEW_DEVICE_REPORT ROOT --device-only
MEMORY_SDK/check-linux-audio NEW_MEMORY_REPORT ROOT
```

All device checks are muted; no GPU window or audible output was opened.
Ordinary Vulkan integration, bounded queue adoption, remaining service exits
and presentation/compiler admission are still open. The desktop default
remains web and both follow-up issues remain open.

## Ordinary realm dispatcher audio trace

The shared `audio-session.coax` authority now owns gesture, coverage and
terminal retirement. Its root gate kills 33/33 mutants. The actual Linux
PCM callback and PipeWire adapter consume its offer; inactive owners retain
their exact Coax clock and partial-block cursor. The existing 59-mutant
device checker passes with this adapter under ASan/UBSan/LeakSanitizer.

`check-linux-audio-product.coax` drives the ordinary `realm-host.c` API with
a Coax recording realm and actual muted PipeWire. Normal and instrumented
runs observe two assets/16 retained sample words; startup control result 128;
pre-gesture cue result -2; zero pre-gesture and covered clocks; a covered
gesture remembered without starting callbacks; one running callback/1024
Coax frames; the same clocks while hidden; two callbacks/2048 frames after
resume; and unchanged 2,048 frames after terminal device retirement. The
device and context are explicitly closed. Actual output planes contain
zero nonzero samples. Exactly three nonempty sound packets drain; repeated
`rh_after` does not replay their accepted commands.

The metadata length exceeds its physical list allocation in one workload,
which refuses before forming a borrowed view. Another offers its first WAV
at the physical storage end: that row refuses, its cue is unavailable, and
the following valid asset remains playable. Two real producer frames
continue in each failure workload. Oversized, null and wrapping sound
packet spans each drain once, publish no command, and leave two producer
frames able to advance. The semantic observer kills 93/93 root mutants,
without equivalent annotations. The recording realm's 67 checks pass.

`linux-audio/product/` retains both actual receipts, the exact field order
from `HostAudioProductObserved`, mutation logs and source observations.
Generated native audio, fixture and checker source tables join 21,14 and11
files respectively with zero differences. The original admitted 598 image
emitted each from the genuine c224 compiler root with pure product inputs
overlaid separately. These are explicit development product overlays, not
a new compiler source receipt or release. Frozen C/headers, SDK source,
profiles and actual executable/build manifests remain in
`~/.openagents/aethyrforge/host-contract-12869/linux-audio/` under
`product-sdk-sources`, `sdk-product-packets{,-asan}` and
`product-packets{,-asan}`. Earlier 31-field trace and build failures remain
separate historical artifacts.

Build with `build-linux-audio ... --product [--sanitize]`, then run
`NEW_SDK/check-linux-audio NEW_REPORT ROOT`. `--product` is exclusive with
`--memory`. This qualifies ordinary realm dispatch, physical memory views
and device ownership using a recording face. It contains no Vulkan window,
current Everglade producer or room and does not close those remaining gates.
