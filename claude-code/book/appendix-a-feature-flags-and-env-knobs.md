# Appendix A — Complete Inventory of Build-Time `feature()` Flags & Environment Knobs

> **Scope:** Cross-cutting reference companion to [Chapter 00](prologue-how-to-read-a-1900-file-agent.md) through [Chapter 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)
> **Snapshot Census:** 90 unique build-time `feature('...')` flags (933 executable call sites across 197 files importing `bun:bundle`), 357 `process.env.USER_TYPE` checks in `.ts` files vs. 95 constant-folded `"external" ===/!== 'ant'` checks in `.tsx` files, and 472 distinct `process.env.*` keys across 1,884 `.ts`/`.tsx` source files.
> **Primary Source Files:**
> - `bridge/bridgeEnabled.ts` — canonical positive-ternary `feature()` DCE idiom
> - `tools/BashTool/bashPermissions.ts` & `tools/BashTool/pathValidation.ts` — Bun's per-function `feature()` DCE complexity cliff
> - `components/MemoryUsageIndicator.tsx` — constant-folded `USER_TYPE` build define in `.tsx`
> - `utils/envUtils.ts` — `isEnvTruthy()`, `isEnvDefinedFalsy()`, `isBareMode()`, `getClaudeConfigHomeDir()`
> - `utils/managedEnv.ts` & `utils/managedEnvConstants.ts` — `SAFE_ENV_VARS` allowlist and three-layer `filterSettingsEnv()` security boundary
> - `utils/subprocessEnv.ts` — child-process secret scrubbing (`CLAUDE_CODE_SUBPROCESS_ENV_SCRUB`) and CCR proxy injection
> - `entrypoints/cli.tsx` & `constants/system.ts` — top-level `ABLATION_BASELINE` injection and `NATIVE_CLIENT_ATTESTATION`

---

## Why Four Separate Gating Layers Exist

Reading Claude Code without understanding its gating taxonomy creates an illusion of arbitrary complexity: why does one capability check `feature('BRIDGE_MODE')`, another check `process.env.USER_TYPE === 'ant'`, a third read `process.env.CLAUDE_CODE_NO_FLICKER`, and a fourth query `getFeatureValue_CACHED_MAY_BE_STALE('tengu_ccr_bridge', false)`?

Each layer defends a distinct boundary that the others cannot enforce:

```text
  COMPILE TIME (Bun bundler + JSX compiler)
  +------------------------------------------------------------------------+
  | 1. feature('FLAG') from 'bun:bundle'   -> Dead-code elimination (DCE)  |
  |    & MACRO.* / USER_TYPE define           Strips code, strings, & deps |
  +------------------------------------------------------------------------+
                                      |
                                      v  (surviving binary)
  PROCESS STARTUP & SETTINGS LOAD (init.ts -> managedEnv.ts)
  +------------------------------------------------------------------------+
  | 2. process.env.* & settings.json `env` -> Operator / CI / IDE overrides|
  |    Filtered by SAFE_ENV_VARS pre-trust    & stripped for SSH/CCD/host  |
  +------------------------------------------------------------------------+
                                      |
                                      v  (runtime process)
  REMOTE CONTROL PLANE & POLICY (growthbook.ts / policyLimits)
  +------------------------------------------------------------------------+
  | 3. GrowthBook (`tengu_*`) & Policy     -> Live kill switches & A/B     |
  |    Disk-cached + background refresh       rollouts without shipping    |
  +------------------------------------------------------------------------+
                                      |
                                      v  (per-request / per-tool)
  CHILD PROCESS BOUNDARY (subprocessEnv.ts)
  +------------------------------------------------------------------------+
  | 4. GHA_SUBPROCESS_SCRUB + proxy env    -> Strips parent auth secrets   |
  |    before Bash / MCP / LSP / hooks        from prompt-injectable tools |
  +------------------------------------------------------------------------+
```

Compile-time flags exist for **bundle size, secrecy, and zero-overhead exclusion**: unreleased codenames, internal GrowthBook keys, and ant-only telemetry never ship in external binaries. Environment variables exist for **machine-local configuration, CI/SDK orchestration, and emergency escape hatches** before a network round-trip can occur. GrowthBook gates provide **instant post-release kill switches**. Understanding how these layers compose prevents misreading a dead external branch as live production behavior.

---

## 1. How Build-Time `feature()` Dead-Code Elimination Works (And Where It Breaks)

### The Three Compile-Time Primitives

Across the repository, Bun's bundler resolves three compile-time mechanisms before emitting a release artifact:

1. **`import { feature } from 'bun:bundle'`** (imported by **197** files; **90** unique flags; **933** executable call sites). Every flag compiles to a literal `true` or `false` at bundle time, letting the minifier drop unreachable `require(...)` calls, prompt text, tools, and GrowthBook gate names.
2. **`MACRO.*` build constants** (injected globally): `MACRO.VERSION`, `MACRO.BUILD_TIME`, `MACRO.PACKAGE_URL`, `MACRO.NATIVE_PACKAGE_URL`, `MACRO.FEEDBACK_CHANNEL`, `MACRO.ISSUES_EXPLAINER`, and `MACRO.VERSION_CHANGELOG`. Used for zero-import `--version` (`entrypoints/cli.tsx:37-42`) and updater URLs.
3. **`USER_TYPE` define** (`'ant'` vs. `'external'`), covered in Section 3.

### Three Production Scars in Bun's `feature()` Evaluator

Treating `feature('...')` like an ordinary boolean helper causes silent leaks and silent feature regressions. Three scars documented directly in the source capture the exact traps:

**Scar 1 — Positive Ternary Required to Strip String Literals.** In `bridge/bridgeEnabled.ts`, `voice/voiceModeEnabled.ts`, `hooks/useReplBridge.tsx`, and `tools/BriefTool/upload.ts`, engineers deliberately avoid early-return negative guards:

```ts
// bridge/bridgeEnabled.ts:28-36
export function isBridgeEnabled(): boolean {
  // Positive ternary pattern — see docs/feature-gating.md.
  // Negative pattern (if (!feature(...)) return) does not eliminate
  // inline string literals from external builds.
  return feature('BRIDGE_MODE')
    ? isClaudeAISubscriber() &&
        getFeatureValue_CACHED_MAY_BE_STALE('tengu_ccr_bridge', false)
    : false
}
```

When written as `if (!feature('BRIDGE_MODE')) return false; return getFeatureValue_...('tengu_ccr_bridge', false)`, Bun's DCE pass fails to eliminate the trailing string literal `'tengu_ccr_bridge'` from the bundle. Wrapping the entire body inside a positive `feature('...') ? ... : false` (or `if (feature('...')) { ... }`) is mandatory whenever a gated branch holds internal strings or `require()` calls.

**Scar 2 — Bun's Per-Function DCE Complexity Cliff.** In `tools/BashTool/bashPermissions.ts` and `tools/BashTool/pathValidation.ts`, a large permission function hit an internal complexity ceiling inside Bun's constant folder:

```ts
// tools/BashTool/bashPermissions.ts:81-89
// DCE cliff: Bun's feature() evaluator has a per-function complexity budget.
// bashToolHasPermission is right at the limit. `import { X as Y }` aliases
// inside the import block count toward this budget; when they push it over
// the threshold Bun can no longer prove feature('BASH_CLASSIFIER') is a
// constant and silently evaluates the ternaries to `false`, dropping every
// pendingClassifierCheck spread. Keep aliases as top-level const rebindings
// instead. (See also the comment on checkSemanticsDeny below.)
const bashCommandIsSafeAsync = bashCommandIsSafeAsync_DEPRECATED
const splitCommand = splitCommand_DEPRECATED
```

Worse still, `tools/BashTool/pathValidation.ts:1155-1163` records that an obsolete 80-line dead function in `bashPermissions.ts` **cannot be deleted** because removing it shifted the module across the complexity threshold and caused 8 of 30 classifier tests to silently fail (`feature('BASH_CLASSIFIER')` folded to `false`).

**Scar 3 — Zero-Allocation Native Attestation & Static Libc.** Not every `feature()` flag gates an experimental product feature. Two target the compiled binary runtime itself:
- `NATIVE_CLIENT_ATTESTATION` (`constants/system.ts:64-83`) embeds a fixed-width `cch=00000;` placeholder in the attribution header string so Anthropic's custom Bun HTTP stack (`bun-anthropic/src/http/Attestation.zig`) can overwrite the five zeros in-place with a computed attestation hash without altering `Content-Length` or reallocating the request buffer.
- `IS_LIBC_MUSL` / `IS_LIBC_GLIBC` (`utils/envDynamic.ts:52-59`) short-circuit Linux libc detection at compile time in native binaries, falling back to an async fire-and-forget `stat('/lib/libc.musl-...')` check only in unbundled Node runs.

