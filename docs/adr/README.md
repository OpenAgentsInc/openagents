# Architecture decision records

This folder holds every current architecture decision for OpenAgents. It
started on 2026-10-24 when the owner deprecated all earlier decision records
([0001](0001-one-decision-record-folder.md)).

A record says what was decided, why, and what it replaces. Each record has a
status:

- **Accepted:** the owner decided it. Build to it.
- **Accepted in part:** the decided part is binding. The undecided part is
  marked and listed in [open decisions](../aethyr/merge.md#decisions-for-the-owner).
- **Proposed:** a recommendation awaiting the owner. Don't build to it yet.
- **Superseded:** replaced by a later record, which it names.

Write a new record instead of rewriting an accepted one. Number records in
order and never reuse a number.

| # | Record | Status |
| --- | --- | --- |
| 0001 | [One decision record folder](0001-one-decision-record-folder.md) | Accepted |
| 0002 | [Everything in the openagents repository](0002-everything-in-openagents.md) | Accepted |
| 0003 | [Aethyr, the GPU-native framework](0003-aethyr.md) | Accepted |
| 0004 | [Engine core and GPU interface](0004-engine-core-and-gpu-interface.md) | Accepted |
| 0005 | [Effect controls the UI](0005-effect-controls-the-ui.md) | Accepted in part |
| 0006 | [One design system](0006-one-design-system.md) | Accepted |
| 0007 | [One capability and detail language](0007-capability-and-detail.md) | Accepted in part |
| 0008 | [Launch order: Kiki on the web first](0008-launch-order.md) | Accepted |
| 0009 | [Zones, and the Grid](0009-zones-and-the-grid.md) | Accepted |
| 0010 | [Loading zones in the mobile app](0010-mobile-zone-loading.md) | Accepted |
| 0011 | [Rendering and shaders](0011-rendering-and-shaders.md) | Accepted |

Background:

- [The Aethyr merge plan](../aethyr/merge.md): what moves into this
  repository, what to deprecate, and the owner's open decisions.
- [Consolidation audit](../aethyr/imported/2026-10-24-consolidation-audit.md)
  and [GPU Native exploration](../aethyr/imported/2026-10-24-gpu-native.md),
  imported from Aethyrforge.
