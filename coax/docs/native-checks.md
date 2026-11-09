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

## What happens

With `COAX_NATIVE_CHECKS=1`, every place that runs a unit's checks (an
`#authority` compile, a loaded unit's standalone library check, `coax test`)
first runs them as native code:

1. **Plan.** The binary holds every procedure the program's C holds (each
   non-generic procedure and each generic instance the program calls), plus
   each selected check's body and the instances only that check calls. A
   check is left to the evaluator when its body reaches an unevaluated `#run`,
   when a `#forall` has more than 64 bindings, or when its bounds can't be
   evaluated. A `#forall`'s bounds are evaluated once, in the evaluator, as
   they are for an evaluator run, so the cases and their order are the same.
2. **Emit.** `emit_check_binary_c` is the ordinary C emitter in verification
   mode (private names for every procedure, the source-mapped trap runtime)
   with three changes: output (`print`), `bytes_overlap` and foreign
   procedures stop the check with a refusal, since the evaluator rejects them
   (`E0404`); each procedure marks itself in a coverage array on entry; and a
   driver `main` follows the verification ABI 2 table. Arithmetic is always
   checked, as in the evaluator, whatever the compile's profile.
3. **Compile.** `$COAX_CC` (default `cc`) at `-O1` (`COAX_NATIVE_CHECKS_OPT`),
   C11, `-ffp-contract=off`, `-fno-builtin-sin`, `-fno-builtin-cos`, as
   `coax build` does.
4. **Run.** The driver runs the checks in order on a thread with a 1 GiB
   stack. For each it writes `B i` to file descriptor 3, then `E i cases
   procs...` when it passes, `T case line path what` when it traps (the runtime
   then aborts) or `R what` when it refuses. A check that stops the binary, or
   runs longer than `COAX_CHECK_SECONDS` (default 120) of wall time, ends that
   process; the runner starts the binary again at the next check.

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

The whole-unit verdict keys (`authority_options`, `library_key`) gain
` native=1` (or ` native=compare`) when native checks are on, so a verdict
native code gave is never taken by an evaluator-only compile, or the other
way. Natively passed checks are not yet written as per-check reuse records
(ax-check-reuse); after an edit, they run natively again. Recording them
coordinates with #13123, which changes those keys.

## Time limits instead of step budgets

A native check has a wall-time limit, not a step budget. When it runs out,
the runner kills the binary and the evaluator runs that check, which then
reports its step budget as before. Once native is the default, `#steps` and
`#traversal` matter only to checks the evaluator still runs, and the
annotations that no longer matter come out of the tree in their own commit.

## Measurements

See the table in the issue and below, filled in as each unit lands.
