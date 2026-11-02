# 11. The WGSL renderer

> **Status: partly implemented.** Shared admission and the bounded CPU
> reference exist. Host and image qualification remains in progress.
> The shared Coax frame, binding, pass and resource modules now exist. Their
> checks and the new CPU reference are being qualified with the actual realm
> producers and both GPU hosts. This chapter is not yet a claim that every
> renderer acceptance check has passed.

An incorrect draw count can make a shader read past its vertices. A correct
count can still draw the wrong image if the browser and native host disagree
about a binding, a depth clear or a water pass. The renderer must agree on
both the bytes a frame may read and the order in which it reads them.

The realm writes its drawn state into buffers. Coax admits the measured
ranges, derives the geometry domains, and produces the pass operations.
The host creates resources and executes those admitted operations. Shader
arithmetic also stays in Coax: one checked procedure is emitted as WGSL for
WebGPU and SPIR-V for Vulkan.

[asset-profile.coax](../render/asset-profile.coax) also owns the delivery
rows. A pack expands into data, storage, index and layer deliveries with
one shared container URL. An ordinary index file keeps its own row. Each
host reports which rows are ready or failed; Coax derives whether the
frame may draw its scene, continue with its background, or wait. A host
does not infer readiness from the buffers it happened to create.

## Start with the existing shader source

The published CPU metadata interface in
[frame.coax](../render/frame.coax) also remains a public component.
[verse/grid-render.coax](../../verse/grid-render.coax) uses it before the
native Grid projection writes its sixteen matrix words, preserving output
on a short destination or a nonfinite viewport. That component's retained
[qualification](../fixtures/render/qualification.json) establishes symbolic
identity, range, refusal and cleanup behavior. Actual GPU allocation,
submission and lifetime have their own host qualification below.

The earlier G6 migration already replaced the hand-written shader source:
the 31 page WGSL modules are emitted from Coax. The generated `.wgsl` files
are build artifacts. This chapter brings their frame admission, resource
views, pass ordering and CPU reference into shared engine modules; it does
not repeat that source migration.

The catalog covers all 16 manifest rows, the direct Garden page, and the
retained Galaxy fixture. A profile records its identity, page, assets,
uniform extent, vertex stride, features and shader entry pairs. Retaining
Galaxy as a fixture does not put it back in the shipping manifest. Waterlab
has 3,937 uniform vec4s, or 62,992 bytes; Everglade has 21 logical bindings
across four WGSL groups. A small example is useful for teaching, but cannot
stand in for admitting those profiles.

## A draw still occupies four words

The published draw record is four `u32` words:

```text
[first, count, encoded_base_instance, instance_count]
```

A set high bit in `encoded_base_instance` selects a nonindexed draw. The
remaining 31 bits are its base instance. Without that flag the draw is
indexed. The pass is selected by the shared graph and its record range;
there is no fifth pass word. A zero count or zero instance count skips the
record, while a required clear can still take place.

All interval ends are exclusive. Admission widens the arithmetic before
adding `first + count` or `base + instances`. An indexed draw must fit both
the GPU's initialized index range and the actual supplied index payload.
Every referenced index is checked against its admitted vertex intervals.
An instance range must remain in its admitted prefix interval.

[geometry.coax](../render/geometry.coax) derives these domains from the
catalog profile and the actual typed resource views. Its encodings include
stored meshes, procedural ground and shapes, Myrealm's separate round
vertex range, Icecream's chunks and outline flags, and Everglade's packed
and skinned vertices. Skin checks include the pose range and every joint
byte, including a joint whose weight is zero. The direct HUD, grass,
particle, surface and caster counts have their own resource-derived caps.
A host-provided maximum is not a geometry receipt.

An empty shader path can still belong to a valid instance range. Everglade
reserves three spray slots at both river tiers. The desktop tier fits all
64 droplets into the first slot; the other two return a clipped vertex
before reading a particle. Admission must retain those bounded reserved
slots as well as the live droplet range. Deriving only the smallest visible
range rejected the desktop's water draw and left its page dark. The checks
now admit that actual draw and refuse its first instance past the declared
range, an excess index and a short water-node payload.

