---
spec_format_version: "0.1"
title: "OpenAgents Desktop Codex Workroom MVP"
artifact_type: "prd"
spec_revision: 7
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T23:00:00Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-success-metric-context"
    label: "Success Metric Context"
    after: "success_metrics"
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "custom-success-metric-context"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
  - id: "custom-decision-trace"
    label: "Decision Trace"
    after: "custom-promise-links"
tool_metadata:
  openagents_epic: "8756"
  openagents_lane: "MVP-01 #8756; sole active Sol product lane"
  openagents_assurance_level: "signed-local-codex-workroom"
  openagents_evidence: "docs/mvp/2026-07-13-openagents-codex-workroom-mvp-audit.md"
  openagents_productspec_workflow: "native ProductSpec workbench plus built-in productspec-work skill"
  openagents_id_map: "docs/mvp/openagents-codex-workroom-mvp.id-map.json"
---

## Problem

Codex is a capable local agent engine, but its execution power does not by
itself provide the OpenAgents product: one signed, durable place to find work,
understand typed turns and child agents, resolve blockers, inspect repository
effects, and return after restart without guessing what is authoritative. It
also lacks an easy native path from product intent to systematic agent work:
users should be able to define a ProductSpec, approve a plan derived from its
acceptance criteria, and see agents work those criteria through evidence.

The broader OpenAgents program combines Desktop, mobile, Sync, Fleet, managed
workrooms, portability, voice, and multiple runtimes. Requiring that entire
program before naming the first useful product makes the initial customer
promise difficult to explain and easy to overclaim. A chat-only shell is too
small. The whole platform is too large.

Internal execution has also been split across the Codex app and CLI, Claude
Code, Codex VR Fleet, VR Pylons, and increasingly OpenAgents Desktop. The
productive pattern has been one coordinator, clean worktrees, explicit claims,
and an asynchronous agent working a sequential issue list. The failure pattern
has been dispatching to available accounts before distinct work was admitted:
agents then overlap on issues or hot contracts, duplicate work and pull
requests, and spend the apparent concurrency gain on cancellation and
reconciliation. The MVP must therefore be dogfooded with the same identity,
lease, recovery, and evidence rigor that the product will enforce.

## Hypothesis

If OpenAgents ships a signed, local-first Desktop workroom that uses Codex's
app-server as its only model/tool engine and surrounds it with metadata-first
session navigation, a typed causal timeline, durable controls and blockers,
complete child-agent topology, adjacent file/Git review, and honest restart
recovery—and makes ProductSpec the native unit for guided authoring, accepted
decomposition, agent allocation, and evidence-backed completion—then developers
will complete and resume consequential Codex tasks inside OpenAgents without
needing an OpenAgents account or falling back to another Codex interface.

## Scope

