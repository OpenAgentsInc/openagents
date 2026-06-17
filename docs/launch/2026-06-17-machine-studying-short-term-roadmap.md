# Machine Studying Short-Term Launch Roadmap

Date: 2026-06-17
Status: launch proposal, not product copy
Scope: tying machine studying into the June 17 short-term roadmap without
changing the current v1.0, payout, Tassadar, product-promise, or marketplace
claim states.

## Source Material Reviewed

Launch folder:

- `docs/launch/JUNE15_LAUNCH_PLAN.md`
- `docs/launch/JUNE16_ROADMAP.md`
- `docs/launch/JUNE17_ROADMAP.md`
- `docs/launch/2026-06-17-customer-one-dogfood-audit.md`
- `docs/launch/2026-06-17-orrery-payout-accounting-and-spark-unification-audit.md`
- `docs/launch/2026-06-17-tassadar-live-page-accuracy-audit.md`
- `docs/launch/2026-06-17-tassadar-training-run-visual-language.md`

Machine-studying source docs:

- `docs/research/machine-studying/research-note.md`
- `docs/research/machine-studying/2026-06-17-blueprint-marketplace-ties.md`
- `docs/research/machine-studying/2026-06-17-tassadar-openagents-repo-studying-roadmap.md`
- `docs/research/machine-studying/2026-06-17-studybench-openagents-benchmark-audit.md`

External benchmark reference:

- StudyBench dataset: `https://huggingface.co/datasets/jacobli/studybench`

## Short Version

Machine studying should enter the short-term roadmap as an internal dogfood and
Forge Autopilot Coder acceleration lane, not as a new public training claim.

The immediate wedge is:

1. Freeze a public-safe `openagents` repo corpus manifest.
2. Generate a launch-focused `BlueprintStudyPacket` for the repo.
3. Build a hidden repo-edit exam around the exact short-term launch surfaces:
   payout/settlement truth, `/tassadar` projection truth, Customer #1 evidence,
   product-promise copy gates, and Forge Autopilot Coder repo memory/retrieval.
4. Measure whether agents edit the repo better with the packet mounted.
5. Use Tassadar only for deterministic corpus/refinery artifacts at first:
   manifests, redaction checks, source-map indexes, exam manifests, and
   replayable scoring material.

This fits the launch plan because Customer #1 is OpenAgents itself, #5107 is
already the productization lane for the Forge Autopilot Coder, and `/tassadar`
already has the receipt-first visual/projection discipline that repo studying
should inherit.

## Launch Constraint

Do not let machine studying block the current v1.0 cut gate.

The near-term launch priorities remain:

- Gate 2: real settlement / bounded auto-payout dispatch.
- Signed RC and stable v1.0 readiness.
- Built-in agent from-install smoke.
- Forge Autopilot Coder productization under #5107.
- Honest `/tassadar` public run projection.

Machine studying should help agents work on those surfaces more accurately and
cheaply. It should not create a second launch, a second settlement authority, or
a public "trained repo agent" promise.

## Why This Belongs In Launch

The launch docs already establish three patterns that map directly to machine
studying.

First, Customer #1 is OpenAgents. The #5104 closeout accepted internal dogfood
rows when they passed the same completion, privacy-review, public-safe
projection, and audit rules as an external cohort. Repo studying is the same
shape: prove it internally before productizing it.

Second, #5107 is already building the Forge Autopilot Coder around diff review,
plan/todo receipts, session control, repository memory, bounded retrieval, and
guarded extensibility. Machine studying gives that work a sharper metric:
expertise lift on hidden repo-edit tasks, not just feature presence.

Third, the Tassadar launch work is strict about evidence. The run page only gets
to show refs that exist, settlement simulation cannot animate like real Bitcoin,
and accepted trace corpus growth is not a trained-model claim. The same labeling
rule should apply to repo studying: a study packet can improve agent behavior,
but it is not exact execution, settlement authority, or a model claim.

## Proposed Launch-Scoped Workstream

Working name:

`machine_studying.openagents_launch_repo.v0`

Related artifacts:

- `openagents_repo_corpus_manifest.v0`
- `openagents_launch_study_packet.v0`
- `machine_studying_openagents_launch_exam.v0`
- `openagents_studybench.v0`
- `machine_studying_openagents_launch_attempt_receipt.v0`
- `tassadar.repo_refinery_artifact.v0`

