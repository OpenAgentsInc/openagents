# 179. Ship without downtime

> **Status: unimplemented — design draft.** Nothing here runs yet.

A new build must reach users without dropping requests and without changing answers by accident. This chapter adds canary rollouts with gradual traffic ramps and an automatic revert.

Latency objectives alone do not catch a wrong answer. Each canary also runs golden requests whose outputs must match retained evidence.

## What this chapter covers

- **Canary rollout.** A new build takes a small share of traffic, then larger shares at stated steps. Section 7.4 of *Inference Engineering* prefers canaries to blue-green swaps at GPU scale.
- **Two kinds of gate.** Latency and error objectives guard performance. Golden requests guard correctness: their outputs must match retained execution evidence for the same model and profile.
- **Automatic revert.** A failed gate at any step moves all traffic back to the previous build.
- **Draining.** A replica that is about to stop takes no new requests and finishes those in flight before it exits.
- **Scope.** This chapter rolls out new builds of one model. [chapter 224](ch224-shadow-a-new-model-then-promote-it.md) rolls out a different model.

## What it builds

- Planned rollout and revert policies in an `#authority` Coax file.

## Dependencies

- [Chapter 173](ch173-serve-a-model-with-an-honest-capability.md): the manifest that names each build's identity and profile.
- [Chapter 177](ch177-route-queue-and-scale-replicas.md): the router that splits traffic by share.
- [Chapter 22](ch022-run-it-live.md): golden warm-up and versioned health.
- [Chapter 218](ch218-a-pylon-checks-itself-before-serving.md): the self-check against shipped reference decisions.
- [Chapter 224](ch224-shadow-a-new-model-then-promote-it.md): promotion of a different model, which this chapter does not cover.

## Done means

- A deliberately wrong build fails its golden requests and is reverted automatically, with no change to the active build's answers.
- A deliberately slow build fails its latency objective and is reverted.
- A rollout under [chapter 172](ch172-benchmark-a-service-not-a-kernel.md)'s load generator drops zero requests while replicas drain.
- The rollout policy passes its authored checks and mutation testing.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
