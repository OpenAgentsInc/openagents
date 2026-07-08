# OpenAgents Terminal-Agent Systems Current State

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-29

Issue: #6965

This audit compares the documented terminal-agent system map in
`docs/autopilot-coder/terminal-agent-systems/` with the current implementation
in this checkout. It focuses on the implementation areas called out by the
issue: Probe runtime (`packages/probe`), Pylon agent execution (`apps/pylon/src`,
especially Codex/Claude executors, workspace materialization, tools,
permissions, and sessions), and Khala CLI (`clients/khala-cli`).

## Executive Summary

OpenAgents already has a substantial terminal-agent substrate, but it is split
across two layers:

- **Production coding delegation is mostly Pylon + external agent SDKs.**
  `apps/pylon/src/agent-runner-registry.ts` routes assignment leases to the
  Claude Agent and Codex lanes. `apps/pylon/src/claude-agent-executor.ts` and
  `apps/pylon/src/codex-agent-executor.ts` materialize bounded workspaces,
  run the SDKs, execute verification commands, collect changed-file summaries,
  and submit public-safe closeouts.
- **OpenAgents-owned tool primitives exist primarily in Probe.**
  `packages/probe/packages/runtime/src/computer-use/tools.ts` exposes browser,
  terminal, and scoped filesystem tools through the Probe LLM tool contract in
  `packages/probe/packages/runtime/src/llm/tool.ts` and
  `packages/probe/packages/runtime/src/llm/tool-runtime.ts`.
- **The planned terminal-agent systems are partly implemented as typed
  evidence modules, not yet a single integrated terminal product.**
  `apps/pylon/src/tas/*.ts` contains small, tested domain modules for tool
  registry, commands, workspace boundary, compaction, MCP, plugin/skill, hooks,
  retention, policy, retrieval, task supervision, and related systems. Many are
  contract/evidence modules rather than live UI/runtime integrations.
- **Khala CLI can onboard and fan out Codex capacity.**
  `clients/khala-cli/src/fleet.ts` handles paste-free isolated Codex account
  linking. `clients/khala-cli/src/fleet-run.ts` plans/runs issue burn-down
  through Pylon. `clients/khala-cli/src/spawn.ts` supports local Codex workers
  and Pylon-backed `codex_agent_task` assignments.

The biggest gap is not absence of pieces. It is consolidation: OpenAgents has
workspace materialization, session control, task closeouts, exact token
reporting, scoped filesystem/terminal/browser tools, approval queues, and
policy modules, but the built-in read/grep/edit/shell tool set is not yet one
unified OpenAgents-native terminal-agent tool runtime that both Khala desktop
and CLI can reuse independent of Codex/Claude SDK tool implementations.

## Implementation Inventory

### Pylon Coding Delegation

