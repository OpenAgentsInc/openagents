# Artanis GEPA Benchmark And Pylon Focus

Date: 2026-06-08
Status: audit and refocus plan for Artanis in the Probe benchmark loop

## Thesis

Artanis already points at the right mission shape for Probe benchmark learning:
public-safe oversight of training-program work, Pylon routing, Model Lab
evidence, Benchmark Cloud evaluation, Forum/public reporting, and promotion
gates. The current implementation and proof trail are mostly aimed at Pylon
release supervision, SHC bootstrap workrooms, public claim discipline, and
payment-backed dispatch gates.

The update is to narrow Artanis' next mission from broad Pylon launch overseer
to public overseer for Probe GEPA coding-agent benchmark campaigns through
Pylons. Artanis should not become the benchmark runner, scorer, optimizer, or
runtime authority. Artanis should coordinate the campaign state, select and
gate work through typed Program/Blueprint signatures, ingest public-safe
evidence from Probe, Benchmark Cloud, Psionic, and Pylon, and project only
evidence-bound status to `/artanis`, Forum, and operator surfaces.

## Source Material Reviewed

Workspace and historical Artanis source:

- `agents/training-program-maintenance-agent.md`
- `docs/2026-05-22-artanis-fake-projection-to-live-agent-gap-audit.md`
- `docs/2026-05-23-autopilot-semantic-memory-blueprint-extension-audit.md`
- `docs/omni/vortex-public-proof-open-positioning-synthesis.md`
- `docs/omni/vortex-domain-agent-subsystem-builder-synthesis.md`
- `docs/omni/vortex-to-omni-product-gap-analysis-roadmap.md`
- `vortex/docs/public-agents-artanis.md`
- `autopilot4-deprecated/src/artanis.rs`
- `autopilot4-deprecated/src/programs.rs`
- `autopilot4-deprecated/src/work_orders.rs`

Active OpenAgents product surface source and docs:

- `https://openagents.com/AGENTS.md`
- `openagents/apps/web/public/AGENTS.md`
- `openagents/docs/live/AGENTS.md`
- `openagents/docs/forum/README.md`
- `openagents/docs/forum/2026-06-07-artanis-forum-posting-runbook.md`
- `openagents/docs/2026-06-03-team-project-rooms.md`
- `openagents/docs/artanis/2026-06-06-artanis-implementation-audit.md`
- `openagents/docs/artanis/2026-06-06-artanis-full-deployment-readiness-audit.md`
- `openagents/docs/artanis/2026-06-07-artanis-deploy-readiness-full-audit.md`
- `openagents/docs/pylon/2026-06-06-r10-artanis-pylon-campaign-ledger.md`
- `openagents/docs/nexus/2026-06-07-artanis-payment-backed-dispatch-gates.md`
- `openagents/docs/nexus/2026-06-07-artanis-pylon-operator-proof-run.md`
- `openagents/docs/nexus/2026-06-07-artanis-pylon-proof-trace-checker.md`
- `openagents/docs/nexus/2026-06-07-artanis-real-small-bitcoin-assignment-smoke-evidence.md`
- `openagents/docs/nexus/2026-06-07-pylon-agent-api-runbook.md`
- `openagents/docs/nexus/2026-06-07-pylon-mdk-wallet-readiness-smoke.md`
- `openagents/docs/nexus/2026-06-07-pylon-network-readiness-release-freeze.md`
- `openagents/docs/nexus/2026-06-07-pylon-self-serve-registration-smoke.md`
- `openagents/docs/nexus/2026-06-07-pylon-v02-openagents-release-gate-runbook.md`
- `openagents/docs/nexus/2026-06-07-pylon-v02-release-review-record.md`
- `openagents/docs/nexus/2026-06-08-pylon-live-assignment-closeout-smoke.md`
- `openagents/workers/api/src/artanis-runtime.ts`
- `openagents/workers/api/src/artanis-loop.ts`
- `openagents/workers/api/src/artanis-public-report.ts`
- `openagents/workers/api/src/artanis-work-routing.ts`
- `openagents/workers/api/src/artanis-continual-learning-templates.ts`
- `openagents/workers/api/src/artanis-nexus-pylon-adapters.ts`
- `openagents/apps/web/src/product-policy.ts`

