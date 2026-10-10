# Self-improving codebases: full product and codebase audit

Date: October 10, 2026. Source: `07805e6a7c3513a057d226b488cb2d40fd974a64`.

**OpenAgents has substantial working parts for a self-improving codebase product, but the reviewed evidence does not yet establish a reliable closed learning loop, two successive recursive gains, or a qualified customer service that delivers those gains.** The shortest path is to connect and harden the existing parts, then prove the result on OpenAgents and a second repository. It is not to build another general agent framework or finish every adjacent product.

The audit reviewed the complete requested product/sales/prior-audit corpus: 49 files, about 243,000 words. It screened 47 open and 917 recently closed issues, followed relevant issue discussions, inspected the current execution, learning, accounting and authority paths, and checked retained trace/corpus evidence. The [method](10-method-and-verification.md) distinguishes static findings, historical measurements, local checks and proposals. Findings describe the pinned revision; later commits may change them.

## What is already valuable

The repository has a useful historical file finder, a briefed coding runner, structured agent traces, independent replay tooling, a partitioned file corpus, calibration and protected-study machinery, local/remote execution infrastructure, and unusually explicit sales acceptance and cost records. These are practical assets, not just specifications.

Earlier experiments show real cost reductions for some constrained Claude configurations. They also show slower routed execution, failed acceptance assumptions and poor model-review detection. The newest two-issue briefing pilot is promising but does not supersede those results or complete its larger experiment. The [evidence chapter](04-evidence-and-experiments.md) preserves each cohort's scope instead of combining them into a favorable headline.

The strongest existing contracts should be reused: the established issue flow's repair/landing behavior, the environment verifier's complete failed/incomplete states, Gym's protected confirmation, sales evidence's full attempt inventory and independent acceptance, finance's unknown-cost handling, and account/host authority boundaries. The experimental path does not inherit these merely because they exist elsewhere in the repository.

## The most consequential findings

| Finding | Why it matters | Evidence and next action |
| --- | --- | --- |
| Structured `verify` can return success after a failed or unavailable check | The system can stop work on a false success signal | [RUN-01](02-execution-and-verification.md); preserve exit/transport states and require the complete final check plan |
| Verification cache omits new-file bytes and staged content | A changed candidate can reuse an old verdict | RUN-02; bind the complete candidate tree and evaluator identity |
| New issue-run can open a PR after failed final checks; concurrent runs share a destructively replaced worktree | Delivery and retained work are not reliably tied to successful execution | RUN-03/RUN-06; reuse stronger existing execution/landing owners |
| A restricted tool list does not protect the evaluator or sandbox generated code | An agent can affect tests/build behavior even without a shell tool | RUN-04 and [CODE-02](07-codebase-leverage.md); protect authoritative checks and bound execution |
| Ranker retraining writes an all-data model directly to the shipped model path | The promised locked-set improvement and calibration gate is bypassed | [LEARN-01](03-learning-and-evaluation.md); immutable candidates and checked activation |
| Replayed traces conflict with the corpus's partitions | All 41 verified traces would be assigned training despite their issue groups being calibration/development | LEARN-02; join exposure/partition rules before combining feeds |
| Feedback can affect retrieval without independent replay | “Only verified outcomes teach the system” is not enforced across inputs | LEARN-04; distinguish observations from qualified learning labels |
| “Relevant file” labels actually mean changed-in-fix, with a different class balance from production | Accuracy and calibrated confidence can be misinterpreted | LEARN-03/06; define the target and evaluate the actual candidate distribution |
| Missing runner cost becomes zero in trace capture | The headline cost-per-accepted-change metric can be understated | [PRODUCT-04](01-product-and-commercial-readiness.md); preserve unknown components and complete failed-attempt costs |
| Training corpus deletion leaves arbitrary teacher JSON | The data lifecycle is incomplete before customer learning | [DATA-03](06-customer-data-and-authority.md); content-minimal tombstones and derivative cleanup |
| Finder cache/feedback lacks adequate repository/workspace identity | A second repository or customer can share state unintentionally | DATA-04; stable namespaces and scoped intake before S5 |
| Consent, replay, learning, serving and withdrawal are separate implementations | No single source proves the customer's agreed purpose follows all derived data | DATA-01/02/05/08; qualify the complete data lifecycle |
| Reviewed sales qualification artifacts are synthetic and the new runner is not joined to full sales evidence | Working accounting code does not establish a delivered profitable pilot | PRODUCT-01/02/03/06; one bounded buyer and exact acceptance/cost packet |

The verifier finding is **P0 for product use that treats it as acceptance**. This is not a finding that the independent A/B grader accepted all false positives: that grader separately checks actual exits and required test counts. The chapters identify this and other compensating controls. Likewise, the corpus overlap is a demonstrated integration hazard, not evidence that a deployed checkpoint has already consumed the conflicting feeds.

## What the numbers really say