The host sometimes needs to stage a decoded asset beside the producer's
Wasm allocations. [host-memory.coax](../render/host-memory.coax) admits
that span too. It aligns the cursor to four bytes, checks the whole end
against the renderer's 64 MiB memory bound, and returns the absolute page
count needed by `memory.grow`. The adapter then grows and copies exactly
that admitted span. A failure preserves the old cursor and publishes no
address. An empty span still needs a valid aligned cursor.

The producer's arena must fit beside its state, static storage and stack.
Adding the renderer exposed an existing allocation constraint in the Grid:
its 8 MiB stack reservation left too little space for the vertex arena in
64 MiB. A conservative call-graph analysis of the actual optimized Wasm
bounds its stack below 2 MiB. The Grid build reserves 4 MiB, retains its
400,000 vertices, and checks the actual memory growth. The
[stack and allocation record](../../docs/verification/data/aethyr-ch11-2026-11-01/grid-memory-stack/qualification.json)
keeps the original refusal and the exact binary used for the analysis.

Some producer getters consume an update when read. If geometry then
refuses the frame, reading that getter again next frame may return zero.
[host-uploads.coax](../render/host-uploads.coax) keeps the copied prefix
pending, takes its union with later prefixes, and clears it only after an
actual submission is acknowledged. A rejected frame therefore cannot
silently lose the producer's last update.

The geometry work limit bounds inspected metadata and index/skin work. A
budget exhaustion refuses the frame. The current bounds are 48 resources,
4,096 draw records, 256 graph operations and 32 native bindings per pass;
the index and geometry work budgets are separate. Chapter 16 extends this
idea to broader realm budgets.

## Admit memory before making a view

The host first reports an upload's source allocation, source address,
requested bytes, GPU offset and allocation size. `render_uploads_admit`
checks every span before the host borrows or reads any Wasm payload. It
also checks alignment and addition overflow. Clipping a request with
`Math.min` would conceal a bad producer; it is not admission.

Only then can the host supply valid typed views to geometry admission.
`render_frame_admit` checks the complete frame with those derived domains.
A failure reports its status and offending upload, draw or direct count,
with zero admitted draws. No GPU write or draw should precede successful
admission of the complete candidate frame.

Consider eight allocated HUD records, each containing 16 f32 values:

```text
allocation = 8 × 16 × 4 = 512 bytes
used       = 6 × 16 × 4 = 384 bytes
source     = [4096, 4608)
upload     = [4096, 4480)
GPU write  = [256, 640), inside a 1024-byte allocation
```

The source numbers below are virtual addresses in a pure arithmetic
fixture. The program does not form or dereference those pointers. A real
host measures its allocation and obtains the geometry domains from the
actual profile and payloads.

The two commands each address three vertices and one instance. The first
reads indices `[0, 2, 1]`; the second uses the nonindexed flag. Changing the
last index to 3 refuses the first draw. Changing the later draw's first
vertex to 1 makes its range `[1, 4)`, outside `[0, 3)`. Both failures leave
the frame's admitted count at zero.

The [runnable example](../examples/admit-a-frame.coax) prints both refusals
and the admitted pass plan.