```productspec-scope
in:
  - signed and notarized OpenAgents Desktop artifact for the first supported macOS target
  - one canonical Effect Native application tree for the visible workroom, typed state, intents, lifecycle, and renderer boundary
  - local-first use without an OpenAgents account
  - one compatible host-owned Codex app-server using the user's ordinary logged-in Codex session
  - explicit repository grant and stable OpenAgents coding-session and WorkContext refs
  - create or open a ProductSpec v0.1 artifact through guided conversation or bounded fields with section-addressed validation
  - immutable ProductSpec digest and spec revision, explicit edit diff, and user confirmation for intent revisions
  - unique author-visible acceptance-criterion IDs for executable specs and explicit cross-revision criterion reconciliation
  - accepted execution plan whose durable work packets cite the exact spec revision and one or more acceptance-criterion refs
  - product-owned built-in productspec-work skill for refinement, decomposition, bounded agent allocation, and evidence reporting
  - criterion and work-packet board with typed planned, active, blocked, evidence-present, verified, failed, superseded, and cancelled states
  - metadata-first top-level session catalog with paging and new, resume, fork, archive, and delete
  - typed text, reasoning-summary, plan, tool, patch, usage, error, interruption, and terminal timeline items
  - send, stop, steer-current-turn, queue-next-turn, question, approval, and plan-review controls
  - complete parent and child agent graph with causal inline cards and independent child transcripts
  - grant-bounded file tree, Git status, and exact diff review beside the conversation
  - durable intent admission, exact-retry reconciliation, stream-gap repair, renderer reload, and app restart recovery
  - explicit Open in Codex escape hatch that preserves the admitted work identity and labels direct thread continuation versus repository-state handoff
  - explicit missing, incompatible, signed-out, quota, rate-limit, policy, revoked-grant, and unavailable states
  - update, rollback, reinstall, cleanup, and public-safe diagnostics for the compatible app and Codex set
out:
  - Claude, Grok, or general provider-neutral parity
  - required OpenAgents account, hosted Khala Sync, mobile, web, VS Code, or PWA clients
  - Fleet, multi-account dispatch, markets, payments, settlement, or public proof surfaces
  - managed Agent Computers, remote workrooms, owner-managed targets, provider adapters, host movement, or failback
  - full editor, interactive PTY, preview, destructive Git, commit, push, pull request, or merge as MVP requirements
  - arbitrary MCP installation, third-party plugin marketplace, or model-authored Code Mode
  - a user-installed skill or plugin dependency for the core ProductSpec workflow
  - autonomous Session Goals, schedules, persistent voice, computer use, browser automation, or ambient memory
cut:
  - a second OpenAgents-owned model or tool loop beside Codex
  - direct raw Codex app-server, rollout, provider event, or terminal-text parsing in the renderer
  - renderer credentials, loopback secrets, generic IPC, raw MessagePort, Node, filesystem, process, or absolute-root authority
  - named Pylon account linking, isolated-account rotation, or Fleet custody on the MVP workroom path
  - model prose, connection health, optimistic UI, or issue closure as completion authority
  - skill-authored spec edits, plans, criterion status, or completion becoming authoritative without host validation and user admission
  - silent retargeting of admitted work when the ProductSpec digest or revision changes
  - simultaneous OpenAgents and external Codex mutation of the same admitted work packet
  - public Codex, Claude, mobile, Fleet, cloud, portability, or voice claims not proven by their own current receipts
```

## User Experience

A developer installs OpenAgents Desktop and reaches a useful local workroom
without creating an OpenAgents account. The app uses the Codex session already
logged in on the machine or shows one precise sign-in prerequisite. There is no
second Pylon account-linking flow. The developer grants a repository, then
creates a ProductSpec conversationally or opens an existing one. The workroom
shows validation at the relevant section, an exact revision/digest, and a
reviewable plan derived from the acceptance criteria. After the developer
accepts that plan, its work packets become the units Codex agents execute.

Questions and approvals remain visible until durably resolved. Child agents
appear at their causal parent event and in a complete navigable graph. Selecting
one opens its own transcript. File changes and the exact Git diff stay beside
the conversation and criterion evidence. The ProductSpec board always shows
what is planned, active, blocked, evidenced, verified, or still open. After
renderer reload, stream loss, or app restart, the same session returns with the
same pinned spec revision and one honest pending, recovering, interrupted,
failed, or completed disposition and no duplicate turn.

If an OpenAgents defect prevents useful work, the developer can choose **Open
in Codex** after OpenAgents has durably stopped or reconciled its attempt. The
workroom says whether the official Codex surface continued the exact recorded
thread or received a repository-state handoff, and it retains the fallback
reason and original work-packet identity. Returning to OpenAgents never invents
transcript continuity or counts external completion as OpenAgents-native proof.

## Solution

Codex owns Thread → Turn → Item execution, model/provider calls, tools,
approvals, sandboxing, local rollout history, and provider-native child threads.
OpenAgents owns the product workroom and stable product refs.

Effect Native is OpenAgents' Effect-based application framework: one set of
typed components, state, intents, resources, and lifecycle rules can use
swappable Electron/DOM, React Native/native, or canvas renderers without
forking the application model. The MVP uses it so UI behavior and host
authority stay schema-decoded, interruption-aware, testable, and reusable
rather than creating a parallel React state or command universe for Desktop.

The signed Effect Native renderer is sandboxed and tokenless. It consumes only
fixed schema-decoded projections and emits registered typed intents. Electron
is the host/renderer, not a second application architecture. A host-owned
Runtime Gateway supervises the compatible Codex app-server, maps Codex identity
into stable OpenAgents session and agent refs, durably admits mutations before
dispatch, reconciles current projection plus durable history before live
resubscription, and composes grant-bounded workspace/Git reads.

