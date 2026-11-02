# Bounded frame admission

`frame.coax` is a CPU metadata contract used by the Grid's native projection
boundary. It does not submit GPU work. The complete renderer and playable-route
migration in [#12628](https://github.com/OpenAgentsInc/openagents/issues/12628)
remain open.
The [qualification](../fixtures/render/qualification.json) retains focused
checks, all120 killed policy mutants, independent native observations,
compiler/source identities and the failed check-first/authoring attempts.

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
rasterization, blend/depth/rounding and fragment-quad derivatives remain open.

## One complete CPU trace

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

## Actual consumer and remaining adapters

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
moving the hosts' pass construction into Aethyr. The browser Grid uses a
different flat-stroke renderer and has not adopted this new contract.

| Grid subsystem | Current implementation and public boundary | Still to migrate |
| --- | --- | --- |
| Browser frame and passes | `apps/openagents-web/newland.coax` → `newland-face.c` exports `coaxial_frame`, vertex/uniform extents. `coax/web/host.js` uploads and draws background/strokes; `verse/gpu/flat-pass.coax` describes its recorded Vulkan/browser comparison. | Shared admitted browser/native pass and resource plans; no host changes in this slice. |
| Browser material/light | `apps/openagents-web/gpu/newland.coax`, `.bindings`, `.module`: group0 uniform28vec4s, sans atlas and sampler, generated WGSL. | Typed common material/light records, atlas/texture/sampler admission and compatible shader layouts. |
| Native line/material/light | `verse/grid.coax`, `verse/home.coax`, `verse/light.coax`; `verse/platform/host.c` receives line/dot/slot packets; `HomeRenderer.swift` owns Metal scene/bloom/composite passes. | Shared GPU resource registry and pass submission, including refusal/cleanup after real allocation failures. |
| Figures | Native `verse/cast.coax` admits `Cast.bin`, poses and collision palettes; `HomeRenderer.swift` allocates buffers/textures. Browser `verse/figure.coax` supplies checked rounded-bone figures through `newland.coax`. | One admitted figure/material/binding interface; preserve interpolated feet and existing cast/figure data. |
| HUD | `verse/hud.coax`, `verse/hud/frame.coax`; reusable `aethyr/ui/{hud,draw}.coax` and the recorded HUD laboratory already exist. | Integrate those components; do not replace them with a second HUD implementation. |
| Asset/frame budgets | `aethyr/budget.coax`; `newland.coax` declares `LAND_BUDGET`; pack budgets and licences remain in `verse/packs/`/pack tooling. Cast/Haunt buffer gates admit their own binary layouts. | Content identity and generation ownership through real GPU allocation/use/release; writer-side reservation rather than silent clipping. |

Next: migrate the browser Grid's actual background+stroke bindings and host
resource lifecycle, then its native counterpart, preserving the four-word ABI.
Retain small independent scene/render references and headless-browser/native
observations for that same declared profile. A nonzero fragment-quad oracle is
required before admitting derivative-dependent shaders. Complete playable
Grid adoption, remaining realms, fixed-step/input/replay/feet continuity and
live audio qualification stay separate exits.
