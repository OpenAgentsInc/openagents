# 189. Place the model

> **Status: unimplemented — design draft.** Nothing here runs yet.

A mesh must decide where a model runs. The rule is: run whole on one machine if it fits, else route the whole request to a peer that holds it, else split.

The placement planner is a pure Coax procedure, checked in simulation before it places anything.

## What this chapter covers

- **Three choices in order.** Whole on this machine, whole on a peer, then a split. Mesh-LLM uses the same order.
- **Inputs to a split.** Per-layer memory from [chapter 33](ch033-measure-the-work-and-plan-the-memory.md), measured stage costs and measured link costs from [chapter 188](ch188-peers-transport-and-discovery.md).
- **A stated objective.** For one request's latency, the cost is the sum of stage times. For throughput, the cost is the slowest stage. Link transfers are charged in both.
- **No hidden savings.** Splitting avoids per-layer collectives. It does not remove stage traffic.

## What it builds

- A planned placement planner in an `#authority` Coax file with a simulation harness.

## Dependencies

- [Chapter 33](ch033-measure-the-work-and-plan-the-memory.md): per-layer memory and measured compute.
- [Chapter 186](ch186-split-a-model-into-stages.md): the boundary contract and its parity gate.
- [Chapter 187](ch187-stage-packages.md): stage package sizes.
- [Chapter 188](ch188-peers-transport-and-discovery.md): measured link costs.

## Done means

- In simulation, the planner chooses whole placement whenever the model fits, and a split only when it must.
- For each split it chooses, the stated objective is computed and reported.
- On real machines, the measured latency or throughput of the chosen placement is within a stated tolerance of the planner's estimate.
- The planner passes its authored checks and mutation testing.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
