# By-value arguments in the evaluator

Issue: #13609. This describes the compile-time evaluator (`coax/src/eval.cpp`),
which runs `#run`, constants, and every check under `coax test` and
`coax mutate`. It does not change the language, the checker or generated C.

## What the language says

A struct or array argument is passed by value. The callee sees the value as it
was at the call, whatever happens to the caller's variable afterwards
([spec section 7](../spec.md)). Parameters are immutable (E0344).

## What the evaluator does

Making the copy was most of the time of a check on large records. The rules'
`RsCreature` is 494 values, each nested array and struct in its own storage,
and a call that read one bit of it spent 15 µs copying it and 0.4 µs reading
it.

The evaluator now gives the parameter the argument's own storage when no
program can tell the difference. It copies in every other case, as before.

A parameter shares its argument's storage only when all of these hold:

1. **The argument names storage.** It is a variable, a constant, a field or an
   element (`c`, `TABLE`, `b[0].c[k]`). A call's result or a literal is already
   a fresh value and was never copied.
2. **The callee does not write the parameter.** The checker refuses that
   (E0344). The evaluator looks for an assignment to the parameter anyway.
3. **The callee takes no slice of the parameter.** A slice is a view of the
   parameter's storage, and a view can outlive the call.
4. **No parameter of the callee is or holds a writable view.** A procedure
   that runs at compile time reaches storage it did not make only through its
   arguments. There are no global variables, and a constant is frozen. It
   writes such storage only through a writable view `[] T`: a read-only view
   `[] const T` cannot be made writable, and cannot hold a writable one
   (E0950). So a callee with no writable view among its parameters cannot
   write the caller's storage, and neither can anything it calls, however the
   argument is aliased.
5. **No later argument of the same call can write.** Arguments are evaluated
   in order. Only a call can write, and by rule 4 only a call whose callee is
   handed a writable view. `f(a[0], bump(a[..]))` copies `a[0]`.
6. **The callee has no `#transaction`.** A transaction saves and restores its
   frame's storage by identity.

A copy of at most eight numbers, bools, enums, strings or views in one
storage (a `Vec3`, a pair) is always made: it costs less than the bookkeeping
that stands in for it.

Every call names its callee, because procedures are not values. So the callee
is always known.

Rules 3 to 6 are read from the checked tree once for each evaluation, never
kept across a mutant. Rule 4 is decided by parameter types alone: it does not
depend on what the callee's body does, so a mutant cannot change it.

## What stays the same

Every observable number is the same as with the copy:

- **Results.** By the rules above.
- **Steps.** A copy charges no steps.
- **Traversal work.** The call charges the visits the copy would make.
- **Value slots, and arrays and structs.** Until the callee returns, the call
  holds the slots and storages the copy would hold. A refusal or a near-limit
  note names them at the same place, with the same counts.
- **Refusals.** When the copy itself would be refused, or would set a peak
  whose holders the compiler records, the evaluator makes the copy, so the
  error is produced by the same code at the same point.

So no check starts or stops reaching a limit, and the budgets that
`coax mutate` derives from measured steps do not move. The advice in an E0400
report still applies: a call that passes a large record by value still charges
its traversal work.

## Checking it

`coax/tests/eval-lend.coax` (unit `eval_lend` of `coax/tests/run.sh`) runs
`coax/tests/eval-lend/` three ways and compares them:

- as shipped;
- `COAX_LEND_ARGS=0`: every argument copied, as before the change;
- `COAX_LEND_ARGS=check`: the evaluator shares the storage and also makes the
  copy. It stops the compiler if the copy charged anything other than what
  was predicted, or if the argument differs from the copy when the callee
  returns.

`cases.coax` has one scene for each way a program could tell: a callee that
writes a copy of its parameter, nested arguments, the argument's storage
behind a writable view, a later argument that writes, borrows, recursion,
transactions, deferred statements, constants, returned parameters. Its answers
are the ones native code prints, written out as literals. `limits.coax` holds
the accounting to one report for all three ways.

Both environment values are for tests. Run any check with
`COAX_LEND_ARGS=check COAX_NATIVE_CHECKS=0 COAX_SERVE=0 coax test FILE` to
verify it on that file.

## Cost elsewhere

A check that passes no large record by value gets nothing from this. On
coderos-4080, a suite that is almost all scalar arithmetic
(`apps/openagents-web/fairy.coax`, 437 checks in the evaluator alone) took
65.3 s before and 67.1 s after, about 3% more. The profile shows no new
function: the evaluator's own procedures are each slightly slower.

## What it does not do

Copies that are not arguments are unchanged: `me := b[0].c[i]`, a returned
record, an assignment, a literal. So is an argument to a callee that takes a
writable view.

The rules' battle checks (647 checks, 1.29 billion steps) visited 5.07 billion
values in copies. Arguments that name storage were 2.15 billion of them, and
2.08 billion of those are no longer copied. The other 2.9 billion visits are
copies that are not arguments.
