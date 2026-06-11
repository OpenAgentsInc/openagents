# Terminal Agent Systems Operationalization Audit, Recommendations, And Roadmap

Date: 2026-06-11

## Scope

This document decides how to operationalize the terminal-agent systems audits
relative to the live Autopilot MVP issue sprint tracked by #4786.

Inputs reviewed:

- #4786 via `gh issue view`, including the Codex executor and Agent Runtime
  Kernel addenda.
- Current #4755-#4809 issue state via `gh issue list`.
- `docs/autopilot-coder/README.md`.
- `docs/autopilot-coder/2026-06-11-autopilot-unified-audit-roadmap.md`.
- `docs/autopilot-coder/terminal-agent-systems/README.md`.
- `docs/autopilot-coder/terminal-agent-systems/2026-06-11-terminal-agent-systems-index.md`.
- The dated terminal-agent subsystem audits in this directory, with closer
  reads on the runtime kernel, worktree materialization, task/background
  execution, scheduling, notifications, artifacts/receipts, structured event
  log, permissions, Git/GitHub, external intake, testing/smokes, credentials,
  settings, security, telemetry, and retention audits.

This is a planning document only. It does not create product promises, widen
public claims, or change runtime invariants.

## Current State From The Issue Ladder

As of the `gh` check on 2026-06-11:

- Closed: B1-B4 (#4755-#4758), M1-M5 (#4759-#4763), CX1-CX5
  (#4788-#4792), the Codex executor epic (#4793), worktree/materializer
  support (#4798, #4799), RK1-RK5 (#4805-#4809), the Agent Runtime Kernel epic
  (#4804), and P8 (#4784).
- Open: M6-M14 (#4764-#4772), A1-A4 (#4773-#4776), P1-P7 (#4777-#4783),
  P9 (#4785), and the parent epic #4786.
- M9 (#4767) is still open even though the roadmap records a CI-safe
  rate-limit-rotation leg as done; the live two-account leg remains a separate
  proof before smart-routing copy should broaden.

The practical result: the sprint has already landed the base dogfood loop,
the own-Pylon/free-lane policy, cloud Pylon, card-on-file, the Codex peer
adapter, the shared runtime event contract, and kernel-derived status
surfaces. The next decision is not whether to build a runtime kernel. The next
decision is which audits turn that landed kernel into repeatable, unattended,
receipt-backed MVP proof.

## Audit Finding

The terminal-agent folder is a dependency graph, not a flat backlog. The 62
audits cluster into five operational sets:

1. **Runtime truth set**: Agent Runtime Kernel, Structured Event Log,
   Conversation/Query, Tool Registry, Context Assembly, Compaction, Token/Cost
   Budgeting, Model Provider Abstraction, Prompt/Instruction Layering, Error
   Taxonomy, and Plan/Todo state. These define the event-sourced truth the rest
   of the product should read.
2. **Authority and workspace set**: Permission/Approval, Sandbox/Workspace
   Boundary, File Tool, Shell Execution, Worktree/Workspace Materialization,
   Settings/Configuration, Authentication/Credential Storage, and Security
   Review. These keep local, hosted, and delegated work from inventing
   separate safety policies.
3. **Operational proof set**: Task/Background Execution, Scheduling/Cron,
   Notifications/Attention, Mobile/Web Companion, Testing/Smoke, Telemetry/
   Privacy, Performance, Migration, Update/Release, and Data Retention. These
   decide whether the product can run while the operator is away and still
   prove what happened.
4. **Delivery and evidence set**: Git/GitHub Workflow, Diff/Patch Review,
   Artifact/Receipt, External Work Intake, Team/Shared Memory, Multi-Agent
   Coordination, and Repository Memory/Onboarding. These connect work orders to
   real repos, reviews, PR drafts, acceptance, and later market inventory.
5. **Extension and polish set**: MCP client/server, Plugin, Skill, Hook/Event,
   IDE/Editor, Browser/Desktop, Voice/Multimodal, Command/Input/TUI, Prompt
   Suggestions, Tips, Theme, Accessibility/Non-Interactive, Localization, and
   Enterprise Policy. These are important, but most should consume the runtime
   and authority sets rather than define new authority paths.

The live issue ladder has already operationalized much of set 1 through RK1-
RK5 and much of the workspace slice through #4798/#4799. The open MVP issues
now stress set 3 first: M6 scheduling/continuation, M7 decisions/
notifications, M9 live rate-limit proof, M10 overnight unattended proof, and
A1 API parity. If those ship without the task, schedule, notification, smoke,
artifact, and event-log contracts, the product will have useful features but
weak receipts.

## Recommendation

Operationalize the audits in packs, not one subsystem at a time.

### Pack A: MVP Proof And Supervision, During The Current Sprint

Operationalize this pack immediately, alongside M6, M7, M9, M10, and A1:

- Task And Background Execution
- Scheduling And Cron
- Notifications And Attention
- Mobile And Web Companion
- Testing And Smoke
- Artifact And Receipt
- Structured Event Log
- Token And Cost Budgeting
- Permission And Approval
- Accessibility And Non-Interactive Mode

Why this pack first:

- M6 cannot be honest without schedule records, continuation policy, budget
  gates, skip/fired receipts, and no double-fire behavior.
- M7 cannot be honest without typed attention events, decision projections,
  remote/mobile-safe approval state, and public-safe notification payloads.
- M10 cannot be accepted as an overnight proof without task supervision,
  scheduling receipts, notification receipts, and replayable event evidence.
- A1 cannot be enforced if browser status, terminal status, and agent API
  status are not projections over the same event and receipt records.
- Non-interactive/headless behavior is not polish here; it is the difference
  between unattended execution and a hidden prompt waiting forever.

Acceptance shape for this pack:

- Every scheduled or continued run has a schedule or continuation receipt.
- Every background run has a task ref, output ref, artifact refs, terminal
  state, and exactly-once completion notification behavior.
- Every proof smoke states its boundary and emits public-safe receipts.
- Every new public or agent-readable projection carries `generatedAt` and the
  applicable staleness metadata under the #4751/#4800 law.
- Every new API route or route shape is represented in the served OpenAPI
  contract under #4752.
- Every no-prompt/headless blocker is a typed denial or waiting state, not an
  indefinite hang.

### Pack B: Account, Credential, And Policy Hardening, Start During M8/M13

Operationalize this pack as soon as M8 account-pool work starts, and treat it
as a prerequisite for broad provider-peer work in M13:

- Authentication And Credential Storage
- Settings And Configuration
- Security Review
- Telemetry And Privacy
- Data Retention And Deletion
- Enterprise Managed Policy, only as the typed policy shape needed for teams
  and approved-user gates

Why this pack second:

- M8 shows provider account state, cooldowns, low-credit flags, and reconnect
  nudges. That surface is only safe if credentials, leases, refresh state, and
  redaction boundaries are represented as refs and effective-policy snapshots.
- M13 adds provider peers. The audit decision says credential storage is the
  trust root before more provider peers, and the roadmap already requires ToS
  review first.
- Team and design-partner use will need policy explanations before enterprise
  polish. The minimum is resolved policy snapshots and denial reasons, not a
  full managed-admin product.

Acceptance shape for this pack:

- Raw tokens and device credentials never enter mission, artifact, receipt, or
  public projection records.
- Account leases and credential refs attach to runs without exposing secrets.
- Provider-peer additions include ToS/credential-boundary review, redaction
  fixtures, and revocation behavior.
- Telemetry uses refs and aggregates rather than raw transcript, prompt,
  shell-output, or private-repo collection.
- Retention/deletion behavior is declared for every persisted data class added
  by M8, M13, and team-budget work.

### Pack C: Repo Scope, Delivery, And Evidence, Start During M11/P3

Operationalize this pack when M11 repo/data-scope UX and P3 writeback symmetry
begin, with early design input during M7/M10 because proofs need artifacts:

- Git And GitHub Workflow
- Diff And Patch Review UI
- File Tool System
- Shell Execution System
- Sandbox And Workspace Boundary
- Worktree And Workspace Materialization
- Artifact And Receipt
- Repository Memory And Onboarding
- LSP And Diagnostics, as optional typed context only

Why this pack third:

- The MVP already uses `git_checkout` and own-Pylon execution, but public
  issue-to-PR claims wait on P3. The Git audit correctly says low-level Git
  safety, repository identity, diff bounding, and review-thread ingestion are
  prerequisites for writeback symmetry.
- M11 needs per-mission scope declarations and placement explanations. That is
  the workspace boundary audit in product form.
- P3 needs change capture and delivery authority records, not shell-command
  transcripts.

Acceptance shape for this pack:

- Repository identity, worktree identity, branch refs, change captures, and
  delivery authority are typed records.
- PR draft writeback consumes artifact/change refs and emits delivery
  receipts.
- Branch names, refs, and Git metadata are safely parsed and never interpolated
  raw into shell commands.
- Public artifacts include summaries, digests, refs, and caveats, not private
  remotes, raw paths, raw shell logs, or private repo contents.
- Scope denial, data-classification denial, and placement explanations are
  visible to both web and agent/API surfaces.

### Pack D: Intake And Market Unification, After MVP Proof Is Credible

Operationalize this pack around A3, A4, P1, P2, P5, P6, and P7:

- External Work Intake
- Multi-Agent Coordination
- Team And Shared Memory
- MCP Server, only for deliberately exported bounded capabilities
- Artifact And Receipt
- Testing And Smoke
- Settlement Visibility Law from P9

Why this waits:

- External work intake should be one admission pipeline with many doors, but
  the current MVP still needs scheduling, decisions, account visibility, data
  scope, and proof smokes.
- P2 mission/work-order unification becomes easier and safer after the event
  log, artifact refs, task supervision, and Git delivery records have already
  been exercised by the MVP surfaces.
- Lane C paid provider work must wait for settlement bridge and settlement
  visibility receipts. Payment evidence must not substitute for acceptance or
  payout settlement.

Acceptance shape for this pack:

- UI, API, Forum, issue, schedule, autonomous, and agent-originated requests
  normalize into one admitted work-order shape with idempotency.
- Admission, rejection, routing, execution, review, delivery, acceptance, and
  settlement are separate receipts.
- Multi-agent decomposition is a supervision graph over normal assignments,
  not a privileged runtime mode.
- Market inventory and backlog faucet projections carry freshness metadata and
  do not expose private repo data.

### Pack E: Extension, Local Developer Experience, And Polish, Defer Until The Spine Holds

Defer broad operationalization of this pack until Pack A and the relevant
parts of Packs B/C are in place:

- MCP Client
- Plugin
- Skill
- Hook/Event
- IDE/Editor Integration
- Browser/Desktop Integration
- Voice/Multimodal Input
- Terminal UI Shell
- Input/Keybinding
- Command System
- Prompt Suggestions
- Tips/Education
- Theme/Visual Design
- Internationalization/Localization

The exception is when a specific open rung requires a thin slice. For example,
M2 already needed Pylon CLI status/review, and A1 needs structured JSON/non-
interactive output. In those cases, implement the minimal slice against the
runtime, permission, artifact, and settings services. Do not let extension or
polish systems become alternate authority channels.

## Roadmap

### R0: Lock The Operating Rule For The Current Sprint

Before taking more M-rungs, add Pack A audit references to the implementation
checklists for M6, M7, M9, M10, and A1.

Required rule:

- A rung that schedules, continues, notifies, blocks, asks for approval,
  completes in the background, or claims a proof must emit typed events and
  receipts. Model prose is not acceptance evidence.

### R1: M6/M7/M10 As One Unattended-Execution Slice

Implement scheduled launches, auto-continuation, decision queue,
notifications, and the overnight proof smoke as one operational slice even if
the GitHub issues stay separate.

Deliverables:

- Schedule records and fired/skipped/cancelled receipts.
- Continuation policy with budget, credential, workspace, and approval gates.
- Task supervisor projection for each queued or background run.
- Decision and notification projection derived from runtime events.
- Mobile-responsive review surface plus API parity.
- Overnight smoke covering SHC and own-Pylon/cloud-Pylon, with both web and
  terminal status visible.

Primary issue anchors:

- #4764 M6
- #4765 M7
- #4768 M10
- #4773 A1

### R2: Finish The Rate-Limit And Account Visibility Story

Treat M8 and the remaining M9 live leg as a paired credibility slice.

Deliverables:

- Account-pool dashboard reading credential refs, lease load, cooldowns,
  reset hints, low-credit state, and reconnect state.
- Live two-account rate-limit rotation proof, or an explicitly scoped blocker
  that keeps smart-routing copy narrow.
- Telemetry/privacy fixtures proving account-health and rate-limit metrics do
  not collect raw prompts, private repo data, or credentials.
- Credential revocation and stale-lease invalidation behavior.

Primary issue anchors:

- #4766 M8
- #4767 M9
- #4771 M13, only for the ToS/credential-boundary review that should start
  early

### R3: Repo Scope And Writeback

Make M11 and P3 a typed delivery slice, with M12 spend-to-evidence consuming
the same artifact/receipt model.

Deliverables:

- Repo connection and data-scope records.
- Placement explanations backed by trust-tier and lane-policy refs.
- Repository identity and safe Git state snapshots.
- Change capture records for patches, verification output, and PR draft
  candidates.
- Delivery authority and delivery receipt records.
- Ledger-to-mission-to-artifact joins for team and spend views.

Primary issue anchors:

- #4769 M11
- #4770 M12
- #4779 P3

### R4: Agent Parity And Intake Unification

Once scheduled/background work and repo delivery have receipts, converge agent
and human intake.

Deliverables:

- A1 parity matrix covering submit, status/events, decisions/review,
  scheduling, lane/pricing visibility, repo scope, and receipts.
- Forum-to-coding intake as a normal admitted work order.
- Autonomic work proposals using the same admission and budget rules.
- Mission/work-order unification plan that treats the event log and
  artifact/receipt layer as the shared record substrate.

Primary issue anchors:

- #4773 A1
- #4775 A3
- #4776 A4
- #4778 P2

### R5: Market Lane And Settlement

Only after MVP proof and intake unification are credible, operationalize the
market-facing audits.

Deliverables:

- First negotiated labor job pointed at a real backlog issue.
- USD-credit-to-sats settlement bridge with conversion refs and linked ledger
  entries.
- Backlog faucet into budgeted work requests.
- Spare-capacity Pylon provider mode behind capability envelopes.
- Lane C fanout with opt-in and public-tier-only placement at first.
- Settlement visibility law: recipient- and auditor-readable payout receipts
  before any broad labor claim.

Primary issue anchors:

- #4777 P1
- #4780 P4
- #4781 P5
- #4782 P6
- #4783 P7
- #4785 P9

### R6: Extension And Product Polish

After the spine is receipt-backed, expand local developer experience and
extension systems without changing authority boundaries.

Deliverables:

- MCP client and plugin installation as scoped capability ingestion.
- MCP server export only for narrow, schema-bound trusted capabilities.
- IDE/editor and browser/desktop integrations behind explicit capabilities and
  private evidence defaults.
- Voice/multimodal as an attachment-to-context pipeline, not an alternate
  instruction authority.
- Theme, tips, localization, and prompt suggestions that reflect capability
  state and never outrun receipts.

## Decision

Operationalize Pack A during the current #4786 sprint. It is the smallest set
that turns the already-landed runtime kernel and work-order loop into
unattended, inspectable, MVP-gating proof.

Start Pack B in parallel only where it directly serves M8 and M13. Start Pack
C design now, but implement it with M11/P3 rather than before the M6/M7/M10
proof slice. Hold Pack D and Pack E until the product can run, notify, resume,
show status, and produce public-safe receipts from the same event log.

The audit rule for future work is simple: if a capability changes work state,
spends money, touches credentials, mutates files, pushes code, asks for
approval, or supports public copy, it must land as a typed event, policy
decision, artifact, receipt, or projection. If it only lands as terminal text,
it is not operationalized.