> [!NOTE]
> **Snapshot Absences Behind `feature()` Gates:** Because external builds dead-code-eliminate ungated modules, **31 of the 90 flags** guard `require(...)` or `import(...)` targets that are **absent from this source snapshot** (only their call sites survive). Those flags are marked with `†` in the tables below; see Appendix B for call-site reconstruction of key missing modules.

---

## 2. Complete Inventory of All 90 Build-Time `feature()` Flags

Counts below report **executable code call sites** across all `.ts` and `.tsx` files (`933` total; raw regex hits including 39 documentation comments equal `972`). Every single unique flag in the repository appears exactly once across Tables 2.1–2.6 (`14 + 15 + 12 + 11 + 22 + 16 = 90`). Flags marked `†` reference at least one snapshot-absent module.

### 2.1 Agent, Swarm, Proactive & Autonomous Modes (14 flags, 332 sites)

| Flag | Sites (Files) | Primary Source Files | Role & Companion Chapter |
| :--- | :---: | :--- | :--- |
| `KAIROS` † | 147 (54) | `main.tsx`, `screens/REPL.tsx`, `bridge/bridgeMain.ts` | Always-on assistant mode: daily memory logs, assistant daemon bridge, push/file tools ([Ch 10](ch010-memory-systems-claudemd-auto-memory-and-dream.md), [Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `KAIROS_BRIEF` | 39 (20) | `main.tsx`, `constants/prompts.ts`, `tools/BriefTool/BriefTool.ts` | Concise "brief" output mode (`SendUserMessage` / `ctrl+shift+b`) for async updates ([Ch 04](ch004-prompt-engineering-and-cache-boundaries.md), [Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `PROACTIVE` † | 37 (15) | `screens/REPL.tsx`, `cli/print.ts`, `constants/prompts.ts` | Autonomous tick/wake loop (`--proactive`, `SleepTool`, periodic wakeups) ([Ch 17](ch017-background-task-runtime.md)) |
| `COORDINATOR_MODE` † | 32 (15) | `cli/print.ts`, `main.tsx`, `coordinator/coordinatorMode.ts` | Multi-agent orchestrator mode stripping direct edit tools from leader ([Ch 19](ch019-agent-teams-swarms-and-coordinator-mode.md)) |
| `UDS_INBOX` † | 18 (10) | `tools/SendMessageTool/SendMessageTool.ts`, `main.tsx`, `setup.ts` | Unix domain socket cross-session peer inbox (`ListPeersTool`, `/peers`) ([Ch 19](ch019-agent-teams-swarms-and-coordinator-mode.md)) |
| `BG_SESSIONS` † | 11 (7) | `utils/concurrentSessions.ts`, `query.ts`, `screens/REPL.tsx` | Background tmux-backed CLI sessions (`claude --bg`, detach-on-exit) ([Ch 17](ch017-background-task-runtime.md), [Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `AGENT_TRIGGERS` | 10 (6) | `cli/print.ts`, `screens/REPL.tsx`, `tools/ScheduleCronTool/prompt.ts` | Local cron trigger tools (`CronCreate`, `CronDelete`, `CronList`) ([Ch 17](ch017-background-task-runtime.md)) |
| `ULTRAPLAN` | 10 (5) | `screens/REPL.tsx`, `components/PromptInput/PromptInput.tsx`, `commands/ultraplan.tsx` | Deep remote planning workflow triggered via `/ultraplan` ([Ch 11](ch011-the-permission-engine.md), [Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `WORKFLOW_SCRIPTS` † | 10 (7) | `commands.ts`, `tasks.ts`, `constants/tools.ts` | Scriptable multi-step workflow engine (`WorkflowTool`, `local_workflow` tasks) ([Ch 17](ch017-background-task-runtime.md)) |
| `TEMPLATES` † | 6 (5) | `query/stopHooks.ts`, `query.ts`, `utils/markdownConfigLoader.ts` | Template-driven background job definitions (`templates/` config dir) ([Ch 07](ch007-the-heartbeat-query-loop.md), [Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `FORK_SUBAGENT` † | 5 (5) | `tools/AgentTool/forkSubagent.ts`, `commands.ts`, `commands/branch/index.ts` | Cache-sharing forked subagents and `/fork` conversation branching ([Ch 18](ch018-subagents-context-forking-and-worktrees.md)) |
| `VERIFICATION_AGENT` | 4 (4) | `constants/prompts.ts`, `tools/AgentTool/builtInAgents.ts`, `tools/TaskUpdateTool/TaskUpdateTool.ts` | Ant-only A/B verification agent contract (`tengu_hive_evidence`) ([Ch 18](ch018-subagents-context-forking-and-worktrees.md)) |
| `AGENT_TRIGGERS_REMOTE` | 2 (2) | `tools.ts`, `skills/bundled/index.ts` | Remote scheduled agent trigger tools & bundled skill ([Ch 16](ch016-skills-plugins-output-styles-and-lsp.md), [Ch 17](ch017-background-task-runtime.md)) |
| `BUILTIN_EXPLORE_PLAN_AGENTS` | 1 (1) | `tools/AgentTool/builtInAgents.ts` | A/B gate controlling whether built-in `Explore` and `Plan` agents are registered ([Ch 18](ch018-subagents-context-forking-and-worktrees.md)) |

### 2.2 Compaction, Context & Memory (15 flags, 142 sites)

| Flag | Sites (Files) | Primary Source Files | Role & Companion Chapter |
| :--- | :---: | :--- | :--- |
| `TEAMMEM` | 46 (15) | `utils/collapseReadSearch.ts`, `utils/claudemd.ts`, `services/teamMemorySync/watcher.ts` | Shared team memory directory, secret guard, and background sync watcher ([Ch 10](ch010-memory-systems-claudemd-auto-memory-and-dream.md)) |
| `CONTEXT_COLLAPSE` † | 22 (13) | `query.ts`, `services/compact/autoCompact.ts`, `setup.ts` | Stage 4 compaction: collapsible history spans and `CtxInspectTool` ([Ch 09](ch009-five-layer-compaction-pipeline.md)) |
| `HISTORY_SNIP` † | 16 (8) | `utils/messages.ts`, `QueryEngine.ts`, `query.ts` | Stage 2 compaction: surgical history snipping (`SnipTool`, `/force-snip`) ([Ch 09](ch009-five-layer-compaction-pipeline.md)) |
| `CACHED_MICROCOMPACT` † | 11 (5) | `services/api/claude.ts`, `query.ts`, `services/compact/microCompact.ts` | Server-side cache-editing microcompact (`cache_edits` API blocks) ([Ch 09](ch009-five-layer-compaction-pipeline.md)) |
| `PROMPT_CACHE_BREAK_DETECTION` | 9 (6) | `services/api/claude.ts`, `services/compact/microCompact.ts`, `services/api/promptCacheBreakDetection.ts` | Diffs system prompt/tools/messages between turns to diagnose cache misses ([Ch 04](ch004-prompt-engineering-and-cache-boundaries.md)) |
| `TOKEN_BUDGET` | 9 (6) | `screens/REPL.tsx`, `query.ts`, `constants/prompts.ts` | Multi-turn token-budget tracking and continuation nudges ([Ch 07](ch007-the-heartbeat-query-loop.md)) |
| `CONNECTOR_TEXT` | 8 (5) | `services/api/claude.ts`, `utils/messages.ts`, `constants/betas.ts` | `summarize-connector-text-2026-03-13` beta for compacting inter-tool prose ([Ch 05](ch005-talking-to-the-model.md)) |
| `EXTRACT_MEMORIES` | 6 (3) | `query/stopHooks.ts`, `cli/print.ts`, `utils/backgroundHousekeeping.ts` | End-of-turn background memory extraction agent ([Ch 10](ch010-memory-systems-claudemd-auto-memory-and-dream.md)) |
| `REACTIVE_COMPACT` † | 5 (5) | `query.ts`, `services/compact/autoCompact.ts`, `commands/compact/compact.ts` | On-demand compaction triggered reactively on `prompt_too_long` (`tengu_cobalt_raccoon`) ([Ch 07](ch007-the-heartbeat-query-loop.md), [Ch 09](ch009-five-layer-compaction-pipeline.md)) |
| `MEMORY_SHAPE_TELEMETRY` † | 3 (2) | `utils/sessionFileAccessHooks.ts`, `memdir/findRelevantMemories.ts` | Logs recall shape telemetry for memory directory queries ([Ch 10](ch010-memory-systems-claudemd-auto-memory-and-dream.md)) |
| `AGENT_MEMORY_SNAPSHOT` | 2 (2) | `main.tsx`, `tools/AgentTool/loadAgentsDir.ts` | Syncs custom agent memory snapshots on startup ([Ch 18](ch018-subagents-context-forking-and-worktrees.md)) |
| `BREAK_CACHE_COMMAND` | 2 (1) | `context.ts` | Dev-only `/break-cache` injection into dynamic system prompt section ([Ch 04](ch004-prompt-engineering-and-cache-boundaries.md)) |
| `COMPACTION_REMINDERS` | 1 (1) | `utils/attachments.ts` | Injects `compaction_reminder` system attachment as context fills ([Ch 09](ch009-five-layer-compaction-pipeline.md)) |
| `KAIROS_DREAM` † | 1 (1) | `skills/bundled/index.ts` | Registers bundled `/dream` cross-session memory consolidation skill ([Ch 10](ch010-memory-systems-claudemd-auto-memory-and-dream.md)) |
| `ULTRATHINK` | 1 (1) | `utils/thinking.ts` | Enables `ultrathink` keyword boost (`tengu_turtle_carbon`) ([Ch 05](ch005-talking-to-the-model.md)) |

### 2.3 Tools, Shell, Permissions & Sandbox (12 flags, 195 sites)

| Flag | Sites (Files) | Primary Source Files | Role & Companion Chapter |
| :--- | :---: | :--- | :--- |
| `TRANSCRIPT_CLASSIFIER` † | 108 (44) | `utils/permissions/permissionSetup.ts`, `utils/permissions/yoloClassifier.ts`, `main.tsx` | Auto-mode LLM safety classifier over full conversation transcripts ([Ch 11](ch011-the-permission-engine.md)) |
| `BASH_CLASSIFIER` | 45 (19) | `tools/BashTool/bashPermissions.ts`, `hooks/useCanUseTool.tsx`, `utils/classifierApprovals.ts` | Per-command Bash ML/LLM classifier for permission auto-approval ([Ch 11](ch011-the-permission-engine.md), [Ch 12](ch012-shell-execution-sandboxing-and-workspace-tools.md)) |
| `MONITOR_TOOL` † | 13 (9) | `tools/BashTool/prompt.ts`, `tasks/LocalShellTask/LocalShellTask.tsx`, `tasks.ts` | Streaming shell/MCP monitor tool (`MonitorTool`, `monitor_mcp` task) ([Ch 17](ch017-background-task-runtime.md)) |
| `TREE_SITTER_BASH_SHADOW` | 5 (2) | `tools/BashTool/bashPermissions.ts`, `utils/bash/parser.ts` | Shadow-mode tree-sitter Bash AST parser comparing against legacy regex parser (`tengu_birch_trellis`) ([Ch 12](ch012-shell-execution-sandboxing-and-workspace-tools.md)) |
| `TERMINAL_PANEL` † | 5 (5) | `tools.ts`, `keybindings/defaultBindings.ts`, `hooks/useGlobalKeybindings.tsx` | Embedded terminal capture tool (`TerminalCaptureTool`) and `meta+j` toggle ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `REVIEW_ARTIFACT` † | 4 (2) | `components/permissions/PermissionRequest.tsx`, `skills/bundled/index.ts` | Code-review artifact tool (`ReviewArtifactTool`) and `hunter` skill ([Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `WEB_BROWSER_TOOL` † | 4 (3) | `screens/REPL.tsx`, `tools.ts`, `main.tsx` | Native Bun `WebView`-backed embedded browser tool ([Ch 02](ch002-the-tool-contract.md)) |
| `TREE_SITTER_BASH` | 3 (1) | `utils/bash/parser.ts` | Primary tree-sitter Bash AST parser activation ([Ch 12](ch012-shell-execution-sandboxing-and-workspace-tools.md)) |
| `FILE_PERSISTENCE` | 3 (2) | `cli/print.ts`, `utils/filePersistence/filePersistence.ts` | Uploads modified output files to Files API at end of each turn in remote/BYOC sessions ([Ch 23](ch023-machine-readable-modes-headless-sdk-and-control-protocols.md)) |
| `OVERFLOW_TEST_TOOL` † | 2 (2) | `tools.ts`, `utils/permissions/classifierDecision.ts` | Synthetic context-overflow test tool for compaction/classifier testing ([Ch 02](ch002-the-tool-contract.md)) |
| `POWERSHELL_AUTO_MODE` | 2 (2) | `utils/permissions/yoloClassifier.ts`, `utils/permissions/permissions.ts` | Extends auto-mode classifier support to `PowerShellTool` ([Ch 11](ch011-the-permission-engine.md), [Ch 12](ch012-shell-execution-sandboxing-and-workspace-tools.md)) |
| `HOOK_PROMPTS` | 1 (1) | `screens/REPL.tsx` | Exposes interactive `requestPrompt` callback from REPL to hook engine ([Ch 13](ch013-the-hook-system.md)) |

### 2.4 MCP, Skills, Plugins & Commands (11 flags, 72 sites)

| Flag | Sites (Files) | Primary Source Files | Role & Companion Chapter |
| :--- | :---: | :--- | :--- |
| `EXPERIMENTAL_SKILL_SEARCH` † | 19 (9) | `tools/SkillTool/SkillTool.ts`, `constants/prompts.ts`, `utils/attachments.ts` | Semantic local/remote skill search and prefetch pipeline ([Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `KAIROS_CHANNELS` | 18 (13) | `services/mcp/useManageMCPConnections.ts`, `cli/print.ts`, `services/mcp/channelNotification.ts` | Inbound MCP channel notifications & push messages (`tengu_harbor`) ([Ch 15](ch015-model-context-protocol-mcp.md)) |
| `CHICAGO_MCP` | 13 (7) | `services/mcp/client.ts`, `query.ts`, `services/mcp/config.ts` | Built-in desktop computer-use MCP server integration ([Ch 15](ch015-model-context-protocol-mcp.md)) |
| `MCP_SKILLS` † | 9 (3) | `services/mcp/client.ts`, `services/mcp/useManageMCPConnections.ts`, `commands.ts` | Exposes MCP server prompts as discoverable skills ([Ch 15](ch015-model-context-protocol-mcp.md), [Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `KAIROS_GITHUB_WEBHOOKS` † | 4 (4) | `tools.ts`, `hooks/useReplBridge.tsx`, `commands.ts` | PR subscription tool (`/subscribe-pr`, `SubscribePRTool`) and webhook sanitizer ([Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `MCP_RICH_OUTPUT` | 3 (1) | `tools/MCPTool/UI.tsx` | Truncated rich parameter rendering in MCP tool UI blocks ([Ch 15](ch015-model-context-protocol-mcp.md)) |
| `NEW_INIT` | 2 (1) | `commands/init.ts` | Enhanced `/init` scaffolding `CLAUDE.md`, skills, and hooks (`CLAUDE_CODE_NEW_INIT`) ([Ch 10](ch010-memory-systems-claudemd-auto-memory-and-dream.md)) |
| `BUILDING_CLAUDE_APPS` | 1 (1) | `skills/bundled/index.ts` | Registers bundled `claudeApi` skill for building apps with the Claude API ([Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `RUN_SKILL_GENERATOR` † | 1 (1) | `skills/bundled/index.ts` | Registers bundled skill-generator meta-skill ([Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `SKILL_IMPROVEMENT` | 1 (1) | `utils/hooks/skillImprovement.ts` | Post-sampling hook updating skills from execution traces (`tengu_copper_panda`) ([Ch 13](ch013-the-hook-system.md), [Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `TORCH` † | 1 (1) | `commands.ts` | Registers internal `/torch` slash command (module absent from snapshot) ([Ch 03](ch003-command-and-message-contracts.md)) |

### 2.5 UI, Voice, Bridge, Remote & Daemon (22 flags, 153 sites)

| Flag | Sites (Files) | Primary Source Files | Role & Companion Chapter |
| :--- | :---: | :--- | :--- |
| `VOICE_MODE` † | 47 (15) | `hooks/useVoiceIntegration.tsx`, `voice/voiceModeEnabled.ts`, `screens/REPL.tsx` | Push-to-talk speech-to-text input (`space` hold-to-talk, streaming STT) ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `BRIDGE_MODE` | 27 (13) | `hooks/useReplBridge.tsx`, `bridge/bridgeEnabled.ts`, `main.tsx` | Remote Control REPL bridge connecting local CLI to `claude.ai` (`tengu_ccr_bridge`) ([Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `BUDDY` † | 18 (7) | `screens/REPL.tsx`, `buddy/CompanionSprite.tsx`, `components/PromptInput/PromptInput.tsx` | Terminal ASCII companion sprite and `/buddy` reactions ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `LODESTONE` | 6 (4) | `main.tsx`, `utils/backgroundHousekeeping.ts`, `interactiveHelpers.tsx` | Deep-link URI protocol registration and `--handle-uri` launcher ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `DIRECT_CONNECT` | 5 (1) | `main.tsx` | `--connect` direct WebSocket/HTTP server session attachment ([Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `DOWNLOAD_USER_SETTINGS` | 5 (3) | `cli/print.ts`, `services/settingsSync/index.ts`, `commands/reload-plugins/reload-plugins.ts` | Pulls cloud-synced user settings into remote/headless sessions ([Ch 14](ch014-configuration-settings-and-enterprise-policy.md)) |
| `MESSAGE_ACTIONS` | 5 (2) | `screens/REPL.tsx`, `keybindings/defaultBindings.ts` | `shift+up` message action menu (copy/rewind/branch on prior turns) ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `QUICK_SEARCH` | 5 (2) | `components/PromptInput/PromptInput.tsx`, `keybindings/defaultBindings.ts` | `ctrl+shift+f` / `cmd+shift+f` global transcript search overlay ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `CCR_MIRROR` | 4 (3) | `bridge/remoteBridgeCore.ts`, `bridge/bridgeEnabled.ts`, `main.tsx` | Read-only mirror of local CLI turns to `claude.ai` without full remote control ([Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `HISTORY_PICKER` | 4 (2) | `components/PromptInput/PromptInput.tsx`, `hooks/useHistorySearch.ts` | Interactive scrollable prompt history picker modal ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `KAIROS_PUSH_NOTIFICATION` | 4 (3) | `components/Settings/Config.tsx`, `tools.ts`, `tools/ConfigTool/supportedSettings.ts` | Mobile/desktop push notifications on task completion (`PushNotificationTool`) ([Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `SSH_REMOTE` | 4 (1) | `main.tsx` | `claude ssh` remote execution launcher with forwarded auth socket ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `AUTO_THEME` † | 3 (3) | `components/ThemePicker.tsx`, `components/design-system/ThemeProvider.tsx`, `tools/ConfigTool/supportedSettings.ts` | OS dark/light automatic theme switching (`systemThemeWatcher`) ([Ch 20](ch020-inside-ink-terminal-react-renderer.md)) |
| `CCR_AUTO_CONNECT` | 3 (2) | `utils/config.ts`, `bridge/bridgeEnabled.ts` | Automatic remote-control bridge reconnection (`tengu_cobalt_harbor`) ([Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `DAEMON` † | 3 (2) | `entrypoints/cli.tsx`, `commands.ts` | Persistent background daemon supervisor (`claude daemon` / `/remote-control-server`) ([Ch 22](ch022-cold-start-mastery-cli-init-main.md), [Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `AWAY_SUMMARY` | 2 (2) | `hooks/useAwaySummary.ts`, `screens/REPL.tsx` | Generates catch-up summary after terminal loses focus for 5+ minutes ([Ch 20](ch020-inside-ink-terminal-react-renderer.md), [Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `NATIVE_CLIPBOARD_IMAGE` | 2 (1) | `utils/imagePaste.ts` | Native macOS `NSPasteboard` image read (~0.03ms) avoiding `osascript` spawn ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `UPLOAD_USER_SETTINGS` | 2 (2) | `services/settingsSync/index.ts`, `main.tsx` | Uploads local user settings to cloud sync in background ([Ch 14](ch014-configuration-settings-and-enterprise-policy.md)) |
| `BYOC_ENVIRONMENT_RUNNER` † | 1 (1) | `entrypoints/cli.tsx` | Fast-path `claude environment-runner` entrypoint for Bring-Your-Own-Cloud containers ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `CCR_REMOTE_SETUP` | 1 (1) | `commands.ts` | Registers `/web-setup` command for Claude Code on the web (`tengu_cobalt_lantern`) ([Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `SELF_HOSTED_RUNNER` † | 1 (1) | `entrypoints/cli.tsx` | Fast-path `claude self-hosted-runner` worker entrypoint ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `STREAMLINED_OUTPUT` | 1 (1) | `cli/print.ts` | Compact NDJSON transformer for `--output-format stream-json` (`CLAUDE_CODE_STREAMLINED_OUTPUT`) ([Ch 23](ch023-machine-readable-modes-headless-sdk-and-control-protocols.md)) |

### 2.6 Internal, Telemetry, Build & Release (16 flags, 39 sites)

| Flag | Sites (Files) | Primary Source Files | Role & Companion Chapter |
| :--- | :---: | :--- | :--- |
| `COMMIT_ATTRIBUTION` † | 12 (9) | `cli/print.ts`, `utils/sessionRestore.ts`, `setup.ts` | Tracks AI-authored commit attribution hooks & git trailers ([Ch 14](ch014-configuration-settings-and-enterprise-policy.md)) |
| `SHOT_STATS` | 10 (3) | `utils/stats.ts`, `utils/statsCache.ts`, `components/Stats.tsx` | Per-session prompt/shot distribution histograms in `/stats` ([Ch 05](ch005-talking-to-the-model.md)) |
| `ALLOW_TEST_VERSIONS` | 2 (1) | `utils/nativeInstaller/download.ts` | Allows installing `99.99.*` synthetic test release versions ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `COWORKER_TYPE_TELEMETRY` | 2 (1) | `services/analytics/metadata.ts` | Attaches `CLAUDE_CODE_COWORKER_TYPE` dimension to analytics events ([Ch 05](ch005-talking-to-the-model.md)) |
| `HARD_FAIL` | 2 (2) | `main.tsx`, `utils/log.ts` | Adds hidden `--hard-fail` CLI flag that crashes immediately on any `logError()` ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `ABLATION_BASELINE` | 1 (1) | `entrypoints/cli.tsx` | Harness-science L0 ablation preset setting 7 disable env vars before module imports ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `ANTI_DISTILLATION_CC` | 1 (1) | `services/api/claude.ts` | Sends `anti_distillation: ['fake_tools']` on 1P CLI requests (`tengu_anti_distill_fake_tool_injection`) ([Ch 05](ch005-talking-to-the-model.md)) |
| `DUMP_SYSTEM_PROMPT` | 1 (1) | `entrypoints/cli.tsx` | Zero-REPL `--dump-system-prompt` fast-path for prompt inspection ([Ch 04](ch004-prompt-engineering-and-cache-boundaries.md), [Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `ENHANCED_TELEMETRY_BETA` | 1 (1) | `utils/telemetry/sessionTracing.ts` | Enables detailed OpenTelemetry span tracing beta ([Ch 05](ch005-talking-to-the-model.md)) |
| `IS_LIBC_GLIBC` | 1 (1) | `utils/envDynamic.ts` | Compile-time assertion that target Linux binary uses glibc ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `IS_LIBC_MUSL` | 1 (1) | `utils/envDynamic.ts` | Compile-time assertion that target Linux binary uses musl ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `NATIVE_CLIENT_ATTESTATION` | 1 (1) | `constants/system.ts` | Inserts `cch=00000;` in attribution header for Zig HTTP stack attestation ([Ch 05](ch005-talking-to-the-model.md)) |
| `PERFETTO_TRACING` | 1 (1) | `utils/telemetry/perfettoTracing.ts` | Enables Perfetto protobuf performance trace writer (`CLAUDE_CODE_PERFETTO_TRACE`) ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `SKIP_DETECTION_WHEN_AUTOUPDATES_DISABLED` | 1 (1) | `components/AutoUpdaterWrapper.tsx` | Skips update version checks completely when auto-updates are disabled ([Ch 14](ch014-configuration-settings-and-enterprise-policy.md)) |
| `SLOW_OPERATION_LOGGING` | 1 (1) | `utils/slowOperations.ts` | Swaps in internal slow-JSON/op logging thresholds (`slowLoggingAnt` vs. `slowLoggingExternal`) ([Ch 01](ch001-atoms-types-stores-and-primitives.md)) |
| `UNATTENDED_RETRY` | 1 (1) | `services/api/withRetry.ts` | Allows infinite/extended retries for headless evals (`CLAUDE_CODE_UNATTENDED_RETRY`) ([Ch 05](ch005-talking-to-the-model.md)) |

---

## 3. The `USER_TYPE === 'ant'` Split: Constant Folding Across `.tsx` vs. `.ts`

Alongside `feature()`, the codebase contains a second compile-time axis distinguishing Anthropic internal ("ant") builds from public releases (`"external"`). Inspecting this snapshot reveals an immediate, striking asymmetry between `.ts` and `.tsx` files:

| Pattern | `.ts` Files (1,332 total) | `.tsx` Files (552 total) | Notes |
| :--- | :---: | :---: | :--- |
| `process.env.USER_TYPE` | **357** hits (165 files) | **0** executable hits (3 comments) | Top `.ts` sites: `firstPartyEventLoggingExporter.ts` (19), `growthbook.ts` (17), `insights.ts` (10), `mockRateLimits.ts` (9), `prompts.ts` (12) |
| `"external" === 'ant'` | **0** hits | **88** hits (27 files) | Constant-folded by build define prior to React Compiler output |
| `"external" !== 'ant'` | **0** hits | **7** hits (7 files) | Combined with `===`, **95** folded checks across **31** unique `.tsx` files |
| Inline `sourceMappingURL=` | **0** / 1,332 files | **552** / 552 files | Every `.tsx` file passed through the React Compiler + define transform |

Why does `"external" === 'ant'` appear exclusively in `.tsx` files? Because in the build pipeline that produced this snapshot, `.tsx` files were run through the React Compiler transform (`_c(...)` memoization slots + inline base64 `sourceMappingURL`) with the bundler define `process.env.USER_TYPE = "external"` already substituted—while `.ts` files bypassed the JSX/React Compiler pass and preserved their raw `process.env.USER_TYPE === 'ant'` expressions.

You can verify that `USER_TYPE` is structurally guaranteed to be a build-time constant (never toggled at runtime) from `components/MemoryUsageIndicator.tsx`:

```tsx
// components/MemoryUsageIndicator.tsx:5-16
export function MemoryUsageIndicator(): React.ReactNode {
  // Ant-only: the /heapdump link is an internal debugging aid. Gating before
  // the hook means the 10s polling interval is never set up in external builds.
  // USER_TYPE is a build-time constant, so the hook call below is either always
  // reached or dead-code-eliminated — never conditional at runtime.
  if ("external" !== 'ant') {
    return null;
  }

  // eslint-disable-next-line react-hooks/rules-of-hooks
  // biome-ignore lint/correctness/useHookAtTopLevel: USER_TYPE is a build-time constant
  const memoryUsage = useMemoryUsage();
```

Placing a React hook after an `if` return would violate the Rules of Hooks if `USER_TYPE` could ever vary dynamically; it is safe *only* because the bundler folds `"external" !== 'ant'` to `true` and dead-code-eliminates `useMemoryUsage()` in external builds (and folds `'ant' !== 'ant'` to `false` in internal builds).

> [!IMPORTANT]
> **`feature()` vs. `USER_TYPE === 'ant'`:** Use `USER_TYPE === 'ant'` for behavior strictly scoped to Anthropic employees (internal telemetry exporters, `/heapdump`, `isUndercover()`, ant-only model overrides, `mockRateLimits.ts`). Use `feature('...')` for capabilities that ship in *some* external builds or need independent build-matrix ablation—as `main.tsx:2662` explicitly notes: *"Gated on `TRANSCRIPT_CLASSIFIER` (not `USER_TYPE`) so GrowthBook kill switch runs for external builds too."*

---

## 4. Environment Variable Architecture: Trust Boundaries, Filters, and Subprocess Scrubbing

The codebase reads **472 distinct `process.env.*` keys** (`196` `CLAUDE_CODE_*`, `31` other `CLAUDE_*`, `26` `ANTHROPIC_*`, `21` `DISABLE_*`, `16` `OTEL_*`, `12` `ENABLE_*`, `8` `MCP_*`, `7` `USE_*`, plus standard OS/cloud/proxy variables). Before consulting the tables in Section 5, four load-bearing invariants govern how every variable enters, overrides, and exits `process.env`.

### Invariant 1: Strict Tri-State Boolean Parsing

In `utils/envUtils.ts:32-47`, boolean knobs are never parsed with bare `Boolean(process.env.X)` (which would treat `"0"` and `"false"` as truthy):

```ts
// utils/envUtils.ts:32-47
export function isEnvTruthy(envVar: string | boolean | undefined): boolean {
  if (!envVar) return false
  if (typeof envVar === 'boolean') return envVar
  const normalizedValue = envVar.toLowerCase().trim()
  return ['1', 'true', 'yes', 'on'].includes(normalizedValue)
}

export function isEnvDefinedFalsy(
  envVar: string | boolean | undefined,
): boolean {
  if (envVar === undefined) return false
  if (typeof envVar === 'boolean') return !envVar
  if (!envVar) return false
  const normalizedValue = envVar.toLowerCase().trim()
  return ['0', 'false', 'no', 'off'].includes(normalizedValue)
}
```

Notice the subtle tri-state boundary: an empty string `""` returns `false` for **both** `isEnvTruthy("")` and `isEnvDefinedFalsy("")`, allowing unset/empty values to fall back to defaults (e.g., `CLAUDE_CODE_NO_FLICKER=0` explicitly opts out an ant user, while empty falls through to the build default in `utils/fullscreen.ts:108-125`).

### Invariant 2: Pre-Trust `SAFE_ENV_VARS` Allowlist

Because `settings.json` supports an `"env": { ... }` map, a malicious Git repository could commit `.claude/settings.json` with `ANTHROPIC_BASE_URL` pointing to an attacker's proxy, `NODE_EXTRA_CA_CERTS` trusting an attacker's CA, or `HTTP_PROXY` hijacking first-run auth traffic *before* the user answers the workspace trust dialog.

`utils/managedEnv.ts:93-199` solves this with a two-phase loader wired into `init.ts` (see [Chapter 14](ch014-configuration-settings-and-enterprise-policy.md) and [Chapter 22](ch022-cold-start-mastery-cli-init-main.md)):

```text
  Phase 1: applySafeConfigEnvironmentVariables()   [BEFORE trust dialog]
    - Trusted sources (userSettings, flagSettings, policySettings): ALL vars
    - Untrusted sources (projectSettings, localSettings): ONLY SAFE_ENV_VARS
      (blocked: ANTHROPIC_BASE_URL, HTTP(S)_PROXY, NODE_EXTRA_CA_CERTS,
       NODE_TLS_REJECT_UNAUTHORIZED, ANTHROPIC_API_KEY, LD_PRELOAD, PATH)
                              |
                              v  (user grants workspace trust)
  Phase 2: applyConfigEnvironmentVariables()       [AFTER trust dialog]
    - Merges all settings.env keys -> clears CA, mTLS, & proxy caches
    - Rebuilds global HTTP/HTTPS agents via configureGlobalAgents()
```

### Invariant 3: Three Host-Protection Strip Filters

Every settings-sourced `env` object passes through `filterSettingsEnv()` (`utils/managedEnv.ts:85-91`), composing three strip guards:

1. **`withoutSSHTunnelVars()`**: When `ANTHROPIC_UNIX_SOCKET` is set (`claude ssh`), strips `ANTHROPIC_UNIX_SOCKET`, `ANTHROPIC_BASE_URL`, `ANTHROPIC_API_KEY`, `ANTHROPIC_AUTH_TOKEN`, and `CLAUDE_CODE_OAUTH_TOKEN` so the remote host's `~/.claude/settings.json` cannot clobber the forwarded local auth tunnel.
2. **`withoutHostManagedProviderVars()`**: When `CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST` is truthy, strips all `PROVIDER_MANAGED_ENV_VARS` plus any `VERTEX_REGION_CLAUDE_*` prefix (`utils/managedEnvConstants.ts:14-70`) so a user's personal Bedrock/Vertex settings cannot hijack an enterprise or desktop host's inference routing.
3. **`withoutCcdSpawnEnvKeys()`**: When `CLAUDE_CODE_ENTRYPOINT === 'claude-desktop'`, snapshots all spawn-time env keys on first call and strips them from settings so user settings (e.g., `OTEL_LOGS_EXPORTER=console`) cannot corrupt the desktop host's stdio JSON-RPC stream.

### Invariant 4: Child-Process Secret Scrubbing (`subprocessEnv()`)

When running in untrusted automation (such as GitHub Actions with `allowed_non_write_users`), prompt injection inside a Bash tool could execute `echo $ANTHROPIC_API_KEY`. `utils/subprocessEnv.ts:15-99` wraps every child spawn (Bash, shell snapshots, MCP stdio, LSP, shell hooks): when `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` is truthy, it deletes 23 secret keys (`GHA_SUBPROCESS_SCRUB`: Anthropic auth, OTLP headers, AWS/GCP/Azure credentials, GitHub OIDC & runtime tokens, `SSH_SIGNING_KEY`, `ALL_INPUTS`) **plus their `INPUT_<KEY>` GitHub Actions duplicates**, while keeping them intact inside the parent Claude Code process and merging CCR's `_getUpstreamProxyEnv()` relay vars.

---

## 5. Load-Bearing Environment Knobs by Subsystem

Every variable name and primary file path below is verified directly against this snapshot.

### 5.1 Startup, Paths, Isolation & Diagnostics

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `CLAUDE_CONFIG_DIR` | `utils/envUtils.ts`, `utils/env.ts` | Overrides `~/.claude` config root (memoized & NFC-normalized; keyed on var) |
| `CLAUDE_CODE_SIMPLE` | `utils/envUtils.ts`, `main.tsx` | `--bare` mode: skips hooks, LSP, plugin sync, skill walk, and all keychain reads (~30 gates) |
| `CLAUDE_CODE_ENTRYPOINT` | `main.tsx`, `services/analytics/metadata.ts` | Identifies launch surface (`cli`, `claude-desktop`, `sdk-ts`, etc.); activates CCD env snapshot & attestation |
| `CLAUDE_CODE_ABLATION_BASELINE` | `entrypoints/cli.tsx` | Behind `feature('ABLATION_BASELINE')`: forces 7 disable env vars at line 21 before module load |
| `CLAUDE_CODE_PROFILE_STARTUP` | `utils/startupProfiler.ts`, `utils/headlessProfiler.ts` | Emits cold-start checkpoint timing breakdown ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `CLAUDE_CODE_PROFILE_QUERY` | `utils/queryProfiler.ts` | Profiles per-turn query loop latency stages |
| `CLAUDE_CODE_EXIT_AFTER_FIRST_RENDER` | `cli/print.ts`, `hooks/useAfterFirstRender.ts` | Exits immediately after initial Ink frame for startup benchmarking |
| `CLAUDE_CODE_DEBUG_LOGS_DIR` | `utils/debug.ts` | Redirects debug log files from default temp/config directory |
| `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` | `utils/privacyLevel.ts` | Sets privacy level `essential-traffic`: disables telemetry, auto-updater, error reporting, and marketplace calls |
| `DISABLE_TELEMETRY` | `utils/privacyLevel.ts` | Sets privacy level `no-telemetry` (keeps non-telemetry network calls active) |
| `DISABLE_AUTOUPDATER` | `utils/config.ts` | Disables background CLI auto-updates |
| `CLAUDE_CODE_ENABLE_TELEMETRY` | `utils/telemetry/instrumentation.ts` | Activates OpenTelemetry metrics/logs/traces export (`OTEL_EXPORTER_OTLP_*`, `OTEL_METRICS_EXPORTER`, `OTEL_LOGS_EXPORTER`) |
| `CLAUDE_CODE_OTEL_FLUSH_TIMEOUT_MS` | `utils/telemetry/instrumentation.ts` | Caps OTel exporter flush wait during graceful shutdown |
| `IS_DEMO` | `components/LogoV2/LogoV2.tsx`, `utils/status.tsx` | Hides sensitive account/email/org details in terminal banners for demos |
| `CLAUDE_CODE_TMPDIR` | `utils/imagePaste.ts`, `utils/permissions/filesystem.ts` | Overrides temp directory used by shell providers, clipboard paste, and scratch files |

### 5.2 Model, Inference Providers, Auth & Streaming

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `ANTHROPIC_API_KEY` | `utils/auth.ts`, `components/Settings/Config.tsx` | First-party API key (stripped from untrusted project settings and scrubbed subprocesses) |
| `ANTHROPIC_AUTH_TOKEN` | `utils/auth.ts`, `services/api/client.ts` | Custom `Authorization: Bearer` token for gateways/proxies |
| `ANTHROPIC_BASE_URL` | `services/api/client.ts`, `services/api/logging.ts` | Overrides first-party API origin (non-1P host auto-disables optimistic ToolSearch unless explicitly set) |
| `ANTHROPIC_MODEL` | `main.tsx`, `utils/model/model.ts` | Overrides primary loop model alias or full model ID |
| `ANTHROPIC_SMALL_FAST_MODEL` | `utils/model/model.ts`, `services/api/logging.ts` | Overrides auxiliary fast model (Haiku-tier helpers, summaries, prefetches) |
| `ANTHROPIC_DEFAULT_{OPUS,SONNET,HAIKU}_MODEL` | `utils/model/model.ts`, `utils/model/modelOptions.ts` | Remaps model tier aliases to provider-specific IDs (e.g., Bedrock ARNs or Vertex IDs) |
| `ANTHROPIC_BETAS` / `ANTHROPIC_CUSTOM_HEADERS` | `utils/betas.ts`, `services/api/client.ts` | Comma-separated extra beta headers / custom `Key: Value` HTTP headers injected on API calls |
| `CLAUDE_CODE_USE_{BEDROCK,VERTEX,FOUNDRY}` | `utils/auth.ts`, `services/api/client.ts` | Switches inference backend to AWS Bedrock, Google Vertex AI, or Azure Foundry ([Ch 05](ch005-talking-to-the-model.md)) |
| `CLAUDE_CODE_SKIP_{BEDROCK,VERTEX,FOUNDRY}_AUTH` | `services/api/client.ts`, `utils/model/bedrock.ts` | Bypasses cloud SDK SigV4/IAM/ADC auth for authenticating proxy gateways |
| `CLAUDE_CODE_OAUTH_TOKEN` | `utils/auth.ts`, `bridge/bridgeEnabled.ts` | Injects OAuth access token directly (note: lacks `user:profile` scope for some GrowthBook org gates) |
| `CLAUDE_CODE_API_KEY_HELPER_TTL_MS` | `utils/auth.ts` | Cache TTL for `apiKeyHelper` shell command output |
| `CLAUDE_CODE_MAX_OUTPUT_TOKENS` | `query.ts`, `services/api/claude.ts` | Overrides per-turn `max_tokens` ceiling sent to the Messages API |
| `MAX_THINKING_TOKENS` | `utils/thinking.ts`, `main.tsx` | Explicit extended-thinking token budget (`0` disables thinking) |
| `CLAUDE_CODE_EFFORT_LEVEL` | `utils/effort.ts`, `commands/effort/effort.tsx` | Sets reasoning effort (`low`/`medium`/`high`/`max`/`unset`); `CLAUDE_CODE_ALWAYS_ENABLE_EFFORT` forces on 3P |
| `CLAUDE_CODE_DISABLE_THINKING` / `DISABLE_INTERLEAVED_THINKING` | `services/api/claude.ts`, `utils/betas.ts` | Disables extended thinking or interleaved-thinking beta header |
| `CLAUDE_CODE_DISABLE_FAST_MODE` | `utils/fastMode.ts`, `query/config.ts` | Hard kill switch disabling fast-mode inference |
| `CLAUDE_CODE_DISABLE_EXPERIMENTAL_BETAS` | `utils/betas.ts`, `utils/toolSearch.ts` | Strips 1P experimental beta headers and `schema.strict` for third-party proxy compatibility |
| `CLAUDE_CODE_MAX_RETRIES` / `API_TIMEOUT_MS` | `services/api/withRetry.ts`, `services/api/client.ts` | Tunes transient HTTP retry count and per-request socket timeout |
| `CLAUDE_CODE_ATTRIBUTION_HEADER` | `constants/system.ts` | Set falsy (`0`/`false`) to omit `cc_version`/`cc_entrypoint` attribution header |
| `DISABLE_PROMPT_CACHING` | `services/api/claude.ts` | Disables `cache_control` breakpoints (also per-tier: `DISABLE_PROMPT_CACHING_{OPUS,SONNET,HAIKU}`) |

### 5.3 Compaction, Context Window & Memory

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `DISABLE_COMPACT` / `DISABLE_AUTO_COMPACT` | `services/compact/autoCompact.ts`, `commands/compact/index.ts` | Disables all compaction (incl. `/compact`) or disables automatic compaction only ([Ch 09](ch009-five-layer-compaction-pipeline.md)) |
| `CLAUDE_AUTOCOMPACT_PCT_OVERRIDE` | `services/compact/autoCompact.ts` | Triggers auto-compaction at custom percentage (`1`–`100`) of effective context window |
| `CLAUDE_CODE_AUTO_COMPACT_WINDOW` | `services/compact/autoCompact.ts` | Caps effective token window used when computing auto-compact thresholds |
| `CLAUDE_CODE_BLOCKING_LIMIT_OVERRIDE` | `services/compact/autoCompact.ts` | Overrides hard token ceiling where the loop refuses new turns |
| `ENABLE_CLAUDE_CODE_SM_COMPACT` / `DISABLE_CLAUDE_CODE_SM_COMPACT` | `services/compact/sessionMemoryCompact.ts` | Force-enables or disables session-memory-backed compaction (avoids full summarization call) |
| `USE_API_CLEAR_TOOL_RESULTS` / `USE_API_CLEAR_TOOL_USES` | `services/compact/apiMicrocompact.ts` | Switches Stage 3 microcompaction to server-side `context_management` tool-clearing API edits |
| `USE_API_CONTEXT_MANAGEMENT` | `utils/betas.ts` | Ant-only opt-in for server-side context management beta header |
| `CLAUDE_CODE_DISABLE_1M_CONTEXT` / `CLAUDE_CODE_MAX_CONTEXT_TOKENS` | `utils/context.ts` | Opts out of 1M-token context window or clamps effective context window size |
| `CLAUDE_CODE_DISABLE_CLAUDE_MDS` | `context.ts` | Suppresses loading hierarchical `CLAUDE.md` instruction files ([Ch 10](ch010-memory-systems-claudemd-auto-memory-and-dream.md)) |
| `CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD` | `utils/claudemd.ts` | Loads `CLAUDE.md` files from `--add-dir` extra directories (off by default) |
| `CLAUDE_CODE_DISABLE_AUTO_MEMORY` | `memdir/memdir.ts`, `memdir/paths.ts` | Disables persistent cross-session auto-memory directory (`memdir/`) |
| `CLAUDE_CODE_REMOTE_MEMORY_DIR` | `memdir/paths.ts`, `tools/AgentTool/agentMemory.ts` | Redirects auto-memory storage to a mounted remote/shared directory |
| `CLAUDE_CODE_DISABLE_FILE_CHECKPOINTING` / `CLAUDE_CODE_ENABLE_SDK_FILE_CHECKPOINTING` | `utils/fileHistory.ts` | Disables `/rewind` disk snapshots in interactive mode or enables them in headless SDK runs ([Ch 08](ch008-session-envelope-query-engine-and-transcripts.md)) |

### 5.4 Permissions, Shell, Sandboxing & Workspace Tools

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `IS_SANDBOX` | `setup.ts`, `services/api/withRetry.ts` | Declares execution inside an isolated container (permits `--dangerously-skip-permissions` as root) |
| `CLAUDE_CODE_BUBBLEWRAP` | `utils/envDynamic.ts`, `setup.ts` | Signals Linux `bwrap` sandbox environment ([Ch 12](ch012-shell-execution-sandboxing-and-workspace-tools.md)) |
| `CLAUDE_CODE_DISABLE_COMMAND_INJECTION_CHECK` | `tools/BashTool/bashPermissions.ts` | Disables Bash AST/regex command-injection validator (high-risk override) |
| `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB` | `utils/subprocessEnv.ts` | Strips `GHA_SUBPROCESS_SCRUB` + `INPUT_*` credentials from all child tool/hook/MCP/LSP processes |
| `CLAUDE_CODE_MANAGED_SETTINGS_PATH` | `utils/settings/managedPath.ts` | Overrides enterprise `managed-settings.json` location ([Ch 14](ch014-configuration-settings-and-enterprise-policy.md)) |
| `CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST` | `utils/managedEnv.ts`, `utils/managedEnvConstants.ts` | Locks inference provider/endpoint/model env vars against user settings overrides |
| `CLAUDE_CODE_SHELL` / `CLAUDE_CODE_SHELL_PREFIX` | `utils/Shell.ts`, `utils/shell/bashProvider.ts` | Forces specific shell binary or wraps every shell/MCP command in a prefix wrapper |
| `CLAUDE_BASH_MAINTAIN_PROJECT_WORKING_DIR` | `utils/envUtils.ts` | Resets shell `cwd` back to project root after every `BashTool` invocation |
| `BASH_DEFAULT_TIMEOUT_MS` / `BASH_MAX_TIMEOUT_MS` | `utils/timeouts.ts` | Sets default and upper-bound timeout for `BashTool` commands |
| `BASH_MAX_OUTPUT_LENGTH` | `utils/shell/outputLimits.ts` | Caps inline Bash stdout/stderr chars before truncation/spill |
| `CLAUDE_CODE_USE_POWERSHELL_TOOL` | `utils/shell/shellToolUtils.ts` | Enables native Windows `PowerShellTool` alongside/instead of Git Bash |
| `CLAUDE_CODE_MAX_TOOL_USE_CONCURRENCY` | `services/tools/toolOrchestration.ts` | Max parallel read-only tool calls per assistant turn (default `10`; [Ch 06](ch006-executing-tools.md)) |
| `CLAUDE_CODE_FILE_READ_MAX_OUTPUT_TOKENS` | `tools/FileReadTool/limits.ts` | Overrides per-call token ceiling for `FileReadTool` |
| `USE_BUILTIN_RIPGREP` | `utils/ripgrep.ts` | Forces bundled vs. system `rg` binary (`0` uses system PATH `rg`) |
| `CLAUDE_CODE_UNDERCOVER` | `utils/undercover.ts` | Suppresses internal/Anthropic attribution strings when working in public repos |

### 5.5 MCP, Skills, Hooks & Plugins

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `ENABLE_TOOL_SEARCH` | `utils/toolSearch.ts` | Controls deferred tool loading: `true`/`auto:0` (always defer), `auto`/`auto:N` (defer above `N%` context, default `10%`), `false`/`auto:100` (eager) ([Ch 15](ch015-model-context-protocol-mcp.md)) |
| `MCP_TIMEOUT` / `MCP_TOOL_TIMEOUT` | `services/mcp/client.ts` | Server startup handshake timeout and per-tool-call timeout for MCP servers |
| `MAX_MCP_OUTPUT_TOKENS` | `utils/mcpValidation.ts` | Token cap on inline MCP tool responses before warning/truncation |
| `ENABLE_MCP_LARGE_OUTPUT_FILES` | `services/mcp/client.ts` | Spills oversized MCP payloads to disk files rather than truncating |
| `ENABLE_CLAUDEAI_MCP_SERVERS` | `services/mcp/claudeai.ts` | Enables/disables synced `claude.ai` hosted MCP connectors |
| `ENABLE_LSP_TOOL` | `tools.ts` | Opts into the background Language Server Protocol tool ([Ch 16](ch016-skills-plugins-output-styles-and-lsp.md)) |
| `SLASH_COMMAND_TOOL_CHAR_BUDGET` | `tools/SkillTool/prompt.ts` | Character budget for listing available skills/commands in system prompt |
| `CLAUDE_CODE_SESSIONEND_HOOKS_TIMEOUT_MS` | `utils/hooks.ts`, `utils/gracefulShutdown.ts` | Caps shutdown wait for `SessionEnd` hooks ([Ch 13](ch013-the-hook-system.md)) |
| `CLAUDE_CODE_SYNC_PLUGIN_INSTALL` | `setup.ts`, `cli/print.ts` | Blocks startup until plugin installation completes (headless/CI determinism) |
| `CLAUDE_CODE_PLUGIN_CACHE_DIR` | `utils/plugins/pluginDirectories.ts` | Overrides on-disk plugin cache directory |
| `FORCE_AUTOUPDATE_PLUGINS` | `utils/config.ts` | Forces plugin marketplace updates even when CLI auto-updater is disabled |
| `CLAUDE_CODE_ENABLE_XAA` | `services/mcp/xaaIdpLogin.ts` | Enables Cross-App Access (XAA) IdP authentication flow for enterprise MCP servers |

### 5.6 Background Tasks, Subagents, Swarms & Coordinator

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `CLAUDE_CODE_DISABLE_BACKGROUND_TASKS` | `tools/BashTool/BashTool.tsx`, `tools/PowerShellTool/prompt.ts` | Disables `run_in_background` across shell and agent tools (captured at module import time!) |
| `CLAUDE_CODE_ENABLE_TASKS` / `CLAUDE_CODE_TASK_LIST_ID` | `utils/tasks.ts`, `main.tsx` | Enables persistent shared task list and pins a shared cross-session task list ID ([Ch 17](ch017-background-task-runtime.md)) |
| `CLAUDE_CODE_SUBAGENT_MODEL` | `utils/model/agent.ts` | Overrides default model used when spawning subagents ([Ch 18](ch018-subagents-context-forking-and-worktrees.md)) |
| `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS` | `utils/agentSwarmsEnabled.ts` | Opt-in gate enabling multi-agent teams (`TeamCreateTool`, mailbox, in-process/tmux teammates; [Ch 19](ch019-agent-teams-swarms-and-coordinator-mode.md)) |
| `CLAUDE_CODE_COORDINATOR_MODE` | `coordinator/coordinatorMode.ts`, `main.tsx` | Activates Coordinator Mode (behind `feature('COORDINATOR_MODE')`) |
| `CLAUDE_CODE_DISABLE_CRON` | `tools/ScheduleCronTool/prompt.ts` | Disables scheduled cron trigger tools for the session |
| `CLAUDE_CODE_PROACTIVE` / `CLAUDE_CODE_BRIEF` | `main.tsx`, `tools/BriefTool/BriefTool.ts` | Runtime opt-in for proactive wake loop / brief mode (behind `PROACTIVE` / `KAIROS_BRIEF`) |
| `CLAUDE_CODE_PLAN_V2_AGENT_COUNT` | `utils/planModeV2.ts` | Tunes parallel planner/explorer subagent fan-out in Plan Mode V2 |
| `CLAUDE_CODE_EAGER_FLUSH` | `QueryEngine.ts` | Forces immediate transcript persistence flushes inside `QueryEngine` ([Ch 08](ch008-session-envelope-query-engine-and-transcripts.md)) |

### 5.7 Terminal UI, Fullscreen, IDE & Accessibility

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `CLAUDE_CODE_NO_FLICKER` | `utils/fullscreen.ts` | Toggles Ink alternate-screen buffer + virtualized scrollback (`0` opts out; auto-disabled under `tmux -CC`; [Ch 20](ch020-inside-ink-terminal-react-renderer.md)) |
| `CLAUDE_CODE_DISABLE_MOUSE` / `CLAUDE_CODE_DISABLE_MOUSE_CLICKS` | `utils/fullscreen.ts` | Keeps alt-screen while disabling SGR mouse tracking (preserves native terminal copy-on-select) or click handlers |
| `CLAUDE_CODE_DISABLE_VIRTUAL_SCROLL` | `components/Messages.tsx`, `screens/REPL.tsx` | Disables message-list virtualization in fullscreen mode |
| `CLAUDE_CODE_DISABLE_TERMINAL_TITLE` | `screens/REPL.tsx`, `utils/gracefulShutdown.ts` | Prevents OSC escape sequences from overwriting terminal tab/window title |
| `CLAUDE_CODE_ACCESSIBILITY` | `ink/components/App.tsx`, `components/TextInput.tsx` | Enables screen-reader-friendly Ink rendering hints |
| `CLAUDE_CODE_SYNTAX_HIGHLIGHT` | `native-ts/color-diff/index.ts`, `components/ThemePicker.tsx` | Toggles native syntax highlighting in structured code diffs (`0` disables) |
| `CLAUDE_CODE_TMUX_PREFIX` | `components/LogoV2/LogoV2.tsx` | Surfaces tmux prefix conflict guidance when spawned inside managed tmux sessions |
| `CLAUDE_CODE_IDE_HOST_OVERRIDE` / `CLAUDE_CODE_SSE_PORT` | `utils/ide.ts`, `hooks/useIDEIntegration.tsx` | Connects CLI to VS Code / JetBrains IDE extension lockfile & SSE bridge |
| `CLAUDE_CODE_ENABLE_PROMPT_SUGGESTION` | `services/PromptSuggestion/promptSuggestion.ts`, `query/stopHooks.ts` | Enables speculative next-prompt ghost text after assistant turns ([Ch 21](ch021-the-interactive-repl-screens-keybindings-and-vim.md)) |
| `CLAUDE_CODE_DISABLE_FEEDBACK_SURVEY` | `components/FeedbackSurvey/useFeedbackSurvey.tsx` | Suppresses post-compact, memory, and session quality feedback prompts |

### 5.8 Remote Sessions, Bridge, Proxy & TLS

| Environment Variable | Primary Source File(s) | Effect & Invariant |
| :--- | :--- | :--- |
| `CLAUDE_CODE_REMOTE` | `cli/print.ts`, `entrypoints/cli.tsx` | Marks headless remote container execution (auto-injects `--max-old-space-size=8192` into `NODE_OPTIONS` at `cli.tsx:9-14`) |
| `CLAUDE_CODE_REMOTE_SESSION_ID` | `services/analytics/metadata.ts`, `utils/filePersistence/filePersistence.ts` | Binds container turn telemetry and Files API persistence to remote session UUID |
| `CLAUDE_CODE_USE_CCR_V2` | `cli/print.ts`, `bridge/remoteBridgeCore.ts` | Selects CCR v2 transport (`HybridTransport` / `SSETransport` + worker epochs; [Ch 24](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)) |
| `CLAUDE_BRIDGE_SESSION_INGRESS_URL` | `bridge/bridgeMain.ts`, `bridge/initReplBridge.ts` | Overrides session ingress endpoint for remote bridge connections |
| `CLAUDE_CODE_SESSION_ACCESS_TOKEN` | `utils/sessionIngressAuth.ts`, `bridge/remoteBridgeCore.ts` | Bearer token authenticating remote session ingress WebSockets/POSTs |
| `CLAUDE_CODE_IS_COWORK` | `QueryEngine.ts` | Identifies Cowork desktop-managed container execution inside `QueryEngine` |
| `ANTHROPIC_UNIX_SOCKET` | `utils/managedEnv.ts`, `utils/auth.ts` | Routes API traffic over a `-R` SSH-forwarded Unix socket proxy (activates `withoutSSHTunnelVars()`) |
| `CLAUDE_CODE_CLIENT_CERT` / `CLAUDE_CODE_CLIENT_KEY` | `utils/mtls.ts`, `utils/apiPreconnect.ts` | Client certificate & private key paths for mutual TLS (mTLS) to corporate gateways |
| `NODE_EXTRA_CA_CERTS` | `utils/caCerts.ts`, `utils/caCertsConfig.ts` | Custom CA PEM bundle; read during `init.ts` *before* BoringSSL caches its cert store ([Ch 22](ch022-cold-start-mastery-cli-init-main.md)) |
| `HTTP_PROXY` / `HTTPS_PROXY` / `NO_PROXY` | `utils/proxy.ts`, `upstreamproxy/upstreamproxy.ts` | Global outbound proxy routing (blocked from untrusted repo settings prior to trust dialog) |
| `CLAUDE_CODE_PROXY_RESOLVES_HOSTS` | `utils/proxy.ts` | Signals that the corporate proxy handles DNS resolution for SSRF/proxy routing |

---

## Verification & Exploration Commands

Every count and claim in this appendix can be independently verified against the repository with read-only `rg` commands:

1. **Reproduce the complete 90-flag `feature()` census:**
   ```sh
   rg --glob '*.ts' --glob '*.tsx' --glob '!book/**' -o "feature\(['\"][A-Za-z0-9_-]+['\"]\)" | sort | uniq -c | sort -nr
   ```
2. **Observe the `.tsx` constant-folding split vs. `.ts` runtime checks for `USER_TYPE`:**
   ```sh
   rg --glob '*.tsx' --glob '!book/**' -c '"external"\s*[!=]==\s*'\''ant'\'''
   rg --glob '*.ts'  --glob '!book/**' -c 'process\.env\.USER_TYPE'
   ```
3. **Inspect the Bun DCE complexity cliff and positive-ternary invariant comments:**
   ```sh
   rg --glob '*.ts' --glob '*.tsx' --glob '!book/**' 'DCE cliff|Negative pattern \(if \(!feature'
   ```
4. **Audit every env var permitted before the workspace trust dialog vs. scrubbed from subprocesses:**
   Read `utils/managedEnvConstants.ts` (`SAFE_ENV_VARS` & `PROVIDER_MANAGED_ENV_VARS`) alongside `utils/subprocessEnv.ts` (`GHA_SUBPROCESS_SCRUB`).
