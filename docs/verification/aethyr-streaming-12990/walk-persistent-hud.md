# Persistent streaming HUD — November 7, 2026

**#12990 remains open.** Starting at local checkpoint `83db823dd9`, the
reused issue worktree adds a persistent texture-free HUD to the actual
browser WebGPU and native Vulkan corridor. It renders before area resources,
through the full out-and-back route, physical device replacement and final
cleanup. This record supersedes the earlier HUD-adoption gap, while keeping
the wider streaming/recovery and physical-planning exits open.

## Behavior and ownership

`stream-hud.coax` reuses Aethyr's Filament strokes and progress rectangles;
it does not introduce another font or asset pipeline. The bounded caption
shows distance along the original 768 m corridor. Geometry XY is snapped
once to a declared 1/16-pixel grid; stroke-local coordinates retain their
original meaning. The additive `stream-walk-presentation.coax` adapter reads
only the admitted player's pose, with no cell reference or clock advance.

The fixed CPU/GPU vertex spans are each 98,304 bytes, with a 256-byte GPU
uniform. They persist independently of cell resources. Fixed renderer
grants cover these buffers, the existing 4,096-byte uniform, four-byte depth
and display targets, and a new eight-byte-per-pixel linear FP16 target.
World and premultiplied HUD compose into FP16; a Coax fullscreen pass
samples it once into UNORM. `aethyr.stream-walk-display.v1` declares this
linear fixture without tone/gamma transformation. This does not qualify a
general color-management pipeline. Native allocation extents include SDK
alignment and remain distinct from these logical sizes and RCM1 categories.

Inspection found the preceding native carrier released its fixed grants
just before physical surface destruction. The order is corrected, and the
SDK now counts missing grants at actual close. The checked verdict and
independent oracle both require zero violations, including replacement and
final teardown. Final-zero counts alone would have missed that ordering.
The native receipt is explicitly v2; historical v1 remains accepted with
its original fields. V2 binds all three shaders, the display profile,
per-frame HUD observations and the destruction-order counter.

The world projection also now stages all twenty words, rejects nonfinite
input/output atomically and chooses an alternate up vector for a vertical
view. Checks independently work the oblique matrix and every vertex lane,
and sample separate base/bake channels and normal-response boundaries.

## Actual qualification

The unchanged original `corridor-v2` scene identity is
`ceefa05a481f8c8562646be74be333607075ae874fa3a076982ba23a81d00810`.
Its source resources, all four RCM1 companions and cost-model identities
remain unchanged. Browser and Vulkan use their own backend manifests.

- Four actual muted headless browser histories cover **320×180 and
  640×360, each at both image tiers**. Every history starts with a zero-dt
  HUD-only draw and zero area GPU objects, then completes 3,233 observed
  frames plus cleanup. The first cell unloads while HUD grants stay live;
  returning preserves stable entity IDs and advances saved actor state.
- Controlled replacement forbids source fetches, retains state and gives
  exactly equal complete before/after images. HUD remains live through
  all three cleanup frames, for 3,237 draws. Resource, ledger and display
  owners reach zero. No browser GPU errors occur.
- All four accepted browser histories replay through the **unchanged
  prior ASan/UBSan/LSan CPU binary**, with all **12,931 canonical paid-step
  observations** agreeing. Each independent replay oracle rejects six
  altered histories. Different live completion schedules are not claimed
  equal to one another.
- Both native tiers run the full 3,235-frame route on RTX 4080 and again
  under ASan/UBSan/leak detection on Lavapipe with two workers. All 114
  attempts and 776 physical allocation/free events drain. Khronos
  validation reports zero errors/warnings; raw logs contain no sanitizer
  diagnostics. Each independent native oracle rejects **30** altered
  histories, including the new shader/HUD/teardown cases.
- Both hardware tiers preserve **every one of 12,931 pre-HUD canonical
  digests**, accepted events, allocation records and actor checkpoints
  from `83db823dd9`'s world-only native histories. Display counters/storage
  remain outside the explicitly declared simulation state profile.

The independent triangle/top-left/capsule/FP16-composition oracle compares
every channel, with a fixed bound of **two**, no masks and no alignment
shifts. All four native composites (230,400 channels each) and all four
browser startup images (230,400 or 921,600 channels) have maximum error
**one**. Four hand-worked 2×2 raster cases check shared-edge coverage,
winding, layered premultiplied composition and degenerate triangles; three
deliberate channel corruptions are rejected. The native reference starts
with the frozen prior world-only GPU image; this is **not an independent
world-shader reference**. Browser startup has an authored clear background
and no world draw. Layout/labels are checked separately in Coax.

At the common 320×180 recovery view, the complete RTX/Vulkan and browser
images are byte-identical at each tier. Their RGBA SHA-256 is
`80e658f14f95f2092605fc1262224825f1f2012f3f923dd6d9802794811c8082`.
Lavapipe preserves its own full recovery image exactly; its distinct hash
is recorded rather than asserted to match hardware. The actual final
native image was visually inspected for legible text and unobscured world.

