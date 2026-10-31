# 184. A pipeline of models

> **Status: unimplemented — design draft.** Nothing here runs yet.

A voice answer passes through several models: voice activity detection, transcription, a decision model and speech synthesis. This chapter serves that compound workload with each stage scaled separately in one cluster.

The goal is an end-to-end latency budget that you can split into a budget per stage and per network hop.

## What this chapter covers

- **Four stages.** Voice activity, transcription ([chapter 153](ch153-transcribe-with-parakeet.md) and [chapter 154](ch154-transcribe-while-someone-speaks.md)), a decision model (Elephant) and speech ([chapter 155](ch155-speak-the-answer.md)).
- **Separate scaling.** Each stage has its own replicas and autoscaler settings, as section 7.2 of *Inference Engineering* describes for compound pipelines.
- **A latency budget per stage.** The end-to-end budget is written down and split across stages before measurement.
- **A cost per hop.** Each network hop between stages has a measured latency and byte count.
- **Streaming between stages.** Partial transcripts flow to the decision stage before the utterance ends, where the stage contract allows it.

## What it builds

- A planned pipeline description that names each stage's model, profile and budget.

## Dependencies

- [Chapter 154](ch154-transcribe-while-someone-speaks.md): streaming transcription with measured per-chunk latency.
- [Chapter 155](ch155-speak-the-answer.md): streamed speech output.
- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): the service benchmark.
- [Chapter 177](ch177-route-queue-and-scale-replicas.md): per-stage scaling.
- [Chapter 16](ch016-are-the-probabilities-any-good.md): published decision semantics for the decision stage.

## Done means

- [chapter 172](ch172-benchmark-a-service-not-a-kernel.md) reports end-to-end latency at P50, P95 and P99 and each stage's share, against the written budget.
- Each hop's latency and bytes are measured.
- Scaling one stage changes that stage's latency and leaves the others within their budgets.
- A stage that misses its budget is named in the report.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
