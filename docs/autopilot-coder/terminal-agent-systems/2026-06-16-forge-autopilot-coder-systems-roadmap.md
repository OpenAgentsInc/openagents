# Forge Autopilot Coder Terminal-Agent Systems Roadmap

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


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

Implementation status, 2026-06-16: #5147 adds the first authority-gated
session control contract. Session summaries can now advertise supported
resume/fork/rewind/cancel verbs, public-safe authority refs, policy refs,
freshness, and blockers; `/autopilot` renders enabled POST controls only when
that advertised contract is fresh and unblocked. Unsupported, stale, missing-
authority, and missing-policy controls remain disabled with explicit blocker
refs. Public-safe control receipts render alongside the session summaries. The
web surface still does not execute or fabricate runtime state; the authoritative
bridge/runtime owns the actual control effect.

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

Implementation status, 2026-06-16: #5148 adds the durable repository memory
profile projection and renders it inside the `/autopilot` Context snapshot
lane. The profile carries repo identity, command/test/instruction/invariant
profile refs, latest refresh receipt refs, changed profile kinds, freshness,
generated/refreshed timestamps, and blockers. It marks profiles stale when the
current worktree is dirty or current instruction refs differ from the persisted
profile, blocks profiles without dev-doctor/profile evidence, and omits raw
workspace content, local paths, prompts, provider payloads, and private repo
material before rendering.

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

Implementation status, 2026-06-16: #5149 adds bounded live retrieval adapters
for file, documentation, and diagnostic source refs. The adapters emit the
existing `ForgeRetrievalPlanInput` contract, require explicit workspace-boundary
refs, block semantic/model-selected/hybrid modes without provider-evidence
refs, preserve deterministic selected/skipped ranking, and omit unsafe/private
source/query/provenance material before the `/autopilot` Retrieval search lane
renders it.

