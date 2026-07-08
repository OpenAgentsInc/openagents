# Terminal Agent Systems

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


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
- `2026-06-11-settings-configuration-system-audit.md`: Defines typed settings,
  policy precedence, live reload, remote sync, safe mutation, validation, and
  resolved configuration snapshots.
- `2026-06-11-authentication-credential-storage-audit.md`: Defines login,
  headless credentials, secure local storage, provider accounts, refresh,
  revocation, redaction, and auth-dependent cache invalidation.
- `2026-06-11-git-github-workflow-system-audit.md`: Defines repository
  identity, safe Git state reads, bounded diff capture, branch/PR delivery,
  review comment retrieval, issue writeback, and push authority.
- `2026-06-11-ide-editor-integration-audit.md`: Defines optional editor/IDE
  discovery, workspace matching, authenticated local transport, file opening,
  diagnostics, selected context, and fallback editor behavior.
- `2026-06-11-browser-desktop-integration-audit.md`: Defines safe URL opening,
  browser or desktop bridges, screenshot/clipboard evidence, GUI action
  approvals, and desktop-session handoff boundaries.
- `2026-06-11-voice-multimodal-input-audit.md`: Defines speech, screenshots,
  clipboard images, files, transcriptions, attachment review, provider
  capability checks, and typed media context refs.
- `2026-06-11-remote-session-bridge-audit.md`: Defines paired companion
  sessions, scoped remote observation/control, approval forwarding, connection
  records, revocation, typed bridge protocol, cursor resume, and backpressure
  behavior.
- `2026-06-11-mobile-web-companion-system-audit.md`: Defines mobile and web
  companion surfaces for status, approvals, artifact review, bounded
  instructions, cancellation, notification delivery, offline action rules, and
  read-only authority modes.
- `2026-06-11-team-shared-memory-system-audit.md`: Defines scoped shared
  memory records for teams, repositories, missions, provenance, retrieval
  policy, redaction, correction, and removal.
- `2026-06-11-multi-agent-coordination-system-audit.md`: Defines decomposed
  multi-lane work, assignment ownership, lane supervision, evidence
  aggregation, conflict handling, and coordinated closeout.
- `2026-06-11-external-work-intake-system-audit.md`: Defines API, issue,
  Forum, workroom, schedule, and agent-submitted work intake as typed work
  orders with admission, budget, scope, and review policy.
- `2026-06-11-artifact-receipt-system-audit.md`: Defines artifacts and
  receipts for patches, diffs, tests, previews, screenshots, closeouts,
  payment evidence, assignment events, and settlement projections.
- `2026-06-11-scheduling-cron-system-audit.md`: Defines delayed runs,
  recurring checks, overnight tasks, retry windows, continuation policy,
  approval requirements, and scheduled-work receipts.
- `2026-06-11-structured-event-log-audit.md`: Defines the append-only event
  log for replay, projections, receipts, debugging, audit trails, sequence
  integrity, redaction classes, and export.
- `2026-06-11-telemetry-privacy-system-audit.md`: Defines privacy-first
  telemetry modes, redacted diagnostics, metric categories, opt-out controls,
  local-only behavior, and raw private data exclusion.
- `2026-06-11-performance-system-audit.md`: Defines latency, throughput,
  memory, model streaming, tool duration, queueing, background responsiveness,
  rate limits, and redacted performance profiles.
- `2026-06-11-update-release-system-audit.md`: Defines signed releases,
  platform support, rollout channels, compatibility checks, release notes,
  smoke receipts, update policy, and rollback.
- `2026-06-11-migration-system-audit.md`: Defines state schema upgrades,
  restore points, validation, rollback boundaries, migration receipts, optional
  cache rebuilds, and private-data handling.
- `2026-06-11-testing-smoke-system-audit.md`: Defines unit, contract,
  integration, CI-safe, local-device, staging, and live smokes with explicit
  proof boundaries and retained evidence.
- `2026-06-11-evaluation-regression-system-audit.md`: Defines task suites,
  model/provider comparisons, regression detection, fixture promotion,
  redacted reporting, cost tracking, and release gates.
- `2026-06-11-security-review-system-audit.md`: Defines threat models, policy
  refs, redaction checks, approval gates, high-risk integration review,
  diagnostic bundles, and regression tests.
- `2026-06-11-data-retention-deletion-system-audit.md`: Defines retention
  policy, deletion, tombstones, exports, cache clearing, memory correction, and
  projection invalidation.
- `2026-06-11-onboarding-system-audit.md`: Defines first-run setup, repository
  connection, instruction/invariant review, provider readiness, capability
  snapshots, and setup smokes.
- `2026-06-11-output-style-persona-system-audit.md`: Defines tone, verbosity,
  formatting, persona, domain style, accessibility preferences, and policy-safe
  style composition.
- `2026-06-11-prompt-suggestions-autocomplete-audit.md`: Defines command,
  prompt, file, symbol, issue, session, and artifact suggestions while keeping
  intent selection explicit.
- `2026-06-11-tips-education-system-audit.md`: Defines contextual tips,
  first-run education, capability caveats, dismissals, rate limits, and
  claim-safe help content.
- `2026-06-11-theme-visual-design-system-audit.md`: Defines visual tokens,
  themes, contrast, status badges, density, terminal/web/mobile consistency,
  and reduced-motion support.
- `2026-06-11-accessibility-non-interactive-mode-audit.md`: Defines screen
  reader, keyboard-only, high-contrast, no-color, reduced-motion, JSON output,
  CI, scripting, and headless-mode behavior.
- `2026-06-11-internationalization-localization-boundary-audit.md`: Defines
  localized UI text, help, dates, numbers, and errors while keeping schemas,
  refs, policies, receipts, commands, and JSON stable.
- `2026-06-11-enterprise-managed-policy-system-audit.md`: Defines
  organization/team/repository/device policy, provider allowlists, budgets,
  retention, telemetry, plugin policy, update channels, and audit trails.

## Index

- `2026-06-11-terminal-agent-systems-index.md`: Lists the full 62-system map,
  grouped by P0/P1/P2 priority, and names the first ten system docs that form
  the core safety and capability spine.
- `2026-06-11-terminal-agent-systems-operationalization-roadmap.md`: Decides
  which subsystem audits to operationalize during and after the active #4786
  Autopilot MVP sprint, grouped into proof/supervision, credential/policy,
  repo/delivery, intake/market, and extension/polish packs. Pack A is tracked
  by GitHub issue #4813 and child issues #4814-#4823; Pack B is tracked by
  #4824 and child issues #4825-#4830.
- `2026-06-16-forge-autopilot-coder-systems-roadmap.md`: Audits #5107 against
  the current Forge cockpit, Pack A/B/C, Pylon, and control-protocol surfaces,
  splits the long arc into G1-G7 epics, and selects G1.1 as the first child
  issue.
- `2026-06-11-open-issue-delegation-plan.md`: Splits the open Autopilot MVP,
  Pack A, market, and W3 evaluation issue set across eight named delegated
  agents, including worktree/branch rules, issue status comment templates,
  merge waves, deferred E2E policy, parent closeout order, and the Pack B
  addendum for provider/account/policy hardening.
