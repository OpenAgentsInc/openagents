# CoderBench, and the first recorded episode

`crates/gym` scores a door on one item. `crates/coderbench` scores an
episode: the whole path from the operator's sentence to the final summary,
recorded as one ATIF trace.

The first task is `devin-fan-out-six`, the delegation
[`programs.md`](programs.md) specifies. It ran on 2026-09-20 and the golden
is **recorded**, not authored:
`crates/coderbench/goldens/devin-fan-out-six.atif.jsonl`.

## What ran

Six read-only questions, one per file, delegated to the Devin CLI on this
computer, in parallel.

| | |
| --- | --- |
| Executor | `devin 3000.10.31`, resolved at `~/.local/bin/devin` |
| Delegations | 6, in parallel |
| Correct | **6 of 6** |
| Wall clock | 23.7 s |
| Summed agent time | 99.0 s |
| Files written | 0 |

## What the decision models were asked

**Program selection**, on kev-4b: `delegate-fan-out` at confidence 1.000.

**Independence**, on kev-4b, over three `Noul` questions:

| Question | Answer | Correct |
| --- | --- | --- |
| The six tasks can run in parallel without colliding | **0.94** | yes |
| Every task is read-only and writes no file | **0.16** | **no** |
| At least one task needs a tool restriction | 0.49 | no signal |

The door got the hard question right and the easy one wrong. The state says
"Every task is read-only" in as many words, and the answer came back at
0.16. This is recorded rather than smoothed over, because a golden that
showed only the flattering half would be worth nothing.

It did not affect the run: the fan-out gated on `independent`, and admission
gated on the deterministic bounds check rather than on
`needs_tool_restriction`, which carried no signal anyway.

## Can a local door do the program lookup?

The question the operator asked. Eight operator sentences over four
programs, with the answer known for each.

| Door | Accuracy | Mean latency | The miss |
| --- | --- | --- | --- |
| kev-4b | **7 of 8** | 1,914 ms | confidence **0.360** |
| kev-0.5b | 6 of 8 | **212 ms** | confidence **0.980** |

**Both misses matter less than how they failed.** kev-4b's one miss was its
lowest confidence on the panel by a wide margin — every correct answer came
back at 0.97 or above, so a threshold near 0.9 would have caught it and
routed to a fallback. kev-0.5b was **confidently wrong**: it missed at 0.980,
where no threshold helps.

So a local door can do this lookup, and the smallest one cannot be trusted
to say when it could not.

**Eight items is not a measurement.** The standard error is about 0.117, so
0.875 against 0.750 is well inside the noise, and nothing here establishes
that kev-4b is better than kev-0.5b at this task. What it does establish is
that neither needs the hosted door to be tried, and that the failure modes
differ in a way worth measuring properly on a real suite.

The panel is in `crates/coderbench/goldens/devin-fan-out-six.evidence.json`.

## Two things the run found that nobody was testing for

**The capability probe cannot trust `PATH`.** The first attempt at the
delegations failed six times with `command not found: devin`. The binary is
on the operator's interactive `PATH` and not on the one a spawned subshell
inherits. [NIP-CC](../nips/coder/NIP-CC.md)'s `detect` resolves a binary
rather than assuming a name resolves, and this is why. A probe that shelled
out to `devin --version` would have reported the capability absent on a
machine that has it.

**A stale checkout answers honestly and wrongly.** A warm-up delegation
asked how many crates the repository has and answered 7. That was correct
for the working copy on disk, which was 20 commits behind `main`, where the
answer is 9. The delegate read what was there. Any future task whose grade
depends on repository contents has to pin the commit, which is why the task
manifest carries `requires.repository` and why the reference
implementation's manifests carry a `base` commit.

## What is not tested yet

The six tasks are independent **by construction** — each reads a different
file — so the independence decision had an easy instance and a known answer.
The interesting case is six tasks that are *not* independent, where the
decision has to refuse the fan-out. That is the second task this directory
should hold, and it needs tasks that write, which also brings conflict
recovery into scope.
