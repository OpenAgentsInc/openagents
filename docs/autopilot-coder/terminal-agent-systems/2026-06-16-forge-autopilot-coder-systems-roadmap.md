# Forge Autopilot Coder Terminal-Agent Systems Roadmap

Date: 2026-06-16

Parent issue: #5107.

## Scope

This audit translates the terminal-agent-systems inventory into the current
Forge Autopilot Coder product surface. It is a planning artifact only: it does
not create product promises, broaden public copy, or change runtime authority.

Inputs reviewed:

- #5107 issue body.
- `2026-06-11-terminal-agent-systems-index.md`.
- `2026-06-11-terminal-agent-systems-operationalization-roadmap.md`.
- Focused system audits for systems 10, 12, 18, 19, 24, 26, 28, 31, and 32.
- Pack C records:
  - `docs/autopilot-coder/2026-06-12-pack-c-change-capture.md`
  - `docs/autopilot-coder/2026-06-12-pack-c-delivery-readiness.md`
- Current Forge and cockpit source:
  - `apps/openagents.com/apps/web/src/page/loggedIn/page/autopilot-work.ts`
  - `apps/openagents.com/apps/web/src/page/loggedIn/page/forge.ts`
  - `apps/openagents.com/workers/api/src/pack-c-change-capture.ts`
  - `apps/pylon/src/workspace-materializer.ts`
  - `apps/pylon/src/tas/diff-review.ts`
  - `apps/pylon/src/context-projection.ts`
  - `apps/pylon/src/session-record-store.ts`
  - `apps/pylon/src/tas/resume-rewind.ts`
  - `packages/autopilot-control-protocol/src/artifact-review-view.ts`

## Current State

The #5107 runtime spine already exists. The issue should not recreate it.

Already built and reusable:

- Agent runtime schema, Probe/Pylon runtime pieces, tool contracts, permission
  boundaries, and typed closeout/event/receipt records.
- Pack A supervision surfaces: task/background, schedule, event log, artifacts,
  receipts, token/cost, approvals, notifications, and non-interactive mode.
- Pack B provider/account/security/telemetry/retention/managed-policy surfaces.
- Pack C repo/worktree identity, change capture, workspace authority, delivery
  readiness, and PR-readiness projections.
- Pylon `git_checkout` materialization, lane-scoped change capture, conflict
  detection, commit guards, and dirty-worktree retention.
- Pylon context projection from dev doctor (`openagents.pylon.context.v0.3`).
- Minimal Pylon TAS diff-review artifact (`artifactKind: "diff_review"`).
- Minimal resume/rewind checkpoint resolver and session-record persistence.
- Web `/autopilot` cockpit and `/forge` factory dashboard on the shared
  `@openagentsinc/ui` surface.
- `autopilot-control-protocol` artifact review projection for extracting
  outcome, edited-file counts, command counts, token counts, dev-check state,
  artifact ref, and deviations from varied closeout shapes.

Main gap:

Forge has the data primitives and the operator cockpit, but the cockpit does
not yet expose terminal-agent systems as first-class workflows. The current UI
primarily shows Runs, routing, pool capacity, accepted-outcome receipts, and
factory metrics. It does not yet give an operator a typed diff-review lane,
durable plan/progress lane, session navigation lane, context/memory lane, or
retrieval/extensibility lane.

## Non-Goals And Boundaries

- Do not put raw patches, private repo content, local paths, shell logs,
  provider payloads, prompts, secrets, wallet material, or customer-private data
  into public or customer projections.
- Do not treat a diff preview as proof that code changed. Applied edits,
  accepted outcomes, delivery readiness, and settlement remain separate refs.
- Do not let the Forge browser UI become settlement, payout, deployment, or
  public-claim authority.
- Do not add keyword-only intent routing for context, retrieval, tool
  selection, or user-facing work routing. Use typed selectors, structured
  planners, or semantic retrieval contracts.
- Do not let MCP, skills, hooks, or plugins bypass the same approval,
  workspace, provider-account, and public-projection boundaries as native tools.

## Issue / Epic Split

### G1 - Diff Review And Accepted-Outcome Review Lane

