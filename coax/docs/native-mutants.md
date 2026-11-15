# Native mutants

Issue: [#13582](https://github.com/OpenAgentsInc/openagents/issues/13582)
(after #13581), and the follow-up
[#13587](https://github.com/OpenAgentsInc/openagents/issues/13587). Code: `coax/src/native_mutants.cpp` (the runner), the mutant
sites in `emit_check_binary_c` (`coax/src/emit_c.cpp`), the decisions in
`coax/lib/compiler/mutation-schedule.coax` (`NM_*`), and the hooks in
`mutate_unit` (`coax/src/verify.cpp`). Test:
`coax/tests/native-mutants.coax`.

## Why

`coax mutate` runs every mutant's checks in the compile-time evaluator. Under
a large root that is slow: the rules' battle root has 641 checks and about
3.2 billion evaluator steps, so one mutant that survives costs what the whole
unmutated run costs, and a first pass over a 1,130-mutant unit took 90
minutes on 20 cores (#13581). The same checks as native code run in under two
seconds.

## What it does

```shell
coax mutate rules/srd/battle.coax --library --unit rules/srd/battle-strike.coax --native
```

1. **Emit once.** After the unmutated checks pass in the evaluator, the
   checks and everything they reach are emitted to C as the native check
   binary is ([native-checks.md](native-checks.md)): the ordinary C emitter in
   verification mode, with checked arithmetic, bounds and casts. Every
   mutation site of the run's inventory is behind a switch,
   `coax_nm_mutant == ID`. `ID` is the mutant's place in the evaluator's
   inventory plus one, and 0 is the unmutated program, so the mutant set, its
   names and its order are the evaluator's by construction: the sites come
   from the same list.
2. **Compile once.** One C compile and link, as for native checks
   (`$COAX_CC`, default `cc`; `-O0` unless `COAX_NATIVE_CHECKS_OPT` says
   otherwise).
3. **Run the unmutated binary.** Each check that passes there is one native
   code can judge. The run also counts each check's ticks: one for each
   procedure entered and each loop iteration.
4. **Run each mutant.** One process a mutant. It runs the checks the
   evaluator would run for that mutant, those that enter the mutated
   procedure first, each group cheapest first, and stops at the first that
   fails. Before them it tries the check a `--killers` file names for the
   mutant, then the checks that killed its procedure's other mutants: the
   file's, then this pass's, most often first (see "The order" below). A check fails when it traps (an assertion, an overflow, a bound),
   refuses (output or a host effect, which the evaluator rejects as `E0404`),
   crashes, or runs out of ticks or wall time. The processes run at once, each
   one a build worker from the machine-wide budget, as the evaluator's
   mutation workers are (`--jobs`). A worker keeps its token from one mutant
   to the next, so the pass's own load doesn't shrink it; on a host already
   at its load limit a pass starts with one worker, as any coax build does.
5. **Record the kills.** With `--killers FILE`, the native kills go into the
   file as soon as the native pass ends (see "An interrupted run" below).
6. **Let the evaluator decide.** A mutant native code killed gets the check
   it stopped in as its hint, exactly as `--killers` gives hints. The
   evaluator runs that one check first, and only its failure there is the
   kill. A mutant native code ran without a kill runs the evaluator's whole
   schedule, cut into chunks across the workers (see "A survivor across the
   workers" below).

So the report is the one `coax mutate` prints without `--native`, byte for
byte, with the same exit status. Native code only orders the evaluator's
work. At its end, `--killers FILE` records the evaluator's killers.

## A survivor across the workers

A mutant native code did not kill is usually a survivor, and a survivor runs
every check its schedule holds. Under a large root that is about the cost of
the unmutated run on one core. So its schedule is cut into chunks of checks,
and the mutation workers take chunks as they take mutants
(`MS_chunked`, `MS_chunk_target`, `MS_chunk_cut` and `MS_chunk_runs` in
`lib/compiler/mutation-schedule.coax`):

- The order is fixed before any worker starts: the order the evaluator would
  read the mutant's checks in, without the other mutants' outcomes from this
  run (those depend on timing). With `--mutation-trace` or `--source-order`
  it is the recorded check, then source order, as before.
- A chunk is a run of consecutive checks in that order, cut so that no chunk
  costs more than the heaviest check, or a quarter of each worker's share of
  all the chunked work when that is more. A check's cost is its unmutated
  steps. The workers take the costliest chunks first, then the other mutants
  whole.
- The mutant survives only when every chunk passes. Its killer is the failing
  check earliest in its order. A chunk runs a check only while no failure
  earlier in the order is known; it stops for an earlier failure, never for a
  later one. So every check before the killer ran and passed, whichever
  worker ran it.
- Its checks and skips in `--mutation-stats` count what one worker running the
  whole schedule in order would have run, up to the killer.

So the report, `--mutation-trace` and the stats are the same for every
`--jobs`, and one survivor ends in about the time of its heaviest check.
`--mutation-stats` adds a line when there are chunks:

```text
mutation-chunks 3 mutants in 41 chunks, at most 20 workers at once
```

Mutants native code killed or did not run stay whole. Without `--native` no
mutant is cut. Two variables are for the tests: `COAX_MUTATE_CHUNK_ALL=1` cuts
every mutant, and `COAX_MUTATE_CHUNK_CHECKS=N` makes every chunk N checks.

## An interrupted run

With `--killers FILE`, the native kills go into the file when the native pass
ends, not only the evaluator's killers at the end of the run. The file keeps
its other lines: other units' killers and the mutants native code didn't
kill. A run stopped while the evaluator confirms the kills starts again from
them: each killed mutant's native process runs the recorded check first and
stops there, and the evaluator runs it first too. The end of the run writes
the evaluator's killers, as before.

## The order

Without `--mutation-trace` or `--source-order`, a mutant's native process
tries, before its procedure's schedule: the check the `--killers` file names
for it, then up to eight checks that killed its procedure's other mutants,
the file's first and then this pass's, most often first. Mutants of one
procedure are mostly killed by the same few checks, so most die on the first
check they run. With `--mutation-trace` or `--source-order` the order is the
procedure's schedule alone, so the trace is the same for every `--jobs`. The
order changes which failing check native code names, never a verdict.

## The limits

The baseline runs the checks as native code first, as `coax test` does. A
check that passes there gets an evaluator budget of a thousand times the
20,000,000-step default (`MS_baseline_budget`), so a check that `coax test`
passes can't fail the baseline only for its step count. Its `#steps`, when it
has one, still binds a check that native code didn't pass. The evaluator then
gives a mutant's check ten times its measured unmutated steps and never fewer
than 10,000, capped by the check's `#steps` when that is larger than its
measured steps (`MS_mutant_budget`). `tests/native-mutants/heavy.coax` is a
check of about 30,000,000 evaluator steps: it passes the baseline, and its
mutant that loops forever runs out. Native code gives a mutant's check ten
times its unmutated ticks and never fewer than 10,000 (`NM_tick_limit`). Ticks bound every loop and every
call, so a mutant that loops or recurses forever stops. A wall-time limit of
twenty times the check's unmutated time, and never under ten seconds
(`NM_wall_ms`), stops anything ticks can't see.

Ticks are not steps. A mutant that makes a check somewhat longer can run out
of one and not the other. Then native code and the evaluator differ, and the
evaluator's answer is the report's: a native kill the evaluator doesn't
confirm runs the whole schedule there, and a native survivor does too. The
same holds for the rules only the evaluator has: its call depth (`E0403`), a
read of an unset local (`E0405`) and a non-finite result (`E0412`).

## Checks the evaluator cannot run

Native code has no traversal limit, no value-slot limit and no rule against a
NaN bit pattern, and the evaluator has all three. So a check `coax test`
passes can still fail in the baseline's evaluator: over the aggregate
traversal limit, over the value-slot limit (the checks' slots add up in one
evaluator), or on `f32_from_bits(0x7fc00000)` (`E0412`). Such a check, failed
by the evaluator and passed as native code (`MS_evaluator_cannot`), doesn't
fail the baseline. It is left out of every mutant's evaluator schedule
(`MS_scheduled`), and native code runs it for each mutant, with or without
`--native`. It runs last in each native schedule, after the checks the
evaluator would confirm, in source order. A native kill there is the
mutant's verdict, the one place native code decides one (`MS_verdict`). A
mutant that loops forever there runs out of native ticks. The report names
each such check and why:

```text
evaluator cannot run #test 'many copies, then a sum' (E0400 compile-time aggregate traversal limit exceeded: 200001 value and field visits needed, the limit is 200000 at tests/native-mutants/traversal.coax:13:10): native code runs it for each mutant
```

A mutant that passes every check the evaluator runs is a survivor when
native code ran the left-out checks for it. When native code could not (no
mutant binary, or the site isn't in it), the mutant is not decided: the report
names it and says why, and the summary counts it (`25 mutants, 4 killed, 0
survived, 21 not decided`). A mutant not decided doesn't fail the run. A
check that fails both in the evaluator and as native code fails the baseline
as before. `coax mutate --slots N` gives the checks a smaller value-slot
limit, as `coax test --slots N` does.

## What native code can't judge

- **A site with no switch.** A site the emitter never writes, such as one
  inside a value folded at compile time, has no switch. Its mutant is the
  evaluator's alone.
- **A check the binary can't hold or that doesn't pass there.** A `#forall`
  of more than 64 bindings, a body that still holds `#run`, or a check that
  stops its unmutated native run. A mutant such a check is scheduled for can
  only survive "what ran" natively.

The run needs a C compiler. Without one, when the C compiler rejects the
binary, or when no check can be emitted, the run says so once on standard
error and continues as without `--native`:

```text
coax: mutate --native: no C compiler ($COAX_CC, else cc, is not on PATH); the evaluator runs every mutant's checks, as without --native
```

## When native code and the evaluator differ

After the evaluator finishes, each mutant's native outcome is compared with
its verdict (`NM_compare`):

- **Agree.** Both killed it with the same check, or both let it survive.
- **Limits differ.** One of them stopped at a limit the other doesn't have.
  Counted, and listed with `--mutation-stats`.
- **Disagree.** Anything else: a native trap in a check the evaluator
  passes, or a native survivor that a check native code ran kills in the
  evaluator. This is a failure of the native path, a coax bug. It is
  reported on standard error, mutant by mutant, and never changes a verdict.
  With `COAX_NATIVE_MUTANTS_STRICT=1` the command exits 3.

`--mutation-stats` adds one line for the pass:

```text
native-mutants agree=1130 limits=0 disagree=0 not-compared=0 emit=0.08 compile=2.31 baseline=1.90 run=14.20 after-baseline=61.40
```

`after-baseline` is the time from the end of the evaluator's unmutated run to
the end of the evaluator's confirmation: the whole cost of the mutants.

| Environment | Meaning |
| --- | --- |
| `COAX_NATIVE_MUTANTS_TRACE=1` | One line on standard error: sites with a switch, checks run natively, outcomes by kind, the checks the mutant processes began and how many mutants the first one killed, times |
| `COAX_NATIVE_MUTANTS_REPORT=FILE` | One line a mutant: its index, native outcome, the check it stopped in, its `--killers` key. The rows that say `survived` are the native survivors |
| `COAX_NATIVE_MUTANTS_STRICT=1` | A disagreement makes the command exit 3 |
| `COAX_NATIVE_MUTANTS_KEEP=1` | Keeps the C, the binary and the schedules |
| `COAX_NATIVE_MUTANTS_JOBS=N` | Mutant processes at once (default: the mutation workers) |

## How a site is emitted

| Mutant | C |
| --- | --- |
| A literal: `3` became `4` | `(coax_nm_mutant == 7 ? INT64_C(4) : INT64_C(3))` |
| An operator: `a < b` became `a <= b` | Both operands once, in source order, into temporaries; then `(coax_nm_mutant == 8 ? (t0 <= t1) : (t0 < t1))`. Arithmetic uses the checked helper of each form |
| `a && b` became `a \|\| b` | `a` once; `b` only when the operator chosen needs it, so the short circuit is each operator's own |
| A condition negated | `if ((coax_nm_mutant == 9) != (condition))` |
| A statement removed | `if (!(coax_nm_mutant == 10)) { statement }` |
| `+=` became `-=` | `if (coax_nm_mutant == 11) { x -= v } else { x += v }` |
| A lowered spatial operator | The arguments once, then the call to one procedure or the other |

Each form is the emitter's own output for the tree before and after the
mutant's own change (the change `coax mutate` applies in the evaluator), so
no second description of a mutant exists.

## Why not the checked-AST profile

`coax checks-c` has a narrower contract
([verification-differential.md](../verification-differential.md)). For the
battle root it refuses 479 sites in 198 of 1,515 reached procedures: every
assignment or return of a string or view, optionals, and `switch`. `checks-c`
now lists them all. Admitting them there, each with its differential case,
is a larger project. The native check binary already runs all 641 battle
checks, and here its verdicts are never trusted: the evaluator confirms each
one. That is why native mutants use it.

## The test

`coax/web/tool.sh coax/tests/native-mutants.coax COMPILER [UNIT.coax ...]`
mutates a fixture and seven real `#authority` units both ways. The two
reports must be the same bytes with the same status, and for every mutant
the native outcome must be the evaluator's. It also runs the three cases
where native code can't run. Then, for the fixtures and each unit, the report
and the trace of `--jobs 1` must be those of `--jobs 3` and `--jobs 8`, whole,
with every mutant in chunks, and with one check a chunk, and with `--native`
the trace must be the same for every `--jobs`. `tests/native-mutants/chunks.coax`
holds a mutant two checks kill, the later one the heaviest, so it runs and
fails first, and the earlier stays the killer. `tests/native-mutants/sole.coax` holds the heaviest check, over the
evaluator's default budget, as the only check that reaches a procedure: its
mutants die by it with and without `--native`, whole and in chunks, at
`--jobs` 1, 3 and 8 (#13613). `tests/native-mutants/resume.coax`
is stopped after its native pass and run again: the file holds the native
kills, every mutant dies on its recorded check, and the report is that of a
run that was never stopped. `tests/native-mutants/traversal.coax`, `slots.coax`
and `nan.coax` each hold a check the evaluator cannot run (`--traversal
200000`, `--slots 3000`, a NaN bit pattern): every mutant gets a verdict, the
same at `--jobs 1` and `--jobs 8`, a loop mutant runs out of ticks, and with
native checks off the baseline still fails. With a C compiler that refuses
the mutant binary, the traversal fixture's mutants that only the left-out
check reaches are not decided. `tests/run.sh --only native_mutants` runs it.

## Measured

coderos-4080 (28 cores, `--jobs 20`, other jobs running), 2026-11-14:
`rules/srd/battle-strike.coax` under the battle root, 1,132 mutants.

| Step | Time |
| --- | --- |
| The evaluator's unmutated run (unchanged by `--native`) | about 13 min |
| Emit the C | 0.06 s |
| Compile and link | 1.0 s |
| The unmutated native run, 641 checks | 1.8 s |
| 1,132 mutants as native code | 34 s |
| The evaluator's confirmation, one check a mutant | 205 s |
| From the end of the unmutated run to the report | 243 s |

All 1,132 mutants had a switch and all 641 checks ran natively. Native code
killed every mutant (1,125 by a trap, 7 by ticks) and the evaluator confirmed
each with the check native code named: 1,132 agree, none differ. The closing
comment on #13582 has the evaluator-only time for the same pass.

Then a mutant that survived still cost the evaluator its whole schedule on
one worker: under this root, about what the unmutated run costs. Since
#13587 that schedule is spread across the workers.
