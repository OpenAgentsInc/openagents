# 44. A self-hosted compiler

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R25.

Today `coax0` is two languages. The lexer, parser and type checker are Coax
compiled to C, and the compile-time evaluator and C emitter are C++20
([chapter 27](ch27-inside-coax0.md)). R25 moves the remaining semantic work
into Coax in slices that can each be reviewed on its own, and finishes the
supported toolchain. Native code stays the layer for effects, storage, ABIs
and platform SDKs.

## What this chapter will cover

- The order in which compiler passes move into Coax.
- The evidence each migrated pass needs.
- Trusted seeds, regeneration and fixed points, and what a fixed point does
  not prove.
- What a clean machine needs to build the real consumers.

## Outline

1. **What is still C++.** Read the source map in chapter 27 and list the
   passes that remain native: bounded evaluation, C, header, layout and
   shader emission, and the driver.
2. **The order.** Verification, coverage and mutation policy first; then
   bounded evaluation; then C, header, layout and shader emission; then
   driver, workspace and tooling policy. Each slice uses the shared typed
   graph, not a new tree or wire format.
3. **Evidence per slice.** Checks and mutation tests, rejected-input and
   budget cases, native sanitizers and independent semantic oracles. Matching
   the previous output shows compatibility. Running against C, `protoc` or
   target oracles shows correctness. The chapter keeps the two apart.
4. **Provenance for synthetic code.** Generated code carries its origin, so a
   source mutation is not mistaken for a corrupted compiler artifact.
5. **Seeds and fixed points.** Explicit trusted seeds, deterministic
   regeneration and fixed points over several stages. Where it's feasible,
   two independent bootstrap chains are compared. A fixed point alone is not
   a bootstrap-security proof.
6. **The trust boundary.** A written list of the seeds, native shims, tools
   and passes that remain trusted.
7. **Distributions.** Versioned host interfaces, reproducible toolchains,
   platform conformance and release evidence. Language core stability, target
   support and library maturity each get their own status.

## The consumer and the gate

The consumers are the real native and browser applications. The exit:

- A clean machine bootstraps and builds them with the documented tools.
- Python and `protoc` are not normal compiler dependencies.
- The library, diagnostics, ABI, mutation and conformance suites pass.
- The remaining trust boundary is written down.

## Done when

- R25's exit evidence is recorded in a release.
- Chapter 27's source map describes the migrated compiler, and this chapter
  shows a transcript of a clean bootstrap.
- The chapter's examples run under `check-examples.sh`.

## Dependencies

- R2–R8, as each pass needs them. This track can start earlier whenever
  those phases provide enough for the next pass.
- [Chapter 27](ch27-inside-coax0.md) for the current compiler and
  [chapter 23](ch23-verification-boundaries.md) for what checks establish.