| Area | Status | Code path | Notes |
| --- | --- | --- | --- |
| Assignment lease and closeout protocol | Done | `apps/pylon/src/assignment.ts` | Defines leases, acceptance/progress/closeout records, local lifecycle events, and no-spend coding assignment handling. |
| Agent runner registry | Done | `apps/pylon/src/agent-runner-registry.ts` | Chooses Claude or Codex runner from typed assignment payloads and capability refs. |
| Codex readiness and credentials | Done | `apps/pylon/src/codex-agent.ts` | Lazy SDK probe, presence-only credential detection, isolated per-account Codex homes, public-safe capability refs. |
| Codex composer | Partial/done for Pylon | `apps/pylon/src/codex-composer.ts` | Runs Codex SDK/CLI stream, parses events/usage, supports bounded vs owner-local danger execution; not a generic OpenAgents tool runtime. |
| Codex assignment executor | Done for own-capacity delegation | `apps/pylon/src/codex-agent-executor.ts` | Materializes fixture or public git checkout, runs Codex with owner-local `danger-full-access` and `never` approvals, then validates file changes remained inside workspace. |
| Codex turn/event reporting | Done | `apps/pylon/src/codex-turn-reporter.ts` | Posts exact token usage and raw event chunks/final raw events to owner-scoped ingest endpoints. |
| Claude readiness and credentials | Done | `apps/pylon/src/claude-agent.ts` | Lazy SDK probe, BYOK/local session presence, public-safe readiness refs. |
| Claude assignment executor | Done | `apps/pylon/src/claude-agent-executor.ts` | Maps abstract tool kinds to Claude tools (`Read`, `Edit`, `Write`, `Bash`, `Glob`, `Grep`), runs verification, blocks tool inputs that escape workspace. |
| Claude turn reporting | Done | `apps/pylon/src/claude-turn-reporter.ts` | Posts exact own-capacity Claude turn usage to `/api/pylon/claude/turns`. |
| Workspace materialization | Done | `apps/pylon/src/workspace-materializer.ts` | Validates public GitHub checkout payloads, pinned commits, safe verification argv, checkout cache, change capture, conflict scans, and cleanup refs. |
| Pull request publishing from assignments | Done | `apps/pylon/src/codex-pr-publisher.ts` | Publishes verified assignment diffs to GitHub branches/PRs. |
| Active run capacity | Done | `apps/pylon/src/active-assignment-runs.ts` | Tracks fresh local Claude/Codex runs, including per-account Codex load for heartbeat capacity projection. |
| Quota-aware account routing | Partial/done for Codex fleet | `apps/pylon/src/account-quota.ts`, `apps/pylon/src/account-quota-ledger.ts`, `apps/pylon/src/account-usage.ts` | The 2026-06-13 design doc is implemented in pieces for usage observations and account availability, but not a general provider-neutral router. |

### Probe Runtime Tools

| Area | Status | Code path | Notes |
| --- | --- | --- | --- |
| LLM tool contract | Done | `packages/probe/packages/runtime/src/llm/tool.ts` | Tool definitions have name, description, JSON input schema, optional output schema, Effect execute handler, and result projection. |
| Tool dispatch | Done | `packages/probe/packages/runtime/src/llm/tool-runtime.ts` | Dispatches by tool name, validates object input shape, emits tool result/error events. |
| Browser tools | Done | `packages/probe/packages/runtime/src/computer-use/tools.ts`, `browser.ts`, `page.ts`, `playwright-page.ts` | Exposes navigate, click, type, read text/DOM, wait, and screenshot over a real browser surface. |
| Terminal tool | Done | `packages/probe/packages/runtime/src/computer-use/tools.ts`, `terminal.ts`, `node-pty.ts` | Exposes `terminal_run` over PTY; raw output is returned to the caller but timeline beats keep public summaries ref-only. |
| Filesystem tools | Done | `packages/probe/packages/runtime/src/computer-use/tools.ts`, `filesystem.ts`, `workspace.ts` | Exposes `fs_read` and `fs_write` scoped by `resolveWorkspacePath`; writes are permission-gated. |
| Permission handler | Partial | `packages/probe/packages/runtime/src/permission.ts` | Has typed `PermissionRequest` and pluggable handler, but the default handler allows. The file notes interactive prompt plumbing is future work. |
| Timeline/evidence | Done for Probe | `packages/probe/packages/runtime/src/computer-use/timeline.ts`, `benchmark/closeout-writer.ts` | Records public-safe beats and closeout artifacts for Probe runs. |
| Provider tool schemas | Partial | `packages/probe/packages/runtime/src/backends/gemini/tool-schema.ts`, `backends/apple-fm/tools.ts` | Provider-specific adapters exist, but there is not yet one shared OpenAgents-native coding-tool schema consumed by Pylon Codex/Claude assignment lanes. |

### Khala CLI And Fleet

