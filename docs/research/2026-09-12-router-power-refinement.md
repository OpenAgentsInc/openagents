# Router Power against Coder's program model: Specific Intelligence and the untested agent runtime

Date: 2026-09-12
Status: refinement of the Router Power analysis memo. This is a copy of the
document at `docs/evaluation/2026-09-12-router-power-refinement.md` in the
Coder repository, placed here beside the harness-optimization and DSPy
audits it draws on; the Coder copy is the one the loop updates. The memo it
refines lives beside it there as
`docs/evaluation/2026-09-12-router-power-analysis.md`. Read that memo first.
This document restates the same evidence in the vocabulary of the Router
Power research: Specific Intelligence, residual transferable uncertainty,
and the agent runtime. It tests the memo against Coder's plugin, memory,
context-program, and optimizer designs, reads the Router Power code for what
each study varied and held fixed, and records where the evidence decides a
recommendation and where it only bounds one. Written against the Coder tree
at `98ddd0be1d` and `router-power` as frozen on 2026-09-07. Paths in
parentheses that name the Coder repository are files there, not here.

## Summary

Router Power asks whether residual transferable uncertainty is large enough
that a centralized router delivers cheaper agent labor than a firm that
learns on its own. Residual transferable uncertainty is the uncertainty that
remains after local learning and that transferred learning from other
customers can reduce. If it stays high, Specific Intelligence does not hold. Specific Intelligence
is the claim that a firm's own outcome stream is a compounding loop others
cannot recreate. Most firms should then buy managed outcomes rather than
keep an autarkic learning loop. The public writeup is bearish Specific
Intelligence. It locates outcomes pricing at the router, neocloud, or
rollup layer, not at the model or app layer.

The analysis memo's engineering recommendations survive. The studies do not
reach Coder's program model. The research plan names the arm as the agent
runtime: model, harness, tools, retrieval, inference effort, and provider.
Every harness study varied one thing: the system prompt. Learning ran in
two ways, both prompt-side: a playbook learner that turns failed transcripts
into a short operating memo, and a case-retrieval learner that injects two
successes and one failure before every turn. No study ran an optimizer with
a gate, changed a tool, a bound, or control flow, or compiled an instruction
block that could return empty. The strong models' residual failures were a
fixed 360-second deadline, so the model-swap measured prompt bloat under a
deadline. Smarter models needed less harness; the remaining failure was
time.

What Router Power settles for Coder:

- Distilling commodity information into a standing playbook and shipping it
  across models hurts the next model. A playbook of one model's failures is
  knowledge about that model, not a portable Specific Intelligence asset.
  Coder's loop found the same on its own model in rounds 13 to 15, and the
  harness-optimization audit in this repository recorded on 2026-07-04 that
  code transferred across model families and prompts did not. The accepted-memory design already refuses to produce this artifact.
- A learned per-ticket selector over model names, trained on outcomes, did
  not beat the best fixed model when the arms were close in price and
  quality. The research plan said the arm should be the entire runtime.
  Coder's bench has a noise floor that would defeat a name-only selector on
  its own. Keep lanes as runtimes. Keep the door a transport.
- Retrieval the model must ask for is not called. The case-retrieval learner
  won because the host injected it on every turn. Inject it from the host.
- Pooled settled outcomes built from commodity information are not a
  compounding moat. The pooled edge is a novelty advantage: about ten points
  on problems new to the customer and known to the pool, gone by about 144
  tickets, reset by a model release. Coder's docs never claimed one. The
  Tassadar program stated the data advantage as a test to run and a pitch
  line to avoid.

What Router Power leaves open, and where the memo treats it as closed:

1. **A compiled, gated, model-bound instruction block.** The playbook that
   cost the next model 6 to 10 points was one model draw, format-checked,
   never run on a held-out ticket before deployment. The one study that had
   a validation gate rejected its candidates correctly. A compile step whose
   gate can return no artifact was never tested on a model change. That is
   the DSPy rule. The memo's model-identity stamp is the right rule; the
   stamp is a cutover gate, and the gate may return the empty artifact.
2. **Instructions plus exemplars.** Retrieval beat the playbook by about 18
   points as alternatives. No arm combined a short instruction block with
   host-injected exemplars, which is what a compiled module is. The memo's
   "the best policy was no policy at all" compares the two halves of that
   module rather than the module against nothing.