Systems: system 24 Diff And Patch Review UI, system 44 Artifact And Receipt
System, system 6 Permission And Approval, Pack C change capture/delivery
readiness.

Goal: make `/autopilot` and/or `/forge` show a first-class "Review changes"
lane for delivered Runs. Start with refs-only review summaries, not raw patch
rendering.

Initial issue to open now:

- G1.1 - Forge cockpit diff-review lane over Pack C change captures.

Acceptance shape:

- Build a typed view model that joins Run closeout refs, `projectArtifactReview`
  output, Pack C change-capture refs, delivery-readiness refs, verification
  refs, review caveat refs, and blockers.
- Render the lane in the Run detail/cockpit with file counts, added/removed
  counts when available, patch digest ref, verification status, caveats, and
  explicit missing-evidence blockers.
- Keep existing accept/request-changes actions separated from review evidence.
- Add tests proving raw patch text and local/private material are not rendered.

Why first: Pack C already created the review-ready evidence layer, and the
existing cockpit already owns Run review actions. Surfacing that evidence is
the smallest user-visible #5107 slice.

Implementation status, 2026-06-16: #5123 adds the first refs-only
`/autopilot` Review changes lane, optional Pack C closeout fields, Pylon
worker-closeout pass-through, and regression coverage for unsafe review
material omission/rejection. Follow-on G1 work should only add raw diff
inspection through a separately fetched and projected public-safe artifact
surface, not by placing raw patches into the Run projection.

### G2 - Plan, Todo, Progress, And Blocker Lane

Systems: system 10 Plan/Todo/Progress, system 25 Notifications/Attention,
Pack A runtime supervision and event log.

Goal: promote runtime task/progress events into a stable Forge "Plan" lane.

Candidate child issues:

- G2.1 - Define `ForgeRunProgressView` over Pack A task/progress/event refs.
- G2.2 - Render active/pending/completed/blocked plan items in `/autopilot`
  detail and `/forge` stage panels.
- G2.3 - Add closeout consistency tests so UI cannot show pending or blocked
  work as completed.

Implementation status, 2026-06-16: #5125 adds the first
`ForgeRunProgressView` projection over Run state, lifecycle events,
next-action refs, and closeout evidence. It is intentionally view-model only;
G2.2 should render this projection rather than re-deriving progress status in
the page layer.

Implementation status, 2026-06-16: #5126 renders the projection as the first
`/autopilot` Run progress lane, including task statuses, blocker refs, and
unsafe-ref omission warnings while keeping the raw lifecycle panel available as
drilldown. `/forge` stage-panel rendering remains a follow-on G2.2 slice.

Implementation status, 2026-06-16: #5127 adds `/forge` stage progress
summaries derived from loaded Run summaries, with live provenance, bounded
Run-detail links, progress chips, and unsafe-ref omission handling. Full
per-Run lifecycle evidence remains in `/autopilot`.

Implementation status, 2026-06-16: #5128 adds closeout/progress consistency
regressions and fixes the progress projection so stale closeout refs cannot
make scheduled, running, blocked, or invalid Runs appear completed.

### G3 - Resume, Rewind, Session Navigation

Systems: system 26 Resume/Rewind/Session Navigation, system 39 Remote
Session Bridge, Pylon `session-record-store`, TAS `resume-rewind`.

Goal: let operators see and resume/fork/cancel sessions without inventing
state. Start with read-only listing and explicit blocker states.

Candidate child issues:

- G3.1 - Define a session-navigation projection over local Pylon session refs,
  external Codex/Claude session summaries, and bridge session refs.
- G3.2 - Render session list/detail in the cockpit with resume/fork/rewind
  unavailable states until the authoritative control verbs exist.
- G3.3 - Add safe export/public-summary receipts for session summaries.

Implementation status, 2026-06-16: #5129 adds the first
`ForgeSessionNavigationView` projection over Pylon, Codex, Claude, and bridge
session summaries. All resume/fork/rewind/cancel controls are explicit
unavailable states until authoritative verbs exist.

Implementation status, 2026-06-16: #5130 renders that projection in the
`/autopilot` Run detail surface as a read-only Session navigation lane with
disabled control buttons, explicit blocker refs, empty/no-session handling, and
unsafe-ref omission coverage.