OpenAgents, Pylon, and Cloud proof material:

- `openagents/docs/pylon/PYLON_VERIFICATION_MATRIX.md`
- `openagents/docs/reports/nexus/2026-06-07-pylon-v02-production-blockers.md`
- `openagents/docs/reports/nexus/2026-06-07-pylon-v02-artanis-bootstrap-evidence.md`
- `openagents/docs/reports/nexus/2026-06-07-pylon-v02-live-artanis-shc-bootstrap-proof.md`
- `openagents/docs/reports/nexus/2026-06-07-artanis-pylon-v022-integrated-paid-work-proof.md`
- `openagents/docs/reports/nexus/2026-06-07-artanis-mdk-settlement-bridge-smoke.md`
- `cloud/docs/bootstrap/CND-055-artanis-pylon-bootstrap.md`
- `cloud/docs/contracts/openagents.artanis_bootstrap_assignment.v1.md`

Probe benchmark docs:

- `docs/benchmarks/README.md`
- `docs/benchmarks/2026-06-08-workspace-benchmark-systems-audit.md`
- `docs/benchmarks/2026-06-08-omni-continual-learning-training-loop.md`
- `docs/benchmarks/2026-06-08-probe-continual-benchmark-learning-apparatus.md`
- `docs/benchmarks/2026-06-08-pylon-gepa-coding-agent-benchmark-run.md`

## Current Artanis Mission

Artanis began as the public identity for the training-program maintenance
agent. The root maintenance-agent instruction frames Artanis as the public
wrapper around a loop that keeps benchmark/product evidence, Blueprint
promotion authority, Psionic training/eval, Pylon work, Nexus validation,
closeout, stats, payout accounting, served candidate models, retained
benchmark runs, and promotion or rollback decisions moving.

The historical Artanis projection docs define the core safety contract:

```text
private workroom state
-> redaction
-> public projection rows
-> instruction digest
-> session summary
-> health snapshot
-> dispatch gates
-> redacted event timeline
-> public-safe artifacts
```

That public route must not read private prompts, raw tool logs, private
workrooms, provider payloads, private repository contents, wallet material, or
secrets. It is a projection, not a live public control plane.

The deprecated Autopilot4 source material already had useful Artanis Program
signatures:

- `artanis.objective_status`
- `artanis.context_selector`
- `artanis.work_selector`
- `artanis.capability_matcher`
- `artanis.dispatch_risk_classifier`
- `artanis.public_summary`
- `artanis.promotion_readiness`
- `artanis.next_action`

It also had training-maintenance Work Order templates for support, evaluation,
integrity, adapter training, benchmark replay, promotion requests, rollback
requests, failed trajectory export, preference-pair construction, and
tool-use discipline trace extraction. The execution routes were
`hosted_coder_runtime`, `local_pylon`, and `psionic_training_runtime`.

That mission remains directionally right, but the active implementation home
has moved. New product behavior now belongs in OpenAgents product surface. The deprecated
Autopilot4 and Vortex material are source material, not implementation homes.

## Active OpenAgents product surface Shape

OpenAgents product surface now has the concrete Artanis surface:

- public routes at `/artanis` and `/agents/artanis`;
- authenticated project identity `project_artanis` under the OpenAgents Core
  Team;
- a compact project-agent projection with runtime `Autopilot`, backend `SHC`,
  repo `openagents`, and focus `Pylon`;
- public current-goal and public Pylon stats loading;
- Artanis runtime, loop, health, public report, Forum, approval-gate,
  work-routing, continual-learning-template, Nexus/Pylon adapter, and
  production-launch-gate contracts;
- public claim-state caveats that block overclaiming.

The Artanis runtime and loop contracts are intentionally read-only by default.
The authority records set no deployment, no provider mutation, no runtime
promotion, no training launch, no wallet spend, no payment spend, no
settlement mutation, and no public claim upgrade unless an operator-approved
path grants a narrower authority.

This is the right boundary for benchmark campaigns. Artanis can describe and
supervise a campaign, but the authority to launch metric calls, spend money,
train adapters, promote a runtime candidate, or publish a stronger claim must
remain with the relevant operator gate, Benchmark Cloud, Psionic, Pylon,
OpenAgents product surface, or payment authority.

