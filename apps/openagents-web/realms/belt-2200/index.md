# Belt 2200

A Verse realm at https://openagents.com/realms/belt-2200: a hard sci-fi strategy game set in the solar system in the year 2200. The Sun, Mercury to Saturn, the Moon, Ceres, Vesta and a field of Belt rocks move on their real orbits. You run the Free Belt's Hearth Station at Ceres and three ships; the Mars Union and the Terran Compact fly their own routes.

## How to play

- Select a ship: tap or click it on the map, or its card at the bottom; on a desktop, drag a box around ships (Shift adds to the selection). On a phone, rest a finger on a ship to add it to the selection or take it out.
- Fleets: one order sends every selected ship. MINE ICE with several selected sends each on its own plan from where it is: a full hold flies home to unload, a ship already mining keeps at it, and the rest go to the ice rock each reaches soonest. Ctrl and 4 to 9 keeps the selection as a group, and the digit alone selects it again. Hold Shift as you give an order to a ship in flight and it goes there once it arrives.
- Pick where it goes: tap a world, an ice rock or the station, or press MINE ICE (the ice rock it reaches soonest) or HOME. The planner shows two transfers, GO ECONOMY and GO FAST, each with its time, delta-v and propellant; the arc is the coast between the two burns, a tick every 30 days, ending where ship and target meet on the date shown. A ship's card shows its propellant and ice in tonnes.
- A ship that reaches an ice rock mines until its hold is full, flies home, delivers its load to Hearth Station, refuels and goes back for more. A load is mostly ice, the rest metal-bearing rock. The station's refinery melts 40 t of ice a day into reaction mass (80%), water (15%) and air (5%), and ships refuel from the reaction mass. The refinery needs 30 workers; the station has 46 people, and a ship's crew is off the shifts while it is away. MASS, ICE, METAL and WORKERS at the top right are the station's.
- A miner leaves a rig at each rock it works, drilling 15 t of ice a day into a stock there. BUILD FREIGHTER (under the station's stores) lays down a freighter from 150 t of metal and 4 of the station's free people; it is launched at Hearth 20 days later, up to three. A freighter can't mine: HAUL ICE sends it to the soonest rock with a rig, and it keeps hauling the stock home (2,000 t hold) with no further orders.
- Time: PAUSE, 1, 7 or 30 days a second (Space pauses; 1, 2 and 3 pick a speed).
- The game is kept in this browser as it runs and comes back when you return. NEW (pressed twice) starts over.
- Leaving or reaching a world, a ship swings out from it, or in to it, on a hyperbola inside the world's sphere of influence before it joins its arc about the Sun.
- The map: the wheel or a pinch zooms, from the whole system down to a ship's hull; drag (or WASD and the arrows) pans; FOLLOW (or F) rides with the selected ship; with no ship selected, a tap on a world, a rock or the station flies the view to it and rides with it; SYSTEM (or C) goes back to the whole system; Escape cancels.
- Without WebGPU (a current Chrome, Edge or Safari) the page shows only its name and a link here.

## How it is made

- The orbits are the engine's: `verse/orbit.coax` (two-body Kepler motion, the planets' J2000 mean elements, spheres of influence, Lambert transfers and the rocket equation), `verse/spacecam.coax` (a floating origin in f64, so a 120 m hull and Jupiter's orbit draw with one camera) and `verse/stars.coax` (the starfield and the Sun). `docs/verse/space.md` in the OpenAgents repository lists them.
- `apps/openagents-web/belt.coax` is the game: its rules (ships, fuel, mining, the rivals' routes) and its drawing: glowing line art as on the homepage over shaded worlds (Saturn's rings, the Earth's air, Jupiter's bands) and the Belt's dust with its Kirkwood gaps. `belt-face.c` binds it for the browser and `belt.wgsl` draws it.
- The design is `docs/verse/game-ideas/belt-2200.md`. The factions, ships and places are original.

## Make your own realm

An agent can build one from a shallow clone of the repository and open a pull request: [verse/SKILL.md](https://github.com/OpenAgentsInc/openagents/blob/main/verse/SKILL.md).
