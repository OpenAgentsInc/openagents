# Rendering and live shaders

Status: proposed specification. The current browser renderer is owned TypeScript
over direct WebGPU, with WGSL shaders and compiled asset inputs.
[Decision 0006](decisions-history/0006-cross-platform-engine-core.md) moves it, one pass
at a time, into the Rust core for web and native builds;
[decision 0007](decisions-history/0007-gpu-driven-rendering.md) selects the GPU-driven
target design. This document makes
no measured performance claim. [Architecture](architecture.md) owns the stack
decision; [platform support](platform-support.md) owns device qualification.

The baseline uses the runtime's single preview worker for simulation and GPU
command encoding, keeping kernel batches and presentation scheduling together.
An isolated script worker and compiler workers remain separate. A separate
render worker is an optional measured optimization, not the default topology.

React DOM, Effect Atom, and Effect 4 provide the surrounding editor frontend.
The preview worker retains the simulation, camera, physics, animation, and GPU
scheduler. React renders, component effects, and Atom subscriber notifications
never gate a frame, start another frame loop, or own GPU resource lifetimes.
UI interaction enters through the trusted broker as typed commands or bounded,
coalesced input. Engine-origin summaries return through a bounded bridge;
render transforms and GPU state do not become a per-frame Atom graph.

## Medieval Town implementation plan