The host also owns a ProductSpec service backed by
`@openagentsinc/product-spec`. It parses and validates specs, assigns an
immutable digest, and persists a `ProductSpecRun` bound to work context,
granted spec path, revision, digest, and plan ref. Workroom-executable specs
require unique author-visible criterion IDs. Work packets cite
`path@revision+digest#criterion-id`. The accepted plan projects into existing
typed work-unit, dependency, intent, agent, and evidence contracts rather than
creating a second scheduler or claim universe.

A product-owned, read-only, hash-pinned `productspec-work` skill supplies the
reusable Codex method for elicitation, spec-edit proposals, decomposition,
agent allocation, and evidence reporting. Runtime Gateway registers the
app-managed skill root into the current logged-in Codex app-server through the
native skill surface and selects its exact typed catalog identity, never prose
or keyword routing. The skill remains under the signed application resources.
it is not copied into the default Codex home and never uses the current
user-plugin `/skill` path.
The skill can propose but cannot approve a spec edit, grant work, change a
pinned revision, or verify its own result. Systematic execution is bounded to
the user-accepted foreground plan and stops on blockers.

Runtime Gateway is an adapter and lifecycle owner, not a second conversation
engine, public server, or parallel session database. ProductSpec declares
intent but does not replace roadmap sequence, behavior contracts, Eval Suites,
receipts, owner gates, or the promise registry. Raw Codex history stays
owner-local by default.

## Acceptance Criteria

```productspec-acceptance-criteria
- id: AC-1
  criterion: A signed/notarized release candidate installs and launches without a source checkout, resolves only its pinned compatible Codex runtime path, and reports missing or incompatible runtime state explicitly.
- id: AC-2
  criterion: Local-first mode can reach the first useful Codex workroom without an OpenAgents account or hosted service. It uses only the user's ordinary logged-in Codex session, clears any inherited `CODEX_HOME`, and exposes no named-Pylon account linking, isolated device-auth, or account rotation in the MVP workroom.
- id: AC-3
  criterion: Granting one repository creates a stable WorkContext and product session ref that do not derive from a path, process, port, machine, or provider thread ID.
- id: AC-4
  criterion: From one guided conversation, the workroom creates a validator-clean ProductSpec v0.1 draft or opens an existing spec. Validation failures identify the exact section. An unlabeled legacy spec remains viewable, but executable criteria require unique author-visible IDs and no work starts while any ID is missing or duplicated.
- id: AC-5
  criterion: The workroom shows the exact ProductSpec digest and `spec_revision`, previews every intent-changing edit as a diff, requires user confirmation plus a revision bump, and retains the prior revision for already admitted work. Retained criterion IDs may map across revisions; changed or removed IDs require explicit reconciliation.
- id: AC-6
  criterion: A user-accepted execution plan contains at least two durable work packets. Every packet cites the exact spec revision and one or more criterion refs; at least one packet can be allocated to a child agent and opened from both the criterion board and causal timeline. Before execution, every criterion is mapped or explicitly deferred, every mutating packet has at most one active execution lease, and duplicate or cyclic work packets refuse.
- id: AC-7
  criterion: The product-owned `productspec-work` skill ships hash-pinned in the signed compatibility set, is registered from the app-owned resource root into the current Codex session through the native app-server surface, and can refine, decompose, allocate, and report through typed host tools. Removing, corrupting, or version-mismatching it produces an explicit incompatible workflow state; it never falls back to an ambient/user-installed skill and never copies itself into the default Codex home.
- id: AC-8
  criterion: Skill or agent prose cannot approve a spec edit, admit a work packet, change the pinned revision, or mark a criterion verified. Evidence-present and verified remain distinct. Verification requires linked test/verifier output, behavior/Eval oracle, artifact or diff review, or receipt; owner acceptance or waiver remains a separate typed disposition.
- id: AC-9
  criterion: A spec revision/digest change while work is active produces a typed mismatch. New dispatch stops until the user reconciles, supersedes, or cancels the old plan; active work is never silently retargeted and no evidence crosses revisions without an explicit mapping.
- id: AC-10
  criterion: The session rail paints bounded metadata before transcript hydration, lists only top-level sessions, pages without an age ceiling, and preserves stable titles, status, attention, ordering, and selected session through restart.
- id: AC-11
  criterion: One real Codex task is durably admitted before dispatch and renders typed text plus at least one non-text plan, tool, patch/file-change, usage, blocker, or lifecycle item and exactly one terminal disposition.
- id: AC-12
  criterion: Exact retry reconciles to the admitted intent; conflicting reuse refuses. Send, stop, steer, queue, question, approval, and plan-review actions use the same registered command identities across direct, keyboard, palette, and native-menu entry points.
- id: AC-13
  criterion: The complete child graph retains exact parentage and lifecycle. A causal inline card opens one child's independent transcript; reload/reconnect never flattens, duplicates, re-roots, or leaks a child into the top-level catalog.
- id: AC-14
  criterion: The granted repository exposes a bounded file tree, Git status, and exact diff correlated to timeline item refs. Revocation and post-image conflict fail visibly without exposing general filesystem or Git mutation authority.
- id: AC-15
  criterion: Renderer reload does not stop or duplicate host-owned work. App-process restart restores the exact persisted prefix and either continues the recorded Codex thread at most once or records an explicit interrupted terminal outcome; it never silently reruns the task. Open in Codex is offered only after the OpenAgents attempt is quiescent or authoritatively reconciled, preserves the admitted packet identity, and labels exact-thread continuation separately from repository-state handoff and transcript-gap recovery.
- id: AC-16
  criterion: Lost acknowledgement, duplicate/out-of-order frame, cursor gap, stale generation, revoked grant, quota exhaustion, rate limit, auth revocation, and policy denial converge to distinct typed states. Durable repair precedes live resubscription.
- id: AC-17
  criterion: Diagnostics and non-content renderer control envelopes contain no credential, account identity, loopback URL/secret, raw provider event, prompt/transcript body, repository content, absolute root, generic IPC, process handle, or general filesystem handle. Content views receive only bounded transcript and repository projections admitted for the selected work context; they never receive raw provider payloads, credentials, absolute roots, or general process/filesystem authority.
- id: AC-18
  criterion: The exact release candidate passes install, launch, one real Codex workroom task, renderer reload, app restart, interrupted update, rollback/downgrade refusal, diagnostics export, uninstall/reinstall, and cleanup receipts.
```