### Workstream 1: Corpus Freeze

Create a reproducible manifest for the `openagents` repo at a pinned commit.
This is private/internal at first, with public-safe digest refs available later.

Admit launch-critical source:

- `AGENTS.md`, `INVARIANTS.md`, and child repo instructions.
- `docs/launch/`.
- `docs/promises/`.
- `docs/research/machine-studying/`.
- `docs/tassadar/`.
- `docs/autopilot-coder/terminal-agent-systems/`.
- `apps/openagents.com/workers/api/src/public-tassadar-run-summary-routes.ts`.
- Training-run authority, settlement, product-promise, pylon-stats, Customer #1,
  and Blueprint code paths.
- Forge Autopilot Coder web projection/retrieval/repo-memory code and tests.
- Pylon wallet/readiness docs and tests relevant to v1.0 gates.

Exclude:

- `.git/`, `.claude/`, `.pylon-local/`, `tmp/`, caches, build outputs,
  `node_modules/`, `dist/`, DMGs, tarballs, local runtime state, and secrets.
- Raw prompts, provider payloads, wallet material, payment material, private
  customer data, raw logs, or raw local paths.

Acceptance:

- Every admitted file has path, size, digest, kind, and source authority.
- Every exclusion class is listed.
- A redaction check fails closed before any public-safe projection exists.
- The manifest can be regenerated from the same commit with the same digests.

### Workstream 2: Launch Study Packet

Generate a `BlueprintStudyPacket` for launch work. This is not a context dump.
It should be a compact operating artifact for future agents.

Required sections:

- Launch source map: what each launch doc controls and which docs are historical.
- Authority map: product promises, settlement, payout, public projection,
  Customer #1 rows, `/tassadar`, Forge Coder, and Pylon release authority.
- Claim boundary map: what can be said now, what is yellow/red/planned, and what
  must not appear in public copy.
- Trap catalog: simulation settlement as real Bitcoin, fake motion on
  `/tassadar`, stale projections, owner nodes as independent proof, raw payment
  refs, direct Program Run mutation, and ad hoc keyword routing.
- Test command catalog: focused commands for public Tassadar summary,
  product-promises, Customer #1 cohort projection, Forge Coder web tests,
  Pylon wallet tests, and docs-only checks.
- Launch edit playbooks: adding a launch audit, updating `/tassadar` copy,
  extending Customer #1 evidence, adding Forge Coder repo-memory/retrieval
  surfaces, and touching payout/settlement copy without overclaiming.
- Ref glossary: run refs, promise refs, receipt refs, cohort refs, study packet
  refs, corpus refs, settlement refs, and verification challenge refs.

Acceptance:

- A future agent can orient on the launch roadmap without rereading all 4,000
  lines of launch docs.
- The packet names source authorities instead of summarizing them as generic
  "docs say."
- The packet carries caveats and stale-state rules, not just happy-path steps.
- The packet has no raw private material.

### Workstream 3: Hidden Launch Repo-Edit Exam

Build an internal exam that evaluates whether studying helps agents edit the
actual launch surfaces.

The exam should use a StudyBench-compatible row shape instead of an ad hoc
rubric format. Each task should have a stable id, topic, question, gold answer,
weighted core/supporting rubric claims, and source evidence spans from a pinned
`openagents` commit. OpenAgents-specific extensions should add corpus refs,
authority refs, test refs, forbidden-claim refs, visibility tier, and private
material policy refs.

The public `jacobli/studybench` dataset should be used as an external
calibration lane, especially the `dspy` subset for DSPy/GEPA behavior. It is not
a hidden benchmark, so it should not be used as standalone product-claim
evidence. The launch exam should become our own `openagents_studybench.v0`
slice, with public-retained rows for examples/regression and private validation
or holdout rows for real lift measurement.

Task families:

- Product-promise copy: rewrite a launch claim so it maps to the current promise
  state and cites the right caveat.
- `/tassadar` truth: add or review a projection/display change while preserving
  public refs, staleness, simulation caveats, and no anonymous motion.
