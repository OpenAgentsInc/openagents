# 99. A decode step with no host waits

> **Status: unimplemented — design draft.** Nothing here runs yet.

A DeepSeek V4.1 decode step launches many small device operations: routing,
expert products, attention over compressed state and sampling. If the host
waits for a result between them, the step time is set by host round trips,
not by the device. This chapter records the whole decode step as one device
command sequence and keeps the host one step ahead.

## What this chapter covers

- One recorded device command sequence per decode step, built on
  [chapter 70](ch070-replay-the-device-program.md)'s replayable device program.
- Expert ids that stay on the device: the router writes the selected ids into
  a device buffer, and the expert products read them there. The host never
  downloads routes during a step.
- Sampling on the device, so a stage returns a token id rather than a full
  logit vector.
- A host that schedules step N+1 while step N runs. This follows the old
  CUDA-graph stages and DeepSeek's asynchronous scheduler (after SGLang and
  NanoFlow), reimplemented in Coax.
- Buffer lifetimes that allow speculative lengths to be learned one step
  late: a step may start before the previous step's accepted length is known,
  and the buffers it reads stay valid until that length arrives.
- Cancellation and failure at step boundaries: a failed or cancelled step
  leaves the cache and position counters as they were before the step.

## What it builds

- A planned decode-step program for the V4.1 engine that combines the expert
  cache of chapter 96, the packed cache of chapter 98 and device sampling.

## Dependencies

- [Chapter 70](ch070-replay-the-device-program.md): replayable device
  programs with bounded dynamic token and position control.
- [Chapter 96](ch096-hot-experts-in-fast-memory.md): the expert cache whose
  residency the step reads.
- [Chapter 98](ch098-pack-the-cache-exactly.md): the packed KV layout.
- [Chapter 53](ch053-choosing-the-next-token.md): the sampling contract that
  the device sampler must reproduce.
- [Chapter 102](ch102-verify-drafts-by-expected-gain.md) uses the late
  speculative-length path.
- Coax language jobs and asynchronous effects remain planned; until then the
  host scheduler is library code with explicit lifetime gates.

## Done means

- The recorded step produces the same tokens, cache contents and stop
  behavior as the unrecorded step under the same named profile.
- The device sampler matches the CPU sampler of chapter 53 for the same seed
  and counter on fixed logits.
- A trace of one step shows no host wait between the first launch and the
  token read-back.
- Host enqueue time and step time are measured before and after, with the
  same workload, host and build identity.
- A cancelled or failed step leaves the previous state intact, checked by
  continuing from it.

## Sources

- [Chapter plan](PLAN.md)
- [DeepSeek target notes](../docs/targets/deepseek.md)
