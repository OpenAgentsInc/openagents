# Master Outline & Chapter Blueprints

This book is structured as **8 Parts and 24 Chapters** (plus a Prologue). Each part represents one architectural layer sitting cleanly atop the previous parts. You can think of the book as rebuilding the entire agent harness in miniature, one load-bearing beam at a time.

---

## Prologue

### Chapter 00 — How to Read a 1,900-File Agent *(Drafted)*
- **File:** [chapters/00-how-to-read-a-1900-file-agent.md](prologue-how-to-read-a-1900-file-agent.md)
- **Goal:** Orient the reader without drowning in `main.tsx`. Map file distribution across top-level directories (`utils/`, `components/`, `commands/`, `tools/`, `services/`, `hooks/`, `ink/`), explain the five idioms seen everywhere (`feature()` DCE, `lazySchema()`, `bootstrap/state.ts` leaf isolation, React Compiler `_c(...)` output in `.tsx`, and snapshot gaps), and establish the bottom-up reading path.

---

## Part I — Foundations: Atoms, State, and Contracts (Layers 0–1)

*Before an agent can call a model or edit a file, it needs identity, state, bounded caches, concurrency primitives, and a contract for capabilities.*

### Chapter 01 — The Atoms: Identity, Reactive Stores, Lazy Schemas, and Async Generators *(Drafted)*
- **File:** [chapters/01-atoms-types-stores-and-primitives.md](ch001-atoms-types-stores-and-primitives.md)
- **Primary Source:** `types/ids.ts`, `state/store.ts`, `state/AppStateStore.ts`, `state/AppState.tsx`, `bootstrap/state.ts`, `utils/lazySchema.ts`, `utils/generators.ts`, `utils/fileStateCache.ts`
- **Key Questions Answered:**
  - How do branded types prevent session/agent ID mix-ups at compile time?
  - Why does the app build its own 34-line store instead of pulling Redux/Zustand, and how does `useSyncExternalStore` prevent whole-tree re-renders?
  - What belongs in `bootstrap/state.ts` (process leaf singleton) vs. `AppState` (immutable reactive UI/session state)?
  - How does `all()` in `utils/generators.ts` multiplex async generators with a concurrency cap?
  - Why does `FileStateCache` normalize paths, evict by byte size, and track `isPartialView`?

### Chapter 02 — The Tool Contract: Defining What an Agent Can Do *(Drafted)*
- **File:** [chapters/02-the-tool-contract.md](ch002-the-tool-contract.md)
- **Primary Source:** `Tool.ts`, `tools.ts`, `tools/GlobTool/GlobTool.ts`, `constants/tools.ts`, `constants/toolLimits.ts`
- **Key Questions Answered:**
  - What are the four faces of the `Tool<Input, Output, P>` interface (schema, safety/scheduling, lifecycle, UI/transcript projection)?
  - Why does `buildTool()` default to fail-closed (`isConcurrencySafe: false`, `isReadOnly: false`)?
  - Why does `ToolUseContext` carry both `setAppState` and `setAppStateForTasks`, plus `renderedSystemPrompt` and `contentReplacementState`?
  - Why does `assembleToolPool()` sort built-in tools and MCP tools *separately* instead of sorting one flat array?

### Chapter 03 — The Command & Message Contracts: User Input Meets the Model *(Drafted)*
- **File:** [chapters/03-command-and-message-contracts.md](ch003-command-and-message-contracts.md)
- **Primary Source:** `types/command.ts`, `commands.ts`, `utils/messages.ts`, `utils/slashCommandParsing.ts`, `utils/processUserInput/processUserInput.ts`, `utils/processUserInput/processTextPrompt.ts`
- **Key Questions Answered:**
  - How do the three command variants (`prompt`, `local`, `local-jsx`) unify slash commands, skills, plugins, and MCP prompts?
  - How are internal `Message` objects normalized into Anthropic API `MessageParam` payloads (`normalizeMessagesForAPI`), and why are UI-only system messages stripped at that exact boundary?
  - How does `processUserInput()` route plain text, slash commands, image pastes, IDE selections, and attachments?
  - What invariants govern `tool_use` / `tool_result` pairing and thinking-block preservation?

---

## Part II — The Prompt, the Model, and the Engine (Layer 2)

*Now that we have messages and tools, how do we talk to the LLM, keep the prompt cache hot, execute tools concurrently while streaming, and recover from every API failure mode?*

