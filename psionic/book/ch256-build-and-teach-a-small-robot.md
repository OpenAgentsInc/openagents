# 256. Build and teach a small robot

> **Status: unimplemented — design draft.** This capstone plans the complete
> small-biped pipeline: agent-assisted assets and robot admission, simulation,
> new policy training, perception, evaluation, export, authorized physical
> transfer and several coordinated robots. No new model, training run,
> simulator, embedded target or hardware is qualified here.

The reference is Pollen Robotics/Hugging Face's Microduck in Jensen Huang's
October 21, 2026 GTC Berlin keynote. The
[specification and primary sources](../../docs/nvidia/microduck-pipeline-2026-11-15.md)
identify the film around 1:20:44, the onstage demonstration, NVIDIA's workflow
and the separate public training/runtime implementations. Their configurations
and evidence are not interchangeable.

We want to reproduce the complete capability in Coax and Psionic. A development
agent plans a course, creates editable assets and requests bounded experiments.
The engine simulates a robot that learns from its own attempts. Psionic trains
the controller, evaluates and exports it. A local supervisor runs the qualified
package on the physical robot. Finally, several robots execute a recorded
sequence of cues and perception-based tasks, with operator intervention visible.

## The chapters we will build

This chapter is the overview and experiment ledger. The worked draft sequence
is [257: contract](ch257-a-robot-description-becomes-an-experiment.md),
[258: servo](ch258-a-servo-that-behaves-like-the-motor.md),
[259: collector](ch259-rollouts-from-independent-worlds.md),
[260: neural learner](ch260-train-a-neural-walking-policy.md),
[261: skills](ch261-teach-skills-and-switch-between-them.md),
[262: perception](ch262-walk-by-what-the-robot-sees.md),
[263: package/runtime](ch263-export-a-policy-and-supervise-its-steps.md),
[264: physical commissioning](ch264-run-the-policy-on-a-real-robot.md), and
[265: the final route](ch265-a-course-a-flock-and-a-repeatable-result.md).
Chapter 257 is now partly implemented for decoded CPU admission and action
mapping. The other chapters remain design drafts until their own checks and
runnable evidence exist.

The [coding-agent roadmap](../docs/robotics/microduck-implementation-roadmap.md)
defines source owners, concrete work packages, dependencies and what an agent
can implement now. Start with the decoded contract, actuator/collector math
and new neural learning on an existing analytic plant. Connect that same
learner to the owned biped when engine articulation/contact is qualified.

## Count work before claiming progress

Four environments collected for eight action steps produce 32 transition
rows. Four PPO epochs over minibatches of eight produce 4 × (32/8) = 16
optimizer transactions for one rollout/update cycle. A budget of 256 such
cycles therefore means 8,192 collected rows and 4,096 optimizer transactions,
not 256 of both. Physics substeps are another counter.

Keep those counters, scene/actor/normalizer identities and consumed compute
in the experiment ledger. The final route reports which gates were actually
run; an imported demonstration or an optimizer counter alone cannot show
new learned physical behavior. These numbers specify proposed work rather
than observed execution.

## What already exists

Chapter 171 supplies checked return targets, PPO/Gaussian primitives and a
scalar-LQR training experiment. It does not supply a legged environment or
general neural actor/critic. Chapter 170's GR00T/Panda loop and owned queries
have different embodiment, simulator and timing boundaries. The small CPU
framework, selected device training and bounded interchange in earlier chapters
are reusable foundations, rather than automatic support for a biped actor.

This chapter composes those components after their own acceptance gates.
It adds no second trainer, simulator, UI toolkit, artifact codec or agent harness.

## The planned route

1. **Freeze the experiment.** Record task, robot/assembly, source rights,
   observation/action/actuator contracts, training budget, baselines, held-out
   terrain/course/parameter groups, seeds and promotion thresholds.
2. **Build the world.** Coder prepares dimensioned course assets through
   constrained tools. Admit robot links/joints/inertias, calibrated sensors,
   full contact geometry and a measured or source-bound actuator model.
3. **Check compatibility.** Run small independent model/actuator vectors and,
   where useful, an imported upstream policy in a separately labeled profile.
   Preserve exact normalization and joint maps through interchange.
