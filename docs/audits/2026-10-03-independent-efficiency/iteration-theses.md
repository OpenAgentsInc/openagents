# Testable theses for the next briefing iterations

Test whether preparation removes a specific obstacle to an accepted change. The six
mechanisms below can fail independently. Keep the current packer frozen while testing
them as separate treatments.

## Evidence and limits

The known [#10166 development preview][development] supplies the strongest concrete
retrieval example. Its 15,045-byte pack includes the complete fault declaration, two
specification sections, and a cache-target test. It misses the closer worktree test and
the selected test's `env` helper. Preparation took 231.668 ms with the supplied index;
creating a fresh index took 1,692.405 ms. These are individual observations. The
implementation passed 61 Boat tests and formatting; neither establishes executor
improvement.

The earlier [development executor audit][development-audit] reports four runs per arm on
this same known task: all first drafts failed the frozen acceptance check, and all eight
repaired drafts passed. The ratio of arm medians gives a **12.36% reduction in
CLI-reported list-price cost estimates**; the median paired reduction is 9.76%. This
misses the registered 20% cost-win target and concerns the earlier treatment. It
establishes neither a general benefit nor net engineering return on investment.

The [conversation audit][conversations] identifies repeated workspace-command mistakes,
missing prerequisites, cross-run log attribution, stale ownership, and useful
integration discovery. Its search counts, cached context sizes, and timestamp gaps are
not measured waste. Preserve useful exploration and existing session-cache benefits.

No Jev experiment ran for these proposals. No heldout or reserve outcomes, hidden
checker implementation, or model patches informed this document.

## 1. Select evidence for the behavioral boundary

**Evidence.** Complete syntax extraction recovered `removable`, but lexical ranking
selected `a_checkout_target_needs_cachedir_tag_and_git_ignore`. Both concern cleanup;
only the omitted worktree test directly exercises the relevant lifecycle. The
conversation audit also describes an initially unwired entrypoint that an agent
correctly discovered after receiving a plausible source briefing.

**Mechanism.** Code enumerates public entrypoints, source spans, existing tests, and
call relationships with their resolution limits. A semantic ranker judges how each
candidate bears on one stated behavior: trigger, precondition, protected state,
transition, or observable result. Code keeps explicit anchors first and copies selected
spans verbatim. TypeSafe's [reranking pattern][reranking] is a useful analogy; it cannot
retrieve a missing candidate.

**Isolated metric.** Label required behavioral boundaries from public requests and
pre-task source. Measure candidate recall first, then complete boundary coverage per
fixed 16 KiB pack and irrelevant bytes. Report each boundary separately, including
missing entrypoints and tests.

**Executor comparison.** Hold the candidate pool, byte limit, executor, and checks
constant; vary lexical versus semantic ranking only. Compare first-attempt acceptance
and total cost through the allowed repair. Test candidate expansion in a separate arm.

**Falsifier.** Better ranking fails to improve boundary coverage on unseen cases, or
better coverage adds enough preparation and context cost to erase any executor benefit.
Missing candidates falsify the retrieval stage, not the ranker alone.

## 2. Complete the selected test's fixture context

**Evidence.** The development pack contains whole declarations but omits `env` at
`tests.rs:74–87`. Its conservative shadowing rule misreads `let env = env(...)`: the
initializer calls the module helper before the new binding enters scope. The pack
reports the unresolved call.

**Mechanism.** Improve deterministic binding and scope handling before adding model
judgments. Track initializer scope, parameters, nested bindings, imports, helper cycles,
and type-associated calls. Keep unresolved cross-file, macro, and dispatch edges
explicit. Package a test and its supported fixture dependencies atomically; if they
exceed the budget, identify what remains unread.

**Isolated metric.** On synthetic and independently labeled source fixtures, measure
same-file helper recall, incorrect binding expansion, complete declaration rate, and
bundle bytes. Include genuine shadowing counterexamples; resolving every equal name
would increase recall while adding false dependencies.

**Executor comparison.** Fix the selected test and all other pack content. Vary only
fixture completion, under the same byte ceiling. Measure follow-up helper reads,
fixture-related compile failures, and accepted-result cost. Keep displaced evidence
visible when a larger fixture consumes the budget.

**Falsifier.** Binding fixes expand the wrong helper, or the executor still performs the
same reads with no acceptance or cost benefit. Complete fixtures that crowd out more
useful requirements also defeat this mechanism.

## 3. Establish the public acceptance contract before implementation

**Evidence.** The development audit separates two unsafe removal approvals from many
failures at a diagnostic-string assertion. Most failed cases stopped before later
content-integrity assertions. The task did not explicitly require that diagnostic
string. The conversation audit separately records a user clarifying that a plugin must
be created through the product flow; that later clarification was new intent.

**Mechanism.** Before execution, map each public requirement to its source and its
observable check. Distinguish behavior, diagnostic content, command success, and
required user journey. Flag unspecified expectations rather than inventing them. A
semantic judgment can match an assertion description to a public clause; code retains
the cited clause and records unsupported matches. Hidden checker assertions never become
briefing input. Keep the frozen historical verdict intact.

**Isolated metric.** Measure requirement-to-check coverage and unsupported requirement
insertion. Use cases where wording changes but behavior does not, and where a useful
error message accompanies unsafe behavior. A passing diagnostic must not count as a
passing state-integrity check; an early assertion failure leaves later checks unknown.

**Executor comparison.** Give both arms the complete public request and specification.
Add only a sourced acceptance map to one arm. Separate first-draft behavior failures,
diagnostic failures, and failures of the check infrastructure; retain the same final
acceptance rule and feedback in both arms.

**Falsifier.** The map merely leaks undisclosed expectations, turns later user steering
into an earlier requirement, or reduces diagnostic repairs without improving total cost
or reliable completion. Report such a narrower effect directly.

## 4. Cache tool semantics with validity conditions

**Evidence.** The conversation audit reports a corrected mobile `--manifest-path`
command followed by another invalid workspace `-p` invocation. Other tasks repeated
missing-compiler and missing-include failures. A shared log also made one agent read
another agent's results.

**Mechanism.** Cache a small command record: purpose, exact supported arguments,
workspace and tool identity, prerequisites, known failure category, working alternative,
and invalidation rule. Keep immutable source facts separate from fresh runtime
observations. For example, a Git status mode's treatment of ignored files is useful
command knowledge; the current checkout's cleanliness must be observed. Code enforces
run identity and exit status; cached prose cannot establish a live claim or passing
build.

**Isolated metric.** Replay valid and stale command records across workspace, tool,
include-path, and environment changes. Count invalid command choices avoided, stale
alternatives reused, wrong-run receipts rejected, and lookup latency. Include successful
cached reuse as well as required invalidation.

**Executor comparison.** Add these records without changing the source pack. Measure
known-invalid attempts, time to the first usable diagnostic, and complete task cost,
including record preparation and refresh. Preserve the baseline's prompt-cache policy.

**Falsifier.** Stale records cause regressions, refresh costs exceed avoided setup, or
repeated commands were necessary because their inputs changed. Lower command count alone
is insufficient.

## 5. Route checked work through a cheaper executor

**Evidence.** The known development panel needed repair in both arms; prompt reduction
alone did not remove that stage. Targeted reruns in the conversation audit exposed real
compile and integration failures. Measure the executor and checking system together.

**Mechanism.** Try a cheaper executor with the same bounded evidence, then run
independent deterministic checks. Escalate failed or unresolved work with the exact
failure receipt and retained attempt state. Initially, route by observed checks rather
than an uncalibrated prediction of task difficulty. TypeSafe's [extraction
cascade][cascade] motivates this architecture, but its extraction results do not
establish coding performance. Semantic failure classification can be a later, separately
measured routing component.