### Chapter 04 — Prompt Engineering as Systems Engineering: Context & Cache Boundaries *(Drafted)*
- **File:** [chapters/04-prompt-engineering-and-cache-boundaries.md](ch004-prompt-engineering-and-cache-boundaries.md)
- **Primary Source:** `context.ts`, `constants/systemPromptSections.ts`, `constants/prompts.ts`, `utils/systemPrompt.ts`, `utils/systemPromptType.ts`, `utils/api.ts`, `utils/claudemd.ts`, `services/api/promptCacheBreakDetection.ts`
- **Key Questions Answered:**
  - Why is `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` load-bearing for cross-org global prompt caching?
  - Why does `DANGEROUS_uncachedSystemPromptSection()` require a `_reason` parameter?
  - Why is `git status` snapshotted *once* at conversation start in `context.ts` and memoized?
  - How does `buildEffectiveSystemPrompt()` resolve priority across override, coordinator, agent, custom, and default prompts?

### Chapter 05 — Talking to the Model: API Client, Streaming, Retries, and Accounting *(Drafted)*
- **File:** [chapters/05-talking-to-the-model.md](ch005-talking-to-the-model.md)
- **Primary Source:** `services/api/claude.ts`, `services/api/client.ts`, `services/api/withRetry.ts`, `services/api/errors.ts`, `services/api/usage.ts`, `cost-tracker.ts`, `utils/model/model.ts`, `utils/thinking.ts`, `utils/effort.ts`
- **Key Questions Answered:**
  - How does `queryModelWithStreaming()` construct requests, attach beta headers, stream SSE chunks, and yield internal `StreamEvent` / `AssistantMessage` records?
  - How does `withRetry.ts` classify transient errors, backoff, and trigger model fallbacks (`FallbackTriggeredError`)?
  - How are token usage, cache read/creation tokens, and USD cost tracked across models and turns?

### Chapter 06 — Executing Tools: Validation, Hooks, Concurrency, and Streaming Execution *(Drafted)*
- **File:** [chapters/06-executing-tools.md](ch006-executing-tools.md)
- **Primary Source:** `services/tools/toolExecution.ts`, `services/tools/toolOrchestration.ts`, `services/tools/StreamingToolExecutor.ts`, `services/tools/toolHooks.ts`, `utils/toolResultStorage.ts`
- **Key Questions Answered:**
  - What is the exact lifecycle of a single tool call (`runToolUse`): lookup → Zod parse → `validateInput` → `PreToolUse` hooks → `canUseTool` permission check → `call()` → spill-to-disk result budgeting → `PostToolUse` hooks?
  - How does `partitionToolCalls()` split tool calls into concurrent read-only batches and serial mutating steps, and why are `contextModifier`s replayed in original block order?
  - How does `StreamingToolExecutor` execute tools *before the assistant finishes streaming*, emit results in strict arrival order, and cancel sibling subprocesses via `siblingAbortController` without killing the turn?
  - How does `toolResultStorage.ts` spill oversized outputs to `<persisted-output>` files without creating a circular Read→file→Read loop (`maxResultSizeChars: Infinity`)?

### Chapter 07 — The Heartbeat: `query.ts` and the Agentic Turn State Machine *(Drafted)*
- **File:** [chapters/07-the-heartbeat-query-loop.md](ch007-the-heartbeat-query-loop.md)
- **Primary Source:** `query.ts`, `query/config.ts`, `query/deps.ts`, `query/stopHooks.ts`, `query/tokenBudget.ts`
- **Key Questions Answered:**
  - How is the entire multi-turn agent loop modeled as an `AsyncGenerator` over an explicit `State` struct with typed `Continue` transitions and `Terminal` exits?
  - Why is `stop_reason === 'tool_use'` NOT trusted as the loop continuation signal (and what replaces it)?
  - How do `QueryDeps` and `QueryConfig` decouple I/O and runtime gates from loop logic?
  - Why are `max_output_tokens` and `prompt_too_long` errors *withheld* from downstream SDK consumers until recovery has been attempted?
  - How do stop hooks (`stop_hook_blocking`, `stop_hook_prevented`) and token-budget continuations keep the agent working or halt it cleanly?