Implementation status, 2026-06-17 (epic #5107): system 27 Help/Doctor now has
its own first-class refs-only projection, `projectForgeSupportDiagnostics()`,
and a dedicated `/autopilot` Help, doctor, and debug lane separate from the
retrieval planner. The projection folds help-command, doctor-check (with
category, severity, evidence, and fix refs), preflight, support-bundle-section
(with per-section consent), and diagnostic-log evidence into one view. It
derives an empty/ready/attention/failing status from doctor severity, emits a
consent-gated export-readiness verdict (blocked/consent_required/ready) that
stays blocked while any doctor check errors or unsafe material is present, and
omits secret-like tokens, raw diffs/prompts, absolute/relative paths, URLs, and
shell-metacharacter refs before rendering. It carries an all-false authority
block and cannot run doctor/preflight checks, build or export support bundles,
grant consent, mutate settings, or read credentials.

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

Implementation status, 2026-06-16: #5150 adds guarded extensibility execution
request receipts over that effective-config projection. MCP tool/resource
requests, skill-body disclosure requests, hook enablement, plugin activation,
and settings activation now project callable/disabled/needs-auth/needs-trust/
blocked/failed/pending receipts with config/catalog/policy/source/auth/provider/
workspace-trust/failure refs. Skill disclosure remains explicit and body-free by
default; hooks/plugins/settings stay blocked without workspace-trust refs; MCP
tool calls stay blocked without provider-account refs. The browser still does
not execute MCP calls, hooks, plugin code, settings writes, or skill-body loads.

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

Implementation status, 2026-06-16: #5143 adds a cross-lane private-material
regression fixture covering representative G1-G6 projections: context, session
navigation, retrieval, MCP, skill descriptors, hooks, and unified
extensibility config. The test uses synthetic private markers only and proves
safe refs survive while local paths, raw prompts/transcripts/shell material,
provider payloads, tokens, and private material markers are omitted.

Implementation status, 2026-06-16: #5144 adds the first refs-only
product-promise evidence gate for exact #5107 claims. The gate reports ready
only when claim, product-promise, signed deploy, live-smoke, signature, and
public-safety refs are present; otherwise it emits missing-evidence blockers.
It is explicitly non-authoritative and does not write product promises, change
public copy, run deploys or smokes, mutate deployment gates, or change runtime
authority.

## Completion Status

The scoped child slices for this roadmap are implemented in issues #5123
through #5144. The result is an operator-facing projection foundation for the
terminal-agent systems named by #5107, plus safety tests and readiness gates.
This does not mean OpenAgents can publish broader product claims by default:
any public product-promise update still needs exact signed deploy and live-smoke
evidence through the G7.3 gate.

## Next Wave Filed

The next #5107 wave moves from read-only projections toward operational
integration while preserving the same authority boundaries:

- #5145 / H1: public-safe diff artifact drilldown for the Forge review lane.
- #5146 / H2: runtime plan/todo mutation receipts behind authority gates.
- #5147 / H3: session resume/fork/rewind control verbs through the authoritative
  bridge.
- #5148 / H4: repository memory profile persistence and onboarding refresh
  events.
- #5149 / H5: bounded live retrieval adapters for files, docs, and diagnostics.
- #5150 / H6: guarded extensibility execution-request receipts.

This wave should turn the first-wave evidence surfaces into constrained
operator workflows. It still does not close the #5107 parent epic, because the
full parent covers the wider 62-system terminal-agent map.

Implementation status, 2026-06-16: #5145 adds the public-safe diff artifact
drilldown for the existing Forge Review changes lane. The drilldown is composed
from bounded Pack C/artifact-review refs, renders artifact/file/hunk summary
refs, digest/provenance/caveat/blocker refs, and explicit non-authority flags,
and keeps raw patches, private repo content, local paths, shell output, prompts,
and provider payloads out of the Run projection.

Implementation status, 2026-06-16: #5146 adds typed plan/todo mutation request
and receipt projection for the Run progress lane. The projection renders
requested, applied, blocked, and stale mutations with actor, generatedAt,
provenance, request, receipt, and blocker refs; carries explicit non-authority
flags; blocks plan-complete receipts from implying Run completion without
closeout evidence; and omits raw/private plan material before rendering.

Implementation status, 2026-06-16: #5147 adds the session control request and
receipt surface for the Session navigation lane. Resume/fork/rewind/cancel
controls become submit-capable only when the session advertises support plus
fresh public-safe authority and policy refs; otherwise the UI preserves the
blocked/stale/unavailable state. Receipts expose only refs, actor/provenance
refs, outcome, generatedAt, and blockers, with private material omitted before
rendering.

Implementation status, 2026-06-16: #5148 adds `ForgeRepositoryMemoryProfile`,
the durable refs-only repository-memory record linked from context snapshots.
It reuses the existing repository-profile refresh receipt projection, exposes
latest freshness and changed profile kinds in `/autopilot`, and invalidates
profiles on dirty worktrees, changed instruction refs, missing dev-doctor
evidence, missing profile evidence, or unsafe material.

Implementation status, 2026-06-17: #5295 extends the same
`ForgeRepositoryMemoryProfile` projection with OpenAgents StudyBench and study
packet refs for Forge Autopilot Coder. The Context snapshot now carries
study-packet, corpus-manifest, dataset, public-retained score, private
validation trend, holdout evaluation, freshness, and blocked-claim refs while
labeling the lane as internal dogfood with an evidence-only authority boundary
and no mutation authority. The sanitizer omits hidden rubrics, hidden gold
answers, raw repo archives, private customer source refs, local paths, raw
commands, and credential-shaped material before the UI can render it.

Implementation status, 2026-06-16: #5149 adds the live-adapter builder for
bounded file/doc/diagnostic retrieval. It keeps the retrieval plan as the only
UI-facing contract, requires explicit source and workspace-boundary refs,
blocks provider-backed semantic/model modes without provider evidence, records
low-score/duplicate/stale/missing-source/unsupported skips, and includes a
regression proving prose-like query text does not trigger keyword-only routing.

Implementation status, 2026-06-16: #5150 adds the guarded extensibility
execution-request receipt surface and renders it in `/autopilot`. It turns the
G6 effective-config refs into request receipts without execution authority,
preserves progressive skill-body disclosure, and records the policy/auth/trust/
provider blockers that must clear before runtime can execute anything.

## I1 - Error Taxonomy And Recovery Lane

System: system 11 Error Taxonomy And Recovery.

Issue: #5198.

Implementation status, 2026-06-17: #5198 adds the first refs-only
`ForgeErrorRecoveryView`, typed Run projection fields for failure category,
severity, retryability, recovery strategy, redaction class, recovery events,
recovery refs, and blockers, plus a new `/autopilot` Run-detail Error recovery
lane. The projection has no automatic retry, runtime mutation, deployment,
public-claim, accepted-outcome, payout, or settlement authority. It fail-closes
invalid runs when recovery evidence is missing, blocks backoff retry without
idempotency evidence, and drops raw stacks, shell logs, provider payloads,
local paths, prompts, credentials, and unsafe private diagnostics before
rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 20 files / 130 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy` and `stageNodeGlyph`, unrelated to this slice.

## I2 - Compaction And Summarization Boundary Lane

System: system 13 Compaction And Summarization.

Issue: #5199.

Implementation status, 2026-06-17: #5199 adds the first refs-only
`ForgeCompactionSummaryView`, optional Run projection fields for compaction
boundary refs, trigger, strategy, state, pre/post context estimates, summary
source refs, preserved message/tool/task/plan/adapter refs, restored file/
task/plan/adapter/skill refs, policy/hook refs, failure/retry refs, and
blockers, plus a new `/autopilot` Run-detail Compaction lane. The projection
has no transcript mutation, model summarization, automatic compaction,
runtime retry, deployment, public-claim, accepted-outcome, payout, or
settlement authority. It blocks failed/cancelled boundaries that appear to
have post-compaction state, unmatched tool request/result pairs, repeated
failed automatic compactions, missing boundary evidence when a compaction
record is present, and unsafe/private compaction material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 21 files / 138 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy` and `stageNodeGlyph`, unrelated to this slice.

## I3 - Token Usage And Budget Lane

System: system 14 Token And Cost Budgeting.

Issue: #5202.

Implementation status, 2026-06-17: #5202 adds the first refs-only
`ForgeUsageBudgetView`, optional Run projection fields for usage refs, context
estimate refs, provider/model refs, token counts by dimension, usage truth,
budget thresholds, cost estimate refs, known/unknown pricing state, rate-limit
refs, quota blockers, and policy refs, plus a new `/autopilot` Run-detail Usage
and budget lane. The projection has no budget enforcement, spend
authorization, provider retry, max-output escalation, pricing write,
deployment, public-claim, accepted-outcome, payout, or settlement authority. It
blocks unknown pricing from exact-cost claims, output-only usage from context
headroom projection, synthetic usage from real-provider accounting, mixed
token/cost thresholds, quota/rate-limit blockers, and unsafe/private usage
material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 22 files / 146 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy` and `stageNodeGlyph`, unrelated to this slice.

## I4 - Model Provider Resolution Lane

System: system 15 Model Provider Abstraction.

Issue: #5205.

Implementation status, 2026-06-17: #5205 adds the first refs-only
`ForgeModelProviderView`, optional Run projection fields for requested alias
refs, selected provider/model refs, public-safe provider-facing model refs,
capability refs and limits, entitlement refs, validation state/refs, fallback
refs, policy refs, pricing refs, privacy/telemetry refs, and blockers, plus a
new `/autopilot` Run-detail Model provider lane. The projection has no provider
SDK, model-call, stream-parsing, provider-retry, model-switch, settings-write,
credential, pricing-write, deployment, public-claim, accepted-outcome, payout,
or settlement authority. It blocks selected models without capability or
entitlement evidence, fallback selections without fallback evidence,
unavailable claims without failed validation evidence, and unsafe/private model
provider material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 23 files / 153 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy` and `stageNodeGlyph`, unrelated to this slice.

## I5 - Prompt And Instruction Layering Lane

System: system 16 Prompt And Instruction Layering.

Issue: #5206.

Implementation status, 2026-06-17: #5206 adds the first refs-only
`ForgeInstructionLayeringView`, optional Run projection fields for instruction
snapshot refs, projection refs, version refs, layer refs, layer kind,
precedence, state, freshness, redaction class, token estimates, source refs,
metadata refs, policy refs, allowed-tool refs, capability-delta refs,
replacement-source refs, and blockers, plus a new `/autopilot` Run-detail
Instruction layering lane. The projection has no prompt-assembly, prompt
override-write, model-call, memory-write, settings-write, skill/command-load,
tool-grant, provider, deployment, public-claim, accepted-outcome, payout, or
settlement authority. It blocks skipped/replaced runtime policy layers,
replacements without evidence, skill/command tool grants without policy refs,
provider projections without snapshot refs, populated layers without snapshot
refs, and unsafe/private instruction material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 24 files / 160 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I6 - Session Memory Lane

System: system 17 Session Memory System.

Issue: #5209.

Implementation status, 2026-06-17: #5209 adds the first refs-only
`ForgeSessionMemoryView`, optional Run projection fields for memory snapshot
refs, projection refs, version refs, memory entry refs, memory kind, lifecycle
state, scope, freshness, retention class, redaction class, source refs, summary
refs, retrieval refs, compaction refs, policy refs, conflict refs, and blockers,
plus a new `/autopilot` Run-detail Session memory lane. The projection has no
memory-write, prompt-assembly, transcript-summarization, memory-compaction,
retention-policy-write, model-call, skill/command-load, tool-grant, provider,
deployment, public-claim, accepted-outcome, payout, or settlement authority. It
blocks stale memory without refresh evidence, superseded memory without conflict
evidence, retained memory without policy refs, populated entries without
snapshot refs, projections without snapshot refs, and unsafe/private session
memory material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 25 files / 169 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I7 - LSP And Diagnostics Lane

System: system 20 LSP And Diagnostics System.

Issue: #5211.

Implementation status, 2026-06-17: #5211 adds the first refs-only
`ForgeDiagnosticsView`, optional Run projection fields for diagnostics snapshot
refs, version refs, language-server refs, workspace-boundary refs, diagnostic
refs, severity counts, source refs, freshness, indexed-at refs/timestamps,
policy refs, remediation/code-action refs, skipped diagnostic refs, and
blockers, plus a new `/autopilot` Run-detail Diagnostics lane. The projection
has no LSP process/configuration, diagnostics execution, file-read, code-action,
edit, retrieval-routing, shell, tool-grant, provider, deployment, public-claim,
accepted-outcome, payout, or settlement authority. It blocks stale diagnostics
without refresh evidence, diagnostics without workspace-boundary refs or
language-server evidence, remediation/code-action refs without policy refs,
populated entries without snapshot refs, and unsafe/private diagnostic material
before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 26 files / 178 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I8 - Terminal UI Shell Lane

System: system 21 Terminal UI Shell.

Issue: #5213.

Implementation status, 2026-06-17: #5213 adds the first refs-only
`ForgeTerminalUiShellView`, optional Run projection fields for terminal surface
snapshot refs, version refs, surface refs, shell refs, pane refs, stream refs,
transcript summary refs, command/input descriptor refs, non-interactive refs,
accessibility refs, parity refs, freshness, mode, state, policy refs, and
blockers, plus a new `/autopilot` Run-detail Terminal UI shell lane. The
projection has no terminal-emulator, terminal-process, PTY, input-injection,
keybinding-write, command-execution, shell-execution, file-read, tool-grant,
provider, deployment, public-claim, accepted-outcome, payout, or settlement
authority. It blocks stale terminal surface evidence, interactive available
surfaces without policy refs, available surfaces without shell evidence,
available surfaces without stream or pane evidence, populated surfaces without
snapshot refs, and unsafe/private terminal material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 27 files / 187 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I9 - Input And Keybinding Lane

System: system 22 Input And Keybinding System.

Issue: #5214.

Implementation status, 2026-06-17: #5214 adds the first refs-only
`ForgeInputKeybindingView`, optional Run projection fields for input/keybinding
snapshot refs, version refs, binding map refs, input mode refs, command
descriptor refs, keymap refs, conflict refs, platform refs, accessibility refs,
non-interactive fallback refs, policy refs, freshness, state, and blockers,
plus a new `/autopilot` Run-detail Input and keybinding lane. The projection
has no keyboard-capture, input-injection, command-execution,
keybinding-execution, keybinding-write, input-mode-write, terminal-process,
shell-execution, file-read, tool-grant, provider, deployment, public-claim,
accepted-outcome, payout, or settlement authority. It blocks stale input
evidence, available input modes without policy refs, degraded keymaps without
conflict evidence, interactive modes without non-interactive fallback refs,
available modes without command descriptor refs, populated entries without
snapshot refs, and unsafe/private input or keybinding material before
rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 28 files / 197 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I10 - Command System Lane

System: system 23 Command System.

Issue: #5216.

Implementation status, 2026-06-17: #5216 adds the first refs-only
`ForgeCommandSystemView`, optional Run projection fields for command snapshot
refs, version refs, command refs, command kind, command state, parser/planner
refs, input mode refs, command descriptor refs, capability refs, policy refs,
conflict refs, route/semantic selector refs, fallback refs, freshness, and
blockers, plus a new `/autopilot` Run-detail Command system lane. The
projection has no command-execution, intent-routing, retrieval-routing,
parser-execution, catalog-write, settings-write, keybinding-write,
input-mode-write, shell-execution, file-read, tool-grant, provider, deployment,
public-claim, accepted-outcome, payout, or settlement authority. It blocks stale
command evidence, available commands without policy refs, available command
routes without semantic selector refs, available commands without parser/planner
refs, conflicted commands without conflict evidence, unavailable commands
without fallback refs, available commands without descriptor refs, populated
commands without snapshot refs, and unsafe/private command material before
rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 29 files / 207 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I11 - Notifications And Attention Lane

System: system 25 Notifications And Attention System.

Issue: #5217.

Implementation status, 2026-06-17: #5217 adds the first refs-only
`ForgeNotificationAttentionView`, optional Run projection fields for attention
snapshot refs, version refs, attention refs, attention state, severity,
freshness, notification refs, channel refs, delivery refs, decision/action
refs, dedupe/fold refs, invalidation/resolution refs, policy refs, and
blockers, plus a new `/autopilot` Run-detail Notifications and attention lane.
The projection has no notification-send, notification-subscription,
approval-request, decision-action, attention-resolution, run-state-mutation,
shell-execution, file-read, tool-grant, provider, deployment, public-claim,
accepted-outcome, payout, or settlement authority. It blocks stale attention
evidence, active/waiting attention without policy refs, notifications without
delivery refs, waiting attention without decision/action refs, resolved/
invalidated attention without closeout refs, populated entries without snapshot
refs, and unsafe/private notification or attention material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 30 files / 217 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I12 - Help, Doctor, And Debug Lane

System: system 27 Help, Doctor, And Debug Surfaces.

Issue: #5222.

Implementation status, 2026-06-17: #5222 adds the first refs-only
`ForgeHelpDoctorDebugView`, optional Run projection fields for help/doctor/debug
snapshot refs, version refs, surface refs, help topic refs, doctor check refs,
diagnostic refs, debug bundle refs, remediation refs, source refs, policy refs,
state, severity, freshness, and blockers, plus a new `/autopilot` Run-detail
Help, doctor, and debug lane. The projection has no doctor-check execution,
diagnostics execution, debug-bundle collection, log collection, file-read,
shell-execution, settings-write, run-state-mutation, tool-grant, provider,
deployment, public-claim, accepted-outcome, payout, or settlement authority. It
blocks stale doctor evidence, failed/blocked checks without remediation refs,
debug bundle refs without policy refs, entries without help/doctor/diagnostic
evidence, populated entries without snapshot refs, and unsafe/private help,
doctor, diagnostic, or debug material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 31 files / 227 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I13 - MCP Server Export Lane

System: system 29 MCP Server System.

Issue: #5224.

Implementation status, 2026-06-17: #5224 adds the first refs-only
`ForgeMcpServerExportView`, optional Run projection fields for MCP server/export
snapshot refs, version refs, server refs, exported capability/tool/resource/
prompt refs, schema refs, transport refs, auth policy refs, audience/trust-tier
refs, invocation receipt refs, exposure state, freshness, source refs, policy
refs, and blockers, plus a new `/autopilot` Run-detail MCP server export lane.
The projection has no MCP server hosting, transport exposure, remote invocation,
tool routing, tool execution, settings/effective-config mutation, credential,
file-read, shell-execution, deployment, public-claim, accepted-outcome, payout,
or settlement authority. It blocks stale server export evidence, exported
surfaces without schema refs or policy refs, remote exposure without auth/
audience/trust-tier refs, invocation receipts without exported capability refs,
populated entries without snapshot refs, and unsafe/private MCP server export
material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 32 files / 238 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I14 - Settings And Configuration Lane

System: system 33 Settings And Configuration System.

Issue: #5227.

Implementation status, 2026-06-17: #5227 adds the first refs-only
`ForgeSettingsConfigurationView`, optional Run projection fields for settings/
configuration snapshot refs, version refs, scope refs, setting refs, source
refs, effective-value refs, default/override refs, validation refs, policy
refs, redaction class, redaction refs, state, freshness, and blockers, plus a
new `/autopilot` Run-detail Settings and configuration lane. The projection has
no settings-read, settings-write, settings-activation, effective-config
mutation, credential, tool-routing, tool-execution, file-read, shell-execution,
deployment, public-claim, accepted-outcome, payout, or settlement authority. It
blocks stale configuration evidence, enabled/overridden settings without policy
refs, effective values without validation refs, private/local settings without
redaction refs, populated entries without snapshot refs, and unsafe/private
settings material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 33 files / 249 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I15 - Authentication And Credential Storage Lane

System: system 34 Authentication And Credential Storage.

Issue: #5229.

Implementation status, 2026-06-17: #5229 adds the first refs-only
`ForgeCredentialStorageView`, optional Run projection fields for auth/credential
snapshot refs, version refs, account refs, credential refs, credential kind,
storage backend refs, lease/session refs, scope refs, entitlement refs,
rotation/revocation refs, validation refs, policy refs, redaction class,
redaction refs, state, freshness, and blockers, plus a new `/autopilot`
Run-detail Authentication and credential storage lane. The projection has no
credential-read, credential-write, credential-mint, credential-refresh,
credential-rotation, credential-revocation, authentication, provider-account,
tool-routing, tool-execution, file-read, shell-execution, deployment,
public-claim, accepted-outcome, payout, or settlement authority. It blocks
stale credential evidence, usable credentials without policy refs, validation
refs, or storage refs, revoked/expired credentials without rotation/revocation
closeout refs, private/local credentials without redaction refs, populated
entries without snapshot refs, and unsafe/private credential material before
rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 34 files / 262 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I16 - Git And GitHub Workflow Lane

System: system 35 Git And GitHub Workflow System.

Issue: #5231.

Implementation status, 2026-06-17: #5231 adds the first refs-only
`ForgeGitWorkflowView`, optional Run projection fields for Git/GitHub workflow
snapshot refs, version refs, workflow refs, repository refs, branch refs,
commit refs, worktree refs, diff refs, PR refs, issue refs, review refs, check/
status refs, writeback refs, policy refs, state, freshness, and blockers, plus
a new `/autopilot` Run-detail Git and GitHub workflow lane. The projection has
no git/gh execution, branch/commit/tag creation, PR creation, review
submission, issue-comment, check-run, GitHub write, repository file-read,
writeback, deployment, public-claim, accepted-outcome, payout, or settlement
authority. It blocks stale workflow evidence, PR-ready state without branch/
diff/check refs, review-ready state without review/policy refs,
writeback-ready state without writeback/policy refs, populated entries without
snapshot refs, and unsafe/private Git/GitHub material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 35 files / 273 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I17 - IDE And Editor Integration Lane

System: system 36 IDE And Editor Integration.

Issue: #5234.

Implementation status, 2026-06-17: #5234 adds the first refs-only
`ForgeEditorIntegrationView`, optional Run projection fields for IDE/editor
integration snapshot refs, version refs, integration refs, editor refs,
workspace refs, extension refs, command refs, diagnostic refs, diagnostic
handoff refs, file-open refs, selection refs, deep-link refs, status refs,
policy refs, state, freshness, and blockers, plus a new `/autopilot`
Run-detail IDE and editor integration lane. The projection has no editor/IDE
automation, editor-command, extension-install, file-open, file-read, file-write,
selection-read, shell-execution, tool-routing, tool-execution, deployment,
public-claim, accepted-outcome, payout, or settlement authority. It blocks
stale editor evidence, ready/connected state without workspace/policy refs,
deep links without policy refs, diagnostic handoffs without diagnostic refs,
populated entries without snapshot refs, and unsafe/private editor material
before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 36 files / 284 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  only by pre-existing `src/scene/tassadarRunSnapshot*` typing errors for
  `motionPolicy`, `sceneChrome`, and `stageNodeGlyph`, unrelated to this
  slice.

## I18 - Browser And Desktop Integration Lane

System: system 37 Browser And Desktop Integration.

Issue: #5235.

Implementation status, 2026-06-17: #5235 adds the first refs-only
`ForgeBrowserDesktopIntegrationView`, optional Run projection fields for
browser/desktop integration snapshot refs, version refs, integration refs,
surface refs, browser refs, desktop app refs, extension refs, deep-link refs,
permission refs, notification refs, companion refs, install/update refs, status
refs, policy refs, state, freshness, and blockers, plus a new `/autopilot`
Run-detail Browser and desktop integration lane. The projection has no browser
automation, desktop control, extension-install, notification-send,
permission-inspect, session-inspect, deep-link-open, file-read, shell-execution,
tool-routing, tool-execution, deployment, public-claim, accepted-outcome,
payout, or settlement authority. It blocks stale browser/desktop evidence,
ready/connected surfaces without policy refs, deep links without policy refs,
notifications without permission refs, installed surfaces without install/
update refs, populated entries without snapshot refs, and unsafe/private
browser or desktop material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 37 files / 296 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by current baseline scene typing errors in `src/scene/tassadarRunElement.ts`
  (`cameraMode`, implicit `locked`) and `src/scene/tassadarRunSnapshot*`
  (`motionPolicy`, `sceneChrome`, and `stageNodeGlyph`), unrelated to this
  slice.

## I19 - Voice And Multimodal Input Lane

System: system 38 Voice And Multimodal Input.

Issue: #5240.

Implementation status, 2026-06-17: #5240 adds the first refs-only
`ForgeMultimodalInputView`, optional Run projection fields for voice/multimodal
input snapshot refs, version refs, input refs, capture surface refs, modality,
attachment refs, transcript refs, VAD/endpoint refs, consent refs, redaction
refs, context-ingestion refs, policy refs, state, freshness, and blockers, plus
a new `/autopilot` Run-detail Voice and multimodal input lane. The projection
has no microphone/camera/screen/clipboard access, media capture, transcription,
VAD execution, image/video processing, file attach/read, prompt authority,
instruction injection, shell-execution, tool-routing, tool-execution,
deployment, public-claim, accepted-outcome, payout, or settlement authority. It
blocks stale multimodal evidence, capture-ready state without consent/policy
refs, transcripts without redaction refs, context ingestion without attachment
refs, populated entries without snapshot refs, and unsafe/private media,
transcript, or prompt material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 38 files / 307 tests.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by current baseline scene typing errors in `src/scene/tassadarRunElement.ts`
  (`cameraMode`, implicit `locked`) and `src/scene/tassadarRunSnapshot*`
  (`motionPolicy`, `sceneChrome`, and `stageNodeGlyph`), unrelated to this
  slice.

## I20 - Remote Session Bridge Lane

System: system 39 Remote Session Bridge.

Issue: #5242.

Implementation status, 2026-06-17: #5242 adds the first refs-only
`ForgeRemoteSessionBridgeView`, optional Run projection fields for bridge
snapshot refs, version refs, bridge refs, session refs, transport refs,
protocol refs, controller refs, heartbeat refs, reconnect refs, permission
refs, policy refs, state, freshness, and blockers, plus a new `/autopilot`
Run-detail Remote Session Bridge lane. The projection has no remote-session
open/control/reconnect/terminate, remote-command, remote-host-inspection,
log-streaming, file-read, shell-execution, tool-grant, credential/settings
mutation, deployment, public-claim, accepted-outcome, payout, or settlement
authority. It blocks stale bridge evidence, ready/connected bridge state
without transport/protocol/policy refs, reconnecting bridge state without
reconnect refs, controller refs without permission refs, populated entries
without snapshot refs, and unsafe/private remote session, host, transport, log,
command, or token material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work/remote-session-bridge.test.ts src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 2 files / 77 tests.
- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 39 files / 318 tests.
- `git diff --check` passes.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by current baseline scene typing errors in `src/scene/tassadarRunElement.ts`
  (`cameraMode`, implicit `locked`) and `src/scene/tassadarRunSnapshot*`
  (`motionPolicy`, `sceneChrome`, and `stageNodeGlyph`), unrelated to this
  slice.

## I21 - Mobile And Web Companion Lane

System: system 40 Mobile And Web Companion System.

Issue: #5245.

Implementation status, 2026-06-17: #5245 adds the first refs-only
`ForgeCompanionSurfaceView`, optional Run projection fields for companion
snapshot refs, version refs, companion refs, surface refs, pairing refs, stream
refs, cursor refs, run/session refs, decision refs, notification refs,
artifact/closeout refs, progress refs, budget refs, action refs, capability
refs, idempotency refs, delivery-tier refs, receipt refs, policy refs, lag
refs, state, freshness, and blockers, plus a new `/autopilot` Run-detail
Mobile and web companion lane. The projection has no notification-send,
decision-resolution, offline-action queue, pause/resume/cancel/interrupt/spawn,
instruction-queue, terminal-open, private-log streaming, file-read,
session-mutation, deployment, public-claim, accepted-outcome, payout, or
settlement authority. It blocks stale or lagged companion evidence, action refs
without capability/policy/pairing/idempotency/receipt refs, stream refs without
cursor refs, populated entries without snapshot refs, and unsafe/private
companion, mobile, terminal, artifact, decision, progress, prompt, session, log,
token, or credential material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work/companion-surface.test.ts src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 2 files / 79 tests.
- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 40 files / 329 tests.
- `git diff --check` passes.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by current baseline scene typing errors in `src/scene/tassadarRunElement.ts`
  (`cameraMode`, implicit `locked`) and `src/scene/tassadarRunSnapshot*`
  (`motionPolicy`, `sceneChrome`, and `stageNodeGlyph`), unrelated to this
  slice.

## I22 - Team And Shared Memory Lane

System: system 41 Team And Shared Memory System.

Issue: #5246.

Implementation status, 2026-06-17: #5246 adds the first refs-only
`ForgeTeamSharedMemoryView`, optional Run projection fields for shared-memory
snapshot refs, projection refs, version refs, memory refs, memory
scope/kind/review state/visibility/redaction class/freshness, owner refs, team
refs, evidence refs, retrieval policy refs, typed/semantic query refs,
application receipt refs, deletion/tombstone receipt refs, consent refs,
promotion refs, policy refs, expiry/review refs, and blockers, plus a new
`/autopilot` Run-detail Team and shared memory lane. The projection has no
memory create/update/delete/promotion, semantic-retrieval, prompt-assembly,
model-call, tool-grant, team/project/mission mutation, deployment,
public-claim, accepted-outcome, payout, or settlement authority. It blocks
stale shared-memory evidence, team visibility without team/policy refs, public
visibility without public-safe redaction and policy refs, applied memories
without application receipts, deleted memories without deletion/tombstone
receipts, promoted memories without consent/policy refs, populated entries
without snapshot refs, and unsafe/private memory text, prompts, logs, provider
payloads, customer data, local paths, secrets, tokens, or credentials before
rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work/team-shared-memory.test.ts src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 2 files / 83 tests.
- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 41 files / 342 tests.
- `git diff --check` passes.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by current baseline scene typing errors in `src/scene/tassadarRunElement.ts`
  (`cameraMode`, implicit `locked`) and `src/scene/tassadarRunSnapshot*`
  (`motionPolicy`, `sceneChrome`, and `stageNodeGlyph`), unrelated to this
  slice.

## I23 - Multi-Agent Coordination Lane

System: system 42 Multi-Agent Coordination System.

Issue: #5247.

Implementation status, 2026-06-17: #5247 adds the first refs-only
`ForgeMultiAgentCoordinationView`, optional Run projection fields for
coordination snapshot refs, plan refs, parent run refs, version refs, lane refs,
assignment refs, dependency refs, budget cap refs, provider/adapter refs,
capability refs, artifact refs, receipt refs, conflict refs, merge strategy
refs, lane inbox refs, steering receipt refs, closeout refs, acceptance policy
refs, state, freshness, lane kind, mandatory/optional criticality, and
blockers, plus a new `/autopilot` Run-detail Multi-agent coordination lane.
The projection has no fanout planning, lane start/pause/resume/cancel, lane
inbox messaging, provider selection, artifact merge, lane output
accept/reject, assignment mutation, deployment, public-claim, accepted-outcome,
payout, or settlement authority. It blocks stale lane evidence, active lanes
without assignment/capability/policy refs, failed mandatory lanes without
closeout refs, market/external/hosted provider lanes without provider/policy/
receipt refs, conflict refs without merge-strategy/policy refs, lane inbox refs
without steering receipts, populated entries without snapshot refs, missing
coordination plan refs, and unsafe/private lane, artifact, provider, inbox,
prompt, log, local path, token, or credential material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work/multi-agent-coordination.test.ts src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 2 files / 85 tests.
- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 42 files / 355 tests.
- `git diff --check` passes.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by current baseline scene typing errors in `src/scene/tassadarRunElement.ts`
  (`cameraMode`, implicit `locked`) and `src/scene/tassadarRunSnapshot*`
  (`motionPolicy`, `sceneChrome`, and `stageNodeGlyph`), unrelated to this
  slice.

## I24 - External Work Intake Lane

System: system 43 External Work Intake System.

Issue: #5248.

Implementation status, 2026-06-17: #5248 adds the first refs-only
`ForgeExternalWorkIntakeView`, optional Run projection fields for intake
snapshot refs, version refs, intake refs, request refs, requester/account refs,
channel, work kind, scope/data-classification refs, capability refs, adapter
preference refs, budget/payment refs, verification refs, acceptance/review
policy refs, idempotency refs, admission/rejection/routing/work-order/status/
delivery receipt refs, API parity refs, expiration refs, status, freshness, and
blockers, plus a new `/autopilot` Run-detail External work intake lane. The
projection has no admission/rejection, work-order creation, work enqueue,
adapter selection, budget reservation, payment, execution start, deployment,
public-claim, accepted-outcome, payout, or settlement authority. It blocks
stale intake evidence, missing requester/account refs, required budget without
budget/payment refs, admitted/routed/delivered intake without capability refs,
adapter preferences without routing/policy refs, payment refs without
admission/routing receipts, pending/admitted/routed intake without idempotency
refs, browser intake without API parity refs, private/restricted intake scope
without policy refs, populated entries without snapshot refs, and unsafe/private
intake payload, request, repository, workspace, prompt, provider, customer data,
local path, token, or credential material before rendering.

Verification:

- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work/external-work-intake.test.ts src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 2 files / 87 tests.
- `bun run --cwd apps/openagents.com/apps/web test -- src/page/loggedIn/autopilot-work src/page/loggedIn/page/autopilot-work.test.ts`
  passes with 43 files / 368 tests.
- `git diff --check` passes.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by current baseline scene typing errors in `src/scene/tassadarRunElement.ts`
  (`cameraMode`, implicit `locked`) and `src/scene/tassadarRunSnapshot*`
  (`motionPolicy`, `sceneChrome`, and `stageNodeGlyph`), unrelated to this
  slice.

## I25 - Artifact And Receipt Index Lane

System: system 44 Artifact And Receipt System.

Issue: #5249.

Filed, 2026-06-17: #5249 should add a refs-only Artifact and Receipt index
projection and `/autopilot` Run-detail lane for artifact-index snapshot refs,
artifact refs, artifact kind, subject/run/mission/work-order/assignment/lane
refs, digest refs, size/media/visibility/redaction refs, retention refs,
producer refs, related receipt refs, receipt refs, receipt transition kind,
actor/service refs, idempotency refs, input/output refs, policy refs,
verification/caveat refs, claim requirement refs, freshness, blockers, and
unsafe-material omission warnings. It must not fetch/download raw artifact
payloads, store/delete artifacts, append/revoke receipts, widen artifact
visibility, satisfy claims, deploy, publish public claims, accept outcomes, pay
workers, or settle funds.

Implemented, 2026-06-17: #5249 added the `artifactReceiptIndex` Run projection
schema, the refs-only `projectForgeArtifactReceiptIndex` view model, and the
Run-detail Artifact and receipt index lane. The lane surfaces artifact,
digest, media, size, summary, subject, producer, retention, policy, related
receipt, receipt transition, actor, service, idempotency, input/output,
verification, caveat, claim requirement, satisfaction, freshness, snapshot,
version, and blocker refs while keeping all artifact storage/download/delete,
receipt append/revoke, visibility widening, claim satisfaction, deploy,
public-claim, accepted-outcome, payout, and settlement authorities false.
Unsafe raw artifact, receipt, provider payload, wallet, token, local path, and
shell material is omitted before projection/rendering and converted into an
explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/artifact-receipt-index.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 90 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 44 files, 382 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I26 - Scheduling And Cron Lane

System: system 45 Scheduling And Cron System.

Issue: #5251.

Filed, 2026-06-17: #5251 should add a refs-only Scheduling and Cron projection
and `/autopilot` Run-detail lane for schedule snapshot refs, version refs,
schedule refs, owner/team refs, work-order template refs, workspace/repo refs,
trigger kind, timezone refs, next/last run refs, budget policy refs,
permission policy refs, provider/adapter preference refs, notification policy
refs, retention policy refs, continuation policy refs, fire/skip/failure/
cancel/no-double-fire receipt refs, status, freshness, blockers, and unsafe
material omission warnings. It must not create, edit, pause, resume, delete, or
fire schedules; enqueue scheduler work; approve continuations; mutate budget,
spend, provider, credential, workspace, notification, deployment, public-claim,
accepted-outcome, payout, or settlement state.

Implemented, 2026-06-17: #5251 added the `schedulingCron` Run projection
schema, the refs-only `projectForgeSchedulingCron` view model, and the
Run-detail Scheduling and cron lane. The lane surfaces schedule, owner/team,
work-template, workspace/repo, trigger, timezone, next/last run, budget,
permission, provider/adapter, notification, retention, continuation, fire,
run, skip, failure, cancel, no-double-fire, freshness, snapshot, version, and
blocker refs while keeping all schedule create/update/pause/resume/delete,
scheduler enqueue/fire, continuation approval, budget, provider, credential,
notification-send, deployment, public-claim, accepted-outcome, payout, and
settlement authorities false. Unsafe raw schedule/cron material, local paths,
provider payloads, wallet material, tokens, credentials, customer-private data,
and shell material are omitted before projection/rendering and converted into
an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/scheduling-cron.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 93 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 45 files, 397 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I27 - Structured Event Log Lane

System: system 46 Structured Event Log.

Issue: #5253.

Filed, 2026-06-17: #5253 should add a refs-only Structured Event Log projection
and `/autopilot` Run-detail lane for event-log snapshot refs, version refs,
event stream refs, event refs, run refs, sequence refs/ranges, event kind,
subject refs, actor/service refs, timestamp refs, visibility, redaction class,
payload schema version refs, idempotency refs, parent/correlation refs, replay
refs, projection refs, export refs, retention refs, status, freshness,
blockers, and unsafe-material omission warnings. It must not append events,
delete events, tail private streams, execute replay, generate exports, migrate
schemas, delete retention records, mutate projections, deploy, publish public
claims, accept outcomes, pay workers, or settle funds.

Implemented, 2026-06-17: #5253 added the `structuredEventLog` Run projection
schema, the refs-only `projectForgeStructuredEventLog` view model, and the
Run-detail Structured event log lane. The lane surfaces event stream, event,
run, sequence, subject, actor/service, timestamp, payload schema version,
idempotency, parent/correlation, replay, projection, export, retention, policy,
visibility/redaction, freshness, status, snapshot, version, and blocker refs
while keeping all event append/delete/tail, replay execution, export
generation, schema migration, retention deletion, projection mutation,
deployment, public-claim, accepted-outcome, payout, and settlement authorities
false. Unsafe raw event payloads, private repo/prompt/log/provider/customer
material, wallet material, credentials, tokens, local paths, and shell material
are omitted before projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/structured-event-log.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 93 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 46 files, 410 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I28 - Telemetry And Privacy Lane

System: system 47 Telemetry And Privacy System.

Issue: #5255.

Filed, 2026-06-17: #5255 should add a refs-only Telemetry and Privacy
projection and `/autopilot` Run-detail lane for telemetry/privacy snapshot
refs, version refs, mode refs, collection class refs, sink refs, visibility
refs, retention refs, exportability refs, opt-out refs, policy refs, redaction
scan refs, privacy filter refs, health/cost/safety/usage aggregate refs,
diagnostic bundle refs, delivery/failure refs, status, freshness, blockers,
and unsafe-material omission warnings. It must not emit telemetry, change
telemetry mode/settings, activate/write sinks, generate diagnostic exports,
bypass privacy filters, mutate usage/billing, delete retention records, deploy,
publish public claims, accept outcomes, pay workers, or settle funds.

Implemented, 2026-06-17: #5255 added the `telemetryPrivacy` Run projection
schema, the refs-only `projectForgeTelemetryPrivacy` view model, and the
Run-detail Telemetry and privacy lane. The lane surfaces telemetry mode,
class, sink, visibility, retention, exportability, opt-out, policy, privacy
filter, redaction scan, aggregate, diagnostic bundle, delivery, failure,
freshness, status, snapshot, version, and blocker refs while keeping all
telemetry emit, mode/settings write, sink activation/write, diagnostic export,
privacy-filter bypass, usage/billing mutation, retention deletion, deployment,
public-claim, accepted-outcome, payout, and settlement authorities false.
Unsafe raw prompts, private code, raw command output, provider payloads,
invoices, wallet material, credentials, tokens, local paths, customer records,
and shell material are omitted before projection/rendering and converted into
an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/telemetry-privacy.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 95 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 47 files, 423 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I29 - Performance Diagnostics Lane

System: system 48 Performance System.

Issue: #5256.

Filed, 2026-06-17: #5256 should add a refs-only Performance diagnostics
projection and `/autopilot` Run-detail lane for performance snapshot refs,
version refs, span refs, counter refs, run refs, latency class, resource class,
queue/backpressure refs, timeout refs, output-volume refs, truncation refs,
artifact refs preserved by truncation, provider/rate-limit refs, budget-stop
refs, local-resource-pressure refs, redacted profile refs, status, freshness,
blockers, and unsafe-material omission warnings. It must not record metrics,
control backpressure, enforce timeouts, pause/cancel runs, generate profile
exports, read raw outputs, mutate budget/provider state, deploy, publish public
claims, accept outcomes, pay workers, or settle funds.

Implemented, 2026-06-17: #5256 added the `performanceDiagnostics` Run
projection schema, the refs-only `projectForgePerformanceDiagnostics` view
model, and the Run-detail Performance diagnostics lane. The lane surfaces
performance snapshot, version, span, counter, run, latency-class,
resource-class, queue/backpressure, timeout, output-volume, truncation,
truncation-preserved artifact, provider/rate-limit, budget-stop,
local-resource-pressure, redacted profile, redaction, policy, status,
freshness, and blocker refs while keeping all metrics-record,
backpressure-control, timeout-enforcement, run pause/cancel, profile-export,
raw-output-read, budget/provider mutation, deployment, public-claim,
accepted-outcome, payout, and settlement authorities false. Unsafe raw output,
profile payload, prompt, provider payload, local path, secret, credential,
token, key, wallet, invoice, customer, and private repo material are omitted
before projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/performance-diagnostics.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 97 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 48 files, 436 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I30 - Update And Release Lane

System: system 49 Update And Release System.

Issue: #5258.

Filed, 2026-06-17: #5258 should add a refs-only Update and Release projection
and `/autopilot` Run-detail lane for release snapshot refs, manifest refs,
version/channel refs, platform artifact refs, checksum/signature refs,
runtime compatibility refs, migration/restore-point refs, managed pin/policy
refs, rollout refs, release-note refs, smoke receipt refs, rollback refs,
deprecation/support refs, status, freshness, blockers, and unsafe-material
omission warnings. It must not perform update checks, fetch or verify release
manifests, install packages, run migrations or smokes, roll back versions,
mutate channels/pins/managed policy, deploy, publish public claims, accept
outcomes, pay workers, or settle funds.

Implemented, 2026-06-17: #5258 added the `updateRelease` Run projection schema,
the refs-only `projectForgeUpdateRelease` view model, and the Run-detail Update
and release lane. The lane surfaces release snapshot, manifest, version,
channel, platform artifact, checksum, signature, runtime compatibility,
migration, restore-point, managed pin, policy, rollout, release-note, smoke
receipt, rollback, deprecation, support, freshness, status, active-run,
safe-update-window, and blocker refs while keeping all update-check network,
manifest fetch/verification, installer, migration, smoke execution, rollback,
channel/pin mutation, managed-policy mutation, deployment, public-claim,
accepted-outcome, payout, and settlement authorities false. Unsafe release
notes, manifests, artifact payloads, shell logs, provider payloads, local
paths, credentials, tokens, secrets, customer records, and private repo
material are omitted before projection/rendering and converted into an
explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/update-release.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 100 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 49 files, 450 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I31 - Migration Evidence Lane

System: system 50 Migration System.

Issue: #5259.

Filed, 2026-06-17: #5259 should add a refs-only Migration evidence projection
and `/autopilot` Run-detail lane for migration snapshot refs, registry refs,
domain refs, schema version refs, migration step refs, idempotency refs,
restore-point refs, rollback-boundary refs, validation refs, receipt refs,
optional cache rebuild refs, downgrade refs, recovery refs, redaction/policy
refs, status, freshness, blockers, and unsafe-material omission warnings. It
must not mutate migration registries, execute migrations, create snapshots,
restore or roll back state, run validation, rebuild caches, transition startup
or recovery mode, execute downgrades, generate exports, deploy, publish public
claims, accept outcomes, pay workers, or settle funds.

Implemented, 2026-06-17: #5259 added the `migrationEvidence` Run projection
schema, the refs-only `projectForgeMigrationEvidence` view model, and the
Run-detail Migration evidence lane. The lane surfaces migration snapshot,
registry, domain, schema-version, migration-step, idempotency, restore-point,
rollback-boundary, validation, receipt, optional-cache rebuild, downgrade,
recovery, redaction, policy, freshness, status, and blocker refs while keeping
all migration registry mutation, migration execution, snapshot creation,
restore, rollback, validation execution, cache rebuild, startup/recovery mode
transition, downgrade execution, export generation, deployment, public-claim,
accepted-outcome, payout, and settlement authorities false. Unsafe private
state payloads, credential values, raw fixtures, migration logs, provider
payloads, local paths, tokens, secrets, customer records, and private repo
material are omitted before projection/rendering and converted into an
explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/migration-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 102 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 50 files, 464 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I32 - Testing And Smoke Evidence Lane

System: system 51 Testing And Smoke System.

Issue: #5260.

Filed, 2026-06-17: #5260 should add a refs-only Testing and Smoke evidence
projection and `/autopilot` Run-detail lane for test snapshot refs, test layer
refs, command refs, fixture refs, adapter/workspace/provider/credential
availability refs, smoke receipt refs, proof-boundary refs, no-spend/paid/
local/staging/live classification, explicit live-approval refs, redaction scan
refs, failure/blocker refs, environment/version refs, freshness, status, and
unsafe-material omission warnings. It must not execute tests or smokes, trigger
live spend/write/deploy/push/settlement, call providers, read credentials,
fixtures, artifacts, or smoke output, mutate product promises or public
claims, accept outcomes, pay workers, or settle funds.

Implemented, 2026-06-17: #5260 added the `testingSmokeEvidence` Run projection
schema, the refs-only `projectForgeTestingSmokeEvidence` view model, and the
Run-detail Testing and smoke lane. The lane surfaces test snapshot, test layer,
command, fixture, adapter/workspace/provider/credential availability, smoke
receipt, proof-boundary, no-spend/paid/local/staging/live classification,
explicit live-approval, redaction scan, failure, blocker, environment, version,
freshness, and status refs while keeping all test execution, smoke execution,
live spend/write/deploy/push/settlement, provider call, credential read,
fixture read, artifact read, smoke-output read, product-promise mutation,
public-claim mutation, accepted-outcome, payout, and settlement authorities
false. Unsafe private logs, secrets, provider payloads, fixture bodies,
workspace paths, smoke output, artifact contents, tokens, customer records, and
private repo material are omitted before projection/rendering and converted
into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/testing-smoke-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 102 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 51 files, 476 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I33 - Evaluation And Regression Evidence Lane

System: system 52 Evaluation And Regression System.

Issue: #5266.

Filed, 2026-06-17: #5266 should add a refs-only Evaluation and Regression
evidence projection and `/autopilot` Run-detail lane for eval snapshot refs,
suite refs, fixture refs, fixture provenance/redaction refs, adapter/provider/
model refs, runtime version refs, tool-policy refs, budget-policy refs, result
verdict refs, first-divergence refs, artifact refs, cost/latency summary refs,
safety verdict refs, public/private report refs, regression gate refs,
threshold refs, status, freshness, blockers, and unsafe-material omission
warnings. It must not load eval suites, execute evals or regressions, call
providers/models, generate reports, promote fixtures, mutate regression gates,
enforce release gates, mutate product promises or public claims, accept
outcomes, pay workers, or settle funds.

Implemented, 2026-06-17: #5266 added the `evaluationRegressionEvidence` Run
projection schema, the refs-only `projectForgeEvaluationRegressionEvidence`
view model, and the Run-detail Evaluation and regression lane. The lane
surfaces eval snapshot, suite, fixture, fixture provenance/redaction, adapter,
provider, model, runtime version, tool-policy, budget-policy, result verdict,
first-divergence, artifact, cost/latency summary, safety verdict, public/
private report, regression gate, threshold, fixture promotion, review,
freshness, status, and blocker refs while keeping all eval suite loading,
eval/regression execution, provider/model calls, report generation, fixture
promotion, regression-gate mutation, release-gate enforcement,
product-promise mutation, public-claim mutation, accepted-outcome, payout, and
settlement authorities false. Unsafe private task data, raw transcripts,
provider payloads, fixture bodies, customer/repo data, local paths, secrets,
artifact contents, tokens, and private report material are omitted before
projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/evaluation-regression-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 104 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 52 files, 488 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I34 - Security Review Evidence Lane

System: system 53 Security Review System.

Issue: #5267.

Implemented, 2026-06-17: #5267 added the `securityReviewEvidence` Run
projection schema, the refs-only `projectForgeSecurityReviewEvidence` view
model, and the Run-detail Security review lane. The lane surfaces security
snapshot, domain, threat-model, risk, owner-policy, approval-gate,
denial-receipt, exception, expiry, redaction-scan, regression-fixture,
provider-credential-policy, release-integrity, public-projection-scan,
diagnostic-bundle, freshness, status, blocker, and unsafe-material omission
refs while keeping all security gate execution, approval granting, denial and
exception mutation, redaction scan execution, diagnostic bundle generation,
export, or read, capability mutation, credential read, release verification,
public projection mutation, product-promise mutation, accepted-outcome, payout,
and settlement authorities false. Unsafe raw secrets, credential values,
provider payloads, private repo data, diagnostic contents, local paths, shell
logs, artifact contents, tokens, customer records, and private security
material are omitted before projection/rendering and converted into an explicit
blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/security-review-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 108 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 53 files, 502 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I35 - Data Retention And Deletion Evidence Lane

System: system 54 Data Retention And Deletion System.

Issue: #5269.

Implemented, 2026-06-17: #5269 added the `dataRetentionDeletionEvidence` Run
projection schema, the refs-only `projectForgeDataRetentionDeletionEvidence`
view model, and the Run-detail Data retention and deletion lane. The lane
surfaces data class, retention policy, deletion request, deletion receipt,
tombstone, export manifest, retention sweep, projection freshness, projection
invalidation, legal hold, legal/payment caveat, status, freshness, blocker, and
unsafe-material omission refs while keeping all delete execution, cache purge,
retention sweep execution, tombstone creation, projection invalidation
execution, retention policy mutation, export generation/read, private data
read, credential read/revocation, public projection mutation, receipt deletion,
accepted-outcome, payout, and settlement authorities false. Unsafe raw deleted
payloads, memory contents, artifact contents, credential values, private event
payloads, telemetry payloads, export contents, local paths, shell logs, cache
contents, tokens, and private retention material are omitted before
projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/data-retention-deletion-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 110 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 54 files, 516 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I36 - Onboarding Capability Evidence Lane

System: system 55 Onboarding System.

Issue: #5270.

Implemented, 2026-06-17: #5270 added the `onboardingCapabilityEvidence` Run
projection schema, the refs-only `projectForgeOnboardingCapabilityEvidence`
view model, and the Run-detail Onboarding capability lane. The lane surfaces
user/device, workspace, repository profile, selected mode, capability probe,
provider readiness, credential policy, permission decision, data-scope,
project instruction, invariant, first-run smoke, integration, completion
receipt, skip receipt, status, freshness, blocker, and unsafe-material omission
refs while keeping all secret collection, credential storage/write, provider
connection, integration enablement, repository scan/write, onboarding step
mutation, data-scope mutation, permission grant, first-run smoke execution,
settings mutation, team invitation acceptance, capability enablement, paid
workflow activation, accepted-outcome, payout, and settlement authorities
false. Unsafe raw secrets, provider credential values, private repository data,
workspace paths, project instruction bodies, smoke logs, user/device
identifiers, integration payloads, tokens, and private onboarding material are
omitted before projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/onboarding-capability-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 112 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 55 files, 530 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I37 - Output Style And Persona Policy Evidence Lane

System: system 56 Output Style And Persona System.

Issue: #5271.

Implemented, 2026-06-17: #5271 added the `outputStylePersonaEvidence` Run
projection schema, the refs-only `projectForgeOutputStylePersonaEvidence` view
model, and the Run-detail Output style and persona lane. The lane surfaces
style policy, verbosity, formatting, persona constraint, domain mode, audience,
accessibility, safety policy, disallowed-claim, claim receipt, citation,
evidence requirement, final-answer expectation, conflict resolution, override,
tool-authority boundary, style audit, status, freshness, blocker, and
unsafe-material omission refs while keeping all prompt mutation, instruction
mutation, style preference write, persona install, managed policy mutation,
formatter execution, output rewrite, hidden-chain access, private data read,
tool-authority change, safety/privacy/approval bypass, product-claim mutation,
accepted-outcome, payout, and settlement authorities false. Unsafe hidden
prompt text, chain state, private user preferences, private project
instructions, unsafe persona text, raw output bodies, secret-bearing override
payloads, unsupported capability claims, tokens, and private style material are
omitted before projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/output-style-persona-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 114 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 56 files, 544 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I38 - Prompt Suggestions And Autocomplete Evidence Lane

System: system 57 Prompt Suggestions And Autocomplete.

Issue: #5276.

Implemented, 2026-06-17: #5276 added the `promptSuggestionsEvidence` Run
projection schema, the refs-only `projectForgePromptSuggestionsEvidence` view
model, and the Run-detail Prompt suggestions lane. The lane surfaces suggestion
refs, suggestion kind, display refs, insert refs, action refs, confidence refs,
provenance refs, ranking refs, semantic selector refs, scope refs, permission
refs, privacy classification/refs, expiration/freshness refs,
action-separation refs, destructive/external action refs, disablement refs,
audit refs, validation refs, status, blocker, and unsafe-material omission refs
while keeping all autocomplete streaming, prompt insertion/submission, command
execution, action execution, tool invocation, permission grant, external action
trigger, semantic routing decision, settings mutation, suggestion indexing,
ranking execution, private file/artifact read, accepted-outcome, payout, and
settlement authorities false. Unsafe raw prompt text, inserted text bodies,
private file paths, private artifact contents, unvalidated model outputs,
destructive command text, repository/private scope data, token-bearing
suggestion payloads, and private suggestion material are omitted before
projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/prompt-suggestions-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 115 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 57 files, 557 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I39 - Tips And Education Evidence Lane

System: system 58 Tips And Education System.

Issue: #5277.

Implemented, 2026-06-17: #5277 added the `tipsEducationEvidence` Run
projection schema, the refs-only `projectForgeTipsEducationEvidence` view
model, and the Run-detail Tips and education lane. The lane surfaces tip,
topic, trigger, audience, scope, capability, required live-state, dismissal
receipt, required-warning, docs/help, version, expiration/freshness,
payment/provider/payout/settlement caveat, non-interactive mode/docs,
unsupported-claim, status, blocker, and unsafe-material omission refs while
keeping all tip rendering, dismissal mutation, help search, docs read/export,
capability enablement, approval prompt dismissal, policy caveat dismissal,
payment/provider/payout/settlement activation, product-claim mutation,
accepted-outcome, payout, and settlement authorities false. Unsafe raw tip
copy, private refs, raw run data, secret-bearing help payloads, unsupported
capability copy, private docs content, payment/provider payloads, tokens, and
private education material are omitted before projection/rendering and
converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/tips-education-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 118 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 58 files, 571 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I40 - Theme And Visual Design Evidence Lane

System: system 59 Theme And Visual Design System.

Issue: #5278.

Implemented, 2026-06-17: #5278 added the `themeVisualEvidence` Run projection
schema, the refs-only `projectForgeThemeVisualEvidence` view model, and the
Run-detail Theme and visual design lane. The lane surfaces theme, token,
density, typography, status visual, status label/icon, runtime receipt,
contrast, high-contrast, monochrome, reduced-motion, focus-ring,
diff/progress/attention color, managed policy, cross-surface, visual snapshot,
warning-preservation, freshness/status, blocker, and unsafe-material omission
refs while keeping all theme install, preference write, managed policy
mutation, renderer mutation, CSS injection, remote/plugin theme execution,
visual snapshot generation/read, product-claim mutation, runtime status
mutation, accepted-outcome, payout, and settlement authorities false. Unsafe
executable theme material, remote/plugin theme code, raw CSS/theme files,
private branding payloads, private visual snapshots, local paths, unsupported
green/success claims, credentials, and private visual material are omitted
before projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/theme-visual-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 119 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 59 files, 584 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I41 - Accessibility And Non-Interactive Evidence Lane

System: system 60 Accessibility And Non-Interactive Mode.

Issue: #5279.

Implemented, 2026-06-17: #5279 added the
`accessibilityNonInteractiveEvidence` Run projection schema, the refs-only
`projectForgeAccessibilityNonInteractiveEvidence` view model, and the
Run-detail Accessibility and non-interactive mode lane. The lane surfaces
interaction mode, terminal capability, structured output, schema, exit-code,
status-label, no-color, high-contrast, reduced-motion, screen-reader status,
keyboard navigation, prompt availability, approval resolver, typed prompt
blocker, notification, remote bridge availability, CI policy,
spend/push/deploy/provider-mutation caveat, freshness/status, blocker, and
unsafe-material omission refs while keeping all prompt-answer, approval grant,
approval-policy mutation, live-spend, push, deploy, provider-account mutation,
remote-bridge start, headless-command execution, structured-output emission,
exit-code mutation, terminal-capability mutation, preference write, theme
install, accepted-outcome, payout, and settlement authorities false. Unsafe
raw structured output, private output, local paths, prompt text, provider
payloads, terminal captures, credentials, and private accessibility material
are omitted before projection/rendering and converted into an explicit
blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/accessibility-non-interactive-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 121 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 61 files, 604 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I42 - Localization Boundary Evidence Lane

System: system 61 Internationalization And Localization Boundary.

Issue: #5280.

Implemented, 2026-06-17: #5280 added the `localizationBoundaryEvidence` Run
projection schema, the refs-only `projectForgeLocalizationBoundaryEvidence`
view model, and the Run-detail Localization boundary lane. The lane surfaces
locale preference, locale, message catalog, catalog validation, formatter,
visible fallback, missing-translation, stable-id boundary, permission
action/policy/id stability, payment language review, public receipt stability,
JSON/schema stability, command/tool id stability, freshness/status, blocker,
and unsafe-material omission refs while keeping all locale runtime mutation,
locale preference write, catalog execution, runtime identifier translation,
schema/JSON mutation, permission prompt mutation, payment language mutation,
public receipt mutation, command/tool id mutation, accepted-outcome, payout,
and settlement authorities false. Unsafe raw localized copy, private catalogs,
prompt text, customer/private language, local paths, provider/payment
payloads, credentials, and translated machine identifiers are omitted before
projection/rendering and converted into an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/localization-boundary-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 125 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 62 files, 619 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## I43 - Enterprise Managed Policy Evidence Lane

System: system 62 Enterprise And Managed Policy System.

Issue: #5281.

Implemented, 2026-06-17: #5281 added the
`enterpriseManagedPolicyEvidence` Run projection schema, the refs-only
`projectForgeEnterpriseManagedPolicyEvidence` view model, and the Run-detail
Enterprise managed policy lane. The lane surfaces effective policy snapshot,
organization/team/repository/user/device/project/session/provider/budget/
retention/telemetry/update/plugin/MCP/hook/remote-bridge policy, rule-kind,
enforcement mode, decision, typed denial, user-safe reason, ask/restrict/
allow, conflict, conflict-resolution, priority, admin/owner, version,
effective/expiration, emergency override receipt, audit/change, public-safe
summary, runtime capability boundary, caveat, freshness/status, blocker, and
unsafe-material omission refs while keeping all policy load/write/mutation/
install/export/enforcement, capability grant, runtime-authority broadening,
budget/provider/retention/telemetry/update-channel/integration-gate mutation,
emergency override application, public projection mutation, accepted-outcome,
payout, and settlement authorities false. Unsafe private org details, raw
policy internals, credentials, raw prompts, provider payloads,
wallet/payment material, private repo/customer data, local paths, and silent
broadening claims are omitted before projection/rendering and converted into
an explicit blocker.

Verification, 2026-06-17:

- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work/enterprise-managed-policy-evidence.test.ts
  src/page/loggedIn/page/autopilot-work.test.ts` - 2 files, 126 tests passed.
- `bun run --cwd apps/openagents.com/apps/web test --
  src/page/loggedIn/autopilot-work
  src/page/loggedIn/page/autopilot-work.test.ts` - 63 files, 633 tests passed.
- `git diff --check` passed.
- Full `bun run --cwd apps/openagents.com/apps/web typecheck` remains blocked
  by unrelated baseline scene and SpacetimeDB binding type errors, including
  missing `spacetimedb` module declarations, missing
  `WasdMouseLookControllerOptions`/`WasdMouseLookDebugSnapshot` exports from
  `@openagentsinc/three-effect`, `src/scene/tassadarRunElement.ts`
  `cameraMode`/implicit `locked`, `src/scene/tassadarRunSnapshot*`
  `motionPolicy`/`sceneChrome`/`stageNodeGlyph`, and
  `src/scene/tassadarSpacetimeWorld.ts` generated binding API mismatches.

## Parent Completion Status

Completed, 2026-06-17: #5107's current Forge Autopilot Coder
terminal-agent-systems incorporation wave is complete through #5281. The
roadmap now has closed child slices for the operator-facing projection,
readiness, review, session, context, retrieval, extensibility, operational
proof, security, retention, onboarding, accessibility, localization, theme,
and managed-policy surfaces covered by the terminal-agent systems map. The
implemented lanes are intentionally refs-only and preserve the runtime,
workspace, provider, approval, public-projection, accepted-outcome, payout, and
settlement authority boundaries. No open GitHub child issue referencing #5107
remained at the final audit.

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

## Initial Issue Filed

Title:

`[5107/G1] Forge cockpit diff-review lane over Pack C change captures`

Rationale:

It is the smallest, most grounded continuation of #5107: the evidence exists in
Pack C and Pylon, the cockpit exists in `/autopilot`, and the work is scoped to
rendering/guarding review facts rather than changing settlement, payout, or
runtime authority.

Status, 2026-06-16: this initial slice was opened as #5123 and the roadmap was
then split and completed through #5144.
