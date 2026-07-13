---
spec_format_version: "0.1"
title: "OpenAgents Desktop Codex Workroom MVP"
artifact_type: "prd"
spec_revision: 2
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T13:52:48Z"
linked_github_repo: "OpenAgentsInc/openagents"
custom_sections:
  - id: "custom-owner-gates"
    label: "Owner Gates"
    after: "success_metrics"
  - id: "custom-receipts"
    label: "Receipts"
    after: "custom-owner-gates"
  - id: "custom-promise-links"
    label: "Promise Links"
    after: "custom-receipts"
tool_metadata:
  openagents_epic: "8566"
  openagents_lane: "Desktop #8574; CUT-27 #8707 remains the broader cutover authority"
  openagents_assurance_level: "signed-local-codex-workroom"
  openagents_evidence: "docs/mvp/2026-07-13-openagents-codex-workroom-mvp-audit.md"
  openagents_productspec_workflow: "native ProductSpec workbench plus built-in productspec-work skill"
---

## Problem

Codex is a capable local agent engine, but its execution power does not by
itself provide the OpenAgents product: one signed, durable place to find work,
understand typed turns and child agents, resolve blockers, inspect repository
effects, and return after restart without guessing what is authoritative. It
also lacks an easy native path from product intent to systematic agent work:
users should be able to define a Product Spec, approve a plan derived from its
acceptance criteria, and see agents work those criteria through evidence.

The broader OpenAgents program combines Desktop, mobile, Sync, Fleet, managed
workrooms, portability, voice, and multiple runtimes. Requiring that entire
program before naming the first useful product makes the initial customer
promise difficult to explain and easy to overclaim. A chat-only shell is too
small; the whole platform is too large.

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
  - local-first use without an OpenAgents account
  - one compatible host-owned Codex app-server and one named isolated Codex account
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
  - use of the default Codex home for device login or automatic work dispatch
  - model prose, connection health, optimistic UI, or issue closure as completion authority
  - skill-authored spec edits, plans, criterion status, or completion becoming authoritative without host validation and user admission
  - silent retargeting of admitted work when the ProductSpec digest or revision changes
  - public Codex, Claude, mobile, Fleet, cloud, portability, or voice claims not proven by their own current receipts
```

## User Experience

A developer installs OpenAgents Desktop and reaches a useful local workroom
without creating an OpenAgents account. The app either shows ready named Codex
capacity or one precise prerequisite. The developer grants a repository, then
creates a Product Spec conversationally or opens an existing one. The workroom
shows validation at the relevant section, an exact revision/digest, and a
reviewable plan derived from the acceptance criteria. After the developer
accepts that plan, its work packets become the units Codex agents execute.

Questions and approvals remain visible until durably resolved. Child agents
appear at their causal parent event and in a complete navigable graph; selecting
one opens its own transcript. File changes and the exact Git diff stay beside
the conversation and criterion evidence. The ProductSpec board always shows
what is planned, active, blocked, evidenced, verified, or still open. After
renderer reload, stream loss, or app restart, the same session returns with the
same pinned spec revision and one honest pending, recovering, interrupted,
failed, or completed disposition and no duplicate turn.

## Solution

Codex owns Thread → Turn → Item execution, model/provider calls, tools,
approvals, sandboxing, local rollout history, and provider-native child threads.
OpenAgents owns the product workroom and stable product refs.

The signed Effect Native renderer is sandboxed and tokenless. It consumes only
fixed schema-decoded projections and emits registered typed intents. A
host-owned Runtime Gateway supervises the compatible Codex app-server, maps
Codex identity into stable OpenAgents session and agent refs, durably admits
mutations before dispatch, reconciles current projection plus durable history
before live resubscription, and composes grant-bounded workspace/Git reads.

The host also owns a ProductSpec service backed by
`@openagentsinc/product-spec`. It parses and validates specs, assigns an
immutable digest, and persists a `ProductSpecRun` bound to work context,
granted spec path, revision, digest, and plan ref. Workroom-executable specs
require unique author-visible criterion IDs; work packets cite
`path@revision+digest#criterion-id`. The accepted plan projects into existing
typed work-unit, dependency, intent, agent, and evidence contracts rather than
creating a second scheduler or claim universe.

