# Belt 2200

A Verse realm at https://openagents.com/realms/belt-2200: a hard sci-fi strategy game set in the solar system in the year 2200. The Sun, Mercury to Saturn, the Moon, Ceres, Vesta and a field of Belt rocks move on their real orbits. You run the Free Belt's Hearth Station at Ceres and three ships; the Mars Union and the Terran Compact fly their own routes.

## How to play

- Select a ship: tap or click it on the map, or its card at the bottom; on a desktop, drag a box around ships (Shift adds to the selection).
- Pick where it goes: tap a world, an ice rock or the station, or press MINE ICE (the ice rock it reaches soonest) or HOME. The planner shows two transfers, GO ECONOMY and GO FAST, each with its time, delta-v and propellant; the dashed arc is the coast between the two burns.
- A ship that reaches an ice rock mines until its hold is full, flies home, delivers the ice to Hearth Station, refuels from it (water is reaction mass) and goes back for more. ICE at the top right is the station's stock.
- Time: PAUSE, 1, 7 or 30 days a second (Space pauses; 1, 2 and 3 pick a speed).
- The map: the wheel or a pinch zooms, from the whole system down to a ship's hull; drag (or WASD and the arrows) pans; FOLLOW (or F) rides with the selected ship; C goes back to the whole system; Escape cancels.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- The orbits are the engine's: `verse/orbit.coax` (two-body Kepler motion, the planets' J2000 mean elements, spheres of influence, Lambert transfers and the rocket equation), `verse/spacecam.coax` (a floating origin in f64, so a 120 m hull and Jupiter's orbit draw with one camera) and `verse/stars.coax` (the starfield and the Sun). `docs/verse/space.md` in the OpenAgents repository lists them.
- `apps/openagents-web/belt.coax` is the game: its rules (ships, fuel, mining, the rivals' routes) and its drawing, glowing line art as on the homepage. `belt-face.c` binds it for the browser and `belt.wgsl` draws it.
- The design is `docs/verse/game-ideas/belt-2200.md`. The factions, ships and places are original.
