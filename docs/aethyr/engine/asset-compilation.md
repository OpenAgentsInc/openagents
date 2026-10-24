# Incremental world and asset compilation

Status: proposed specification. The compiler is an owned Rust library with
native service and browser-Wasm adapters. This document defines required
behavior, not an implemented asset pipeline. [World model](world-model.md)
owns semantic structures; [contracts](contracts.md) owns artifact envelopes;
[publishing](publishing.md) owns distribution and release activation.

## Inputs and rights

An asset import contains immutable source bytes, a source identifier, creator
or supplier attribution, license and redistribution metadata, import settings,
and a declared intended use. The project stores references to licensed inputs
according to its access policy; a public content hash is never an access grant.
Vendor kits and private inputs do not enter this repository as example data.
Open fixtures use explicit compatible licenses or original generated geometry.

The [Medieval Town reference effort](references/medieval-town.md) additionally
requires an import coverage report for source-map placements, camera intrinsics,
light/sky/fog/postprocess settings, material graphs/parameters, decals, foliage,
and effects. Preserve supported settings with units and source identities; label
missing or approximated features explicitly. A transform-only or base-color-only
conversion cannot be marked visually equivalent to the source scene. Private
reference reconstruction remains separate from public conformance content.

Initial interchange accepts a specified glTF 2.0/GLB subset, image and audio
formats selected by the platform profile, and semantic Aethyrforge world data.
Unknown required glTF extensions fail the import; optional unsupported data
produces an explicit diagnostic. The importer validates buffers, accessor
bounds, transforms, textures, skins, animation channels, and finite values.
glTF defines scenes, meshes, materials, skins, and animations; it does not
describe an editable architectural floor plan.
[Khronos glTF specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html)
is the interchange authority.

A decorative house mesh is not automatically a walkable house. An importer
must either receive semantic rooms, openings, and collision annotations, or
guide the creator through authoring them. An inferred layout remains a
reviewable proposal. The compiler never labels a solid exterior as a working
interior by omitting collision everywhere.

Imports run in isolated, bounded jobs. Archive expansion has limits for bytes,
files, recursion, and compression ratio. Paths cannot escape the import root;
network references require an allowed fetch adapter and pinned retrieved bytes.
Decoders receive memory and time budgets. No importer executes a source file's
embedded script. License restrictions propagate to derived assets and exported
releases; optimization does not remove the source's attribution obligations.

Normalization converts supported inputs into meters, Y up, right-handed space,
with explicit axis and handedness transforms. Semantic building topology stays
in the world model's integer millimeters. Import metadata records conversion
and pivot choices so repeated imports cannot silently change placement.

## Dependency graph and keys

The compiler expands a project revision into a directed acyclic graph of
versioned tasks. Tasks cover import, normalization, semantic geometry,
materials, texture mip chains, mesh optimization, LOD, collision, navigation,
selection, scripting, and cell manifests. A task key hashes a domain tag,
canonical task description, input content hashes, exact compiler/tool versions,
target profile, settings, and random seeds. The canonical encoding is defined
in [contracts](contracts.md), not by filesystem iteration order or incidental
JSON whitespace. A key is distinct from the hash of its resulting bytes.

Task outputs are immutable blobs with declared types, sizes, dependencies, and
integrity hashes. Cache metadata publishes only after complete writes and
validation. A canceled or crashed job cannot leave a successful cache entry.
Independent ready tasks run under bounded CPU, memory, and I/O concurrency;
identical tasks share work within an authorized cache scope. A bad remote cache
entry is discarded, reported, and recomputed from trusted inputs.

