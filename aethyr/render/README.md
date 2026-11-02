# Aethyr renderer

Chapter 11 is partly implemented and is being qualified. Its status stays partial
until the actual producers, CPU reference and both GPU hosts pass their
declared checks. The running record is
[aethyr-book-implementation-2026-11-01.json](../../docs/verification/aethyr-book-implementation-2026-11-01.json).

The current shared catalog contains sixteen historical manifest profiles, the
direct Garden and the retained Galaxy fixture. Latest main also adds Anime Lab;
its private adapter is not yet part of this shared catalog or its qualification.
Galaxy remains absent from the shipping realm list.
Its development route is `/render-fixtures/galaxy/`.

| Source | Responsibility |
| --- | --- |
| `frame.coax`, `geometry.coax` | Measured upload spans, the four-u32 draw ABI, actual vertex/index/instance domains and bounded admission work |
| `registry.coax` | The GPU resource ledger: slot, generation and epoch for every object the browser renderer creates, its word budget, release, and device loss |
| `catalog.coax`, `bindings.coax`, `binding-sources.coax` | Profile identities, shader entry pairs, logical resources, physical reflection and resource origins |
| `passes.coax`, `resources.coax` | Pass operations, attachment state, resource extents, sampler declarations and observed buffer initialization |
| `runtime.coax` | Atomic composition of the canonical graph, effective work counts, geometry and frame admission |
| `build-policy.coax`, `interface.coax` | Compiler reflection and the catalog's vertex/fragment interfaces, including vertex-only casters |
| `build.coax`, `package.coax` | Content-addressed shader packages, dependency fences, runtime catalog/device admission and fetched-byte hashes |
| `noise.coax`, `footprint.coax`, `sampling-core.coax`, `sampling.coax` | Portable integer noise, fine derivative footprints and the shared eight-tap texture filter |
| `texture-codec.coax` | Physical texel decoding; sRGB RGB becomes linear before filtering, while alpha stays linear |
| `reference-quad.coax`, `reference-ir.coax`, `reference-io.coax` | Four helper lanes, checked tree-v2/v3 execution, resource views and stage word layouts |
| `reference-raster.coax`, `reference-depth.coax`, `reference-pipeline.coax` | Clipping, interpolation, coverage, depth bias, actual stage execution, blending and target rounding |
| `reference-frame.coax`, `reference-work.coax` | Full admitted CPU frame, atomic publication and explicit bounded offline work profiles |
| `host-policy.coax`, `host-inputs.coax`, `host-media.coax` | Package transaction, measured native/Wasm spans, raw flags, canonical buffer and physical media observations |
| `native-texture-profile.coax`, `tools/native-textures.coax` | Coax-owned native raw texture descriptors and build-time staging from existing decoders |

The native and browser adapters report measurements and execute admitted
operations. They use the compiled resource-origin table rather than guessing a
binding from its name or slot. Published scene draw records remain four words.

## Build a package

Run from the repository root with the source-matching Coax 0.47 compiler that
supports fine derivatives, helper demotion and version-3 array texture queries:

```sh
coax/web/tool.sh aethyr/render/build.coax --profiles
coax/web/tool.sh aethyr/render/build.coax the-grid /tmp/grid-render
```

`render.json` uses `aethyr.render.package.v1`. Its WGSL modules, per-pipeline
SPIR-V files and actual checked CPU trees are relative siblings named by their contents. The builder checks
all source inputs before and after emission. It reuses a package only when its
compiler, sources, catalog admission and every product hash match. It replaces
the manifest atomically after all products succeed.

The app build writes packages to `apps/openagents-web/build/render/<profile>/`.
Realm pages name them with `data-render` and the compiled catalog index in
`data-render-profile`. The generator transports `page-render.coax`'s Coax-owned
adoption decision; `build.coax --profile-table` still exposes the compiled
catalog identities. Anime Lab is explicitly tracked as using its existing
renderer until shared adoption is qualified. Unknown or mismatched page/app
identities refuse rather than receiving a fallback profile. A fork keeps its
base renderer identity when its realm/save-slot slug changes.
Release staging copies a complete
package under a directory named by the manifest hash, retaining every sibling
filename and byte. The comment-stripping step refuses any transform that would
invalidate a packaged shader hash. Dynamic made/forked/reply pages inherit the
attributes from their existing base-page script.