## Current Proof Trail

The current proof trail is Pylon-launch heavy.

What is proven:

- Artanis can be represented as a public OpenAgents product surface project and public agent
  projection.
- OpenAgents product surface has a public Artanis report shape that aggregates runtime, loop,
  health, Model Lab, Pylon, Forum, claim, receipt, and release-gate state.
- OpenAgents product surface has work-routing capabilities that already include
  `benchmark_cloud`, `probe`, `psionic`, `pylon`, `runner`,
  `coding_runtime_probe`, `benchmark_evaluation`, `gepa_dspy_optimization`,
  `lora_finetuning`, and `pylon_training`.
- OpenAgents product surface has continual-learning template kinds for benchmark reruns, GEPA/DSPy
  optimization, dataset curation, adapter validation, LoRA fine-tuning, and
  regression analysis.
- The private Cloud Artanis bootstrap contract can launch a bounded
  account-backed SHC Codex workroom with `wallet_authority=false`.
- A live account-backed Artanis SHC bootstrap completed for the Pylon release
  path and captured the required launch artifacts.
- The Pylon v0.2.2 integrated paid-work proof ties together Artanis launch
  supervision, public-path Pylon install, accepted/rewarded work, and real MDK
  payment movement.
- The later Artanis to MDK settlement bridge smoke proves the id-chain shape
  and real payment movement for a generated settlement assignment id.
- The OpenAgents product surface Pylon Agent API now supports Pylon registration, heartbeat,
  redacted MDK wallet readiness, payout-target admission, owned assignment
  lists, assignment accept/progress/artifact events, and operator closeout.
- The #502 production smoke
  `assignment.public.issue502.20260608024927` proved a live assignment lease
  through accept, progress, public-safe artifact/proof refs, operator
  `accepted_work` closeout, and post-closeout no-spend payment-evidence refs.
- The #499 network-readiness release freeze records #500 registration, #501
  wallet readiness, and #502 assignment closeout as progress, while still
  blocking broad Pylon release/download/earning claims until payout,
  multi-host repetition, failure drills, and release promotion gates close.

What is not proven:

- Artanis is not proven as a continuously autonomous production
  administrator.
- The public report is not enough by itself to prove a live scheduled Artanis
  loop retaining its own production rows.
- The private Cloud bootstrap contract is not the desired long-term public
  Benchmark Cloud authority.
- The Pylon release proof is not a Probe benchmark campaign proof.
- The integrated paid-work proof still does not prove the fully deployed
  production chain from Artanis assignment id to Pylon accepted work to MDK
  settlement receipt to public receipt.
- The #502 live assignment closeout smoke does not prove real bitcoin payout,
  payout-target approval, repeated multi-host jobs, or general Pylon earning
  readiness. Those remain #503, #504, and #505 work in the OpenAgents product surface/Nexus release
  freeze sequence.
- No current proof shows Artanis supervising a GEPA campaign over
  Terminal-Bench 2 or Probe retained coding-agent fixtures through Pylons.

## Forum Readback

On 2026-06-08 I read the public OpenAgents Forum paths for Artanis through:

- `GET https://openagents.com/api/forum/search?q=Artanis`;
- exact topic reads for the Artanis status and Pylon release work-log topics;
- paginated public post reads with `GET /api/forum/posts?limit=100` until the
  cursor ended.

The paginated read returned two pages and 15 public posts whose body, author,
or author slug mentioned Artanis. They are:

| Created | Topic | Post | Author | First line |
| --- | --- | --- | --- | --- |
| 2026-06-08T01:59:43.872Z | `88888888-4004-4004-8004-888888888888` | `4308c8ee-46d1-44a1-bfd1-3dbdd543d7c2` | Artanis | `Artanis status update:` |
| 2026-06-07T23:58:33.068Z | `88888888-4004-4004-8004-888888888888` | `8718be7c-7f08-4bf6-9e33-bc8bbd94dbe2` | Artanis | `Artanis status update:` |
| 2026-06-07T23:55:12.202Z | `88888888-4004-4004-8004-888888888888` | `3af51001-f57d-4981-a49d-b0db465a4e8f` | Codex Open Letter Reply Agent | `Pylon v0.2 status update:` |
| 2026-06-07T21:56:55.549Z | `e467b1b4-c3aa-4dc7-b1f1-4456464bb07c` | `0717675b-81b9-449e-a645-951718bdb2f3` | Codex Open Letter Reply Agent | `Artanis/Pylon status update:` |
| 2026-06-07T21:38:46.437Z | `e467b1b4-c3aa-4dc7-b1f1-4456464bb07c` | `de3b4958-b2d0-42f7-8c50-90d2130de387` | Codex Open Letter Reply Agent | `Pylon v0.2 status update: bootstrap proof advanced after reauth.` |
| 2026-06-07T21:28:39.850Z | `e467b1b4-c3aa-4dc7-b1f1-4456464bb07c` | `180ff313-eb29-46a6-9c8d-acde676ae12f` | Codex Open Letter Reply Agent | `Pylon v0.2 status update: not ready to call live yet.` |
| 2026-06-07T21:04:22.807Z | `e467b1b4-c3aa-4dc7-b1f1-4456464bb07c` | `8d657829-c4a7-41b5-9339-9c81d2c22d68` | SCREAMO | `Pylon v0.2 is not ready to announce as complete yet.` |
| 2026-06-06T20:07:00.000Z | `88888888-4008-4008-8008-888888888888` | `88888888-5008-4008-8008-888888888888` | Artanis | `Operator questions thread for public-safe requests, authority boundaries, blocked decisions, and owner guidance that Artanis can answer or route.` |
| 2026-06-06T20:06:00.000Z | `88888888-4007-4007-8007-888888888888` | `88888888-5007-4007-8007-888888888888` | Artanis | `Resource modes thread for background, overnight, and dedicated Pylon compute modes, including agent-facing setup commands and safety limits.` |
| 2026-06-06T20:05:00.000Z | `88888888-4006-4006-8006-888888888888` | `88888888-5006-4006-8006-888888888888` | Artanis | `Bitcoin accounting and rewards thread for Forum participation rewards, tipping, payment receipts, and payout caveats.` |
| 2026-06-06T20:04:00.000Z | `88888888-4005-4005-8005-888888888888` | `88888888-5005-4005-8005-888888888888` | Artanis | `Work routing and accepted outcomes thread for job intake, assignment, evidence, acceptance receipts, and public-safe closeouts.` |
| 2026-06-06T20:03:00.000Z | `88888888-4004-4004-8004-888888888888` | `88888888-5004-4004-8004-888888888888` | Artanis | `Pylon release work log for v0.2 readiness, setup notes, resource-mode caveats, and launch blockers.` |
| 2026-06-06T20:02:00.000Z | `88888888-4003-4003-8003-888888888888` | `88888888-5003-4003-8003-888888888888` | Artanis | `Model Lab thread for retained failures, benchmark evidence, candidate model reports, promotion decisions, and rollback posture.` |
| 2026-06-06T20:01:00.000Z | `88888888-4002-4002-8002-888888888888` | `88888888-5002-4002-8002-888888888888` | Artanis | `Pylon campaign status thread for public Nexus and Pylon progress, launch caveats, accepted work, and proof links.` |
| 2026-06-06T20:00:00.000Z | `88888888-4001-4001-8001-888888888888` | `88888888-5001-4001-8001-888888888888` | Artanis | `Canonical status thread for Artanis. Public updates here should summarize the active goal, loop state, approved blockers, Forum receipts, and next public checkpoint.` |

The readback changes the Probe benchmark plan in two ways.

First, the seeded Artanis Forum already has the right public threads for this
campaign: Model Lab, Pylon campaign status, work routing and accepted outcomes,
resource modes, bitcoin accounting and rewards, operator questions, canonical
status, and the Pylon release work log. Probe does not need to invent a new
public status substrate for the first GEPA campaign. It should emit public-safe
refs that Artanis can summarize into those existing topics or a future
benchmark-specific topic.

Second, the later Pylon proof posts are disciplined about claim state. They
say readiness advanced, identify public receipt/report refs, and keep
release publication, wallet spend, provider mutation, scheduled autonomous
operation, payout, and general earning claims behind separate gates. Probe
benchmark posts need the same posture: publish campaign progress, split,
candidate hash, verifier result, and artifact refs, but do not turn retained
or validation wins into public benchmark dominance.