| Area | Status | Code path | Notes |
| --- | --- | --- | --- |
| Codex fleet onboarding | Done | `clients/khala-cli/src/fleet.ts` | Resolves Pylon home, creates isolated Codex account homes, forces file credential store, runs device login, updates Pylon config, and reports readiness. |
| Fleet run supervisor | Partial/done | `clients/khala-cli/src/fleet-run.ts` | Builds public-safe issue work plans, validates repo/commit/verify args, publishes capacity, and routes through Pylon. The loop still uses polling/backoff rather than a full durable queue UI. |
| Spawn local/Pylon workers | Partial/done | `clients/khala-cli/src/spawn.ts` | Supports local Codex workers and Pylon `codex_agent_task` assignments with bounded counts, local run projections, cancellation, and per-worker summaries. |
| Local Codex invocation | Done | `clients/khala-cli/src/codex.ts` | Runs bounded local Codex tasks for Khala CLI workflows. |
| CLI command parsing | Done | `clients/khala-cli/src/cli.ts` | Wires `fleet`, `spawn`, `run`, status, and input validation surfaces. |

## Documented System Map

Status definitions:

- **Done** means there is live implementation matching the documented system's
  core runtime responsibility.
- **Partial** means there is a meaningful implementation slice, but the full
  documented system is not integrated end-to-end.
- **Not built** means this checkout has no substantial implementation beyond
  docs, fixtures, or isolated stubs.

