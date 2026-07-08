# OpenAgents Launch Study Packet v0

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-17
Status: public-retained study packet / internal dogfood only

## Packet Ref

- Packet ref: `repo_study_packet.openagents_launch.v0`
- Planned schema ref: `repo_study_packet.v0`
- Source package ref:
  `dataset.openagents_studybench.public_retained.openagents_launch.v0`
- Source package path:
  `docs/research/machine-studying/openagents-studybench/public-retained/openagents-launch-v0.jsonl`
- Source package digest:
  `sha256:7f115a355c7e5ac3cbe094e9e7389aff2a07f2dcb411e57eb6c441e0756478aa`
- Corpus manifest ref:
  `openagents_repo_corpus_manifest.482866d34.public_retained.v0`
- Source commit ref: `commit.openagents.482866d34`

This packet is a launch editing aid for OpenAgents agents. It is not a trained
model, not a product claim, not a marketplace package, not runtime authority,
not payout authority, and not hidden benchmark evidence.

## Boundary

Included material:

- public-retained StudyBench row ids and topics;
- public source file refs and public line-span evidence refs already committed
  in the retained package;
- authority refs, test refs, forbidden claim refs, and private-material policy
  refs copied from public-retained rows;
- public launch-edit playbooks derived from those refs.

Excluded material:

- private validation rows;
- private holdout rows;
- private task prompts, gold answers, rubrics, evidence excerpts, scorer
  rationales, candidate transcripts, provider payloads, wallet material, or
  customer-private data;
- any claim that this packet proves customer repo studying is live.

## Public-Retained Row Map

| Row ref | Topic | Source authority refs | Primary source files |
| --- | --- | --- | --- |
| `studybench_task.openagents_public_retained.openagents_launch_0001` | launch claims and promises | `authority.openagents.product_promises`, `authority.openagents.machine_studying_docs` | `docs/research/machine-studying/README.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0002` | Tassadar projection truth | `authority.openagents.tassadar_public_projection`, `authority.openagents.product_promises` | `docs/launch/2026-06-17-tassadar-live-page-accuracy-audit.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0003` | settlement and wallet truth | `authority.openagents.launch_payout_audit`, `authority.openagents.product_promises` | `docs/launch/2026-06-17-orrery-payout-accounting-and-spark-unification-audit.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0004` | Customer #1 evidence | `authority.openagents.customer_one_cohort`, `authority.openagents.launch_audit` | `docs/launch/2026-06-17-customer-one-dogfood-audit.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0005` | Forge Coder repo memory | `authority.openagents.forge_coder`, `authority.openagents.action_submission` | `docs/launch/2026-06-17-machine-studying-short-term-roadmap.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0006` | Blueprint, Probe, and GEPA contracts | `authority.probe.benchmark_candidate_execution`, `authority.blueprint.contribution_gates` | `docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0007` | Pylon assignment and wallet readiness | `authority.openagents.launch_roadmap`, `authority.openagents.pylon_release_gates` | `docs/launch/2026-06-17-machine-studying-short-term-roadmap.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0008` | StudyBench schema adaptation | `authority.openagents.studybench_contracts` | `docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0009` | answer and patch modes | `authority.openagents.studybench_mvp_scope`, `authority.probe.runner_contracts` | `docs/research/machine-studying/README.md`, `docs/research/machine-studying/2026-06-17-openagents-studybench-mvp-issue-roadmap.md` |
| `studybench_task.openagents_public_retained.openagents_launch_0010` | product promise and marketplace gates | `authority.openagents.product_promises`, `authority.blueprint.marketplace_gates` | `docs/launch/2026-06-17-machine-studying-short-term-roadmap.md` |

## Authority Map

- Product claims: use `authority.openagents.product_promises`. Public copy may
  mention upstream StudyBench only as external public calibration and
  OpenAgents public-retained rows only as examples/regression fixtures.
- Tassadar public projection: use
  `authority.openagents.tassadar_public_projection`. Motion, counters, beams,
  and payout bursts need public refs or timestamped live-state transitions.