<!-- source: aethyr/examples/admit-a-frame.coax example -->
```jai
#load "../render/types.coax" as rt;
#load "../render/frame.coax" as frame;
#load "../render/passes.coax" as passes;
#import "Basic";

#test "the worked frame admits both four-word draw encodings" {
  result := worked_frame(0);
  #assert result.status == rt.RenderFrameStatus.ok && result.admitted_draws == 2 && result.checked_indices == 3;
}
#test "the worked frame refuses a bad index before admitting any draw" {
  result := worked_frame(1);
  #assert result.status == rt.RenderFrameStatus.bad_index && result.bad_draw == 0 && result.admitted_draws == 0;
}
#test "the worked frame refuses a later span before admitting any draw" {
  result := worked_frame(2);
  #assert result.status == rt.RenderFrameStatus.bad_draw && result.bad_draw == 1 && result.admitted_draws == 0;
}

worked_frame :: (fault: int) -> rt.RenderFrameResult {
  counts := rt.FrameCounts.{width = 320, height = 180, hud_vertices = 6, vertex_stride = 16, draw_count = 2};
  // These virtual addresses are numbers in a pure admission fixture. No
  // pointer is formed or read. A real host measures the source allocation.
  uploads := [1] rt.RenderUpload.[rt.RenderUpload.{
    source_address = 4096, source_base = 4096, source_bytes = 512,
    bytes = 384, gpu_offset = 256, gpu_bytes = 1024}];
  words := [8] u32.[0, 3, 0, 1, 0, 3, 0x80000000, 1];
  indices := [3] u32.[0, 2, 1];
  if fault == 1 { indices[2] = 3; }
  if fault == 2 { words[4] = 1; }
  // A real host gets these domains from render_geometry_admit and its
  // admitted typed payloads, rather than supplying its own maxima.
  domains := [2] rt.RenderDrawDomain.[
    rt.RenderDrawDomain.{vertices = 3, instances = 1, indices = 3},
    rt.RenderDrawDomain.{vertices = 3, instances = 1, indices = 3}];
  return frame.render_frame_admit(counts, rt.RenderFrameLimits.{},
    rt.RenderDirectDomains.{hud_vertices = 8}, uploads[..], words[..], domains[..], indices[..]);
}

main :: () {
  good := worked_frame(0);
  bad_index := worked_frame(1);
  bad_span := worked_frame(2);
  print("frame: %, admitted %, checked indices %\n", good.status, good.admitted_draws, good.checked_indices);
  print("bad index: %, draw %, admitted %\n", bad_index.status, bad_index.bad_draw, bad_index.admitted_draws);
  print("later span: %, draw %, admitted %\n", bad_span.status, bad_span.bad_draw, bad_span.admitted_draws);
  counts := rt.FrameCounts.{width = 320, height = 180, hud_vertices = 6, vertex_stride = 16, draw_count = 2};
  plan := passes.render_plan(passes.RenderFeatures.{scene = true}, counts, passes.RenderReadiness.ready, false);
  print("shared plan: %, % ops\n", plan.status, plan.count);
}
```

```shell
coax/bin/coax run aethyr/examples/admit-a-frame.coax
```

```text
frame: ok, admitted 2, checked indices 3
bad index: bad_index, draw 0, admitted 0
later span: bad_draw, draw 1, admitted 0
shared plan: ok, 5 ops
```

The five operations in this small ready graph are begin, background draw,
scene-record draw, HUD draw and end. The scene operation consumes the two
admitted records. A larger graph needs more operations and intermediate
targets; it keeps the same four-word records.

## Resource identity and resource views

[bindings.coax](../render/bindings.coax) names each logical resource and its
kind, layout and WGSL group/binding pair. Each pipeline has its own active
uses. Those uses map the logical resources to the reflected Vulkan slots,
kinds, shapes, strides and stage masks. A global Vulkan slot table would
be wrong: two entry pairs can encounter their resources in different
orders. A logical WGSL uniform is explicitly transported as storage in
the current SPIR-V path.

The declared view is as important as the allocation. Everglade's main
uniform uses 575 vec4s, while its particle shader uses only seven. A shared
9,200-byte allocation therefore needs a 112-byte particle view. Giving
Vulkan the whole allocation changes the runtime array length seen by that
shader. The CPU reference uses the same admitted view length.

[binding-sources.coax](../render/binding-sources.coax) also declares each
resource's origin: app or media uniforms, decoded storage or mesh assets,
numbered realm stores, particle emitters, layers, atlas, sampler or render
target. Bloom's step uniform comes from the current operation; its source
image comes from that operation's declared target. These are explicit
roles, so the JS/C adapters do not rediscover their meaning from a name or
slot.

[resources.coax](../render/resources.coax) distinguishes declared uniform
extent, immutable asset payload and the existing stores' capacity views.
The last mode preserves their historical 16-byte minimum and padding.
An upload of zero bytes can mean that an earlier upload remains current;
it does not imply an empty view. Upload history, initialized highwater,
raw source capacity, physical allocation and logical view are recorded
separately. Padding a height buffer never creates another authored grid
vertex: geometry uses its declared dimensions and checks that the view
contains at least those points.

