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
check, plus the complete capstone gates. Record simulation, physical and
manufacturing milestones separately as they land.

## Dependencies and ownership

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
