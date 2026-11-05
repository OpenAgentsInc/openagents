# Complete shared Everglade frame — iteration observations

**Provenance correction:** these retained executions used an iteration
compiler without a trusted recorded source receipt. Supplying a reconstructed
digest made the measurement tool's source comparison pass, but did not
establish compiler provenance. These results are numerical/SDK diagnostics,
not admitted host qualification or permission to change the desktop default.
The raw reports and hashes remain unchanged for audit. Preparation with an
actually admitted compiler and its original provenance is still required.

This freezes one real Everglade 5E producer frame, all of its physical
inputs and the complete final image. It exercises the shared renderer;
the seven older G5 per-pass fixtures remain independent qualifications.
The producer/source fence is `a6fd79666a` plus the new
`aethyr/render/carriers/everglade5e.coax` composition. Subsequent room
changes on main do not alter this retained capture's inputs.

The browser observer passed at 320×180 in both `rgba16float` and ordinary
`bgra8unorm`, with no visible browser, no sound, no source drift and no GPU
validation error. It retained the complete source producer outputs: 217
draw records, 59 active admitted records, 1,638 HUD vertices, one water
record, 13,824 grass instances and 106 particle instances. All eight
textures, their complete mip chains and all seven authored hotbar media
images are retained. The layers array contains 38 slices; the byte rows
and level offsets are in `shared-browser-half.json`.

The Vulkan SDK replay uses exactly that RFI, package, physical buffers,
textures and media. It executes 136 actual draw calls on the RTX 4080,
including repeated records in the shared passes. API validation reports
zero errors and warnings. The complete 460,800-byte RGBA16F readback,
native executable, emitted Coax C/header profiles, SDK/driver identities,
input archive and before/after source fences are retained under
`~/.openagents/aethyrforge/host-contract-12869/shared-frame/`.
`shared-vulkan-half.json` is the execution record, and
`shared-vulkan-browser.json` is the inherited G5 comparison verdict.

The iteration compiler is SHA256
`c0561aaa7b630175f7e8efc1718c76397ea4584dac04acfacaa6d9cc6ab36b7a`.
Its original retained folder has no source-digest sidecar. The sidecar for
this replay is explicitly reconstructed from the published #12867
provenance and the unchanged compiler sources: `git diff 4fa20ba31b
a6fd79666a -- coax/src coax/bootstrap coax/lib/compiler coax/lib/proto`
is empty. The compiler-source digest is
`11c8cc08d42efc3f9dc61054f3dcbd3f256e4fabd1cef951b5f8359b05267d50`.
That reconstruction is not a trusted source-digest receipt and must not be
reused for admission. The native preparation gate's matching-source fields
in the retained raw JSON record only the supplied comparison; they do not
prove this image passed the compiler release gates. No compiler release
was published by #12867.

## Numerical scope

The published G5 live/post gate permits at most 2,000 values beyond
`abs(actual-reference) <= 0.002 * max(1,abs(reference))`, with every
nonfinite fatal. That allowance is unchanged in
`coax/tests/metal-parity.coax`; `aethyr/render/emit.coax` remains the
comparison authority. The full frame has 230,400 channel values.

| Readbacks | Beyond per-value tolerance | Nonfinite | Iteration numerical result |
| --- | ---: | ---: | --- |
| browser–Vulkan | 53 | 0 | pass |
| Metal–Vulkan | 53 | 0 | pass |
| browser–Metal | 0 | 0 | pass, recorded by #12868 |

The supplemental zero-allowance browser–Vulkan comparison correctly
**fails** and is retained as `shared-vulkan-strict.json`. Its maximum
absolute difference is 0.01270751953125; its first differences are near
the overlapping additive HUD strokes at the top of the image. A blend
precision explanation remains an inference. No image mask, shader change,
threshold adjustment or new allowance was used. Passing the inherited
gate does not mean those 53 values are equal.

`unpack-readback.coax` expands the existing comparison decoder's half or
byte values to little-endian RGBA f32 without selecting a tolerance. Its
focused check covers half values, signed zero and BGRA lane order.
`check-live-graphics.coax` applies the existing G5 live comparison directly
to the saved final images. The strict comparator is also retained.

The native Metal owner replays these same inputs through its production
MF interpreter, with API/shader validation and a post-close zero-resource
ledger. Its output is under
`~/.openagents/aethyrforge/metal-host-12868/replay-half/`. The final Metal
source/executable join belongs to #12868 and must be recorded before the
desktop default changes. Frozen-frame execution alone does not establish
interactive producer, input, room, lifecycle or audio parity.

## Reproduction

For a qualifying replacement, use an actually admitted compiler with its
original recorded source provenance, retain the package's source and shader
identities, and run the existing observer with the new carrier. The iteration
compiler and reconstructed digest above cannot supply that admission:

```sh
coax/web/tool.sh aethyr/tests/check-render-host.coax \
  SHARED_5E.wasm PACKAGE_DIR PRIVATE_PAGE.html NEW_CAPTURE \
  --autoentry --require-draws --size=320x180 --half --surface-root PRIVATE_SURFACE
```

The private page is the actual Everglade page with
`data-render="/package/render.json" data-render-profile="2"` added. The
private surface serves the actual public source assets and resolves
`/worlds` to `/realms`, preserving the authored hotbar URL and image bytes.
The public page's host selection is unchanged. Omitting `--half` captures
ordinary browser presentation. Keep the complete resulting `frame.config`
and every file it names.

Pre-emit `aethyr/render/native-host.coax` and
`aethyr/render/native-texture.coax` with `coax c --library --header ...
--profile-report ...`, retaining `RenderHost.c/.h`, `RenderTexture.c/.h`,
`emission-profile.json` and `texture-emission-profile.json`. Then:

```sh
coax/web/tool.sh aethyr/render/tests/measure-native-frame.coax \
  --work-dir NEW_VULKAN --coax QUALIFIED_COMPILER \
  --compiler-source-digest TRUSTED_RECORDED_SOURCE_DIGEST \
  --frame CAPTURE/frame.config --capture-root CAPTURE \
  --package PACKAGE_DIR --native-dir PREEMITTED_NATIVE \
  --measure --host coderos-4080 --format rgba16f
coax/web/tool.sh aethyr/host/unpack-readback.coax \
  CAPTURE/gpu.bin rgba16f NEW_BROWSER.f32
coax/web/tool.sh aethyr/host/check-live-graphics.coax \
  NEW_BROWSER.f32 NEW_VULKAN/screen.raw rgba16f NEW_VERDICT.json
```

Every command has a bounded deadline. The Vulkan observer retains all
created outputs and removes only its owned remote namespace after the
source, artifact and SDK fences pass. The full producer's generated C and
header are also retained in `shared-frame/producer/`, alongside the exact
Wasm and both captures. These saved bytes remain usable independently of
later source or room changes.
