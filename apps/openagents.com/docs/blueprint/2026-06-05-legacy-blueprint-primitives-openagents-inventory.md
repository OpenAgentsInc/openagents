# Legacy Blueprint Primitives OpenAgents product surface Inventory

Issue #221 records the first OpenAgents product surface-owned inventory of legacy Blueprint source
material.

## Source Material Reviewed

- `autopilot4-deprecated/blueprint/README.md`
- `autopilot4-deprecated/src/programs.rs`
- `autopilot4-deprecated/migrations/0003_blueprint_program_runtime_bridge.sql`
- `autopilot4-deprecated/blueprint/docs/programs-optimization-and-rlm.md`
- `autopilot4-deprecated/blueprint/docs/action-types-and-automations.md`
- `vortex/docs/2026-05-31-blueprint-system-convex-native-audit.md`
- OpenAgents product surface roadmap section "Blueprint Rebuild Plan For OpenAgents product surface"
- Current OpenAgents product surface Omni workroom, accepted outcome, evidence, route, lifecycle,
  economics, classification, and projection modules

The old standalone `blueprint/` root archive is not present in this workspace.
The available source of truth is the absorbed source material under
`autopilot4-deprecated/blueprint` plus the later Vortex and OpenAgents product surface planning
docs.

## Ownership Decision

OpenAgents product surface owns the first live Blueprint kernel.

The deprecated Blueprint material is source material only. OpenAgents product surface must not add a
production dependency on the old Rust workspace, old Blueprint API, old
Blueprint worker, old generated SDK, or any `blueprint_client` path.

Rust components such as Pylon, Probe, Psionic, Nexus, Treasury, `oa-node`, and
`oa-workroomd` should consume or emit typed contracts and receipts through
future exported schemas. They do not own the customer-facing Blueprint authority
for the first Sites/Autopilot fulfillment system.

## Inventory

| Legacy primitive | Disposition | OpenAgents product surface v1 target | Notes |
| --- | --- | --- | --- |
| Business Object Type / Business Object | Defer | Later Omni object graph or business workroom records | Useful for CRM/company/domain objects, but not required before Program Runs and Action Submissions. Avoid rebuilding a full ontology first. |
| Object Sets / Views | Defer | Workroom/search/query projections | Keep the idea of typed projections, but do not port the old object-set engine now. |
| Source Authority | Keep, rename only by scope | `SourceAuthority` / `ContextPack` in Epic Q | Must become the authority for Exa cards, repos, emails, customer assets, artifacts, and generated summaries. |
| Context Pack | Keep | `ContextPack` in Epic Q | Critical for agent-safe work. Should include source refs, freshness, consent, confidence, classification, included/excluded refs, and public-safe state. |
| Access Explanation | Keep, later | Classification/projection policy plus action approval receipts | Useful for explaining why an action or projection was allowed. Can follow core Program Run and Action Submission records. |
| Program Type | Keep | `BlueprintProgramType` | Versioned behavior contract. Not a prompt. Owns family, purpose, risk, allowed strategies, evidence requirements, receipt requirements, release gate, and direct-mutation policy. |
| Program Signature | Keep | `BlueprintProgramSignature` | Stable input/output schema for a Program Type. The old "Guidance Module" wording should not carry forward unless a specific separate primitive is needed. |
| Module Version | Keep | `BlueprintModuleVersion` | Implementation artifact for deterministic reducers, model prompts, Effect modules, runtime adapters, human review modules, and optimizer candidates. |
| Program Run | Keep | `BlueprintProgramRun` | Decision evidence record. It can recommend, classify, draft, or route. It cannot deploy, send email, create PRs, spend money, mutate source-backed facts, or upgrade public claims. |
| Optimizer Run | Keep, later | `BlueprintOptimizerRun` | Retained failures can produce candidate Module Versions without self-promotion. |
| Eval Suite / Fixture | Keep | `BlueprintReleaseGate` and fixtures | Needed before promoting continuation, routing, email, proof, or source-selection signatures. |
| Release Gate | Keep | `BlueprintReleaseGate` | Promotion requires fixtures, review, policy, rollback posture, scorecards, receipts, and explicit decisions. |
| Action Type | Defer as type, keep concept | Action Submission policy can reference future action type refs | A full reusable action registry is valuable, but first OpenAgents product surface needs approval-gated submissions for deploy/email/PR/source/public-claim/payment/legal actions. |
| Action Submission | Keep | `BlueprintActionSubmission` | Required write-side boundary. Program Runs may propose submissions but cannot execute writes directly. |
| Action Log Object | Rename/defer | Receipt/evidence projection | Use receipts and workroom evidence first. Objectized logs can come later. |
| Durable Outbox Event | Defer | Existing queue/event paths plus future webhook subscriptions | Needed for webhooks and external integrations, but not first kernel foundation. |
| Trust Receipt | Keep as receipt kind | Existing Omni receipts plus future Blueprint receipt catalog | Keep trust/failure/action/denial receipt semantics, but implement through OpenAgents product surface receipt refs first. |
| Failure Receipt | Keep as receipt kind | Program Run fallback and workroom lifecycle receipts | Important for failures not disappearing into logs. |
| Evidence Bundle | Already kept | `omni_evidence_bundles` | Implemented before this issue. Blueprint should consume it rather than duplicate it. |
| Data Classification | Already kept | `omni-data-classification.ts` and workroom classification fields | Implemented before this issue. Extend to Blueprint records and exports. |
| App Manifest | Defer | Later Program Registry / UI/API surface | Useful for UI/tool exposure, but not needed before core schemas. |
| MCP Agent Profile | Defer | Later agent API contract export | Useful for external agents and Pylon/Probe adapters after contract export. |
| Domain Package | Rename/defer | Program/package contribution and workroom template packages | The concept should return as reviewed packages, not as an early source of runtime authority. |
| RLM / GEPA / DSPy optimizer concepts | Keep as later optimizer inputs | `BlueprintOptimizerRun` and candidate Module Versions | Do not vendor DSPy. Preserve typed optimization lineage and release gates. |
| Blueprint Manager UI | Discard for now | OpenAgents product surface/Vortex operator surfaces later | Do not resurrect the old manager. Add focused registry/run detail views when records exist. |
| Blueprint API / Worker service split | Discard for now | OpenAgents product surface Worker modules and D1 repositories | Separate service boundary is premature and would slow first fulfillment. |
| Generated old TypeScript SDK | Discard for now | Future generated schemas/OpenAPI from OpenAgents product surface source | Re-export only after OpenAgents product surface owns the contracts. |
| Postgres-specific migration shape | Discard | D1 migrations and Effect repositories | Keep field semantics where useful, not the old storage engine. |