| Run | Dynamic CPU peak | Explicit Vulkan peak | Replacement ms | Frame ms p50 / p95 / p99 |
| --- | ---: | ---: | ---: | --- |
| RTX 4080 / tier 0 | 14,139 | 1,648,592 | 126.814 | 1.139 / 1.657 / 2.572 |
| RTX 4080 / tier 1 | 14,232 | 1,650,640 | 124.731 | 1.136 / 1.278 / 2.487 |
| Lavapipe + sanitizers / tier 0 | 14,139 | 1,258,440 | 69.313 | 10.323 / 11.685 / 19.314 |
| Lavapipe + sanitizers / tier 1 | 14,232 | 1,259,464 | 69.954 | 10.314 / 11.558 / 19.417 |

Native fixed core/face metadata is 873,793 bytes; SDK carrier metadata is
22,690 bytes. Report/readback/Coax stack arenas, shader-word arrays, browser
Wasm/transport arenas and opaque driver/pipeline internals remain separate
owners/observations. These are unpaced headless timings including canonical
observation, not interactive FPS or phone-hardware qualification. The first
hardware run overlapped the software sanitizer run; timings retain that
condition. Browser replay CPU peaks are in the individual oracle files.

## Checks, failures and reproducibility

Root mutation gates retain their full loaded baseline checks and kill
**269 mutants, zero survivors**: world projection/shader 107, HUD layout
108, HUD shader 18, final display shader 4, presentation adapter 7 and
native conformance 25. No equivalent annotations or tolerance exemptions
were added. Unchanged prior owner/coordinator/state gates are reused from
their pinned preceding records, not rerun as confirmation.

The initial UNORM composition failed the predetermined two-level channel
bound: one channel differed by three. The geometry grid alone still failed
that bound. FP16 composition followed by one UNORM conversion passes at
one, without loosening the bound. The grid remains an explicit geometry
profile, not the explanation for fixing the outlier. The
[Vulkan blend specification](https://docs.vulkan.org/spec/latest/chapters/framebuffer.html)
allows destination-format precision; that motivates higher-precision
composition but does not prove the exact cause of the observed outlier.
Both failed raster logs are retained. Earlier weak mutation checks were
strengthened at actual boundaries rather than marking mutants equivalent.

The first full browser observer exceeded CDP's 65,536 JSON-node limit after
its product journey passed. The final Coax observer carries bounded raw
JSON as one returned string. An initial native replay oracle used a report
directory without the required executable and failed FileNotFoundError;
the positive replay had passed. Reports now live beside the same immutable
binary, so all six negative cases execute. Neither harness failure is
reported as a product pass; historical logs remain in the artifact set.

Coax 0.52.0 and GCC 15.2.0 are unchanged. All three current WGSL emissions
match the actually served files byte-for-byte. All three current SPIR-V
programs pass `spirv-val --target-env vulkan1.1`. The executed browser Wasm
is 141,838 bytes. Exact source/SDK/compiler pins, generated artifacts,
receipts, raster and replay oracles accompany this record. Book registry
and tools-reason results are retained separately; chapter status is unchanged.

Artifact root: `~/.openagents/aethyrforge/streaming-12990/2026-11-07/`.
Final native builds are `native-hud-owned-final{,-san}`; routes are
`native-hud-owned-{low,high}{,-san}`. Final browser histories are
`browser-hud-{small,large,high,small-high}-final`, with replay reports beside
`native-replay-source-current-san/replay`. Prototype `native-hud-*` and
`browser-hud-small-v1` artifacts retain their narrower/failed scope; earlier
`native-hud-*-final` runs precede the fixed-grant teardown correction and
are not substituted for the final ownership join.

Reproduction uses `coax/web/tool.sh aethyr/tools/stream-native-build.coax
NEW_BUILD [--sanitize]`, then the invocation and installed SDK environment
in [native-runtime.json](native-runtime.json). The browser build uses
`coax/web/wasm.sh aethyr/world/stream-walk-presentation.coax
aethyr/host/tests/stream-walk-presentation-face.c NEW.wasm`; serve this as
`/build/stream-walk.wasm` with the three generated WGSL files and the
original scene. Run `coax/web/tool.sh aethyr/tools/stream-hud-browser.coax
BASE SCENE_JSON NEW_OUTPUT WIDTH HEIGHT TIER`. The independent HUD oracle
supports `--native OUTPUT PRIOR_WORLD_OUTPUT`, `--browser OUTPUT NATIVE_OUTPUT`
and `--compare-hosts BROWSER_OUTPUT NATIVE_OUTPUT`. Commands are bounded to
60–240 seconds, mutations to 890 seconds. The schema/ownership record
remains additive to published asset, host and canonical-state contracts.

## Remaining #12990 requirements

Automatic/repeated unsolicited loss, the ten-second deadline and failed
replacement rescue remain unfinished. So do stale/superseded callbacks,
remaining allocator/SDK failure histories, actual category-pressure refusal
with resident content still usable, complete transport/transient and
physical native planning, the independent world-shader GPU reference,
offline CPU replay of the native GPU backend history, and the final
consumer/residual-adapter inventory. Existing controlled replacement,
browser replay and HUD raster proofs do not replace those requirements.