A stateful producer getter is read once at its scheduled point. Some
producers consume an upload retry when their byte getter is called. A
rejected candidate must retain the pending upload for a later attempt;
calling that getter again to log its value can change the program.

The Sans atlas also has one description in Coax:
[sans-profile.coax](../render/sans-profile.coax) declares its 111 glyphs,
sixteen columns, 96-pixel cells and five mip levels. It gives the platform
the exact glyph positions at each level, including the final 4.5-pixel
baseline. The platform rasterizes those commands with its font API. The
offline atlas tool reads the same commands. The extraction preserves all
1,374,912 alpha bytes of the previous atlas; a GPU upload still needs its
own check of the resulting linear, premultiplied texture.

For the Grid and retained Galaxy, [sans-filter-core.coax](../render/sans-filter-core.coax)
also owns the bilinear weights and the blend between mip levels.
[sans-filter.coax](../render/sans-filter.coax) loads the four texels of each
level explicitly. Both use the canonical atlas dimensions and five-level
layout. The old sampler parameter stays bound to preserve the shader ABI;
these paths use the shared arithmetic instead of device filtering weights.
Independent WebGPU and Vulkan probes check the arithmetic and wrong-size
refusal. Complete frame comparisons check its use by the real shaders.

The resource descriptions also specify target formats and extents,
texture filterability, sampler address/filter/mip/compare behavior and
binding ranges. The sheet's eight-tap footprint is explicit shader
arithmetic with an admitted isotropic hardware sampler. Merely lowering
a sampler's anisotropy setting would change the filtering contract.

## The pass graph is data

[passes.coax](../render/passes.coax) returns a bounded operation sequence.
The features in the catalog select the graph. The host does not inspect
WGSL with a regular expression to guess whether a shadow or water pass
exists.

The sequence describes background, shadow and caster depth, ground and
scene records, water copies, each water draw's transmit/emit pair,
particles, multiple render targets, five bloom steps, post and HUD. Its
states carry depth comparison and writes, load/store policy, bias, color
and alpha blending, and auxiliary target masks. Water's two operations
remain interleaved per draw; drawing every transmit first would change
the result.

A physical attachment and a shader output are different things. Water and
particles can keep two attachments in their render pass while returning
only one color: the second attachment's write mask is empty.
[fragment-layout.coax](../render/fragment-layout.coax) admits that case in
both package validation and the CPU pipeline. It still requires both
outputs when both attachments are writable. Requiring two shader outputs
unconditionally would reject the existing water and particle programs.

A declared shadow map is cleared even if this frame has no mesh records.
Grass or another consumer can still read it. A graph that omitted this
clear could reuse last frame's depth. A skipped command and a skipped
pass are different decisions.

Readiness is explicit. `loading` admits no effects. A profile waiting for
scene assets can continue its background, post and HUD path. `ready`
permits the admitted scene work; particle readiness is separate. These
states prevent an incomplete bind group from becoming an accidental
control-flow rule in each host.

## A CPU reference needs fragment neighbors

A scalar call to a shader procedure is useful for ordinary arithmetic.
It cannot supply the neighboring values required by `dpdx`, `dpdy`,
`fwidth` or implicit texture LOD. The earlier scalar derivative bodies
returning zero are not an image oracle for those shaders.

The new reference has three layers:

- [reference-ir.coax](../render/reference-ir.coax) executes the checked
  shader tree with bounded instructions, calls, storage views and control
  flow. [reference-io.coax](../render/reference-io.coax) reads its stage
  entries, packed parameters, attributes, flat varyings and depth output.
- [reference-quad.coax](../render/reference-quad.coax) executes four fragment
  lanes. Helper lanes participate in derivatives even when they do not
  publish a pixel; discard changes publication. Texture views include
  their layers and mip levels, and samplers state their address, filtering,
  LOD and comparison rules.
