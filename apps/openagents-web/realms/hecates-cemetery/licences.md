# Hecate's Cemetery: licences

Every model in Hecate's Cemetery is drawn in code from the Verse engine's own shapes (a box, a sphere, a cylinder, a cone and a ball), in `apps/openagents-web/villa.coax`, part of the OpenAgents repository. Nothing comes from an outside kit or model library.

| Piece | Made from | Source | Licence |
| --- | --- | --- | --- |
| Tombstones: a round-topped slab, a cross on a plinth, a small obelisk, a gabled stone, each with a plaque for a name | boxes, a cylinder, a cone | `villa.coax` `tombstone` | Apache-2.0 |
| Plot beds and their kerbs | boxes | `villa.coax` `plot_bed` | Apache-2.0 |
| Cypresses and round trees | cones, cylinders, spheres | `villa.coax` `tree_draw` | Apache-2.0 |
| Hedges | boxes | `villa.coax` `scenery` | Apache-2.0 |
| The garden wall and its two gateways | boxes, spheres, a cylinder | `villa.coax` `wall_run`, `gateway` | Apache-2.0 |
| Benches | boxes | `villa.coax` `bench` | Apache-2.0 |
| The fountain | cylinders, a sphere | `villa.coax` `fountain` | Apache-2.0 |
| The field's edge stones | boxes | `villa.coax` `edge_stone` | Apache-2.0 |
| Paths and ground | the engine's heightfield | `verse/terrain.coax`, `villa.wgsl` | Apache-2.0 |
| You and the dog | the engine's figures and motions | `verse/figure.coax`, `verse/motion.coax`, `villafig.coax` | Apache-2.0 |
| The planet, the moon and the stars | a sphere, the sky shader | `villa.coax` `planet`, `villa.wgsl` | Apache-2.0 |
