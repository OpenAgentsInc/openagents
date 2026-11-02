# The Grid: entry, movement, replay and rendering

The declared headless Grid route now passes from the actual homepage control
through movement, two jumps, camera following, shared rendering, a real missing
resource and recovery, and replay. The native face consumes the same recorded
inputs. Its final frame produces exactly the same complete renderer input as
the browser capture, which joins it to the retained Vulkan and CPU checks.
The machine-readable record and raw archives are in
[grid-playable-route/qualification.json](data/aethyr-ch11-2026-11-02/grid-playable-route/qualification.json).

The Coax journey clicks the measured WALK AROUND homepage control and follows
the real navigation to `/realms/the-grid?from=home`. It runs 240 frames at an
observed 960×540 viewport with 14 actual keyboard edges: run, jump, turn, walk
and strafe. The session completes 254 fixed steps and takes two jumps. The
drawn feet rise on 38 frames, reach 1.11328125 metres, and land. Feet, camera
eye, aim and facing come from read-only exports of the actual `GridWalk`.
Observation never advances the session. Replay uses a separate state and
preserves the live session's steps and digests.

This is muted headless Chrome with normal animation and no golden or probe
URL. The clock, external requests and input schedule are deliberately frozen
by the test. External API fetches remain pending and external sockets are
refused; live service readiness is a separate qualification. The browser's
viewport and canvas dimensions are measured after explicit CDP device metrics.
An earlier passing route had an actual 960×397 viewport; that result remains
physics/pose-only and is not used for matched-viewport raster claims.

The resource control removes the actual RenderHost Sans allocation. The
existing Coax texture-shape admission refuses before any GPU buffer write or
submission. Restoring the identical texture object permits the next frame and
GPU validation passes. The earlier plain-Error failure is retained. Its repair
passes observed zero layers to the existing Coax rule; JavaScript adds no new
resource admission policy.

The native program uses the same admitted production C/H as the ordinary Wasm
build, the actual C face and the existing realm host. Coax owns the input
schedule, output and verdict. Debug, optimized and ASan/UBSan runs each match
all 2,400 integer fields, 2,400 interpolated f32 pose words and 254 step marks.
Controls that drop jumps, freeze time, read the wrong pose field or skip replay
are caught. Sanitizer stderr is empty; leak detection is explicitly disabled.

After the route, two zero-time resource-control frames and two zero-time resize
frames produce a separately observed 320×180 RGBA16F frame. Coax writes the RFI
and captures the actual GPU bytes and all five physical Sans mip levels. The
native face independently dumps its actual uniforms, HUD, draw and index bytes
at the same final state and extent. Counts, identities, flags and stroke modes
are exact. The predeclared finite f32 allowance is abs1e-6 or rel1e-6; all
401,080 uniform/HUD words are bit-identical, so that allowance is unused.

The Coax assembler binds those four native files to a complete 14-file
RFI/config/package/shader/resource inventory. Every file equals the browser
capture byte for byte, every config token is accounted for and hashes remain
unchanged across the check. It publishes `frame.config` last in a new output
directory. The dump bridge's optimized ASan/UBSan run passes the same complete
join. The shared Sans bytes are explicit scene inputs; this does not qualify
native font rasterization or network acquisition.

That complete equality permits exact-input reuse of the already retained
[moving-pixel qualification](data/aethyr-ch11-2026-11-02/grid-moving-pixels/qualification.json).
Both WebGPU and Vulkan comparisons check all 230,400 RGBA channels with zero
colour failures, nonfinite failures or masked pixels. Maximum normalized error
is 0.0009643201542912247 under the unchanged 0.002 tolerance. Vulkan reports
zero validation errors and warnings. Those numerical checks cover the final
320×180 state. The 960×540 route has actual submissions and exact session/pose
evidence; it does not have numerical coverage for every moving frame.

Compiler and executor epochs stay explicit. The ordinary producer uses retained
compiler `1fac3750…`, while its shader package uses historical compiler
`30868e8c…`. The numerical native and CPU executors are retained historical
artifacts. No matching-main compiler or current-source executor emission is
claimed. Exact executed tools, generated sources, input bytes, failures and
successful observations are archived; host/render source snapshots were saved
after the browser run, so they are not a before/after whole-repository source
fence. The capture package guard added after capture3 has its own focused check
and independent retained-input readback.

The new read-only Grid probes pass 328/328 checks and kill all 256 root mutants,
with no equivalence waivers. A fresh
[audit of the exact final Wasm](aethyr-grid-final-stack-2026-11-02.json) bounds
all defined/exported linear-memory stack paths at 1,953,936 bytes inside the
4 MiB reservation; the newly reachable replay path is included. The separately
reviewed [stock visual migration](data/aethyr-ch11-2026-11-02/grid-visual-migration/qualification.json)
retains the old image and passes without changing its allowance.

The interactive `vk-realm` window and its complete 240-frame lifecycle remain
unqualified. Touch, live network, accessibility, every-realm rollout and final
matching-main toolchain acceptance remain separate work. This record closes
the stated headless producer-to-renderer route and makes no whole-chapter or
book-completion claim.
