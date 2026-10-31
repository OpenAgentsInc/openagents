# 185. A decision model steers voice

> **Status: unimplemented — design draft.** Nothing here runs yet.

The voice engine already accepts decisions through a gate that is never awaited. This chapter makes Elephant a `decisions:<model>` decider behind that gate.

A slow or absent model can only fail to steer. It never stalls an audio frame.

## What this chapter covers

- **The existing gate.** Proposals pass clamping, a confidence floor, epoch and deadline fences, and recorded drop reasons. This chapter uses the gate and does not change it.
- **Elephant as the decider.** The voice engine sends a typed decision request through `/v1/systemone`. Elephant's answer becomes a proposal.
- **Never awaited.** The audio path never waits for a decision. A late answer is dropped with a recorded reason.
- **Published semantics.** The decision's meaning comes from [chapter 16](ch016-are-the-probabilities-any-good.md)'s published semantics, and its threshold comes from the consumer, as Part XXII requires.
- **Consent and floor rules.** The voice engine's consent and floor rules stay unchanged.

## Dependencies

- [Chapter 16](ch016-are-the-probabilities-any-good.md): published decision semantics.
- [Chapter 20](ch020-serve-systemone.md): the decision API.
- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): latency measurement.
- [Chapter 184](ch184-a-pipeline-of-models.md): the voice pipeline.
- [Chapter 216](ch216-thresholds-from-written-costs.md): thresholds from written costs.
- The plan also names [chapter 163](ch163-execute-nemotron-nano.md)'s measured latency as an input to this chapter.

## Done means

- With Elephant stopped, the voice engine produces the same frames on time, and every dropped proposal has a recorded reason.
- With Elephant answering late, no frame misses its deadline.
- A proposal outside its clamp, below the confidence floor or from an old epoch is dropped, and each case has a test.
- Measured decision latency and the share of proposals that arrive in time are reported.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
