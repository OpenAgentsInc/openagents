# Verse

Verse is OpenAgents' real-time engine (formerly named Metareal). The same
name covers the world and experience it runs, the Verse, whose individual
worlds are realms. Coax is the target implementation language for the engine and
for OpenAgents' other surfaces; the current compiler is a scalar bootstrap.

The defining interchange contract is the
[OpenAgents Protocol](../protocol/openagents-protocol-spec.md), with
[Verse's chapter](openagents-verse-protocol.md). The shared engine namespace
stays `openagents.engine.*`; world, simulation and product contracts use
`openagents.verse.*`. A brand rename does not duplicate those namespaces.

The current OpenAgents checkout contains Coax and the recovered protocol
foundation. The native engine implementation and previous app are historical
migration sources. A reduced [native iOS home](../../apps/openagents-ios/README.md)
now ports graph/HUD behavior and scalar shader math into Coax, with a thin
native storage/Metal adapter. The rest of the engine and app are not restored.
The source archive
and renamed identifiers are recorded in
[the migration registry](../../proto/migrations/metareal-v1.json).

The native reference path is Swift → C ABI → engine → metal-cpp → Metal.
Preserve resident host ownership, GPU buffers, frame scheduling and platform
rules while moving subsystems into Coax. Browser delivery has an independent
Wasm/host-binding/shader integration gate. A successful native compile does
not prove that browser path.

The earlier performance plan identified five independent improvements:

1. A reloadable game layer behind a versioned C interface on a development host.
2. Plain data and short C++ bootstrap builds while Coax grows.
3. Data-oriented storage and jobs with validated effects and aliases.
4. GPU-driven rendering with checked upload/resource layouts.
5. SIMD only where measured workloads justify it.

Coax provides a way to share checked declarations and metadata across those
systems. Native CPU layout, GPU layout, protocol encoding and reload/save
migration remain separate. Keep working native code and platform shims until
actual Coax consumer parity is demonstrated; no whole-engine rewrite is
claimed here.