## Success Metrics

```productspec-success-metrics
- id: SM-1
  metric: opted_in_first_launches_creating_or_opening_a_valid_spec_accepting_a_plan_and_starting_its_first_criterion_within_15_minutes
  target: ">= 60%"
  window: first 30 days of invited MVP dogfood
- id: SM-2
  metric: qualifying_codex_tasks_reaching_one_reviewed_diff_and_terminal_outcome_without_opening_another_codex_interface
  target: ">= 70%"
  window: first 30 days of invited MVP dogfood
- id: SM-3
  metric: accepted_productspec_plans_whose_work_packets_all_retain_exact_revision_criterion_and_terminal_evidence_links
  target: "100%"
  window: release acceptance and first 30 days of invited MVP dogfood
- id: SM-4
  metric: activated_developers_starting_a_second_durably_admitted_codex_task_within_7_days
  target: ">= 40%"
  window: rolling 30-day cohorts after MVP distribution
- id: SM-5
  metric: confirmed_incidents_where_the_workroom_showed_completed_without_the_matching_terminal_outcome_and_review_post_image
  target: "0"
  window: release acceptance and first 30 days of invited MVP dogfood
- id: SM-6
  metric: admitted_mvp_work_packets_observed_with_more_than_one_active_mutation_lease_or_duplicate_executor
  target: "0"
  window: development dogfood through release acceptance
- id: SM-7
  metric: raw_codex_fallbacks_recording_reason_last_durable_openagents_state_same_work_packet_and_return_disposition
  target: "100%"
  window: development dogfood through first 30 days of invited MVP dogfood
```

## Success Metric Context

Upstream ProductSpec success metrics do not carry the OpenAgents `segment`
and `source` provenance fields. This keyed section preserves each metric's
exact revision-6 segment and consent/source provenance without extending the
upstream Success Metric schema. The legacy snake_case metric IDs remain
author-visible here and in the ID map artifact.

- **SM-1** (`codex_workroom_activation`):
  - segment: developers with a supported macOS host and an eligible Codex account
  - source: consented_public_safe_local_activation_receipts
- **SM-2** (`codex_workroom_completion_without_fallback`):
  - segment: opted-in repository tasks that pass the supported-runtime preflight
  - source: consented_public_safe_task_and_review_receipts
- **SM-3** (`productspec_guided_execution_integrity`):
  - segment: consequential MVP tasks started from a ProductSpec plan
  - source: consented_public_safe_spec_execution_receipts
