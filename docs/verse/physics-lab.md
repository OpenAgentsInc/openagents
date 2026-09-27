# Physics Lab: the shared physics crate, live

The **Physics Lab** is a Verse zone where you watch each mechanism of the
shared [`physics`](../../crates/physics/) crate run live and change it with
on-screen knobs. It has nine scenarios, from a contact manifold to a
24-thruster attitude controller. Each one is a small scene on
`physics::World`, stepped at a fixed rate and drawn with the shared Verse mesh
pipeline.

The lab is a teaching and debugging surface for the crate that
[Lagrange 1](lagrange-1.md) and later zones build on. It follows the
[Genesis port roadmap](../physics/2026-09-27-genesis-port-roadmap.md)
(tracking issue #9788). Its geometry is generated in Rust, so nothing is
downloaded.

| Part | Code |
| --- | --- |
| Scenarios: build, step, and readouts; no rendering | [`zones/lab/scenes.rs`](../../crates/verse/src/zones/lab/scenes.rs) |
| Knobs, fixed-step clock, HUD text, and snapshot | [`zones/lab/mod.rs`](../../crates/verse/src/zones/lab/mod.rs) |
| Hall, stage, bodies, and overlays | [`zones/lab/draw.rs`](../../crates/verse/src/zones/lab/draw.rs) |
| Tests | [`zones/lab/tests.rs`](../../crates/verse/src/zones/lab/tests.rs) |
| Offline capture | [`examples/lab_capture.rs`](../../crates/verse/examples/lab_capture.rs) |

## Enter and leave

The lab's arch, lettered `PHYSICS LAB`, stands 12 m behind the plaza spawn:
turn away from the computer and the pylon and walk straight back. On the map,
choose **Lab portal**.
Near the arch, tap its opening or select **Enter Lab**; on desktop, press
**F**.

You arrive beside a railed stage with the scenario on it. The first scenario
is the contact manifold. Walk around the rail and orbit the camera to look
from any side. **Plaza**, or the return arch behind you and to your right as
you arrive, takes you back to where you left the plaza. Leaving releases the lab's
simulation and geometry; entering again starts fresh.

The lab is local-only, like the other zones. Plaza presence pauses while you
are inside.

## Controls

The zone HUD shows a four-line caption above two rows of controls:

- The scenario, its number, the time scale or **paused**, and gravity.
- The selected knob, its place in the list, and its value, as
  `[5/6] Load = 0.50 × µN`.
- Two lines of readouts from the running scenario.

| Control | Desktop key | Effect |
| --- | --- | --- |
| **Prev**, **Next** | `1`, `2` | Select the previous or next knob. The list wraps. |
| **-**, **+** | `3`, `4` | Move the selected knob down or up one option. |
| **Reset** | `5` | Rebuild the scenario from its knobs. |
| **Pause** / **Run** | `6` | Pause or resume the simulation. |
| **Step** | `7` | Pause and advance exactly one fixed step (1/120 s). |
| **Plaza** | `8` | Return to the plaza. |

On desktop, the number keys press the zone's controls in order, so they also
work in other zones. On a phone, tap the same controls. The HUD, its hit
testing, and its accessibility actions are the shared Rust zone HUD. iOS and
Android admit the lab's zone ID (`physics_lab`) and its intents (`knob_prev`,
`knob_next`, `decrease`, `increase`, `reset`, `pause`, and `step`).

### Knobs

Every scenario starts with four shared knobs, followed by its own:

| Knob | Options | Effect |
| --- | --- | --- |
| Scene | The nine scenarios | **+** and **-** switch scenarios and wrap around. Switching sets gravity to the scenario's default. |
| Time scale | 0.1×, 0.25×, 0.5×, 1×, 2×, 4× | Multiplies frame time before the fixed-step clock. |
| Gravity | zero-g, uniform | Uniform is 9.81 m/s² down. Changes apply on the next step. |
| Overlay | off, on | Shows contacts, normals, contact impulses, and joints. |

Each knob has a closed list of options, so every setting is reproducible. A
*live* knob, such as a load, a torque, or a joint limit, applies on the next
step. Any other knob rebuilds the scenario. Each scenario remembers its knob
settings while you visit the others.

## Stepping and determinism

The lab steps like Lagrange 1: frame time, multiplied by the time scale,
accumulates in a `physics::FixedStep` of 1/120 s, and whole steps run. A frame
runs at most 48 steps, enough for 4× at 20 frames per second. The renderer
interpolates each body between its last two poses.

A scenario is a function of its knob settings and the gravity flag. It uses
no randomness, clock time, or I/O; the scattered pile comes from a fixed hash.
**Reset** therefore replays bit for bit, and the frame rate does not change the
result. Tests check both. Scenarios that loop on their own, such as the
tunneling shots, rebuild the same world each time.

## Scenarios