### Chapter 08 — The Session Envelope: `QueryEngine.ts` and Transcript Persistence *(Drafted)*
- **File:** [chapters/08-session-envelope-query-engine-and-transcripts.md](ch008-session-envelope-query-engine-and-transcripts.md)
- **Primary Source:** `QueryEngine.ts`, `utils/sessionStorage.ts`, `utils/sessionRestore.ts`, `utils/conversationRecovery.ts`, `utils/fileHistory.ts`, `utils/queryHelpers.ts`
- **Key Questions Answered:**
  - What responsibilities live in `QueryEngine` (conversation lifetime, `mutableMessages`, `readFileState`, `permissionDenials`, `totalUsage`, SDK framing) versus `query()` (a single multi-step turn)?
  - How are session transcripts written to JSONL, resumed, forked, and recovered from partial writes?
  - How does `fileHistory.ts` snapshot edited files so `/rewind` can roll back both conversation and disk state?

---

## Part III — Surviving Long Horizon: Context Management & Memory (Layer 3)

*LLMs have finite context windows and zero memory across restarts. This part dissects how the agent compresses history without breaking prompt caches or losing critical facts.*

### Chapter 09 — The Five-Layer Compaction Pipeline *(Drafted)*
- **File:** [chapters/09-five-layer-compaction-pipeline.md](ch009-five-layer-compaction-pipeline.md)
- **Primary Source:** `services/compact/microCompact.ts`, `services/compact/apiMicrocompact.ts`, `services/compact/autoCompact.ts`, `services/compact/compact.ts`, `services/compact/sessionMemoryCompact.ts`, `services/compact/prompt.ts`, `services/compact/postCompactCleanup.ts`
- **Key Questions Answered:**
  - How do the five stages compose in `query.ts`: (1) aggregate tool-result budgeting, (2) history snipping, (3) microcompaction (clearing old tool results & cache edits), (4) context collapse, and (5) full summarization (`autoCompact` / `sessionMemoryCompact`)?
  - How does compaction preserve the `compact_boundary`, re-inject active files/skills/plans, reset system-prompt caches, and guard against infinite retry loops with a circuit breaker (`consecutiveFailures`)?
  - Why must `taskBudgetRemaining` be adjusted manually across compaction boundaries?

### Chapter 10 — Memory Systems: `CLAUDE.md`, Auto-Memory, Session Memory, and Auto-Dream *(Drafted)*
- **File:** [chapters/10-memory-systems-claudemd-auto-memory-and-dream.md](ch010-memory-systems-claudemd-auto-memory-and-dream.md)
- **Primary Source:** `utils/claudemd.ts`, `memdir/memdir.ts`, `memdir/memoryTypes.ts`, `memdir/findRelevantMemories.ts`, `memdir/memoryScan.ts`, `services/SessionMemory/sessionMemory.ts`, `services/extractMemories/`, `services/autoDream/autoDream.ts`, `services/teamMemorySync/`
- **Key Questions Answered:**
  - How are hierarchical `CLAUDE.md` files discovered, filtered, deduplicated (`loadedNestedMemoryPaths`), and injected as nested memory attachments when the agent touches new subdirectories?
  - How does `startRelevantMemoryPrefetch()` hide memory retrieval latency behind the LLM stream using `using` disposable resource management?
  - How does `SessionMemory` maintain a live structured scratchpad in the background, and how does `autoDream` consolidate cross-session memories under a lock?

---

## Part IV — Safety, Policy, and Extensibility (Layer 4)

*Giving an LLM a shell and filesystem access requires layered defense in depth—and giving users safe ways to customize the loop.*

### Chapter 11 — The Permission Engine: Modes, Rules, Classifiers, and Persistence *(Drafted)*
- **File:** [chapters/11-the-permission-engine.md](ch011-the-permission-engine.md)
- **Primary Source:** `types/permissions.ts`, `utils/permissions/PermissionMode.ts`, `utils/permissions/permissions.ts`, `utils/permissions/permissionSetup.ts`, `utils/permissions/filesystem.ts`, `utils/permissions/shellRuleMatching.ts`, `utils/permissions/yoloClassifier.ts`, `utils/permissions/denialTracking.ts`, `hooks/useCanUseTool.tsx`
- **Key Questions Answered:**
  - How do permission modes (`default`, `plan`, `acceptEdits`, `dontAsk`, `bypassPermissions`, `auto`) interact with `alwaysAllowRules`, `alwaysDenyRules`, and `alwaysAskRules` across 8 rule sources?
  - How does filesystem path validation (`filesystem.ts`, `pathValidation.ts`) handle symlinks, additional working directories, and protected paths?
  - How does the auto-mode security classifier (`yoloClassifier.ts`) evaluate tool actions, and how does `denialTracking.ts` fall back to interactive prompting after consecutive denials?