3. **The tools half of the agent runtime.** Output processors, typed tools,
   bounds, and control flow were never an arm. Study 7A's timeouts are
   prompt bloat under a deadline, which is evidence for host-side bounding
   rather than against harness work. Coder's suite members are host-side and
   deterministic except for one prelude. The memo's "prompt-side plugins do
   not move the numbers" is not a finding about the suite.
4. **Routing over runtimes, and a cascade as a bound.** Study 1 routed three
   cheap flash-class models a factor of 1.6 apart in price on their names
   alone. The research plan says the arm should be the entire runtime. A
   deterministic escalation policy on a gate refusal is a bound on an
   observed outcome, not a learned router, and is untested.
5. **Scarce sensing, which the benchmark assumed away.** The environment had
   one manual and one tool set. The report says store-specific knowledge
   could not exist there by construction, and that a domain with a larger,
   more idiosyncratic knowledge space would likely show slower catch-up and
   less transfer. Specific Intelligence, in the research writeup, exists
   where a firm's problems are genuinely its own, in scarce sensing regimes.
   A repository is that domain. For coding, the own-history effect is
   unknown, not small.
6. **Records against prescriptions.** Coder's accepted memory holds settled
   facts about a repository with provenance, bounded at eight facts and 1,200
   characters, and never extracts. That is a case store of records, not a
   playbook of behavior, and a fact about a build is model-generic. The
   memo's rule that every memory fact goes to opt-in on a model change is
   broader than the evidence. Only behavioral instructions need the model
   stamp.

Section 5 turns those six into the rounds that would decide them.

## 1. What Router Power varied and what it held fixed

The memo reports the numbers. This section reports the interventions, read
from the code under `release/opx/harness/` and `scripts/`, because the
interventions bound what the numbers can say about Coder.

| Axis | Router Power | Held fixed or never tried |
| --- | --- | --- |
| The learned playbook | One free-text Markdown document of at most 900 words, produced by a single outer-model call from at most 45 sampled failure records, at temperature 0.7 (`scripts/multicompany_heterogeneity_improve.py`, `PLAYBOOK_WORD_LIMIT` and `FAILURE_RECORD_CAP` in `scripts/multicompany_heterogeneity_common.py`). | Held-out validation before deployment; a candidate pool; Pareto selection; per-instance demonstrations inside the playbook; a metric-driven accept or reject. |
| The playbook's acceptance check | `validate_playbook`: word count, heading order, no tool names, no task ids, no company labels. A playbook ships when it parses. | Any ticket run to compare the new playbook with the old one. |
| The one validation gate | Study 2 only: `mine`, `tune`, `gate`, `test` splits; a candidate deploys when gate success is at least base and mean cost at most 1.25 times base, else base deploys (`scripts/self_harness_analyze.py`). It fired, and rejected two of three candidates. One revision, one candidate, about ten gate tasks per domain; the writeup calls one rejection a likely small-gate false negative. | The gate applied to any later study, including the model swap. |
| Study 7A, the swap | Study 5's frozen playbook and case store, hash-pinned, applied verbatim to two newer models. "Nothing is relearned" (`scripts/multicompany_model_swap.py`). | Recompiling on the new model; a gate that could return the empty playbook. |
| Study 7B, the relearn | `LEARNERS = ("retrieval",)`: the case store was refilled from the new model's own outcomes. | The outer improver on the new model. It never ran on DeepSeek or GLM in any study. |
| The retrieval learner | Host-injected before every agent turn, no model in the loop: BM25 over the customer's words so far, two most similar successes and one failure, 450 words a case, 1,500 words a packet (`release/opx/harness/retrieval_agent.py`, `case_retrieval.py`). | A dose other than three cases; a query that includes tool output, excluded because tool results were near-identical account JSON across tickets. |
| Instructions plus exemplars | Arms were `base`, `pb_*`, `rt_*` in Study 5 and `stock`, `playbook`, `retrieval` in Study 7A. | Any arm combining an instruction block with retrieved demonstrations. |
| Below the prompt | `PlaybookAgent` and `RetrievalAgent` override `system_prompt` and pass tau2's tools through unchanged. The playbook prompt forbids naming a tool. | Tool wrappers, output processors, bounded or digested tool results, typed or structured output, control flow, decomposition into modules. |
| Bounds | 360 seconds, a constant in every runner. No turn cap variable exists in the tree. | A bound as an arm; a raised cap. The summary's next-steps item 11 asks for one. |
| Routing arms | Three flash-class models at $0.00277 to $0.00437 a run, a 14.5-point quality spread, routed on model names by a domain router, an intent router, and a leave-one-trial-out selector. | A price gap; an expensive frontier arm; a cascade (the advertised cascade arm was never implemented, per `writeups/REVIEW.md`); the runtime as the arm, which `writeups/ROUTER_POWER_RESEARCH.md` recommends first. |
| Heterogeneity | Six synthetic companies from tau2 telecom personas, sharing one policy and one tool set, differing in which fault mechanisms they met first. | Business-rule or tool heterogeneity; the report names it as the axis where store-specific knowledge could exist. Per-firm implementation work that would give each company its own tools and policy is assumed away. |
| Headroom | DeepSeek 90.3% and GLM 79.2% stock in Study 7A, DeepSeek 95.6% in 7B. Every failure of both models in every version was the 360-second limit. | A newer model whose failures are of a kind experience can fix; the summary's next-steps item 5 asks for one. |

