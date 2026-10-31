# 47. Backends chosen by profiling

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R28.

Coax compiles to plain C, and the C backend stays the working reference. R28
is conditional: a new backend is added only when a measurement on a real
consumer shows it is needed. A fast native debug backend comes only if C
compilation and linking dominate the measured loop after caching and
generated code size are addressed. An optimizing backend comes only if
runtime or code-size results justify its dependencies and maintenance.

## What this chapter will cover

- The measurements that decide whether a backend is worth adding.
- What every backend must share with the C backend.
- The suites a new backend must pass.
- Why the number of backends is not a goal.

## Outline

1. **Measure the loop first.** Use the whole-loop measurement from
   [chapter 19](ch19-builds-hot-reload.md) and the budgets from
   [chapter 41](ch41-compile-budgets.md). Attribute time to C compilation,
   linking and the rest.
2. **A fast debug backend.** The case for it, and the measurement that must
   show C compilation and linking dominate after caching.
3. **An optimizing backend.** The runtime and code-size results that would
   justify it, decided independently of the debug backend.
4. **Shared semantics.** Every backend consumes the same checked semantics,
   target descriptions, effects and source maps.
5. **The same suites.** Language, ABI, uninitialized and overflow,
   verification, debugging and platform suites run on every backend.
6. **Comparing backends.** Whole-loop latency, output behavior, runtime, code
   size and peak build memory on named workloads.
7. **An evaluator.** An internal bytecode evaluator is also conditional, on
   measured evaluation cost. It is not a runtime VM or a way around the
   semantics.

## The consumer and the gate

The consumer is whichever real application shows the measured bottleneck.
The exit is a measured gain on that consumer without losing correctness or
tooling. Without that gain, Coax keeps C output only.

## Done when

- Either a backend ships with its measured gain and passes every shared
  suite, or the chapter records the measurement that kept C as the only
  backend.
- The chapter's comparisons appear as recorded results with their workloads
  and toolchain identities.
- Any examples run under `check-examples.sh` on every backend they claim.

## Dependencies

- Earlier adoption and target evidence (R0, R10) and a measured backend
  bottleneck.
- [Chapter 27](ch27-inside-coax0.md) for the C emitter,
  [chapter 12](ch12-debugging-and-cost.md) for measuring cost and
  [chapter 41](ch41-compile-budgets.md) for iteration budgets.