- [reference-raster.coax](../render/reference-raster.coax) clips triangles,
  applies top-left edge ownership, interpolates varyings and evaluates
  covered quads. It applies the declared depth, blend and target-format
  behavior, including half-float or unorm storage conversion.

Framebuffer precision is an explicit reference profile. The measured
Apple WebGPU and NVIDIA Vulkan devices both used eight fractional position
bits, with different halfway rounding. A profile records that choice and
whether XY clipping uses the viewport planes or a bounded guard region.
An unsupported profile is refused. The measurements qualify those devices;
WebGPU does not promise one universal subpixel rule. See its
[rasterization rules](https://www.w3.org/TR/2026/CRD-webgpu-20260915/#rasterization).

Storage precision also matters. One unorm8 step is `1 / 255`, about
0.00392, which exceeds the 0.002 absolute comparison floor below. Vulkan
permits either adjacent integer when converting a nonintegral scaled value
to fixed point; nearest rounding is a recommendation. Its
[conversion rules](https://docs.vulkan.org/spec/latest/chapters/fundamentals.html#fundamentals-fixedfpconv)
therefore do not establish exact equality with the CPU's selected byte.
The strict point comparison remains unchanged. Matching rgba16f runs
separately test the shader arithmetic, while the recorded unorm failures
still need their format and golden-image evidence. A passing float run
does not rename an earlier byte-image failure as a pass.

The CPU pass driver joins those layers to the same pass and resource
plan. It refuses malformed trees, unsupported operations, unavailable
resources and exhausted budgets. It does not replace an unsupported
operation with zero and call the resulting image a reference.

[reference-work.coax](../render/reference-work.coax) names the offline work
profiles. `standard` keeps its 100-million-step pass limit. `large-world`
admits one billion steps per pass; `complex-world` admits three billion.
Every individual IR invocation still has the original 100-million-step
ceiling. Pixel, geometry and scratch-memory limits are separate. A caller
selects a profile explicitly, and the measurement records its actual limits
and consumed work. The tooling also keeps a wall-clock deadline.

These limits matter on real scenes. The Garden's 160×90 reference frame
used about 779 million shader steps, including about 752 million in its
scene pass. Reducing the viewport cannot remove all vertex work. Its
`standard`-profile refusal remains a refusal; the separately selected
`large-world` run supplies the full image comparison. Such a work receipt
does not promise a GPU frame rate.

The package carries the actual checked CPU trees alongside WGSL and
SPIR-V. Each product has a content hash and a relative sibling filename.
The Coax package reader binds the manifest to the expected catalog
profile, admits its reflected interfaces and enabled capabilities, and
checks each fetched product's bytes before use. The bounded text decoder
then validates the whole tree before writing caller-owned word storage.
The CPU executor declares a separate `reference` backend with its own
implemented helper and integer capabilities.

A realm page carries its compiled catalog index in `data-render-profile`.
This identity stays with the base renderer when a fork changes its realm
or save-slot name. Parsing a self-consistent manifest is insufficient: the
host must admit it against that expected profile. Candidate JSON and entry
storage occupy an inactive bank, so a failed replacement leaves the active
package's borrowed strings and records alive.

The native build records the same expected identity in a generated header:

```shell
verse/native/build.sh watchtower /tmp/watchtower --render-profile watchtower
```

The Coax builder writes the complete package into `/tmp/watchtower/render`.
`RenderIdentity.h` names its compiled profile and manifest hash; it does
not infer a renderer from a saved realm's name. The producer and renderer
facade share one physical source graph and generated header. The Grid's
existing homepage reader owns its SHA kernel, so its facade joins that
unit. Emitting another private copy would create conflicting C exports.

Ordinary catalog Wasm builds include the facade. The native launcher reads
`build.coax --app-table`, stages the complete package and Coax-selected
assets, and executes the same admitted plan. The platform reports the
actual allocation behind an asset destination, including a face's static
catalog buffer. Coax checks that extent before a copy. A native pack is
loaded from the compiled profile's staged URL.

Native layer and font staging reuses the existing Coax decoders and Sans
rasterizer. The generated `textures.config` records formats, dimensions,
layers, mip offsets and pitches; it is published after the raw files.
Its legacy image-construction profile has its own byte-preservation checks.
Browser resize and image-decoder parity is a separate claim.

## A picture is another admitted draw

The producer's media text remains a small line format:

```text
image /picture 10 20 210 120
over /caption 12 24 208 116
```

[media-commands.coax](../render/media-commands.coax) reads the first 112
lines, including empty or invalid lines. It preserves each accepted row's
original line number: line 111 is a valid image context even though a
shader has at most 48 logical resource slots. Invalid rows are skipped;
later lines are explicitly reported as truncated. Parsing first counts
the accepted rows, then writes them only when caller storage is sufficient.
The accepted URL borrows the retained text.

The same module computes the centered aspect fit and eight f32 uniform
words. A pending image, or a box at most one pixel wide or high, produces
no draw. Static image dimensions are bounded to 2,048 pixels per side;
video dimensions retain their measured size. Image decoding and upload are
platform effects. Row selection, resizing dimensions, rectangle arithmetic
and the before-HUD or after-HUD phase belong to Coax. The independent native
gate compares the transported records and exact uniform bits with the
existing JavaScript calculations for authored whole-pixel rows and selected
exact decimal inputs.

The ready image observations retain their actual kind, URL and dimensions.
An existing texture called `media_image-9` does not prove that this frame's
line 9 URL is ready. [host-media.coax](../render/host-media.coax) matches
those observations against the parsed requests and publishes rectangles
only after the whole candidate is admitted. Raw native integers are
checked before conversion to a kind or readiness flag, so a value such
as 257 cannot wrap into a valid image kind.

The native renderer consumes decoded physical image observations supplied
through its raw texture input. A pending remote image stays pending until
its host supplies that image. The captured memorial fixture qualifies the
renderer with a real decoded image; it does not establish asynchronous
native HTTP image acquisition.

Texture bytes also need a declared interpretation. The actual sheet arrays
are `rgba8unorm-srgb`; the Sans atlas and media images are linear unorm.
[texture-codec.coax](../render/texture-codec.coax) applies the sRGB EOTF to
RGB before filtering and leaves alpha linear. It also decodes byte order,
finite half/float colors and depth, with explicit row, layer and mip spans.
Filtering encoded byte values and decoding the average afterward would
produce a different image. Its malformed or nonfinite input leaves both
caller-owned decoded output views unchanged.

Shader reflection uses schema version 2. The checked tree keeps version 2
for existing operations; programs using the new GPU operations require
version 3, and lowerers refuse those operations under version 2. These
are separate versioned contracts, not an in-place reinterpretation of an
old tree.

The live catalog shaders now use [noise.coax](../render/noise.coax), rather
than amplifying a platform's last `sin` bits into different noise. Coordinates
are quantized at 1/64 after explicit saturation to the signed32 conversion
range. The permutation wraps in `u32`; only its high 24 bits become an exact
f32 fraction. Ordinary artistic waves and trigonometric motion remain.
The Garden's retained image comparison isolates this change. With identical
uniform, HUD and draw bytes, putting the new shader in the old WebGPU pass
executor produces the new renderer's image exactly. A diagnostic Coax variant
that restores only the old sine hash produces the old image exactly. At
320×180, the hash change affects 47,333 pixels, with a maximum byte difference
of 25. The record keeps both images and both compiler-emitted variants; this
is evidence for the visible noise change, not an image-tolerance pass.
The painterly fixture has a separate [migration record](../../docs/verification/data/aethyr-ch11-2026-11-01/image-goldens/fixture/noise-migration.json).
Its four clock keys, four phone views and shaft view retain their original
screenshots beside the new ones. Restoring only `fixture_hash` and
`verse_style_hash` to their old formulas makes all nine scenarios pass their
original golden policy. Putting the current integer shader in the old
executor makes all nine pass against the reviewed new baselines. The noon
control also compares complete raw images and identical producer bytes.
The visible change is in cloud shapes, crown gaps and grass placement;
the camera, HUD, palette and lighting keep their layout. Both reviewers
inspected every pair before the nine PNG baselines changed. The existing
six-block screenshot allowance remains unchanged; numerical image
comparisons still use the separate strict per-channel rule.

The Fairy Forest's [migration record](../../docs/verification/data/aethyr-ch11-2026-11-01/image-goldens/fairy/noise-migration.json)
uses the same two controls. Restoring only its two sine hashes reproduces
the old raw image and passes the original saved screenshot with zero changed
blocks. The integer shader in the old executor reproduces the shared
renderer exactly. Both reviewers checked the retained pair before its
baseline changed: framing, the fairy, paths, bridge, palette and HUD keep
their layout; canopy and ground patterns change. Both executors then pass
the reviewed baseline with zero changed blocks under the original policy.

This is a finite-coordinate rendering profile. Its scalar nonfinite-zero
fallback is not GPU input admission, and frame span checks do not establish
that every shader value is finite. Likewise, Coax's native `int` is not a
promise of arbitrary 64-bit integer arithmetic in WGSL's 32-bit profile.

Changing the noise also exposed a real dust containment gap: seed 2,
particle 88 at `88 * 0.37` seconds reached about -0.3804222 m, beyond the
existing -0.3 m lower bound. The vertical dust offset now keeps that bound;
the witness and the original room-containment check both remain. This
visible fix belongs in the image comparison record alongside the noise
change.

## What counts as evidence

A passing frame check establishes admission, not GPU image parity. A
successful WGSL compile establishes shader validity, not the pass order.
The release record must keep those claims separate:

- Sharp checks and zero-survivor mutation for frame, geometry, bindings,
  graph, resources, emitter admission and CPU reference decisions.
- An independent inventory of every manifest profile and retained fixture,
  with per-pass reflection checked against the declared uses. Normal-C
  producer fixtures use the actual retained payload bytes, including
  nonempty meshes, water, grass, particles and the large uniform profile.
- Both hosts consuming the shared plan and resource views, all emitted
  WGSL compiling headlessly, and SPIR-V plus Vulkan validation reporting
  their actual errors and warnings.
- The checked CPU pass, WebGPU and Vulkan compared with matching resources
  and formats. The finite color rule is
  `abs(gpu - cpu) <= 0.002 * max(1, abs(cpu))`. Nonfinite values fail even
  when both sides contain them. Edge/depth uncertainty has its own stated
  mask and count; it does not excuse arbitrary interior color errors.
- Each shipping page's golden compared with its previous image. A visible
  fix must have its own before/after evidence; an image tolerance is not
  permission to replace the baseline.

The normal-C catalog inventory checks identity, assets, every logical
binding and entry pair, and every active use. The geometry producer gate
checks wire ranges, actual indices, all six direct caps and atomic refusal.
Neither certifies the truth of an asset's BVH or proves arbitrary shader
arithmetic. The GPU/CPU and host-adoption checks supply the remaining
renderer evidence. Until those final gates pass, the status at the top of
this chapter remains in progress.

The image comparison tool reads complete raw images, including alpha. It
decodes the declared byte, half or float storage format and checks every
channel against the CPU's f32 output. A matching pair of NaNs still fails.
Its bounded diagnostic sample does not bound the failure count: every
pixel is inspected. Its initial gate uses no exclusion mask; any later
edge or depth mask needs its own stated oracle and count.

## Dependencies

Chapter 1 supplies the drawn state; chapter 2 fixes camera and screen
spaces. Chapters 8–10 supply ground, a moving character and a following
camera. The shader library in [verse/gpu/shader.coax](../../verse/gpu/shader.coax)
and [verse/gpu/wgsl.coax](../../verse/gpu/wgsl.coax), Coax's
[GPU chapter](../../coax/book/ch17-gpu.md), and Psionic’s [compute subset](../../psionic/book/ch037-a-coax-compute-subset.md)
provide the checked shader operations and target lowering. Coax’s R9
target interfaces and R10 browser slice supply the host boundaries.
The earlier [native GPU record](../../docs/verse/native-gpu.md) explains
G0–G6; it is historical evidence for the shader migration, not a receipt
for this chapter's new shared renderer.
