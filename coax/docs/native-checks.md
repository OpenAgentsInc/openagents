# Native checks

Issue: [#13122](https://github.com/OpenAgentsInc/openagents/issues/13122)
(tracking: #13128). Code: `coax/src/native_run.cpp` (the runner),
`emit_check_binary_c` in `coax/src/emit_c.cpp` (the binary), and the hooks in
`parallel_checks`, `run_compilation_checks` and `test_command`.

## Why

Checks dominate compile time. On 2026-11-09, fivelab spent 78.8 s of its
92.6 s compile to C running 1,195 checks (6.8 billion interpreted steps);
Character Create spent 18.9 of 30.5 s on 389 checks; the status page spent
45.6 of 45.9 s on 90 checks. Step budgets (`E0400`) were also the most common
compiler failure in 7,884 agent runs. The tree-walking evaluator pays for
every node it visits. The same checks compiled to C run in milliseconds.

## Where it is on

Native checks are on by default for every unit (#13143). The evaluator
remains the fallback for every check native code doesn't pass.

| `COAX_NATIVE_CHECKS` | What runs a unit's checks |
| --- | --- |
| unset | Native code first, the evaluator for the rest. A run of one check, without `#steps` or `#traversal`, stays in the evaluator |
| `1` | Native code first, for every run |
| `0` | The evaluator alone, as before #13122 |
| `compare` | Both, for every check; differences are reported |

Compiling and starting the check binary costs 0.2 to 0.5 s, more when the
check reaches much of its unit: a library with five small checks takes
0.06 s in the evaluator and 0.45 s natively (a Mac, 2026-11-10). A check
without `#steps` can't spend more than the default 20 million steps, a few
seconds at most, and most single checks spend far less: measured below,
leaving runs of one check to the evaluator was faster on every unit. So a
run of one such check stays in the evaluator. That is also the usual run
after an edit that adds or changes one check.

An earlier version of #13143 left every run of fewer than 16 checks to the
evaluator. Measured, that cost far more than it saved: a run's size says
little about its cost, and Character Create's loaded units include runs of
two or three checks that together take most of a minute in the evaluator.
Cold compiles to C on one build host, one run each, by the least run that
goes native (`COAX_NATIVE_CHECKS_MIN`), 2026-11-10:

| Unit | 1 (every run) | 2 (the default) | 4 | 8 | 16 | evaluator only |
| --- | --- | --- | --- | --- | --- | --- |
| status | 8.3 s | 6.0 s | 5.6 s | 4.1 s | 4.2 s | 76.8 s |
| charactercreator | 35.0 s | 28.7 s | 71.2 s | 64.6 s | 67.0 s | 200.8 s |
| fivelab | 27.0 s | 15.9 s | 57.9 s | 58.9 s | 55.2 s | 266.2 s |

The host was shared with other jobs, so compare within a row, not across
tables. `COAX_NATIVE_CHECKS_MIN=N` (with `COAX_NATIVE_CHECKS` unset) sets
the least; `COAX_NATIVE_CHECKS=1` sends every run native.

## What happens

Every place that runs a unit's checks (an
`#authority` compile, a loaded unit's standalone library check, `coax test`)
first runs them as native code:

1. **Plan.** The binary holds each selected check's body and the procedures
   it can call (the call closure, generic instances included), plus the
   procedures struct defaults call. A check is left to the evaluator when its
   body reaches an unevaluated `#run`, when a `#forall` has more than 64
   bindings, or when its bounds can't be evaluated. A `#forall`'s bounds are
   evaluated once, in the evaluator, as they are for an evaluator run, so the
   cases and their order are the same.
2. **Emit.** `emit_check_binary_c` is the ordinary C emitter in verification
   mode (private names for every procedure, the source-mapped trap runtime)
   with three changes: output (`print`), `bytes_overlap` and foreign
   procedures stop the check with a refusal, since the evaluator rejects them
   (`E0404`); each procedure marks itself in a coverage array on entry; and a
   driver `main` follows the verification ABI 2 table. Arithmetic is always
   checked, as in the evaluator, whatever the compile's profile. The C is
   split into up to `COAX_NATIVE_CHECKS_PARTS` translation units (default:
   the cores, at most 32; about one per 128 KB of C), largest procedures
   first.
3. **Compile.** `$COAX_CC` (default `cc`) compiles every part at once, then
   links them: C11, `-O0` (`COAX_NATIVE_CHECKS_OPT`), `-ffp-contract=off`,
   `-fno-builtin-sin`, `-fno-builtin-cos`, as `coax build` does. `-O0` wins
   for checks: on fivelab, as one translation unit, `-O1` took 19 s more to compile and saved 5 s of
   running.
4. **Run.** The binary runs as several processes at once
   (`COAX_NATIVE_CHECKS_JOBS`; default: the cores, at most 16, and about one
   per 8 checks). Process `w` of `n` runs checks `w`, `w+n`, `w+2n`, and so
   on, each on a thread with a 1 GiB stack. coax0 has no global variables,
   so checks share no state and the split changes no verdict. For each
   check, the driver writes `B i` to file descriptor 3, then `E i cases
   procs...` when it passes, `T case line path what` when it traps (the
   runtime then aborts) or `R what` when it refuses. A check that stops its
   process, or runs longer than `COAX_CHECK_SECONDS` (default 120) of wall
   time, ends that process; the runner starts it again at the process's next
   check.

`COAX_NATIVE_CHECKS_TRACE=1` prints one line per native run on stderr
(eligible, passed, trapped, refused, timed out and crashed counts, and the
emit, compile and run times). Set it to a file name to append the lines to
that file instead: a loaded unit's standalone check runs in a child whose
stderr is discarded. `COAX_NATIVE_CHECKS_KEEP=1` keeps the C and the binary.

A check that passes natively is taken as passed, with its case count, its
time and the procedures it ran. The procedures feed `#authority` execution
coverage (`E0380`) exactly as the evaluator's do. Every other check (a trap,
a refusal, a crash, a time limit, or one the plan left out) runs in the
evaluator as before. So a failing check's diagnostic, location and
`#forall` counterexample are still the evaluator's, and a check native code
can't run costs only its evaluator run.

## What still needs the evaluator

- Compile-time constants, struct defaults and `#run` are evaluated before
  any check runs and reach the binary as values (`coax_kN`), as they reach a
  product build.
- A check whose body still holds `#run` runs in the evaluator.
- A failing check: native code proves passes; the evaluator explains
  failures.
- `coax test --profile steps`, `coax mutate` and `--evidence` records stay
  evaluator-only: they report steps, or are the reference.

## Where native and evaluator can differ

Native code has no step, traversal, storage or call-depth (`E0403`) limit:
those are evaluator costs, not program behaviour. A check that only passes
because its native run has no budget is the point of the change. Native code
does not report reads of `---` locals (`E0405`) or non-finite math results
(`E0412`); a check that relies on those errors fails only in the evaluator.
`COAX_NATIVE_CHECKS=compare` runs both for every check and reports each
disagreement on stderr and, with `COAX_NATIVE_CHECKS_REPORT=FILE`, as one
JSON line per check (`agree`, `disagree`, `native-only-budget`, `fallback`,
`evaluator-only`). The gate for making native the default is no `disagree`
row on the whole tree.

## The check cache

The whole-unit verdict keys (`authority_options`, `library_key`) hold
` native=1` (or ` native=compare`) when native checks are on, so a verdict
native code gave is never taken by an evaluator-only compile, or the other
way.

A check that passes natively in an `#authority` compile is written to the
per-check reuse record (ax-check-reuse), under the key an evaluator pass
gets: the check's tokens, the tokens of every procedure it ran, the
signatures and declarations those name, and the compile's options (#13123,
#13131). The binary's coverage array says which procedures the check ran.
The record holds 0 steps and the storage the checks start with, since
native code uses none of the evaluator's. After an edit, the compile takes
every check the edit doesn't reach from the record and runs only the others,
natively. `--timings` counts both under `check_reuse` (`reused`, `run`).

## Time limits instead of step budgets

A native check has a wall-time limit, not a step budget. When it runs out,
the runner kills the binary and the evaluator runs that check, which then
reports its step budget as before. Native is the default, so `#steps` and
`#traversal` matter only to checks the evaluator still runs; the
annotations that no longer matter come out of the tree in their own commit.

## Measurements

Cold compiles with `--library` on one remote build host, each with a fresh
check cache, 2026-11-09 (`COAX_NATIVE_CHECKS=1` against the evaluator):

| Unit | Checks | Before | After | Check phase before | Check phase after |
| --- | --- | --- | --- | --- | --- |
| status | 90 | 84.6 s | 5.8 s | 83.0 s | 1.4 s |
| charactercreator | 439 | 68.3 s | 39.7 s | 26.0 s | 2.3 s |
| fivelab | 1,256 | 144.5 s | 44.5 s | 104.5 s | 7.2 s |

fivelab's native run: emit 0.6 s, compile 2.1 s (parallel parts at `-O0`),
run 3.7 s (parallel processes). Its evaluator checks took 1,382 s of CPU
time. Warm compiles are unchanged (13.5 s for fivelab). What remains of a
cold compile is mostly the loaded units' standalone library checks, each
of which loads and checks its unit again. All checks in the three units
passed natively, and `compare` found no disagreement in their 2,400
checks, library checks included.

On `main` of 2026-11-10 (after #13131), one build host, two runs each way,
each with a fresh check cache:

| What | Evaluator | Native |
| --- | --- | --- |
| fivelab compile to C, whole | 161.6 s, 162.4 s | 24.8 s, 24.6 s |
| its check phase | 102.5 s, 99.2 s | 8.8 s, 7.8 s |
| its loaded units' library checks | 50.6 s, 53.8 s | 7.2 s, 7.5 s |
| the fivelab module through `wasm.sh` (release, no C or object cache; one run) | 314.4 s | 81.9 s |

Both give the same C and the same module bytes. `compare` on the compile
found 1,662 `agree` rows and no other; `coax test` of the unit passed all
1,662 checks natively in 15.4 s; the module build ran 3,785 checks natively
in 98 runs, and all passed.
