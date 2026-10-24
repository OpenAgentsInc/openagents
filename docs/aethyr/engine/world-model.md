# Semantic world model

Status: proposed design, 2026-10-22. The types below define required meaning; [Contracts](contracts.md) owns their serialized envelopes and version negotiation.

## Source of truth and identity

The authored world is a typed semantic document. Rendering meshes, collision, navigation, selection geometry, thumbnails, and spatial indexes are compiled outputs of that document. A creator edits an entity, wall, or material parameter; generated triangles are not the authoritative representation of an editable building.

Project, branch, entity, building, storey, room, wall, opening, asset, behavior, and entry-point identities are stable opaque IDs. Human names are mutable and need not be unique. A command resolves a name to an ID before mutation and rejects unresolved multiplicity. Deleting and recreating an object produces a new ID. Rebuilding, moving, renaming, or recompiling preserves its ID. References to deleted IDs produce explicit diagnostics; compilers cannot retarget them by array position. Command construction or the authoring validator allocates new structural IDs and persists them, along with split/remapping results, in the accepted semantic document. Rebuilding outputs cannot allocate new authoritative rooms, walls, or openings.

World units are meters, coordinates are right-handed with Y up, and forward is -Z. Rotation uses normalized quaternions in authored transforms. Building-local structural coordinates use integer millimeters to make shared boundaries and validation reproducible; compiler tolerances are versioned, never dependent on the camera or device. The command boundary parses exact metric values, quantizes only under the selected explicit snapping policy, and presents the resulting dimensions. Without that policy, sub-millimeter structural coordinates are rejected rather than silently rounded. Rendering floats derive from this topology; they cannot write back a different boundary. Building transforms permit translation, yaw rotation, and positive uniform scale. Mirroring and nonuniform structural scale require an explicit baked conversion with revalidation. Runtime spatial math uses the precision and origin-shifting rules in [Runtime](runtime.md).

An initial compilation profile limits one building to 128 rooms, 2,048 wall edges, 64 boundary vertices per room polygon, and 16 openings per edge. These are declared resource guards, not advertised world-size limits. A project can contain multiple buildings and streaming cells; larger structures require a documented profile and verification rather than unbounded processing.

## World and building structure

| Semantic record | Responsibility |
| --- | --- |
| World | Cells, environment, entry points, project dependencies, game rules, and placement roots |
| Entity | Stable transform, render or prefab reference, collider policy, properties, and behavior attachments |
| Building | Structural graph, storeys, kit mapping, owned entities, and public entrances |
| Storey | Local elevation, room floor regions, vertical connections, and story-specific defaults |
| Room | Named navigable region, floor polygon, ceiling height, finish defaults, and usage tags |
| Wall | One building-owned edge with thickness, height, adjacent regions, finish, and structural role |
| Opening | A stable cut in a wall with position, width, height, sill, traversal policy, and optional door entity |
| Connection | An entrance, doorway, stair, ramp, lift, or explicit portal with endpoint and traversal requirements |
| Asset reference | Immutable content identity, source revision, license metadata, and declared capabilities |

A storey uses a planar boundary graph. Each wall edge exists once, even when two rooms share it. Room boundaries refer to directed edges, and the exterior is an explicit adjacent region. Simple floor polygons may contain declared holes; self-intersections, zero-area faces, contradictory adjacency, and overlapping occupied floors at the same elevation are invalid. Intersections split edges into stable child edges within a transaction, with a mapping for affected references. Curved walls are a later versioned primitive; the initial profile uses straight edges.

Removing a partition changes its wall policy to an open boundary or removes the physical wall while preserving named room regions. It does not automatically destroy room names, triggers, or furniture ownership. **Merge rooms** is a separate operation that specifies the surviving room ID and treatment of references. This distinction preserves gameplay meaning when an artist opens a kitchen into a dining room.

Openings are wall-relative intervals with vertical extents. Two openings cannot overlap, exceed the available edge, or leave an unsupported sliver below the profile's minimum width. A door's visible leaf, frame, collision, interaction, and navigation state derive from the opening and its attached door state. Cutting a doorway must not leave an invisible collider. Closing a door updates collision and traversal at the same simulation boundary.

## Walkable and editable buildings

