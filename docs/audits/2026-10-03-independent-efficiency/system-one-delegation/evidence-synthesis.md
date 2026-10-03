# What supports the efficiency thesis

Recorded October 3, 2026. Tracking:
[#10356](https://github.com/OpenAgentsInc/openagents/issues/10356).

**The evidence supports lean delegation and selective preparation. It does
not yet establish that adding System One to an otherwise identical lean
native agent improves cost, elapsed time, and correctness together.** The
useful thesis is conditional: compute facts cheaply, use a small semantic
judgment where it changes a decision, preserve necessary evidence, and hand
a bounded task to the least expensive executor that passes independent
checks. Every extra judgment, context byte, and control step must repay its
cost in the complete workflow.

This synthesis combines the independently recomputed audit results with
explicitly identified historical reports. It does not pool unlike trials or
treat multiple reports of the same runs as new evidence. The new native
panel has **zero scored sessions** while funded Jev access is unavailable.

## Evidence by claim

| Claim | Evidence | What the evidence establishes |
| --- | --- | --- |
| A tighter product configuration can beat a bare harness. | The [latest standing study](../claude-startup-followup.md) has 84 runs, seven tasks, and 21/21 passes per arm. Lean Claude costs 0.4488 times raw Claude and takes 0.8587 times its recorded time. | A substantial observed package benefit on familiar small tasks. Prompts, tools, preparation, and settings change together; some preparation costs and final independent checking time are outside the comparison. |
| Routing is consistently faster across engines. | In the same [standing study](../claude-startup-followup.md), routed Codex costs 0.8518 times raw Codex and takes 1.6500 times its recorded time. Relative to raw Claude, its ratios are 0.3160 for cost and 1.8322 for time. | Unsupported. The observed Codex configuration trades time for cost. A Claude configuration win does not establish a Codex win on both measures. |
| A cheaper executor can retain measured quality. | The [independent factorial](../briefing-model-factorial/README.md) finds Sonnet control 47.1% cheaper and 5.8% faster by median than Opus control; both pass 4/4 frozen acceptances and all eight controls pass the later deeper-import check. | A bounded model-choice win on one historical task, using common file tools and a verification-and-repair policy. This is not a full native-harness result or a Jev result. |
| Better context can lower recorded cost and time. | The [second briefing iteration](../briefing-iteration-2/README.md) has all final candidates passing frozen checks, with 12.6% lower median cost and 7.5% lower median time on its held-out task. The [historical version arc](../../../terminal-bench/2026-09-24-version-arc.md) reports a log-summary context repair changing Luna from 0/3 to 3/3. | Promising bounded evidence. The first misses its 20% cost threshold, increases median tool calls and reads, and its [later diagnostic](../briefing-iteration-2/posthoc-execution.md) confirms order-dependent defects in three of eight final patches. It does not establish reduced discovery or complete correctness. The historical change also changes rendered guidance, so it does not isolate the packing algorithm. |
| More briefing is reliably better. | In the factorial, the brief makes Opus 19.6% more expensive and 7.1% slower. It makes Sonnet cheaper, but a later diagnostic demonstrates a missed deeper-import notification in one briefed patch. The [first replay](../historical-replay-10166/README.md) has 0/4 independent acceptances despite all 22 ordinary tests passing when run externally afterward; those agents cannot execute commands. | Unsupported. Relevance and coverage matter more than the presence of a brief. Passing ordinary tests is insufficient. |
| System One should participate at every step. | The [version arc](../../../terminal-bench/2026-09-24-version-arc.md) reports the four-task Gemini loop with Jev at 3/4, $0.250 and 478 seconds per task; the same loop without Jev reaches 4/4, $0.242 and 369 seconds. | This historical matched ablation supplies contrary evidence for unconditional per-step hints. It does not disprove a targeted preparation judgment. |
| More control improves cost per accepted result. | The original audit independently recomputes the [matched controller study](../README.md#4-historical-evidence-that-changes-the-interpretation): 40.0% higher cost per accepted result, with an inconclusive pass gain. | Contrary evidence for restoring the entire older controller. Persistence and monitoring need separate justifications. |
| A typed semantic judgment improves a useful decision. | The [stall study](../../../terminal-bench/2026-09-25-stall-detection.md) reports 30/35 correct detections, but 83% of its evaluation checkpoints are stalls. Jev confirmation reduces recall without a statistically distinguishable precision gain over code signals. | Accuracy must be measured against a relevant baseline and base rate. A high-looking percentage alone does not establish value. |
| Better correctness can be measured. | The [archive confirmation study](../../../terminal-bench/2026-09-25-archive-check-confirmation.md) reproduces real specification defects in three officially passing circuit candidates. The independent factorial finds a missed notification after a frozen check passes; the [V2 diagnostic](../briefing-iteration-2/posthoc-execution.md) confirms three further defective final candidates. | Independent execution and contract review find defects that a benchmark grade misses. These observations do not establish that the proposed new workflow produces fewer defects. |
| The intended System One treatment actually runs. | The initial attempt at a [previous knowledge comparison](../../../terminal-bench/2026-09-26-kb-154-check.md) is uninformative because HTTP 402 leaves both arms with no entries. After credit is restored, its complete 12-run comparison records 1/6 passes with 104 entries and 1/6 with 154. This round's [capability receipt](jev-capability-refusal.json) records HTTP 402 and deterministic fallback. | Verify provider availability and delivered context before attributing outcomes to System One. The later funded comparison finds no observed pass-rate difference at this small sample size; it does not establish equivalence or a benefit from adding entries. |

The later V2 diagnostic executes the already-published two-case fixture without
changing it. The original source fails both cases, the historical reference
passes both, and five of eight final agent patches pass both: three of four
controls and two of four briefed candidates. These descriptive post hoc results
confirm the earlier static concern. They do not change the original registered
scores or establish a causal quality difference from briefing. They do prevent
using that panel's original perfect final acceptance as evidence of complete
correctness.

The historical version-arc, stall, archive-confirmation, and knowledge-check
figures above come from their retained reports; this round has not rerun
those experiments. The standing study and earlier independent replays have
their own source bindings and recomputation artifacts linked from the audit.

The knowledge report also carries a later configuration correction: its
Codex route requests strict function calls and receives zero reasoning
tokens despite the named effort setting. That further limits attribution
to knowledge selection. Record actual model usage and delivered inputs;
requested settings alone do not prove which mechanism ran.

## A sharper theory of preparation

The [conversation audit](../conversation-briefing-audit.md) identifies real
sources of delay: repeated environment discovery, stale paths, repeated
reads, missing test fixtures, and coordination overhead. The
[startup follow-up](../claude-startup-followup.md) also finds productive
integration work and long benchmark batches inside an apparently long agent
session. Tool-call counts and total session duration cannot label all of
that time as waste.

The resulting hypotheses have different implementation and measurement needs:

1. **Prepare stable facts once.** Cache the source graph, package ownership,
   test entry points, executable identity, and bounded Git history by source
   commit. Invalidate facts whose inputs change. A source hash, failed probe,
   or missing output is a fact; a semantic score must not override it.
2. **Preserve the contract before compressing evidence.** Keep requirements
   and repository instructions complete. Select exact, complete source
   units. Record omissions and the paths the executor can inspect next.
   The deeper-import miss motivates explicit dependency and fixture
   coverage checks; it does not justify another arbitrary token cap.
3. **Use System One for semantic uncertainty.** Candidate relevance,
   requirement-to-evidence matching, and deciding which bounded probe is
   useful are plausible decisions. The first new ablation tests only
   candidate ranking. It asks whether one batch changes downstream work
   enough to cover its own latency, context, and cost.
4. **Keep execution focused.** A lean prompt and six native tools can reduce
   fixed context while preserving file edits and shell checks. Shell access
   remains broad execution authority; a short tool list is not a security
   boundary. Isolated filesystems, network rules, and provider accounting
   supply the common benchmark boundary.
5. **Measure through an accepted candidate.** Include preparation, internal
   checks, final independent checks, and failed attempts. Preserve native
   caching. Report cold setup, queue time, review effort, and machine cost
   separately so a warm inference saving cannot imply a total engineering
   saving.

Production already has an `Ahead` preparation path. The startup follow-up
identifies freshness, duplicate-work, and accounting gaps there. The
experiment should inform that path after measurement. Creating a second
speculative preparation service would add another lifecycle to audit.

The [full acceptance preflight](acceptance-preflight.md) adds a concrete
preparation target: baseline test health. A historical reference cannot
satisfy the original gate because an unrelated ordinary test already fails.
Knowing this before dispatch prevents asking a paid executor to rediscover
the same problem. That knowledge is expensive to obtain here: the two large
checks take 160.79 and 176.75 seconds. Bind the result to source and environment
and reuse it when those inputs match. Running a full baseline suite before
every small turn could erase an inference saving. This is a measured setup
finding and a proposed reuse policy, not a demonstrated executor speedup.

The [C/D follow-up](eligibility-cd.md) makes two further requirements concrete.
First, source retrieval must follow the relevant helper: looking only at a
caller's ordering incorrectly suggests a missing-directory defect, while the
helper already creates the directory. Second, history can explain a conflicting
test expectation: an earlier authority change and another public test agree
with the behavior that an older integration fixture rejects. The review cancels
a planned repeat compilation without claiming an unmeasured time saving.
These cases motivate separate helper-coverage and behavior-history experiments;
they do not justify adding every dependency and commit to every prompt.

The [SDK qualification](eligibility-gamma.md) supplies an executed example of
additional verification coverage. A deliberately incorrect fix removes typed
request/response checking and still passes the ordinary tests; the independent
checker rejects it. The reference passes both. This establishes sensitivity to
that omission, not a defect rate for generated code. The checker also preserves
the API's historical calibrated-selection and optional-probability behavior.
A briefing or test generator needs the contract at the pinned revision;
applying a newer contract can incorrectly reject valid behavior.

Freeze any test-health packet against the source tree, ordinary-test command,
toolchain, build profile, platform, fixture inputs, and result identity. A dirty
tree or changed dependency invalidates the affected result. Include the exact
failure and its scope instead of telling an executor that a whole repository is
healthy. In a later isolated experiment, compare the same executor with and
without this packet and count rediscovery, unnecessary edits, and final defects
as well as time. Charge the initial qualification once and report the number of
subsequent tasks over which it is reused.

## Components that can be tested separately

### What preparation has to repay

The [standing-study arithmetic](preparation-headroom.json) gives a useful scale:
raw Claude averages 56.14 seconds and $0.29725 per task; lean Claude averages
48.21 seconds and $0.13341. The point differences are **7.93 seconds and
$0.16384 per task**. These are means over the same 21 observations per arm,
with three trials for each of seven tasks. Five of seven tasks have a positive
mean time difference.

Those differences are not a guaranteed preparation budget. Some preparation
costs and final checking time are absent, the tasks are familiar, and the
retrospective task-resampling interval includes no time improvement. Adding
7.93 seconds to the lean route without changing anything else would erase
its observed mean time advantage. A new brief may also change execution or
checking time, so a subsecond preview alone cannot establish a net saving.

For the incremental System One comparison, count its call, any extra input
tokens, changed executor work, and changed verification work against the
identical deterministic workflow. For preparation performed in advance,
also count discarded work and the portion that remains on the critical path.
Cache reuse can amortize indexing or baseline qualification; report the
initial cost and the actual reuse count. No measured cold index or preview
from this audit is added to the standing-study result because the tasks and
execution paths differ.

### Isolated tests

| Component | Cheap diagnostic | End-to-end decision |
| --- | --- | --- |
| Environment and history facts | Correctness against pinned Git objects and fresh probes; cache invalidation after each input changes | Does the packet remove repeated discovery without inducing a stale assumption? |
| Task workspace contents | On the same commit, compare the full export with a declared package, dependency, instruction, and fixture closure; run the same checks against both | Does smaller staging reduce setup time without repairs caused by missing files? Retain fallbacks and all construction costs. |
| AST candidate retrieval | Repeated-output identity, useful declaration recall on separately labeled tasks, required dependency omissions, latency and size | Does the agent read less while retaining acceptance and reviewed behavior? |
| Jev ranking | Paired ranking quality on the exact same pool, semantic relevance, one-call latency and actual usage | Compare C/B and F/E under the frozen protocol; do not credit the deterministic pack to Jev. |
| Briefing coverage | Deliberately omit a dependency, fixture, or requirement; measure which omissions the validator detects | Does adding coverage reduce independent defects enough to repay larger context or additional retrieval? |
| Prompt and tool constraints | Fixed prompt bytes, actual tool definitions, served model and effort records | Compare B/A and E/D. This first panel estimates a bundle; a later panel can separate prompt, tool count, and packing. |
| Independent acceptance | Base fails, historical reference passes, and plausible wrong implementations fail | Compare final acceptance and demonstrated defects, with check latency and cost included. |
| Preparation in advance | Freshness races, cancellation, abandoned requests, cache-hit behavior, and duplicate-charge accounting | Compare demand preparation with the same work done ahead of time, including discarded speculation. |

The current warm component measurements cover output identity, elapsed time,
and size. They do **not** establish retrieval recall, dependency coverage,
semantic ranking quality, or executor improvement. No treatment tuning used
reserve executor outcomes because no such outcomes exist.

The qualified [alpha task's preview](alternative-alpha/README.md) supplies a
concrete coverage diagnostic. Two available source units are omitted from the
delivered pack, while a larger function exceeds the unit-size cap before any
semantic ranker sees it. A Jev reranker could change the first selection; it
cannot recover evidence absent from its candidate pool. The preview completes
in 0.665 seconds after separate indexing, with exact source spans verified.
That establishes fast, faithful extraction for this observation, not sufficient
task coverage. Preserve the frozen policy for the experiment and test any later
candidate-expansion policy separately.

The [beta](alternative-beta/README.md) and [gamma](alternative-gamma/README.md)
previews strengthen that warning. Both take about 0.264 seconds, but beta's
pool omits the ATIF reader and writer implementations, and gamma's pool has
no unit from `jev/src`. Those declarations exist in the index. This is a
failure to supply obvious implementation evidence, even though every retained
source span is accurate. It is not a measured coding failure: the executor can
still inspect files, and no executor trial has run.

The next retrieval hypothesis should therefore precede semantic ranking:
preserve explicit package and file anchors, include the relevant implementation
alongside tests, and account for each requested public contract. Test candidate
recall and required-reading coverage separately from exact-span fidelity and
latency. A semantic judge can prioritize uncertain relevance after deterministic
retrieval makes necessary evidence available. These inspected tasks now supply
development feedback for any future revision; they cannot remain unseen
confirmation tasks for a policy tuned to their omissions. The current treatment
and its retained previews remain unchanged.

Workspace staging is another deterministic hypothesis prompted by the measured
1.5 GB source exports and their Git snapshot costs. A dependency graph alone
is insufficient: build scripts, included files, runtime fixtures, repository
instructions, and tests that inspect directory contents also need coverage. Test export
completeness before measuring agent performance, and fall back to the full tree
when the required file set is unknown. This is a future matched experiment;
none of this panel's source exports are reduced.

Some public tasks already name a file or function. A semantic ranker may add
little when deterministic retrieval finds that evidence directly. This panel
does not assume every task needs Jev. A later study can prospectively separate
tasks with explicit locations from tasks that require broader discovery;
selecting that subgroup after observing favorable outcomes would not provide
an independent confirmation.

## What would change the recommendation

The [prospective design](protocol.md) preselects F, lean Sonnet with one Jev
ranking call. It must beat E, the identical deterministic workflow, and A,
native Opus, with complete accounting and independent quality gates. C/B
checks whether the same ranking helps Opus. B/A and E/D estimate the lean
workflow benefit within a model. The four task clusters limit generalization.

If E beats D and F does not beat E, adopt or further test deterministic lean
delegation and retain the negative Jev result. If F saves inference cost but
adds total elapsed time, report that tradeoff. If either loses required
behavior, reject a practical superiority claim even when its bill is lower.
If both cost and time improve with equal observed quality, describe it as
equal measured quality, not better correctness.

Future trials should test new tasks under a new prospective protocol. Keep
the failed configurations and development costs in the ledger. Repeatedly
retuning this panel until it passes would weaken the evidence the user is
trying to build.