Two quotations from the report carry the boundary. On the swap:
"Every failure in the primary table is a timeout, so the survival fractions
above measure the harness's effect on conversation length, not on the
model's ability to resolve the ticket" (`data/multicompany_model_swap_v1/report.md`).
On heterogeneity: "store-specific knowledge could not exist here by
construction. A domain with a larger, more idiosyncratic knowledge space
would likely show slower catch-up and could show less transfer"
(`data/multicompany_heterogeneity_v2/report.md`).

## 2. Coder's program model, sorted by what Router Power tested

Router Power's harness arms fall into two classes: a playbook of instruction
text learned from outcomes, and a case store of exemplars retrieved by
similarity. Its routing arm is a third, and it routed on model names. Coder's
mechanisms fall into four, and the fourth, program structure on the tool
side of the agent runtime, is the largest.

| Coder mechanism | Class | Runs where | Model may decline | Router Power evidence | Coder evidence |
| --- | --- | --- | --- | --- | --- |
| Output processors: `test_report`, `shell_digest`, `git_diff_summary`, `cargo_diagnostic_filter` | Program structure, tool side | Host, deterministic, after a tool call, one owner per call (plugin suite, `docs/plugin-suite.md` in the Coder repository, "Interaction rules") | No | None. No study bounded or digested a tool result. | Rounds 16, 17, 20: tokens down 20% to 36%, seconds up 10% to 19%; reverted by the guard, not by a verdict on the class (loop, `docs/optimization-loop.md` in the Coder repository). |
| The `env_facts` prelude | Instruction text | Host, once, appended to the first turn | No | Study 3's shared-playbook interference; Study 7A's decay. | Rounds 13 to 15: helped one task, hurt the other by 27% to 71% on prompt tokens; reverted three times. Agrees with Router Power. |
| Skills | Instruction text plus an authority layer | One summary line a skill in the first turn, body on invocation; a skill never widens the manifest (skill loader, `docs/plugins/2026-09-10-skill-loader.md` in the Coder repository) | Yes, the invocation | None for the authority layer. The text half is the playbook class. | Unmeasured as a class. |
| Capability tools: `repo_context`, `repo_search_bounded` | Tool side, model-chosen | Host, when the model calls it | Yes | None directly. Study 5's retriever was host-driven for the same reason: the report notes retrieval delivered on 100% of turns. | Declared and never called in rounds 21, 26b, 28, 28b; worse on every metric from the declaration bytes. Agrees with the memo's host-injection rule. |
| Typed search, `code_search` | Tool side, model-chosen | Host, when called | Yes | None. | Round 23b: called in every trial that declared it, more in the arm with no prompt sentence about it, at more calls and seconds than `shell` alone on that one lookup. |
| Context program, `context_eval` | Program structure | Host interpreter, typed JSON, bounded, no ambient filesystem or network (context RLM, `docs/evaluation/context-rlm.md` in the Coder repository) | Yes | None. Router Power has no program below the prompt. | Validated in isolation; no loop row yet. |
| Retained context and bounded tool results | Program structure | Host, every tool result over budget stored whole and previewed inside the budget | No | Study 7A, read as prompt bloat under a deadline, is evidence for this class. Router Power's own retriever excluded tool output because it was near-identical JSON. | The 16,000-byte packet rule; the checkpoint window in round 29. |
| Inherited context on a fork | Exemplars of settled facts | Host, before the child's first turn (selected context, `docs/2026-09-10-selected-context-and-forks.md` in the Coder repository) | No | Study 5's host-injected retrieval, in kind. | Round 25: change-directive calls 14.3 to 4.3, prompt tokens down 72%. |
| Accepted memory | Settled records with provenance | Host, at session opening, eight facts and 1,200 characters (proposed memory, `docs/2026-09-11-proposed-memory.md` in the Coder repository) | No | Closer to the case store than to the playbook: a record of what is so, not a prescription of what to do. Study 7A's case store survived the swap at +3.5 and 0.0. | Measured admission cost 1,064 characters; no value row yet. |
| Per-turn fact extraction | Instruction text, learned | Held back until a loop row exists (accepted memory, `docs/2026-09-10-accepted-memory.md` in the Coder repository, "What this pass does not do") | n/a | Study 7A's playbook is the artifact this would produce. | The refusal is already written. |
| Typed signatures compiled against a metric with a held-out split | Instruction text plus exemplars, validated | Offline compile, immutable artifact, gate before promotion (Tassadar, deprecated; DSE history audit in this repository) | n/a | Never tested. No arm combined the two halves, and no study compiled with a gate except Study 2, whose gate worked. | Not built. The Gym GEPA lane kept a separate holdout and landed nothing automatically; the one numeric result is an internal comparison. |
| A learned plugin selector against a fixed rule | Routing, over plugins not models | Proposed as the first learning experiment, with the rule retained when adequate (Tassadar, deprecated) | n/a | Study 1 is about model arms. The plugin-selection question was never posed. | Not run. The deterministic owner-per-call rule is what the suite ships. |
| Fixed model per lane, chosen on the bench | Routing, coarse | Configuration | n/a | Study 1: the fixed arm won. | Lanes exist. |

