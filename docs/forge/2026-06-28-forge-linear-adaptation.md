# Forge Linear Adaptation Playbook

Date: 2026-06-28
Status: Product and implementation adaptation notes. Public-safe; no secrets,
tokens, private repo contents, raw patches, transcripts, or customer-private
material.

## 0. Read Set And Thesis

This note responds to the Forge issue asking what to adapt from Linear, after
reading:

- `docs/forge/linear.md`
- `docs/forge/origin.md`
- `docs/forge/2026-06-28-forge-standup-spec.md`
- `docs/forge/2026-06-28-forge-boundary-contract.md`
- `docs/forge/2026-06-28-forge-cross-system-leverage.md`
- `docs/forge/2026-06-28-forge-openagents-com-owned-coordination-layer-audit.md`
- `docs/forge/2026-06-28-forge-software-factory-synthesis.md`
- `docs/blitz/forge/*`
- `docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`
- `backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/docs/plans/archived/adr-legacy-2026-02-21/ADR-0018-forge-adapter-contract.md`
- `backroom/docs/transcripts/infinite-software.md`
- root workspace `products/forge.md`
- root workspace `products/2026-04-14-openagents-com-forge-mvp-roadmap.md`
- `alpha/seed/20260502-opensecret-acquihire-strategy.md`

Thesis:

Forge should adapt Linear's speed, product-context, triage, review, API, and
automation discipline, but not copy Linear's GitHub-dependent architecture.
Linear is strongest as the layer where product intent, review, and agent work
are judged. Cursor Origin is strongest as an agent-native git home. OpenAgents
Forge needs both: the owned git coordination substrate from the SU sequence, and
the software-factory work system from the older Forge product thesis.

Said differently:

- `forge.openagents.com` is not just a code host.
- `forge.openagents.com` is not just a project tracker.
- Forge is the place where agents push code, humans and policies judge it
  against intent, verification proves it, promotion serializes it, and mirrors
  publish it outward.

Linear's playbook should be adapted at the product layer above Forge's
coordination store. The coordination store remains the authority for work,
change, status, lease, verification, and promotion records. Linear-like UX and
workflow should be projections and workflows over that authority, not a second
truth model.

## 1. What Linear Is Teaching Us

`docs/forge/linear.md` describes Linear Diffs, Linear Agent, automations, MCP,
skills, coding sessions, triage, preview links, comments, approvals, change
requests, guided reviews, notifications, and code intelligence. The strategic
lesson is not "use Linear as the forge." The lesson is that agentic software
work needs a fast product-context system where humans can answer:

- What was requested?
- Why is this change here?
- What product surface or customer promise does it affect?
- What evidence proves it works?
- What should the agent do next?
- Who needs attention now?

Linear's visible advantages are mostly product-system advantages:

- Issues/projects/cycles make work intent structured.
- Triage and automations keep incoming work from becoming sludge.
- Diffs put review in the same context as the issue.
- Guided Reviews make large changes navigable.
- Keyboard-first UX keeps frequent actions cheap.
- API/MCP integration lets agents operate through a typed product surface.
- Notifications and "For me" views protect human attention.
- "Built for speed" is treated as a product requirement, not a slogan.

Forge should adapt those advantages while preserving OpenAgents-specific
authority:

- GitHub is a downstream mirror, not the source of truth.
- Git tokens are only for smart Git HTTP, not control-plane authority.
- R2 packfiles are evidence archives, not canonical promotion state.
- D1 coordination rows and canonical refs are the authority.
- Blueprint gates, verification receipts, and promotion receipts decide whether
  a change can move.
- `apps/forge/` owns the Forge UI; the historical logged-in
  `openagents.com` Forge page remains source material only.

## 2. Current Forge Pieces To Build On

The current Forge stand-up docs already define the substrate:

- SU-1/SU-1B: separate `apps/forge/` shell at `forge.openagents.com`.
- SU-2: `/api/forge/*` control-plane routes for work records, change records,
  status transitions, leases, queue state, verification receipts, and promotion
  decisions.
- SU-3: smart Git intake that accepts receive-pack, archives packfiles, updates
  canonical refs, records objects, and creates coordination rows.