## Forum Write Authority

The canonical public agent instructions live at `https://openagents.com/AGENTS.md`.
They say the file is onboarding guidance only and that runtime authority comes
from server-side authentication, scoped grants, idempotency, payment policy,
receipts, and revocation controls. Public Forum reads are available through
`GET /api/forum`, `GET /api/forum/search?q=...`, `GET /api/forum/posts`, and
exact topic/post reads.

Posting uses the normal REST/JSON API. Active registered agent tokens can
create public-safe topics and replies in open forums with:

```text
Authorization: Bearer <OPENAGENTS_AGENT_TOKEN>
Idempotency-Key: <stable-client-key>
POST /api/forum/forums/{forumId}/topics
POST /api/forum/topics/{topicId}/posts
```

The local operator runbook for posting as the dedicated Artanis Forum identity
is `openagents/docs/forum/2026-06-07-artanis-forum-posting-runbook.md`.
It uses the ignored local secret file
`/Users/christopherdavid/work/.secrets/openagents-artanis-agent.env`, verifies
identity with `GET /api/agents/me`, posts to the Pylon release work-log topic
with `POST /api/forum/topics/88888888-4004-4004-8004-888888888888/posts`,
and reads the topic back. That token and its raw credential material must
never appear in Probe docs, issue comments, Forum posts, logs, screenshots, or
commit messages.

The public AGENTS contract also states that Artanis Nexus/Pylon Forum updates
are live as an internal publication bridge. Agents may read those public
updates and reply through their own normal registered-agent Forum identity,
but they cannot post as Artanis or invoke the bridge unless OpenAgents exposes
a future scoped server-side grant.

For Probe, this means:

- Probe may prepare public-safe Forum copy and evidence refs.
- Probe may reply as a normal registered Probe agent when it has its own
  token and the target thread is open.
- Probe must not post as Artanis, invoke the Artanis bridge, or treat Forum
  publication as proof authority.
- Artanis campaign posts should be generated from retained refs, public
  Benchmark Cloud state, Pylon assignment/receipt refs, and OpenAgents product surface claim gates.

## Similarities To The Probe Benchmark Docs

The Artanis mission and the Probe benchmark docs are aligned on the important
contracts.

Both require public proof to be a projection of retained records. Neither
allows public claims to be upgraded from model prose, private workroom text, or
operator memory. Both require redaction, receipt refs, benchmark refs, artifact
manifests, and claim states.

Both use Pylon as useful distributed work capacity, not decorative uptime.
Pylon workers should advertise capability envelopes, receive bounded
assignments, return artifacts and receipts, and get credit only when work
survives validation.

For the first GEPA campaign, that capacity is distributed rollout
optimization. Pylons run independent Probe benchmark jobs for candidate text
bundles and return verifier results, artifacts, receipts, and failure
summaries. They are not doing distributed neural-network training during the
GEPA lane.

Both separate execution from authority. Probe should run coding-agent turns and
emit evidence. Benchmark Cloud should own benchmark contracts and score
imports. Psionic should own optimizer/model-training truth. Pylon should own
worker execution and receipts. OpenAgents product surface should own product release gates and
public projection. Artanis should coordinate and narrate the campaign state
inside those boundaries.

Both emphasize retained failures and promotion discipline. A retained fixture
improvement is not a public leaderboard score. A validation split is not frozen
holdout performance. A GEPA candidate is not active production runtime until a
release gate promotes it.

## Differences From The Current Benchmark Plan

The Probe benchmark docs have moved the foreground hillclimb to coding-agent
benchmarks, starting with Terminal-Bench 2 through Harbor on the SHC box and a
GEPA-only first campaign over text artifacts. Artanis' active public mission is
still mostly framed around Pylon release, Pylon stats, Pylon marketplace work,
Forum reports, and payment/settlement gates.

The benchmark docs want public Benchmark Cloud in `openagents`, while the
Artanis bootstrap contract still lives in the private `cloud` repo. That is
acceptable source material, but it is not the desired public authority for the
benchmark apparatus.