Implementation status, 2026-06-16: #5131 adds
`projectForgeSessionSummaryReceipt()`, a refs-only public-safe receipt over the
session navigation projection with source/state counts, safe session/evidence
refs, control blocker refs, non-authority flags, and unsafe-title/ref omission
regressions.

### G4 - Context Assembly, Repository Memory, And Onboarding

Systems: system 12 Context Assembly, system 18 Repository Memory/Onboarding,
existing Pylon context projection and dev-doctor checks.

Goal: turn `pylon context --json` and repo/dev-doctor facts into a Forge
"Context" lane that explains what the agent knows before it runs.

Candidate child issues:

- G4.1 - Define a typed `ForgeContextSnapshot` from Pylon context projections,
  repo identity, instructions, adapter readiness, and current-job refs.
- G4.2 - Render context freshness, instruction refs, repo dirty state, adapter
  state, and current-job blockers in `/autopilot`.
- G4.3 - Add repository-profile refresh receipts for command/test/invariant
  changes.

Implementation status, 2026-06-16: #5132 adds the first
`ForgeContextSnapshot` projection for refs-only context readiness across repo
identity, instructions, adapter readiness, dev-doctor evidence, current-job
refs, dirty state, freshness, blockers, and unsafe-material omission.

Implementation status, 2026-06-16: #5133 renders that projection in the
`/autopilot` Run detail surface as a Context snapshot lane with freshness,
dirty state, repo/instruction/adapter/dev-doctor/current-job refs, missing
evidence blockers, and unsafe-ref omission coverage.

Implementation status, 2026-06-16: #5134 adds
`projectForgeRepositoryProfileRefreshReceipt()`, a refs-only public-safe receipt
for repository command/test/instruction/invariant profile refreshes with
freshness, changed profile kinds, non-authority flags, blockers, and unsafe-ref
omission regressions.

### G5 - Retrieval And Search Planner

Systems: system 19 Semantic Retrieval/Search, system 20 LSP/Diagnostics,
system 27 Help/Doctor.

Goal: give the Forge coder a typed search/retrieval pipeline before adding
conceptual or semantic routing to user-facing commands.

Candidate child issues:

- G5.1 - Define retrieval plan/candidate/result-set schemas for exact,
  structured, semantic, model-selected, and hybrid modes.
- G5.2 - Add file/repo/doc exact adapters plus deterministic fixture ranking.
- G5.3 - Add a Forge search panel that shows selected/skipped candidates with
  provenance and freshness.

Implementation status, 2026-06-16: #5135 adds the first
`projectForgeRetrievalPlan()` refs-only contract for exact, structured,
semantic, model-selected, and hybrid retrieval plans, including candidate,
skipped-candidate, and result-set shapes, deterministic ordering, blockers,
freshness, and unsafe-material omission coverage. It does not add retrieval
execution, adapter IO, embeddings, routing, or UI.

Implementation status, 2026-06-16: #5136 adds bounded exact retrieval adapters
for file, repository, and documentation fixtures. The adapters rank only
already-indexed public-safe refs, emit selected/skipped candidate inputs for
the #5135 projection, and cover duplicate, private-filtered, unsupported,
missing-source, and low-score skips without adding filesystem crawling,
embeddings, LSP calls, routing, or UI.

Implementation status, 2026-06-16: #5137 renders the first `/autopilot`
Retrieval search lane from the #5135 projection, including plan/query/source
refs, selected and skipped candidates, provenance, freshness, blockers, and
unsafe-material omission warnings. It remains read-only and does not add live
search UI, adapter execution, filesystem crawling, embeddings, LSP calls,
model calls, or routing.

### G6 - Extensibility: MCP, Skills, Hooks, Plugins

Systems: system 28 MCP Client, system 30 Plugin, system 31 Skill, system 32
Hook/Event, system 33 Settings.

Goal: ingest external capabilities without weakening runtime authority. This is
an epic, not the next implementation issue.

Candidate child issues:

- G6.1 - MCP client capability catalog projection only: configured/pending/
  disabled/failed/needs-auth states, no tool calls yet.
