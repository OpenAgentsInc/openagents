# Porting the realms

Status: plan, October 28. The Haunted House is live at /realm/haunted-house.
This plan brings the demo's other worlds to openagents.com in Coax and builds
Everglade. The tasks are in the Verse project under "Port the realms"
(https://openagents.com/projects/verse).

## Order

1. **One realm page for every world.**
   - `/realm/<slug>` loads any realm's world module, assets, HUD and save
     slot, and the galaxy opens each one.
   - An asset pack compiler streams kit art in on entry.
   - The release walk opens every realm.
2. **Ice Cream Man**, OpenAgents' seeded house game.
   - The rules are ported step for step and checked frame by frame against
     the Rust build's recorded input traces, so parity is proven rather than
     eyeballed.
   - Then the house, play, the HUD and launch.
   - The owner signs off parity side by side with the demo.
3. **Dead Gears**, the ship survival game.
   - First a small entity system in Coax: 200 creatures within 2 ms a
     frame.
   - Then the demo's rules and their tests, the ship, the creatures, the
     controls and the day-to-dusk lighting.
4. **Everglade**, the forest city.
   - The demo only had it as a plan, and the old Rust repo had a glade with
     a workshop, so this is a new build.
   - Order: plan and blockout, forest and paths, buildings from the CC0 kits,
     characters who remember you, the quest board with three quests, and the
     bird companion.
   - Players can't edit it.
5. **Later:** the Sandbox, the Grid and Medieval Town. Medieval Town may
   fold into Everglade.

## Sources (read only)

| Realm | Source |
| --- | --- |
| Ice Cream Man | `~/work/aethyr-demo`: `src/icecream/*` and `third_party/icecream` (the Rust original), plus `tools/icecream-trace` |
| Dead Gears | `~/work/aethyr-demo`: `src/dead_gears*.cpp`, `src/ecs/` and `docs/ecs.md` |
| Everglade | `~/work/aethyr-demo/docs/realms.md`, plus the old OpenAgents Everglade at `0b401561a0^:docs/verse/everglade.md`, its map and `crates/verse-zone-everglade` |
| Sandbox, Grid | `~/work/aethyr-demo`: `web/sandbox.html` and `src/grid_world.cpp` |

All three kits are CC0, in `~/work/aethyr-cpp`: Quaternius Stylized Nature,
Medieval Village and Fantasy Props.

## Engine work they lean on

These are Verse engine tasks already in the queue:
- Dead Gears lighting needs moonlight and the shadow work.
- Everglade's forest needs terrain and paths.
- Its cats need four-legged characters.
- Its companion needs particles.

Those are linked as blockers, so realm tasks become ready as the engine
lands.