A building template is not considered walkable merely because its exterior contains a drawn door. To expose **Walkable building**, compilation must verify a reachable entrance, supported floor surfaces, collision-consistent openings, declared clearance for the default character capsule, and an interior path to every room marked publicly reachable. Default validation uses a 0.3-meter capsule radius, 1.8-meter standing height, and 0.25-meter maximum step; projects can choose another versioned traversal profile. A navigation bake is evidence for that profile, not proof every possible controller fits.

A template consists of a semantic structural graph plus a kit mapping from semantic surfaces to visual panels, trims, roof parts, and materials. The compiler can cut or substitute kit panels around doors and changed partitions. Roof and facade rules respond to extensions; they cannot retain the old shell over a new room. Temporary debug surfaces are visible only in an explicit diagnostic view. An incomplete candidate does not replace the last complete building generation.

An opaque imported mesh initially has **Decorative model** capability. It may have a collision proxy, but it cannot advertise editable rooms. Conversion into a building requires human-approved semantic reconstruction or a supplied structural sidecar, a supported kit mapping, and the same validation as native construction. The original licensed geometry stays immutable. Reconstruction records which surfaces were inferred and which a person verified. This prevents a command such as “remove this wall” from deleting an arbitrary bounding-box face while the source mesh remains closed.

First acceptance covers single-storey buildings with exterior entrances and same-level room extensions. Multi-storey content must implement tested stairs or ramps, floor holes, ceiling clearance, landing collision, and reachability before exposing an editable upper storey. Unsupported vertical additions fail before mutation; they do not produce cosmetic floors that players cannot reach.

## Prefabs, relationships, and streaming

A reusable prefab definition is immutable and versioned. An instance pins its definition and has explicit parameter values and overrides keyed by stable semantic paths. Updating a definition creates a migration proposal. It cannot overwrite local room changes or move a running player without review and runtime compatibility checks. An instance can become a local fork while retaining its source identity and rights metadata.

Parent transforms express spatial relationships; semantic references express ownership and gameplay meaning. Moving a building moves its owned local entities. A crate placed independently beside the building remains in world space unless the creator reparents it. Removing a parent requires explicit handling of children and references. Defaults preserve owned content by rejecting ambiguous deletion rather than silently cascading.

Streaming cells partition compiled storage and runtime residency, not document authority. A wall crossing a cell boundary has one semantic owner; neighboring chunks reference compiled seams with matching generation and dependency hashes. An unloaded cell cannot become empty traversable space. The runtime holds a boundary, loads a coarse valid representation, or keeps the player in the existing area according to [Runtime](runtime.md) and [Asset compilation](asset-compilation.md).

## State and mutation

The authored document contains initial transforms, behavior definitions, materials, structure, and game defaults. Runtime state contains current positions, velocities, door state, script state, health, and session outcomes. Presence contains editor cameras, cursor positions, and selections. Releases pin document and compiler identities. These classes never share an implicitly writable field.

All mutations use [Contracts](contracts.md). Examples include add-building, set-transform, add-room, set-wall-policy, add-opening, set-material, attach-behavior, and merge-rooms. A transaction validates authority, bounds, topology, references, dependency availability, and runtime compatibility before activation. User undo creates an inverse intent checked against current state. Runtime-to-authoring capture is an explicit operation with a selected set of entities and fields.

## Acceptance requirements

- **WORLD-01:** A cottage can be placed, entered through its doorway, extended by a room, and edited without opaque shell geometry or collision blocking the new path.
- **WORLD-02:** A partition deletion updates visible surfaces, collision, navigation, selection, and dependent facade or roof outputs in one compatible activation.
- **WORLD-03:** Shared walls have one identity; cutting an opening from either adjacent room produces the same structural result.
- **WORLD-04:** Self-intersection, opening overlap, unsupported traversal, stale references, and profile overflow fail with bounded diagnostics and preserve the prior active generation.
- **WORLD-05:** Rebuild, save/reload, prefab upgrade, and cell unload/reload preserve surviving semantic IDs and explicitly report remapped or removed references.
- **WORLD-06:** Changing runtime position, opening a door during play, or another creator moving a camera cannot mutate authored source without an authorized capture transaction.