- **SM-4** (`codex_workroom_seven_day_return`):
  - segment: developers with one accepted MVP task
  - source: consented_public_safe_local_session_counters
- **SM-5** (`codex_workroom_false_completion`):
  - segment: all MVP Codex tasks with consented diagnostic receipts
  - source: acceptance_exception_register_and_public_safe_support_receipts
- **SM-6** (`codex_workroom_dogfood_duplicate_execution`):
  - segment: ProductSpec-linked MVP implementation packets
  - source: private_ref_only_dogfood_claim_and_lease_reconciliation
- **SM-7** (`codex_workroom_dogfood_fallback_accounting`):
  - segment: MVP packets leaving OpenAgents for an official Codex surface
  - source: private_ref_only_dogfood_fallback_ledger

## Risks

- Codex app-server evolves quickly. Compatibility must fail explicitly and
  remain tied to a tested app/runtime set instead of silently parsing a changed
  provider protocol.
- The Runtime Gateway can accidentally become a second engine or database.
  Any alternate model/tool loop or independent session truth blocks launch.
- The built-in skill can accidentally become hidden authority. All durable
  spec, plan, work-packet, criterion, and evidence transitions must remain
  host-validated typed operations that the workroom can inspect.
- ProductSpec ceremony can slow small work. The guided draft must be quick, and
  the workroom must not require a spec for mechanical tasks outside the repo's
  consequential-work threshold.
- A polished timeline can hide data loss. Completeness, explicit gaps, durable
  admission, and restart fault receipts remain acceptance requirements.
- Reusing the ordinary Codex session can inherit stale or missing auth. The
  host clears inherited `CODEX_HOME`, probes the exact current session, and
  reports a precise sign-in or quota prerequisite without rotating elsewhere.
- Read-only review may be too narrow for repeated daily use. That is a
  falsifiable post-launch result, not permission to add editor/PTY/Git breadth
  before the first complete workroom is accepted.
- Opt-in metrics can bias toward expert dogfood users. Segment and consent
  provenance must remain visible. No prompt, path, account, or machine identity
  is collected to improve the number.
- A raw Codex escape can hide an OpenAgents defect if external completion is
  reported as workroom success. Every fallback must remain visible, and only an
  exact OpenAgents rerun may convert that packet into OpenAgents-native proof.
- Fleet capacity can be mistaken for available work. Concurrency is bounded by
  distinct admitted packets, non-overlapping paths and hot contracts, and
  review capacity—not connected accounts or idle workers.
- Closed broader issues can tempt a premature claim. Only the exact current
  artifact and MVP journey prove this spec. CUT-27 and portable/mobile/Fleet
  claims retain their own gates.

## Open Questions

- Is a bundled pinned Codex binary the only supported MVP posture, or may a
  separately installed compatible app-server satisfy the same signed
  component ledger?
- Is read-only file/diff review enough for the first 30-day return target, or
  does one narrow conflict-safe edit action become necessary before launch?
- Which stable Codex app-server capability/version window is supported, and
  what is the user-visible upgrade path when it closes?
- What authoring affordance best preserves stable criterion IDs through a
  revision while keeping the underlying ProductSpec plain Markdown?
- Which local-only, opt-in counters are sufficient to evaluate activation and
  return without collecting stable account, machine, repository, prompt, or
  transcript identity?
- Which pinned Codex app/CLI versions can directly continue the ordinary logged-in
  thread, and what bounded handoff is required when direct continuation is not
  supported?
- What evidence would justify raising the initial dogfood concurrency ceiling
  above two disjoint mutating packets?

## Rollout

### Evidence from current practice

OpenAgents has already benefited from raw Codex and Claude Code sessions,
Codex VR Fleet, VR Pylons, and early work through the actual Desktop app.
Long-running asynchronous agents can reliably burn a sequential issue list
when one coordinator owns integration, each agent has a clean current-main
worktree and exact claim, and releases are reconciled before the next item.
This remains the default background operating shape.

The reviewed work logs also show that account count is not a safe concurrency
target. Broad issues attracted overlapping claims, ostensibly file-disjoint
changes collided on shared contracts and acceptance, and generated or orphaned
pull requests duplicated evidence already on main. Closing or cancelling work
before every active claim was reconciled made the ledger briefly wrong. The
development process must now dogfood the intended product law: one accepted
ProductSpec revision/digest, one stable work-packet identity, at most one active
mutation lease for that packet, and evidence—not a running process, issue, or
pull request—as completion authority.