| # | Documented system | Status | Primary implementation paths | Notes |
| ---: | --- | --- | --- | --- |
| 1 | Agent Runtime Kernel | Partial | `apps/pylon/src/agent-runner-registry.ts`, `apps/pylon/src/assignment.ts`, `apps/pylon/src/node/sessions-exec.ts`, `packages/probe/packages/runtime/src/runtime/backend-assignment.ts` | Pylon has assignment runners and sessions; Probe has backend assignment contracts. Not one unified runtime kernel across all tools. |
| 2 | Conversation And Query Engine | Partial | `packages/probe/packages/runtime/src/llm/request.ts`, `packages/probe/packages/runtime/src/llm/events.ts`, `apps/pylon/src/codex-composer.ts`, `apps/pylon/src/claude-composer.ts` | Streaming/event loops exist per adapter, but no shared OpenAgents-native conversation engine owns all model/tool iteration. |
| 3 | Tool Registry And Tool Contracts | Partial | `apps/pylon/src/tas/tool-registry.ts`, `packages/probe/packages/runtime/src/llm/tool.ts`, `packages/probe/packages/runtime/src/llm/tool-runtime.ts` | Probe has executable tools; Pylon TAS has a minimal contract registry. They are not yet merged. |
| 4 | File Tool System | Partial | `packages/probe/packages/runtime/src/computer-use/filesystem.ts`, `packages/probe/packages/runtime/src/computer-use/tools.ts`, `apps/pylon/src/tas/workspace-boundary.ts`, `apps/pylon/src/claude-agent-executor.ts` | Scoped read/write exists in Probe; Claude lane delegates to SDK file tools with guard checks; Codex lane delegates to SDK and verifies changes post-hoc. |
| 5 | Shell Execution System | Partial | `packages/probe/packages/runtime/src/computer-use/terminal.ts`, `packages/probe/packages/runtime/src/computer-use/node-pty.ts`, `apps/pylon/src/codex-agent-executor.ts`, `apps/pylon/src/claude-agent-executor.ts` | Probe owns PTY terminal tools. Pylon assignment verification uses local command runners and SDK shell tools, not a shared shell service. |
| 6 | Permission And Approval System | Partial | `packages/probe/packages/runtime/src/permission.ts`, `apps/pylon/src/node/approval-queue.ts`, `apps/pylon/src/node/auto-approval-policy.ts`, `apps/pylon/src/node/sessions-exec.ts` | Typed approvals exist, including bounded auto-approval for sessions. Probe default is allow, so enforcement depends on caller wiring. |
| 7 | Sandbox And Workspace Boundary | Partial | `apps/pylon/src/workspace-materializer.ts`, `apps/pylon/src/codex-agent-executor.ts`, `apps/pylon/src/claude-agent-executor.ts`, `packages/probe/packages/runtime/src/workspace.ts` | Workspace scope is real. Codex owner-local danger mode relies on post-hoc file-change checks; Probe filesystem is pre-scoped. |
| 8 | Worktree And Workspace Materialization | Done | `apps/pylon/src/workspace-materializer.ts` | Public git checkout validation, pinned commits, safe verify argv, cache cleanup, change capture, and conflict scans are implemented. |
| 9 | Task And Background Execution | Partial | `apps/pylon/src/assignment.ts`, `apps/pylon/src/active-assignment-runs.ts`, `apps/pylon/src/tas/task-supervision.ts`, `clients/khala-cli/src/spawn.ts` | Assignments and spawn workers are live; TAS supervision is smaller contract logic. |
| 10 | Plan, Todo, And Progress State | Partial | `apps/pylon/src/assignment.ts`, `apps/pylon/src/tas/task-supervision.ts`, `apps/pylon/src/node/sessions-exec.ts` | Assignment progress and session outcomes exist, but not a general plan/todo state model exposed across clients. |
| 11 | Error Taxonomy And Recovery | Partial | `apps/pylon/src/tas/error-taxonomy.ts`, `apps/pylon/src/session-error-class.ts`, `apps/pylon/src/presence-error.ts` | Typed classifiers exist; adapter-specific recovery remains scattered. |
| 12 | Context Assembly System | Partial | `apps/pylon/src/tas/context-assembly.ts`, `packages/probe/packages/runtime/src/benchmark/openagents-autopilot-coder-studied-context.ts` | Evidence/context snapshots exist; not wired as a central runtime context assembler. |
| 13 | Compaction And Summarization | Partial | `apps/pylon/src/tas/compaction.ts`, `packages/probe/packages/runtime/src/llm/messages.ts` | Decision and record helpers exist; no full session compaction loop in Pylon assignment runners. |
| 14 | Token And Cost Budgeting | Partial | `apps/pylon/src/codex-turn-reporter.ts`, `apps/pylon/src/claude-turn-reporter.ts`, `apps/pylon/src/tas/budget.ts`, `packages/probe/packages/runtime/src/fleet/token-usage.ts` | Exact usage reporting is strong for Pylon SDK turns; broader budget stop/cost model remains partial. |
| 15 | Model Provider Abstraction | Partial | `apps/pylon/src/tas/model-provider.ts`, `packages/probe/packages/runtime/src/backends/registry.ts`, `packages/probe/packages/runtime/src/backends/backend-profile.ts` | Probe has backend registry/profile; Pylon has Codex/Claude-specific runners. |
| 16 | Prompt And Instruction Layering | Partial | `apps/pylon/src/tas/prompt-layering.ts`, `apps/pylon/src/codex-agent-executor.ts`, `apps/pylon/src/claude-agent-executor.ts` | TAS ordering exists; live assignment prompts are manually assembled per executor. |
| 17 | Session Memory System | Partial | `apps/pylon/src/tas/session-memory.ts`, `apps/pylon/src/session-record-store.ts` | Session records exist; memory is not a full user-facing recall/edit/delete system. |
| 18 | Repository Memory And Onboarding | Partial | `apps/pylon/src/tas/repo-memory.ts`, `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/repository-memory-profile.test.ts` | Repo memory/profile logic exists mostly as evidence/projection code and tests. |
| 19 | Semantic Retrieval And Search | Partial | `apps/pylon/src/tas/semantic-retrieval.ts` | Has vector similarity/top-k helpers; not a production semantic code search service. |
| 20 | LSP And Diagnostics System | Partial | `apps/pylon/src/tas/eval-regression.ts`, `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/context-snapshot.test.ts` | Diagnostics appear in projections/tests, but no integrated LSP manager was found. |
| 21 | Terminal UI Shell | Partial | `apps/pylon/src/node/control-server.ts`, `apps/pylon/src/node/control-client.ts`, `apps/pylon/src/node/runtime.ts`, `apps/pylon/src/tas/non-interactive.ts` | Pylon node/control surfaces exist; no full terminal TUI shell matching the audit. |
| 22 | Input And Keybinding System | Partial | `apps/pylon/src/node/keybinds.ts`, `packages/input-bindings` | Keybinding modules exist, but not a complete prompt editor/mode system. |
| 23 | Command System | Partial | `apps/pylon/src/tas/command-system.ts`, `apps/pylon/src/index.ts`, `clients/khala-cli/src/cli.ts` | CLI commands are real; TAS slash-command parser is minimal and not the sole command router. |
| 24 | Diff And Patch Review UI | Partial | `apps/pylon/src/tas/diff-review.ts`, `apps/pylon/src/workspace-materializer.ts`, `packages/autopilot-control-protocol` | Change summaries and artifacts exist; review UI is elsewhere/partial. |
| 25 | Notifications And Attention | Partial | `apps/pylon/src/node/notification-projection.ts`, `apps/pylon/src/node/notification-router.ts`, `apps/pylon/src/notifications/notification-delivery.ts` | Notification projection/routing exists; broader companion attention state is partial. |
| 26 | Resume, Rewind, Session Navigation | Partial | `apps/pylon/src/node/control-sessions.ts`, `apps/pylon/src/session-record-store.ts`, `apps/pylon/src/khala-requester.ts` | Session and durable Khala resume exist; rewind/fork restore is not fully implemented. |
| 27 | Help, Doctor, Debug Surfaces | Partial | `apps/pylon/src/dev-doctor.ts`, `apps/pylon/src/cli-catalog.ts`, `clients/khala-cli/src/cli.ts` | Doctor/help surfaces exist in CLI form; support bundles/perf diagnostics are partial. |
| 28 | MCP Client System | Partial | `apps/pylon/src/tas/mcp-client.ts`, `apps/pylon/src/khala-mcp.ts`, `packages/mcp-contract` | MCP contracts and Khala MCP path exist; full configurable client lifecycle is partial. |
| 29 | MCP Server System | Partial | `apps/pylon/src/tas/mcp-server.ts`, `apps/pylon/src/khala-mcp.ts` | Tool listing/dispatch contracts exist; export of selected local capabilities is limited. |
| 30 | Plugin System | Partial | `apps/pylon/src/tas/plugin-system.ts` | Registry/contribution helpers exist; full install/update/sandbox policy is not built here. |
| 31 | Skill System | Partial | `apps/pylon/src/tas/skill-system.ts` | Skill descriptor/registry helpers exist; not integrated as a full progressive-disclosure runtime. |
| 32 | Hook And Event System | Partial | `apps/pylon/src/tas/hook-event.ts`, `apps/pylon/src/assignment.ts` lifecycle events | Hook dispatch contracts exist; global hook integration is partial. |
| 33 | Settings And Configuration | Partial | `apps/pylon/src/tas/effective-config.ts`, `apps/pylon/src/state.ts`, `clients/khala-cli/src/fleet.ts` | Config snapshots and Pylon config writes exist, but no single settings service owns all domains. |
| 34 | Authentication And Credential Storage | Done/partial | `apps/pylon/src/account-connect.ts`, `apps/pylon/src/account-registry.ts`, `apps/pylon/src/codex-agent.ts`, `apps/pylon/src/claude-agent.ts`, `clients/khala-cli/src/fleet.ts` | Strong for provider/Codex/Claude account presence and isolated homes; broader auth UX still spread across tools. |
| 35 | Git And GitHub Workflow | Partial | `apps/pylon/src/workspace-materializer.ts`, `apps/pylon/src/codex-pr-publisher.ts`, `apps/pylon/src/git-receive-pack.ts` | Checkout/change capture/PR publish exist; review-comment and issue workflows are not one complete terminal-agent system. |
| 36 | IDE And Editor Integration | Partial | `apps/pylon/src/node/external-sessions.ts`, `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/editor-integration.test.ts` | Evidence/projection code exists; direct editor integrations are limited. |
| 37 | Browser And Desktop Integration | Partial | `packages/probe/packages/runtime/src/computer-use/browser.ts`, `playwright-page.ts`, `apps/pylon/src/node/apple-fm-*` | Browser tools and Apple FM bridge work exist; desktop GUI bridge is not unified with terminal-agent tools. |
| 38 | Voice And Multimodal Input | Partial | `clients/khala-ios/Khala`, `apps/pylon/src/node/apple-fm-*` | Voice/mobile app work exists outside terminal-agent runtime; not a terminal agent input subsystem yet. |
| 39 | Remote Session Bridge | Partial | `apps/pylon/src/node/control-server.ts`, `apps/pylon/src/node/control-client.ts`, `apps/pylon/src/node/bridge-*` | Loopback control and bridge pairing modules exist; broad remote observation/control is partial. |
| 40 | Mobile And Web Companion | Partial | `apps/pylon/src/node/control-server.ts`, `apps/openagents.com/apps/web`, `clients/khala-ios/Khala` | Companion surfaces exist, but approval/artifact review authority is split. |
| 41 | Team And Shared Memory | Partial | `apps/pylon/src/tas/team-memory.ts` | Contract helpers only. |
| 42 | Multi-Agent Coordination | Partial | `apps/pylon/src/tas/coordination.ts`, `clients/khala-cli/src/spawn.ts`, `clients/khala-cli/src/fleet-run.ts` | Claim ledger and spawn/fleet fanout exist; conflict/evidence aggregation is partial. |
| 43 | External Work Intake | Partial | `apps/pylon/src/tas/work-intake.ts`, `apps/pylon/src/khala-requester.ts`, `apps/openagents.com/workers/api/src/autopilot-work-request.ts` | Typed intake and Khala/Pylon assignment creation exist; not all sources share one intake pipeline. |
| 44 | Artifact And Receipt System | Partial/done for assignments | `apps/pylon/src/assignment.ts`, `apps/pylon/src/tas/evidence-receipt.ts`, `apps/pylon/src/workspace-materializer.ts`, `packages/probe/packages/runtime/src/benchmark/closeout-writer.ts` | Assignment closeouts and Probe closeouts are real; universal receipt index is partial. |
| 45 | Scheduling And Cron | Partial | `apps/pylon/src/tas/schedule-receipts.ts`, `clients/khala-cli/src/fleet-run.ts` supervisor loop | Scheduling receipt helpers exist; no full cron/delayed-run subsystem was found. |
| 46 | Structured Event Log | Partial | `apps/pylon/src/tas/event-log-projection.ts`, `apps/pylon/src/codex-turn-reporter.ts`, `packages/probe/packages/runtime/src/llm/events.ts` | Event shapes exist per subsystem; no single append-only event log for all terminal-agent activity. |
| 47 | Telemetry And Privacy | Partial | `apps/pylon/src/tas/telemetry.ts`, `apps/pylon/src/proof-redaction.ts`, `packages/probe/packages/runtime/src/receipt-redaction.ts` | Redaction and telemetry helpers exist; end-to-end privacy controls are partial. |
| 48 | Performance System | Partial | `apps/pylon/src/tas/performance.ts`, `apps/pylon/src/node/runtime.ts` | Performance summary/budget helpers and runtime loops exist; product diagnostics are partial. |
| 49 | Update And Release System | Partial | `apps/pylon/src/self-update.ts`, `docs/DEPLOYMENT.md`, `apps/oa-updates` | Release/update systems exist, but not terminal-agent-specific in one module. |
| 50 | Migration System | Partial | `apps/pylon/src/tas/migration.ts` | Migration planning helpers only. |
| 51 | Testing And Smoke System | Partial/done in many lanes | `apps/pylon/src/tas/smoke-proof.ts`, `apps/pylon/src/*smoke*.ts`, `packages/probe/packages/runtime/tests`, `apps/pylon/tests` | Strong tests and smoke fixtures exist, but no unified smoke orchestrator for all terminal-agent systems. |
| 52 | Evaluation And Regression | Partial | `apps/pylon/src/tas/eval-regression.ts`, `packages/probe/packages/runtime/src/benchmark/studybench-*` | Probe has studybench/eval harnesses; Pylon TAS has regression helpers. |
| 53 | Security Review | Partial | `apps/pylon/src/blueprint-gates/*`, `apps/pylon/src/proof-redaction.ts`, `packages/probe/packages/runtime/src/receipt-redaction.ts` | Security/redaction gates exist in specific lanes; no full threat-review subsystem. |
| 54 | Data Retention And Deletion | Partial | `apps/pylon/src/tas/retention.ts`, `apps/pylon/src/workspace-materializer.ts` cleanup/prune functions | Retention policies and workspace cleanup exist; user-facing delete/export flows are partial. |
| 55 | Onboarding System | Partial | `clients/khala-cli/src/fleet.ts`, `apps/pylon/src/bootstrap.ts`, `apps/pylon/src/dev-doctor.ts` | Fleet onboarding is strong; terminal-agent first-run onboarding remains partial. |
| 56 | Output Style And Persona | Not built/partial | `apps/pylon/src/tas/prompt-layering.ts` | Instruction layering can carry style, but no dedicated persona/style system was found. |
| 57 | Prompt Suggestions And Autocomplete | Not built/partial | `apps/pylon/src/cli-catalog.ts` | CLI catalog helps command discovery; autocomplete/suggestions are not substantially implemented. |
| 58 | Tips And Education | Partial | `apps/pylon/src/tips.ts`, `clients/khala-cli/README.md` | Tips/network docs exist; contextual terminal education is partial. |
| 59 | Theme And Visual Design | Partial | `packages/design-tokens`, `packages/ui`, `apps/openagents.com/apps/web` | UI tokens exist outside terminal-agent runtime. |
| 60 | Accessibility And Non-Interactive Mode | Partial | `apps/pylon/src/tas/non-interactive.ts`, `clients/khala-cli/src/cli.ts`, JSON outputs across Pylon/Khala | JSON/headless modes exist; complete accessibility coverage is partial. |
| 61 | Internationalization And Localization Boundary | Not built | No substantial implementation found | Schemas/refs are stable English; localization boundary remains a doc-level plan. |
| 62 | Enterprise And Managed Policy | Partial | `apps/pylon/src/tas/managed-policy.ts`, `apps/pylon/src/node/auto-approval-policy.ts`, provider-account policy modules under `apps/openagents.com/workers/api/src` | Policy evaluators exist, but enterprise/device/team policy is not a complete product surface. |