**Isolated metric.** Test the checks' false-pass and false-failure rates on separately
authored positive and negative patches, plus attribution to the correct revision and
environment. Measure routing decisions against observed check outcomes and count
unnecessary escalations. A test pass covers only the tested contract.

**Executor comparison.** Start with executor choice crossed with briefing presence,
keeping tools, public requirements, check feedback, and total repair budgets fixed. Then
compare a cheap-first cascade with a direct stronger executor. Charge failed cheap
attempts, checks, handoff context, and escalation to the cascade.

**Falsifier.** False passes increase, most tasks require escalation, or the cascade's
accepted-result cost or tail latency exceeds the direct executor. A lower first-call
bill does not establish this thesis.

## 6. Request a targeted read or abstain from a misleading brief

**Evidence.** The first structured development preview spent 8,020 of 16,223 bytes on
non-source content and admitted no test. The corrected pack still knows that a fixture
is missing. In the conversation audit, a real historical study initially answered the
wrong question; later discovery of an unwired entrypoint was useful.

**Mechanism.** Return explicit coverage gaps with source pointers. Choose among bounded
next reads that can resolve a named gap, or provide no optional evidence when the
candidate set cannot support a useful brief. The executor keeps its normal read tools
and complete instructions. A semantic selector may nominate the next candidate; code
fetches its exact current bytes and reassesses coverage. New evidence requires a new
judgment, not an answer inferred from an earlier batch.