The "Router Power evidence" column is empty for program structure on the
tool side, which is where Coder spends. The class Router Power tested
hardest, a playbook of learned instruction text shipped without a gate, is
the class Coder's loop already reverts and the accepted-memory design
already refuses. That class is Specific Intelligence built from commodity
information: a bad router.

## 3. Where the memo is stronger than the evidence

Each item names the memo's claim, what the evidence supports, and the
correction in Router Power's own terms.

### 3.1 A playbook of one model's failures is commodity information

The memo: "Harness knowledge is knowledge about one model's failure modes,"
and "a sentence learned on one model cost the next model 6 to 10 points."

The evidence: an unvalidated sentence did. The 7A artifact was one draw of
the outer model, checked for format, and applied verbatim. Nobody ran it on a
single held-out ticket of the new model before reporting its cost. The one
gate in the corpus, Study 2's, compares a candidate with base on an untouched
set and deploys base when the candidate is not better. Applied to 7A's
numbers, that gate declines the playbook on both models, since both intervals
exclude zero, and declines the case store on GLM at +0.0. The report reaches
the edge of this reading ("a high stock rate for a candidate leaves little
room for any harness to help") and does not draw it. Smarter models needed
less harness; the playbook was a tax on conversation length.

The correction: the memo's recommendation 5, stamp the model identity on
every measured artifact and withdraw it to opt-in on a model change, is a
gate described as a withdrawal. Describe it as a gate. On cutover, every
instruction-side artifact is re-measured on the new model against no
artifact, and an artifact that does not beat nothing is not admitted. That is
the DSPy rule, compile per model, restated for a suite: the compile step may
return the empty artifact, and on a model at ceiling it will. Coder's plugin
suite admission already has the shape (a strict per-task test, a held-out
ten, the suite compared with and without the member); what it lacks is the
trigger on a catalog change.

A standing playbook distilled from outcomes and carried across models
without that gate still loses. Router Power measured that. That is Specific
Intelligence built from commodity information. The memo's item in "What not
to build" stays.

### 3.2 Retrieval beats a playbook, so the best policy is no policy

The memo: "the best policy was no policy at all: raw exemplars retrieved by
similarity," and "do not add a step that distills facts into a standing
playbook."

The evidence: retrieval and the playbook were alternatives in every study.
No arm carried a short instruction block beside the three retrieved cases.
The 18-point gap is between the two halves of a compiled module, not
between a module and nothing. The Study 2 writeup names the mechanism that a
combined arm would have to survive, instruction interference, and says it
"should be tested, not treated as settled."

The correction: do not distill facts into a standing playbook. That artifact
lost. "No policy at all" is not a finding. The untested arm is a validated
instruction block of a few hundred words, bound to the model it was measured
on, beside host-injected exemplars. Section 5 gives it a round. Until that
round runs, Coder's rule is the loop's own: prefer the tool layer, and
measure a sentence before it stays.

### 3.3 Timeouts are where the win rate is

The memo: Study 7A's timeouts are "the single most useful row for Coder,"
and the fixes are caps with typed endings, pointer answers, early stop on a
failed gate, and deterministic tools.

The evidence agrees. The strong models' residual failure surface was entirely
the deadline, so the survival ratio the report computes divides by headroom
that no harness could address. What 7A measured is the token economy of
prompt bloat under a fixed deadline: a 900-word block of stale prose
lengthened conversations, and a 1,500-word block of exemplars shortened them
slightly. That is a result about what enters the context each turn, and the
mechanisms that decide that in Coder are host-side: bounded tool results,
the checkpoint window, the packet rule, and the output processors. Those are
the tools half of the agent runtime. Router Power's retriever excluded tool
output from its query because the tool results were near-identical JSON,
which is the input `shell_digest` and `test_report` exist to digest.

The correction: add the output-processor class to the memo's section 3.3
table as model-generic and untested by Router Power, and keep the suite's
seconds guard. Round 17 reverted `shell_digest` by that guard at +10.1% on
seconds against a 27.6% token saving. That guard is Study 7A's lesson applied
before Router Power stated it: an admission rule that budgets accuracy and
tokens without time re-derives Study 7A. Then run the memo's raised-cap round
so a change is measured on resolution and not on the deadline.

### 3.4 Do not build a per-call router

The memo: decline the per-call model router behind `responses::Source`; keep
model choice fixed per lane.

The evidence supports the decline in the regime tested: three cheap models a
factor of 1.6 apart in price, routed on their names by a pre-execution
selector, with per-task rankings that did not repeat. Coder's bench has a
control spread of 63% on calls and 148% on tokens on one task, which is the
noise a learned selector would learn. Router Power's falsification threshold
was a fixed arm within two points of the held-out router, and the fixed arm
beat every router.

Three things sit outside that regime and the memo folds them in.

- **The runtime as the arm.** Router Power's research plan says first to
  treat the arm as the entire runtime: model, prompt, harness, tools,
  retrieval, inference effort, and provider, and not merely a model name. No
  study did. A Coder lane is a runtime: a model, a suite version, a bound
  set, a price. Choosing among lanes is the decision Study 1 did not test.
  The memo's lane rule already puts the choice at that granularity. Study 1
  does not argue against it. The evaluation lane in the memo's recommendation
  6 is where lanes get compared. That is also the information advantage the
  writeup names at a model release: whoever knows which runtime to cut over
  to captures the gain.
- **A cascade as a bound.** Try the cheap lane, and escalate to the strong
  lane when the gate refuses delivery. That is a policy on an observed
  outcome, not a selector on pre-execution information, and Router Power
  removed its cascade arm before running it. Whether it lowers cost per
  landed change against the strong lane alone is one bench round.
- **Selection over plugins.** The Tassadar program's first learning
  experiment was never a model router. It was "given a shell result and the
  admitted capabilities, return ordinary output, use `test_report`, or
  request more structure," comparing a deterministic rule with a model
  selection, keeping the rule when adequate. Study 1 says nothing about it.
  The suite ships the rule (one owner per call), and the experiment stays
  open.

What stays decided: no learned pre-execution selector over model names.
`Source` stays a transport.

### 3.5 Own history holds nothing others cannot supply

The memo concedes in section 3.2(f) that the axis is untested and then
applies Studies 5 and 6 to Coder's pooling decisions: "sell it as
onboarding, a time-to-competence lead for a new repository, and nothing
more."

The evidence: the report says store-specific knowledge could not exist in
its environment by construction, and that a domain with a larger,
idiosyncratic knowledge space "would likely show slower catch-up and could
show less transfer." The simulation assumed away per-firm implementation
work that would give each company its own tools and policy. A repository is
the idiosyncratic domain: its own build, gate, conventions, and layout, and
no shared manual. In the research writeup, Specific Intelligence exists
where a firm's problems are genuinely its own, in scarce sensing regimes,
and most firms that learn from commodity information are building a bad
router. A git checkout is closer to scarce sensing than to pooling telecom
personas that share one manual. For repositories the sign and size of the
own-history effect are unknown. Coder's own rows point the other way from
Study 5: the context that cut change-directive calls by 70% was facts about
this checkout.

One more bound the memo does not weigh: Study 4. Releasing three relevant
other-company trajectories before a company's first encounter with a problem
did not help against releasing them after (−0.98, interval −7.03 to +5.06,
excluding the planned +10). Study 6's novelty edge and Study 4's null are
both about the first encounter, and they disagree. The memo builds the
onboarding pitch on Study 6 alone.

The correction: keep the memo's Study 6 replication, take Study 4's null as
the prior, and do not write the onboarding sentence until the row exists.
Scope retrieval to the repository first, as the memo says, because that is
where the evidence says the knowledge is, not because pooling was shown to
be worth ten points.

### 3.6 Every memory fact goes to opt-in on a model change

The memo's recommendation 5 stamps the model identity on "plugin admission
rows, memory facts, instruction sentences" and sends every instruction-side
artifact to opt-in on cutover.

The evidence distinguishes two artifacts Router Power kept apart. The
playbook prescribed behavior ("do not transfer prematurely") and hurt the
next model, which did not have that failure. The case store recorded what a
resolved ticket looked like and survived the swap. A Coder memory fact is a
one-sentence record with an evidence digest, scoped to a session, workspace,
or repository, proposed from a person's correction or a child's finding, and
never extracted from a conversation. "The gate needs Postgres on port 5432"
is a fact about the world; it does not describe a model and does not expire
with one.

The correction: split the stamp. Behavioral instructions (a prelude, a skill
body, a sentence in `agent_instructions.rs`, a plugin whose effect depends on
the model calling it) carry the model identity and re-validate on cutover.
Repository facts and settled records carry their evidence digest and expire
on evidence, as the accepted-memory design already says. The one caution
Router Power adds for facts is dose: the admission bound of eight facts and
1,200 characters is the kind of cap Study 7A rewards, and it should not grow
without a row.

### 3.7 This is new evidence against Coder's direction

The memo addresses the private strategy document and treats the flywheel
claim as the position Router Power overturns. In this repository the
position was already the other way.

- The harness-optimization audit in this repository (2026-07-04) recorded that
  the code that repaired tool calls transferred across model families and
  tuned prompt playbooks backfired, that five of the top six harness improvements were
  deterministic code, and that candidate manifests should record the model
  family they were evolved against so the admission gate can require
  re-evaluation when the target differs.
- The Tassadar program (deprecated 2026-09-06) stated the data advantage as a
  test: remove the proprietary inputs and measure the loss on fresh tasks;
  "if there is no loss, the data advantage has not been demonstrated." Its
  pitch guidance was not to lead with an automatic data moat.
- The loop's rule, written before Router Power, prefers the tool layer over
  the instructions because a sentence "holds until the model ignores it."
- The accepted-memory design refuses autonomous extraction and summarization
  into facts, pending a loop row.

Router Power confirms four positions this repository already held and adds
numbers to them. The memo's strategy rewrite is for `alpha`. The engineering
docs here need no reversal, and a reader should not infer one. Specific
Intelligence built from commodity information was already declined. The
agent runtime's tools half was already the bet.

## 4. What Router Power settles

The findings that decide something for Coder, with no correction:

- A distilled playbook learned from outcomes on one model, shipped to
  another without a measurement, hurts. That is a bad router: Specific
  Intelligence built from commodity information. Keep per-turn extraction
  held back. Keep the loop's revert of the prelude and the two shell-guidance
  edits.
- A learned pre-execution selector over model names does not beat a fixed
  strong model among close arms. Keep `Source` a transport; keep lanes
  fixed as runtimes.
- Retrieval that the model must ask for is not called. Inject it. Coder
  learned this in four rounds before the memo said it.
- Under a deadline, what enters the context each turn decides the finish
  rate. Budget time in every admission rule, and report resolution
  conditional on finishing.
- The value of observing many runs on many models at a release is knowing
  first which runtime to switch to. That is an information advantage about
  cutover, not a compounding Specific Intelligence loop. Build the
  evaluation lane before any learner.
- Pooled settled outcomes are a short novelty advantage in the only
  environment measured. Say nothing stronger until a repository-scoped row
  exists.

## 5. The rounds that would decide the open items

Each is one loop round on the three bench directives and the held-out ten,
three trials a side, interleaved, with the model identity and the
task-clustered interval on the row. In the memo's order of value.

| Round | Candidate against control | Decides | Prior |
| --- | --- | --- | --- |
| Cutover gate | On the next door catalog change, re-measure every admitted prelude, skill, and instruction sentence on the new lane model against no artifact, through the suite's strict test. Admit what beats nothing. | Section 3.1: whether the survival check is a gate that can return the empty artifact. | Study 2's gate rejected correctly; Study 7A's artifacts would have been declined. |
| Bounds as arms | Raise the wall cap until conversations finish; report resolution conditional on finish beside the finish rate; then set the cap from what finishing costs. The memo's recommendation 8. | Section 3.3: whether a change moves resolution or only the deadline. | Router Power next-steps item 11. |
| Instructions plus exemplars | Host-injected two successes and one failure from this repository's runs, alone, against the same packet beside a validated instruction block of at most 300 words bound to the lane model. | Section 3.2: whether the module beats its retrieval half, or interference costs the difference. | Untested; the interference hypothesis is in the Study 2 writeup. |
| Cascade as a bound | Free lane first, escalate to the pro lane on a gate refusal, against the pro lane alone. Metric: cost per landed change, then calls. | Section 3.4: whether a policy on an observed outcome pays where a selector on pre-execution information did not. | Router Power removed its cascade arm unrun. |
| Output processors under time | The suite with `shell_digest` and `test_report` on, against off, with the seconds guard kept and the raised cap from the bounds round. | Section 3.3: whether the processor class pays once the deadline is not the failure surface. | Rounds 16, 17, 20: tokens down, seconds up by the guard's margin. |
| Repository against pool | The memo's Study 6 replication: retrieval from a fixed pooled trajectory store on directives new to the target repository, against retrieval from the repository's own runs, novel and familiar contrasts read separately. | Sections 3.5 and 3.6: whether pooling pays on first-of-kind directives, and whether facts about this checkout do more than cases from others. | Study 6 says +11 on novel; Study 4 says −1 on early release; Coder's round 25 says checkout facts are worth 70% of calls. |

A round that makes a number worse goes in the results table the same as one
that helps.

## 6. Recommendations, refined

The memo's section 5.1 stays, with these edits.

1. Recommendation 1 stays: no per-call model selection behind
   `responses::Source`. Add: a lane is a runtime, and the evaluation lane
   compares lanes, not models.
2. Recommendation 2 stays: host-injected pre-turn retrieval, bounded, two
   successes and one failure, repository-scoped. Add: measure the packet
   alone and beside a validated instruction block, as one round each.
3. Recommendations 3 and 4 stay: typed endings for every cap, resolution
   conditional on finishing, and no done after a failed gate, enforced in
   the loop.
4. Recommendation 5 splits. Behavioral instructions carry the model identity
   and re-validate through the suite's strict test on a catalog change, with
   no artifact as the control. Repository facts and settled records carry
   their evidence digest and expire on evidence. The recall bound stays at
   eight facts and 1,200 characters until a row moves it.
5. Recommendation 6 stays and moves up: the evaluation lane runs on every
   catalog change and its verdict row triggers item 4. That is the
   information advantage at a release.
6. Recommendation 7 stays with Study 4's null as the prior and no
   onboarding claim until the row exists.
7. Recommendation 8 stays and runs before item 2, so retrieval is measured
   on resolution.
8. Recommendation 9, billing, stays. Router Power adds nothing against it
   and the separability argument in the memo's section 4.3 is the right one.
9. Add: the output-processor class joins the memo's section 3.3 table as
   model-generic and untested by Router Power, and its admission keeps the
   seconds guard. That is the tools half of the agent runtime.
10. Add: a cascade on gate refusal is a bound to measure, not a router to
    decline.
11. The memo's "What not to build" stays whole. Add nothing to it on the
    strength of Router Power alone: a compiled, gated, model-bound
    instruction block is not on the list, because it was not tested.

Net for the business: the verification and settlement loop, the neutral
door, and the evaluation lane that decides cutovers remain the defensible
claims. Deterministic plugins and typed programs are the durable,
model-portable half of the agent runtime. Specific Intelligence built from
commodity information is not. Six one-round experiments in section 5 would
close the open items.

## Evidence and derivation

Observed Coder baseline: the loop's results table through round 33
(`docs/optimization-loop.md` in the Coder repository); the suite's admission
rules and the repository-context decision of 2026-09-11
(`docs/plugin-suite.md` in the Coder repository); the memory designs
(accepted, `docs/2026-09-10-accepted-memory.md` in the Coder repository,
proposed, `docs/2026-09-11-proposed-memory.md` in the Coder repository); the fork and inheritance
design (selected context, `docs/2026-09-10-selected-context-and-forks.md` in the Coder repository);
the context program (context RLM, `docs/evaluation/context-rlm.md` in the Coder repository); the skill loader
(skill loader, `docs/plugins/2026-09-10-skill-loader.md` in the Coder repository); the catalog
(plugin catalog, `docs/plugin-catalog.md` in the Coder repository); and the deprecated Tassadar
program (return of Tassadar, `docs/deprecated/2026-09-04-return-of-tassadar.md` in the Coder repository,
Gym summary, `docs/deprecated/2026-09-04-gym-summary.md` in the Coder repository).

This repository, read for the DSPy lineage:
[the harness-optimization audit](2026-07-04-harness-optimization-evolve-the-harness-audit.md);
[the DSPy and RLM backend audit](2026-06-28-dspy-rlm-python-backend-vs-effect-audit.md);
[the DSPy-in-Effect history audit](../dspy/2026-07-20-dspy-in-effect-git-history-audit.md);
[the MemoHarness integration analysis](2026-07-18-memoharness-blueprint-integration-analysis.md);
[the continual-learning architecture audit](2026-06-28-continual-learning-architecture-audit.md);
[the prompt-optimization governance note](../loupe/2026-08-01-forensic-prompt-optimization-governance.md).

Router Power (`~/work/projects/repos/router-power`, frozen 2026-09-07), read
for the interventions: `release/opx/harness/playbook_agent.py`,
`release/opx/harness/retrieval_agent.py`,
`release/opx/harness/case_retrieval.py`,
`scripts/multicompany_heterogeneity_improve.py`,
`scripts/multicompany_heterogeneity_common.py`,
`scripts/multicompany_model_swap.py`, `scripts/self_harness_common.py`,
`scripts/self_harness_analyze.py`, `release/patches/README.md`;
and for the findings, the reports the memo's Sources section lists, plus
`writeups/ROUTER_POWER_RESEARCH.md` (the runtime-as-arm recommendation and
the falsification thresholds), `writeups/REVIEW.md` (the removed cascade
arm), and the public writeup that names Specific Intelligence, residual
transferable uncertainty, and outcomes pricing at the router layer
([Soren Larson, 2026-09-10](https://x.com/hypersoren/status/2098130598858551596)).

How this document differs from the memo: the memo maps Router Power's
findings onto the strategy claims and the Coder plan; this document maps
Router Power's interventions onto Coder's mechanisms, uses the research's
own terms for those interventions, and treats a mechanism Router Power never
varied as open rather than as decided.
