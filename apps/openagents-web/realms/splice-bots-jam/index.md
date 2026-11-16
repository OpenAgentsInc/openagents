# Splice Bots — Unreal jam build

The Splice Bots jam build's four levels (the farm, the lab, the plain and the
ring), brought over from Unreal by the Metaverse Engine's level importer
(`metaverse-engine/tools/unreal-level-import.coax`, GitHub #13648) and walked
here (#13649).

Walk with **WASD** or the arrows, **Space** to jump, **Shift** to run. **1** to
**4** or **M** change level, **R** goes back to the level's start. On a phone,
drag on the left to move and on the right to look; tap **NEXT MAP** at the top
right.

Each level starts at its player start. What you see is what stops you: every
collider is checked against the instance it belongs to, and the route walk
(`metaverse/splice-bots/tools/jam-route-walk.coax`) finds no invisible
blockers.

The level pack is private and served locally only (`?pack=splice-bots-jam`);
the game's files are not in the repository. This first build is the shipped
game's data: its blockout boxes, ramps and walls, its lights and its markers.
The real models from the Unreal editor come in a later import.
