# 211. Measure the harness

> **Status: unimplemented — design draft.** Nothing here runs yet.

Every addition to the harness has to earn its place. This chapter measures the
harness itself and runs end-to-end task suites, changing one thing at a time
against [Chapter 200](ch200-the-simplest-loop.md)'s simplest loop.

## What this chapter covers

- Harness benchmarks:
  - opening a session;
  - folding a long conversation;
  - reconnecting a stream;
  - continuing an agent.
- End-to-end task suites with tokens, cost, cache hits and wall-clock time per
  task.
- One change per run, compared with the simplest loop on the same tasks,
  seeds and model identity.
- Tasks the change was not built from. A gain on the development tasks alone
  does not count.
- Reporting through the suite digests, noise floors and gates of chapters
  [239](ch239-a-suite-is-three-digests.md), [240](ch240-how-far-a-metric-moves-by-itself.md) and [241](ch241-gates-that-can-say-unverifiable.md).

## Dependencies

- [Chapter 200](ch200-the-simplest-loop.md) through [Chapter 210](ch210-speak-the-protocols.md): the additions under test.
- [Chapter 172](ch172-benchmark-a-service-not-a-kernel.md): service timing.
- [Chapter 239](ch239-a-suite-is-three-digests.md), [Chapter 240](ch240-how-far-a-metric-moves-by-itself.md) and [Chapter 241](ch241-gates-that-can-say-unverifiable.md): suites, noise floors and verdicts.

## Done means

- Each harness benchmark reports P50 and P95 on a named host.
- Each addition has a verdict of passed, failed or unverifiable against the
  simplest loop, beyond the measured noise floor.
- An addition with no measured gain is removed, and the chapter records that.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