## Later Addenda

| Documented addendum | Status | Code path | Notes |
| --- | --- | --- | --- |
| Pylon quota-aware account routing and failover | Partial | `apps/pylon/src/account-quota.ts`, `apps/pylon/src/account-quota-ledger.ts`, `apps/pylon/src/account-usage.ts`, `clients/khala-cli/src/fleet-run.ts` | Account readiness, usage refresh, and capacity planning exist. Full automatic failover semantics are narrower than the planning doc. |
| Pylon session evidence schemas | Partial/done for Codex/Claude | `apps/pylon/src/codex-turn-reporter.ts`, `apps/pylon/src/claude-turn-reporter.ts`, `apps/pylon/src/khala-requester.ts` proof/status types | Exact token rows, raw event chunks, final traces, and closeout checklists are implemented for current Pylon SDK lanes. |
| Forge Autopilot Coder systems roadmap | Partial | `apps/pylon/src/tas/*`, `apps/openagents.com/apps/web/src/page/loggedIn/autopilot-work/*` | Many Forge evidence modules/tests exist, but this is still distributed across web projections and TAS modules. |

## Built-In Tools Khala Can Reuse

The reusable OpenAgents-owned primitives today are:

- **Scoped filesystem read/write:** `fs_read` and `fs_write` in
  `packages/probe/packages/runtime/src/computer-use/tools.ts`, backed by
  `packages/probe/packages/runtime/src/computer-use/filesystem.ts` and
  `packages/probe/packages/runtime/src/workspace.ts`.
