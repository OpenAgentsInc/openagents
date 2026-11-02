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

## What you can measure today

The budgets aren't built, but you can already see where a compile spends its
time. Add `--timings-table` to any `check`, `c`, `build`, `test` or `run`
command, and the driver prints each phase's wall time on stderr when it
finishes:

```shell
$ coax/bin/coax check apps/openagents-web/newland.coax --timings-table
coax timings: 1.494 s wall
  phase                         seconds   share
  load                            0.070    4.7%
  lex                             0.047    3.1%
  imports                         0.003    0.2%
  libraries                       0.631   42.2%
  parse                           0.023    1.5%
  check                           0.718   48.1%
    check.checker                 0.700   46.8%
    check.effects                 0.003    0.2%
    check.storage                 0.009    0.6%
    check.transfer                0.007    0.5%
  evaluate                        0.002    0.1%
  authority                       0.000    0.0%
  emit                            0.000    0.0%
  libraries: 27 checked alone (0.608 s), 0 cached (0.000 s); slowest:
        0.077 s checked verse/edit/edit.coax
        0.063 s checked apps/openagents-web/newland-network.coax
        ...
```

The phases, in order:

| Phase | What runs |
| --- | --- |
| `load` | Reading the entry file and every `#load`ed file. |
| `lex` | Turning the source into tokens. |
| `imports` | Reading `#import_proto` descriptors and generating their Coax. |
| `libraries` | Checking each loaded file on its own, once, so its `#test` and `#forall` bodies can be left out of this compile. A verdict is cached in `~/.cache/coax-check` (or `$COAX_CHECK_CACHE`). |
| `parse` | Building the syntax tree. |
| `check` | The type checker. `check.checker` is the Coax checker itself: names, types, borrows, effects and generic instances. `check.effects` lowers effects, `check.storage` and `check.transfer` set up its storage and copy its results back. |
| `evaluate` | Compile-time evaluation: `#run`, constants and defaults. |
| `authority` | Every `#test` and `#forall` of an `#authority` file, which run on each compile. |
| `emit` | Lowering and writing C, a header or a report. |
| `native_compile`, `link` | The C compiler and linker (`build`, `lib`, `run`). |

Below the phases, the table lists the ten slowest checks with their steps
(for `coax test`, and for an `#authority` file's checks during any
compile), and the ten slowest library checks.

`--timings-json FILE` (or the older spelling `--timings FILE`) writes the same
facts as JSON: `seconds` per phase plus `total`, `detail_seconds` for the
parts of a phase, a `checks` array (`kind`, `name`, `phase`, `seconds`,
`steps`, `cases`, `passed`) and a `libraries` array (`path`, `seconds`,
`cached`, `checks_alone`). Don't add `detail_seconds` to `seconds`: the parts
are already inside their phase. With neither flag, the driver doesn't read the
clock.

To see which procedures inside a phase cost the time, sample the running
compiler. On macOS, `sample PID 20` while `coax check` runs lists the
checker's procedures by their C names (`coax_p_CK_lookup` is the checker's
`CK_lookup`).

### A baseline for the biggest apps

`coax/perf/baseline.json` records these times for Everglade, newland, myrealm
and the OpenAgents CLI. Compare the current compiler against it from the
repository root:

```shell
coax/web/tool.sh coax/perf/compile-times.coax
coax/web/tool.sh coax/perf/compile-times.coax --app apps/openagents-web/newland.coax
```

The tool runs `coax check APP --timings-json` with an empty library-check
cache, so each run does the same work, and prints each phase beside the
baseline. A phase that grows by more than a quarter and at least half a
second is marked `SLOWER`, and the exit status is 1. After a change that makes
a compile faster or slower on purpose, record a new baseline with `--write`
(and `--note TEXT` to say how busy the machine was) and commit it. Wall times depend on the machine and its load, so compare on
the machine the baseline names.

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
