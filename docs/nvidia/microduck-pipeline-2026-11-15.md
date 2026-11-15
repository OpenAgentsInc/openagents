# Microduck: the complete OpenAgents robot-learning pipeline

**Date:** November 15, 2026. **Status:** specification and implementation plan.
Owner-selected goal: reproduce the complete small-robot development pipeline
demonstrated at GTC Berlin using OpenAgents' own maintained stack, including
training new skills and deploying them to physical hardware. No part of this
new capstone is qualified merely by specifying it.

## 1. The demonstration and what its sources establish

The event was **Jensen Huang's October 21, 2026 GTC Berlin keynote**. The
robots were **Microducks from Pollen Robotics/Hugging Face**. The development
film starts around **1:20:44** in the
[full keynote](https://www.nvidia.com/en-us/on-demand/session/gtcberlin26-sb61112/).
NVIDIA also publishes the focused
[RTX Spark Brings Microducks to Life clip](https://www.youtube.com/watch?v=zHBrUVyQJWc).
Its [official recap](https://blogs.nvidia.com/blog/gtc-berlin-2026-keynote/)
describes four robots appearing onstage and sitting, standing and lying down
on cue. The stage performance establishes visible behavior; it does not
establish the origin of every command, speech autonomy or a general success rate.

The narrated development sequence is: describe a course and desired behavior;
agents plan and create assets; assemble and simulate the robot and environment;
train many robots in parallel; evaluate the learned policy locally; deploy
to the robot. NVIDIA's film names Hermes/Nemotron and Astra for agent work,
FreeCAD/Blender for assets, and Isaac Sim/Newton/Isaac Lab for simulation and
learning. This is the capability sequence we want to own, with replaceable
models and hardware rather than mandatory NVIDIA products.

Three primary implementation references have different scopes:

- NVIDIA's [October 23 agent-workflow article](https://developer.nvidia.com/blog/using-ai-agents-to-teach-robots-like-microduck-to-walk-in-simulation/)
  covers planning, editable assets, scene assembly, perception/locomotion
  training and local simulation validation. It describes 4,096 environments
  and a head-camera depth input. Its physical-transfer discussion is a further
  refinement step, rather than evidence that every generated course was
  completed by hardware.
- NVIDIA's [October 21 sim-to-real tutorial](https://huggingface.co/blog/nvidia/microduck-isaac-lab-sim-to-real)
  describes Isaac Lab/Newton walking, actuator modeling, randomization,
  evaluation, ONNX export and robot deployment. Its stated walking contract
  is 61 observation values to 14 joint targets; its parameters must not be
  silently combined with a different vision policy.
- Pollen's [training repository](https://github.com/pollen-robotics/microduck_rl)
  uses mjlab/MuJoCo Warp and PPO, and publishes walking, recovery, sit/stand,
  rolling and other task families. Its [robot runtime](https://github.com/pollen-robotics/microduck)
  runs exported policies on the embedded computer. These are additional
  references, not proof that the keynote used precisely the current files.

Inspected on November 15, 2026. Pin revisions, exact artifacts and complete
contracts when implementing a reference profile. Do not infer an identical
training run from a similar robot, network shape or demonstration video.

## 2. Completion means the whole pipeline

This is a required capstone of the
[open robotics platform](open-robotics-platform-2026-11-15.md), with a
[Psionic book chapter](../../psionic/book/ch256-build-and-teach-a-small-robot.md).
The tabletop arm remains an independent product route; its purchase or
completion is not a prerequisite for the biped program.

Our final route must accept a task brief, produce versioned assets and an
admitted robot, train a new neural controller, evaluate it, package it and run
the qualified controller on a physical small biped. We must also demonstrate
perception-guided movement and explicit skill selection across several robots.
A viewer playing an imported policy is an interoperability milestone, not
completion of our training pipeline.

Separate three kinds of intelligence:

1. **Development agent:** Coder plans, edits assets/configurations, submits
   bounded experiments and interprets results through existing capabilities.
2. **Task and skill supervisor:** checked Coax code selects a qualified skill
   and bounds requested movement. Every selection records whether it came
   from a person, a script, a learned router or a perception-based planner.
3. **Learned controller:** a Psionic actor maps admitted observations to
   action targets, with local device supervision before actuation.

The first locomotion actor need not be a VLA. GR00T and Cosmos are optional
later integrations, with their own embodiment and task gates. The development
agent's language model is not the motor controller.

## 3. Required stages, owners and evidence

| Stage | OpenAgents deliverable | Acceptance before promotion |
| --- | --- | --- |
| Task brief and plan | Coder records robot, course dimensions, skills, budget, outputs and frozen evaluator | Agent proposals cannot edit held-out tasks, acceptance criteria or hardware authority |
| Editable assets | Dimensioned course, ramps, obstacles, task props and optional printable robot modules in the shared registry | Units, source rights, collision geometry, material assumptions, physical dimensions and artifact hashes checked independently |
| Robot admission and rigging | Engine model with links, joints, actuators, inertias, limits, calibrated frames and sensors | Compare admitted values, FK and selected dynamics with independent references; unsupported model features refuse |
| Actuator identification | Measured or explicitly sourced servo model including control law, torque/current limits, friction, delay, voltage and gearbox play | Bench/reference response and held-out motions establish the declared envelope; ideal PD alone cannot qualify transfer |
| Physical environment | Engine articulated/contact scene and bounded stepping | Ground, foot/body contact, falls, limits and reset behavior checked; the full-contact skill model is identified separately from any simplified walking model |
| Parallel collection | Isolated environment batches with per-environment RNG, resets, observations, actions and termination/truncation semantics | Solo-versus-batch invariance in the deterministic profile; resetting one robot leaves other environments unchanged |
| Learning | Psionic neural actor and learned critic, GAE/PPO, optimizer, normalization, curriculum and whole-experiment checkpoints | Actual gradients and updates change the actor; fresh predeclared seeds, full curves, resume and held-out quality/cost gates retained |
| Local evaluation | Headless workbench runner loads a specific trained artifact | Quantitative tracking, stability, fall/recovery, energy and task metrics; complete failure ledger and independent reference comparison |
| Perception and navigation | Engine camera/depth observations, Psionic encoder or qualified geometric perception, local planner and walking-policy interface | Actor sees only declared onboard-available inputs; obstacle avoidance and course completion judged separately from balance |
| Export and package | Psionic native policy package and bounded ONNX interchange where the graph is supported | Tensor/action/normalization identity, independent output comparisons, malformed artifact refusals and exact model/assembly binding |
| Physical deployment | Local Coax device gateway, command/skill supervisor, calibration, stop and rollback | Measured target timing, sensor age, actuator behavior and fault cases on the exact robot; no cloud dependency in actuator supervision |
| Multiple robots | Bounded mission/cue coordinator plus independent local policy state and authority | Per-robot commands/cancellation, collision separation and disconnect faults; every intervention and cue source visible |
| Improvement and manufacture | Failure replay, authorized new training rows, repaired parts and revised releases | Held-out exposure ledger, predeclared promotion gate, new production/calibration evidence when hardware changes |

Engine code remains below Psionic and product contexts. Extend shared
articulation, collision, sensors, rendering and replay; do not create a second
robotics simulator. General-purpose numerical extraction must respect
[the context map](../architecture/contexts.md). Product logic, collectors,
training tools and deployment policy are Coax; C handles platform effects.

## 4. The controller and transfer contract

Specify each observation in order: source sensor/state, frame, unit,
calibration, timestamp, scaling, clipping and history. Include previous actions
and recurrent state if the actor uses them. Keep actor-visible inputs separate
from privileged critic/teacher values; deployment must work without simulator
truth. A depth-trained policy needs a qualified real depth path or an evaluated
student using available sensors.

For every action, specify joint ordering, zero/sign, target units, home offset,
scale, saturation, rate limits and the mapping into the actuator control mode.
For upstream compatibility, audit the distinction between the learned joint
set and all physical motors, including the independently controlled beak.
The runtime's [control-loop design](https://github.com/pollen-robotics/microduck/blob/main/docs/design/robotd-design.md)
describes a 50 Hz application loop and a 14-output learned policy within a
15-servo robot. That is a versioned reference to inspect, not an OpenAgents
hardware timing qualification.

Package observation normalization with the actor; never normalize a second
time because export already incorporated it. Retain the training transforms,
weights, precision and expected input/output vectors. Current Psionic
[ONNX import](../../psionic/book/ch220-read-a-bounded-onnx-graph.md) and
[export](../../psionic/book/ch221-write-a-bounded-onnx-graph.md) cover bounded
subsets. Extend the shared codec/executor for required nonlinearities and
normalizer operations or refuse the graph. A compatible filename is insufficient.

Randomization must be justified by measured uncertainty or an explicitly
declared robustness experiment: mass/COM, friction, inertias, encoder zero,
latency, sensor noise/dropout, voltage/sag, backlash, pushes and terrain.
Hold out parameter combinations, terrain, course layouts and physical units.
Freeze distributions, metrics, seeds, horizons and selection rules before
training. Report every prescribed seed; do not present the best checkpoint
or one successful performance as a reliability estimate.

Model the sensor/encoder location relative to gearbox play; joint state on
the motor side and output side can differ. Keep physics step, servo feedback,
actor update, perception and mission rates separate. Select measured physical
deadlines rather than copying the simulated Panda or upstream duck schedule.

Physical introduction progresses through observation-only replay, an isolated
actuator bench, supported/restrained commissioning and a bounded test area.
Qualify the particular restraint/support method and stop outcome, including
gravity and falling. Policy updates need explicit arming and reversible
activation; old epochs, stale state and command expiry cannot be bypassed by
an agent or remote coordinator. Physical operation remains a separately
authorized integration; this planning change purchases or actuates nothing.

## 5. Skills and public demonstration

The required first skill set is standing/balance, commanded walking/turning,
stop/hold, sit-to-stand and controlled lying-down/recovery. Add forward roll
and return to stable stance as an explicitly qualified dynamic skill. Distinguish
controlled rest from a fault-induced fall. Crouch/beak pickup, kicking and
roller-skating are later module/skill extensions with separate dynamics.

Train and evaluate each skill and its transition rules. Switching policies
requires matching observation/action semantics, normalization, state reset,
entry posture and exit/abort conditions. A change of support contacts or wheel
attachment may require a different model and controller package.

The capstone has four separate demonstrations:

- **Learn:** produce a new actor from a declared initialization using our
  collector/trainer; show full learning curves, baselines and held-out results.
- **See and navigate:** complete predeclared obstacle courses using actual
  policy-visible observations, including blocked-route and sensor-loss cases.
- **Transfer:** deploy the same qualified package to the named physical
  embodiment, with measured behavior and all interventions recorded.
- **Coordinate:** several robots walk into separated positions, stop, sit,
  stand, rest/recover and perform qualified skills on explicit cues. Report
  teleoperation, scripted sequencing and autonomous decisions separately.

Recompute deterministic simulation traces under a declared profile. Evaluate
physical repeatability statistically; do not promise bitwise physical replay.
Publish latency, falls, interventions, recovery, collisions, course completion,
resource use and cost per successful trial, including unsuccessful trials.

## 6. Hardware rights and a platform we can sell

Pollen's [model license](https://github.com/pollen-robotics/microduck_rl/blob/develop/LICENSE-MODELS.md)
places specified STL geometry under **CC BY-NC-SA 4.0**, separately from the
Apache-2.0 software and other specified files. A source-code license does not
authorize selling copies of those printed shapes. Use a properly licensed
reference study and obtain suitable rights or independently authored,
commercially usable geometry for an OpenAgents kit. Retain attribution,
provenance, manufacturing tolerances, BOM and qualification for each design.

The [NV-DLI workshop](https://github.com/NV-DLI/Microduck-Workshop) is another
educational reference. Its [notices](https://github.com/NV-DLI/Microduck-Workshop/blob/main/THIRD_PARTY_NOTICES.md)
distinguish code, policies and noncommercial geometry; its overview also
separates fixed-model demonstrations from student training and unqualified
future work. Do not convert a successful course example into a claim that
we reproduced the keynote or a physical robot.

Our shipping pipeline must build from retained, permitted inputs and run
without NVIDIA services or a mandatory NVIDIA device. A small CPU reference
and a qualified non-NVIDIA execution route are explicit milestones; scalable
training on an additional backend requires new evidence. Existing CUDA can
accelerate development, while the resulting small robot's local controller
must operate independently of that training machine.

## 7. Delivery gates and book ownership

| Gate | Required result |
| --- | --- |
| D0: evidence and interfaces | Pinned reference source/artifacts/rights; one admitted robot, complete observation/action/actuator contract and frozen task suite |
| D1: compatible execution | Independent policy-output vectors and a reference simulator comparison; clearly labeled imported policy |
| D2: owned learning | Shared neural actor/critic and GAE/PPO train on actual articulated/contact environments; fresh seeds, resume and held-out gates |
| D3: complete simulation | Our engine, rendered sensors and task planner close the observation/action loop; new skills and course success, not only prerecorded movement |
| D4: owned deployment | Native package and checked interchange, local supervision, timing and authorized physical transfer with real fault/stop evidence |
| D5: complete capstone | Reproducible agent-assisted asset-to-learning workflow, perception-guided courses, several robots and a commercially usable modular release |

D1 can use independent SDK references while engine components are built;
D3 requires the owned backend. Pretrained execution, owned learning, physical
transfer and a saleable hardware release remain separately reported outcomes.

[The coding-agent roadmap](../../psionic/docs/robotics/microduck-implementation-roadmap.md)
now breaks implementation into RB-00…RB-13 with source owners and checks.
Draft chapters 257–265 provide worked calculations and component contracts;
chapter 265 owns the final complete-route acceptance.

[Psionic chapter 171](../../psionic/book/ch171-learn-a-gait-in-many-worlds.md)
owns the reusable learner, actor/critic, collector and locomotion components.
Its scalar-LQR evidence retains its original scope; quadruped acceptance also
remains separate. New [chapter 256](../../psionic/book/ch256-build-and-teach-a-small-robot.md)
introduces the cumulative capstone and links its detailed chapters. The
[book plan](../../psionic/book/PLAN.md#xxvi-build-and-teach-a-small-robot) and
[delivery roadmap](../../psionic/docs/roadmap.md#microduck-capstone-selected-end-to-end-robotics-goal)
map stages to existing chapters, engine work and Coax requirements.
