# 41. Compile and startup budgets

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built. This is roadmap R22 (spec stage 11).

How long you wait to see a result is a property of the language and its
tools. R22 splits that wait into five measured cases, gives each one a
budget and keys a persistent kernel cache by content identity. A stale cache
entry is never reused, and a valid one is never rebuilt.
[Chapter 19](ch19-builds-hot-reload.md) introduces the plan; this chapter
covers it in full.

## What this chapter will cover

- The five iteration cases and their budgets.
- What goes into a kernel's cache key.
- Why a parameter edit must not recompile, and why a structural edit must.
- How the front end skips unchanged kernels.

## Outline

1. **Five cases.** Each one is measured on its own:
   1. Compiling a kernel for the first time.
   2. Loading a kernel that is already compiled.
   3. Rebuilding a scene from cached assets.
   4. Changing a runtime parameter. This must not recompile.
   5. Changing a structural property. This must recompile, and the report
      says why.
2. **The cache key.** Source, specialization values, layout
   ([chapter 40](ch40-logical-tensors.md)), target, compiler build and
   options, all as R1 content identities.
3. **Refusing stale entries.** A planted entry with a changed layout,
   compiler or option is refused, not loaded.
4. **Skipping the front end.** For an unchanged kernel, parsing and checking
   are skipped too, not only code generation.
5. **Reading the report.** One table per target: case, budget, measured time
   and the reason for every recompilation.
6. **Budgets per target and consumer.** The expansion: different budgets for
   the Mac, the browser and Psionic's CUDA hosts.

## The consumer and the gate

The consumers are the Aethyr physics engine and one Psionic model. The gate:

- The five cases are reported for both, on each qualified target.
- A planted stale cache entry is refused.
- A parameter edit that recompiles fails its budget.

## Done when

- R22 has shipped with the gate above.
- This chapter shows the five-case report as an exact transcript.
- The chapter's examples run under `check-examples.sh`.
- Chapter 19's planned section links here and states what shipped.

## Dependencies

- R1 content identities, R8 verified local caching and R9 target
  descriptions.
- [Chapter 12](ch12-debugging-and-cost.md) for how Coax measures cost, and
  [chapter 19](ch19-builds-hot-reload.md) for the whole-loop measurement.
- [Chapter 40](ch40-logical-tensors.md) for specialization values.
