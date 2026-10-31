# 183. Split a model across devices

> **Status: unimplemented — design draft.** Nothing here runs yet.

Some models are too large or too slow on one device. This chapter splits one model across devices that share a fast interconnect: tensor parallelism for latency and expert parallelism for MoE throughput.

Part XX handles the loosely connected case across machines. This chapter is the tightly coupled case in one machine or rack.

## What this chapter covers

- **Collectives.** All-reduce, all-gather and all-to-all over the devices in one domain, shared with [chapter 132](ch132-train-across-devices.md)'s training collectives.
- **Tensor parallelism.** Attention and feed-forward weights are sharded by rows or columns, and the results are combined by collectives.
- **Expert parallelism.** For gpt-oss, DeepSeek and other MoE models, experts live on different devices, and routed tokens move to them and back.
- **Expert parallelism stays inside one fast domain.** Attention runs data-parallel. Redundant experts are rebalanced from a load window.
- **Balance by time.** Balance is reported as busiest over mean, measured by time rather than token count. DeepSeek reports 1.03–1.09 for its own layout; that is an upstream figure.
- **Reference comparison.** Results are compared with the single-device reference under [chapter 17](ch017-qualify-honestly.md)'s rules, because the reduction order changes.

## What it builds

- Planned sharded-weight layouts and a collective layer shared with distributed training.

## Dependencies

- [Chapter 132](ch132-train-across-devices.md): gradient collectives on one homogeneous topology.
- [Chapter 133](ch133-a-tensor-spread-over-devices.md): device meshes and sharded layouts.
- [Chapter 61](ch061-a-few-experts-then-a-router.md): the expert router.
- [Chapter 17](ch017-qualify-honestly.md): distribution checks for results whose arithmetic changes.
- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): the measurement that shows the need.
- Two or more devices in one fast domain, which is an owner hardware decision.

## Done means

- Tensor-parallel and expert-parallel runs pass [chapter 17](ch017-qualify-honestly.md)'s frozen distribution checks against the single-device reference on the same tokens.
- Greedy output matches the single-device run for the fixture prompts, or each difference is quoted with its near-tie margin.
- Latency for tensor parallelism and throughput for expert parallelism are measured against one device.
- Expert balance is reported as busiest over mean by time.

## Sources

- [Chapter plan](PLAN.md)
- [Inference Engineering notes](../docs/serving/inference-engineering.md)
- [DeepSeek target](../docs/targets/deepseek.md)
