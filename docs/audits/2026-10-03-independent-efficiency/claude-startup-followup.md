# What the Claude startup work changes

Reviewed October 3, 2026, at source
`2ce591f1af2f06052911fe976e73dfb84eb0c158`. This follows the owner's named
Claude conversation, its startup agent, the implementation, and the committed
study rows. Private conversations are not reproduced. No new executor or
live test ran for this review.

**The newer lean Claude configuration beats raw Claude defaults on the
standing task set. It should be the control for our next product briefing
experiment.** Its useful changes are explicit stopping conditions, removal
of redundant discovery and checks, and preparation alongside routing. Our
separate historical replay tests different components and does not replace
this product measurement.

## The reported result checks out

Independent recomputation finds all 84 expected rows: seven tasks, four arms,
three trials, every arm passing 21/21. The
[derived results](claude-startup-followup.json) retain input hashes and task
breakdowns; the [original rows](../../../bench/efficiency/results/2026-10-03b.jsonl)
remain the source.

| Comparison | Recorded cost ratio | Recorded time ratio |
| --- | ---: | ---: |
| Lean Claude / raw Claude | 0.4488 | 0.8587 |
| Routed default Codex / raw Claude | 0.3160 | 1.8322 |
| Routed default Codex / raw Codex | 0.8518 | 1.6500 |