## Concrete Mapping To Current OpenAgents product surface

Already implemented in OpenAgents product surface:

- Accepted Outcome Contract maps the request/outcome promise.
- Workroom maps the mission container.
- Evidence Bundle maps source, artifact, test, deployment, email, and redaction
  proof.
- Lifecycle Decision maps accept/reject/revision/unavailable decisions.
- Mission Briefing maps customer-safe state explanation.
- Accepted Outcome Economics maps internal cost/value evidence without payment
  settlement claims.
- Route Scorecard maps route selection evidence.
- Public Proof Bundle maps redacted public proof.
- Workroom Kind Templates map work kind policy.
- Market Memory Hooks map evidence-only learning.
- Data Classification maps projection boundaries.
- Workroom Surface Projections map public/customer/team/agent/operator views.

Next OpenAgents product surface primitives to build:

- Blueprint package boundary.
- Objective and Outcome schemas.
- Program Type and Program Signature schemas.
- Module Version schema.
- Program Run repository.
- Program Run evidence-only enforcement.
- Action Submission write boundary.
- Source Authority and Context Pack.
- Release Gate and eval fixtures.
- Continuation Program Signatures.
- Optimizer Runs and candidate modules.
- Simulation Branches.
- Program Registry/API/UI seed.
- Smoke/probe discipline.
- Contract exports for agents and Rust consumers.

## Non-Negotiable Safety Rules

- Program Runs are decision evidence, not write authority.
- Action Submissions are the only Blueprint path for external writes, deploys,
  PRs, email sends, public claim upgrades, payment actions, and legal-sensitive
  commitments.
- Context Packs can narrow access but cannot widen the actor's base authority.
- Public/customer/agent surfaces must read projection helpers, not raw workroom
  or runner state.
- Source-card, Exa, repo, transcript, and generated-summary data must carry
  source refs and freshness/confidence state.
- Release Gates are required before a continuation Program Signature, route
  selector, email classifier, proof projector, or optimizer-produced module can
  be promoted.
- Semantic/program routing must not degrade into ad hoc keyword matching.

## Implementation Boundary For Remaining Epic Q

Implementation should happen inside OpenAgents product surface using Effect Schema, D1 migrations
where persistence is required, typed repository helpers, focused tests, and
docs under `docs/blueprint` and `docs/omni`.

The deprecated Rust source remains a reference. If a later Rust/Pylon/Probe
consumer needs compatibility, OpenAgents product surface should export JSON Schema/OpenAPI/event
contracts rather than linking to the deprecated workspace.