- Settlement truth: distinguish settlement recorded, real Bitcoin moved,
  recipient-confirmed, pending, failed, and expired.
- Customer #1 evidence: add or audit a cohort-style evidence row while preserving
  public-safe privacy rules.
- Forge Coder #5107: extend repo memory, bounded retrieval, or guarded
  extensibility without creating write authority or keyword routing.
- Built-in agent: review a from-install smoke or readiness claim without
  promoting `autopilot.builtin_compute_agent.v1` ahead of evidence.
- Docs synthesis: write a launch audit that does not reopen closed work or
  misroute new work into deprecated systems.

Budgets:

- Baseline without study packet.
- Study packet mounted.
- Study packet plus source-map hints.
- Study packet plus retained failure examples.

Score:

- Hidden deterministic checks and focused tests.
- StudyBench-style weighted claim rubric for authority, claim discipline, edit
  scope, privacy, evidence use, and expected tests.
- Tokens, wall-clock time, tool calls, wrong-file reads, and first divergence.
- Reach versus use: did the right source refs enter the trajectory, and were
  they applied correctly?

Acceptance:

- At least 20 hidden tasks exist before claiming lift.
- Each task has source evidence spans from the pinned corpus manifest.
- Public-retained rows are separated from private validation and holdout rows.
- Baseline and study-packet runs use the same budget classes.
- The report distinguishes real lift from memorizing answer keys.
- Failed attempts become retained failure refs only after labeling.

### Workstream 4: Forge Autopilot Coder Integration

Mount the launch study packet as a refs-only repository-memory/context artifact
for #5107.

Near-term UI/projection shape:

- `studyPacketRef`
- `corpusManifestRef`
- `generatedAt`
- `sourceAuthorityRefs`
- `changedProfileKinds`
- `freshness`
- `blockerRefs`
- `examAttemptRefs`
- `expertiseCurveRefs`

The Forge Coder should show that the packet exists and whether it is fresh, but
should not show raw private corpus content or imply the packet can mutate code by
itself.

Acceptance:

- The packet can be selected by a typed context/retrieval planner.
- It does not bypass Action Submission, session control, approval, payout,
  deployment, public-claim, or source-authority gates.
- Bounded retrieval remains exact/structured/semantic/model-selected/hybrid by
  typed plan, not ad hoc keyword routing.
- Product UI labels the lane as internal dogfood until measured lift exists.

### Workstream 5: Tassadar Deterministic Refinery

Use Tassadar-style exactness only where it is actually true.

Good deterministic refinery candidates:

- Corpus manifest generation.
- Redaction/exclusion validation.
- Source-map index generation from manifest refs.
- StudyBench-style evidence span extraction and hashing.
- StudyBench-style row-schema and split-manifest validation.
- Hidden-exam manifest generation without answer leakage.
- Attempt receipt normalization.
- Scoring harness replay for deterministic checks.

Bad candidates:

- Claiming the model's repo edit is exact.
- Claiming the study packet is a trained model.
- Claiming repo studying is public distributed training.
- Claiming marketplace package quality before validation, metering, pricing,
  payout eligibility, and settlement.

Acceptance:

- Deterministic artifacts can emit accepted/rejected verifier receipts.
- Statistical agent edits stay in Probe/Forge evidence, not Tassadar exact
  language.
- `/tassadar` public run copy does not change because of repo studying unless a
  specific public-safe ref and promise gate exist.

## Short-Term Sequencing

### Now: before stable v1.0

Do:

- Add the corpus manifest design and launch study-packet plan.
- Keep work docs/schema-first unless it directly helps current launch gates.
- Add no public promise flip.
- Add no marketplace UI.
- Add no new settlement or payout authority.

Do not:

- Block Gate 2.
- Touch real-money flows except through the existing Gate 2 lane.
- Add public "repo expert" marketing copy.
- Describe machine studying as model training.

Exit criterion:

- The plan is concrete enough that an agent can start corpus manifesting and exam
  construction without rereading the whole launch folder.

### Next RC window

Do:

- Generate `openagents_repo_corpus_manifest.v0`.
- Create `openagents_launch_study_packet.v0`.
- Create the first 20 hidden launch repo-edit tasks.
- Run baseline attempts through the existing agent workflow.
- Record public-safe evidence refs for attempts, tests, failures, and caveats.