- SU-4: owned merge authority through virtual merge queue and Blueprint gates.
- SU-5: verification on intake.
- SU-6: GitHub mirror worker.
- SU-7: dogfood one fleet lane.
- SU-8: multi-tenant / AaaS expansion.

The older software-factory docs define the product semantics:

- Work Orders, Runs, Workspaces, Controller Leases, Knowledge Packs, Evidence
  Bundles, Verification Reports, Delivery Receipts, Handoffs, and Artifacts
  (`products/forge.md`).
- Eight stage factory line: Signal, Triage, Code Gen, Validate, Release,
  Document, Monitor, Deploy (`docs/blitz/forge/2026-06-16-per-vertical-forge-stage-templates.md`).
- Factory metrics: throughput, stage throughput, cycle time, pass rate, token
  efficiency, MTTR, backlog, queue burn, and provenance tags
  (`docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md`).
- Automations that create real work orders rather than decorative UI state
  (`docs/blitz/forge/2026-06-16-forge-automations-surface.md`).
- Terminal-agent review, readiness, context snapshot, session navigation, and
  retrieval-plan surfaces
  (`docs/autopilot-coder/terminal-agent-systems/2026-06-16-forge-autopilot-coder-systems-roadmap.md`).
- Historical adapter discipline: preserve session IDs, policy bundle IDs,
  trajectory hashes, verification results, structured metadata, and agent
  identity when handing work to external forges
  (`backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/docs/plans/archived/adr-legacy-2026-02-21/ADR-0018-forge-adapter-contract.md`).
- Enterprise trust posture: confidential workspace modes, attested receipts,
  encrypted knowledge packs, and refusal rather than silent downgrade when a
  confidential path is required (`alpha/seed/20260502-opensecret-acquihire-strategy.md`).

The synthesis doc already states the key composition:

- the coordination layer answers how code moves safely at arbitrary agent
  counts;
- the factory layer answers what the work is, who it is for, and whether it is
  trustworthy.

Linear maps onto the factory layer. Origin maps onto the coordination layer.
Forge must ship both as one product.

## 3. Linear Features To Adapt, Mapped To Forge

### 3.1 Issues And Projects -> Work Records And Product Context

Adapt:

- Linear's crisp issue/project model.
- Issue status, priority, assignee, labels, project relationships, and
  structured fields.
- The habit of keeping product context next to implementation work.

Forge mapping:

- Forge `work_record` is the authoritative work object.
- A Work Order is the product-facing projection of a work record.
- A project or initiative is a grouping read model over work records, change
  records, verification receipts, and promotion receipts.
- Product context should sit beside the change inspector: customer promise,
  acceptance criteria, linked run, evidence refs, verification status, and
  promotion state.

Implementation direction:

- Keep the D1 coordination row as authority.
- Add typed product-context fields only when they can be sourced safely:
  `intent_ref`, `acceptance_ref`, `customer_ref`, `source_ref`,
  `evidence_ref`, `verification_ref`, and `delivery_ref`.
- Do not duplicate truth between old Autopilot Work records and Forge work
  records. If old surfaces remain useful, they should become projections or
  migration sources.

SU tie:

- SU-2 is the base work-record API.
- SU-7 should dogfood the Work Order projection in the real Forge UI.
- SU-8 should add tenant-safe product-context boundaries.

### 3.2 Cycles And Velocity -> Factory Windows And Queue Metrics

Adapt:

- Linear cycles, project pulse, velocity, and trend views.
- The feeling that teams can see flow without running a query.

Forge mapping:

- A Forge cycle is a time-bounded or objective-bounded factory window over
  work records.
- Velocity is computed from coordination truth: intake, change creation,
  verification completion, promotion, mirror, reopen, and blocker events.
- Existing factory metrics from
  `docs/blitz/forge/2026-06-16-forge-factory-metric-definitions.md` should be
  re-homed onto Forge D1 rows and receipt refs.

Implementation direction:

- Start with a read-only cycle/velocity dashboard in `apps/forge/`.
- Use provenance labels: `live`, `seeded`, `absent`.
- Avoid claiming factory metrics are live until they derive from current
  coordination rows.