- Payout and wallet copy: use `authority.openagents.launch_payout_audit`.
  Treasury rows without recipient-side evidence cannot prove exact receipt or
  exact wallet balance.
- Customer #1 dogfood: use `authority.openagents.customer_one_cohort` and
  `authority.openagents.launch_audit`. Internal rows count only when public-safe
  completion and privacy gates pass.
- Forge Coder study-packet projection: use `authority.openagents.forge_coder`
  plus `authority.openagents.action_submission`. The packet can be selected by
  typed context/retrieval planning but cannot mutate code.
- Blueprint and Probe gates: use `authority.blueprint.contribution_gates`,
  `authority.probe.benchmark_candidate_execution`, and
  `authority.probe.runner_contracts`. Candidate text cannot grant itself
  runtime promotion, public-claim authority, signature expansion, or tool
  expansion.
- Marketplace gates: use `authority.blueprint.marketplace_gates`.
  Marketplace package claims require separate validation, metering, pricing,
  privacy, payout eligibility, and settlement gates.

## Invariant Map

- Public projection is evidence-only. It does not grant runtime, deployment,
  merge, accepted-work, payout, settlement, provider-account, product-promise,
  or customer-success authority.
- Public StudyBench rows are not hidden benchmark rows. They may train docs,
  examples, loaders, and regression tests, but they cannot alone support a live
  product claim.
- Candidate agents must not see private gold answers, private rubrics, private
  evidence excerpts, or hidden holdout manifests.
- Bounded retrieval must be exact, structured, semantic, model-selected, or
  hybrid through a typed plan. Do not add ad hoc keyword routing.
- Release readiness for Blueprint StudyBench contributions requires approved
  review, fixture refs, release gate refs, target refs, and retained failure
  refs.
- Action Submission proposals may cite StudyBench closeout refs as evidence,
  but they remain pending approval and carry no direct execution authority.

## Trap Catalog

These are forbidden claim refs from the public-retained package. Treat them as
launch-edit tripwires:

- `blocked_claim.repo_studying_public_product`
- `blocked_claim.trained_repo_expert_model`
- `blocked_claim.anonymous_motion`
- `blocked_claim.fake_live_settlement_burst`
- `blocked_claim.recipient_confirmed_without_receipt`
- `blocked_claim.exact_wallet_balance_without_evidence`
- `blocked_claim.customer_one_without_privacy_review`
- `blocked_claim.cohort_projection_grants_authority`
- `blocked_claim.study_packet_mutates_code`
- `blocked_claim.raw_private_corpus_projection`
- `blocked_claim.keyword_routing`
- `blocked_claim.runtime_promotion_from_gepa`
- `blocked_claim.assignment_signature_bypass`
- `blocked_claim.repo_studying_blocks_gate_2`
- `blocked_claim.second_settlement_authority`
- `blocked_claim.public_trained_repo_agent`
- `blocked_claim.unweighted_rubric`
- `blocked_claim.missing_evidence_span`
- `blocked_claim.answer_only_repo_expertise`
- `blocked_claim.private_gold_in_candidate_context`
- `blocked_claim.customer_repo_studying_live`
- `blocked_claim.marketplace_package_quality`
- `blocked_claim.machine_studying_payouts`

No private validation or private holdout failure pattern is included here. No
run-derived failed attempt is included until a public-retained Probe closeout
adds a retained failure ref.

## Test Catalog

Use these refs when deciding whether an edit can claim readiness:

- `test.probe.studybench_public_retained_fixtures`
- `test.probe.studybench_contracts`
- `test.probe.studybench_answer_runner`
- `test.probe.studybench_patch_runner`
- `test.probe.benchmark_candidate_execution`
- `test.probe.blueprint_contribution`
- `test.openagents.tassadar_live_page_smoke`
- `test.openagents.payout_claim_copy_review`
- `test.openagents.customer_one_cohort_audit`
- `test.openagents.forge_coder_study_packet_projection`
- `test.openagents.launch_claim_review`
- `test.openagents.product_promise_copy_gate`

