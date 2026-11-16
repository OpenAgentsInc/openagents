# GPU composition v1

The shared opt-in renderer for images, affine isolated groups, group opacity,
source blur, and backdrop blur with sharp children (#13645). The installed
iOS app exposes its specimens at **Settings → Renderer**. Ordinary app pages
continue using their existing flat renderer and native active text editor.

`coax/native/gpu-composition.coax` owns the pass plan and resource admission, reusing the
existing `RenderRegistry`, now shared under `coax/lib/gpu/` with the engine. `composition-shader.coax` loads the
existing component shader and adds generated entry points `uc_vs` / `uc_fs`.
Shader arithmetic is Coax on Metal, WebGPU and Vulkan. The SDK adapters only
allocate, upload, bind, encode and submit.

## Component API

Use `uiframe_layer(f, [a,b,c,d,tx,ty], opacity, blur, backdrop)` and
`uiframe_unlayer(f)` around a component. Its under and over paint are gathered
into one isolated group, so overlapping children fade once. Its semantic rows
follow the same affine. A transform maps `(x,y)` to
`(a*x+c*y+tx, b*x+d*y+ty)`; encode an origin into the translation. Groups and
clips nest to the existing sixteen-level semantic limit. Singular or
nonfinite transforms refuse the candidate.

The direct display-list API is `prim_layer` / `prim_unlayer`.
`PRIM_ALPHA` remains the old per-primitive opacity operation. `PRIM_IMAGE`
names an immutable image descriptor supplied to `uc_build`; its rectangle
scales the full image, and the ordinary nested clip supplies cropping.
Canonical PNG/JPEG bytes come from the existing `htm_plan` / `htm_decode`
in `metaverse-engine/render/host/host-image.coax`. Do not add an SDK decoder.
Procedural images may supply authored straight RGBA8 directly.

`uc_build` lowers through the existing `gpu_draw`, emits the same 28-float
vertices plus the new texture/filter kinds, and returns a complete `UcPlan`.
A false return publishes no plan. Supply at least one spare primitive/vertex
block: capacity exhaustion must refuse, rather than truncate labels.

## Pass and lifetime contract

The pinned [profile](../../proto/components/composition-v1.json) and its
[registry row](../../proto/registry.json) define eight-float pass records.
`coax/native/tests/gpu-composition-profile.coax` compares exact golden records with the encoder.
Old vertex/shader entry points and the world renderer's AFP1 are unchanged.

Targets are transparent, premultiplied RGBA8 in the existing non-sRGB blend
space. Ordinary primitives and canonical straight image texels are
premultiplied by the shader; image texels are premultiplied **before** bilinear
interpolation. Composition uses One / OneMinusSourceAlpha. Blur is two bounded
normalized Gaussian passes over premultiplied pixels with transparent borders.
Backdrop capture samples only the already-painted parent, before the group's
sharp children. A pass never samples its own attachment. No interactive frame
reads pixels back to the CPU. Filter draws cover conservative source bounds
and the kernel halo; backdrop capture includes the clip's sampling margin.

Every target is a complete local viewport: blur can extend beyond a primitive
within that viewport; the local canvas itself clips. Transforms operate on
that completed canvas. An unbounded retained scene outside the local canvas
is not part of v1. Maximum dimensions are 4096×4096, sixteen targets and images,
4096 ordered passes, 64 distinct target/source/load binding sets, 32 device-pixel
blur radius, and 128 MiB of effect targets, source images and vertex buffers.
The existing glyph atlases and output drawable remain separately owned by
their established host. These are conservative bounds, not a performance
promise at every maximum simultaneously.

The host reserves a presentation opportunity and waits for the previous
immutable frame before invoking a producer. It creates all resources before
encoding a prefix. Image generations retain immutable CPU bytes and upload
only on change; a new host/device replays them. Sibling layers reuse scratch only after their last encoded read, keeping
the phone specimen at four targets rather than eight. The budget charges the
full allocated vertex capacity. Targets are reused at unchanged
dimensions and retired when leaving the composed path. Static glyphs remain
in the existing atlas during opacity/transform motion. Hosts include pass and
image changes in redraw invalidation. Web callers reserve before producing,
keep their vertices/images immutable until `submit` completes, and recreate
the adapter after device loss.

The phone's composed screen stages its semantic generation. The completed GPU
frame acknowledgement publishes it; SDK refusal leaves the last generation
in place for input, VoiceOver and driver readers.
Transformed rows use clipped convex polygons for hit testing and enclosing
visible boxes for accessibility. Native fields may translate but cannot yet
rotate, shear or scale; selectable lines inside isolated groups also refuse.
This prevents an editor or selection overlay from diverging from its pixels.
Reduced motion settles the animated specimen; reduced transparency disables
blur and uses opaque group opacity.

## Qualification and inspection

`coax/native/tests/gpu-composition-fixture.coax` feeds the same scene into native SDK and browser
pixel oracles. The six cases check isolated overlap opacity, transformed
clipping, source blur halo, backdrop blur with a sharp child, transparent
RGBA image interpolation, and translucent overlap over a gray destination. `composition-oracle.c` uses actual offscreen SDK
readback exclusively for qualification. `ui_readback` is not a frame path.

The web inspector is `components/gallery/composition.html`, served from the
repository root. Generate the fixture with:

```sh
coax/remote/remote.sh run -- coax/web/wasm.sh coax/native/tests/gpu-composition-fixture.coax components/backend/composition-face.c build/composition/fixture.wasm
coax/bin/coax wgsl components/backend/composition-shader.coax --vertices uc_vs --fragments uc_fs --bindings components/backend/composition.bindings -o build/composition/composition.wgsl
coax/web/serve.sh . --static --listen 127.0.0.1:9027
```

Then use the shared headless `shot` tool on
`http://127.0.0.1:9027/components/gallery/composition.html`.
The page exposes `window.compositionResults` with six pixel results and SDK
validation errors. It renders GPU specimens inside a normal HTML document.

World texture/depth borrowing, production desktop migration, Android SDK
qualification, animated image playback and transformed active-text selection
remain separate work in the broader #13645 port.