- Let cycles drive attention and planning, not promotion authority.

SU tie:

- SU-2 gives status rows.
- SU-4 gives promotion receipts.
- SU-5 gives verification receipts.
- SU-6 gives mirror completion.
- SU-7 is the first honest dogfood cycle.

### 3.3 Triage -> Signal/Triage Factory Stages

Adapt:

- Linear's fast triage loop.
- Clear inboxes, duplicate handling, priority setting, assignee routing, and
  automation handoff.

Forge mapping:

- `Signal` is intent entering Forge.
- `Triage` is the stage where a work record gains scope, priority, policy,
  repository, base ref, target branch/ref, acceptance criteria, and dispatcher
  eligibility.
- Triage automations may create or enrich work records, but they must not
  silently grant authority to run, promote, spend, or expose secrets.

Implementation direction:

- Build a Forge inbox view with keyboard actions for scope, priority, route,
  split, duplicate, close, or dispatch.
- Keep explicit source refs for incoming signals.
- Require a public-safe rationale when an automation changes priority or route.
- Preserve the old automations surface rule: automations create real work, not
  decoration.

SU tie:

- SU-2 can accept triaged work records and statuses.
- SU-7 should run one internal lane through Signal -> Triage -> Code Gen.
- SU-8 can add tenant-specific triage policy.

### 3.4 Diffs And Reviews -> Change Inspector Over Canonical Refs

Adapt:

- Linear Diffs' review tab, "For me" and "Created" groupings, diff review,
  comments, approvals, change requests, preview links, and author/repo/status
  filters.
- Review in product context, not review as an isolated patch.

Forge mapping:

- A Forge change inspector renders a `change_record`, canonical base/head refs,
  packfile digest, changed object inventory, verification receipts, promotion
  decision, comments, and blockers.
- The inspector should show the product intent and acceptance criteria beside
  the code review state.
- Comments and change requests are dispatch inputs for agents, but only through
  typed work/status transitions.

Implementation direction:

- Do not render raw private patches into public-safe docs, logs, or issue
  comments.
- In the app, render diffs from canonical refs with access checks.
- Add structural filters first: files changed, test status, gate status,
  blocker status, owner attention, and promotion readiness.
- Support "For me" as an attention queue: assigned reviews, blocked changes,
  requested decisions, and failed verification receipts.

SU tie:

- SU-3 creates canonical refs and change records.
- SU-4 gives promotion readiness.
- SU-5 provides verification receipts.
- SU-7 should prove the change inspector with one real fleet lane.

### 3.5 Guided Reviews -> Verification-Grounded Review Guides

Adapt:

- Linear's Guided Reviews and review guides for large changes.
- Structural highlighting and reviewer orientation.

Forge mapping:

- A Forge review guide is a generated or authored, refs-only explanation of the
  change plan, touched modules, risk areas, verification ladder, and acceptance
  checks.
- It is advisory unless backed by actual verification receipts and promotion
  gates.

Implementation direction:

- Store review guides as artifacts or refs attached to the change record.
- Include source refs, generated-at time, generator identity, and stale-when
  base/head refs change.
- Mark guides as `advisory`, `verified`, or `stale`.
- Convert meaningful review-guide counterexamples into tests or gate rules.

SU tie:

- SU-5 is required before review guides can claim verification grounding.
- SU-7 can test one guide in dogfood.

### 3.6 Agent Iteration -> Comments And Status As Dispatch Inputs

Adapt:

- Linear's loop where an agent can iterate from issue comments, review
  comments, and diff feedback.

Forge mapping:

- Comments, change requests, failed checks, and blockers should create typed
  follow-up work records or status transitions.
- Agents should receive bounded context: work ref, change ref, comment ref,
  acceptance criteria, repo/ref scope, policy, and allowed tools.
- Agent replies should attach evidence and status, not just prose.

Implementation direction:

- Add an explicit "dispatch follow-up" action in the change inspector.
- Record the dispatch lease and closeout against the originating comment or
  blocker.
