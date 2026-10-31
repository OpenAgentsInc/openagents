# Orbit League

Pitched by Topher on the October 30, 2026 recording: "a racing game like
Rocket League, but with Tesla Roadsters, in space."

Status: playable at https://openagents.com/realms/orbit-league: you against
one bot in a glass box arena with spin-downs to zero-g (milestone 1, with
parts of 2 and 3). The roadster is drawn as low-poly hulls (wedge body,
fender pods, windscreen, speedster humps, the driver's helmet, a rear wing,
glowing head and tail lights); the floor and glass reflect the ball and the
cars; a ringed ice giant, a moon and the station's ribs sit outside the glass.
A goal blasts a shell of light and sparks in the scorer's colour; hard hits
throw sparks and shake the camera; the ball and the boost leave trails. Six
big and ten small boost pads; the dodge is in `verse/vehicle.coax`.

## The game in one line

Electric roadsters with rocket packs play three-a-side ball in a glass arena
in orbit, where gravity is whatever the arena is spinning at.

## Naming and brand

The idea comes from the red roadster that was launched into space in 2018.
The cars in the game are our own design: a low two-seat electric roadster,
original body, original badge, no carmaker's name, logo or trade dress. A
driver in a white suit with the visor down is our own character too. If a
carmaker ever wanted to license its car in, that is a separate deal; the
game must not depend on it.

## How it plays

- **Teams:** 1v1, 2v2 or 3v3. Five-minute matches; a tied match goes to
  golden goal.
- **The ball:** two metres across, light, and it keeps spinning. You drive
  into it to hit it. A hit's direction comes from where on the car it
  lands, how fast you are going, and your spin.
- **The car:** drive, steer, drift (handbrake), jump, double jump, and
  dodge (a flip that punches the car in a direction). **Boost** is a rocket
  under the rear bumper: hold it to accelerate, including in the air.
  Boost comes from pads on the arena floor and walls.
- **Space is the twist.** The arena is a ring station. Its spin makes
  gravity toward the outer wall, and the spin can change during a match:
  - **Full spin:** about 1 g. It plays like a normal ground game.
  - **Half spin:** about 0.4 g. Long jumps, big aerials.
  - **Spin-down events:** every 90 seconds the station's spin drops to
    zero for 15 seconds. The ball floats, cars float, and boost is the only
    way to move. Every car gets a full boost tank when it starts, so
    zero-g is the highlight, not a stall.
  - **Arena shapes:** a ring (play on the inside of a drum, so the floor
    curves up into the walls and ceiling) and a sphere (the goals at the
    poles).
- **Driving on any surface.** Wheels stick to the floor, walls and ceiling
  while they are touching and the car is fast enough, as in other car-ball
  games. In the drum arena the floor simply keeps curving.
- **Goals:** a goal explodes in that team's colour and knocks every car
  back. A replay of the last five seconds plays from three cameras.
- **Cameras:** a chase camera behind the car; ball cam (keep the ball on
  screen, toggled with one button); a free camera when spectating.
- **Phone:** left thumb steers; right side has JUMP, BOOST and
  ball-cam. Dodge is jump then a flick of the left thumb. Gyro steering is
  an option (the iOS app already turns the phone's gyro into camera look
  with `coaxial.tilt`).

## What it reuses

| Need | What we have | Where |
|---|---|---|
| Rigid bodies in f64 with quaternions | Newton for the centre of mass, Euler's equations, RK4 rotation, sleeping | `verse/rigid.coax` |
| A ball that flies, bounces and rolls | Thrown balls on terrain, knocking scenery | `verse/throw.coax` |
| Cameras that blend shots | Overview, dolly and free orbit as blended shots | `verse/camera.coax` |
| Boost trails, sparks, goal explosions | Particle emitters drawn in one instanced pass | `verse/particles.coax`, `verse/particle-motion.coax` |
| Phone thumbs and buttons | Sticks, hotbar slots, long-press tooltips | `verse/grid.coax` (sticks), `verse/hud.coax`, Everglade's hotbar |
| Deterministic replays | Ice Cream Man's seeded rules replay traces against a reference | `verse/icecream/replay.c`, `verse/icecream/trace.coax` |
| Rooms and names over players | The Grid's multiplayer (positions five times a second) | `apps/openagents-web/thegrid-people.coax`, `apps/openagents-web/room.coax` |
| Analytics per realm | Counted actions, sent every 30 s | `apps/openagents-web/analytics.coax` |

## What the engine needs

1. **Contacts for rigid bodies** (`verse/rigid.coax`): sphere-against-box,
   box-against-box (the car as an oriented box), sphere-against-mesh (the
   arena), with friction and restitution. Swept tests for the ball at
   speed, so it never passes through the goal frame. Dead Gears already
   sweeps spheres against boxes (`verse/deadgears/arena.coax`
   `arena_sweep`); generalise it.
2. **A vehicle controller**: a new `verse/vehicle.coax` with four wheel
   rays, suspension springs, tyre grip, a handbrake, and "sticky" driving
   on walls and ceilings when the wheels touch. The jump, double jump and
   dodge impulses are fixed numbers, so skill shots repeat exactly.
3. **Changing gravity**: gravity as a field (toward the drum's outer wall,
   scaled by spin), not a constant. `verse/player.coax` and
   `verse/throw.coax` take a constant gravity today.
4. **Fast authoritative multiplayer**: a WebSocket room on the API with a
   60 Hz server tick, client prediction for your own car, interpolation for
   others, and rollback on correction. The ball must be the server's.
   Today's rooms are HTTP at five posts a second (`coax/web/api/room.c`).
   Matchmaking: a queue per mode, with bots filling empty seats.
5. **Bots**: a car-ball bot that drives to an intercept point and hits
   toward the goal. Plain code, no model calls, so a match plays offline.
6. **Replays**: record inputs per tick and replay them through the same
   deterministic physics. The goal replay and a shareable clip both come
   from that. Ice Cream Man's trace and replay tools are the pattern.
7. **A curved arena mesh**: the inside of a drum as the drivable surface
   and the collision mesh, drawn in the flat or ink style
   (`docs/verse/art-styles.md`).

## Milestones

1. **Pitch.** One car, one ball, a flat floor in 1 g, a goal. Contacts and
   the vehicle controller. Offline.
2. **Space.** The drum arena, spin and spin-down, boost pads, zero-g.
3. **Bots.** 1v1 and 2v2 against bots, so the game is playable alone.
4. **Online.** The 60 Hz room, prediction and rollback, quick match.
5. **Show-off.** Goal replays, clips, car colours and decals made by
   players.

## Open questions

- Does spin-down every 90 seconds feel fun or chaotic? Playtest with 60 s,
  90 s and off.
- Is three-a-side too many for phones? Start with 2v2 as the default.