| # | Scenario | Mechanism | What you see | Knobs |
| --- | --- | --- | --- | --- |
| 1 | Box-on-box manifold | Narrow phase: separating-axis box–box with face clipping, at most four points | A scripted wireframe box sweeps through flat contact, a 10° rock onto an edge, a 45° yaw, a 60 mm press, a slide off the edge, and a lift apart, every 12 s. Contact points, normals, and the manifold polygon are drawn. | Sweep (automatic or manual), tilt, yaw, penetration, slide |
| 2 | Friction breakaway | Coulomb friction on an elliptic cone | A 2 kg box on the floor under a steady sideways load. It holds (green) below µN and slips (red) past it. Orange shows the load and red the friction force the floor returns. | Load as a fraction of µmg (live), friction µ |
| 3 | Torsional friction | Torsional friction about the contact normal | A 1 kg ball spun by a torque about the vertical. It holds below the torsional limit, µ_t N, and spins up past it. | Torque (live), torsional coefficient |
| 4 | Tunneling | Speculative contacts within the distance a body closes in one step | A sphere, a box, and a capsule fired at a 4 cm panel. They stop at its surface at every speed up to 200 m/s, which is 1.67 m per step. | Speed |
| 5 | Zero-g momentum | Momentum-conserving contacts; `physics::Ledger` | A cube strikes a tank and a truss in zero g. The caption shows total linear and angular momentum and the ledger's relative error. With gravity on, the lab records gravity as an external impulse, so the ledger stays balanced. | Cube speed, cube spin |
| 6 | Stack and pile | Contact solver iterations, restitution, and island sleep | Boxes stacked or dropped on the floor. The caption counts awake and asleep boxes; asleep boxes draw dimmer. | Mode, count, restitution, friction µ, iterations (live), island sleep (live) |
| 7 | Soft grip | A soft weld: an implicit spring by frequency and damping ratio, with force and torque limits | A 5 kg part hangs from a kinematic hand, settles, and then follows the hand as it sways. The part sags by its static deflection, g/ω². Below the part's weight, the grip saturates and the part slips away. | Frequency, damping ratio, force limit (all live) |
| 8 | Tether and weld | Point joints, tethers that only pull, and hard or soft welds | A rigid pendulum on a point joint; a box that falls with slack until its tether catches it; and an inverted-T pair of welded boxes that a ball strikes and moves as one. | Tether length, impact speed, weld (hard or soft) |
| 9 | Thrusters | `ThrusterSet::box_corners`, the bounded allocator, and `Pid` | A 6 kg craft with 24 thrusters at its corners recovers from a tumble, holds attitude, and flies a commanded path. Firing thrusters show orange plumes, and a green cross marks the target. | Command (hold, right 1 m, up 0.5 m, yaw 90°, square; live), start still or tumbling |

Scenarios 4, 5, and 9 start in zero g; the others start under gravity. You can
toggle gravity in any scenario. In zero g, the friction and torsion limits are
zero because nothing presses the body on the floor, so any load slips.

## Colors and overlay

The lab uses its own blueprint colors, not the plaza's amber. Solid bodies have
dark faces and colored edges:

| Color | Meaning |
| --- | --- |
| Cyan | An awake dynamic body |
| Dim cyan | A sleeping body |
| Amber | A kinematic or scripted body |
| Gray | A fixed body or joint pivot |
| Green, red | Holding or slipping, in the friction, torsion, and grip scenarios |

With **Overlay** on, the lab draws the lines from `World::debug_lines`:
contact normals (magenta), contact impulses (yellow), joints (green, or red at
their limit), plus a cross at each contact point and joint anchor.

## Physics API gaps and findings

The lab uses `crates/physics` as it stands on `main` and does not change it.
It works around these gaps in the zone:

- **Detection needs a responding body.** `World::detect` skips pairs in which
  neither body can respond. The manifold scenario scripts a dynamic stand-in
  that it never steps, as the crate's own manifold test does.
- **Loads do not wake sleepers.** `Body::apply_force` and `Body::apply_torque`
  on a sleeping body are ignored. The lab wakes each body it pushes (the
  friction box, the torsion ball, and the craft) every step.
- **Moving kinematic bodies do not wake sleepers.** A sleeper joined to a
  moving kinematic body stays asleep, because only an awake dynamic body wakes
  what it touches. The soft grip's part slept while the hand was still and then
  stayed put as the hand swung away. The lab wakes the part while the hand
  moves.
- **The thruster allocator has a deadband.** Its first-phase fuel penalty
  returns zero throttles for small requests: below roughly 0.3 N·m of torque
  on the lab's craft. A soft attitude loop stalls several degrees off target, and an
  integral term winds up during tumble recovery and then cancels the
  proportional term. The lab uses stiff proportional-derivative gains, which
  leave about 1° of error.
- **Tall stacks creep in yaw.** A straight stack of five 0.4 m boxes rotates
  about the vertical at about 0.02 rad/s at the default settings. That rate
  sits at the sleep threshold, so the stack never sleeps. A stack of three
  sleeps. The stack scenario starts with three boxes; choose five or more to
  see the creep.
- **Bodies cannot be removed.** Scenarios that fire or drop objects on a loop
  rebuild the world instead.

## Tests and capture

`cargo test -p verse zones::lab` covers the following:

- Entering from the plaza and returning to the saved pose.
- Every scenario building, stepping, and drawing without non-finite values,
  with and without gravity.
- Every knob option building and stepping.
- The friction box holding at 0.5 load and slipping at 1.2.
- The torsion ball holding at 0.45 N·m and slipping at 0.55 N·m.
- No projectile passing the panel at 2, 40, or 200 m/s.
- The momentum ledger staying balanced with and without gravity.
- Sleep counts and the sleep knob.
- The soft grip's sag, its following the hand, and its slip past the force
  limit.
- The tether catch and the weld holding under impact.
- Thruster recovery and commanded moves.
- **Reset** replaying bit for bit for every scenario.
- Results matching at 30 and 60 frames per second.
- The intents and their serialized names.

To render a scenario offline with the shared renderer, run the capture
example with an output path, a scenario number from 1 to 9, and seconds of
simulated time:

```sh
cargo run -p verse --release --example lab_capture -- target/verse/lab.png 8 3
```
