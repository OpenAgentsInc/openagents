# Machine Studying Ties To Blueprint, Tassadar, DSPy, And Marketplaces

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-17

Status: research synthesis. This is not product copy and does not upgrade any
marketplace, signature monetization, payment, training, or Tassadar claim.

Primary external source:

- `docs/research/machine-studying/README.md`
- `docs/research/machine-studying/research-note.md`

Local source refs reviewed:

- `docs/tassadar/2026-06-11-tassadar-plugin-marketplace-audit.md`
- `apps/openagents.com/docs/blueprint/2026-06-05-legacy-blueprint-primitives-openagents-inventory.md`
- `apps/openagents.com/docs/blueprint/2026-06-05-openagents-blueprint-package-boundary.md`
- `packages/probe/docs/2026-06-07-blueprint-signature-lookup-apple-fm-tool-use-audit.md`
- `docs/artanis/2026-06-06-pylon-marketplace-job-contract.md`
- `docs/artanis/2026-06-06-pylon-marketplace-job-intake-api.md`
- `apps/openagents.com/workers/api/src/blueprint/schemas/program.ts`
- `apps/openagents.com/workers/api/src/blueprint/schemas/program-run.ts`
- `apps/openagents.com/workers/api/src/blueprint/services/program-run-authority.ts`
- `apps/openagents.com/workers/api/src/blueprint-routes.ts`
- `apps/openagents.com/workers/api/src/blueprint/exports/contract-export.ts`
- `apps/openagents.com/workers/api/src/pylon-marketplace-jobs.ts`
- `apps/openagents.com/workers/api/src/signature-marketplace-revenue-gate.ts`
- `apps/openagents.com/workers/api/src/artanis-continual-learning-templates.ts`
- `apps/openagents.com/workers/api/src/product-promises.ts`
- `docs/promises/registry.md`
- `docs/transcripts/206.md`, `docs/transcripts/207.md`,
  `docs/transcripts/211.md`
- Workspace root `products/2026-04-14-dspy-dsrs-gepa-rlm-forge-and-probe-audit.md`

## Short version

Machine studying gives OpenAgents a measurement language for something the
system has been building toward for years: reusable, typed, improvable machine
capability units that can be discovered, evaluated, routed, composed, and
eventually paid for.

The local architecture already has most of the nouns:

- Blueprint owns typed program contracts: Program Types, Program Signatures,
  Module Versions, Program Runs, Release Gates, Source Authority, Context
  Packs, Optimizer Runs, and Action Submissions.
- DSPy/DSRS supplied the core idea: build AI behavior with typed signatures and
  modules instead of one prompt blob, then optimize modules behind evaluation
  and promotion gates.
- Probe consumes Blueprint contracts to choose a bounded tool menu, execute a
  backend session, emit evidence, and propose write-side Action Submissions.
- Artanis/Pylon marketplace records already include `gepa_dspy_optimization`
  and other learning/evaluation job kinds, but those records are explicitly not
  dispatch, charge, payout, or settlement authority.
- Signature marketplace revenue gates already model the missing business chain:
  validated -> metered -> attributed -> priced -> eligible -> payable ->
  settled.
- Tassadar adds the proof-bearing tier: deterministic, digest-pinned execution
  whose trace can be replay-verified before purchase or settlement.

Machine studying can tie these together by asking one concrete question:

> Given only a corpus, can the agent produce a Blueprint-governed study artifact
> that improves downstream expertise curves, not just peak accuracy or raw
> retrieval?

That artifact is the missing "study packet" between corpus and marketplace.

## Why this matters for Blueprint

The machine-studying article defines expertise as the efficiency of converting
inference compute into accurate work. Blueprint already has a compatible
contract shape:

- `BlueprintProgramType` is the behavior family and risk envelope.
- `BlueprintProgramSignature` is the stable typed input/output contract.
- `BlueprintModuleVersion` is the implementation artifact behind the signature.
- `BlueprintProgramRunRecord` is evidence-only execution output.
- `BlueprintReleaseGate` decides promotion.
- `BlueprintOptimizerRun` is the future place for GEPA/DSPy-style improvement.
- `BlueprintActionSubmission` is the write-side approval boundary.

The important fit is that machine studying treats "studying" as anything the
agent does to itself before downstream evaluation. In OpenAgents, that should
not be an untyped notes file alone. It should be a Blueprint-owned artifact
with:

- corpus/source refs and freshness;
- the signature family it is meant to improve;
- tool scopes and backend constraints;
- generated notes, rubrics, probes, retained failures, examples, and fixtures;
- the measured expertise curve before and after use;
- the release gate that says whether it can become a promoted Module Version,
  Context Pack, Probe tool-menu policy, or marketplace package candidate.

In short: a study packet is not runtime authority. It is candidate evidence for
promotion.

