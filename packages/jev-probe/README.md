# Jev-probe as a subject

The Jev-probe context-construction policy, the strongest claim in
[the test-time capabilities essay](../../docs/essays/2026-09-29-test-time-capabilities.md)
("the same correctness at a fifth of the cost" on Terminal-Bench,
[development results](../../docs/terminal-bench/development-results.md#jev-probe-arms-2026-09-22)),
packaged so it can enter the Gym as a subject ([#9961](https://github.com/OpenAgentsInc/openagents/issues/9961),
the essay's Part III "Package Jev-probe as a subject").

What the policy is, in Coder's program vocabulary (`programs/jev-probe.json`,
a NIP-PRG definition with its host binding):

1. **probe**: a `program` step that runs the read-only evidence guests
   (`evidence-guests`: the repository map, a code search for the task's
   terms, and any test report) in parallel, the battery Jev-probe runs
   before delegating.
2. **select**: a `decide` step that asks Jev's `openagents.evidence-relevance.v1`
   which of the observed evidence the task needs, so the briefing carries
   what helps and nothing else.
3. **work**: a `delegate` step that hands the task to the `coder-one-ask`
   executor with that briefing, under the `directions.md` skill (the v3
   directions: few large steps, run the checks the task names, exercise
   every changed path, search a bulk replace for misses and doubles).

Published, the definition is a NIP-PRG program head (kind `30182`), which
NIP-EVAL accepts as a subject (`nostr::eval_ext::SUBJECT_KINDS`); the
package is also an extension directory `openagents ext eval` can run, with
the program as its one component, so the same bytes can be released under
NIP-EXT and cited by a result.

## The test set

`evals/` is a **cost-primary** suite: it names the `ext-eval-cost-v1` gate
(`openagents ext eval run packages/jev-probe --gate ext-eval-cost-v1`),
which reads **Better** only when the subject arm does the suite at a lower
cost per attempt than the baseline while passing no fewer cases and being
not materially worse on score or time, and never when the cost is unknown.
Four should-fire cases are small, self-contained edits of the shape the
Terminal-Bench panel tasks have (an off-by-one, a missing re-export, a
rename across files, a new test file), each graded on the file the run
leaves; two should-not-fire cases (an explanation, a haiku) check that the
policy stays out of the way of a task no probe helps. Every case asks for
`read` and `write`; none needs a shell.

## What a run here can and cannot show

The claim the essay makes was measured in the Terminal-Bench harness
(`crates/coder-one`, `microcoder tbench`), where the delegate is Codex or
Claude Code with a priced door. Inside `ext-eval`'s sandbox the `work`
step needs the `coder-one-ask` executor and the probe needs the guests,
and a run's cost is priced only when the door prices its lane; Coder's
gateway lanes are unpriced today, so a cost-primary verdict from this
suite is **inconclusive** by construction until a priced door runs it.
The package makes the claim citable and the suite runnable; it does not
carry the Terminal-Bench numbers over. The record of the first local run
is in [the first adoption](../../docs/extensions/measurements/2026-09-29-first-adoption.md).