4. **Collect physical rollouts.** Batch real articulated/contact simulations,
   with reset isolation and per-environment randomness. Distinguish terminal,
   truncated and collector-tail observations. Keep privileged critic data
   out of the actor's deployment interface.
5. **Train new skills.** Use chapter 171's general neural actor/critic and
   GAE/PPO through the shared trainer/optimizer. Learn balance, walking,
   sit/stand, controlled rest and recovery; add a forward roll under its
   own contact/transition qualification. Retain failed seeds and all curves.
6. **Add perception.** Render calibrated camera/depth observations, train or
   qualify an encoder/student and connect obstacle-aware task commands to the
   locomotion actor. Test on courses absent from training and agent tuning.
7. **Evaluate and export.** Compare against conventional and imported-policy
   baselines; save the complete resumable experiment, model and normalizer.
   Independently check native and supported ONNX outputs. Unsupported graph
   operations refuse until the shared importer/executor supports them.
8. **Transfer.** Commission the separately authorized hardware integration in
   stages. Measure timing, sensor age, limits, voltage/thermal behavior,
   calibration, watchdogs and stop/fall outcomes. The local control path
   remains independent of remote training, models and the UI.
9. **Coordinate and improve.** Execute explicit cues for several robots,
   keeping authority and controller state separate. Record human/script/model
   origins, interventions and faults. Admit new training rows through the
   existing exposure ledger and promote changes under the frozen evaluator.

The hardware specification covers commercially usable printable modules,
repair and manufacturing evidence. Pollen's noncommercial model geometry is
a separately licensed reference; it cannot become a saleable OpenAgents kit
merely because its software is open source.

## Acceptance checks and independent evidence

- Imported joint/frame/inertia and actuator values match independent sources;
  malformed models, unsupported operations and inconsistent calibration refuse.
- A batched robot equals its solo deterministic run, with isolated resets,
  RNG and recurrent/controller/normalizer state.
- Actor/critic gradients and updates match independent small references;
  seeded neural training produces complete curves and exact resume within
  the declared numerical/target profile. A scalar-LQR result cannot satisfy this.
- Policies trained from declared initializations meet prospective task gates
  across prescribed seeds, held-out surfaces and randomization. Every miss
  is reported. Teacher/student and privileged/onboard observations are explicit.
- Walking tracks commands; sit/stand and rest/recovery meet posture/contact
  criteria; the roll returns to stable stance; policy switching respects
  entry/exit conditions and stale-command refusal.
- Perception-based courses use only declared actor/planner observations.
  Blocked routes, occlusion and sensor loss receive their specified response.
- Model packaging preserves normalization/action meaning, matches independent
  output vectors and rejects model/assembly/firmware mismatches.
- A named physical robot passes the separately authorized timing, fault,
  stop and repeatability profile. No physical bitwise replay is claimed.
- Several robots preserve local authority under delayed, reordered and lost
  cues; reports distinguish imported playback, our training, teleoperation,
  scripted choreography and autonomous decisions.
- A clean-machine route runs the permitted small reference without NVIDIA
  services; at least one complete non-NVIDIA execution profile is qualified.
  Full training on another backend requires its own measurement.

Promotion to an implemented teaching chapter still needs a worked trace,
runnable source/example, a deliberately refused case and an independent
check for its stated profile. The final complete-route gates belong to
chapter 265. Record simulation, physical and manufacturing milestones
separately as they land; an implemented overview cannot promote them.

## Dependencies and ownership

The overview's minimum reading is chapter 17's qualification discipline,
171's learning boundary, 239/241's suite/verdict rules and Coder's typed tools.
The component chapters below introduce their own prerequisites. The full-route
dependencies that follow are integration gates, not reasons to delay the
decoded contract or analytic-plant learner.

Psionic: graph/device/training chapters 37–40, 103–107, 117–124 and 129;
vision/observations 142 and 165; control and locomotion 167–171; model
interchange 8, 220–221; evaluation/exposure 239–242 and 247–248.
The first biped does not depend on completing Cosmos or a large VLA.

Metaverse Engine: scene/rigid/articulated/contact chapters 17–22, batches 31,
sensors/observations 33–34, import 35, replay 36 and measurement 37.
Coax: relevant R12–R20 physical-world contracts and measured consumers of
R21–R24 simulation compute. Coder owns the development agent and tools;
the proposed robotics context owns device admission and local authority.
All dependencies keep the existing downward layer rules.