### Chapter 12 — Shell Execution, Sandboxing, and Core Workspace Tools *(Drafted)*
- **File:** [chapters/12-shell-execution-sandboxing-and-workspace-tools.md](ch012-shell-execution-sandboxing-and-workspace-tools.md)
- **Primary Source:** `tools/BashTool/`, `tools/PowerShellTool/`, `tools/FileReadTool/`, `tools/FileEditTool/`, `tools/FileWriteTool/`, `tools/GrepTool/`, `utils/bash/`, `utils/sandbox/sandbox-adapter.ts`, `utils/shell/`
- **Key Questions Answered:**
  - How does `BashTool` parse shell ASTs/redirections to distinguish read-only commands from mutating commands, enforce timeouts, and promote long-running commands to background tasks?
  - How does `SandboxManager` (`utils/sandbox/sandbox-adapter.ts`) enforce OS-level filesystem and network isolation?
  - How do `FileReadTool`, `FileEditTool`, and `FileWriteTool` cooperate with `FileStateCache` to prevent stale blind overwrites?

### Chapter 13 — The Hook System: Intercepting Every Lifecycle Event *(Drafted)*
- **File:** [chapters/13-the-hook-system.md](ch013-the-hook-system.md)
- **Primary Source:** `types/hooks.ts`, `schemas/`, `utils/hooks.ts`, `utils/hooks/hookEvents.ts`, `utils/hooks/hooksConfigManager.ts`, `utils/hooks/AsyncHookRegistry.ts`, `utils/hooks/execHttpHook.ts`, `utils/hooks/execPromptHook.ts`, `utils/hooks/execAgentHook.ts`, `utils/hooks/ssrfGuard.ts`, `services/tools/toolHooks.ts`
- **Key Questions Answered:**
  - What are the four hook execution backends (shell command, HTTP webhook with SSRF guard, single-shot LLM prompt hook, and full agent hook)?
  - How can a `PreToolUse` hook approve, deny, ask, or rewrite `updatedInput`?
  - How do `hooksConfigSnapshot.ts` and `sessionHooks.ts` guarantee consistent hook behavior across subagents and mid-session settings edits?

### Chapter 14 — Configuration, Settings Precedence, and Enterprise Policy *(Drafted)*
- **File:** [chapters/14-configuration-settings-and-enterprise-policy.md](ch014-configuration-settings-and-enterprise-policy.md)
- **Primary Source:** `utils/config.ts`, `utils/settings/settings.ts`, `utils/settings/constants.ts`, `utils/settings/types.ts`, `utils/settings/changeDetector.ts`, `utils/settings/applySettingsChange.ts`, `services/remoteManagedSettings/`, `services/policyLimits/`, `migrations/`
- **Key Questions Answered:**
  - How do user, project, local, CLI flag, MDM, and remote-managed settings merge, and which sources are fail-closed vs. fail-open?
  - How does live settings file watching (`changeDetector.ts`) propagate changes into both interactive `AppState` and headless SDK sessions?
  - How do `migrations/` run idempotently during startup?

---

## Part V — Dynamic Capabilities: MCP, Skills, Plugins, and LSP (Layer 5)

*An agent shouldn't be limited to hardcoded built-in tools. Here we reverse-engineer every dynamic extension point.*

### Chapter 15 — Model Context Protocol (MCP): Clients, Transports, Auth, and Tool Search *(Drafted)*
- **File:** [chapters/15-model-context-protocol-mcp.md](ch015-model-context-protocol-mcp.md)
- **Primary Source:** `services/mcp/client.ts`, `services/mcp/config.ts`, `services/mcp/auth.ts`, `services/mcp/useManageMCPConnections.ts`, `services/mcp/elicitationHandler.ts`, `services/mcp/InProcessTransport.ts`, `services/mcp/SdkControlTransport.ts`, `tools/MCPTool/`, `tools/ToolSearchTool/`, `utils/toolSearch.ts`
- **Key Questions Answered:**
  - How does the MCP client manager handle stdio, SSE/HTTP, WebSocket, in-process, and SDK-control transports plus OAuth flows?
  - How do MCP tools get normalized (`mcp__server__tool`), permission-checked, and exposed alongside built-ins?
  - How does `ToolSearchTool` + `shouldDefer: true` solve tool-schema context bloat when dozens of MCP servers are attached?