- G6.2 - Skill descriptor catalog with progressive disclosure; no full skill
  body in default context.
- G6.3 - Hook catalog and doctor surface; hooks disabled until workspace trust
  and policy refs allow them.
- G6.4 - Unified settings/effective-config projection for MCP, skills, hooks,
  and plugins.

Implementation status, 2026-06-16: #5138 adds the first refs-only MCP
capability catalog projection with configured, pending, disabled, failed, and
needs-auth states, stable ordering, state counts, public-safe server/capability/
policy/auth/blocker refs, explicit non-authority flags, and unsafe-material
omission coverage. It does not add MCP runtime clients, network calls, tool
calls, settings edits, auth flows, or UI.

Implementation status, 2026-06-16: #5139 adds the first refs-only skill
descriptor catalog projection with available, pending, disabled, failed, and
needs-review states, stable ordering, state counts, summary/trigger/source/
policy/body-request refs, progressive-disclosure flags proving full skill
bodies are not included by default, explicit non-authority flags, and
unsafe-material omission coverage. It does not add skill loading, full-body
rendering, prompt injection, tool execution, settings UI, or runtime policy
changes.

Implementation status, 2026-06-16: #5140 adds the first refs-only hook catalog
and doctor projection with configured, pending, disabled, failed, and
needs-trust states, trust/policy execution gates, doctor/event/descriptor refs,
explicit non-authority flags, and unsafe-material omission coverage. Configured
hooks without workspace-trust and policy refs are projected as needs-trust. It
does not add hook execution, shell execution, filesystem watching, event
dispatch, UI, settings edits, or runtime policy changes.

Implementation status, 2026-06-16: #5141 adds the first unified refs-only
extensibility effective-config projection across MCP, skills, hooks, and
plugins, including per-domain counts, effective-state counts, config/catalog/
policy/source/blocker refs, explicit non-authority flags, deterministic
ordering, and unsafe-material omission coverage. It does not add settings UI,
config writes, runtime policy changes, loading/execution, tool calls, hook
execution, or network calls.

### G7 - Verification, Ops, And Product-Gate Evidence

Systems: system 51 Testing/Smoke, system 52 Evaluation/Regression, system 53
Security Review, system 54 Retention/Deletion, system 47 Telemetry/Privacy.

Goal: make every surfaced system carry proof and retention posture before it
affects public copy.

Candidate child issues:

- G7.1 - Add a #5107 dashboard/readiness projection that shows which systems
  are surfaced, tested, public-safe, stale, or blocked.
- G7.2 - Add regression fixtures for private material across the G1-G6 lanes.
- G7.3 - Update product-promise evidence only after a signed, deployed, live
  smoke proves the exact claim.

Implementation status, 2026-06-16: #5142 adds the first refs-only terminal-agent
systems readiness projection for #5107, including system/group/evidence/test/
public-safety/blocker refs, surfaced/tested/public-safe/stale/blocked counts,
deterministic ordering, public-safe output, and unsafe-material omission
coverage. It does not add UI, product-promise changes, deployment gates, public
claims, eval execution, retention policy changes, or runtime authority changes.

## Recommended Sequencing

1. G1.1 diff-review lane, because Pack C already provides safe refs and the
   cockpit already owns review actions.
2. G2.1/G2.2 progress lane, because plan/progress turns Runs into an operator
   workflow instead of a flat list.
3. G4.1/G4.2 context lane, because it explains why the runner is ready or
   blocked before execution starts.
4. G3 session navigation, once the read-only evidence is clear and destructive
   resume/rewind actions can be gated.
5. G5 retrieval planner, before any broad semantic routing claims.
6. G6 extensibility catalog, only as catalog/readiness first; tool invocation
   comes later behind approvals.
7. G7 verification/readiness projection, then product-promise changes only if
   the deployed product has live evidence.

## First Issue To File

Open exactly one child issue now:

Title:

`[5107/G1] Forge cockpit diff-review lane over Pack C change captures`

Rationale:

It is the smallest, most grounded continuation of #5107: the evidence exists in
Pack C and Pylon, the cockpit exists in `/autopilot`, and the work is scoped to
rendering/guarding review facts rather than changing settlement, payout, or
runtime authority.
