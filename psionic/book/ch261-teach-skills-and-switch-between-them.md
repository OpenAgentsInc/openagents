# 261. Teach skills and switch between them

> **Status: unimplemented — design draft.** Skill training, transition
> admission and the traces below are proposed. Existing animation graphs are
> references for organization, not physical robot-control qualification.

A walking actor is not automatically a sitting actor. Nor can we load a
recovery policy at an arbitrary instant and assume its first target is
appropriate. Each skill has a valid entry set, a completion condition and a
response to failure. Switching is a control decision that needs its own
evidence.

Begin with balance/walk, stop/hold, sit-to-stand and controlled rest/recovery.
A forward roll follows only with a model that includes the body's contacts
and a declared impact/clearance envelope. Wheels, beak manipulation and
different foot attachments are separate embodiment/profile changes.

## A cue is a request, not the transition itself

Consider a proposed application schedule with 20 ms action updates. A sit
request arrives with sequence 12, expiry 1,000 ms and the current authority
epoch. The latest sensor state is from 940 ms. At application time 960 ms its
age is 20 ms; under a 40 ms maximum it is fresh. Admission can still refuse
because the current posture is outside the sit skill's entry envelope.

If we retry at 1,020 ms, the original cue is expired. Receiving it again over
another transport cannot refresh it. The robot should report the reason and
continue its declared local behavior. These times illustrate a contract;
they are not a measured deadline or upstream Microduck timing requirement.

Represent requested skill, active skill and completed transition separately.
Record the observation generation used to admit the switch. A pending cue
must not overwrite the active actor's controller state.

## Make entry and exit measurable

Each skill package names its observation/action/normalization contract,
controller identity, entry-state envelope, permitted transition sources,
completion predicate, maximum duration and abort behavior. A recurrent skill
must declare how hidden state is initialized or transferred. Resetting a
history buffer is different from inheriting another actor's history.

Completion can require a posture band over a contiguous window rather than
one passing frame. For example, a draft stand completion might require every
sample in a chosen final window to satisfy orientation, foot contact and
rate bounds. The window length and thresholds are frozen before evaluation;
this chapter does not supply manufacturer-safe values.

Do not blend two actors' outputs merely to make the transition look smooth.
If blending is selected, define its duration, limits and contact assumptions,
train/evaluate the resulting closed loop and retain both proposals plus the
final supervised action. A smooth animation is not evidence of stable contact.

## Train the skill, then train or verify its handoff

Skills can be separate actors sharing a compatible interface, or one
command-conditioned actor. Choose one strategy for the first experiment and
state its consequences. Start with separately trained small actors and an
explicit deterministic selector so the action source is inspectable.

For sit/stand, include the relevant initial states, both directions and
terminal stability. For recovery, hold out initial orientations and physical
parameters. For a roll, measure the body-contact sequence, clearance and
return to a stable stance. Keep intended controlled rest distinct from an
unexpected fall; a fault must not count as successful execution of a nap cue.

Stage reward changes as new experiments. Reuse the frozen evaluator and
exposure ledger; do not reward-hack it into accepting a failed skill. A
policy reaching a pose once and one staying there for a full window have
different task outcomes.

## Checks and shared owners

The selector is `#authority`: stale epoch, duplicate sequence, expiry,
contract mismatch, wrong entry state, failed effect receipt and invalid
completion data must each have a refusal check and a meaningful mutation
witness. Refusals preserve the admitted active skill. A failed downstream
physical effect can halt the run; do not promise rollback of motion.

Build table-driven traces with an independent state-machine interpreter.
Then run scripted cue sequences against the actual simulator and report
transition success separately from within-skill success. Removing an entry
check must make a scenario fail. Keep exact software traces separate from
statistical physical repeatability.

Reuse the existing [Panda supervisor concepts](ch170-close-the-simulated-control-loop.md)
through a deliberate generalized interface, preserving its old profile.
The engine's [animation graph](../../metaverse-engine/character/graph.coax)
can display skill state but must not decide motor authority. Shared UI and
agent capabilities display/propose cues above the local supervisor.

[RB-08](../docs/robotics/microduck-implementation-roadmap.md#rb-08-skill-training-and-transition-policy)
names the implementation split. Perception in
[chapter 262](ch262-walk-by-what-the-robot-sees.md) will supply task commands
through that same admission boundary.
