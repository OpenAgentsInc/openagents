# Verse Engine core

This headless crate owns generation-safe entity storage and life identities,
a bounded fixed-step schedule, the version-one asset pack, skeleton sampling and
crossfades, named animation states and compiled playback bindings, and a deterministic cinematic director. The native original chamber
and renderer use these contracts directly. It has no GPU, platform, game rules,
transport, or credential dependency.

The modules move existing project-owned Rust code out of `verse-wow`, which
re-exports them to preserve adapter compatibility. This is engine extraction,
not a copy from a reference engine. Pack coordinates retain the version-one
Z-up convention and explicit conversion to Y-up meters. The scene's legacy
`origin_wow` serialization field remains a compatibility detail until the next
schema revision.

Game authority, physics integration, persistence, and multiplayer are separate
boundaries on the [roadmap](../../docs/verse/engine/roadmap.md).

Original scenes select semantic states instead of source clip numbers. Each model declares its clip reference, loop or hold behavior, and transition duration. Playback blends local transforms and resets when the actor life changes. Numeric selection remains an explicit compatibility path for retained research packs.

The optional `asset-io` feature prepares a complete pack from an explicit directory before GPU allocation. It verifies every texture digest and static eight-bit RGBA declaration, rejects symlink files and invalid references, and enforces manifest, encoded-file, aggregate encoded/decoded, and decoder-workspace budgets. Prepared bytes are immutable; the receipt records digests and exact byte and geometry counts. Default authority builds do not enable this loader or its PNG decoder.

The portable `residency` catalog resolves logical model names and pack-local texture slots into distinct typed handles. Each catalog receives a nonreused process-local identity; rebuilding rejects prior handles even when names or slots match. These handles are runtime references, not persistent asset IDs or distribution credentials. The native renderer extracts immutable resolved frames, validates their catalog before GPU writes, and indexes model and texture draws through typed handles. Original inventories add persistent model and texture IDs; catalog identities still fence runtime references across reloads.

`inventory` declares persistent IDs, content digests and lengths, dependency edges, creator/license evidence, and compiler revisions. It validates complete model/texture coverage, material edges, graph bounds, and cycles. Original-local, capture, and redistribution admission inspect the resolved provenance closure; research origins are refused, and owner-supplied local assets cannot be redistributed. The `asset-io` loader verifies model fingerprints and actual encoded texture lengths before GPU allocation. Source declarations are attributable metadata, not legal attestation or signed distribution approval.

`presentation` owns portable instance values and immutable catalog-bound frame extraction. It borrows source instances, preserves actor lives and animation selections, limits a frame to 256 instances, and rejects missing models or nonfinite transforms, animation times, and emissions. Native drawing consumes this extracted contract and validates its catalog before GPU writes, including empty frames. Camera, lighting, UI batches, and GPU submission remain renderer contracts.

`presentation::View` owns finite camera projection and eye values. `lighting` owns local point sources, atmosphere, the 32-light/four-shadow-source budget, authored flicker sampling, and cube-shadow camera construction. Admission rejects invalid inputs and nonfinite sampled intensities or shadow projections before native uniform writes. GPU uniform packing and shader layouts remain in the renderer.