No task reads an undeclared clock, home directory, network resource, locale,
or mutable tool path. Tool binaries and external decoder versions are pinned.
The build records its environment profile, but excludes timestamps and absolute
paths from reproducible outputs. Reproducibility means byte-identical declared
artifacts for specified inputs and environment, consistent with the
[Reproducible Builds definition](https://reproducible-builds.org/docs/definition/).
Cross-platform equality is a qualification requirement for any shared artifact
profile; a native optimizer that cannot meet it gets an explicit distinct
profile and cannot silently overwrite another profile's result.

## One semantic generation

The world compiler reads one immutable authored revision. It derives visible
surfaces, collision shapes, navigation regions and portals, selection geometry,
occlusion bounds, and streaming cells from the same semantic structures and
coordinate conventions. Every output records its world generation and
dependencies. A generation may reuse unchanged blobs from an older generation;
reuse does not make its manifest a mixture of incompatible revisions.

Deleting a wall invalidates its mesh surfaces, collision, doorway connectivity,
local navigation tiles, selection targets, and relevant cell bounds. Tile
invalidation includes a boundary halo and adjacent portal endpoints, preventing
seams caused by rebuilding only one side. Changing a material parameter usually
invalidates only its parameter data; moving an instance updates transforms and
affected spatial memberships. Changing texture bytes rebuilds dependent texture
variants without recompiling unrelated scripts or world geometry.

Each task reports diagnostics tied to source IDs and, where possible, editor
selection targets. A malformed opening, disconnected stair landing, or
nonmanifold input is a creator-visible failure with an actionable location.
Preview may display an explicitly marked draft representation while a candidate
builds. That representation never silently replaces authoritative collision or
becomes a publishable complete generation.

## Runtime outputs

Geometry output includes indexed mesh clusters, bounds, LOD error, material
references, optional skinning data, and stable selection mappings. Collision
uses qualified shapes or simplified meshes appropriate to body type, independent
of visual LOD. Navigation uses per-agent profiles, tiles, and explicit links.
Animated visual bounds remain conservative. Instancing references one shared
asset bundle, rather than expanding a full copy for each placement.

Textures record color space, alpha mode, channel meaning, mip levels, and
compression variants. KTX2 is the preferred interchange container for qualified
compressed texture artifacts; the renderer selects an available target format
or a budgeted uncompressed fallback. KTX2 defines levels, metadata, and
supercompression; device format support is still checked separately.
[Khronos KTX2 specification](https://registry.khronos.org/KTX/specs/2.0/ktxspec.v2.html)
defines the container. Audio variants record sample characteristics and looping
metadata; decode or transcode latency belongs in the profile budget.

Cells have dependency manifests and a low-detail render fallback. Required
collision and navigation must be ready before gameplay enters a cell. Optional
visual refinements can stream afterward without changing gameplay geometry.
The compiler rejects profiles that exceed declared dimensions, memory, material,
or feature limits. It does not generate an enormous artifact and hope the
browser accepts it.

The [Town renderer plan](town-renderer-redesign.md) applies these outputs to
compiled spatial batches, specialized material families, and resident GPU tables.
Its first phase defines bounded rendering pages and stable semantic selection/edit
mappings without changing world authority or the compatible-generation contract.
This is a proposed implementation sequence, not a completed compiler feature.

## Staging and activation

A build request includes the authored revision, profile, and candidate identity.
Completion cannot activate after that candidate becomes stale. The compiler
produces a complete generation manifest only when all required outputs validate.
The runtime stages required resources and checks occupant migration before
switching the matching simulation and rendering generation. A failure leaves
the previous working generation active. [Runtime](runtime.md) and
[rendering](rendering.md) define those boundaries.

Local Wasm compilation prioritizes small interactive changes; native workers
handle expensive imports and optimization under the same task contracts. The
creator can keep editing during a long import. Job cancellation stops wasted
work where possible and suppresses stale activation everywhere. Engine binary
and ABI updates use a separate compatibility and restart path; ordinary scene
edits do not rebuild the engine.

## Qualification

Tests compare clean and incremental builds byte for byte, repeat builds in
independent environments, mutate individual inputs, corrupt caches, cancel at
each write boundary, and verify dirty-region completeness. A walkable-building
fixture checks visible walls, collisions, navigation, and selection after
removal and undo. Record cache hits, invalidation fanout, peak memory, transferred
bytes, and edit-to-activation latency. [Performance](performance.md) sets targets;
[verification](verification.md) defines the release evidence.

- **COMPILE-01:** Clean and incremental builds produce identical declared
  artifacts for a pinned input and environment profile.
- **COMPILE-02:** A structural edit updates rendering, collision, navigation,
  selection, and required cell seams as one complete candidate generation.
- **COMPILE-03:** Canceled, corrupt, unauthorized, or stale outputs never
  activate or become successful cache entries; decorative models never gain
  walkable-building capability without validated semantics.