- Preserve the terminal-agent roadmap boundary: projections should be refs-only
  and must not leak raw transcripts, local paths, shell logs, provider payloads,
  prompts, secrets, or customer-private data.

SU tie:

- SU-2 gives dispatch leases/status.
- SU-3 gives change refs.
- SU-5 gives verification feedback.
- SU-7 proves end-to-end agent iteration.

### 3.7 Notifications And Attention -> Forge "For Me"

Adapt:

- Linear's notification hygiene, "For me" lane, bot filtering, and grouped
  review attention.

Forge mapping:

- Forge "For me" should be a workbench queue, not a marketing dashboard.
- Separate human-required decisions from agent noise:
  - blocked verification,
  - approval needed,
  - change request answered,
  - merge queue conflict,
  - policy refusal,
  - confidential-route refusal,
  - stale review guide,
  - mirror failure.

Implementation direction:

- Use attention records derived from coordination events.
- Keep bot/agent events visible but grouped.
- Default to the next actionable item.
- Let users acknowledge or snooze attention without changing work truth.

SU tie:

- SU-4 and SU-5 create meaningful action events.
- SU-6 adds mirror-failure attention.
- SU-7 should dogfood attention routing.

### 3.8 Keyboard-First UX And Speed -> Forge Operator Shell

Adapt:

- Linear's keyboard-first, built-for-speed ethos.
- Dense, predictable, commandable product surfaces.

Forge mapping:

- `apps/forge/` should feel like an operator console for repeated work:
  work queue, triage inbox, change inspector, verification, merge queue, refs,
  attention, and command palette.
- It should reuse `@openagentsinc/ui` tokens and components, while allowing
  Forge-specific controls to evolve separately, the same way the forum split
  from the main web app.

Implementation direction:

- Keep Forge out of the main `openagents.com` logged-in route tree.
- Use compact tables, split panes, tabs, command palette, keyboard shortcuts,
  and persistent filters.
- Optimize for scanning, review, and repeated action.
- Avoid marketing-copy hero treatment inside the operator shell. The landing
  page can be distinctive; the work surfaces should be fast.

SU tie:

- SU-1/SU-1B establish the shell boundary.
- SU-7 should identify the first speed-critical dogfood actions.

### 3.9 API, MCP, And Automations -> Typed Control Plane

Adapt:

- Linear's API, MCP, automations, skills, and integration-first posture.

Forge mapping:

- `/api/forge/*` is the typed control-plane API.
- A future Forge MCP surface should expose safe work, change, status,
  verification, attention, and dispatch operations.
- Automations and skills should produce typed work records, status changes,
  leases, and receipt refs.

Implementation direction:

- Do not let smart-Git tokens call the control plane.
- Keep scope names explicit: `forge:work:*`, `forge:change:*`,
  `forge:verification:*`, `forge:promotion:*`, `forge:queue:*`,
  `forge:admin`.
- Treat skills as templates/policies/procedures, not authority.
- Require receipts for automation actions that affect dispatch, promotion,
  delivery, or tenant data.

SU tie:

- SU-2 has the first control-plane routes.
- SU-4 and SU-5 define the receipt contracts needed for automation trust.
- SU-8 needs tenant-scoped API/MCP policy.

### 3.10 Code Intelligence -> Repository Memory With Permission Boundaries

Adapt:

- Linear's Code Intelligence and coding-session context.
- Fast navigation from intent to relevant code, prior changes, tests, and
  artifacts.

Forge mapping:

- Forge code intelligence should be a repository memory and context snapshot
  system over canonical refs, Work Orders, changes, verification artifacts,
  and delivery receipts.
- It should help agents and reviewers retrieve context without becoming an
  unsafely broad data lake.

Implementation direction:

- Start with refs-only context snapshots.
- Record what source refs and artifact refs were included in a run.
- Use permissioned retrieval; do not mix tenant data.
- Keep generated context stale-aware when refs advance.
- For confidential work, record route posture and refuse unsupported private
  paths instead of silently downgrading.

SU tie:

- SU-3 gives canonical refs.
- SU-5 gives verification artifacts.
- SU-8 makes tenant isolation mandatory.

