# Model selection and structured briefings

**Sonnet without a brief costs 47.1% less than Opus on this task, with 4/4
accepted patches in each arm and lower cost in every matched block.** Its
median recorded endpoint is 5.8% faster. This passes the cost-win rule fixed
before execution. All four Sonnet controls also pass the separate retrospective
deep-import diagnostic. **Use Sonnet with the common verification-and-repair
loop as the candidate for broader evaluation.**

The prepared brief passes its own registered cost comparison on Sonnet:
34.7% lower median estimated cost, cheaper in 3/4 blocks, with 4/4 final
acceptance. A separate diagnostic confirms that one briefed candidate misses
a notification for a deeper, catalog-visible import. The original checker
misses that defect. The briefing has a measured cost benefit on this task,
but this version needs better evidence coverage and quality checks before
becoming a default.

| Arm | Configuration | Median CLI estimate | Median recorded endpoint | First-attempt acceptance | Final acceptance |
| --- | --- | ---: | ---: | ---: | ---: |
| A | Opus control | $0.4533 | 121.78 s | 3/4 | 4/4 |
| B | Opus with brief | $0.5421 | 130.47 s | 4/4 | 4/4 |
| C | Sonnet control | $0.2399 | 114.67 s | 1/4 | 4/4 |
| D | Sonnet with brief | $0.1567 | 108.75 s | 0/4 | 4/4 |

Costs include the allowed repair and count the final cumulative CLI estimate
once. They are list-price estimates, not verified bills. First-attempt
acceptance includes the common automatic formatting step. The original
create-and-write regression passes every first draft; the additional atomic
import cases explain the lower first-attempt acceptance. The recorded endpoint
includes preparation and checks but excludes final artifact capture and
cleanup. Full timing definitions follow below.

The effective panel uses four balanced blocks of the same historical issue,
with exact model IDs, medium effort, complete common instructions, five file
tools, and the same external checks and one-repair limit. The
[prospective protocol](plan.md) fixes the thresholds and order. A separately
registered infrastructure replacement, described below, preserves the first
three blocks and replaces the entire fourth block.

Read the [complete results](results/README.md), [machine-readable metrics](results/metrics.json),
[arm summaries](arm-summary.json), and [exact prepared brief](task/treatment.md).
The [independent final audit](independent-audit-final.json) reproduces all five
comparisons, the arm summaries, replacement lineage, and cumulative accounting.
Four repetitions of one selected issue establish a bounded engineering result;
they do not estimate performance across arbitrary issue work.

## All planned comparisons

| Candidate versus reference | Median cost change | Median recorded endpoint change | Cheaper blocks | Registered cost gate |
| --- | ---: | ---: | ---: | --- |
| Sonnet control versus Opus control | 47.1% lower | 5.8% faster | 4/4 | Pass |
| Briefed Sonnet versus Opus control | 65.4% lower | 10.7% faster | 4/4 | Pass |
| Briefed Sonnet versus Sonnet control | 34.7% lower | 5.2% faster | 3/4 | Pass |
| Briefed Opus versus Opus control | 19.6% higher | 7.1% slower | 1/4 | Not met |
| Briefed Sonnet versus briefed Opus | 71.1% lower | 16.6% faster | 4/4 | Pass |

The same brief has different observed effects on the two models. Opus with a
brief avoids repair in all four runs but costs more. Sonnet with a brief uses
fewer tools and costs less, while every first draft needs the additional
import feedback. This panel supports a cost comparison under that repair
rule; it does not establish a statistically reliable model-by-brief interaction
or improved first-draft correctness from briefing.

## Quality beyond the frozen checker

The [effective source and tool review](quality-review-effective.md) covers
all 16 final patches and 24 captured attempts. No existing assertion is
weakened or removed. All 213 recorded tool requests stay inside their assigned
workspaces. These observations concern recorded requests, rather than a full
operating-system access audit.

The final briefed Sonnet patch in run 13 adds a directory scan that stops at
depth four without sending a fallback notification. The catalog reader accepts
deeper paths. A [retrospective public-API diagnostic](posthoc-depth/README.md)
imports a completed directory tree atomically, saves the notification result
under the existing four-second deadline, then performs a fresh catalog read.
That read finds the chat, but the saved notification wait times out for run 13.
The historical fix and all eight effective Opus and Sonnet control patches
pass the same test. Each revision runs once; no model reruns or repairs follow.

This diagnostic was selected after inspecting run 13. It changes no frozen
acceptance score and provides no new latency comparison. It establishes one
concrete defect missed by those checks. The remaining briefed candidates were
not run through this diagnostic, so it cannot estimate a briefing failure rate.
Source review also identifies unexecuted entry-limit, total-work, and symlink
concerns in run 13; the review distinguishes them from the confirmed failure.

