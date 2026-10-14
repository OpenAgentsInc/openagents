# Robot Lab: LeRobot data and policies in the Verse

**Status:** Proposal, 2026-10-14. Nothing here is implemented or approved.
No LeRobot source, Python runtime, or dataset enters the repository through
this document.

The Robot Lab is a proposed Verse zone where real robot arms, rebuilt in
Rust, replay demonstrations recorded with Hugging Face's
[LeRobot](https://github.com/huggingface/lerobot) and then run learned
policies on their own. It follows the pattern the
[Genesis port](../physics/2026-09-27-genesis-port-roadmap.md) set: the
Python project is a reference and an offline oracle, and every mechanism we
use is reimplemented in Rust in a shared crate that zones consume.

## Short answer

Yes, we can use LeRobot's data, at three levels of difficulty:

| Level | What the player sees | What it needs | Difficulty |
| --- | --- | --- | --- |
| 1. Replay | A Verse arm moves exactly as a real arm moved in a recorded episode. | A robot description loader, forward kinematics, and a dataset reader. No physics. | Small |
| 2. Physical replay | The recorded actions drive simulated servos; the arm and the objects it touches obey `physics`. | Hinge joints with limits and position-servo motors, articulated chains, and contact at the gripper. | Medium |
| 3. Policies | A trained policy watches the arm's own cameras and joint state and decides the next action live. | A Rust policy runtime (Candle), in-world cameras rendered offscreen, and demonstrations recorded in the Verse itself. | Large |

Level 1 is a good first milestone because it shows real data in the world
with almost no new engine work. Level 3 has a hard limit described in
[The vision gap](#the-vision-gap).

## What LeRobot provides

- **Datasets** on the Hugging Face Hub, in the `LeRobotDataset` format:
  a `meta/info.json` that declares the frame rate, the robot type, and each
  feature's name, type, and shape; Parquet tables of per-frame rows
  (`observation.state`, `action`, `timestamp`, `frame_index`,
  `episode_index`, `task_index`); and one MP4 stream per camera. Version 3 of
  the format packs many episodes into each Parquet and MP4 file and locates
  an episode through `meta/episodes/`; version 2.1 used one file per
  episode. The reader must support whichever versions the datasets we pin
  use, and check `codebase_version` in `info.json`.
- **Robot descriptions.** The SO-100 and SO-101 arms, the most common
  LeRobot hardware, are open designs with URDF and MJCF models and STL
  meshes published by The Robot Studio and in MuJoCo Menagerie.
- **Policies** with pretrained weights in `safetensors`: ACT, Diffusion
  Policy, VQ-BeT, TD-MPC, π0, π0-FAST, and SmolVLA (about 450M parameters).
- **Simulation environments** (ALOHA, PushT, xArm, LIBERO) whose datasets
  were rendered from simulation rather than real cameras.

Each dataset carries its own license on its Hub card. Pin a dataset by
repository and revision, record its license next to the pin, and admit only
datasets whose license allows redistribution of derived motion.

## What Verse already has

| Need | Today | Gap |
| --- | --- | --- |
| Rigid bodies, fixed step, restorable state, traces | [`physics`](../../crates/physics/src/lib.rs): `World`, `FixedStep`, `Trace` | None for level 1 |
| Joints | [`joint.rs`](../../crates/physics/src/joint.rs): `Point`, `Weld`, and `Tether`, hard or soft, with force and torque caps | No hinge (revolute) or slider (prismatic) joint, no angle limits, no motor |
| Contact and friction | Box manifolds, elliptic Coulomb cone, torsional friction (GP-3) | Meshes for gripper fingers; small, light objects need tuning |
| Sensors | Contact force, IMU, and raycast sensors (GP-7) | Joint position and velocity readouts; cameras as sensors |
| A place to show mechanisms | The [Physics Lab](../verse/physics-lab.md) zone and its knobs, readouts, and overlay | A scenario or zone for arms |
| Neural inference in Rust | Candle 0.11 in [`kev`](../../crates/kev/) and [`laya`](../../crates/laya/), reading `safetensors` | Vision backbones and policy heads (ResNet, ViT, action chunking, diffusion sampling) |
| Offscreen rendering | `verse --capture` renders a view without a window | A per-frame camera feed at a fixed resolution, attached to a body |
| Reference oracle | [`physics/oracle`](../../crates/physics/oracle/README.md) replays scenes in Genesis offline | The same idea for LeRobot: reference actions from the Python policy |

## Proposed shape

- **`crates/robot`** (new, zone-agnostic, no renderer or I/O beyond reading
  files it is given): a URDF reader for the subset arms use (links,
  revolute and prismatic joints, limits, inertials, mesh references), a
  kinematic tree with forward kinematics, and a builder that turns the tree
  into `physics` bodies and joints. Inverse kinematics comes later, for
  teleoperation.
- **`crates/robot-data`** (new): a `LeRobotDataset` reader over Parquet
  (the `parquet` and `arrow` crates) that yields typed episodes of
  timestamps, states, and actions, plus the dataset's `stats.json` for
  normalization. Video decoding stays optional: level 1 and level 2 need
  only the joint streams. A thumbnail or a picture-in-picture panel can
  decode frames with the system `ffmpeg` through `supervise`.
- **Joint mapping.** A per-robot table that maps dataset feature names
  (such as `shoulder_pan.pos`) to URDF joints, with units and offsets.
  LeRobot's calibration can express positions in degrees or in a normalized
  range depending on the robot and version, so the table states which one a
  pinned dataset uses, and a test checks a known pose.
- **`physics` additions:** hinge and slider joint kinds with angle or
  distance limits, and a position servo modeled on the arms' Feetech
  STS3215 motors (proportional-derivative control with a torque cap and a
  speed cap). These are generic mechanisms and belong in `physics`, with
  tests there, like every Genesis port phase.
- **`crates/verse-zone-robotlab`** (new): the zone. A bench with one or two
  SO-101 arms, a few blocks and cups, a dataset and episode picker, and
  knobs for playback speed, kinematic or physical replay, and an overlay of
  recorded against simulated joint angles. It may start as a Physics Lab
  scenario and move to its own zone when it grows.
- **`crates/robot-policy`** (new, level 3): Candle implementations of the
  policies we adopt, each with golden fixtures generated offline by the
  Python reference, the way `kev` and `laya` carry theirs. Weights stay out
  of git, as `~/work/kev-artifacts/` does.

Python stays offline tooling: a pinned LeRobot checkout in a scratch virtual
environment produces golden actions for given inputs, and Rust tests compare
against them. No Verse build, test, or runtime depends on it.

## The vision gap

Policies trained on real camera footage expect real-looking images. The
Verse draws a stylized world, so a policy such as SmolVLA or ACT trained on
real SO-101 footage will see out-of-distribution images and should be
expected to fail when it reads Verse cameras. Three ways around it:

1. **State-only policies.** Train on joint state and object poses, with no
   images. This works in the Verse directly but transfers poorly to a real
   arm.
2. **Record in the Verse.** Teleoperate the simulated arm in the Robot Lab,
   save episodes in the `LeRobotDataset` format, and train on those. This
   makes the Verse a data source as well as a consumer, and the same
   datasets can be published to the Hub.
3. **Simulation datasets.** Start from LeRobot's simulation datasets
   (ALOHA sim, PushT), whose images were never real. Matching their
   renderer still matters, so treat them as a stepping stone.

Option 2 is the one that fits the Verse best; options 1 and 3 are useful
first tests.

## Phases

Each phase lands as its own change with tests, in the order below.

### RL-0: Robot description and forward kinematics

Load the SO-101 URDF into `crates/robot`, compute every link's pose from
joint angles, and draw the arm in the Physics Lab with angles from knobs.

**Accept:** forward kinematics matches the reference implementation's
link poses for a set of fixed joint vectors within 1e-6 m and 1e-6 rad.

### RL-1: Kinematic replay

Read a pinned SO-101 dataset with `crates/robot-data`, map its features to
joints, and replay one episode in the lab at the dataset's frame rate.

**Accept:** a test reads a small checked-in fixture (a few frames cut from a
pinned dataset with a redistributable license) and reproduces the joint
values exactly; an offscreen capture shows the arm in a recorded pose.

### RL-2: Hinge joints, limits, and servos in `physics`

Add revolute and prismatic joints with limits and a position servo with
torque and speed caps.

**Accept:** a single servoed hinge reaches a step target with the expected
rise time and no overshoot beyond a stated bound; a hinge held at its limit
reports the limit force; momentum ledger tests still pass.

### RL-3: Physical replay

Drive the simulated arm's servos with recorded actions and compare the
simulated joint angles with the recorded `observation.state`. Add blocks so
grasps interact through contact.

**Accept:** mean joint tracking error over an episode stays within a stated
bound, and a recorded pick episode lifts its block in simulation.

### RL-4: Record in the Verse

Teleoperate the simulated arm (a second arm as leader, mouse IK, or a real
SO-101 leader arm over serial later) and write `LeRobotDataset` episodes
with offscreen camera streams.

**Accept:** the Python reference loader opens a dataset written by the
Verse, and its statistics match.

### RL-5: First policy

Port ACT to Candle (a ResNet-18 backbone, a small transformer, and
action chunking; small enough for a laptop GPU), train it on Verse-recorded
episodes with the Python reference offline, and run it live in the zone.

**Accept:** Candle's actions match the Python reference's on golden inputs
within a stated tolerance, and the policy completes a pick task in the lab
at a measured success rate.

SmolVLA, π0, and real-hardware control come after RL-5 and need their own
proposals.

## Open questions

- Which dataset to pin first. It should be SO-101, single-arm,
  redistributable, and small.
- Whether arms run in maximal coordinates on the existing sequential-impulse
  solver or need a reduced-coordinate articulation solver for stiff chains.
  RL-2 and RL-3 measurements decide.
- Whether the Robot Lab is local-only like the other labs, or whether other
  players see a shared arm over NIP-MV.
- Where trained weights live and how a zone fetches them; the Everglade
  pinned-pack path is the likely model.
- Whether to drive a real SO-101 from Rust over the Feetech serial protocol,
  which would let a Verse policy move a physical arm.
