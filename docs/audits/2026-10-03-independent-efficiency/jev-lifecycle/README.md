# Jev throughout preparation, evidence gathering, and review

October 3, 2026. Tracking:
[#10356](https://github.com/OpenAgentsInc/openagents/issues/10356).

**Use Jev to help select evidence and route specific uncertainties. The measured
review behavior is too unreliable to determine completion.** This round runs
53 real gateway requests across preparation, source selection, patch review,
and independent-question batching. Reported usage totals **$0.026275620**.
Every response has a valid typed shape; semantic failures remain in the results.

The [runnable prototype](../../../../bench/jev-lifecycle/README.md) and
[recomputable results](results.json) retain all requests, responses, costs,
source identities, and failures. This is a development component study on
exposed tasks. It runs **zero native coding sessions** and establishes no new
cost, time, or acceptance win over bare Claude or Codex.

## Findings

| Component | Observed result | Interpretation |
| --- | --- | --- |
| Package and file admission | The new pool includes the ATIF reader and six SDK implementation units omitted by the earlier previews. Several required boundary functions remain absent. | Deterministic discovery partly repairs coverage. More relevant filenames do not guarantee complete behavioral context. |
| Jev relevance ranking | On the same pool and 16 KiB budget, beta gains the complete ATIF `read` function. Alpha drops a partial implementation while adding enrollment tests; gamma still lacks its key response boundaries. | One concrete selection gain, with retained tradeoffs and failures. |
| Requirement-to-source mapping | All exact task sentences remain available. Some mappings point to candidates that packing drops; others point to background or unrelated validation. | A pointer is a reading suggestion, not proof that the delivered brief covers a requirement. |
| Missing-evidence judgment | Probabilities are 0.86, 0.87, and 0.89 on three known incomplete pools. | Consistent with the observed gaps, but an always-incomplete rule also matches all three. No false-positive or calibration evidence. |
| Six bounded reading choices | Jev matches all six expert-labeled choices; the frozen word-overlap baseline matches five. | A small positive semantic-selection result on three task families. No downstream coding or time result. |
| Open catalog source selection | Two choices are relevant implementation files; gamma chooses introductory TypeSafe guidance. The original reader also starts at line 1 instead of the suggested location. | Selecting the right file and selecting the right source window are separate problems. |
| Broad patch review | Flags **0/6** demonstrated defective patches, and 0/6 patches passing retained checks. | Fails to discriminate defects. All actions recommend executable verification. |
| Focused applied-source review | Flags **5/6** demonstrated defective patches, but also **4/6** patches passing retained checks. | The Gym question imposes an extra implementation requirement; this tally cannot establish valid discrimination against the actual task. Simple guard questions supply useful development examples. |
| Corrected order-invariance question | On the same six Gym states, flags **0/2** demonstrated defects and 0/4 check-passing patches. | Removing the extra implementation requirement also removes defect sensitivity. No completion gate is justified. |
| Four questions together versus separately | Two paired orders show **73.4% lower reported cost** and **70.3% lower combined API time** when batched. Choices and relevance order agree; exact probabilities differ. | A measured API batching benefit. It does not establish a faster coding agent. |
| Batched immutable Git reads | Median assembly falls from 2.052/1.335/2.123 seconds to 0.784/0.437/0.363 seconds, with identical contexts and packs. | A deterministic speed improvement, independent of Jev. |

## The implemented sequence

1. Code resolves the historical commit, package/file anchors, declarations,
   public reading requirements, and bounded source spans. It records omissions.
2. One Jev request ranks the candidate units, maps each exact task sentence to
   an evidence ID or no match, and judges whether implementation evidence is
   missing. These questions share state and cannot see one another's answers.
3. Code builds a 16 KiB brief. A second Jev call chooses one additional file from
   the source catalog. Code retrieves a bounded, source-bound window.
4. Separately, a review component reads an existing candidate and recommends
   revising, inspecting, or running checks. The follow-up asks concrete behavioral
   questions about applied candidate source. Neither stage accepts a patch.

The prototype supplies composable checkpoints, not a new coding executor. The
retained reviews use previously produced candidates; they are not outputs of
the three new preparation runs. This distinction matters when counting a
complete workflow or claiming that a briefing prevented a defect.

Read the [beta Jev briefing](runs/preparation/alternative-beta/jev.md) beside
its [deterministic briefing](runs/preparation/alternative-beta/deterministic.md).
The [preparation analysis](preparation-results.md) explains exact selections,
sentence mappings, original probes, and the separately recorded source-window
repair. The [discovery baseline](discovery-baseline.md) freezes the named-unit
diagnostic before these calls. Three preparation tasks ran; reserved A's syntax
index was not retained locally, so it remains explicitly unrun in this pilot.

The [source-read optimization](source-read-results.md) preserves the original
assembler and compares five alternating warm pairs per task. All non-timing
context fields and both ranking policies' packed bytes remain identical.
A separate [whole-command measurement](whole-cli-timing.json) includes Python
startup, index loading, assembly, file writes, and process exit. Its medians
are **1.057 seconds for alpha, 0.479 for beta, and 0.481 for gamma**; six of nine
runs finish within one second. The full command therefore does not yet meet the
subsecond target on every tested task. These timings use a retained index and
exclude live GitHub fetching, cold indexing, and Jev inference. The
[earlier overlapping test measurement](whole-cli-timing-provisional.json) is
preserved and excluded.

## What the review iteration taught

The first review supplies the public task, complete diff, and common pre-change
source spans. Across 12 cases, the median request takes 0.515 seconds; total
reported usage is $0.007032942. Every case receives `verify`. Even the largest
material-defect probability is below 0.5. This is a negative result, despite
fast responses and valid JSON.

The second iteration applies the patch first and supplies exact final production
source with public contracts. Questions ask about individual behaviors. It
costs $0.008242710 over 12 calls, with a 0.593-second median. Specific examples:

- The missing Noul range check scores **0.15** for implementing that property;
  the passing reference scores **0.95**.
- The missing closed-writer guard scores **0.08**; the passing reference scores
  **0.96**.
- The missing request-aware validation scores **0.48**, versus **0.87** in the
  passing reference. That decision sits close to the development threshold.
- Torn UTF-8 recovery remains a miss at **0.57** for implementing the required
  behavior.
- All four check-passing Gym candidates are flagged by the focused questions.
  An independent audit then finds that the question requires a content-based
  tie breaker, although their stable path ordering can meet the actual task.
  These flags cannot be counted as new demonstrated defects or attributed
  entirely to model errors.

The follow-up uses a recorded rule: flag if any required-behavior probability
is below 0.5. It changes both representation and question granularity after
the audit has already seen the defects. The [follow-up protocol](review-followup-protocol.md)
identifies this as development adaptation. It is not an unseen confirmation,
an isolated prompt effect, or evidence that an agent would make a correct repair.

The [third round](order-question-correction.md) corrects the extra implementation
requirement. It permits stable path or content ordering and considers ordering
performed by callers. The exact six Gym states remain unchanged. All six then
score above 0.5 on every property: **both demonstrated defects are missed**,
and no check-passing candidate is flagged. This round costs $0.004424868.
The model no longer rejects acceptable implementations, but it also does not
detect the actual order-dependence defects. The extra rounds expose both a
question-design error and a remaining model limitation; neither disappears
from the report.

The [case manifest](review-cases.json) has six defective and six check-passing
cases from three families. Cases share implementations and are correlated.
Full diffs, final source spans, and retained executable labels are bound by
hash. Labels and independent checker contents are excluded from requests.
Per-clause probabilities are diagnostics; there are no independent labels for
each clause. Passing retained checks is narrower than universal correctness.

## Batching and accounting

The four-question comparison holds state and question text constant. It runs
batch then sequential, followed by sequential then batch. Across both blocks:

| Delivery | HTTP requests | Reported cost | Summed API time |
| --- | ---: | ---: | ---: |
| Four questions in one request | 2 | $0.000709716 | 1.150 s |
| Four separate requests | 8 | $0.002669268 | 3.873 s |

The chosen source remains `c08`; candidate `c02` ranks above `c01` in every
condition. Score changes are at most 0.07 and gap-probability changes at most
0.02 within a pair. Only one of eight full answer objects matches exactly.
Retain these differences: batching is not a guarantee of identical probabilities.
There are only two blocks on one state, without confidence intervals or a
general latency claim.

The gateway's `cost` is counted once. `marketCost`, `gatewayCost`, and
`surchargeCost` are retained, not summed. These are provider-reported usage
amounts, not reconciled invoices or proof of a cash debit. They are nonzero;
this evidence does not support calling the service free.

All 53 requests return the `typesafe-ai/jev` alias with final provider
`typesafe-ai`. **There are 55 internal provider attempts.** Two requests first
hit a DigitalOcean HTTP 503 and then succeed through TypeSafe: the second
batch request and focused review R12. The reported latency includes these
failovers; their failed-attempt charges are not separately itemized. The client
makes one HTTP attempt and requests no evaluation fallback, but Vercel's own
provider routing still operates. No response establishes an underlying Jev
weight/version pin.

The 53 call timers sum to 29.017 seconds across distinct experiments. This is
not elapsed time for a coding workflow. Preparation, source reads, engineering,
local CPU, and previously executed correctness checks have separate costs.
The earlier direct TypeSafe HTTP 402 and its unknown charge remain retained in
the original study. Successful gateway access removes the need for an owner
funding step for this pilot; it does not settle that older charge.

## What to test next in a coding agent

The strongest next treatment is a **bounded evidence workflow**:

- Code first prepares complete public requirements, source identity, package
  boundaries, relevant declaration relationships, and known test commands.
- Jev selects among specific unresolved questions and enumerated evidence
  probes. Batch independent decisions at a checkpoint.
- Code preserves uncovered requirements and missing implementation pointers.
  A high relevance score cannot declare the brief complete.
- The executor receives source evidence and a concrete task. Jev review can
  suggest a targeted check; actual test outcomes and independent acceptance
  remain authoritative.

Before claiming a coding win, freeze this treatment and compare bare native,
deterministic lean, Jev preparation, Jev runtime checkpoints, and both stages
on fresh tasks with one executor model. Give the lean arms the same checkpoint
opportunities and charge all preparation, reads, review calls, revisions, and
final checks. The original 48-session ranking study remains unsealed and unrun;
its pinned direct-model treatment must not silently become this gateway pilot.

This round strengthens the case for batching and exposes a useful source-ranking
example. It also narrows the theory: cheap semantic decisions need good candidate
construction, precise evidence windows, and independent verification. Adding a
general Jev judgment at every step is not supported by these results.

## Reproduce and inspect

The [tool guide](../../../../bench/jev-lifecycle/README.md) contains runnable
commands. `report.py` recomputes the results without network access and verifies
retained request/response digests. Read the original [protocol](protocol.md),
[review follow-up](review-followup-protocol.md), and [freeze records](freeze/).

The initial review process imports its gateway helper once. A later validation
edit overlaps the last response's completion; the original helper is retained
as [gateway-at-review-launch.py](freeze/gateway-at-review-launch.py). The later
validator independently accepts all 53 saved responses. The original runner
is [retained](freeze/lifecycle-at-round-one.py) before the source-window repair.
No model results were replaced after a code change. The final tool suite has
39 passing offline tests. No product Rust code changed or Cargo build ran.
