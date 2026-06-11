# Terminal Agent Systems

Date: 2026-06-11

This directory collects OpenAgents-native audits for the Bun/Effect terminal
agent system map. Each document defines a subsystem in product/runtime terms:
user-visible capability, durable state, Effect service boundary, authority
rules, projection safety, tests, smokes, and receipts.

Use `2026-06-11-terminal-agent-systems-index.md` as the full numbered system
map. The audits below are the currently imported subsystem records.

## Audits

- `2026-06-11-agent-runtime-kernel-audit.md`: Defines the durable runtime event
  contract, adapter model, native loop boundary, replay projections, and
  failure/cancellation/budget test shape.
- `2026-06-11-conversation-query-engine-audit.md`: Defines turn admission,
  message normalization, model/tool iteration, interruption, retries,
  compaction boundaries, and final result settlement.
- `2026-06-11-tool-registry-contracts-audit.md`: Defines typed tool
  registration, schema validation, permission mediation, progress streaming,
  result mapping, cancellation, and public-safe summaries.
- `2026-06-11-file-tool-system-audit.md`: Defines file read/search/write/edit
  capabilities, path normalization, large-file handling, freshness checks, and
  workspace-boundary enforcement.
- `2026-06-11-shell-execution-system-audit.md`: Defines command execution as an
  authority boundary, including PTY/non-PTY behavior, streaming output,
  sandboxing, timeouts, cancellation, and redaction.
- `2026-06-11-permission-approval-system-audit.md`: Defines permission
  decisions, approval prompts, remembered trust, denials, unattended execution,
  remote approvals, hooks, and audit records.
- `2026-06-11-sandbox-workspace-boundary-audit.md`: Defines the common
  workspace boundary used by tools, tasks, adapters, and context assembly
  before reading, writing, executing, or exposing local state.
- `2026-06-11-worktree-workspace-materialization-audit.md`: Defines isolated
  workspaces, git checkouts/worktrees, cleanup/retention decisions, patch refs,
  verification refs, and retained-workspace receipts.
- `2026-06-11-task-background-execution-audit.md`: Defines foreground and
  background task supervision, subagent/delegated work, output streams,
  cancellation, notifications, and closeout receipts.
- `2026-06-11-plan-todo-progress-state-audit.md`: Defines plans, todos,
  progress updates, blockers, ownership, and closeout state without letting
  model prose fake completion.
- `2026-06-11-error-taxonomy-recovery-audit.md`: Defines typed failure classes,
  recovery decisions, retry/ask/deny/continue/stop behavior, and user-safe
  failure reporting.
- `2026-06-11-context-assembly-system-audit.md`: Defines bounded context
  snapshots with provenance, priority, redaction, token cost, freshness,
  retrieved files, diagnostics, memory, and tool outputs.
- `2026-06-11-compaction-summarization-system-audit.md`: Defines manual and
  automatic compaction, context-limit recovery, tool-result trimming, summary
  boundaries, and post-compaction restoration.
- `2026-06-11-token-cost-budgeting-audit.md`: Defines context-window usage,
  per-run budget spend, provider cost estimates, quota state, budget stops,
  and public-safe usage reporting.
- `2026-06-11-model-provider-abstraction-audit.md`: Defines provider/model
  selection, aliases, capabilities, custom ids, streaming normalization,
  fallbacks, retries, and provider-edge metadata handling.
- `2026-06-11-prompt-instruction-layering-audit.md`: Defines deterministic
  instruction precedence across system, developer, project, memory, skill,
  command, style, and mode sources.
- `2026-06-11-session-memory-system-audit.md`: Defines scoped memory storage,
  discovery, retrieval, updates, summaries, redaction, edit/delete flows, and
  consent boundaries.
- `2026-06-11-repository-memory-onboarding-audit.md`: Defines first-run repo
  scanning, durable project profiles, workflow detection, instruction/invariant
  discovery, and profile refresh.
- `2026-06-11-semantic-retrieval-search-audit.md`: Defines combined exact
  search, structured parsing, semantic ranking, and model-assisted selection
  without keyword-only routing.
- `2026-06-11-lsp-diagnostics-system-audit.md`: Defines language-server
  lifecycle, code facts, diagnostics, symbols, navigation, and post-edit
  feedback as optional typed context.
- `2026-06-11-terminal-ui-shell-audit.md`: Defines terminal rendering of
  messages, scrollback, prompt input, modals, status, background activity,
  search, resize behavior, and runtime/UI separation.
- `2026-06-11-input-keybinding-system-audit.md`: Defines prompt editing,
  history, paste handling, multiline input, action keybindings, modal
  shortcuts, and conflict resolution.
- `2026-06-11-command-system-audit.md`: Defines slash commands and command
  palette entries as typed, discoverable, permission-aware runtime
  capabilities.
- `2026-06-11-diff-patch-review-ui-audit.md`: Defines structured diff and
  patch review artifacts, file groups, hunks, edit intent, approvals,
  comments, and receipt status.
- `2026-06-11-notifications-attention-system-audit.md`: Defines attention
  events, local notifications, waiting-for-user states, background completion,
  quiet hours, and privacy-aware delivery.
- `2026-06-11-resume-rewind-session-navigation-audit.md`: Defines durable
  transcripts, session listing/search, resume, fork, rewind, workspace restore,
  and corrupted-record recovery.
- `2026-06-11-help-doctor-debug-surfaces-audit.md`: Defines help, command
  discovery, environment diagnostics, support bundles, performance diagnostics,
  logs, and redacted debug exports.
- `2026-06-11-mcp-client-system-audit.md`: Defines MCP server discovery,
  trust policy, transport/auth state, tool/resource/prompt projection, and
  per-server privacy boundaries.
- `2026-06-11-mcp-server-system-audit.md`: Defines exposing selected local
  agent capabilities as bounded MCP tools/resources without turning the whole
  runtime into a remote-control endpoint.
- `2026-06-11-plugin-system-audit.md`: Defines plugin manifests, discovery,
  install/update/disable flows, sandboxing, policy gates, and component-level
  capability loading.
- `2026-06-11-skill-system-audit.md`: Defines skill discovery, descriptors,
  trigger rules, progressive disclosure, assets/scripts, and workflow
  boundaries without hidden always-on prompt text.
- `2026-06-11-hook-event-system-audit.md`: Defines lifecycle/tool/policy/
  observability hooks as typed, ordered, policy-governed events that cannot
  bypass normal authority.

## Index

- `2026-06-11-terminal-agent-systems-index.md`: Lists the full 62-system map,
  grouped by P0/P1/P2 priority, and names the first ten system docs that form
  the core safety and capability spine.