Passing a StudyBench row is not the same as passing a launch gate. Launch claims
still need their owning product-promise, public-projection, settlement,
privacy, and deployment gates.

## Launch-Edit Playbooks

### Public Repo-Studying Copy

Allowed copy shape:

- upstream StudyBench is external public calibration;
- OpenAgents public-retained rows are examples and regression fixtures;
- repo studying remains internal dogfood until private validation/holdout,
  product-promise, privacy, and projection gates exist.

Rejected copy shape:

- "trained repo expert model";
- "customer repo studying is live";
- "agents can now edit any repo awesomely";
- "public retained scores prove product quality".

### Tassadar Public Scene

Before adding motion, check whether each moving mark has a public source ref or
a timestamped live-state transition. Static aggregate structure is allowed.
Anonymous motion is not. Payout bursts require public settlement refs and
`realBitcoinMoved:true`.

### Payout And Wallet Review

Use settled treasury rows as evidence of ledger state, not proof of recipient
receipt, when destination or recipient-side evidence is missing. Do not claim
exact wallet balance from split payout rails or inferred attribution.

### Customer #1 Dogfood

Internal dogfood can count only when the row has completion evidence, privacy
review, public-safe projection, and the public audit gate. The cohort projection
is evidence-only and grants no operational authority.

### Forge Coder Study-Packet Projection

Expose refs only: `studyPacketRef`, `corpusManifestRef`, `generatedAt`,
`sourceAuthorityRefs`, `changedProfileKinds`, `freshness`, `blockerRefs`,
`examAttemptRefs`, and `expertiseCurveRefs`. Do not expose raw private corpus
content and do not imply the packet can mutate code.

### Blueprint, Probe, And GEPA

Reject candidate text that expands signatures/tools outside the assignment,
claims runtime promotion authority, or claims public-claim authority. A
StudyBench score can feed Psionic optimizer feedback refs, but runtime
promotion stays behind separate Blueprint and release gates.

### StudyBench Row Authoring

A public-retained row needs upstream-compatible id, topic, question,
gold-answer, rubric, and evidence fields plus OpenAgents extension fields
(`repo`, `commit`, `corpusRef`, `visibility`, `authorityRefs`, `testRefs`,
`forbiddenClaimRefs`, `privateMaterialPolicyRefs`, `expectedFiles`,
`budgetClass`). Rubric weights must sum to 100 and every claim span id must
resolve to a public evidence span.

### Answer Mode And Patch Mode

Answer mode tests source-grounded codebase understanding. Patch mode tests
agentic editing under a pinned checkout, allowed tool menu, budget, tests,
patch artifacts, and Probe closeout. Do not put private gold answers or private
rubrics in candidate context.

### Marketplace And Product Promise Review

Measured public-retained lift can justify a planned/yellow product-promise
discussion. It cannot publish a marketplace package or live customer-repo
studying claim. Marketplace claims need validation, metering, pricing, privacy,
payout eligibility, and settlement gates.

## Glossary

- `repo_study_packet.openagents_launch.v0`: this packet's public ref.
- `openagents_repo_corpus_manifest.482866d34.public_retained.v0`: the public
  corpus manifest ref used by the launch rows.
- `openagents_public_retained`: public examples and regression fixtures with
  public gold/rubric/evidence material.
- Private validation split: private scorer/evaluator split; may tune evaluator
  wiring under the private-boundary policy.
- Private holdout split: private hidden split; cannot feed study packets, GEPA
  training, or public product proof.
- `rubric_score.probe.studybench*`: score artifact refs; not raw judge
  rationale.
- `probe_closeout.probe_run.studybench*`: Probe closeout refs; evidence only.
- `Action Submission`: approval-gated proposal path for external writes.
- `forbiddenClaimRefs`: refs that name what copy or code must not imply.

## Use

Mount this packet only as public-retained context for internal dogfood runs.
When a candidate output changes launch copy or launch-adjacent code, evaluate
the output against the row map, authority map, trap catalog, and tests above.
Do not use this packet as private validation, private holdout, payout, public
claim, or customer-ready product evidence.