A product-owned, read-only, hash-pinned `productspec-work` skill supplies the
reusable Codex method for elicitation, spec-edit proposals, decomposition,
agent allocation, and evidence reporting. Runtime Gateway registers the
app-managed skill root into the named isolated Codex environment through the
native app-server skill surface and selects its exact typed catalog identity,
never prose or keyword routing; it never uses the default Codex home or the
current user-plugin `/skill` path.
The skill can propose but cannot approve a spec edit, grant work, change a
pinned revision, or verify its own result. Systematic execution is bounded to
the user-accepted foreground plan and stops on blockers.

Runtime Gateway is an adapter and lifecycle owner, not a second conversation
engine, public server, or parallel session database. ProductSpec declares
intent but does not replace roadmap sequence, behavior contracts, Eval Suites,
receipts, owner gates, or the promise registry. Raw Codex history stays
owner-local by default.

## Acceptance Criteria

- **CW-AC-01:** A signed/notarized release candidate installs and launches without a source
  checkout, resolves only its pinned compatible Codex runtime path, and reports
  missing or incompatible runtime state explicitly.
- **CW-AC-02:** Local-first mode can reach the first useful Codex workroom without an
  OpenAgents account or hosted service. Connecting Codex uses a named isolated
  home and never mutates the default Codex home.
- **CW-AC-03:** Granting one repository creates a stable WorkContext and product session ref
  that do not derive from a path, process, port, machine, or provider thread ID.
- **CW-AC-04:** From one guided conversation, the workroom creates a validator-clean
  ProductSpec v0.1 draft or opens an existing spec. Validation failures identify
  the exact section. An unlabeled legacy spec remains viewable, but executable
  criteria require unique author-visible IDs and no work starts while any ID is
  missing or duplicated.
- **CW-AC-05:** The workroom shows the exact ProductSpec digest and `spec_revision`, previews
  every intent-changing edit as a diff, requires user confirmation plus a
  revision bump, and retains the prior revision for already admitted work.
  Retained criterion IDs may map across revisions; changed or removed IDs
  require explicit reconciliation.
- **CW-AC-06:** A user-accepted execution plan contains at least two durable work packets.
  Every packet cites the exact spec revision and one or more criterion refs;
  at least one packet can be allocated to a child agent and opened from both
  the criterion board and causal timeline. Before execution, every criterion is
  mapped or explicitly deferred and duplicate or cyclic work packets refuse.
- **CW-AC-07:** The product-owned `productspec-work` skill ships hash-pinned in the signed
  compatibility set, is registered only into the named isolated Codex skill
  root through the native app-server surface, and can refine, decompose,
  allocate, and report through typed host tools. Removing, corrupting, or
  version-mismatching it produces an explicit incompatible workflow state; it
  never falls back to an ambient/user-installed skill or the default Codex home.
- **CW-AC-08:** Skill or agent prose cannot approve a spec edit, admit a work packet, change
  the pinned revision, or mark a criterion verified. Evidence-present and
  verified remain distinct. Verification requires linked test/verifier output,
  behavior/Eval oracle, artifact or diff review, or receipt; owner acceptance
  or waiver remains a separate typed disposition.
- **CW-AC-09:** A spec revision/digest change while work is active produces a typed mismatch.
  New dispatch stops until the user reconciles, supersedes, or cancels the old
  plan; active work is never silently retargeted and no evidence crosses
  revisions without an explicit mapping.
- **CW-AC-10:** The session rail paints bounded metadata before transcript hydration, lists
  only top-level sessions, pages without an age ceiling, and preserves stable
  titles, status, attention, ordering, and selected session through restart.
- **CW-AC-11:** One real Codex task is durably admitted before dispatch and renders typed
  text plus at least one non-text plan, tool, patch/file-change, usage, blocker,
  or lifecycle item and exactly one terminal disposition.
- **CW-AC-12:** Exact retry reconciles to the admitted intent; conflicting reuse refuses.
  Send, stop, steer, queue, question, approval, and plan-review actions use the
  same registered command identities across direct, keyboard, palette, and
  native-menu entry points.
- **CW-AC-13:** The complete child graph retains exact parentage and lifecycle. A causal
  inline card opens one child's independent transcript; reload/reconnect never
  flattens, duplicates, re-roots, or leaks a child into the top-level catalog.
- **CW-AC-14:** The granted repository exposes a bounded file tree, Git status, and exact
  diff correlated to timeline item refs. Revocation and post-image conflict
  fail visibly without exposing general filesystem or Git mutation authority.
- **CW-AC-15:** Renderer reload does not stop or duplicate host-owned work. App-process
  restart restores the exact persisted prefix and either continues the
  recorded Codex thread at most once or records an explicit interrupted
  terminal outcome; it never silently reruns the task.