| Number | Supported interpretation | Unsupported interpretation |
| --- | --- | --- |
| 95.2% finder recall | Existing handwritten changed files in a top-400 shortlist on 100 historical issues | 95.2% correct patches, complete task coverage, or all semantically relevant files |
| 80/100 complete existing-file coverage | All existing changed files appeared for those cases | All new files, generated changes, behavior and documentation were covered |
| 85% on eight newer fixes | Narrow later development check with known misses | A large untouched prospective evaluation |
| 41 verified traces | Reproducible outcomes across three issues: 22 passing and 19 failing checks | 41 accepted PRs or 41 independent tasks |
| 10,234 corpus rows / 820 issue groups | Reproducibly constructed historical edit-membership examples | 10,234 independent gold relevance judgments or proof of model usefulness |
| Roughly 2–5× pilot cost difference | Early two-issue signal, not yet faster | Completed S2, total customer savings or recursive improvement |
| 709 prior health findings | Overlapping historical observations with documented subsequent fixes | 709 current independent defects to repair automatically |
| 917 recent closed issues | Substantial implementation activity in the recorded seven-day window | 917 independently accepted improvements produced by this product |

The trace manifest and N8 artifact hashes were checked locally. Seven existing offline trace tests passed. Historical model and hardware runs were not repeated. See [evidence](evidence/README.md) for the exact counts, identities and validation output.

## Readiness against the product's own milestones

| Gate | Assessment at this snapshot |
| --- | --- |
| S1: 20+ V1 issues through the cloud loop | Components and some operational evidence exist; a complete qualifying inventory was not found |
| S2: at least 30% cheaper than bare Claude Code, equal/better success, no worse median time | Larger #11211 comparison remains open; early results are insufficient |
| S3: two consecutive measured retraining improvements | Not demonstrated; training intake and promotion have concrete integration gaps |
| S4: connected Pylons, no required Jev key, Vertex-only fallback | Work remains open; spec and #11225 disagree about an intermediate hosted-Clef fallback |
| S5: second repository with its own corpus/gates | Not demonstrated; repository namespace and data admission need work first |
| S6: accepted pilot plus the customer's improvement chart | Detailed assisted-pilot machinery exists; synthetic qualification is not real customer proof and training needs separate permission |
| S7: self-service accepted-PR service | Foundations exist; complete customer flow, isolation, economics and data controls remain unqualified |

“Not demonstrated” means the reviewed evidence does not substantiate the gate. It does not imply no private customer or operator evidence exists. A reviewed private packet can establish a result without publishing customer content.

## The next decisive work

First repair the success signal, candidate identity, final-check gating and worktree ownership. Join complete attempts and costs to independent replay. Then produce the S1 inventory and finish the matched S2 comparison. In parallel, stop direct ranker publication, reconcile feedback partitions and enforce the data boundary.

Only then attempt two protected learning cycles. The spec's fixed-heldout wording conflicts with repeated protected confirmation: this audit recommends retaining a fixed development trend while using fresh protected confirmation for each promotion, or agreeing on a preregistered sequential protocol. That is an explicit proposed refinement, not a quiet change to the milestone.

The [32-action roadmap](09-roadmap-and-acceptance.md) gives owners, dependencies and acceptance criteria. It keeps ordinary issue development fast, preserves the no-GitHub-automation rule, and leaves owner-only activation out of code-completion blockers. A small manual pilot can test demand earlier under its existing contract; it must not promise unproved recursive gains or silently change who pays for failed attempts.

## Full audit

| Chapter | Questions answered |
| --- | --- |
| [01. Product and commercial readiness](01-product-and-commercial-readiness.md) | What is the offer, what counts as acceptance, what is actually qualified, and can its economics work? |
| [02. Execution and verification](02-execution-and-verification.md) | Can the runner safely produce, check, preserve and deliver a candidate? |
| [03. Learning and evaluation](03-learning-and-evaluation.md) | What learns, from which labels, under what partitions and promotion rules? |
| [04. Evidence and experiments](04-evidence-and-experiments.md) | Which results are real, what do their denominators mean, and what experiment would prove recursion? |
| [05. Dogfood and operations](05-dogfood-and-operations.md) | Can our actual computers, cloud, integrations and deploys run the loop reliably and affordably? |
| [06. Customer data and authority](06-customer-data-and-authority.md) | What permits each action and data use, how do tenants stay separate, and what happens on withdrawal? |
| [07. The codebase as the first customer](07-codebase-leverage.md) | Which repository capabilities and health improvements matter, and what should remain outside the critical path? |
| [08. Issue map](08-issue-map.md) | How do all open issues and important recent closures affect the assessment? |
| [09. Roadmap and acceptance](09-roadmap-and-acceptance.md) | What should ship next, in what order, with which falsifiable acceptance criteria? |
| [10. Method and verification](10-method-and-verification.md) | What was read, checked and not run, and how can the retained audit evidence be verified? |
| [Evidence index](evidence/README.md) | Source hashes, issue inventory, inherited findings, local checks and a runnable validator |

There are 58 named findings/assessment topics across the seven analytical chapters. Some overlap deliberately: for example, teacher-field deletion has both learning and customer-data consequences. The roadmap consolidates those overlaps into 32 proposed actions. Neither count is presented as a count of independent exploitable bugs.