- **Terminal shell execution:** `terminal_run` in Probe, backed by the PTY
  surface in `packages/probe/packages/runtime/src/computer-use/terminal.ts` and
  `node-pty.ts`.
- **Browser automation:** `browser_navigate`, `browser_click`, `browser_type`,
  `browser_read_text`, `browser_read_dom`, `browser_wait_for`, and
  `browser_screenshot` in `packages/probe/packages/runtime/src/computer-use/tools.ts`.
- **Tool definitions and dispatch:** `ProbeLlmTool` plus
  `dispatchProbeLlmTool` in `packages/probe/packages/runtime/src/llm/tool.ts`
  and `tool-runtime.ts`.
- **Workspace materialization and verification command contracts:** public
  `git_checkout` workspaces in `apps/pylon/src/workspace-materializer.ts`.
- **Assignment runners and closeouts:** `apps/pylon/src/assignment.ts`,
  `apps/pylon/src/agent-runner-registry.ts`, `apps/pylon/src/codex-agent-executor.ts`,
  and `apps/pylon/src/claude-agent-executor.ts`.
- **Approval queue and bounded session auto-approval:** `apps/pylon/src/node/approval-queue.ts`,
  `apps/pylon/src/node/sessions-exec.ts`, and
  `apps/pylon/src/node/auto-approval-policy.ts`.