### Chapter 16 — Skills, Plugins, Output Styles, and LSP Intelligence *(Drafted)*
- **File:** [chapters/16-skills-plugins-output-styles-and-lsp.md](ch016-skills-plugins-output-styles-and-lsp.md)
- **Primary Source:** `skills/loadSkillsDir.ts`, `skills/bundledSkills.ts`, `tools/SkillTool/`, `plugins/builtinPlugins.ts`, `utils/plugins/`, `outputStyles/`, `services/lsp/LSPServerManager.ts`, `services/lsp/LSPClient.ts`, `services/lsp/passiveFeedback.ts`, `tools/LSPTool/`
- **Key Questions Answered:**
  - How are Markdown-defined skills parsed, path-scoped (`paths` glob triggers), discovered, and executed either inline or in a forked subagent?
  - How do plugins bundle commands, skills, agents, hooks, and MCP servers—and how does `/reload-plugins` hot-swap them?
  - How does `services/lsp/` run language servers in the background and feed compiler diagnostics back to the model?

---

## Part VI — Concurrency, Tasks, Subagents, and Swarms (Layer 6)

*One agent turn is good; background jobs, isolated worktrees, forked subagents, and cooperating teammate swarms are where the architecture turns distributed.*

### Chapter 17 — The Background Task Runtime *(Drafted)*
- **File:** [chapters/17-background-task-runtime.md](ch017-background-task-runtime.md)
- **Primary Source:** `Task.ts`, `tasks.ts`, `tasks/types.ts`, `tasks/stopTask.ts`, `tasks/LocalShellTask/`, `tasks/LocalAgentTask/`, `tasks/RemoteAgentTask/`, `tasks/InProcessTeammateTask/`, `tasks/DreamTask/`, `utils/task/`
- **Key Questions Answered:**
  - How does a single unified `TaskStateBase` record track 7 distinct background task kinds (`local_bash`, `local_agent`, `remote_agent`, `in_process_teammate`, `local_workflow`, `monitor_mcp`, `dream`)?
  - How are task outputs persisted to disk with byte offsets (`outputFile`, `outputOffset`) so the agent and UI can tail them without holding gigabytes in RAM?
  - How do scheduled cron tasks (`tools/ScheduleCronTool/`, `utils/cron*.ts`) and `SleepTool` enable proactive/periodic wakeups?

### Chapter 18 — Subagents, Context Forking, and Git Worktrees *(Drafted)*
- **File:** [chapters/18-subagents-context-forking-and-worktrees.md](ch018-subagents-context-forking-and-worktrees.md)
- **Primary Source:** `tools/AgentTool/AgentTool.tsx`, `tools/AgentTool/runAgent.ts`, `tools/AgentTool/forkSubagent.ts`, `tools/AgentTool/loadAgentsDir.ts`, `tools/AgentTool/builtInAgents.ts`, `tools/AgentTool/resumeAgent.ts`, `tools/EnterWorktreeTool/`, `utils/worktree.ts`
- **Key Questions Answered:**
  - How does `createSubagentContext()` isolate a child agent's `readFileState`, `abortController`, `localDenialTracking`, and `setAppState` (while keeping `setAppStateForTasks` wired to the root store)?
  - How does `forkSubagent.ts` freeze `renderedSystemPrompt` and clone `contentReplacementState` so a forked subagent achieves near-100% prompt-cache hits against its parent's conversation prefix?
  - How do sidechain transcripts enable resuming a background agent days later?
  - How do `EnterWorktreeTool` / `ExitWorktreeTool` isolate concurrent code edits in Git worktrees?

### Chapter 19 — Agent Teams, Swarms, and Coordinator Mode *(Drafted)*
- **File:** [chapters/19-agent-teams-swarms-and-coordinator-mode.md](ch019-agent-teams-swarms-and-coordinator-mode.md)
- **Primary Source:** `utils/swarm/inProcessRunner.ts`, `utils/swarm/spawnInProcess.ts`, `utils/swarm/leaderPermissionBridge.ts`, `utils/swarm/permissionSync.ts`, `utils/swarm/teamHelpers.ts`, `utils/teammateMailbox.ts`, `tools/TeamCreateTool/`, `tools/SendMessageTool/`, `coordinator/`
- **Key Questions Answered:**
  - How do in-process teammates and pane/tmux teammates communicate via mailboxes (`SendMessageTool`, `teammateMailbox.ts`)?
  - How does `leaderPermissionBridge.ts` bubble a worker's permission prompt up to the team leader's interactive terminal?
  - How does Coordinator Mode strip editing tools from the leader and turn it into a pure orchestrator over worker agents?

