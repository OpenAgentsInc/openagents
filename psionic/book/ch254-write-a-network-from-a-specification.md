# 254. Write a network from a specification

> **Status: unimplemented — design draft.**

[Chapter 253](ch253-read-and-repair-a-models-weights.md) edits existing
weights. This chapter will ask an LLM to write a complete network from a task
specification, then check its computation against independently implemented
references. The proposed artifact and patch boundary is chapter 253's; no
construction result is claimed here.

## Begin with a calculation we can check

Start with bounded integer maximum, absolute value and a conditional. Work
the arithmetic by hand, and compare generated weights against ordinary Coax
functions and [chapter 229](ch229-compile-a-gate-graph-into-a-block.md)'s
compiler. Use a fixed network architecture and admit its entire finite
artifact before evaluating it. Check exhaustive small domains and larger
held-out values, including signs, zero and boundary ties.

Keep two conditions separate: the proposer emits weight values directly,
or the proposer emits a gate graph that the existing compiler translates.
The latter measures program synthesis plus a compiler. Record network
parameters, bytes, actual inference cost and the native function's cost.
Exactness applies only to the bounded profile and checked domain.

## Then write a small control policy

Use [chapter 110](ch110-a-grid-world-that-learns-plans-and-recovers.md)'s
headless world. Specify observations sufficient for the requested task;
do not ask a memoryless network to solve indistinguishable states requiring
different actions. Compare a constructed policy with the exact planner,
tabular learning and a separately implemented trained neural policy. State
the neural baseline's missing components before claiming a PPO comparison:
chapter 171 currently provides scalar-LQR PPO, not a general game actor.

Freeze map families, evaluation seeds, unseen starts, horizon, action
sampling and collision/success metrics before proposals. Keep fixed-map
memorization, held-out-map transfer and recovery after a map change as
different results. Include a native hand-coded controller. A network which
merely implements that controller has a clear, limited achievement.

## Experiments and acceptance

Run direct text weights, numerical tools without evaluation, and budgeted
evaluation conditions. At each budget record all failed artifacts, model
input/output tokens, sequences, environment steps, CPU/GPU time and total
search cost. A nominal rollout batch is not one cheap forward pass.
Use multiple task families and proposer attempts; pair the same evaluation
seeds across methods. Lock held-out cases until the candidate is saved.

An independent evaluator owns the specification and cannot be rewritten by
the proposer. Tests must reject malformed shapes, nonfinite weights,
out-of-range values, zero work and planted almost-correct policies. Retain
every proposal and exact artifact. Report no gain when the simple baseline
wins; do not extend a successful arithmetic result to game competence.

Graduation requires the book's worked trace, runnable public Coax example,
deliberately broken case with an exact diagnostic, independent oracle,
debug/release transcript and a result over the declared held-out tasks.
Update the registry and all status tables together only when those exist.