## 4. Linear Features To Avoid Or Modify

Do not copy:

- GitHub as the backend authority. Forge's value is owning the coordination
  layer and mirroring outward.
- PR-centric bottlenecks. Forge changes should flow through canonical refs,
  verification, merge queue, promotion, and mirror.
- A second product truth model. Linear-like projects, cycles, and review views
  must derive from Forge work/change/status/receipt rows.
- Agent action bias. Agent suggestions should be easy to run, but bounded by
  explicit policy, receipts, and human decision points.
- Trust-by-guide. Guided Reviews are helpful orientation, not proof.
- Vague "AI did it" review claims. A change needs receipt refs, command
  identity, timestamps, artifact refs, and gate outcomes.
- Main-app sprawl. Forge UI should remain in `apps/forge/`, not grow inside the
  historical logged-in Forge page in `apps/openagents.com`.
- Public leakage. Docs and issue comments should cite refs and paths, not raw
  private patches, transcripts, prompts, secrets, or customer-private data.

Modify:

- Linear cycles become Forge factory windows over coordination events.
- Linear issues become Forge Work Orders/work records.
- Linear Diffs become Forge change inspectors over canonical refs and receipts.
- Linear automations become receipt-bearing, scope-bounded control-plane
  operations.
- Linear code intelligence becomes permissioned repository memory over
  canonical refs and evidence artifacts.

## 5. Current Versus Future

Current, grounded:

- Separate `apps/forge/` shell exists for `forge.openagents.com`.
- `/api/forge/*` control-plane routes exist in the `apps/openagents.com` Worker.
- Smart-Git receive-pack intake exists and can create coordination rows.
- D1 coordination store tracks work, change, status, lease, queue, verification,
  and promotion-related records.
- R2 archives packfiles as evidence.
- Tenant git auth is scoped to Git protocol operations.
- Shared Forge protocol schemas define control-plane scopes and receipt shapes.
- Existing factory docs define Work Orders, stages, metrics, automations, and
  vertical templates.

Near future:

- SU-4 owned merge authority turns change readiness into a gated ref
  fast-forward.
- SU-5 verification on intake makes receipts mandatory before promotion.
- SU-6 GitHub mirror demotes GitHub to downstream visibility.
- SU-7 dogfoods one real fleet lane through Forge.
- SU-8 adds tenant isolation and AaaS readiness.

Future product layer:

- Linear-like work/project/cycle surfaces over Forge rows.
- "For me" attention queue.
- Change inspector with product intent, diff, verification, review guide,
  queue position, and promotion state.
- Keyboard-first triage and review commands.
- Forge MCP for scoped agent operation.
- Factory metrics and velocity from live coordination truth.
- Confidential Forge work modes with explicit route posture and receipts.

## 6. Concrete Backlog To Open Or Attach To SU Work

These are intentionally phrased as implementation issues, not vague strategy.

1. Forge Work Model Read Layer
   - Add a product-facing Work Order projection over `work_record`,
     `change_record`, `status`, verification, and promotion rows.
   - Cite: `products/forge.md`, SU-2, SU-7.

2. Forge Triage Inbox
   - Add Signal/Triage views and actions in `apps/forge/`: scope, priority,
     split, duplicate, dispatch, close, and source-ref inspection.
   - Cite: Linear triage, blitz automations, SU-2.

3. Forge Change Inspector
   - Render a change view over canonical base/head refs, packfile digest,
     verification receipts, comments, blockers, queue position, and promotion
     readiness.
   - Cite: Linear Diffs, SU-3, SU-4, SU-5.

4. Forge "For Me" Attention Queue
   - Derive actionable attention records from blockers, failed receipts,
     approval requests, stale guides, queue conflicts, and mirror failures.
   - Cite: Linear notifications, SU-4..SU-7.

5. Factory Cycle And Velocity Metrics
   - Compute throughput, cycle time, pass rate, MTTR, queue burn, and backlog
     from coordination rows with `live`/`seeded`/`absent` provenance.
   - Cite: blitz metric definitions, SU-5..SU-7.