- **CW-AC-16:** Lost acknowledgement, duplicate/out-of-order frame, cursor gap, stale
  generation, revoked grant, quota exhaustion, rate limit, auth revocation,
  and policy denial converge to distinct typed states. Durable repair precedes
  live resubscription.
- **CW-AC-17:** The renderer contract and diagnostics contain no credential, account
  identity, loopback URL/secret, raw provider event, prompt/transcript body,
  repository content, absolute root, generic IPC, process handle, or general
  filesystem handle.
- **CW-AC-18:** The exact release candidate passes install, launch, one real Codex workroom
  task, renderer reload, app restart, interrupted update, rollback/downgrade
  refusal, diagnostics export, uninstall/reinstall, and cleanup receipts.

## Success Metrics

```productspec-success-metrics
- id: codex_workroom_activation
  metric: opted_in_first_launches_creating_or_opening_a_valid_spec_accepting_a_plan_and_starting_its_first_criterion_within_15_minutes
  target: ">= 60%"
  window: first 30 days of invited MVP dogfood
  segment: developers with a supported macOS host and an eligible Codex account
  source: consented_public_safe_local_activation_receipts
- id: codex_workroom_completion_without_fallback
  metric: qualifying_codex_tasks_reaching_one_reviewed_diff_and_terminal_outcome_without_opening_another_codex_interface
  target: ">= 70%"
  window: first 30 days of invited MVP dogfood
  segment: opted-in repository tasks that pass the supported-runtime preflight
  source: consented_public_safe_task_and_review_receipts
- id: productspec_guided_execution_integrity
  metric: accepted_productspec_plans_whose_work_packets_all_retain_exact_revision_criterion_and_terminal_evidence_links
  target: "100%"
  window: release acceptance and first 30 days of invited MVP dogfood
  segment: consequential MVP tasks started from a ProductSpec plan
  source: consented_public_safe_spec_execution_receipts
- id: codex_workroom_seven_day_return
  metric: activated_developers_starting_a_second_durably_admitted_codex_task_within_7_days
  target: ">= 40%"
  window: rolling 30-day cohorts after MVP distribution
  segment: developers with one accepted MVP task
  source: consented_public_safe_local_session_counters
- id: codex_workroom_false_completion
  metric: confirmed_incidents_where_the_workroom_showed_completed_without_the_matching_terminal_outcome_and_review_post_image
  target: "0"
  window: release acceptance and first 30 days of invited MVP dogfood
  segment: all MVP Codex tasks with consented diagnostic receipts
  source: acceptance_exception_register_and_public_safe_support_receipts
```

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
- Named isolated account custody adds first-run friction. The activation metric
  tests whether the safer flow is still usable without weakening isolation.
- Read-only review may be too narrow for repeated daily use. That is a
  falsifiable post-launch result, not permission to add editor/PTY/Git breadth
  before the first complete workroom is accepted.
- Opt-in metrics can bias toward expert dogfood users. Segment and consent
  provenance must remain visible; no prompt, path, account, or machine identity
  is collected to improve the number.
- Closed broader issues can tempt a premature claim. Only the exact current
  artifact and MVP journey prove this spec; CUT-27 and portable/mobile/Fleet
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

## Owner Gates

- Accept the exact installed-app Codex workroom journey, its ProductSpec-native
  execution loop, and its explicit read-only-review boundary.
- Approve any public language describing OpenAgents as a Codex workroom or
  companion; this spec and code/fixture proof alone authorize no claim.
- Approve the MVP telemetry/consent copy before any post-launch success metric
  collection. Metrics remain absent rather than inferred when consent is off.
- Any decision to release this Codex-only shape before the broader CUT-27
  Codex/Claude/mobile declaration must be explicit and must not mark CUT-27 or
  its parents complete.

## Receipts

- Exact signed/notarized artifact, component-version, install, update,
  rollback/downgrade-refusal, diagnostics, reinstall, and cleanup receipts.
- Packaged real-Codex vertical journey covering named isolated account,
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
- Renderer-boundary and public-safety scans proving the cut authority and data
  classes remain absent.
- A bounded exception register that distinguishes code-landed,
  fixture-proven, distributed, live-proven, owner-accepted, and closed.

## Promise Links

No existing public promise becomes green from adopting this spec. Before any
launch claim, the owning promise-registry entry and behavior/Eval contracts
must link the exact accepted receipts above. The wider Codex/Claude/mobile,
Fleet, cloud, portable-session, and voice promises remain unchanged.
