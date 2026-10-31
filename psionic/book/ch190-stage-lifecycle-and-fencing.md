# 190. Stage lifecycle and fencing

> **Status: unimplemented — design draft.** Nothing here runs yet.

A split model is only as available as its least ready stage. This chapter defines how stages start, how stale commands are fenced out and what happens when one stage fails.

The old DeepSeek stage work passed kill, freeze, fencing and machine-loss tests. This chapter rebuilds those tests in Coax.

## What this chapter covers

- **Downstream first.** Stages load from the last to the first. The entry route is published only when every stage is ready.
- **Topology generations.** Each topology has a generation number. A command from an older generation is refused, so a restarted stage cannot serve a stale request.
- **Partial failure.** A failed stage withdraws the route, and the other stages release their memory.
- **Declared sequence behavior.** Each live sequence declares whether it drains, fails or restores when its topology changes.

## What it builds

- Planned stage lifecycle states and a fencing check in Coax.

## Dependencies

- [Chapter 186](ch186-split-a-model-into-stages.md): the boundary contract.
- [Chapter 187](ch187-stage-packages.md): stage packages and readiness.
- [Chapter 188](ch188-peers-transport-and-discovery.md): peers and the control plane.
- [Chapter 189](ch189-place-the-model.md): placement.

## Done means

- The entry route never appears before every stage reports ready, checked across many start orders.
- A command with an old generation is refused in every lifecycle state.
- Killing, freezing or disconnecting any one stage withdraws the route and releases memory on the others within a stated time.
- A sequence declared to restore produces the same output as an uninterrupted run.

## Sources

- [Chapter plan](PLAN.md)
- [Inference across a mesh](../docs/serving/mesh.md)
- [DeepSeek target](../docs/targets/deepseek.md)