## DSPy lineage

The local DSPy history matters because machine studying is not "add more RAG."
OpenAgents repeatedly used DSPy-like primitives for exactly the right reason:
small typed decisions are easier to evaluate, optimize, promote, and reuse than
giant prompts.

From the current docs and transcripts, the durable mapping is:

| DSPy-era term | Current OpenAgents term | Machine-studying role |
| --- | --- | --- |
| Signature | Blueprint Program Signature | The typed exam interface for a studied skill. |
| Module | Blueprint Module Version | The candidate implementation produced or improved by study. |
| Optimizer | Blueprint Optimizer Run | The study algorithm's improvement loop. |
| Trace/mining | Program Run evidence | The raw material for retained failures and study examples. |
| Promotion | Release Gate | The boundary between a promising study artifact and live use. |
| Marketplace package | Signature/package contribution | A monetizable candidate only after validation, metering, pricing, and settlement gates. |

The rule from the DSPy/DSRS retrospective still holds: do not vendor DSPy and do
not rebuild prompt routing in random UI routes. "DSPy-like" should mean shared
typed contracts, compiled-module lineage, replay/evidence structures, and
promotion gates.

## Probe as a studying runtime consumer

Probe is a natural first consumer for machine-studying artifacts because its
current Blueprint plan already requires preflight selection:

```text
assignment/context/backend/risk
  -> lookup Blueprint Program Signatures
  -> select Program Type, Signature, Module Version, and tool scopes
  -> derive backend tool menu
  -> project backend-specific tool definitions
  -> execute/refuse tools under local policy
  -> record evidence-only Program Run output
  -> propose Action Submission for write-side effects
```

Apple Foundation Models makes this especially clear because tool definitions
must be known when the session starts. That forces study artifacts to be
operationally useful: a good packet should select a smaller, safer, more
accurate tool menu before inference begins.

A Probe machine-studying benchmark should therefore measure:

- baseline Probe turn with generic registry/tool preflight;
- Probe turn with a generated study packet mounted;
- whether the selected signature/tool menu changes;
- whether the right evidence enters the trajectory;
- whether the agent uses that evidence correctly;
- score versus generated tokens or wall-clock budget;
- whether any write-side effect remains an Action Submission instead of direct
  Program Run authority.

This directly mirrors the article's "reach versus recall" split: first ask
whether the right evidence was retrieved, then ask whether the agent recognized
and applied it.

## Marketplace tie-in

The current marketplace surfaces are intentionally gated:

- Pylon marketplace job intake can record and triage work, including
  `gepa_dspy_optimization`, but it returns no live dispatch, buyer-charge,
  payout, or settlement authority.
- Artanis continual-learning templates map `dspy_gepa_optimization` to the
  Pylon `gepa_dspy_optimization` job kind, while keeping training, promotion,
  provider mutation, Pylon dispatch, and payment spend authority false.
- Signature marketplace revenue gates do not allow install, listing mutation,
  runtime activation, payout claims, or settlement claims until the full
  evidence chain exists.
- Product promises keep signature monetization red until usage metering,
  billing, revenue split, and settlement are live.

Machine studying should not bypass any of those gates. It should supply the
evidence those gates need.

A future signature marketplace package should have to answer:

- What corpus was studied?
- What Program Signature or package surface did the study target?
- What study algorithm created the packet?
- What pre-study and post-study expertise curves were measured?
- What retained failures or fixtures were used?
- What release gate promoted it, if any?
- What exact usage subject refs can later be metered?
- What attribution refs identify the author without leaking private material?

Only after that chain exists should marketplace listing, usage pricing,
revenue projection, payout eligibility, or settlement copy become possible.

## MVP-15 product-promise gate

The first OpenAgents StudyBench comparison is enough to support a yellow
internal-dogfood product promise for OpenAgents-owned repo study packets:
`autopilot.repo_study_packets.v1`.

That promise is deliberately not marketplace work. The study packet and rows
are refs-only evidence for OpenAgents-codebase work, not customer repo studying,
not a trained repo expert, not a marketplace package, not payout eligibility,
and not paid work. The product-promise gate review lives at
`docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md`.

Before a Blueprint study packet becomes a marketplace package candidate, the
package lane needs separate gates for customer-data privacy, source authority,
private validation, held-out evaluation, package conformance, usage metering,
exact usage-subject refs, pricing, refunds/disputes, payout eligibility, and
settlement receipts.

## Tassadar tie-in

The Tassadar marketplace audit says the store should be built last. Machine
studying agrees with that sequencing.

Tassadar creates a special high-trust tier for exact or deterministic modules:
execution traces are digest-pinned and replay-verifiable. That makes them
different from normal learned modules or context packets. The marketplace audit
sketches a ladder:

