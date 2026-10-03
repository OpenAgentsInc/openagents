# Verse Engine core

This headless crate owns generation-safe entity storage and life identities,
a bounded fixed-step schedule, the version-one asset pack, skeleton sampling and
crossfades, and deterministic cinematic director. The native original chamber
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
