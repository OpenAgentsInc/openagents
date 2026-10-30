# Game ideas from the recording

Three games people on the October 30, 2026 recording asked for, written up as
design documents for the Verse engine. Each one says what the game is, how it
plays, what it reuses from the realms we have already built (Ice Cream Man,
Dead Gears, Everglade, Fairy Forest, the Grid), and what the engine still
needs before it could ship.

| Game | Pitched by | Genre | Doc |
|---|---|---|---|
| Orbit League | Topher | Car-ball in space (rocket-powered roadsters) | [orbit-league.md](orbit-league.md) |
| Belt 2200 | Ted | Hard sci-fi real-time strategy, the solar system in the year 2200 | [belt-2200.md](belt-2200.md) |
| Hold the Stair | Marc and Chris | Tower defense on the SRD 5e rules | [hold-the-stair.md](hold-the-stair.md) |

These are original games in their genres. They borrow general styles (a
car-ball sport, hard sci-fi, a popping tower defense, tabletop fantasy rules)
and none of any one existing game's names, characters, factions, maps or art.
Real brands stay out of what ships: Orbit League's cars are our own electric
roadsters, not a carmaker's.

## The engine work they share

Three of the needs show up in more than one game, so they are worth doing
once, in the engine, before any of the three:

1. **Fast multiplayer.** The rooms we have (`apps/openagents-web/room.coax`,
   `coax/web/api/room.c`) post where you are a few times a second over HTTP.
   That is enough for walking together on the Grid. Orbit League needs 60
   updates a second with prediction and rollback, and Belt 2200 needs
   deterministic lockstep. Both need a WebSocket room service with an
   authoritative tick.
2. **Contacts in the rigid-body solver.** `verse/rigid.coax` steps bodies in
   f64 with quaternion attitude and RK4 rotation, but has no contacts or
   joints. Car-on-ball, car-on-wall and ship-on-ship all need collision
   detection and impulse response.
3. **Many units, cheaply.** The Dead Gears entity system
   (`verse/deadgears/ecs.coax`) and Everglade's level of detail
   (`apps/openagents-web/everglade-lod.coax`) are the start. Belt 2200 and
   Hold the Stair both want hundreds to low thousands of moving things on a
   phone at 60 FPS: instanced drawing, simulation off the main thread, and
   spatial queries.

Each doc's "What the engine needs" section lists the rest, with the files
they would extend.
