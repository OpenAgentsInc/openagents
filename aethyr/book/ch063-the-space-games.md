# 63. The space games

> **Status: unimplemented — design draft.** Orbit League and Belt 2200 run
> today on their own code at `/realms/orbit-league` and `/realms/belt-2200`,
> over the space primitives already in `verse/`
> ([docs/verse/space.md](../../docs/verse/space.md)). This chapter plans
> rebuilding both only from Aethyr's earlier chapters. Nothing here runs yet.

Two games share the Verse's space layer. Orbit League is rocket roadsters
playing ball in a glass arena in orbit, where the station spins down every
90 seconds and gravity fades for half a minute, or the match moves into a
spinning drum. Belt 2200 is hard science-fiction strategy on the real orbits
of 2200: you run a station at Ceres and send ships on transfers whose
delta-v, propellant and time are computed, not invented.

## What this chapter teaches

- One physics foundation under two very different games: contact-rich
  arcade driving and orbital mechanics on a twelve-decade map.
- Game fidelity as a declared profile over the same state an engineering
  study would use.
- Two kinds of multiplayer: a server-stepped ball and lockstep orders.

## The method

**Orbit League.**

- **Bodies and contact** ([chapters 18](ch018-rigid-bodies-and-time-steps.md),
  [20](ch020-find-the-pairs-that-might-touch.md)–[22](ch022-contact-as-an-optimization.md)):
  cars, ball and arena as rigid bodies stepped at 120 Hz substeps; sphere,
  box and mesh contacts; swept tests for the fast ball.
- **Gravity and thrust** ([chapters 41](ch041-burns-consume-mass.md),
  [44](ch044-inside-a-turning-accelerating-ship.md)): a game budget for the
  boost; the drum's centrifugal down and Coriolis on what moves.
- **Rooms** ([chapters 49](ch049-rooms-and-presence.md),
  [50](ch050-shared-state-over-the-network.md)): the room server steps the
  ball at 120 Hz against the cars the players report and sends the state at
  60 Hz.

**Belt 2200.**

- **Orbits** ([chapters 39](ch039-ephemerides-time-scales-and-frames.md),
  [40](ch040-propagate-with-a-stated-error.md)): Kepler orbits and the
  mean-element ephemeris, now with their error against a pinned table stated.
- **Transfers** ([chapters 41](ch041-burns-consume-mass.md),
  [42](ch042-reach-a-moving-target.md)): Lambert transfers offered as economy
  and fast, burns that consume propellant by the rocket equation.
- **Time** ([chapter 46](ch046-time-warp-that-stops-for-every-event.md)):
  game speed that stops for arrivals and burn boundaries.
- **Space for drawing** ([chapter 2](ch002-spaces-and-units.md)): f64
  heliocentric state, a floating origin and f32 only in camera space.
- **Lockstep** ([chapters 5](ch005-replay-a-session-exactly.md),
  [50](ch050-shared-state-over-the-network.md)): orders name the tick they
  take effect; state hashes compared every few ticks name the first desync.
- **Fidelity** ([chapter 48](ch048-one-world-at-three-fidelities.md)): both
  games declare the game profile; presentation changes never move a
  trajectory.

## What exists today

- Orbit League: [orbitleague.coax](../../apps/openagents-web/orbitleague.coax)
  (rules and drawing; `orbitleague.wgsl` traces the scene) over
  [verse/vehicle.coax](../../verse/vehicle.coax),
  [contact.coax](../../verse/contact.coax),
  [gravity.coax](../../verse/gravity.coax),
  [thrust.coax](../../verse/thrust.coax),
  [rigid.coax](../../verse/rigid.coax) and
  [drum.coax](../../verse/drum.coax); the room server in
  [coax/web/room/room.coax](../../coax/web/room/room.coax).
- Belt 2200: [belt.coax](../../apps/openagents-web/belt.coax) over
  [verse/orbit.coax](../../verse/orbit.coax),
  [spacecam.coax](../../verse/spacecam.coax) and
  [stars.coax](../../verse/stars.coax);
  [belt-lockstep.coax](../../apps/openagents-web/belt-lockstep.coax) and
  [belt-net.coax](../../apps/openagents-web/belt-net.coax) for two players.
- The modules' checks in [space.md](../../docs/verse/space.md) (a dropped
  ball returns to e² of its height, a 100 m drum spun for 1 g, Tsiolkovsky
  to 0.1%); the games' checks at `--steps 2000000000`.
- Makers: Orbit League is Topher's and Belt 2200 Ted's
  ([makers.md](../../docs/verse/makers.md)).

## What it builds

- Both games' rules and drawing over `aethyr/`'s physics and space
  chapters; `verse/`'s space modules kept as independent references until
  Aethyr's own pass the same checks.

## Acceptance checks and oracles

- The checks listed in `space.md` pass against Aethyr's bodies, contacts,
  gravity and thrust.
- A ball at 200 m/s never passes a wall at any substep count.
- Belt 2200's transfers, re-propagated by chapter 40, arrive within their
  stated tolerance; the ephemeris error against the pinned table is reported.
- A two-player Belt game replays from its orders to identical state hashes
  on both clients; an injected difference is reported at its tick.
- `belt-2200.png` and Orbit League's goldens change only where a fix is
  visible; the 60 FPS rule holds.

## Dependencies

Chapters 1–22, 39–42, 44, 46, 48, 49 and 50.