The lean route is 55.1% cheaper and 14.1% faster by these measures. Its mean
cost per pass is $0.1334 against $0.2972; median run time is 34.49 seconds
against 40.91. Ratios use sums of per-task means, not those displayed medians.
The whole study records $13.3317 at list price, not subscription charges.
The [live efficiency page](https://openagents.com/efficiency), read during this
review, shows these results and still correctly says the default Codex route
loses on time.

The quoted small-fix ratios come from a separate five-trial panel. In the
full standing run, lean time is 1.565× raw on `fix-git` and 1.369× on
`fix-code-vulnerability`. Aggregate improvement does not make every task
faster. The [standing runbook](../../../bench/efficiency/README.md) retains
both panels and earlier unsuccessful variants.

## What actually improved

1. **A precise stopping rule removes extra work.** In #10254, the old prompt
   caused a completed small fix to acquire new tests, a revert to prove them,
   and reapplication. Another run waited on interactive standard input. The
   new [finish instruction](../../../crates/coder-one/prompts/headless/finish.md)
   directs the agent to run task-named checks or nearby existing tests and
   confirm requirements from a fresh shell. Its
   [pace instruction](../../../crates/coder-one/prompts/headless/pace.md)
   encourages parallel independent calls and noninteractive commands. The
   fresh-shell clause followed a real failure that depended on a temporary
   environment variable. A good brief needs completion conditions and
   execution assumptions as well as source.
2. **Preparation must replace discovery.** The old workspace survey cost
   4–7 seconds, and the session read the files again. The lean route skips
   it. Briefed Sonnet runs omitted the initial watcher-file read seen in all
   four unbriefed controls; internal use of the supplied source was not
   observed. Our brief omitted the requested test and a relevant reader
   limit. The useful objective is
   sufficient evidence that displaces work, measured against the executor's
   actual behavior.
3. **Independent judgments can overlap.** Commit
   [`b520a8b37c`](https://github.com/OpenAgentsInc/openagents/commit/b520a8b37c405d7521c4f5812cddf016ad66ffda)
   starts issue judgment and recipe groundwork alongside routing. Without
   a survey, class, knowledge, and check selection also run concurrently.
   It does not start the engine before preparation finishes. The reported
   7.9-to-2.2-second reduction is wall time minus session duration, which
   includes settlement and checks outside the session; it is not a directly
   instrumented engine-start interval.
4. **Verification needs an owner.** Commit
   [`0565629714`](https://github.com/OpenAgentsInc/openagents/commit/056562971410ea99a0355760c1a0d36776309505)
   removes a duplicate run of frozen checks when independent verification
   immediately follows. This supports an explicit check schedule. It does
   not justify dropping independent verification: our deeper-import
   diagnostic found a real bug after the original checker passed.

The child conversation adds a concrete briefing example. Component tests
passed, but its first live probe failed because preparation created a fresh
task-store root with default permissions. Commit
[`4c087cb7b0`](https://github.com/OpenAgentsInc/openagents/commit/4c087cb7b02b11eda50372ea8ba410b67b0bcc71)
restores the store's `0700`/`0600` contract. A brief that includes the consumer's
storage contract and a nonexistent-store fixture could prevent that class of
rework. That is a testable hypothesis, not a measured saving.

The conversation's elapsed time also includes builds, three study batches,
deployment, and verification. Treating its entire duration or completion
token counter as reasoning waste or billed usage would be incorrect.

## How this changes our next experiment

**Extend the existing `Ahead` mechanism.** It already provides the parallel
preparation path we wanted. First test its ownership, freshness, and accounting
in isolation; then attach improved deterministic source evidence.

Source review of [the producer](../../../crates/coder/src/task/chat_client.rs)
and [`Ahead`](../../../crates/coder-delegate/src/recipe.rs) finds these limits:

- The match checks the exact request and prior conversation, knowledge-base
  directory count, and only the first directory path. It does not bind KB
  content, project-KB identity, or decision configuration.
- Setup occurs before the pending marker exists. A consumer that arrives
  first can recompute immediately; after a marker exists it waits up to five
  seconds. A late producer is not joined or cancelled. Duplicate work is
  possible; its incidence in the study is unknown.
- Ready results have no age check on consumption. The 60-second value controls
  pending markers and opportunistic cleanup. Consumption reads then removes
  the file, so it does not atomically enforce a single consumer.
- Consumed Jev steps join the run's accounting. Unused, rejected, and late
  preparation lacks a durable aggregate cost record.

These are source observations, not newly reproduced runtime failures. Today's
`Ahead` carries KB entries and request-derived checks, not repository snippets.
A source digest becomes necessary when source excerpts are added; the current
freshness concern is primarily knowledge and configuration.

| Isolated experiment | What to measure |
| --- | --- |
| Preparation ownership and accounting | Two consumers, delayed producer, timeout, cancellation, changed KB/configuration, and expired ready result; exactly one consumption and a receipt for every call |
| Integration contracts in briefs | Whether deterministic retrieval selects the store permission contract, correct test, fixture dependencies, and connected producer/reader limits |
| One verification schedule | Count each required check, bind it to the final candidate, and time preparation through independent acceptance |
| Source evidence in the shipped lean route | Current lean Opus and lean Sonnet, each with and without the frozen source pack, on new tasks; hold the base prompt, tools, effort, checks, and repair allowance fixed within each model pair; pin and verify the CLI version and served model |

The [earlier component proposals](briefing-model-factorial/README.md#next-isolated-experiments)
still apply. The integration target and comparison baseline are now more
concrete. Our Sonnet result motivates the model factor; it does not establish
that its savings multiply with this study's lean-session savings.

Our earlier warm source preview took 0.220 seconds in one measurement. It
could fit inside the observed routing interval, making it worth testing as
parallel groundwork. Measure cold and warm critical-path time under actual
concurrency; separate latency samples do not prove the combined path is free.

## Measurement limits to carry forward

- **The external checker is outside the reported timer.** In
  [`study.py`](../../../bench/efficiency/study.py), arm execution returns
  before `check()` runs. Template copying and environment preparation also
  precede timing. The displayed “time to a checked result” is therefore run
  time for a result subsequently checked, not full time through acceptance.
- **The comparison changes several settings together.** Raw Claude inherits
  defaults; lean explicitly changes its system prompt, tools, effort policy,
  cache duration, connectors, and preparation. Raw rows record Opus 5.5;
  routed rows omit actual model and effort fields. This is useful product
  configuration evidence, not an isolated estimate of Jev or source packing.
- **Some costs remain outside the rows.** The study correctly adds embeddings
  to recipe and engine costs. Router and issue judgments, plus any unused
  speculative work, are not fully represented. Task route cost and study cost
  also have different embedding coverage. Charge every preparation outcome
  before claiming a complete cost comparison.
- **These tasks were used for tuning.** The reported bootstrap resamples
  trials within the same seven tasks. Independent resampling reproduces the
  lean time interval approximately. A retrospective sensitivity calculation
  that also resamples tasks spans 0.653–1.106; it is not a new success gate or
  a generalization guarantee. Use unseen tasks for the next claim.

The next useful advance is to combine their reduced overhead with stronger,
bounded evidence and explicit preparation receipts. A larger prompt or more
preparation calls have to earn their cost independently.