### Issue and concurrency cadence

- [MVP-01 #8756](https://github.com/OpenAgentsInc/openagents/issues/8756)
  remains the parent, claim ledger, and evidence index. The accepted
  ProductSpec plan holds future work. GitHub does not need one speculative
  issue per criterion or subagent.
- A child issue is opened just in time only when its dependency receipts are
  green, an executor can claim it promptly, no active path or hot-contract
  claim overlaps, and its close rule is independently satisfiable. Keep the
  current child plus at most one next-ready child visible. Smaller same-owner
  steps stay work packets or claim updates.
- One root coordinator owns the ProductSpec, schemas, migrations, lockfile,
  generated catalogs, roadmap, release, issue closure, and final integration.
  Pull requests are proposals and require current-main reconciliation, exact
  issue/spec/criterion links, checks, and review before merge.
- Owner-absent or AFK work defaults to one mutating packet. Read-only audit or
  verification may run in parallel. With an active owner/coordinator, at most
  two file- and contract-disjoint packets may mutate concurrently, with one
  additional read-only reviewer. One authorized Fleet run may use those two
  named workers under one claim registry and one Pylon publisher. Connected
  account capacity never creates work by itself.
- Owner presence is required for spec or plan admission, intent revision,
  credentials or physical-device acceptance, production promotion,
  irreversible or public writeback, waivers, and launch language. AFK agents
  stop mutation and release their implementation claim on an owner-only block
  unless a live resource needs a named custodian.
- Every claim records execution mode, base SHA, spec revision/digest,
  criterion and packet refs, paths and hot contracts, proof rung, allowed
  external writes, fallback and stop rules. Claims are updated at durable
  boundaries and released before their issue is closed or superseded.

### OpenAgents-first execution and Codex fallback

Every eligible MVP implementation packet starts in the exact installed
OpenAgents candidate. Eligible means that candidate truthfully advertises the
capabilities the packet needs. A missing pre-MVP capability is recorded as a
bootstrap gap, not disguised as an OpenAgents attempt. The ledger records the
artifact, spec revision/digest, criterion and packet refs, OpenAgents session,
claim, base, and planned verification.

If OpenAgents becomes buggy, the operator first stops or authoritatively
reconciles the attempt to a typed blocked or interrupted state. **Open in
Codex** may then continue the same recorded thread where the pinned
compatibility set proves that path. Otherwise raw Codex resumes from the same
issue/spec/criterion/packet identity and verified repository state with an
explicit transcript gap. OpenAgents and raw Codex never mutate the packet
concurrently, and account, model, provider, target, or worktree never switches
silently.

The preferred circuit breaker is narrow: raw Codex repairs the OpenAgents
blocker, then the original packet reruns in OpenAgents. If consequential work
cannot wait, Codex may complete it as a recorded `raw_codex_only_exception`.
that preserves continuity but does not satisfy OpenAgents dogfood or acceptance,
and the OpenAgents defect becomes a just-in-time repair packet. Claude Code,
Fleet, and Pylons are not implicit fallback executors for the same packet. They
may provide an independently claimed disjoint lane or read-only review, but no
provider or fleet surface inherits a live OpenAgents mutation lease.

Commit, push, pull-request, and merge remain outside the MVP workroom cut. The
root coordinator may perform those outer integration steps, but their success
cannot be reported as a capability exercised inside OpenAgents.

### Dogfood dispositions

- `openagents_completed`: the eligible packet completed and was verified in
  the exact OpenAgents candidate.
- `openagents_blocked_raw_repair_openagents_rerun_completed`: OpenAgents exposed
  a defect, raw Codex repaired the blocker, and the original packet then passed
  in OpenAgents. This counts as eventual dogfood success while retaining the
  failure and repair evidence.
- `openagents_blocked`: the OpenAgents attempt stopped honestly without an
  external completion.
- `raw_codex_only_exception`: work continued or completed outside OpenAgents.
  It remains useful repository progress but does not count as OpenAgents proof.

## Owner Gates

- Accept the exact installed-app Codex workroom journey, its ProductSpec-native
  execution loop, and its explicit read-only-review boundary.
- Approve any public language describing OpenAgents as a Codex workroom or
  companion. This spec and code/fixture proof alone authorize no claim.
- Approve the MVP telemetry/consent copy before any post-launch success metric
  collection. Metrics remain absent rather than inferred when consent is off.
- Any decision to release this Codex-only shape before the broader CUT-27
  Codex/Claude/mobile declaration must be explicit and must not mark CUT-27 or
  its parents complete.
- Accept the initial one-AFK/two-owner-present mutating-lane dogfood ceiling.
  any increase requires reviewed collision, review-capacity, and recovery
  evidence rather than additional connected accounts alone.

## Receipts

- Exact signed/notarized artifact, component-version, install, update,
  rollback/downgrade-refusal, diagnostics, reinstall, and cleanup receipts.
- Packaged real-Codex vertical journey covering the ordinary logged-in session,
  repository grant, guided valid ProductSpec, accepted revision-pinned plan,
  two criterion-linked work packets, built-in skill use, one child transcript,
  evidence/verification distinction, exact diff, terminal outcome, renderer
  reload, and app restart.
- Deterministic ProductSpec receipts for invalid input, rejected spec edit,
  revision/digest mismatch, conflicting work-packet identity, missing evidence,
  unavailable built-in skill, and attempted false completion.
- Deterministic fault receipts for duplicate/conflicting intent, lost ACK,
  stream gap, stale generation, revoked grant, incompatible runtime, auth,
  quota/rate limit, and policy denial.
- Per-packet development-dogfood ledger covering the OpenAgents-first attempt,
  artifact and exact spec identity, issue/criterion/packet/claim refs,
  concurrency decision, duplicate-work fencing, any raw Codex fallback or
  OpenAgents rerun, exact verification, terminal disposition, and proof rung.
- Renderer-boundary and public-safety scans proving the cut authority and data
  classes remain absent.
- A bounded exception register that distinguishes code-landed,
  fixture-proven, distributed, live-proven, owner-accepted, and closed.

## Promise Links

No existing public promise becomes green from adopting this spec. Before any
launch claim, the owning promise-registry entry and behavior/Eval contracts
must link the exact accepted receipts above. The wider Codex/Claude/mobile,
Fleet, cloud, portable-session, and voice promises remain unchanged.

## Related Artifacts

```productspec-related-artifacts
- type: product_spec
  product_spec_path: "./openagents-codex-workroom-mvp.product-spec.md"
  product_spec_revision: 6
  relation: supersedes
```

## Decision Trace

- **2026-07-13 — PSEL-2 intent-identity migration (revision 7, proposed).**
  This revision converts the eighteen legacy `CW-AC-01…CW-AC-18` prose
  acceptance criteria into portable structured `AC-1…AC-18` items and the
  seven OpenAgents-shaped success metrics into structured `SM-1…SM-7` items
  per upstream ProductSpec `0.19.0` conventions and
  `docs/assurance/PRODUCTSPEC_EVIDENCE_LOOP.md` §"PSEL-2 — reconcile the MVP
  ProductSpec". Criterion text is normalized to single lines by whitespace
  collapse only, because the upstream handwritten parser does not accept
  YAML block scalars. No criterion or metric wording changed. Each metric's
  OpenAgents-only `segment`/`source` provenance fields move to the keyed
  Success Metric Context section because they are not part of the upstream
  Success Metric schema. The machine-readable old-to-new ID map artifact is
  [`openagents-codex-workroom-mvp.id-map.json`](./openagents-codex-workroom-mvp.id-map.json).
  the `packages/product-spec` test suite enforces that this revision's
  items, the ID map, and the exact revision-6 text agree.
- **Adoption state: proposed, owner-gated.** The live executable subject
  remains revision 6
  (`docs/mvp/openagents-codex-workroom-mvp.product-spec.md`, document digest
  `sha256:fba7963334eb736582003e7d903d0e57164e7fecb2c158c302af7fb23e3f6ef1`),
  which the checked-in AssuranceSpec proposal and the MVP-01 (#8756) dogfood
  deliberately bind. Owner presence is required for intent-revision
  admission (Rollout §"Issue and concurrency cadence"). PSEL-3 freezes the
  migrated document/intent identities, rebinds the AssuranceSpec, and
  creates the new accepted plan/run for the `AC-*` identity. No existing
  run, packet, or evidence is relabeled by this proposal, and no old
  history is rewritten.
