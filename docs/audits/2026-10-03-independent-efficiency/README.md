# Independent audit of Coder's cost, context, and delegation

Recorded October 3, 2026 UTC (October 2 in America/Chicago).
Audit issue: [#10252](https://github.com/OpenAgentsInc/openagents/issues/10252).
Source snapshot: [`5e22f2af962f7e848df3aee1385e6cf165132123`][snapshot].

Extension: [System One briefing experiments and runnable prototype](system-one-briefing.md),
tracked in [#10253](https://github.com/OpenAgentsInc/openagents/issues/10253).
The extension reads the entire retained TypeSafe corpus and adds a concrete
program for preparing context in advance. Its new measurements are separate
from the historical agent comparisons below. The later
[Tree-sitter experiment](briefing-syntax-results.md) measures source-span
selection independently of file ranking and agent execution.
The [conversation audit](conversation-briefing-audit.md) examines actual recent
Claude activity and identifies concrete preparation experiments from observed
coordination, setup, retrieval, and verification problems.

## Executive assessment

**Some of the earlier context and delegation work is missing from the
current default paths. The evidence supports recovering selected mechanisms,
but does not support restoring the old controller as a package.**

The strongest findings are:

1. **Current execution paths differ substantially.** Modern Codex defaults
   to the Microcoder loop; modern Claude defaults to a native Claude
   session. Both get an initial Jev briefing. Native sessions then give
   context management and execution back to the CLI. The older `coder`
   delegate door has another path that bypasses the modern recipe entirely.
2. **The old requirement-coverage packer is absent from the modern recipe.**
   The recipe selects evidence, then uses a section-based 12,000-character
   briefing. It can omit selected evidence or clip the instruction tail.
   Native sessions receive that capped instruction; the loop also receives
   the original request. This creates a task-coverage difference between
   modes, beyond their prices.
3. **Microluna's short specialist sessions, requirement ownership, and
   isolated parallel edits are not the modern default architecture.**
   Microcoder's per-step knowledge retrieval, generated acceptance suite,
   and stronger-model routing also remain disabled in ordinary repository
   runs. Initial knowledge selection and existing frozen checks survive.
4. **The newest retained head-to-head shows a cost/latency tradeoff.**
   Across seven small tasks, three trials each, all four arms pass every
   independent check. Routed Codex costs **16.6% less** than raw Codex but
   takes **61.9% longer** by the study's aggregate wall-time metric.
   Lean Claude costs **37.3% less** than raw Claude but takes **34.1%
   longer**. These are observed configurations, not isolated estimates of
   Jev's benefit.
5. **Matched historical evidence argues against indiscriminate control.**
   With the Claude model, effort, tools, core prompt, and cache policy
   matched, the larger older controller study costs **40.0% more per
   accepted result**. Its apparent pass gain is inconclusive. Persistence
   consumes about half its Claude cost.
6. **A narrower historical context change is promising.** Preserving the
   records needed by a log-summary task changes Luna from 0/3 to 3/3
   passes. The treatment also changes the rendered guidance, so it supports
   better evidence packaging without isolating one packing algorithm.
7. **Accounting and experiment design need work before broader claims.**
   Preparation embeddings can be omitted from task totals, failed session
   attempts can lose accounting, resumed sessions overwrite raw artifacts,
   and the standing harness excludes independent checker time from its
   wall-time field. All-pass results on seven familiar tasks cannot establish
   reliability on long issue work.

**Recommended direction:** preserve native session caching, make task
requirements and admitted instructions complete, recover targeted evidence
packing, and test narrow specialist delegation as a separate treatment.
Measure each addition against a configuration-matched native agent. Treat
general monitoring, repeated suite generation, and unconditional persistence
as costs that must earn their place.

**The stronger System One opportunity is to compile better briefings from
repository facts and small semantic judgments.** Precompute source structure,
package ownership, and bounded history; retrieve a broad candidate pool; ask
typed questions about relevance and missing evidence; then let code preserve
requirements, select exact source spans, and render an expandable brief.
Preparation can inspect more evidence than the executor initially receives.
This can save repeated discovery while making each stage independently
testable. The [extension](system-one-briefing.md) specifies these experiments
and includes a runnable deterministic baseline. It does not claim a new
Jev, Claude, or Codex outcome improvement.

### Reading guide

- [Earlier context and delegation mechanisms](#1-what-the-earlier-systems-were-trying-to-achieve)
- [Current execution paths](#2-what-currently-runs)
- [Independent current comparisons](#3-independent-head-to-head-results)
- [Historical evidence](#4-historical-evidence-that-changes-the-interpretation)
- [Recent iteration results](#5-what-the-recent-iterations-actually-establish)
- [Source and measurement findings](#6-findings-to-resolve-before-making-broader-claims)
- [Standalone study protocol](#7-a-standalone-head-to-head-program)
- [Recommended work order](#8-recommended-order-of-work)
- [System One theory, code indexing, history, and isolated experiments](system-one-briefing.md)
- [Briefing opportunities in actual agent conversations](conversation-briefing-audit.md)
- [Runnable issue briefing preview](../../../crates/briefing-lab/README.md)

## Scope and method

This is a separate source audit and reanalysis of retained evidence. It
does not reuse the standing report's calculation functions, change routing,
run paid agents, deploy a host, or claim results from concurrent work.
The audit holds the source snapshot above fixed even if newer work lands
before this document.

The analysis distinguishes:

| Evidence | What it establishes |
| --- | --- |
| Source path | A behavior is implemented and reachable under stated conditions |
| Retained trial | That configuration produced a recorded outcome on that task |
| Matched comparison | A narrower set of differences can explain the outcome |
| Proposed experiment | A hypothesis still needs measurement |

The independent [recomputation script](recompute.py) reads files through
`git show` at the frozen revision. It does not import the product report or
benchmark harness. [Results](results.json) include input SHA-256 hashes,
arm totals, per-task means, cache counts, paired outcome keys, independently
resampled intervals, and historical matched results. For the two-task
matched pilot, it also checks retained native Claude result costs against
the trial summary; the maximum discrepancy is zero.

Cost means the retained engine list-price estimate plus recorded controller
charges. It is not an invoice or a measure of subscription allowance.
This audit uses the historical price tables that produced the records;
it does not assert current provider prices. No private conversation,
credential, or new raw user trace is included.

## 1. What the earlier systems were trying to achieve

The useful idea was to decide **what each model needs to know and do**
before paying it to rediscover the workspace. That idea appeared in
several different systems, with different evidence.

### Coder One: prepare a capable native delegate

Coder One combined host probes, Jev judgments, selected files, requirements,
environment facts, and knowledge into a briefing for a native agent.
Policies could also choose a smaller tool list, replace the core prompt,
lower reasoning effort, change cache duration, monitor progress, add
acceptance scenarios, and request repair or persistence.

Those are separate interventions. An improvement against default Claude
does not establish that the controller caused it when effort, tools,
prompt, and cache policy changed too. The early eight-task development
panel is a useful example: both arms pass 24/24, while the controller
configuration costs $1.2846 against $3.2588 and uses 574.75 against 919.23
agent seconds. That reproduces the roughly 61% cost and 37% time saving,
but several configuration changes travel together. See the
[historical assessment][historical-cost] and the independent
`older_retained.development_eight` results.

### Microluna: code constructs short, specialized sessions

Microluna deliberately owned the input items, their ordering, stable
prefixes, and a small set of native tools. It aimed to use many short
sessions whose context code and Jev rebuilt, with typed finishes and
per-call usage. That offered more context control than a native CLI,
whose own instructions, tools, history, and compaction remain involved.
See the [Microluna transport design][microluna].

The [implemented v7 parallel design][microluna-parallel] went further:

- Split requirements among three acceptance writers.
- Prove deciding tests fail on an untouched snapshot.
- Keep already-passing regression guards while identifying requirements
  they do not decide.
- Start an edit session while suite writing proceeds.
- Give edit sessions their own requirements, red tests, file evidence,
  and workspace copies.
- Group dependent edits together and merge independent edits with
  conflicts returned for another attempt.
- Reuse results when the workspace has not changed and rerun failing tests
  before the full suite.
- Preserve a prefix across a targeted repair rather than rebuilding
  every session's context.

This is real implementation history, not evidence that every component
improved success or cost. V6 put substantial suite-writing time on the
critical path; v7 successfully overlapped it. Internally green suites
could still miss the official answer.
Microluna is retained for reproducibility and was deprecated after
Microcoder replaced it.

### Microcoder: maintain explicit state between model actions

Microcoder simplified the loop around a structured next action: commands,
file views, replies, and a finish decision. Its standalone design includes
Jev-selected knowledge on each step, summary-to-full expansion, acceptance
written before edits, and optional stronger-model assistance. A host can
keep the useful state and discard unhelpful transcript detail before the
next call. See the [Microcoder guide][microcoder-guide].

That guide describes capabilities beyond the modern repository route.
Ordinary repository runs use `knowledge: None` for loop retrieval,
`acceptance: false`, `Route::Never` for stronger-model routing, and default
optional gates. The initial repository recipe has its own knowledge and
existing-check selection. Treating all standalone features as active in
the product would overstate what currently runs.

### Mechanisms worth separating

| Mechanism | Earlier implementation | Modern repository loop | Lean Claude/Codex sessions |
| --- | --- | --- | --- |
| Host survey and selected initial evidence | Coder One | Retained through recipe | Retained through recipe |
| Pack evidence against requirement coverage | Coverage packer | Modern recipe uses section packer | Same section packer |
| Initial knowledge selection | Coder One and later recipe | Retained when available | Retained when available |
| Reconsider knowledge after each action | Standalone Microcoder | Disabled | Absent from host loop |
| Own exact model history and file views | Microluna and Microcoder | Retained, now append-only | Native CLI owns them |
| Short specialists with requirement ownership | Microluna | No equivalent default dispatcher | One native session per turn |
| Isolated parallel edits and explicit merge | Microluna v7 | No equivalent default dispatcher | Not supplied by the host |
| Model-written acceptance before edits | Microluna and standalone Microcoder | Disabled | Disabled |
| Select and freeze existing failing checks | Modern recipe | Run after command steps | Run after session ends |
| Stop automatically when frozen checks pass | Controller/loop variants | Retained | Absent during native session |
| Lean native prompt/tools/cache | Coder One Claude policies | Claude loop uses zero native tools | Claude session retains six tools and 5-minute cache |

Here, “absent” refers to the OpenAgents host behavior. A native agent may
perform its own search, delegation, testing, or compaction; the host does
not obtain the old explicit specialist contract merely by invoking that CLI.

## 2. What currently runs

### Entry points and defaults

Modern desktop and CLI chat route typed offers into durable local tasks.
Issue work also enters the repository adapter. A process named
`microcoder` does not prove that the task uses the Microcoder loop: it can
dispatch a whole native session.

The shared provider preference starts with available Codex and then Claude.
At the snapshot, `CodexRuns::default()` is `Loop` and
`ClaudeRuns::default()` is `Session`. Codex Session exists but is opt-in.
Restricted-access and image-bearing tasks can take the loop even when a
session is selected. These conditions matter when labeling a measurement.
See [local admission][local-policy], [session settings][session-defaults],
and [repository dispatch][repository].

The older `coder` delegate door branches differently. Its in-process
Microcoder path builds an environment and task directly, without calling
the modern repository recipe, and disables knowledge and acceptance.
Its external CLI fallback has the older terminal recipe. A claim about
“Coder's context preparation” must name the entry point.
See [the delegate-door implementation][delegate-door].

### Initial context construction

There are several bounds before generation:

1. The shared chat delegation handoff is capped at 16 KiB and includes
   the latest request plus up to six prior turns. Separately, the task
   adapter retains up to eight previous turns with per-prompt/reply
   limits. These are different context sources, not one uniform window.
2. The recipe's task-class judgment sees at most 6,000 characters of the
   current request and 2,000 of earlier context. Tasks Jev classifies as
   questions skip the survey, knowledge selection, and check selection.
3. Other tasks run a read-only survey with Jev file judgments. The survey
   admits up to 40 files for consideration, with nominal content limits
   of 8,000 characters per file and 24,000 overall. Truncation annotations
   can add a little text. Setup/install probes are disabled on this path.
4. Knowledge search uses configured and workspace knowledge, optional
   embeddings or lexical fallback, then Jev selection among 12 candidates.
5. Check selection considers up to eight commands and retains at most
   two. The host freezes only those that fail without timing out before
   edits. A passing regression test is not retained as a deciding check.
6. `Briefing::build_under_knowing` fits the instruction and selected
   sections into a nominal 12,000-character cap. It reserves environment
   space, prioritizes flagged requirements and knowledge, and can omit
   whole evidence sections or clip the instruction tail. Frozen-check
   text is appended outside that cap.

Sources: [chat handoff][chat-handoff], [task context][task-context],
[recipe preparation][recipe], [survey implementation][survey], and
[briefing builder][briefing].

**Selection is not delivery.** The survey can read and judge more evidence
than the briefing has room to carry. The older coverage packer jointly
ranked evidence against requirements; the modern recipe calls the section
builder instead. No present measurement attributes saved rereads,
preserved requirement coverage, or reduced errors to that selection.

When no Jev client is configured, the recipe still wraps and caps the
request but has no judged class, knowledge, or checks. Disabling the recipe entirely
passes the original prompt through; it is a different treatment.

### The loop and sessions have different context contracts

This table describes the recipe-enabled path, with class-based effort
when Jev supplies a class. “Sessions” here means the lean Claude and Codex
adapters. Whole ACP agents have a separate in-flight check watcher.

| Property | Microcoder repository loop | Claude Session | Codex Session |
| --- | --- | --- | --- |
| Main execution | One structured action at a time | One native CLI session | One native CLI session |
| Task text | Briefing plus original current request | Capped briefing | Capped briefing |
| Frozen host instructions/context | Explicit serialized host context | Not explicitly passed as that snapshot | Not explicitly passed as that snapshot |
| History | Host file/state views and append-only transcript | Native session, resumed on later turns | Native session, resumed on later turns |
| Tools | Host executes model commands and reads | Bash, Read, Edit, Write, Glob, Grep | Native Codex tools |
| Model effort | Admitted effort; hard class can raise Codex to high | Medium for changes/hard, low for questions | Medium for changes/hard, low for questions |
| During-run control | Per-step Jev, stuck guard, host steering | Cancellation polling | Cancellation polling |
| Frozen checks | After command-bearing steps; can stop early | After native session | After native session |
| Authority | Commands go through host admission | Full-access admission; CLI bypass permissions | Full-access admission; CLI bypass sandbox |

Sources: [repository loop][repository], [native generation][native],
[Claude Session][claude-session], [Codex Session][codex-session], and
[shared session adapter][lean-session].

Three details are especially relevant to efficiency:

- **The loop duplicates some context.** The request can occur in the
  briefing, original instruction, and serialized host context. Frozen
  instructions can be large. Measure the contribution of each copy before
  blaming loop overhead solely on planning.
- **The session can lose task text that the loop preserves.** The briefing
  clips a long instruction; the session does not append the original
  current request separately. Native instruction discovery may read local
  files, but it does not establish equivalence to the exact admitted
  `host.context()` snapshot. This is a source-confirmed coverage risk;
  the retained seven-task study does not test it.
- **A small host prompt is not the complete billed prompt.** Native CLIs
  inject their own tools, configuration, instruction files, and history.
  Claude Session does not disable settings sources as the Claude loop
  does. Codex Session does not let the host specify a six-tool list or
  cache duration. Replacing the core prompt alone cannot establish equal
  input across modes.

The loop now uses [append-only prompt construction][transcript], which
helps preserve cached prefixes. It compacts after 600,000 bytes or a fixed
head change; the file view allows up to 12 files and 120,000 characters.
Command output keeps 6,000 characters from each end, and compact state
keeps the most recent 12 actions in detail. These are explicit context
controls still present in the loop.

### Constrained delegation needs a precise meaning

The earlier useful constraints included a bounded task, relevant evidence,
owned requirements, a small tool surface, a typed result, and independent
acceptance. Some also had execution limits.

Six tools do not enforce a file boundary: Bash can read or change broad
workspace state. Native session modes are admitted for full access and
use the CLI's permission/sandbox bypass. Prompting a specialist to touch
one file is not the same as enforcing that scope. A future specialist
experiment should distinguish:

- **Context scope:** what the specialist receives and can request.
- **Work scope:** the requirement and proposed paths it owns.
- **Authority:** what the host actually permits.
- **Acceptance:** what independently decides whether its output is useful.

The owner's current policy rejects arbitrary total-run dollar, step, and
time caps for ordinary product work. Operational command timeouts and
quiet-session guards remain. Recovering focused delegation does
not require restoring those caps. Experimental budgets and failure limits
can remain explicit properties of an offline study.

## 3. Independent head-to-head results

### Current standing study: 84 retained runs

The [standing rows][standing-rows] contain seven tasks, four arms, and
three trials, with no duplicate or missing keys in that planned grid.
All 84 pass their recorded independent check. They identify binary
`6b4dbe827d`, Claude Code `2.1.286`, and Codex CLI `0.159.2`.
These executions predate the audit source snapshot; source inspection
and measured behavior have different revision identities.

Four tasks are Terminal-Bench 2.1 tasks adapted to run on the host:
`fix-git`, `fix-code-vulnerability`, `headless-terminal`, and
`build-cython-ext`. Three are public repository fixes:
`mi-seekable`, `mi-one`, and `bottle-etag`.
This is a small localized-change panel, not an unchanged official
Terminal-Bench score or a long-horizon issue benchmark.

| Arm | Checked passes | Total estimated cost | Cost per checked pass | Sum of run times | Median run time |
| --- | ---: | ---: | ---: | ---: | ---: |
| Raw Claude | 21/21 | $5.663988 | $0.269714 | 1,111.07 s | 41.91 s |
| Raw Codex | 21/21 | $2.145817 | $0.102182 | 1,303.86 s | 37.80 s |
| Routed default, Codex loop | 21/21 | $1.790154 | $0.085245 | 2,111.00 s | 79.36 s |
| Lean Claude session | 21/21 | $3.551629 | $0.169125 | 1,490.34 s | 44.45 s |

Total retained estimated spend is **$13.151588**. Summed run times are
accumulated execution time; they are not the batch's elapsed time when
four runs execute concurrently.

The following ratios use the sum of per-task means. With three trials
per task, they also equal the ratio of arm totals. They are **not** the
ratio of the displayed medians or the mean of per-task ratios.

| Comparison | Cost ratio, independent 95% interval | Execution-time ratio, independent 95% interval |
| --- | --- | --- |
| Routed Codex / raw Codex | 0.8343 [0.7872, 0.8863] | 1.6190 [1.5174, 1.7289] |
| Lean Claude / raw Claude | 0.6271 [0.5609, 0.7103] | 1.3414 [1.1968, 1.5225] |
| Raw Codex / raw Claude | 0.3789 [0.3579, 0.4011] | 1.1735 [1.0879, 1.2662] |
| Routed Codex / raw Claude | 0.3161 [0.2993, 0.3343] | 1.9000 [1.7745, 2.0406] |

The script resamples repetitions within each fixed task 10,000 times,
using an independent implementation and seed. The intervals condition on
these seven tasks. They do not describe uncertainty over the population of
future issues.

A sensitivity calculation that also resamples task clusters widens the
routed-Codex cost interval to **[0.7457, 1.0017]** and the lean-Claude
time interval to **[0.8352, 1.9443]**. Seven selected tasks are too few to
turn that sensitivity calculation into a population guarantee.
Likewise, 21/21 has a naive independent-run Wilson interval of about
85%–100%; repeated tasks further limit that interpretation. Equal observed
passes do not prove equal reliability.

**Most of the current saving against raw Claude is already present in
raw Codex.** Pinning raw Codex to the routed arm's documented model and
effort defaults narrows the remaining saving to 16.6%. The effective routed
model and effort are not preserved in the rows. Even that comparison
changes context, tools, execution structure, and host behavior together.
The standing documentation's
claim that the arms differ “only in routing” is too strong.

### Task-level variation

Each ratio below divides means of three trials. Below 1 favors the
routed arm for that metric.

| Task | Codex cost ratio | Codex time ratio | Claude cost ratio | Claude time ratio |
| --- | ---: | ---: | ---: | ---: |
| `bottle-etag` | 0.969 | 1.915 | 0.511 | 0.719 |
| `build-cython-ext` | 0.794 | 1.421 | 0.914 | 2.053 |
| `fix-code-vulnerability` | 0.772 | 1.909 | 0.599 | 1.996 |
| `fix-git` | 1.469 | 3.436 | 0.591 | 1.465 |
| `headless-terminal` | 0.652 | 1.176 | 0.392 | 0.712 |
| `mi-one` | 0.901 | 1.480 | 0.586 | 1.915 |
| `mi-seekable` | 0.846 | 2.024 | 0.523 | 1.087 |

Routed Codex is cheaper on six tasks and slower on all seven. On
`fix-git` it is both 47% more expensive and 3.44 times as slow.
Lean Claude is cheaper on every task and slower on five. The large
`build-cython-ext` time increase contributes heavily to its aggregate.
Those differences argue for testing when preparation helps rather than
assuming every task benefits equally.

### Tokens, cache behavior, and costs

| Arm | Total input tokens including cache | Cache reads | Cache writes recorded | Output tokens | Recorded Jev plus embeddings |
| --- | ---: | ---: | ---: | ---: | ---: |
| Raw Claude | 4,964,609 | 4,571,178 | 393,067 | 80,188 | $0 |
| Raw Codex | 4,652,616 | 4,260,736 | Not separately reported | 50,991 | $0 |
| Routed Codex | 2,146,608 | 1,655,168 | Not separately reported | 42,624 | $0.050000 |
| Lean Claude | 3,196,905 | 2,913,733 | 282,808 | 76,825 | $0.016887 |

Claude input totals combine uncached input, cache creation, and cache
reads. Codex input totals already include cached input. A missing Codex
cache-write field does not mean the provider never writes a cache.

The loop cuts reported Codex input by about 54%, yet cache reads fall
from 91.6% to 77.1%. Its uncached input actually rises from 391,880 to
491,440 tokens. Lower output and fewer total cached tokens still reduce
the retained price estimate. **A shorter context is not automatically a
cheaper context.**

Lean Claude cuts total input by about 36%, preserves a roughly 91%
cache-read fraction, and reduces output only about 4%. This pattern is
consistent with saving on repeated input while retaining native session
caching. It does not identify whether the briefing, core prompt, tool
restriction, or another setting causes the improvement.

Direct Jev and embedding charges are about 2.8% of routed Codex cost
and 0.5% of lean Claude cost in this panel. Their decisions can still
have much larger effects by changing effort, preparation latency,
evidence delivery, and the number of expensive generation steps.

### Efficiency when someone is waiting

Across 21 runs, routed Codex saves $0.355663 and adds 807.14 summed
execution seconds: roughly **1.7 cents saved and 38.4 seconds added per
task**. Lean Claude saves $2.112358 and adds 379.27 seconds: roughly
**10.1 cents and 18.1 seconds per task**.

Dividing estimated savings by extra synchronous wait gives:

| Comparison | Descriptive break-even value per hour of extra waiting |
| --- | ---: |
| Routed Codex / raw Codex | $1.59/hour |
| Lean Claude / raw Claude | $20.05/hour |

This is an accounting illustration, not a valuation of the owner's time.
Background concurrency, subscription limits, machine cost, retries,
intervention, and actual invoices change the decision. The extra summed
seconds are not extra parallel batch elapsed time. Still, the illustration
shows why a token saving alone is an incomplete efficiency claim.

## 4. Historical evidence that changes the interpretation

### Matched Claude controller studies

The most relevant historical control is equally configured Claude, with
the same model, CLI version, medium effort, six tools, headless core
prompt, cache duration, and outer study limits. It asks whether the
controller adds value after those inexpensive configuration choices.

| Study and arm | Passes | Total cost including controller | Total agent minutes | Cost per pass |
| --- | ---: | ---: | ---: | ---: |
| Two-task pilot: plain Claude | 6/6 | $6.643490 | 37.03 | $1.107248 |
| Two-task pilot: controller | 5/6 | $6.136714 | 34.37 | $1.227343 |
| Ten-task study: plain Claude | 15/30 | $27.043859 | 198.89 | $1.802924 |
| Ten-task study: controller | 18/30 | $45.421416 | 437.23 | $2.523412 |

The pilot's 7.6% lower total spend comes with one fewer pass. Cost per
pass rises **10.8%**. A collection error required regrading one unchanged
plain-Claude candidate; excluding that whole pair is included in
`matched_controller.exclude_recovered_pair`. It does not justify
dropping the controller failure.

In the larger study, cost per pass rises **40.0%**, and agent time per
pass rises about **83%**. The controller costs more in 29 of 30 pairs.
Paired outcomes are 12 both pass, six controller-only passes, three
plain-only passes, and nine both fail. The exact McNemar p-value is
0.5078125; the observed pass gain is not strong evidence of a general
quality improvement.

The [targeted study][matched-targeted] attributes roughly $23.31,
about 52% of its Claude usage, to persistence. All 30 runs receive it.
One task, `mvcc-lsm-compaction`, improves from 0/3 to 3/3; another,
`legacy-utility-triage`, falls from 3/3 to 1/3. That suggests a targeted
continuation policy is worth investigating. It does not justify
continuing every task.

The independent script reads all 60 retained final Harbor outcomes and
the controller's usage records. One lost infrastructure attempt was
rerun, so 60 graded attempts do not represent all operational spend.
The matched study also uses selected historical tasks, which limits
generalization. See [the pilot][matched-pilot] for its smaller comparison.

### Evidence packaging: a useful narrow positive

The [coverage-packer study][packer-study] changes
`log-summary-date-ranges` from 0/3 to 3/3. Inspection of the retained
briefings explains a plausible mechanism: the old briefing drops the
needed record content while implying the evidence is complete; the new
briefing carries selected records, requirement coverage, and explicit
omissions.

Mean attempt cost rises from about $0.001864 to $0.003104 while all three
new attempts pass. This is a useful example of spending a little more
to deliver deciding evidence. It is not evidence that minimum input
size is the right objective.

The historical report describes other settings as fixed, but the actual
rendered guidance also changes: the new brief explains trimming and
expansion, whereas the old brief discourages rereading. The policy's
named directions setting stays the same; the delivered prompt does not.
The treatment therefore includes packing and guidance. This audit does
not attribute the whole gain to ranking bytes alone.

### Microluna: internal confidence was a weak completion signal

The [v6–v8 report][microluna-results] records cases where an early
officially correct implementation is changed to satisfy an incorrect
self-written test. V6 suite writing consumes substantial critical-path
time; v7 successfully overlaps it. Despite the implemented edit-lane
machinery, that report says no real task ran two edit sessions concurrently; merge behavior was exercised
by tests. It does not demonstrate a broad parallel-edit speedup.

The later [v18 family measurement][v18] contains 18 attempts and zero
official passes, including nine submitted candidates with full local
scores. Its formal protocol verdict remains inconclusive because of source
changes and retry-history deviations. The recorded known cost lower bound
is $0.682016; a separate conservative estimate is $0.963597. Neither produces a finite cost per
successful answer when there are no successes. Incomplete intermediate
candidate replay means the possible benefit of a perfect selector is
unknown for the whole cohort.

The reusable lesson is to bind a check to an exact candidate and preserve
missing or ambiguous evidence. The [truthful-check implementation][truth]
only attaches a later review when it is read-only and refers to the same
unchanged candidate. Typed finishes and frozen tests are useful records;
their types do not establish that their judgment is correct.

### Microcoder: cheap successes, weaker overall reliability

Independent inspection of 127 retained
`bench/terminal-bench/microcoder-runs/coderos-4080-tb21/*/summary.json`
files finds:

- 83 passes and 44 failures across 65 unique tasks.
- 30 tasks with at least two passes.
- Knowledge disabled in all 127 runs.
- $4.240156 known cost across 126 runs, with one cost unknown.
- Median successful-run cost about $0.007494 and time 109.12 seconds.

Including the known portion of the remaining run gives a cohort cost
lower bound of $4.383770; the complete total remains unknown.
Those are real cheap successes. However, the initial screen passes
31/65, about 48%; additional confirmations focus on screen successes.
The overall 83/127 is therefore success-selected and is not a full-suite
pass-rate estimate. The [reference result][tb21] is 299/325, about 92%,
under a different stronger-model setup. The observations cannot establish
that knowledge caused the wins, or isolate Jev's contribution.

For the harder TB4 studies, the retained [public report][tb4-oos] records
zero held-out passes across four rounds, including a later round after
the reasoning path was corrected. Complete raw collections for the later
rounds were not found in this frozen checkout. These are report-level
findings, distinguished from the independently counted TB2.1 summaries.

Similarly, only an unmatched subset of the original broad Coder One
TB4 comparison's episode directories is retained here. The script
labels them `tb4_retained_subset_only`. It does not reproduce or compare
that subset as the reported full 26-pair study.

### Knowledge does not yet establish compounding improvement

Knowledge retrieval, expansion, contribution, versioning, and provenance
are implemented. Earlier wins often use lessons from the same task
family. Full-body delivery does not ensure the executor applies the
method, and cheap harvesting does not establish useful transfer.

The current [knowledge evidence code][knowledge-evidence] deliberately
leaves historical screening inconclusive, refuses its automatic admission
as causal evidence, and produces no automatic admission/demotion proposals.
That is appropriate. A persistent knowledge base should be evaluated on
held-out tasks with exact version exposure, including retrieval misses,
irrelevant entries, and contamination from source tasks.

## 5. What the recent iterations actually establish

The [first shadow study][shadow] has 105 original rows plus 21 later
lean-Claude rows. The original recipe-on Claude loop costs 1.684 times
raw Claude and takes 1.394 times as long. Recipe-off Claude costs 1.602
times as much with one failed attempt. For Codex, recipe-on costs
1.611 times recipe-off and takes 1.427 times as long.

The recorded class assignments explain an important confound:
36/42 recipe-on tasks are labeled hard, changing effort along with
preparation. The result is evidence against that configuration on these
tasks; it does not isolate knowledge or briefing quality.

Caching provides another explanation. Raw Claude reads about 93% of
input from cache. The old Claude loop reads only 12%–15%, while repeatedly
creating cache entries. Fewer input tokens fail to translate into savings.
The [cache follow-up][cache-rows], after append-only prompt work, retains
nine passes per arm on three tasks:

| Configuration | Cost | Accumulated execution time | Cache-read share |
| --- | ---: | ---: | ---: |
| Raw Claude | $2.783573 | 595.08 s | 92.5% |
| Updated Claude loop | $2.242355 | 820.34 s | 82.6% |

That is 19.4% lower cost and 37.9% longer execution on a three-task
subset. It supports the importance of cache structure. It does not
establish a result across all seven tasks or hard issue work.

The later lean-Claude arm in the first shadow study costs 0.608 times
its earlier raw baseline and takes 1.091 times as long. Its fixed-task
time interval, roughly 0.83–1.43, includes both useful speedup and
substantial slowdown. Calling that equivalence would be incorrect.
It runs later, with changed code and class calibration, so it is also
a bundled intervention.

Keep these studies separate. They reuse tasks but use different revisions,
subsets, execution modes, and timings. Pooling them would make neither a
clean causal experiment nor a representative product sample.

## 6. Findings to resolve before making broader claims

### Context and control

| Finding at the snapshot | Consequence | Next concrete check |
| --- | --- | --- |
| The modern recipe uses section packing and can clip the instruction tail | Selected evidence and late requirements may never reach a native session | Put a deciding requirement after character 12,000; inspect exactly what each mode receives |
| Sessions omit explicit serialization of the frozen host context | Local instruction discovery may differ from the admitted instruction/knowledge snapshot | Use a frozen scoped instruction that ordinary CLI discovery cannot recover |
| The loop carries multiple copies of the request | Visible context savings can be offset by duplication | Count billed tokens by task, instructions, evidence, history, and tool schema |
| The survey can select more than the briefing can carry | Paid selection may be discarded before generation | Record selected, delivered, omitted, expanded, and reread evidence by requirement |
| The check prompt promises checks during execution, but sessions check afterward | The agent is given an inaccurate stopping contract; potential excess execution is unmeasured | Align text with the mode, then measure session completion versus first accepted candidate |
| Existing checks are selected from a narrow candidate set and only initially failing checks survive | No checks is not evidence that the task has no testable requirements | Distinguish no candidate, rejected, already passing, timed out, and frozen |
| Per-step knowledge, generated acceptance, and specialist dispatch are disabled or absent | Their historical benefit cannot be credited to current results | Treat each as a new measured intervention if reintroduced |

The frozen-check mismatch is direct: [`checks_section`][check-prompt]
says the host runs checks while work proceeds and ends the turn when they
pass. [`lean_session::turn`][lean-session] calls the CLI directly with
cancellation polling; [repository completion][repository-completion]
runs the frozen checks afterward. The route table already correctly
describes post-turn checks. Fix the actual prompt contract rather than
assuming the table is wrong.

Preparation also adds latency before generation: class judgment, survey,
knowledge selection, check selection, and initial check execution are
sequential outer phases, even where file judgments run in parallel.
Each selected check can take up to 300 seconds before the engine starts.
Measure those phases; the current aggregate cannot say which phase
causes a particular run's slowdown.

An initial nonzero exit can also reflect a missing dependency or broken
harness. It does not by itself prove that a check decides the requested
behavior. Keep that distinction in the frozen-check evidence.

### Accounting and retained evidence

| Finding | Evidence and limit |
| --- | --- |
| Native task totals omit recipe embedding charges | Recipe records `knowledge.search.embedding_usd`, but `Recipe::cost()` adds only Jev cost. This is a runtime-ledger finding. |
| The benchmark separately tries to reconstruct embeddings | `study.py` adds reconstructed embeddings into `jev_usd`. Do not automatically apply the runtime omission to its published totals. |
| Detailed preparation steps are built but not appended to repository ATIF | `Prepared.steps` contains individual records; repository preparation appends an aggregate summary. Some per-call latency and usage provenance is lost. |
| A failed resume can be retried, but only the final report populates the session total | Any billed first attempt needs separate accumulation. Source confirms a possible omission; the 84-row study does not measure its incidence. |
| Cancellation/refusal can lose usable partial accounting or previous-stage cost | Unknown/partial totals must survive all endings. A refusal is not evidence of a free attempt. |
| Reused session artifact paths overwrite earlier turn evidence | The task-only directory and fresh `delegate-1` filenames are opened with `File::create` on later turns. Summary ATIF survives; exact earlier native input/output may not. |
| Session tool counts mix event meanings | An edit can count as artifact change plus tool result; a read result can be labeled Bash. These counts do not support an accurate tool-efficiency comparison. |
| Native session wrapper copies reported total without the older delegate's partial/error classification | Cost provenance and completeness need explicit fields rather than inference from a number. |

Sources: [recipe records][recipe], [repository preparation and cost][repository-recipe],
[shared session adapter][lean-session], [CLI execution and charging][delegate-cli],
[stream normalization][stream], and [artifact retention][artifact-retainer].

These are source findings, not estimates that the published dollar amounts
are wrong by a known percentage. The current row totals balance
arithmetically; that cannot establish that every contributing call was
retained. The benchmark records $0.00009642 of embeddings for routed Codex
and $0.00003122 for lean Claude. Hosted chat-routing cost is outside its
reported estimate.

A complete ledger should distinguish a measured total, a known lower
bound, a priced estimate, and an unknown total. Missing usage must not
become zero. It must also retain successful, failed, cancelled, retried,
rate-limited, and fallback attempts.

### Standing harness and report

The current [harness][harness] is useful operational evidence, with an
independent check after each agent. It has limits that matter for the
claims built on it:

1. **Raw Claude is not configuration-matched.** The command does not pin
   model, effort, system prompt, tools, service tier, or settings sources.
   All current raw rows do report the same observed
   `claude-opus-5-5[1m]` model, so there is no observed raw-model mixture
   in this panel. Other controls remain unmatched.
2. **Raw Codex pins model and effort, not the entire configuration.**
   It requests `gpt-6.1-sol` at medium. The loop also changes tools,
   context structure, and execution. Requested model metadata is not the
   same as a returned provider identity.
3. **“Time to checked result” excludes the independent checker.**
   `one` stops the raw/routed execution timer before calling `check`.
   Setup and teardown are also outside it. Use “execution time” for these
   rows; record request-to-independent-acceptance time in the next study.
4. **Resume trusts existing files.** Settings are written only if absent;
   existing `result.json` rows are reused without checking a full
   binary/task/configuration digest. Changed code can silently reuse old
   artifacts under an unchanged run ID.
5. **An arm name can drift from behavior.** The optional `routed-claude`
   arm is described as a loop but does not explicitly set Claude mode to
   loop. The snapshot's default is Session. This is a latent rerun hazard;
   it does not relabel the older retained loop trials.
6. **Missing attempts can disappear.** Worker exceptions are printed,
   and collection reads existing result files. An expected-attempt
   manifest is needed to include failed preparation and missing results.
   The current 84-row planned grid is complete; the concern affects
   future or excluded attempts.
7. **Unknown source amounts can become zero.** Reconstruction initializes
   counters to zero and uses `or 0`. It also lacks a Codex Session usage
   branch at this snapshot. Do not measure that new mode through the
   collector before adding and verifying its accounting path.
8. **Published current evidence is aggregated.** The rows do not carry
   every native response and independent-check log needed to reconstruct
   each amount and verdict from first principles. This audit verifies
   row arithmetic and source behavior; it does not rerun those checks.
9. **A non-significant difference is not equivalence.** The report's
   “no measurable difference” wording means the interval crosses 1.
   It does not establish a bounded quality or latency difference.

The [report implementation][efficiency-report] calculates cost per pass
using all recorded attempt costs, which correctly includes failure cost.
Its timing median selects passing runs, and its comparisons resample
trials within fixed tasks. Label those different quantities explicitly.

The earlier shadow study also documents six discarded infrastructure/setup
attempts, checker corrections after inspecting outputs, and an unrun
positive oracle control for `build-cython-ext`. Corrections may be valid,
but they need versioned verdicts and retained original evidence. Excluding
operational failures without separately reporting their cost overstates
end-to-end efficiency.

## 7. A standalone head-to-head program

The next study should answer three questions separately:

1. How much comes from configuring a native agent efficiently?
2. Does host-built evidence improve that equally configured agent?
3. Does a host-controlled loop or scoped specialist improve accepted
   results enough to justify its extra work?

Keep this protocol and its results separate from the existing seven-task
dashboard work and the concurrent Codex Session implementation. Reuse
record schemas where helpful, but do not change an active study's arms
or overwrite its results. This audit runs no new experiments.

### Compare one mechanism at a time

Within each provider/model, use the following conceptual arms:

| Arm | What changes | Question answered |
| --- | --- | --- |
| Native defaults | Actual raw CLI configuration, fully recorded | What does a user get today? |
| Matched native baseline | Explicit model, effort, tools where supported, core prompt, cache policy, authority, and settings | How much of the saving comes from configuration? |
| Baseline plus current brief | Add only the current preparation and delivered evidence | Does the present recipe earn its cost? |
| Baseline plus coverage brief | Replace evidence packing, preserve full requirements, provide explicit omissions/expansion | Does evidence delivery improve over section packing? |
| Current host loop | Hold supported controls equal; change execution architecture explicitly | What does per-step host control add? |
| Scoped specialist treatment | Add one clearly owned subproblem with a typed handoff and independent acceptance | When does constrained delegation help? |

These are proposed comparisons, not six already-existing executable
arms. Some controls cannot be made identical across CLI and direct-loop
interfaces; record those differences and avoid claiming full isolation.
Compare providers only after the within-provider questions are understood.
Test smaller models as another factor rather than crediting their price
to the controller.

Start with instrumented development cases to verify the harness, then
freeze it before a fresh held-out evaluation. Size the held-out sample
around a declared acceptable quality loss and the desired detectable
cost/latency effect. More repeats of seven familiar tasks cannot substitute
for broader task coverage.

### Include tasks that exercise the missing mechanisms

| Task family | Why it is needed |
| --- | --- |
| Small localized fix | Measures preparation overhead and guards the existing cheap cases |
| Long issue with late requirements | Detects instruction loss and wrong summaries |
| Cross-crate behavior change | Tests file selection, consumers, and incomplete evidence |
| Multi-turn correction or resumed task | Tests handoff quality, preserved context, steering, and artifact identity |
| Debugging with misleading initial evidence | Tests selective expansion and repeated failed approaches |
| Several independent changes | Tests actual parallel savings, merge cost, and conflicting assumptions |
| Specialist fact needed, absent from prompt | Tests knowledge retrieval and transfer on tasks excluded from knowledge authoring |
| Partial completion, refusal, and interruption | Tests retry accounting, candidate preservation, and accepted-outcome cost |

Use task starts and independent checks that were not used to tune the
recipe. Exclude knowledge source tasks and exact fix descendants from
transfer claims. Separate task families in the report so easy successes
cannot hide regressions on long work.

### Make the specialist contract reviewable

A candidate handoff should contain:

- The exact requirement IDs and original instruction spans it owns.
- Relevant file regions and their revision identities.
- Established facts, command output, and uncertainties, each distinguished.
- The allowed action scope and the host's actual enforcement.
- Existing failing checks and regression checks, with candidate identity.
- A typed return containing changed paths, proposed diff, checks run,
  unresolved requirements, and evidence references.
- A way to request missing context without inventing facts.

The parent should integrate only against a known base and check the
combined candidate. File-disjoint edits may still share semantics or
interfaces. Start with read-only specialists or demonstrably independent
work; measure copy, merge, conflict, and recheck time.

Preserve native cached prefixes when possible. Do not repeatedly resend
large copied histories just to create a “small” subagent. Compare delivered
evidence and actual billed input, including each specialist and the parent.

### Measure the complete accepted outcome

Record an expected-attempt manifest before launch and one terminal record
for every attempt, including preparation failure. Bind each record to:

- Task, prompt, initial tree, independent checker, and knowledge digests.
- Product binary, provider, actual/requested model, CLI version, effort,
  tool definitions, prompt/configuration hashes, and execution mode.
- Cache scope, cache counts, fresh versus resumed state, and service tier.
- Every call's known cost or unknown status, every retry/fallback, and
  candidate snapshots before and after repair, review, and continuation.
- Host hardware/load, concurrency, randomized run order, and timestamps.

Use at least these outcome metrics:

| Metric | Definition |
| --- | --- |
| Accepted pass rate | Independent check on the submitted exact candidate, across every scheduled attempt |
| Cost per accepted result | All attempt costs divided by accepted results; disclose unknown totals and lower bounds |
| Request-to-acceptance latency | From admission/start through independent checking and required integration |
| Resource use | Sum of model time, machine time, and actual batch elapsed time, kept separate |
| Context efficiency | Bytes/tokens selected, delivered, cached, omitted, expanded, and reread |
| Human intervention | Count and time of corrections, approvals, manual repairs, and abandoned attempts |
| Continuation value | Accepted status before and after each repair, review, or persistence step |
| Knowledge value | Retrieval exposure and accepted outcome by exact entry version on eligible held-out tasks |

Split latency into route/admission, preparation, first useful action,
model work, tool execution, verification, repair, and final integration.
Measure stopping delay after the first independently acceptable candidate.
For completed work, reuse check results only when the candidate and relevant
environment are unchanged.

Report paired task outcomes and task-level uncertainty. Predeclare the
quality margin for accepting a cheaper configuration. If the study cannot
rule out the chosen regression margin, report insufficient evidence.
Do not substitute a non-significant test for equivalence. Publish the
tradeoff between success, cost, and latency rather than one winner
constructed from an unstated value of waiting.

## 8. Recommended order of work

1. **Make the evidence trustworthy.** Preserve every attempt's cost
   status, fix session artifact identity, record complete configuration,
   and include independent acceptance time. Add accounting for any
   newly measured execution mode before including it in comparisons.
2. **Make the context contract consistent.** Preserve the full current
   request and admitted instructions. Correct the frozen-check promise.
   Add fixtures for long requirements, resumed context, and the exact
   frozen instruction snapshot before performance experiments.
3. **Measure evidence delivery.** Compare the present section briefing
   with requirement-aware packing and explicit expansion, using an
   equally configured native session. Count selection work that gets
   discarded and evidence the executor rereads. Start with the
   [isolated briefing experiments](system-one-briefing.md#6-components-to-test-in-isolation):
   cached lexical retrieval, structural extraction, history, one semantic
   selection batch, and deterministic coverage packing. Most iterations
   can run without launching an executor.
4. **Preserve cache gains while removing avoidable work.** Measure
   duplicate request content, unnecessary surveys, repeated checks, and
   startup phases. Use revision identities to justify reuse. Small tasks
   may benefit most from doing less preparation.
5. **Try a narrow specialist intervention.** Give it one owned requirement
   or read-only investigation, track its full cost, and compare accepted
   results. Expand fan-out only after integration and outcome evidence
   support it.
6. **Keep expensive historical control experimental.** Broad persistence,
   repeated model-written suites, best-of-N selection, and automatic
   knowledge promotion require their own held-out evidence.

The immediate opportunity is better evidence delivery and measurement,
with native caching preserved. Move stable preparation work ahead of the
turn and test how much semantic interpretation software can reuse. The
repository already contains useful extraction, packing, and identity
machinery, but the inspected paths lack a persistent source-symbol index.
The retained evidence does
not yet establish that a general controller or small-model specialist
system beats equally configured Claude or Codex on representative issue
work.

## Reproduce this audit

From a checkout containing the recorded commit:

```sh
python3 docs/audits/2026-10-03-independent-efficiency/recompute.py . \
  --output /tmp/independent-efficiency-results.json
cmp docs/audits/2026-10-03-independent-efficiency/results.json \
  /tmp/independent-efficiency-results.json
```

The script uses Python's standard library and Git, reads pinned blobs,
and performs arithmetic and deterministic resampling. It executes no
candidate command, benchmark task, model call, or repository module.
The JSON records the seed, resample count, and hashes of its source files.
Changing `--commit` produces a new analysis; it does not update this
document's frozen claims.

Validation for this documentation change:

- Recompute retained figures independently and compare the deterministic
  output with the committed JSON.
- Check linked repository paths and pinned source references.
- Run `git diff --check` and review the documentation-only diff.

The original audit changed no Rust code and required no Cargo checks.
The later runnable prototype has its own targeted verification and
measurement record in the [System One extension](system-one-briefing.md).
The proposed agent comparisons remain follow-up work.

## Evidence retained with this audit

The [analysis script](recompute.py) and [result JSON](results.json)
cover the current and shadow studies, both matched-controller studies,
the development panel, coverage-packer examples, Microluna v18, and
Microcoder TB2.1. The JSON records 366 input hashes and raw-evidence paths.
Incomplete TB4 evidence stays labeled as such.

Source links throughout the document are pinned to the audit snapshot.
Historical study dates and binary identities remain those of the studies
themselves.

[snapshot]: https://github.com/OpenAgentsInc/openagents/tree/5e22f2af962f7e848df3aee1385e6cf165132123
[historical-cost]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/cost/2026-10-02-system-one-cost-efficiency-audit.md
[microluna]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/coder/design/microluna.md
[microluna-parallel]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/coder/design/microluna-parallel.md
[microcoder-guide]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/coder/guides/microcoder.md
[local-policy]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder/src/task/local.rs#L706
[session-defaults]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder/src/task/autostart.rs#L159
[repository]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder/src/repository.rs#L420
[delegate-door]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder/src/delegate_door/microcoder.rs#L535
[chat-handoff]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/openagents-chat/src/delegation.rs#L85
[task-context]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder/src/task/adapter.rs#L352
[recipe]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-delegate/src/recipe.rs#L348
[survey]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-delegate/src/judge.rs#L843
[briefing]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-delegate/src/delegate.rs#L746
[native]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder/src/repository/native.rs#L295
[claude-session]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder/src/repository/claude_session.rs#L49
[codex-session]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder/src/repository/codex_session.rs#L49
[lean-session]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder/src/repository/lean_session.rs#L171
[transcript]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder-loop/src/transcript.rs#L35
[standing-rows]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/bench/efficiency/results/2026-10-03.jsonl
[matched-targeted]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/terminal-bench/2026-09-23-matched-controller-targeted.md
[matched-pilot]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/terminal-bench/2026-09-23-matched-opus-controller.md
[packer-study]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/terminal-bench/2026-09-23-tunable-results.md#L21
[microluna-results]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/terminal-bench/2026-09-24-microluna-v6-v8-report.md
[v18]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/bench/terminal-bench/experiments/2026-09-25-microluna-v18-family/measurement.json
[truth]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-one/src/checks/truth_micro.rs#L39
[tb21]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/terminal-bench/2026-09-26-tb21-oos-results.md
[tb4-oos]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/terminal-bench/2026-09-26-out-of-sample-study-results.md
[knowledge-evidence]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/knowledge/src/evidence.rs#L275
[shadow]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/cost/2026-10-02-shadow-baseline-measurement.md
[cache-rows]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/docs/cost/2026-10-02-shadow-baseline/collected-10244.jsonl
[check-prompt]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-delegate/src/recipe.rs#L211
[repository-completion]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder/src/repository.rs#L1153
[repository-recipe]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/microcoder/src/repository/recipe.rs#L39
[delegate-cli]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-delegate/src/delegate.rs#L2386
[stream]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-delegate/src/stream.rs#L325
[artifact-retainer]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder-delegate/src/adapter.rs#L159
[harness]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/bench/efficiency/study.py#L384
[efficiency-report]: https://github.com/OpenAgentsInc/openagents/blob/5e22f2af962f7e848df3aee1385e6cf165132123/crates/coder/src/efficiency.rs#L193