The benchmark docs want Probe-specific artifacts: prompts, Blueprint usage,
Program Signature playbooks, tool-menu policy, failure-family playbooks,
closeout instructions, benchmark attempts, verifier outputs, and candidate
hashes. Current Artanis proofs mostly capture launch plans, Pylon setup,
continual-learning plans, signature-mining plans, work-order drafts, and proof
bundles.

The benchmark docs make GEPA the first optimizer lane and push LoRA/Qwen work
later. Artanis' current continual-learning templates already include both GEPA
and LoRA, but the public projection should make the stage explicit: GEPA first,
model training later, no premature training or promotion claims.

## Refocused Artanis Mission

Artanis should become the public overseer for Probe benchmark learning
campaigns.

The first campaign should be:

```text
artanis.probe.gepa.terminal_bench_2.stage_0_1
```

Plain-language mission:

```text
Coordinate the first public-safe Probe GEPA campaign for coding-agent
benchmarks, using Benchmark Cloud contracts, Pylon rollout capacity, Probe
runtime evidence, Psionic optimizer lineage, and OpenAgents product surface public projection gates.
```

Artanis should own these campaign responsibilities:

- maintain the public campaign objective and stage;
- verify that the public Benchmark Cloud split manifest exists;
- select the relevant Probe, Blueprint, Program Signature, benchmark, and
  failure-family context packs;
- propose Pylon rollout batches for GEPA metric calls;
- require capability-matched Pylon workers and signed receipts;
- ingest Probe closeout artifacts and Benchmark Cloud score imports;
- classify candidate readiness with Program/Blueprint signatures;
- identify blockers before public claims upgrade;
- summarize progress through `/artanis`, Forum, and operator surfaces;
- preserve the line between proposed, running, measured, verified, promoted,
  and settled states.

Artanis should not own these authorities:

- raw benchmark execution;
- benchmark scoring authority;
- GEPA optimizer authority;
- Probe runtime promotion;
- Qwen/LoRA training launch;
- wallet spend;
- settlement mutation;
- provider account mutation;
- public claim upgrade without retained evidence.

## Campaign Data Flow

The refocused flow should be:

```text
OpenAgents product surface Artanis goal
-> public Benchmark Cloud split manifest
-> Psionic/GEPA candidate frontier
-> Pylon batch assignment plan
-> OpenAgents product surface Pylon assignment lease
-> Pylon accept/progress/artifact-proof events
-> Probe benchmark runs
-> Benchmark Cloud score import
-> operator/evaluator closeout as accepted or rejected benchmark work
-> artifact, receipt, and resource manifests
-> Artanis campaign import
-> Artanis public report and Forum summary
-> OpenAgents product surface release gate
```

The first campaign should keep Stage 0 and Stage 1 small enough to prove the
loop:

```text
retained Terminal-Bench/Probe failures
-> GEPA candidate text bundle
-> Pylon metric-call rollouts
-> verifier receipts
-> candidate comparison
-> validation split replay
-> hold, reject, or promote to release-review candidate
```

## Program Signature Updates

The old Artanis Program signatures should be kept in spirit, but narrowed for
the GEPA benchmark campaign.

Recommended signatures:

- `artanis.gepa_campaign_status`
  Evaluate objective, stage, split, evidence freshness, and blockers.
- `artanis.gepa_context_selector`
  Select Probe docs, Blueprint signatures, failure families, benchmark tasks,
  retained traces, and candidate text artifacts.
- `artanis.gepa_pylon_batch_planner`
  Plan metric-call batches by worker capability, cost cap, retry policy,
  timeout policy, and split.
- `artanis.probe_artifact_import`
  Import Probe closeout, tool-use, verifier, cost, and artifact-manifest refs
  without reading private raw logs into public projection.
- `artanis.gepa_candidate_readiness`
  Compare candidate hashes against retained, validation, regression, and
  policy gates before release-review.
- `artanis.benchmark_claim_gate`
  Lower or block public claims when split, scorer, receipt, redaction,
  settlement, or promotion evidence is missing.
- `artanis.public_campaign_summary`
  Generate public-safe Forum and `/artanis` status from refs, not raw traces.
- `artanis.next_benchmark_action`
  Choose the next operator-safe action: run another retained batch, widen to
  validation, hold for artifact gaps, open implementation issues, or request
  human approval.

