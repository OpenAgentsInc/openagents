---
spec_format_version: "0.1"
title: "OpenAgents Desktop Codex Workroom MVP"
artifact_type: "prd"
spec_revision: 1
author: "OpenAgents"
created_at: "2026-07-13T00:00:00Z"
updated_at: "2026-07-13T00:00:00Z"
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
  openagents_evidence: "docs/sol/2026-07-13-openagents-codex-workroom-mvp-audit.md"
---

## Problem

Codex is a capable local agent engine, but its execution power does not by
itself provide the OpenAgents product: one signed, durable place to find work,
understand typed turns and child agents, resolve blockers, inspect repository
effects, and return after restart without guessing what is authoritative.

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
recovery, then developers will complete and resume real Codex tasks inside
OpenAgents without needing an OpenAgents account or falling back to another
Codex interface.

## Scope

```productspec-scope
in:
  - signed and notarized OpenAgents Desktop artifact for the first supported macOS target
  - local-first use without an OpenAgents account
  - one compatible host-owned Codex app-server and one named isolated Codex account
  - explicit repository grant and stable OpenAgents coding-session and WorkContext refs
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
  - autonomous Session Goals, schedules, persistent voice, computer use, browser automation, or ambient memory
cut:
  - a second OpenAgents-owned model or tool loop beside Codex
  - direct raw Codex app-server, rollout, provider event, or terminal-text parsing in the renderer
  - renderer credentials, loopback secrets, generic IPC, raw MessagePort, Node, filesystem, process, or absolute-root authority
  - use of the default Codex home for device login or automatic work dispatch
  - model prose, connection health, optimistic UI, or issue closure as completion authority
  - public Codex, Claude, mobile, Fleet, cloud, portability, or voice claims not proven by their own current receipts
```

## User Experience

A developer installs OpenAgents Desktop and reaches a useful local workroom
without creating an OpenAgents account. The app either shows ready named Codex
capacity or one precise prerequisite. The developer grants a repository,
opens an existing top-level Codex thread or starts a new one, submits a task,
and watches typed work rather than terminal text.

Questions and approvals remain visible until durably resolved. Child agents
appear at their causal parent event and in a complete navigable graph; selecting
one opens its own transcript. File changes and the exact Git diff stay beside
the conversation. After renderer reload, stream loss, or app restart, the same
session returns with one honest pending, recovering, interrupted, failed, or
completed disposition and no duplicate turn.

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

Runtime Gateway is an adapter and lifecycle owner, not a second conversation
engine, public server, or parallel session database. Raw Codex history stays
owner-local by default.

## Acceptance Criteria

- A signed/notarized release candidate installs and launches without a source
  checkout, resolves only its pinned compatible Codex runtime path, and reports
  missing or incompatible runtime state explicitly.
- Local-first mode can reach the first useful Codex workroom without an
  OpenAgents account or hosted service. Connecting Codex uses a named isolated
  home and never mutates the default Codex home.
- Granting one repository creates a stable WorkContext and product session ref
  that do not derive from a path, process, port, machine, or provider thread ID.
- The session rail paints bounded metadata before transcript hydration, lists
  only top-level sessions, pages without an age ceiling, and preserves stable
  titles, status, attention, ordering, and selected session through restart.
- One real Codex task is durably admitted before dispatch and renders typed
  text plus at least one non-text plan, tool, patch/file-change, usage, blocker,
  or lifecycle item and exactly one terminal disposition.
- Exact retry reconciles to the admitted intent; conflicting reuse refuses.
  Send, stop, steer, queue, question, approval, and plan-review actions use the
  same registered command identities across direct, keyboard, palette, and
  native-menu entry points.
- The complete child graph retains exact parentage and lifecycle. A causal
  inline card opens one child's independent transcript; reload/reconnect never
  flattens, duplicates, re-roots, or leaks a child into the top-level catalog.
- The granted repository exposes a bounded file tree, Git status, and exact
  diff correlated to timeline item refs. Revocation and post-image conflict
  fail visibly without exposing general filesystem or Git mutation authority.
- Renderer reload does not stop or duplicate host-owned work. App-process
  restart restores the exact persisted prefix and either continues the
  recorded Codex thread at most once or records an explicit interrupted
  terminal outcome; it never silently reruns the task.
- Lost acknowledgement, duplicate/out-of-order frame, cursor gap, stale
  generation, revoked grant, quota exhaustion, rate limit, auth revocation,
  and policy denial converge to distinct typed states. Durable repair precedes
  live resubscription.
- The renderer contract and diagnostics contain no credential, account
  identity, loopback URL/secret, raw provider event, prompt/transcript body,
  repository content, absolute root, generic IPC, process handle, or general
  filesystem handle.
- The exact release candidate passes install, launch, one real Codex workroom
  task, renderer reload, app restart, interrupted update, rollback/downgrade
  refusal, diagnostics export, uninstall/reinstall, and cleanup receipts.

## Success Metrics

```productspec-success-metrics
- id: codex_workroom_activation
  metric: opted_in_first_launches_reaching_a_durably_admitted_codex_task_with_visible_typed_activity_within_15_minutes
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
- Which local-only, opt-in counters are sufficient to evaluate activation and
  return without collecting stable account, machine, repository, prompt, or
  transcript identity?

## Owner Gates

- Accept the exact installed-app Codex workroom journey and its explicit
  read-only-review boundary.
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
  repository grant, durable admission, typed stream, blocker/control, child
  transcript, exact diff, terminal outcome, renderer reload, and app restart.
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
