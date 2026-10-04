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
