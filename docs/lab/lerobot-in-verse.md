# Robot Lab: LeRobot data, the Verse room, and a Rust physics path

**Status:** Proposal, 2026-10-14. Nothing here is implemented or approved.
No LeRobot source, Python runtime, or dataset enters the repository through
this document.

The [Robot Lab design](README.md) is the plan: an inline chat workbench,
modeled on Video Lab ([#11581](https://github.com/OpenAgentsInc/openagents/issues/11581)),
where you load an open robot, change the scene by talking, and compare runs.
This companion fills in four details it leaves open, each tied to one of its
[delivery slices](README.md#delivery-slices-and-acceptance):

| Detail | Slice |
| --- | --- |
| [Reading LeRobot datasets in Rust](#reading-lerobot-datasets) | 3, Recorded motion |
| [Growing `physics` as an alternative to MuJoCo](#growing-physics-as-an-alternative-to-mujoco) | 4, Simulation |
| [The vision gap and recording in the Verse](#the-vision-gap) | 5, Policy and Gym |
| [The Robot Lab room in the Verse](#the-verse-room) | 6, Verse room |

Crate names follow the design: `robot-description`, `robot-sim`, and
`robot-lab`. This document adds `robot-data` and `robot-policy`.

## Reading LeRobot datasets

A `LeRobotDataset` on the Hugging Face Hub has:

- `meta/info.json`: the format version (`codebase_version`), frame rate,
  robot type, and each feature's name, type, and shape.
- Parquet tables of per-frame rows: `observation.state`, `action`,
  `timestamp`, `frame_index`, `episode_index`, and `task_index`.
- One MP4 stream per camera, such as `observation.images.front`.
- `meta/stats.json`: per-feature statistics that policies normalize with.

Version 3 packs many episodes into each Parquet and MP4 file and locates an
episode through `meta/episodes/`; version 2.1 used one file per episode. The
reader supports the versions of the datasets we pin and refuses the rest with
the version it found.

`crates/robot-data` reads these with the `parquet` and `arrow` crates and
yields typed episodes of timestamps, states, and actions. Video stays
optional: joint replay needs only the tables. Camera frames for slice 3's
scrubber decode through the system `ffmpeg` under `supervise`.

**Joint mapping.** A per-robot table maps feature names (such as
`shoulder_pan.pos`) to the description's joints, with units and offsets.
LeRobot's calibration expresses positions in degrees or in a normalized range
depending on the robot and version, so the table states which one a pinned
dataset uses.

**Accept (slice 3):** a test reads a few frames cut from a pinned,
redistributable SO-101 dataset and reproduces its joint values exactly; a
known recorded pose renders in an offscreen capture.

## Growing `physics` as an alternative to MuJoCo

The design proposes MuJoCo through its C API for slice 4. The other choice is
to grow the shared [`physics`](../../crates/physics/src/lib.rs) crate, which
already has fixed steps, restorable state, contact manifolds, an elliptic
friction cone, torsional friction, sensors, and traces. Its
[joints](../../crates/physics/src/joint.rs) are `Point`, `Weld`, and
`Tether`. An arm needs:

- Hinge (revolute) and slider (prismatic) joints with limits.
- A position servo modeled on the SO-101's Feetech STS3215 motors:
  proportional-derivative control with torque and speed caps.
- Self-collision filtering between neighboring links (the existing
  `contype`/`conaffinity` masks).
- Stiff chains. If the sequential-impulse solver can't hold a six-link arm
  stiff enough at the lab's step rate, this path needs a reduced-coordinate
  articulation solver, which is a much larger project.

| | MuJoCo | `physics` |
| --- | --- | --- |
| Arms and grasping | Mature | To build |
| Language | C dependency behind a Rust wrapper | Rust only |
| Web and phone | Render recorded poses only | Can simulate on every platform |
| Shared with other zones | No | Yes |

**Recommendation:** qualify MuJoCo for slice 4, as the design says, and
measure one servoed hinge chain in `physics` alongside it. Switch only if the
chain holds its tolerances. MuJoCo also serves as the oracle for `physics`, the
way [Genesis](../../crates/physics/oracle/README.md) does now.

**Accept (the `physics` spike):** a servoed hinge reaches a step target within
a stated rise time and overshoot; a hinge at its limit reports the limit
force; a six-link chain driven by a recorded episode's actions tracks the
recorded `observation.state` within a stated error.

## The vision gap

Policies such as SmolVLA and ACT trained on real camera footage expect real
images. The Verse draws a stylized world, so these policies will likely fail
on Verse cameras even with correct joint mappings. Slice 5 should plan for
that from the start:

1. **State-only controllers** (joint state and object poses) work in
   simulation directly and make good scripted baselines.
2. **Record in the Verse.** Teleoperate the simulated arm in the lab, write
   episodes in the `LeRobotDataset` format with offscreen camera streams, and
   train on those offline. The lab becomes a data source as well as a
   consumer, and the datasets can be published to the Hub as an explicit
   action.
3. **Simulation datasets** (ALOHA sim, PushT) were never real images, but
   they still came from a different renderer, so treat them only as a
   stepping stone.

**Accept (recording):** the Python reference loader opens a dataset the
Verse wrote, and its statistics match.

`crates/robot-policy` holds Candle implementations of the policies we adopt,
with golden fixtures generated offline by the Python reference, the way
[`kev`](../../crates/kev/) and [`laya`](../../crates/laya/) carry theirs.
ACT (a ResNet-18 backbone, a small transformer, and action chunking) is the
smallest useful first port; SmolVLA follows. Weights stay out of git.

## The Verse room

Slice 6 mounts the same lab in the Verse. The
[Physics Lab](../verse/physics-lab.md) is the template: a zone crate
(`verse-zone-robotlab`) with a bench, one or two SO-101 arms, blocks and cups,
an episode picker, and knobs for playback speed and for recorded against
simulated joint angles. It reads the same lab documents and saved episodes as
the chat card, so a run started in chat can be watched in the room. Before
the room exists, a Physics Lab scenario that replays one episode is a cheap
way to see recorded motion in the world.

## Open questions

- Which SO-101 dataset to pin first: single-arm, redistributable, and small.
- Whether the Verse room is local-only, like the other labs, or shared with
  other players over NIP-MV.
- Where trained weights live and how a zone fetches them; the Everglade
  pinned-pack path is the likely model.
- Whether to drive a real SO-101 from Rust over the Feetech serial protocol.
  The design leaves physical hardware for later.