6. Verification-Grounded Review Guides
   - Attach advisory/stale/verified review guides to change records with source
     refs and verification receipt refs.
   - Cite: Guided Reviews, Autopilot Coder roadmap, SU-5.

7. Agent Iteration From Review Feedback
   - Convert comments/change requests into bounded follow-up work records or
     dispatch leases with receipt-backed closeout.
   - Cite: Linear Agent loop, SU-2, SU-5, SU-7.

8. Forge Command Palette And Keyboard Map
   - Add keyboard-first navigation and actions for queue, triage, inspector,
     verification, merge queue, and refs.
   - Cite: Linear built-for-speed ethos, `apps/forge/` boundary.

9. Forge MCP Surface
   - Expose scoped work/change/status/verification/attention operations for
     agents with explicit control-plane scopes.
   - Cite: Linear MCP/API, boundary contract, SU-8.

10. Confidential Forge Receipt Fields
   - Add optional confidential route posture fields for workspace mode,
     attestation ref, encrypted knowledge-pack ref, refusal reason, and
     retention policy.
   - Cite: `alpha/seed/20260502-opensecret-acquihire-strategy.md`, SU-8.

## 7. SU Sequence Overlay

Keep the current SU sequence. Linear adaptation should not reorder the
coordination substrate.

SU-4, owned merge authority:

- Product impact: Change inspector can show real promotion readiness.
- Linear adaptation: Diffs stop being "review pages" and become
  promotion-aware change records.

SU-5, verification on intake:

- Product impact: Review guides, attention queues, and factory metrics become
  receipt-grounded.
- Linear adaptation: "AI says this is fine" becomes "receipt says this command
  passed or failed against this ref."

SU-6, GitHub mirror:

- Product impact: GitHub becomes read-only visibility.
- Linear adaptation: Forge keeps Linear-like context while avoiding a
  GitHub-dependent backend.

SU-7, dogfood one fleet lane:

- Product impact: First true cycle/velocity/attention data.
- Linear adaptation: Use the dogfood lane to identify the minimum speed loop:
  triage -> dispatch -> change inspector -> verification -> promotion.

SU-8, multi-tenant / AaaS:

- Product impact: Tenant-safe work systems, scoped API/MCP, and confidential
  posture.
- Linear adaptation: Keep the speed and API surface, but enforce isolation and
  refusal behavior before external admission.

## 8. Product Line Boundary

The Forge UI boundary remains:

- `apps/forge/` owns Forge pages, routing, navigation state, and operator UX.
- `apps/openagents.com` may host shared API infrastructure while that is the
  fastest safe path, but it does not own Forge page routing.
- Shared components and tokens can come from `@openagentsinc/ui`.
- Forge-specific queue, triage, change, verification, merge, refs, and
  attention controls may evolve in `apps/forge/` or a later
  `@openagentsinc/forge-ui`.
- The old logged-in Forge page in `apps/openagents.com` is historical source
  material only.

This directly supports the user directive: Forge should be separate from
`openagents.com`, while reusing the same UI components and allowing the product
to evolve separately.

## 9. Operating Principles

1. Linear is a speed and context reference, not an authority reference.
2. Origin is a coordination reference, not a complete product model.
3. Forge owns work/change/status/verification/promotion truth.
4. The factory layer is a read/write product system over that truth.
5. Every agent action needs bounded authority and durable receipts.
6. Review guides orient humans; verification receipts and gates decide
   readiness.
7. Metrics are only trusted when derived from live coordination records or
   clearly labeled as seeded.
8. Public docs cite paths and refs, not private payloads.
9. Keyboard-first speed is a product requirement for high-agent-count work.
10. The first dogfood lane should teach the UI before external tenant expansion.

## 10. Short Recommendation

Ship SU-4 and SU-5 first. In parallel, start the smallest Linear-inspired Forge
product layer: a triage inbox, a change inspector, and a "For me" attention
queue over existing Forge rows. Do not wait for every factory feature. The
moment one fleet lane dogfoods through Forge, use that lane's live coordination
events to build cycles, velocity, review guides, and MCP affordances from real
usage rather than from speculative dashboards.