The SDK-delegated tools are:

- **Claude SDK tools:** mapped from abstract kinds in
  `apps/pylon/src/claude-agent-executor.ts` to `Read`, `Edit`, `Write`,
  `Bash`, `Glob`, and `Grep`.
- **Codex SDK tools:** not registered by OpenAgents directly. The Codex lane
  runs the SDK in owner-local mode and then uses
  `fileChangeEscapesWorkspace()` in `apps/pylon/src/codex-agent-executor.ts`
  as independent post-hoc workspace verification.

## Missing Or Incomplete For Khala Desktop And CLI Reuse

- **Unified OpenAgents-native coding tool runtime.** Probe has tools and Pylon
  has assignment runners, but there is no single shared package exposing
  `read`, `grep/search`, `edit/apply_patch`, and `shell` as OpenAgents-owned
  typed tools with the same permission, workspace, event, and receipt contract
  across desktop, CLI, and Pylon assignments.
- **Pre-execution permission enforcement for every lane.** Probe has a
  pluggable permission handler whose default is allow. Pylon sessions have an
  approval queue and bounded auto-approval. Codex assignment execution is
  intentionally owner-local full access with post-hoc boundary verification.
  These are defensible for their lanes, but not a unified authority model.
- **Grep/search and edit primitives as first-class OpenAgents tools.** Claude
  provides `Grep`, `Glob`, `Edit`, and `Write` through its SDK. Probe currently
  has filesystem read/write and terminal commands, but not a dedicated
  OpenAgents-owned grep/edit/patch API with freshness checks and diff receipts.