- exact compiled modules with replay proofs;
- deterministic host-native plugins with typed packets and replay posture;
- statistical learned modules with eval receipts and divergence histograms;
- effectful/networked goods that fall back to Blueprint governance, Source
  Authority, Action Submissions, approvals, and receipts.

Machine studying can become the evaluation layer for the non-exact tiers and
the authoring loop for exact tiers:

- For exact modules, study creates and verifies corpora of fixtures, refusal
  cases, and conformance traces.
- For deterministic plugins, study improves catalog maps, packet schemas, and
  bounded tool usage examples.
- For statistical modules, study measures expertise curves instead of only
  benchmark peak score.
- For effectful packages, study must never replace Source Authority or Action
  Submission boundaries.

The key product rule is labeling: a learned study packet must not inherit
Tassadar exactness language. If the output is statistical, it needs eval and
release-gate receipts. If the output is exact, it needs replay receipts.

## Proposed primitive: Blueprint Study Packet

Add a future Blueprint primitive or package contribution shape with a name like
`BlueprintStudyPacket`. It can start as documentation and fixtures before any
runtime route.

Suggested fields:

```text
studyPacketRef
corpusRefs
sourceAuthorityRefs
contextPackRefs
targetProgramTypeRefs
targetProgramSignatureRefs
targetToolRefs
studyAlgorithmRef
studyComputeBudgetRef
studyOutputRefs
generatedNoteRefs
generatedFixtureRefs
generatedRubricRefs
retainedFailureRefs
preStudyExpertiseRef
postStudyExpertiseRef
budgetCurveRefs
reachMetricRefs
usedEvidenceMetricRefs
releaseGateRefs
promotionDecisionRefs
marketplaceCandidateRefs
caveatRefs
```

Authority defaults:

```text
runtimePromotionAllowed: false
marketplaceListingAllowed: false
paymentSpendAllowed: false
payoutClaimAllowed: false
settlementClaimAllowed: false
directMutationAllowed: false
```

The packet can be consumed by Probe, Pylon, Psionic, and OpenAgents product
surfaces through Blueprint contract exports, but it should not grant authority
by itself.

## Candidate benchmark

Build an internal `machine_studying_blueprint_probe.v1` benchmark:

1. Choose a corpus: one repo, one package, one API surface, or one Blueprint
   registry slice.
2. Freeze a hidden exam with implementation questions, refusal cases, and
   deterministic checks.
3. Run baseline agents at direct, k=5, k=20, and forced-k=20 budgets.
4. Let the same agent produce a Blueprint Study Packet from only the corpus.
5. Re-run the exam with the packet mounted as a Context Pack/tool-menu input.
6. Record Program Run evidence, reach metrics, used-evidence metrics, token
   cost, latency, test output, and direct-effect denials.
7. Reduce the score-vs-budget curve to expertise.
8. Treat any promotion as release-gated evidence, not automatic runtime change.

Good first corpora:

- `apps/openagents.com/workers/api/src/blueprint/`
- `packages/probe/packages/runtime/src/blueprint/`
- `apps/openagents.com/workers/api/src/pylon-marketplace-jobs.ts`
- `apps/openagents.com/workers/api/src/signature-marketplace-revenue-gate.ts`
- a small Tassadar exact-replay fixture family once the relevant public refs are
  available.

## What not to do

- Do not describe a study packet as learning, training, payment, marketplace,
  or settlement authority.
- Do not promote an optimizer-produced Module Version without release gates.
- Do not select signatures, tools, or marketplace categories by ad hoc keyword
  matching.
- Do not let Program Runs directly create PRs, deploy, send email, spend money,
  mutate source-backed facts, or upgrade public claims.
- Do not store raw prompts, private repo content, private customer data,
  provider payloads, wallet material, payment material, raw logs, or raw
  training payloads in public study artifacts.
- Do not publish marketplace UI before conformance-tested inventory and
  metering/settlement evidence exist.

## Next concrete docs/code work

1. Draft `BlueprintStudyPacket` as a schema-only proposal under the Blueprint
   docs or source boundary.
2. Add a small docs-only benchmark plan for
   `machine_studying_blueprint_probe.v1`.
3. Extend the existing signature marketplace gate notes with "expertise curve"
   and "exact usage subject" as package evidence categories.
4. Add a Probe fixture that models baseline versus study-packet tool-menu
   selection without invoking a live backend.
5. Keep product-promise state unchanged until a live run has package validation,
   usage metering, attribution, pricing, revenue-share policy, payout
   eligibility, and settlement receipts.

The main design answer is straightforward: Blueprint gives machine studying a
contract, Probe gives it a runtime, Pylon gives it work intake, Tassadar gives
it a proof tier, and marketplace gates give it business discipline. The study
packet is the bridge between "read this corpus" and "this capability can be
trusted, routed, promoted, priced, or paid."