The control model comparison survives this additional check: all four runs
in each arm pass the frozen checks and the deeper-import diagnostic. That makes
Sonnet control the clearest result to carry forward. It still needs evaluation
on other tasks before becoming a general routing rule.

## Original panel and replacement

The original 16-run panel is **invalid for its registered comparison**: the
final run initialized Claude CLI 2.1.288 instead of the required 2.1.287. The
coordinator retained the result and stopped. Its
[complete report](results-original/README.md) retains every patch, check, and
cost. Calibration and the original registration were published before warmups
and scored calls.

The protocol permits replacing a whole block after a confirmed infrastructure
defect. A [separate prospective registration](replacement-registration.json)
and [amendment](replacement-block-4.md) replace all four positions in block 4
with a privately pinned 2.1.287 executable. The first three blocks
and original warmups remain unchanged. The original block stays visible and
charged; no outcome from it enters the replacement comparison.

The [original source review](quality-review-original.md) also identifies an
unexecuted concern in the last candidate: its missing, extensionless rename
path heuristic can classify an ordinary renamed file as a directory. This
case is outside the frozen checker's coverage. It remains visible alongside
the version failure. The replacement is triggered by the registered version
rule, and its checker remains unchanged.
The [original independent numerical audit](independent-audit-original.json) confirms
complete original accounting and rejects all five original cost gates while
the CLI binding is invalid.

