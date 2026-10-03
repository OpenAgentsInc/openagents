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
| Better context can reduce executor work. | The [second briefing iteration](../briefing-iteration-2/README.md) has all final candidates accepted, with 12.6% lower median cost and 7.5% lower median time on its held-out task. The [historical version arc](../../../terminal-bench/2026-09-24-version-arc.md) reports a log-summary context repair changing Luna from 0/3 to 3/3. | Promising bounded evidence. The first misses its registered 20% cost threshold. The historical change also changes rendered guidance, so it does not isolate the packing algorithm. |
| More briefing is reliably better. | In the factorial, the brief makes Opus 19.6% more expensive and 7.1% slower. It makes Sonnet cheaper, but a later diagnostic demonstrates a missed deeper-import notification in one briefed patch. The [first replay](../historical-replay-10166/README.md) has 0/4 independent acceptances despite all 22 ordinary tests passing when run externally afterward; those agents cannot execute commands. | Unsupported. Relevance and coverage matter more than the presence of a brief. Passing ordinary tests is insufficient. |
| System One should participate at every step. | The [version arc](../../../terminal-bench/2026-09-24-version-arc.md) reports the four-task Gemini loop with Jev at 3/4, $0.250 and 478 seconds per task; the same loop without Jev reaches 4/4, $0.242 and 369 seconds. | This historical matched ablation supplies contrary evidence for unconditional per-step hints. It does not disprove a targeted preparation judgment. |
| More control improves cost per accepted result. | The original audit independently recomputes the [matched controller study](../README.md#4-historical-evidence-that-changes-the-interpretation): 40.0% higher cost per accepted result, with an inconclusive pass gain. | Contrary evidence for restoring the entire older controller. Persistence and monitoring need separate justifications. |
| A typed semantic judgment improves a useful decision. | The [stall study](../../../terminal-bench/2026-09-25-stall-detection.md) reports 30/35 correct detections, but 83% of its evaluation checkpoints are stalls. Jev confirmation reduces recall without a statistically distinguishable precision gain over code signals. | Accuracy must be measured against a relevant baseline and base rate. A high-looking percentage alone does not establish value. |
| Better correctness can be measured. | The [archive confirmation study](../../../terminal-bench/2026-09-25-archive-check-confirmation.md) reproduces real specification defects in three officially passing circuit candidates. The independent factorial finds a missed notification after a frozen check passes. | Independent execution and contract review find defects that a benchmark grade misses. These observations do not establish that the proposed new workflow produces fewer defects. |
| The intended System One treatment actually runs. | The initial attempt at a [previous knowledge comparison](../../../terminal-bench/2026-09-26-kb-154-check.md) is uninformative because HTTP 402 leaves both arms with no entries. After credit is restored, its complete 12-run comparison records 1/6 passes with 104 entries and 1/6 with 154. This round's [capability receipt](jev-capability-refusal.json) records HTTP 402 and deterministic fallback. | Verify provider availability and delivered context before attributing outcomes to System One. The later funded comparison finds no observed pass-rate difference at this small sample size; it does not establish equivalence or a benefit from adding entries. |

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

## Components that can be tested separately

| Component | Cheap diagnostic | End-to-end decision |
| --- | --- | --- |
| Environment and history facts | Correctness against pinned Git objects and fresh probes; cache invalidation after each input changes | Does the packet remove repeated discovery without inducing a stale assumption? |
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