---

## Part VII — The Terminal UI Engine (Layer 7)

*A terminal app that streams tokens, renders diffs, handles vim motions, supports mouse selection, and shows live spinners at 60 FPS cannot rely on `console.log`.*

### Chapter 20 — Inside `ink/`: A Custom React Renderer for the Terminal *(Drafted)*
- **File:** [chapters/20-inside-ink-terminal-react-renderer.md](ch020-inside-ink-terminal-react-renderer.md)
- **Primary Source:** `ink/dom.ts`, `ink/reconciler.ts`, `ink/ink.tsx`, `ink/render-node-to-output.ts`, `ink/screen.ts`, `ink/log-update.ts`, `ink/parse-keypress.ts`, `ink/selection.ts`, `ink/layout/`, `native-ts/yoga-layout/`
- **Key Questions Answered:**
  - Why does this repo maintain a full vendored/customized Ink implementation (~96 files) instead of a stock npm dependency?
  - How does a React component tree flow through `react-reconciler` → custom DOM nodes → Yoga flexbox layout → 2D character cell `Screen` buffer → diffed ANSI escape sequences?
  - How do terminal focus tracking, mouse hit-testing (`hit-test.ts`), text selection, Kitty/iTerm keyboard parsing (`parse-keypress.ts`), and hyperlinks work?

### Chapter 21 — The Interactive REPL: Screens, Input, Keybindings, and Vim Mode *(Drafted)*
- **File:** [chapters/21-the-interactive-repl-screens-keybindings-and-vim.md](ch021-the-interactive-repl-screens-keybindings-and-vim.md)
- **Primary Source:** `replLauncher.tsx`, `components/App.tsx`, `screens/REPL.tsx`, `interactiveHelpers.tsx`, `components/PromptInput/`, `components/messages/`, `keybindings/`, `vim/`, `hooks/useVimInput.ts`, `hooks/useMergedCommands.ts`, `hooks/useMergedTools.ts`
- **Key Questions Answered:**
  - How does `screens/REPL.tsx` bridge the async generator of `query()` into React state, message lists, progress indicators, permission modals, and queued user prompts?
  - How does `keybindings/` resolve user-customizable shortcuts across contexts without colliding with modal dialogs or terminal sequences?
  - How does `vim/` implement a pure-functional state machine for motions, operators, and text objects inside the prompt editor?

---

## Part VIII — Entrypoints, Startup, SDK, and Remote Control (Layer 8)

*Now—and only now—we arrive at `entrypoints/cli.tsx` and `main.tsx`, seeing how every layer we built gets wired together for every execution mode.*

### Chapter 22 — Cold Start Mastery: From `entrypoints/cli.tsx` Through `init.ts` to `main.tsx` *(Drafted)*
- **File:** [chapters/22-cold-start-mastery-cli-init-main.md](ch022-cold-start-mastery-cli-init-main.md)
- **Primary Source:** `entrypoints/cli.tsx`, `entrypoints/init.ts`, `setup.ts`, `main.tsx`, `utils/startupProfiler.ts`, `utils/gracefulShutdown.ts`, `utils/cleanupRegistry.ts`
- **Key Questions Answered:**
  - How does `cli.tsx` keep `--version` at zero imports and branch fast-paths (MCP servers, daemon workers, remote bridge) before touching the 800 KB `main.tsx`?
  - Why does `init.ts` order config loading → safe env vars → CA certs (before BoringSSL caches the cert store!) → mTLS/proxy → `preconnectAnthropicApi()` TCP+TLS warmup?
  - How does `main.tsx` run parallel prefetches, trust dialogs, migrations, and dispatch into interactive REPL vs. `-p` print mode vs. SDK mode?