The [Town renderer redesign](town-renderer-redesign.md) selects compiled spatial
batches and specialized material families as the first implementation phase,
followed by GPU visibility/light assignment, dependency-correct pass reuse,
bounded frame delivery, and full qualification. It preserves clustered forward
shading, the current visual profile, semantic edit identities, and compatible
generation activation. The plan is unimplemented; the prior Town phase did not
achieve sustained smooth 60+ FPS. [Issue #8](https://github.com/OpenAgentsInc/aethyrforge/issues/8)
tracks the phased work without scheduling an implementation run.

## Capability profiles

Startup requests an adapter, inspects supported features and limits, and chooses
a named renderer profile. It requests only required limits and optional
features actually used by that profile. Scene admission checks buffer sizes,
texture dimensions, bind-group counts, and compute workgroup limits against
the resulting device, rather than assuming a desktop GPU. These limits are
part of the [WebGPU device contract](https://gpuweb.github.io/types/interfaces/GPUSupportedLimits.html).

The initial supported product requires WebGPU. Failure displays a clear
compatibility result and preserves project access and export. There is no
silent WebGL renderer with different scene semantics. Optional timestamp
queries, compression formats, and higher-quality effects have independent
capability checks. Shader source cannot silently require a feature absent
from a release's declared minimum profile.

## Render graph

An owned render graph describes passes, resource reads and writes, formats,
sample counts, dimensions, and lifetime. Graph compilation rejects missing
producers, cycles, incompatible attachment use, and reads before initialization.
It creates a plan with explicit WebGPU usages and a pool of reusable textures
and buffers. A resource pool reuses an allocation only after its previous work
has completed and all readback consumers have released it; WebGPU handles
backend transitions rather than exposing arbitrary native barriers.

The baseline path is depth and shadow preparation, visibility compute,
clustered light assignment, opaque forward shading, transparency, selection,
and post-processing. UI composition remains separate from scene tone mapping.
Linear working color, sRGB sampling/output rules, alpha conventions, exposure,
and tone mapping are versioned material/render-profile contracts. Transparent
objects use a documented sorting and blending policy; opaque culling must not
incorrectly remove them.

Advanced effects, including volumetrics, reflections, or ray tracing
approximations, are replaceable graph modules with measured quality and cost.
They do not become mandatory prerequisites for placing a prop. Temporal passes
invalidate their history after camera teleports, viewport changes, topology
activation, or incompatible material changes. Dynamic resolution changes only
presentation quality and does not change physics or authored units.

## Sky, lighting, and atmosphere target

The [Medieval Town reference plan](references/medieval-town.md) establishes the
first detailed environment target from 18 inspected gallery images. Its
[shot guide](references/medieval-town-shots.md) distinguishes observed appearance
from proposed reconstruction. Reference matching requires scene/layout and
material fidelity together with these rendering systems:

- Authored daytime and nighttime sky, sun/moon placement, clouds, and coherent
  diffuse/specular environment lighting.
- Linear-light PBR and HDR intermediates, explicit exposure/white balance,
  versioned filmic output, and controlled bloom with preserved surface detail.
- Stable directional shadows, near-surface contact/occlusion, alpha-tested
  foliage shadowing, and practical lights with real falloff and shadow budgets.
- Indirect-light treatment for eaves, recesses, and shaded streets, with explicit
  dependencies for any precomputed probes or lightmaps.
- Depth/height-dependent aerial perspective plus bounded local haze/light beams
  where the target requires them. A screen-wide color wash does not establish
  atmosphere or light transport.
- Transparent fountain water, bounded cosmetic effects, and motion-stable fine
  geometry/foliage/transparency under a declared antialiasing profile.

`environment.v1` and `light.v1` own the authored settings described in
[game systems](game-systems.md). Debug views separate direct light, indirect
light, material response, shadows, atmosphere, and display conversion. Lighting
constants use the fast data-update path; interface/resource changes stage a
compatible candidate. React/atom updates never schedule those render passes.

Source Lumen/Nanite use does not prescribe Aethyrforge's algorithms. Qualify
results and cost rather than assuming a technology name establishes parity.
Start with material/lighting calibration, then the full environment. Shadows,
volumes, clouds, and history buffers count toward the declared resource profile.

A changed wall, material, light/environment/sky, or bake setting invalidates
affected precomputed illumination. Its dependency hashes must match the active
scene configuration. An explicit
dynamic-preview profile may run while optional bakes rebuild; a profile requiring
complete indirect data holds activation. Final reference captures require current
inputs and the named reference profile. The transition cannot expose stale bakes
as completed lighting or mix geometry/collision generations.

## Geometry and GPU work

Compiled geometry is indexed, partitioned into bounded clusters, and referenced
by stable mesh and material identities. Repeated assets share immutable vertex
and index buffers. Instance tables carry transforms, material overrides,
selection IDs, and animation references. Moving one object uploads its changed
instance data; it does not rebuild its mesh or every other instance.

CPU cell visibility provides a conservative first cut. GPU compute performs
frustum and optional occlusion culling, selects qualified levels of detail, and
builds bounded indirect argument lists. The renderer uses a fixed set of
indirect draws grouped by pipeline and material compatibility. WebGPU provides
indirect drawing, but the design must work without assuming arbitrary bindless
textures or a portable indirect draw-count extension.
[WebGPU render command reference](https://gpuweb.github.io/types/interfaces/GPURenderPassEncoder.html)
defines those draw operations. Unsupported optimizations fall back to bounded
CPU command preparation within the same WebGPU renderer.

LOD uses projected error with hysteresis; fast camera motion cannot oscillate
between variants every frame. Skinned meshes and animation bounds have
conservative culling volumes. A missing high-detail page uses a lower-detail
resident variant. A mesh page never supplies collision; the matching simulation
generation owns that decision. GPU particles are cosmetic unless a CPU rule
explicitly represents their gameplay effect.

## Streaming and residency

The renderer consumes the compiler's cell manifests and content hashes. It
prioritizes the current view, likely movement corridors, nearby editable
objects, and required low-detail coverage. Fetch, decode, transcode, and upload
have separate bounded queues. Upload work yields to interactive frames.
Integrity failure discards the candidate and reports the affected asset;
corrupt bytes cannot enter a live buffer.

The renderer accounts for geometry, mip levels, render targets, staging,
readbacks, and temporary candidate resources. It uses profile budgets rather
than trusting an unavailable exact free-VRAM measurement. Eviction respects
active references, queued work, and editor pins. A topology candidate reserves
capacity before activation; if old and new generations cannot coexist, it
needs a controlled pause with an explicit restart plan. Unbounded streaming
is not an acceptable response to a creator's unlimited world.

## Shader and material replacement

A shader edit carries source hash, expected active generation, material identity,
compiler/profile versions, and expected interface signature. Compilation and
pipeline creation happen off the active path. The system gathers compilation
diagnostics, creates the candidate pipeline asynchronously, captures validation
errors, and checks bind layouts, vertex inputs, render targets, and required
features. WGSL distinguishes stages of shader validation and execution;
successful text parsing alone does not establish a valid pipeline.
[WGSL shader lifecycle](https://www.w3.org/TR/WGSL/#shader-lifecycle)
defines these stages. [GPUDevice](https://gpuweb.github.io/types/interfaces/GPUDevice.html)
exposes asynchronous pipeline creation, error scopes, and device-loss reporting.

A compatible candidate swaps at a frame boundary, keeping the same simulation,
camera, and compatible material parameters. A newer edit makes older pending
results stale even if they finish later. An interface change stages its required
buffers and bindings as one generation. Failure retains the active pipeline
and reports a source-linked diagnostic; it never resets the player to make
compilation easier. An intentionally broken draft remains available to edit.

Pipeline caches are keyed by shader and interface hashes, render state, feature
profile, and engine compiler version. The browser owns backend compilation;
the engine does not promise portable serialized GPU pipeline binaries or a
fixed compilation time. A valid shader can still be expensive: trusted preview
supervision, workload bounds, and recovery apply to custom GPU code.

## Device loss and lifecycle

Device loss invalidates every object belonging to that device, including the
previous working pipeline. The supervisor stops submissions, retains CPU world
state and content references, obtains a supported replacement device, rebuilds
resident resources, and resumes from the same compatible runtime checkpoint.
If recovery fails, the editor remains usable and reports that preview must
restart. Shader failure and device loss are different outcomes.

Each GPU resource has one owner, a generation reference count, and a disposal
path. Resize destroys superseded size-dependent resources after queued use.
Capture performs bounded asynchronous readback; normal rendering never waits
for a synchronous GPU read. Disposal releases workers, buffers, textures,
queries, event handlers, and pending candidates.

The project-scoped Effect 4 service supervises this lifetime. Opening or closing
React panels only changes subscriptions and controls. Viewport resize or a
replacement surface sends an explicit attachment command; it does not rebuild
the scene or reset camera state as a consequence of a component render.
Project shutdown stops submissions, cancels candidates, releases attachments
and broker ports, then disposes resources with a bounded completion deadline.
The isolated preview/player origin and script-worker boundaries remain those
in [runtime](runtime.md) and [security](security.md).

## Qualification

Required evidence covers missing features, low limits, loss during shader
compilation, out-of-order candidates, repeated resize, streaming cancellation,
occlusion correctness, LOD transitions, texture color correctness, and resource
stability after thousands of edits. CPU submission and GPU pass times are
separate measurements. [Performance](performance.md) and
[verification](verification.md) own workload profiles and acceptance evidence.

- **RENDER-01:** Invalid and stale shader candidates preserve the last working
  pipeline, camera, and compatible simulation state.
- **RENDER-02:** A supported device loss rebuilds resources from CPU state;
  unrecoverable loss preserves editor access and diagnostics.
- **RENDER-03:** Instancing, streaming, LOD, and culling meet the selected
  profile's visual correctness and resource bounds over repeated edits.
- **RENDER-04:** Panel remounts, delayed DOM updates, and dropped UI summaries
  preserve one worker-owned frame loop, live camera state, and bounded GPU
  resource counts; explicit shutdown releases that loop and its resources.
- **RENDER-05:** Reference shots record camera/source/quality identities and
  separate composition, material, light, atmosphere, motion, and cost acceptance.
  A lighting/geometry edit invalidates its dependent bake; a failed shader still
  preserves the working generation and compatible player state.
