# 265. A course, a flock and a repeatable result

> **Status: unimplemented — design draft.** This final route composes the
> drafted small-robot chapters. No completed course, trained biped, coordinated
> hardware demonstration or saleable release is claimed.

We began with a film of small robots learning and performing together. Our
result must connect the development artifacts to a trained controller and
then to observed physical behavior. Four robots moving attractively onstage
would leave unanswered which policy was trained, what each robot observed
and who selected its next skill.

This chapter makes the chain inspectable: a task brief produces an admitted
course and experiment; our learner produces a new actor; a frozen evaluator
judges it; a package binds it to qualified assemblies; local supervisors
execute explicit tasks and cues. The workbench can show the whole chain.

## First a course, then a coordinated demonstration

The simulation route uses four independent robot environments before a
shared scene. Prove reset/RNG/controller isolation there. In a shared scene,
add actual inter-robot collision/separation tests; four independent scenes
rendered beside one another are not a shared multi-robot world.

The final physical showcase targets four separately identified units, after
one-unit commissioning and smaller coordinated trials have passed. This
number is a proposed demonstration target matching the reference's scale,
not a purchase instruction or a performance claim.

Each robot walks to an assigned separated region, stops, sits, stands,
enters controlled rest, recovers and performs any enabled dynamic skill.
Declare the required area and timing windows from measured robot envelopes.
A coordinator sends bounded requests; each local supervisor decides whether
its current state permits execution. One rejected cue does not revoke every
other robot's authority or force them into the same physical response.

## A two-robot trace inside the four-robot run

Robot A accepts cue sequence 7 under epoch 3. Robot B has already advanced
to epoch 4 after a reconnect and refuses a delayed epoch-3 cue. The report
must show that B refused; the coordinator cannot relabel the run successful
because A performed the action. Retrying B requires a new admissible request,
not replaying the expired bytes.

Keep requested, accepted, applied and completed states distinct. The trace
also names the source of the cue: a local person, recorded script, task
planner or learned router. A language instruction can request a skill, but
the evidence does not claim autonomous speech interpretation when an operator
or script actually selected it.

## The development agent is part of the route

Use Coder's existing tools to inspect the robot contract, create course
assets, propose experiment settings, submit bounded jobs and summarize the
result. Preserve the instructions, asset revisions, job manifests and costs.
The evaluator and held-out task inventory remain outside its writable scope.

An agent-produced asset must be admitted just like a human-authored one.
Reject impossible clearance, unknown materials, missing collision geometry
and units inconsistent with the course. The first route can use simple
parametric props; visual concept generation and elaborate materials are
optional presentation steps, not prerequisites for physical task truth.

Remote training workers use existing job/identity/budget components through
explicit extensions. Local controller deadlines never wait for a Pylon or a
model service. Stop, inspect and resume a development job without granting it
authority over powered hardware.

## Report the experiment people actually ran

A demonstration with three completed runs out of four is 3/4 for those four
trials. It is not a general reliability claim. Predeclare a separate evaluation
suite with enough task/seed/unit groups for the desired inference, and reuse
the book's noise-floor and three-way verdict components. Report interventions,
falls, collisions, cue refusals, recovery, latency, power/resource use and cost
per completed task. Keep the showcase separate from the held-out suite.

Preserve source, compiler, assets, assembly/calibration, weights, normalizers,
task/evaluator identities and the complete failure ledger. Provide an offline
reader and a clean-machine simulation route using permitted artifacts. Mark
which compute backend and embedded target actually passed.

For kit publication, include editable commercially usable design source,
BOM, assembly/process instructions, calibration, QA, repairs and the support
matrix. Software redistribution rights do not resolve a model mesh's separate
noncommercial terms. A saleable release has additional production evidence
beyond a working prototype.

## Completion gates

The route is complete only when it shows new neural training, the owned
simulation/perception path, checked native/interchange execution, qualified
physical transfer and coordinated behavior with clear cue origins. Imported
policy playback and independent SDK comparisons remain useful labeled stages.
They cannot stand in for the missing owned implementation.

[RB-12 and RB-13](../docs/robotics/microduck-implementation-roadmap.md#rb-12-course-and-four-robot-coordination)
define integration and reproducible release. [Chapter 256](ch256-build-and-teach-a-small-robot.md)
remains the overview; this chapter is the final end-to-end acceptance route.