`wasm.sh` automatically includes a canonical catalog application's source
carrier. `COAXIAL_RENDER=0` produces the low-level legacy diagnostic artifact;
the shipping pages use the shared facade. A different source with the same
basename is never silently replaced by a catalog carrier.

The native launcher defaults to the shared package. `VERSE_GPU_RENDERER=legacy`
selects its retained comparison route. Run one realm per bounded check command;
`VERSE_GPU_CHECK_SIZES=''` keeps the 90-frame integration check and omits its
separate performance sweep. Native runtime package loading happens at startup;
the browser's live shader replacement has its own transaction controls.

## Borrowing and qualification

`render_package_parse(json, entries)` needs retained JSON and a caller-owned
array of at least fourteen `RenderPackageEntry` records. It returns a view of
the first fourteen. Parse into candidate storage, admit it against the expected
profile and enabled device features, and hash fetched bytes before pipeline
creation. Keep an active receipt's storage separate from its replacement.

`package.coax` is a source part so a producer/facade carrier can own one copy of
the existing SHA kernel. It retains physical `#authority`; source ownership
does not remove its checks or mutation gate. The sampling effect adapter is
also separated from its checked arithmetic receipt.

Each authority module has ordinary checks and a root mutation gate. Independent
normal-C tools under `tests/` inspect actual producer bytes, generated trees,
interfaces and outputs in debug, optimized and sanitizer builds. Their receipts
name the exact compiler and source closure. Historical exploratory evidence
stays historical when a compiler, source or qualification profile changes.

Package framing/hash probes are not SPIR-V validators. Actual GPU qualification
also runs the platform validator and checks images; geometry receipts establish
addressed domains rather than arbitrary shader or BVH truth. The chapter's
verification record tracks the remaining integration and image work.


## Published CPU metadata facade

