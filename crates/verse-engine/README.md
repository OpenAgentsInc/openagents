# Verse Engine core

This headless crate owns generation-safe entity storage and life identities,
a bounded fixed-step schedule, the version-one asset pack, skeleton sampling and
crossfades, named animation states and compiled playback bindings, and a deterministic cinematic director. The native original chamber
and renderer use these contracts directly. It has no GPU, platform, game rules,
transport, or credential dependency.

The modules hold project-owned Rust code extracted from earlier Verse adapters,
not a copy from a reference engine. Pack coordinates retain the version-one
Z-up convention, and `source_position` converts them to Y-up meters. A scene's
`origin` field is in the same pack coordinates.

Game authority, physics integration, persistence, and multiplayer are separate
boundaries on the [roadmap](../../docs/verse/engine/roadmap.md).

Original scenes select semantic states instead of source clip numbers. Each model declares its clip reference, loop or hold behavior, and transition duration. Playback blends local transforms and resets when the actor life changes. Numeric selection remains an explicit compatibility path for retained research packs.

The optional `asset-io` feature prepares a complete pack from an explicit directory before GPU allocation. It verifies every texture digest and static eight-bit RGBA declaration, rejects symlink files and invalid references, and enforces manifest, encoded-file, aggregate encoded/decoded, and decoder-workspace budgets. Prepared bytes are immutable; the receipt records digests and exact byte and geometry counts. Default authority builds do not enable this loader or its PNG decoder.

The portable `residency` catalog resolves logical model names and pack-local texture slots into distinct typed handles. Each catalog receives a nonreused process-local identity; rebuilding rejects prior handles even when names or slots match. These handles are runtime references, not persistent asset IDs or distribution credentials. The native renderer extracts immutable resolved frames, validates their catalog before GPU writes, and indexes model and texture draws through typed handles. Original inventories add persistent model and texture IDs; catalog identities still fence runtime references across reloads.

`inventory` declares persistent IDs, content digests and lengths, dependency edges, creator/license evidence, and compiler revisions. It validates complete model/texture coverage, material edges, graph bounds, and cycles. Original-local, capture, and redistribution admission inspect the resolved provenance closure; research origins are refused, and owner-supplied local assets cannot be redistributed. The `asset-io` loader verifies model fingerprints and actual encoded texture lengths before GPU allocation. Source declarations are attributable metadata, not legal attestation or signed distribution approval.

`presentation` owns portable instance values and immutable catalog-bound frame extraction. It borrows source instances, preserves actor lives and animation selections, limits a frame to 1,024 instances, and rejects missing models or nonfinite transforms, animation times, and emissions. Native drawing consumes this extracted contract and validates its catalog before GPU writes, including empty frames. Camera, lighting, UI geometry, and unified render-world extraction also live here; GPU submission remains in the renderer.

`presentation::View` owns finite camera projection and eye values. `lighting` owns local point sources, atmosphere, the 32-light/four-shadow-source budget, authored flicker sampling, and cube-shadow camera construction. Admission rejects invalid inputs and nonfinite sampled intensities or shadow projections before native uniform writes. GPU uniform packing and shader layouts remain in the renderer. `PointProfile` declares retained authored and physical candela attenuation separately. `FrameLighting` records captured settings, while common EV100 conversion and visible-source ranking serve the physical renderer.

`sockets` resolves bind-space attachment frames from an admitted model and a
caller-supplied evaluated skinning palette. Socket IDs are unique and bounded to
64 per model. Palettes must match the exact borrowed model, bone count, and finite
affine matrix contract; local joint transforms are not interchangeable with
skinning matrices. Frames preserve parent and animated joint rotation/scale,
then apply the socket's bind-space position and authored equipment transform.
Missing sockets, foreign palettes, invalid inputs, and derived overflow are
explicit errors. Resolution owns no playback clock, world state, or inventory.
Native bow positioning consumes this contract. Universal compilation retains
bow sockets 2–4 and adds head socket 5 and right-palm socket 6. Native mounts
resolve after graph blending and grounding from the same final body palette.

Presentation instances can declare a leaf `Mount` with an exact parent life,
parent render model, socket ID, and affine local transform. Catalog extraction
resolves one unique parent independent of instance order and rejects missing
sockets, stale lives, ambiguous parents, nested mounts, and nonaffine transforms.
Render adapters evaluate all body palettes and grounding before resolving mounts;
attachments consume the same final skin matrices as their parents. The native
adapter uses this path for gear and each character's bow, including outfit rigs.

Rendering adapters share `quality::Budget` for instance, optional-effect, surface,
and managed resource admission. `presentation::VisualSelection` validates up to
8,192 source instances and retains required actors and mounts before selecting
optional effects by priority and source order. Lower quality reduces optional
work; timing limits are measurement targets.

`streaming` validates versioned, SHA-256-bound static chunks and their dependency
graph. Its portable scheduler separates desired content, verified CPU payloads,
and complete GPU uploads. Root order sets priority; independent byte budgets,
a fixed pending-job limit, explicit failure retries, and dependency pins govern
eviction. Cancelled workers retain CPU reservations until their results arrive.
Zone generations and upload tickets reject stale work; device recreation retains
verified CPU content and invalidates GPU residency.

With `asset-io`, `streaming::store` reads regular digest-named files under an
explicit root on fixed background workers. Native `verse::render::Renderer` and
`Layer` expose `configure_streaming`; their source uploads under per-frame byte
and soft time limits. The first cooked profile holds static triangles or lines,
single-level sRGB RGBA images, and an optional offline lighting recipe digest.
Vertex colors hold the baked result; runtime performs no lighting bake. Animated
packs and full physical materials keep their existing loading paths. Managed
payload accounting excludes bounded manifest metadata, render targets, platform
allocation padding, and shader-private storage.

`mips` cooks RGBA8 chains by material role. Color and emission filter in linear
light; scalar maps stay linear; normal texels and reductions normalize vectors.
Mask variants include the material's effective alpha cutoff and calibrate coarse
alpha contrast against bilinear coverage. Both native rendering paths use these
recipes and reserve every distinct chain before upload. Fractional coverage at a
single texel, authored compressed chains, and vertex-varying alpha remain content
production limits.

The dedicated `verse-host` and network service use `core::FixedSchedule`. Actor identities use `LifeId`; actor storage remains in `verse-world` rather than adopting the separate `Entities` container. Applications supply generic `MountPose` values to the admitted renderer frame, including planar grips and offset socket frames.


`audio_bank` admits versioned cue banks with bounded sources, mix buses,
priorities, and localized captions. The original bank declares reproducible
procedural cues. Decoded mono clips must match their PCM pins; streamed music and
dialogue declare stereo little-endian float PCM, sample rate, frame count, and
SHA-256. `prepare_stream_reader` verifies the supplied immutable reader before
playback, then returns its prepared voice and bounded feeder. The caller supplies
an immutable snapshot or provider; the bank selects no file or network path.
Compressed decoding belongs on a provider worker. Streamed tracks are finite;
original clip ambience supports looping.

`audio::Mixer` separates control-side preparation from callback admission. It
holds at most 128 logical voices, mixes 32 audible voices, bounds streaming to
eight sources, advances virtual clip clocks analytically, and retains completed
resources for off-callback reclamation. Music has two replacement slots; effects
cannot displace music. Dialogue ducks music. Suspension freezes source clocks.
`audio_bank::Scene` retains captions, volume settings, and zone music progress
without a device. These contracts do not establish hardware audio deadlines or
complete environmental acoustics.