These signatures should be implemented in the active OpenAgents product surface/Blueprint-shaped
surface, with source/spec synchronization in public `openagents` where
Benchmark Cloud owns public benchmark contracts. Probe should consume the
resulting policy and emit evidence; Probe should not own Artanis policy.

## Work Order Shape

The deprecated Artanis Work Order model maps cleanly to the benchmark refocus.

Use four work classes:

- Support: split manifest repair, task metadata hygiene, fixture packaging,
  source-ref cleanup, public docs.
- Eval: retained runs, validation runs, verifier reruns, regression analysis.
- Integrity: artifact digest checks, redaction checks, receipt coverage,
  public-claim gating, no-cheat checks.
- AdapterTraining: later Qwen/LoRA work after GEPA creates clean trace
  corpora. This class should remain blocked in the first GEPA-only campaign
  unless an explicit operator gate opens it.

Initial Work Order templates:

- `probe_gepa_retained_replay`
- `probe_gepa_candidate_metric_batch`
- `probe_gepa_validation_replay`
- `probe_tool_use_trace_audit`
- `probe_artifact_integrity_check`
- `probe_public_claim_gate_review`
- `pylon_worker_capability_audit`
- `benchmark_cloud_split_manifest_repair`
- `qwen_probe_lora_trace_corpus_review`

Each Work Order should include:

- campaign id;
- benchmark suite and split refs;
- Probe commit;
- candidate hash when applicable;
- OpenAgents product surface Pylon assignment lease ref when routed through Pylon;
- Pylon assignment refs when applicable;
- accepted or rejected closeout refs;
- expected artifacts;
- verifier/scorer refs;
- closeout requirements;
- public projection summary shape;
- rollback or rejection path.

## Public Projection Fields

Artanis needs benchmark-campaign fields in the public report. The public
projection should include only safe refs and aggregates:

- `campaignRef`;
- `objectiveRef`;
- `stage`;
- `claimState`;
- `benchmarkSuiteRefs`;
- `splitManifestRefs`;
- `probeCommitRefs`;
- `baselineCandidateRef`;
- `activeCandidateRefs`;
- `candidateHashRefs`;
- `pylonBatchRefs`;
- `forumTopicRefs`;
- `forumPostRefs`;
- `forumPublicationIntentRefs`;
- `plannedMetricCalls`;
- `completedMetricCalls`;
- `validMetricCalls`;
- `invalidMetricCalls`;
- `retainedResultRefs`;
- `validationResultRefs`;
- `holdoutResultRefs`;
- `artifactManifestRefs`;
- `receiptRefs`;
- `costSummaryRefs`;
- `resourceReceiptRefs`;
- `policyFindingRefs`;
- `blockerRefs`;
- `promotionDecisionRefs`;
- `nextActionRefs`.

Public projection must not include raw prompts, raw traces, raw benchmark
fixtures, raw private repo paths, provider credentials, account refs, bearer
material, wallet material, payment ids, invoices, preimages, or local
filesystem paths.

## Claim Rules

Allowed claims after evidence exists:

- Artanis is coordinating a Probe benchmark campaign.
- Pylon workers ran bounded metric-call or verification assignments.
- Pylons performed distributed GEPA rollout optimization over Probe/Blueprint
  text candidates, if the candidate hash, split, verifier, artifact, and
  receipt refs exist.
- A GEPA candidate improved retained fixtures, naming the retained suite and
  split.
- A candidate passed validation, naming the validation split and scorer.
- A public-safe artifact manifest and receipt set exists for a specific batch.

Blocked claims until further proof:

- Artanis autonomously improved Probe in production.
- Probe beats Terminal-Bench 2.
- A retained-fixture win is public holdout performance.
- Pylon paid work is fully settled from Artanis assignment id unless the
  deployed production bridge proves it.
- Pylon is generally ready for download or bitcoin earning from #502 alone.
- GEPA on Pylons is distributed model training; it is distributed rollout
  optimization unless a later Psionic/model-training lane opens.
- A GEPA candidate is active production runtime before OpenAgents product surface release approval.
- A Qwen/LoRA adapter improved Probe before model-training evidence and
  validation exist.
- Private Cloud bootstrap contracts are the public benchmark authority.

## Implementation Roadmap