`frame.coax` preserves the published `Rf*` CPU metadata API used by the Grid's
native projection boundary alongside the `Render*` whole-frame API. The facade
uses the same `render_span_fits` interval predicate as whole-frame admission,
while preserving its public records, cumulative word budgets and atomic result.
It submits no GPU work. The complete playable route in
[#12628](https://github.com/OpenAgentsInc/openagents/issues/12628) remains open.
The original [qualification](../fixtures/render/qualification.json) retains the
pre-integration source/compiler identities, 120 killed policy mutants, native
observations and failed check-first attempts. It is historical evidence for that
exact component; changed-source integration has its own focused checks.

The [merged API qualification](../../docs/verification/aethyr-frame-reconciliation-2026-11-02.json)
records both test groups, the combined root mutation gate, the two independent
native oracles and the explicit older compiler identity. It qualifies these
admission profiles, not a complete GPU or playable-realm route.

| Public value/procedure | Contract |
| --- | --- |
| `RfHandle.{slot,generation}` / `RfResource` | A resource slot, positive generation, kind, word capacity and inclusive `first..last` frame-epoch interval. The supplied table is immutable during admission. |
| `rf_begin(epoch, limits)` | Positive epoch/pass limit; nonnegative binding/draw/word limits. A refused declaration returns an inactive frame. |
| `rf_pass(frame, resources, target, bindings, draws, geometry)` | A color target, uniform/storage buffer bindings and indexed/direct draw ranges. Returns `{accepted,frame}`. Refusal preserves every named frame field; success publishes all counters together. |
| `rf_finish(frame)` | Finish or abort: clear CPU reservations and deactivate the frame, retaining its epoch/declaration. Idempotent. GPU objects remain their host owner's responsibility. |
| `rf_viewport(width,height)` | Finite positive viewport dimensions; no implicit maximum or downscaling. |

Binding offsets/extents and `limits.words` use **four-byte words**, not bytes.
Each binding's positive extent fits its resource; a `(group,binding)` pair is
unique within a pass, in the bounded groups0–3/bindings0–7 profile. The word
budget sums declared binding extents across passes, including repeated reads;
it is not unique allocated GPU memory or the existing `FrameBudget.bytes`.
Draws retain the hosts' four-u32 layout, in order: `first,count,base,instances`.
The high bit of `base` selects direct drawing; the remaining bits select the
first instance. Both index/direct ranges and instance ranges must fit.

The caller keeps the **current returned frame**, unchanged, and supplies valid
live view storage and an immutable truthful resource table. Slot/generation
and epoch intervals are checked symbolic metadata, not unforgeable handles,
physical lifetime proof, a resource allocator or protection against replaying
an earlier copied frame. Admission retains no views. Indexed asset owners must
admit index **values** separately. The opaque color target does not qualify
format, size, sample count, read/write barriers or attachment compatibility.
Texture/depth/sampler bindings, host-specific GPU alignment, shader layouts,
rasterization, blend/depth/rounding and fragment-quad derivatives are outside
this metadata facade. The `Render*` path qualifies those separately. Empty
`RfDraw` records still have their declared ranges checked; the whole-frame GPU
path skips zero-count or zero-instance records. Neither API silently changes
the other profile's meaning.

### One complete CPU metadata trace

[admit-frame.coax](admit-frame.coax) admits epoch9 with one color target at
generation5, a sixteen-word uniform at generation2 and one direct triangle.
Changing only the target generation to4 refuses the whole request, leaving
zero passes reserved. Finishing the accepted frame releases its CPU counters.
Expected stdout is [render-admission.out](../fixtures/render-admission.out).

```sh
"$COAX" run aethyr/render/admit-frame.coax
"$COAX" run aethyr/render/admit-frame.coax --release
"$COAX" test aethyr/render/frame.coax --library
"$COAX" test aethyr/render/frame.coax --library --release
"$COAX" mutate aethyr/render/frame.coax --library --root-only --jobs 4
python3 aethyr/tests/check_render_native.py --coax "$COAX" --mode release --out /tmp/new-render-release
python3 aethyr/tests/check_render_native.py --coax "$COAX" --mode sanitized --out /tmp/new-render-sanitized
```

Use a new output directory for each native command. `--mode debug` is also
available. The driver binds the selected compiler/CC, literal `#load` closure,
generated C/header/native bytes and before/after identities. No browser, GPU,
dataset, network, sound, app build or unrelated gate is invoked. The native
oracle uses independent widened-sum range equations, actual IEEE nonfinites,
the actual Grid function, guard words and named-field refusal/cleanup checks.

### The GPU resource ledger

[registry.coax](registry.coax) makes the `Rf*` slot, generation and epoch
records real for the browser renderer. `coax/web/render.js` asks the ledger
before it creates each buffer and texture: the vertex, uniform, index and
storage buffers, the generated bloom and media uniforms, the Sans atlas, the
layer array, every frame target and every media texture. A refusal (an unknown
kind, an empty or oversized extent, all 512 slots in use, or the 1 GiB word
budget spent) creates nothing. Each object carries its slot and generation;
destroying it releases them, and a replaced mesh placeholder or texture is
released, not leaked.

Each frame advances the ledger's epoch, then checks that every buffer, texture
and attachment it will draw with is live (`rf_live`) before the first GPU
write. A released or foreign handle refuses the whole frame. A lost device
(`coaxial_render_reset` after the first) releases every object and keeps the
generations, so no handle from before the loss is live again, even once its
slot is reused. The ledger's table is `RfResource`, so `rf_pass` admits a pass
over the same records. Engine replacement moves a retained object into the new
engine's ledger. Capture-only readback buffers are created and destroyed in
one call and are not entered.

```sh
"$COAX" test aethyr/render/registry.coax --library
"$COAX" mutate aethyr/render/registry.coax --library --root-only
coax/web/tool.sh aethyr/tests/render-ledger.coax http://127.0.0.1:8800/realms/the-grid/
```

The headless check loads the served Grid, matches the ledger's live count to
the objects the renderer holds, releases the Sans atlas's handle under the
renderer and sees the next frame refused before any GPU write, then loses the
device and sees every new object live and no old handle live.

The native Vulkan host uses the same ledger through the same exports, one per
process. `render-host.c` owns it and its frame targets. `render-realm.c`
asks it for the vertex, index, named and generated buffers, and
`render-textures.c` for the realm's images. Each frame is a new epoch. A
target, buffer or image that is not live refuses the frame before its
candidate is prepared or its buffers are written. `vk-realm` prints the
ledger at the last frame and after close:

```sh
REALM=newland VERSE_GPU_CHECK_SIZES='' verse/gpu/realm.sh local check
```

The iOS Metal host (`HomeRenderer.swift`) does not use the ledger yet.

### Native projection consumer and residual adapters

[`verse/grid.coax`](../../verse/grid.coax)'s exported `verse_grid_view` calls
[`verse/grid-render.coax`](../../verse/grid-render.coax). The adapter constructs
a one-pass, sixteen-word uniform descriptor and delegates viewport/pass
decisions to this authority module before any matrix output write. Short matrices
or nonfinite viewports preserve caller output. This local synchronous
descriptor does not represent a retained GPU object. Existing camera equations
and eye/aim input obligations are unchanged.

The iOS [HomeViewController](../../apps/openagents-ios/OpenAgents/HomeViewController.swift)
already consumes that function to prepare the cast's view matrix, then the
[Metal renderer](../../apps/openagents-ios/OpenAgents/Platform/HomeRenderer.swift)
binds it. The Haunted House controller also uses the function. This proves a
direct shared **component** use, without qualifying a complete application or
moving the hosts' pass construction into Aethyr. The browser Grid uses the `Render*` facade and its flat-stroke profile rather
than this native projection adapter. Recorded adoption of either component
does not qualify the other target or the complete playable route.

| Grid subsystem | Current implementation and public boundary | Still to migrate |
| --- | --- | --- |
| Browser frame and passes | `apps/openagents-web/newland.coax` → `newland-face.c` exports `coaxial_frame`, vertex/uniform extents. `coax/web/host.js` uploads and draws background/strokes; `verse/gpu/flat-pass.coax` describes its recorded Vulkan/browser comparison. | Shared `Render*` plans and browser adapter exist in this continuation; the complete current-source playable route remains to be qualified. |
| Browser material/light | `apps/openagents-web/gpu/newland.coax`, `.bindings`, `.module`: group0 uniform28vec4s, sans atlas and sampler, generated WGSL. | Shared binding, atlas, sampler and shader-layout admission exist in `Render*`; retain their exact profile and source qualification boundaries. |
| Native line/material/light | `verse/grid.coax`, `verse/home.coax`, `verse/light.coax`; `verse/platform/host.c` receives line/dot/slot packets; `HomeRenderer.swift` owns Metal scene/bloom/composite passes. | Shared GPU resource registry and pass submission, including refusal/cleanup after real allocation failures. |
| Figures | Native `verse/cast.coax` admits `Cast.bin`, poses and collision palettes; `HomeRenderer.swift` allocates buffers/textures. Browser `verse/figure.coax` supplies checked rounded-bone figures through `newland.coax`. | One admitted figure/material/binding interface; preserve interpolated feet and existing cast/figure data. |
| HUD | `verse/hud.coax`, `verse/hud/frame.coax`; reusable `aethyr/ui/{hud,draw}.coax` and the recorded HUD laboratory already exist. | Integrate those components; do not replace them with a second HUD implementation. |
| Asset/frame budgets | `aethyr/budget.coax`; `newland.coax` declares `LAND_BUDGET`; pack budgets and licences remain in `verse/packs/`/pack tooling. Cast/Haunt buffer gates admit their own binary layouts. | Content identity and generation ownership through real GPU allocation/use/release; writer-side reservation rather than silent clipping. |

Next: qualify the complete Grid background+stroke route and actual resource
lifecycle on matching declared browser/native profiles, preserving the four-word
ABI. Keep small independent render references, nonzero fragment-quad derivative
checks and exact source/compiler identities. Complete playable Grid adoption,
remaining realms, fixed-step/input/replay/feet continuity and live audio remain
separate exits.