**Isolated metric.** Measure required evidence recovered per additional read and byte,
false claims of completeness, and abstention coverage versus error rate. Include cases
where another read changes nothing. Fix the total context and read budgets.

**Executor comparison.** Compare an eager fixed pack, the same pack with one bounded
follow-up opportunity, and a no-pack baseline. Measure acceptance, duplicate reads,
missed constraints, and total latency and cost. An abstention is useful only if the
fallback still completes the task efficiently.

**Falsifier.** The policy abstains on useful evidence, loops through low-value reads, or
saves preparation while moving more work onto the executor. Suppressing all briefs or
reads must not score as success.

## Where typed judgments fit

The retained TypeSafe talks motivate composable decisions in software: the [June talk at
25:33][june] discusses going beyond strings; the [July talk at 11:29–13:11][july] argues
for richer software primitives; and the [September interview at 15:46–16:34][september]
emphasizes dependable automation. These motivate the design; they do not measure
briefing effectiveness. The [TypeSafe skill][skill] and live docs, checked October 3,
2026, define the question semantics.

State contains the complete public request, sourced requirements, candidate spans, known
syntax edges, and gaps. Code owns hashes, resolution, budgets, fetching, and execution.
Measure deterministic baselines before these semantic judgments:

| Judgment | Proposed primitive and meaning | Code's use |
| --- | --- | --- |
| Candidate relevance to one required behavior | **Score**, with standalone levels: discusses another behavior; describes related setup without the requested transition; directly describes or tests the requested precondition, transition, or result. | Rank candidates on the same scale, then enforce the byte budget. |
| Public support for one proposed acceptance clause | **Noul**: does the cited public text require this particular observable condition? A related topic or plausible improvement does not count as support. | Preserve the probability and source; flag unsupported additions. |
| Evidence availability for one gap | **Noul**: does the current candidate set contain evidence that resolves the named question? | Evaluate whether another read or abstention helps, using calibrated policy. |

A [Score][score] is a probability-weighted position on its stated levels. A [Noul][noul]
returns the probability of yes, with no separate confidence field. Neither is a
probability that the whole patch is correct. Retain the full returned distributions
where provided; [confidence][confidence] summarizes concentration, not correctness or
authorization. Calibrate thresholds and operating points on labeled development cases,
then freeze them before independent evaluation. No threshold is proposed here.

Ask independent candidate and requirement questions together over the same available
state. Put each target and meaning in the instructions; IDs are routing keys. State
speculative premises explicitly and ignore unused answers. Batched questions cannot
consume one another's answers. Measure extra tokens and end-to-end latency. This follows
TypeSafe's [fan-out pattern][fanout].

## Experiment order and accounting

Start with public acceptance maps, binding fixtures, and command-record replay tests.
Then test ranking and bounded reads on labeled source cases. Run matched executor trials
only when the isolated component improves its intended metric. Include negative cases
and successful baseline behavior; blocking everything must not score as success.

For every executor comparison, pin source, issue, instructions, tools, environment,
model settings, and repair feedback. Freeze the treatment before selecting evaluation
tasks; keep post-task fixes and outcomes out of preparation. Report acceptance and false
passes before cost, alongside all attempted runs. Include preprocessing, refresh, cache
creation and reads, model calls, checks, failed attempts, and repairs. Report
fresh-index and warm-preview costs separately, including any amortization assumption.
Engineering effort remains a separate unmeasured cost here; none of these theses
establishes net engineering ROI.

[development]: briefing-model-factorial/development-preparation-final/README.md
[development-audit]: briefing-iteration-2/development-audit.json
[conversations]: conversation-briefing-audit.md
[june]: ../../research/typesafe/2026-06-19-ai-council-diogo-almeida-transcript.md
[july]: ../../research/typesafe/2026-07-31-ai-engineer-diogo-almeida-transcript.md
[september]: ../../research/typesafe/2026-09-28-a16z-jev-transcript.md
[skill]: ../../../.agents/skills/typesafe-ai/SKILL.md
[reranking]: https://docs.typesafe.ai/cookbooks/rerank_typesafe
[cascade]: https://docs.typesafe.ai/cookbooks/sde_cascade
[score]: https://docs.typesafe.ai/primitives/score
[noul]: https://docs.typesafe.ai/primitives/noul
[confidence]: https://docs.typesafe.ai/confidence
[fanout]: https://docs.typesafe.ai/patterns/fan-out