Exit criterion:

- Baseline expertise curve exists.
- Study packet exists.
- No claim beyond "internal dogfood study run" is made.

### After Gate 2 and built-in-agent smoke

Do:

- Run packet-assisted attempts.
- Compare expertise curves.
- Mount the packet into the Forge Coder repository-memory/context lane.
- Add a refs-only internal projection if the packet is useful.

Exit criterion:

- Study packet improves at least two of:
  - pass rate on hidden tasks;
  - tokens or tool calls to passing edit;
  - wrong-file reads;
  - invariant/product-claim violations;
  - test-pass rate without unrelated edits.

### After measured lift

Do:

- Consider a new product-promise record in planned/yellow state for a
  repo-studying system.
- Draft customer-private repo admission and privacy policy.
- Decide whether any study-packet artifact belongs in Blueprint runtime schemas.
- Add deterministic Tassadar refinery receipts for the corpus/exam substrate.

Exit criterion:

- A customer-private pilot can be scoped without leaning on OpenAgents-specific
  docs, raw private data, or unsupported product claims.

## Launch Copy Boundary

Allowed internal phrasing:

- "We are dogfooding repo studying on the `openagents` repo."
- "The goal is to measure whether study packets make agents better at launch
  repo edits under fixed budgets."
- "Tassadar-style deterministic checks can verify corpus/refinery artifacts."
- "Forge Autopilot Coder can consume study packets as refs-only repository
  memory/context."

Forbidden public phrasing until separate evidence and promise gates exist:

- "Tassadar learned the OpenAgents repo."
- "OpenAgents has trained a repo expert model."
- "Repo studying is live for customer repositories."
- "Agents can now edit any repo awesomely."
- "Study packets are marketplace packages."
- "Machine studying work earns payouts."
- "Statistical repo edits are exact-replay verified."

## Concrete Next Artifacts

| Artifact | Home | Purpose | Blocks/blocked by |
| --- | --- | --- | --- |
| `openagents_repo_corpus_manifest.v0` | `docs/research/machine-studying/` first; later schema/code | Reproducible admitted corpus with exclusions and digests | Blocks study packet; blocked by redaction policy |
| `openagents_launch_study_packet.v0` | `docs/research/machine-studying/` or Blueprint docs | Launch-specific source map, traps, test catalog, claim boundaries | Blocks exam-assisted runs |
| `openagents_studybench.v0` | Probe/benchmark docs plus machine-studying docs | StudyBench-compatible rows over the pinned `openagents` repo, with public-retained and private validation/holdout splits | Blocks expertise curve |
| `machine_studying_openagents_launch_exam.v0` | Probe/benchmark docs | Launch-focused slice of `openagents_studybench.v0` | Blocks expertise curve |
| `machine_studying_openagents_launch_attempt_receipt.v0` | Probe/Forge evidence | Attempt trajectory, refs, tests, score, caveats | Blocks measured lift |
| `tassadar.repo_refinery_artifact.v0` | Tassadar docs/design | Deterministic corpus/refinery proof shape | Blocks exact-refinery claims |
| Forge Coder study packet projection | #5107 lane | Refs-only packet visibility/freshness in operator cockpit | Blocked by measured usefulness |

## Kill Conditions

Stop or redesign the short-term lane if:

- The packet improves launch trivia but not real repo edits.
- The corpus manifest cannot reliably exclude private/local/generated material.
- Hidden tasks leak into the packet.
- Agents keep violating product-promise, settlement, or `/tassadar` truth rules.
- The integration encourages keyword routing instead of typed semantic planning.
- The work competes with Gate 2 or the stable v1.0 cut instead of helping it.

## Recommendation

Treat machine studying as the next internal Customer #1 loop:

1. Use the `openagents` repo as the first corpus.
2. Make the launch folder the first high-value study target because it encodes
   the current product truth.
3. Use #5107 Forge Autopilot Coder as the first product integration point.
4. Use Probe for attempts, hidden exams, and evidence.
5. Use Tassadar only for deterministic refinery checks until model-edit
   exactness exists.

This is the shortest path from research idea to launch-useful product muscle:
better agents working on our own repo, with enough evidence discipline that it
can later become a customer repo-studying product.