- **One event log/projection model.** Pylon assignment progress, Codex raw
  chunks, Probe LLM events, TAS event-log helpers, and session records are all
  real, but they are separate projections.
- **Complete terminal product shell.** CLI surfaces and loopback control exist,
  but the documented terminal UI shell, prompt editor, keybinding system,
  autocomplete, diff review UI, and session navigation are not one integrated
  user-facing terminal app.
- **Provider-neutral model/tool loop.** Probe can dispatch tools for Probe
  backends, and Pylon can run Codex/Claude SDKs. A provider-neutral
  OpenAgents-native loop that owns model streaming, tool calls, approval,
  retries, compaction, and receipts across adapters remains partial.

## Recommended Next Consolidation Step

Create a shared OpenAgents terminal-tool package that wraps the already-built
Probe primitives and Pylon workspace/permission contracts:

1. Define typed tools for `read`, `search`, `edit`, `write`, `shell`, and
   optional browser actions using the Probe `ProbeLlmTool` contract or a
   compatible Effect Schema contract.
2. Reuse `apps/pylon/src/workspace-materializer.ts` and
   `apps/pylon/src/tas/workspace-boundary.ts` for workspace authority.
3. Route destructive tools through the Pylon approval queue / bounded
   auto-approval policy when running in sessions, and through an explicit
   installed Probe permission handler when running in Probe/desktop.
4. Emit one normalized event stream with public-safe summaries and private raw
   refs, compatible with `codex-turn-reporter.ts` and Probe timeline beats.
5. Let Khala CLI and desktop choose either OpenAgents-native tools or
   SDK-delegated Codex/Claude tools per workflow, but report them through the
   same closeout and receipt model.

That would turn the current set of strong but separate implementation slices
into a reusable tool substrate for Khala desktop, Khala CLI, Pylon delegation,
and future terminal-agent surfaces.