### Chapter 23 — Machine-Readable Modes: Headless CLI, Agent SDK, and Control Protocols *(Drafted)*
- **File:** [chapters/23-machine-readable-modes-headless-sdk-and-control-protocols.md](ch023-machine-readable-modes-headless-sdk-and-control-protocols.md)
- **Primary Source:** `cli/print.ts`, `cli/structuredIO.ts`, `cli/ndjsonSafeStringify.ts`, `entrypoints/agentSdkTypes.ts`, `entrypoints/sdk/controlSchemas.ts`, `entrypoints/sdk/coreSchemas.ts`, `entrypoints/mcp.ts`, `utils/streamJsonStdoutGuard.ts`
- **Key Questions Answered:**
  - How does `structuredIO.ts` multiplex NDJSON user inputs, control requests, permission round-trips, and MCP elicitations over stdin/stdout while keeping stdout pure?
  - How does `entrypoints/mcp.ts` turn the agent itself into an MCP server?

### Chapter 24 — Across the Wire: Remote Sessions, Always-On Bridge, and Direct Connect *(Drafted)*
- **File:** [chapters/24-across-the-wire-remote-sessions-bridge-and-direct-connect.md](ch024-across-the-wire-remote-sessions-bridge-and-direct-connect.md)
- **Primary Source:** `bridge/bridgeMain.ts`, `bridge/remoteBridgeCore.ts`, `bridge/replBridge.ts`, `bridge/replBridgeTransport.ts`, `bridge/bridgePermissionCallbacks.ts`, `cli/transports/HybridTransport.ts`, `cli/transports/WebSocketTransport.ts`, `cli/transports/SSETransport.ts`, `remote/RemoteSessionManager.ts`, `server/directConnectManager.ts`, `upstreamproxy/`
- **Key Questions Answered:**
  - How does the REPL bridge expose a local terminal session to a remote web/mobile client (`claude.ai`) with bidirectional messages, attachments, and remote permission approvals?
  - How do `HybridTransport`, `WebSocketTransport`, and `SSETransport` handle reconnection, backpressure, and ordered event delivery?
  - How does `upstreamproxy/` inject org credentials into container subprocess requests via a local HTTP `CONNECT` relay?

---

## Part IX — Epilogue: The Snapshot Meets the Shipped Product

*The book ends where its source ends, at 2.1.88. This epilogue carries the nine-layer model forward to the build installed today, and labels every claim by how we know it.*

### Chapter 25 — Bringing It to the Present: From 2.1.88 to 2.1.331 *(Drafted)*
- **File:** [chapters/25-bringing-it-to-the-present.md](ch025-bringing-it-to-the-present.md)
- **Goal:** Map eight months and roughly 240 releases of drift onto Layers 0–8, tagging each claim as Changelog, Binary (2.1.331 or 2.1.296 extraction), or Speculation, and show which 2.1.88 contracts held.
- **Primary Source:** the public changelog (2.1.89–2.1.331), modules carved from the shipped 2.1.331 native binary, `2026-10-11-claude-code-2.1.296-workflows-binary-audit.md`, and snapshot anchors such as `Tool.ts`, `query.ts`, `entrypoints/sdk/coreTypes.ts`, `types/permissions.ts`, `constants/betas.ts`
- **Key Questions Answered:**
  - What can a bun-compiled binary tell us about the current architecture, and what does minification hide?
  - Which snapshot `feature()` gates became shipping features (workflows, auto mode, NO_FLICKER fullscreen, PowerShell, Code Mode), and what was removed (`/tag`, `/vim`, `/buddy`, TaskOutput, TeamCreate)?
  - Which 2.1.88 abstractions (Tool defaults, the generator loop and its terminal reasons, the permission decision-reason union, hook events, the dynamic cache boundary) survived to 2.1.331?

---

## Appendices *(Drafted)*

- **Appendix A — Complete Inventory of Build-Time `feature()` Flags & Environment Knobs:** [appendices/A-feature-flags-and-env-knobs.md](appendix-a-feature-flags-and-env-knobs.md)
- **Appendix B — Reconstructing the Missing Snapshot Boundaries (`types/message.ts`, `query/transitions.ts`) from Call Sites:** [appendices/B-missing-snapshot-boundaries.md](appendix-b-missing-snapshot-boundaries.md)
- **Appendix C — Mini-Harness Blueprint: Building a 500-Line Working Toy Clone Using the Patterns in This Book:** [appendices/C-mini-harness-blueprint.md](appendix-c-mini-harness-blueprint.md)
