# The Metaverse Engine

The **OpenAgents Metaverse Engine** is the Coax engine for the Metaverse's
worlds: frame loop, entities, spatial queries, rendering, physics, hosts,
networking and the tools for creating and editing worlds. The **Metaverse**
is the collection of worlds and experiences it runs. The
[root glossary](../books/glossary.md) and [engine naming history](engine-name.md)
keep those meanings distinct; old published wire names retain their identity.

## Current sources and plans

- [Engine source and owner inventory](../../metaverse-engine/README.md): the
  shared modules and downward dependency rules. `metaverse/` keeps world
  content; individual migrations and remaining private copies are tracked.
- [Engine book and chapter registry](../../metaverse-engine/book/README.md):
  implemented, partly implemented and draft profiles, with qualified targets
  and adoption recorded separately.
- [Construction plan](../../metaverse-engine/book/PLAN.md): the playable-world,
  editor/publication, physics, streaming and multiplayer routes. Advanced
  physical simulation is not a prerequisite for every world.
- [World editor](../../metaverse-engine/editor/README.md) and its
  [product plan](aethyrforge.md): typed documents, one edit log, admitted
  assets, pinned publication and forks, plus remaining editor work.
- [Omniverse comparison](2026-11-15-omniverse-comparison.md): proposed reusable
  assemblies, readiness reports, bounded OpenUSD exchange, completed runtime
  views and consistent sensor/data outputs, assigned to existing owners.

The old description of this checkout as only a scalar compiler, recovered
protocol and reduced Metal home is historical. Current engine and host
profiles have their own source and evidence; the links above say exactly
what they establish. Historical native code remains a migration/reference
source, not a workspace to restore wholesale.

## Contracts and boundaries

The [OpenAgents Protocol](../protocol/openagents-protocol-spec.md) and
[Metaverse protocol chapter](openagents-verse-protocol.md) define published
interchange. Naming or branding changes do not duplicate protocol namespaces.
New stored/exchanged shapes begin with their specification, owning context,
registered version and one codec.

Coax owns product logic and numerical arithmetic. Host code performs platform
effects. CPU layout, GPU resources, wire encoding, save migration and scene
interchange are distinct contracts. A native compile, a C API, a rendered
picture or a data-format import does not qualify the other paths.

Engine product dependencies go downward into shared libraries; Psionic and
product contexts consume the engine. Authoring changes the world document,
simulation changes running state, and durable character/item state keeps its
own authority. Existing clocks, edit logs, resource lifetimes, asset admission
and host recovery are the starting owners for new integrations.

For the native/browser transition and recorded target limits, use the
[native GPU plan](native-gpu.md) and the engine's host/render evidence, rather
than assuming every target shares one implementation or qualification.

Characters: [MetaHuman support](metahuman-support.md) remains a separate design
for private Epic characters, with its own artifact and runtime requirements.