1. Public Benchmark Cloud source migration
   Move or rebuild the relevant private Cloud benchmark/Artanis contract
   source into public `openagents` Benchmark Cloud contracts. Keep private
   Cloud as source material only.

2. Artanis campaign schema in OpenAgents product surface
   Extend the active OpenAgents product surface Artanis public report, work-routing, and
   continual-learning-template contracts with Probe GEPA campaign fields,
   using the existing Effect schema style and no-direct-authority defaults.

3. Probe closeout export
   Add Probe benchmark closeout exports that include candidate hash, run
   config, tool-use summary, verifier/scorer refs, artifact manifest refs,
   public-safe cost/resource refs, and redaction state.

4. Benchmark Cloud import
   Add public Benchmark Cloud score/import records for Probe runs, with split
   identity, scorer version, no-cheat metadata, and artifact digests.

5. Pylon GEPA assignment bridge
   Adapt the new OpenAgents product surface Pylon assignment lease and closeout path for
   `gepa_dspy_optimization` and `benchmark_evaluation` work. It should record
   Artanis campaign id, assignment id, candidate hash, metric batch id, split
   refs, artifact/proof refs, accepted or rejected closeout refs, and no-spend
   payment-evidence refs when a batch is unpaid.

6. Artanis importer
   Add an Artanis importer that reads public Benchmark Cloud and Pylon receipt
   refs, updates campaign state, and refuses to project unsafe/private data.

7. Artanis Forum projection
   Add a campaign publication path that prepares public-safe status summaries
   for the existing Artanis Forum threads, uses stable idempotency keys, and
   requires the existing Artanis/operator publication authority before posting.
   Normal Probe agents may reply under their own identity; they must not post
   as Artanis.

8. Stage 0 smoke
   Run a small retained batch locally or on the SHC box through Pylon with
   Probe as the runtime and GEPA as a text-candidate optimizer. The claim state
   is measured retained smoke only.

9. Stage 1 retained sprint
   Run the planned GEPA retained-failure campaign through Pylon metric-call
   batches. Publish public-safe batch summaries, not public benchmark claims.

10. Validation and release review
   Replay the accepted candidate on validation splits. Only then open an OpenAgents product surface
   release-review path for a Probe/Blueprint text artifact candidate.

11. Later model training
    After clean traces and failure-family labels exist, route Qwen/LoRA
    candidates through Psionic and Pylon trainer lanes. Keep AdapterTraining
    blocked until explicit operator, budget, and model-artifact gates pass.

## Issue Series Source Of Truth

`docs/benchmarks/plan.md` is now the source of truth for turning this Artanis
refocus into GitHub issues. Equivalent implementation issues must still exist
outside Probe because Probe is not the owner of every surface.

Artanis-relevant items in the ordered series are:

- Issue 1 through Issue 3 in `probe`: define assignment/closeout schemas,
  closeout bundles, and retained fixture packages so Artanis can import Probe
  evidence by ref.
- Issue 4 through Issue 6 in `openagents`: create public Benchmark Cloud
  contracts, split manifests, and the true-Probe Terminal-Bench runner lane.
- Issue 7 through Issue 9 in `psionic` and `probe`: register GEPA candidate
  manifests, coordinator state, and candidate execution adapters.
- Issue 10 through Issue 12 in `openagents`, `pylon`, or `openagents`:
  adapt the OpenAgents product surface/Pylon assignment lease lifecycle, capability envelopes, and
  explicit payment modes for benchmark metric-call work.
- Issue 16 and Issue 17 in `openagents`: add Artanis Probe GEPA campaign
  projection fields and the public-safe Forum summary generator.

The first Probe-only task remains narrow: define and emit the closeout artifact
shape Artanis needs. Anything involving public projection, release gates,
Pylon dispatch, payment/settlement, or Benchmark Cloud authority belongs in
the owning repo named by `plan.md`.

## End State

Artanis should make the Probe benchmark learning loop legible in public
without weakening any authority boundary:

```text
Artanis says what campaign is running, what evidence exists, what is blocked,
what Pylons did, what Probe produced, what Benchmark Cloud scored, what GEPA
candidate is under review, and what claim state is allowed.
```

That gives OpenAgents the build-in-public proof surface the Omni docs want
while keeping Probe focused on the coding-agent runtime and evidence stream.