The replacement registration and helper were published in
[`54c2f8763047`](https://github.com/OpenAgentsInc/openagents/commit/54c2f8763047a20810a7e4b791dca0831d3856c5)
before replacement execution. The [completion receipt](replacement-complete.json)
and [eight executable checks](replacement-executable-checks.jsonl) confirm
that all four new arms finish under the registered binary and version.
The extra wrapper costs 3.949 seconds across the four runs, outside the frozen
endpoint formula. The receipt also records the 574.728-second replacement
batch elapsed time, including each coordinator's final capture and cleanup.
The earlier runs establish observed CLI version only; their executable bytes
were not hashed at launch.

## What changed in source discovery

The descriptive [arm summaries](arm-summary.json) show the expected opportunity
and its limits:

| Median per run | Opus control | Opus with brief | Sonnet control | Sonnet with brief |
| --- | ---: | ---: | ---: | ---: |
| Tool calls | 16 | 17.5 | 13.5 | 7.5 |
| Explicit file reads | 4.5 | 4.5 | 4 | 2 |
| Search calls | 5.5 | 6.5 | 4.5 | 1.5 |
| Agent phase | 79.72 s | 83.98 s | 48.76 s | 40.83 s |
| External checks | 31.62 s | 27.27 s | 53.21 s | 60.72 s |

Component medians do not add to the median total. The agent phase includes
provider waiting, generation, and local tools; it is not pure model compute.
Faster, less expensive generation makes verification a larger part of this
workflow. Preserving a warm build target and reusing valid setup facts are
separate opportunities from shortening the prompt.

On Sonnet, median reported output tokens fall from 5,196.5 to 3,546; cache
creation tokens fall from 22,928 to 15,890; cached input reads fall from
478,025.5 to 288,451. These counts are consistent with less repeated discovery,
but they do not identify which snippet caused the change. Provider cache state
remains uncontrolled, and output already includes any thinking tokens the CLI
reports; those must not be added again.

Before the first completion, every control and every briefed Opus run explicitly
reads the watcher file. No briefed Sonnet run does. All 16 runs read the actual
notification test file, which the supplied brief fails to select. Including
repair, 15/16 read the watcher. These recorded patterns suggest that Sonnet
initially substitutes supplied source for some exploration, while Opus still
checks the file. They do not prove internal use of snippets or explain which
read caused a better patch.

## Cost of finding the result

The [experiment ledger](../experiment-costs.json) retains **$25.4494** in known
executor/probe CLI estimates across 40 scored sessions, four shared warmups,
two availability probes, and one two-input accounting smoke. It includes
failed candidates, repairs, the invalid original block, and its replacement.
Copied records add no new charge.

| Work | Recorded CLI estimate |
| --- | ---: |
| First replay, four scored sessions | $2.7749 |
| Second round, development with warmup | $6.8368 |
| Second round, held-out with warmup | $8.3234 |
| This round, original 16, replacement four, and warmups | $7.4850 |
| Availability and accounting probes | $0.0293 |

The original block 4 costs $1.719421 and remains in this ledger. The earlier
rounds failed their acceptance or cost targets; their outcomes are never
pooled with the successful comparisons. The program stops after this fixed
panel rather than extending a sample until its threshold passes. Model usage
for this audit and its orchestration, engineering time, machine charges, and
original historical conversation charges are unmeasured. This ledger is not
the total cost of the research or evidence of net engineering return.

## Why this follows the previous rounds

The first replay produced no accepted patch. The second round supplied full
instructions and an external verification-and-repair loop; its development
panel accepted every patch but did not reach the registered 20% cost saving.
Its unchanged held-out panel also misses the cost gate, with 12.6% lower
median estimated cost and 7.5% lower recorded endpoint time. Those observations
remain published separately, including broader quality concerns that the
frozen checker does not cover.

The new thesis is that a less expensive executor can complete a bounded issue
under the same deterministic checks. The structured brief may remove some
source discovery, but its contribution must be measured against Sonnet alone.
The four arms distinguish these explanations. A combined win does not by
itself establish that briefing helped.

## Historical task

The reserve is [issue #9989](https://github.com/OpenAgentsInc/openagents/issues/9989):
make direct catalog notifications work when a new transcript directory appears
on Linux. The source is `aeb7f9fb19e0d56c13400702894172ae472e3bcf`; the original
fix is `fd305b01df5add38e2bd0ef3f862ee85cc9950c3`, whose parent is that exact
source. The retained fresh child assignment took 312.434 seconds. Its tools,
coordination, and cross-platform duties differ from this experiment, so that
historical elapsed time is context, not a matched control.

The [display copy of the task](task/task.md) removes owner paths, a hostname,
and historical model attribution. Its provenance record hashes both versions.
All scored arms receive the same complete private task and applicable
instructions. Original solutions and independent checks stay outside their
source exports. The operational override uses file tools and isolated Linux
checks; this panel does not fulfill the original request for repeated macOS
checks or deployment.

## Frozen preparation

The packer was frozen before its implementer saw the reserve identity or
checker. It ran once on the task under its registered rules. The resulting
[brief](task/treatment.md) is 14,025 bytes, including 12,232 bytes of source.
Warm preview took 0.220 seconds; building a fresh index separately took 1.425
seconds. These are single samples with operating-system caches uncontrolled,
not latency percentiles or a guarantee for arbitrary GitHub issues.

The brief contains the watcher subscription, delivery, and relevance logic,
with bounded same-file syntax dependencies. It selects a host-policy test as
a lexical fallback, rather than the direct-notification regression requested
by the issue. It also cannot resolve every fixture or call. Those gaps remain
in the treatment. No solution, checker hint, extra model call, or handpicked
source appendix was added after the freeze.

## Acceptance and limits

The checker uses public client, host, catalog, and direct-connection APIs with
synthetic keys and temporary state. Five Linux integration cases cover an
imported day directory, an imported nested tree, unrelated files and sibling
roots, notification before the first catalog read, and an ordinary new chat
inside an existing directory. Atomic imports expose the missing directory
event without depending on the child-watcher registration race. Ordinary
crate tests and formatting run as well, with one pretrial exclusion described
below.

A prospectively recorded amendment replaced the original private-symbol
checker with these public-API tests. This admits internal refactoring and
avoids scoring by resemblance to the original patch. Imported directories
extend the original create-and-write scenario; that choice is disclosed before
model execution. Four-second positive bounds and 300 ms negative observations
are inherited from the existing test and do not establish notification latency.

The checks exercise operating-system notifications and loopback transport.
Repeated [calibration](calibration/README.md) gives the expected result in
11/11 observations for each revision: the base passes 3/5 cases and the original
fix passes 5/5. This measures whether this particular environment reproduces
the bug and accepts the historical fix; it does not make those tests deterministic
or prove complete correctness. Candidate patches also receive an independent
source review. Registered acceptance and any broader concerns are reported
separately.

## Historical test limitation

Calibration also exposed an unrelated, environment-sensitive test failure in
`tests::strict_destinations_schemas_and_private_store_permissions`. It changes
`observer.json` from mode `0600` to `0644` and expects the cached store to reject
it. The historical cache observes ctime but does not include mode in its stamp.
On this sandbox, ten immediate permission changes preserve the same observed
ctime even though the mode changes. The test fails in one full base run, then
passes three isolated runs; a single unchanged full reference gate also passes.
Those observations remain in the calibration evidence.

A prospective amendment excludes only that named test from the ordinary crate
gate in every arm. All other crate tests, formatting, and the five independent
notification checks remain unchanged. A separately named reserve verifier
records this difference; the round-2 verifier is untouched. No scored model
call precedes the amendment. This removes an unrelated source of flaky repair
feedback; it does not establish the omitted permission behavior as correct.

## Transport amendment before execution

Two calibration status reads received an HTTP 502. Each result was recovered
from the original process without relaunching it. The
[transport amendment](transport-amendment.json) binds a separate runner copy
that retries only GET responses with status 502, 503, or 504: at most three
attempts, with one- and two-second backoffs. Command launches, writes, and model
calls are never automatically retried. Sanitized retry events and elapsed time
remain in the results, and retry time stays inside the existing endpoint timer.

The [exact runner difference](transport-runner.diff) and 36 passing synthetic
tests cover the retry behavior, reporting, and unchanged coordinator. This
prospective amendment supersedes only the protocol's reference to the original
runner copy. The models, task, briefing, checks, repair rule, order, and win
thresholds remain unchanged. The original runner remains available for replay.

## Reading the result

Arm A is Opus without a brief; B is Opus with the brief; C is Sonnet without
a brief; D is Sonnet with the brief. The primary comparison is D versus A.
The same predeclared cost gate also evaluates C versus A for model selection
and D versus C for the brief's contribution on Sonnet. B versus A and D versus
B expose the other paired differences.

A cost win requires both compared arms to pass 4/4 runs, at least 20% lower
median CLI cost, lower cost in at least three of four blocks, and no more than
10% higher median recorded endpoint time. Every failure and repair remains in
the fixed panel. Each cost uses the final cumulative CLI estimate once. The
estimate is not a verified bill or subscription charge. Warmups and cold
indexing remain separate; machine and engineering costs are unmeasured.

Four repetitions of one reserved issue support a narrow engineering result.
They do not establish general model superiority, production routing policy,
or a net return on the engineering work required to build this prototype.

## What this tests in our tooling

Both executors are native Claude CLI sessions. Our experimental contribution
is the Rust source packer and the external, bounded verification-and-repair
loop. The optional source pack is the only difference within each model pair.
The shared checks let the experiment compare models at the same acceptance
boundary. No TypeSafe judgment, learned router, escalation cascade, or change
to Coder's default execution path runs in this panel.

The practical candidate is a small workflow: preserve complete instructions,
use the less expensive executor, verify the patch independently, and permit
one repair with the actual failures. Treat verification as part of its cost
and behavior. Test improved source preparation as a separate addition; the
current briefing's extra savings come with a confirmed quality gap. This
experiment cannot justify removing checks or sending every kind of issue to
the same model.

The briefing still selects the wrong nearby test, despite receiving the exact
regression name in the task. Syntax-complete declarations help preserve local
code, but they do not establish coverage of the requested behavior. The
executor's additional reads and the checker's feedback remain useful work.
The next preparation changes should earn their place through small component
experiments before another paid executor panel.

## Next isolated experiments

These proposals follow the recorded gaps. They are untested and change none
of this panel's frozen inputs. The broader [iteration theses](../iteration-theses.md)
remain the prospective design record.

| Component | Small experiment and independent oracle | Measure before another executor trial |
| --- | --- | --- |
| Resolve the requested test | Give the index exact test names and historical path variants. Add tiny Rust fixtures with duplicate function names, import aliases, and one or two caller hops, with a separately authored expected-link table. Keep unresolved trait and macro calls explicit. | Exact-name recall, false canonical links, top-one selection, and bytes. Static reachability alone cannot establish behavioral relevance. |
| Complete fixture bindings | Compare the current declaration-wide shadowing set with call-site scope intervals. Cover `let helper = helper()`, earlier and later bindings, parameters, nested blocks, genuine shadowing, and cycles against a separate expected-resolution table. | Correct and incorrect dependencies, complete fixture bundles before and after the byte budget, and added preview time. Hold test selection fixed. |
| Include connected behavior limits | Build tiny producer/reader pairs with different supported depths, entry limits, and truncation behavior. Ask the deterministic index to retrieve both ends, then optionally ask a typed judgment whether the proposed bound preserves the stated behavior. Grade against separately authored boundary cases. | Recall of linked limits, missed truncation fallbacks, irrelevant bytes, and preparation time. Test retrieval separately from the judgment; a syntax dependency does not imply a behavioral dependency. |
| Reuse environment facts safely | Replay synthetic manifest and filesystem changes: nested workspaces, missing tools/includes, changed PATH precedence, updated launchers, symlink targets, modes, and another run's receipt. Compare recomputation, blind reuse, and reuse with required refreshes. | Stale positive reuse, rejected wrong-run receipts, correct cache hits, filesystem operations, and cold/warm latency. Metadata presence does not prove executable version or usability. |

Start with the binding fixtures because their expected result is narrow and
precise. Measure command and environment reuse on a scripted timeline, then
expand task-to-test resolution and connected behavior limits. Report failures
as well as successes, including replacements with unchanged size and mtime
that the current metadata key cannot detect.
Measure preview latency across fixed repository sizes; this panel's one warm
sample cannot establish a subsecond guarantee for arbitrary issue fetches.

After these component tests, freeze any improved packer and evaluate it on a
new task set. Keep model choice and briefing separate again. Add a TypeSafe
relevance judgment only when the deterministic candidate pool contains the
needed evidence; charge its preparation time and usage to the treatment. No
new confidence threshold or expected saving follows from this experiment.
