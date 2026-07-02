# Claude Code Bring-Up To Codex Parity, And Codex×Claude Synergies

Date: 2026-07-01
Status: analysis + implementation plan. Grounded in a full read of the Claude
Agent SDK TypeScript types (`@anthropic-ai/claude-agent-sdk@0.3.172`, installed
under `apps/pylon/node_modules/`), the SDK repo at
`projects/repos/claude-agent-sdk-typescript`, and a file-by-file seam map of
`clients/khala-code-desktop`. Covers first getting Claude Code up to speed with
the Codex harness in Khala Code Desktop, then the interesting crossovers —
Fable/Claude-Agent-SDK planning delegating to Codex for coding — paired with
the fleet-management work. Companion to the multi-harness section of
`2026-07-01-episode-245-completion-and-multi-harness-orchestration.md`, the
fleet fan-out instructions, and the Effect integration audit in this folder.
Documentation-only; flips no promise state.
Execution: Phases 0–3 and the synergy crossovers in §4 are scheduled as the
Claude-harness and multi-harness workstreams in the unified
[`ROADMAP.md`](./ROADMAP.md). Phase 0's `ChatRuntime` selector is the same
seam as the episode-245 doc's Axis A toggle — build it once; the fan-out
doc's Lane B4 (`workerKind`) is the Axis B half.

## 1. Where We Actually Stand

Two facts frame everything:

**Claude delegation already works — in Pylon, not the desktop.** The
`claude_agent_task` workflow, `apps/pylon/src/claude-agent-executor.ts` (driven
by the SDK's `query()`), isolated per-account `.claude-*` homes via
`CLAUDE_CONFIG_DIR`, per-account capacity refs, the claude-supervisor, and exact
token rows (`pylon-claude-own-capacity` → `/api/pylon/claude/turns`) are all
live at ~80% Codex-lane parity (per the episode-245 doc's audit). So the *worker*
lane exists. What does not exist is Claude as a **chat harness in the desktop**:
`clients/khala-code-desktop` is entirely Codex-centric.

**The desktop has no runtime abstraction to extend.** The seam map is blunt:
there is no shared chat-runtime interface. `CodexAppServerChatRuntime`
(`codex-app-server-chat-runtime.ts:96`) is a concrete object type; the legacy
Khala-native runtime is a bare function; `rpc-handlers.ts` selects between them
with ad-hoc branching (`useLegacyKhalaNativeRuntime()`, `:674`) that is honored
in exactly one place (`submitChatMessage`, `:1950`). Every thread-lifecycle RPC
(`codexThreadList/Read/Rename/Archive/Delete/Fork/Compact`, `codexTurnStart/
Interrupt/Steer`) delegates unconditionally to `requireCodexChatRuntime()`
(`:1843-1887`). So "add Claude" is really "introduce the harness abstraction the
Codex pivot never needed to."

The good news the seam map surfaces: the **transcript event model is already
harness-neutral**. `KhalaCodeDesktopChatTurnEvent` (`shared/rpc.ts:65` —
`thread_ready`/`message_start`/`message_delta`/`message_replace`/`message_done`/
`tool_event`) is not Codex-specific, and the headless path proves it — it
consumes only that neutral stream plus a three-method runtime
(`interruptTurn|startThread|startTurn`, `headless.ts:15`). The neutral event
model is the contract; everything Codex-specific sits on one side of it.

## 2. The SDK In One Page (What We're Wrapping)

The Claude Agent SDK is the mirror image of Codex app-server in shape:

- **Transport**: `query({prompt, options})` spawns the bundled Claude Code CLI
  and speaks a JSON control protocol over stdio (`Transport`, `sdk.d.ts:6327`).
  Bun is first-class (`executable: 'bun'`). `startup()` pre-warms the subprocess.
- **The `Query` object** *is* an `AsyncGenerator<SDKMessage>` — you iterate it
  for the turn stream, and it carries control methods: `interrupt()`,
  `setPermissionMode()`, `setModel()`, `setMcpServers()`, `mcpServerStatus()`,
  `supportedCommands()`, `supportedModels()`, `supportedAgents()`,
  `getContextUsage()`, `accountInfo()`, `rewindFiles()`, `close()`.
- **Message model**: a 35-variant `SDKMessage` union, every variant carrying
  `uuid` + `session_id`. Key ones: `system/init` (tools, mcp servers, model,
  slash_commands, skills, plugins), `assistant` (text + thinking + tool_use
  blocks), `user` (tool_result), `result/success|error_*` (with `usage`,
  `modelUsage`, `total_cost_usd`, `permission_denials`), `stream_event`
  (partial deltas, only with `includePartialMessages`),
  `system/compact_boundary`, `system/session_state_changed`
  (idle|running|requires_action), `system/status`, `rate_limit_event`.
- **Sessions**: persisted as JSONL under `~/.claude/projects/<dir>/<id>.jsonl`;
  session ids are UUIDs (settable via `options.sessionId`); `resume`,
  `continue`, `forkSession`, `resumeSessionAt`. Crucially, sessions are
  **programmatically enumerable**: `listSessions()`, `getSessionMessages()`,
  `forkSession()`, `renameSession()`, `deleteSession()`, `tagSession()`, plus a
  `SessionStore` (@alpha) dual-write mirror adapter with a reusable conformance
  suite in `examples/session-stores/`.
- **Permissions**: the `canUseTool(toolName, input, options)` callback returns
  `PermissionResult` (`allow` with optional `updatedInput`/`updatedPermissions`,
  or `deny` with message/`interrupt`); the `options` bag is rich enough to drive
  a desktop dialog directly (`title`, `displayName`, `description`,
  `blockedPath`, `suggestions`, `toolUseID`). `permissionMode` ∈
  `default|acceptEdits|bypassPermissions|plan|dontAsk|auto`. Thirty **hooks**
  (`PreToolUse`…`MessageDisplay`) are a second, non-interactive permission
  channel — a `PreToolUse` deny bypasses `canUseTool`.
- **Tools/MCP**: in-process MCP via `createSdkMcpServer` + `tool()` (Zod
  schema); external stdio/SSE/HTTP MCP via `options.mcpServers`; status via
  `mcpServerStatus()`.
- **Subagents**: `options.agents: Record<string, AgentDefinition>` — each with
  its own prompt, tools, model, mcpServers, skills, `background`, `memory`,
  `permissionMode`. `options.agent` picks the main-thread agent.
- **Auth**: resolved by the subprocess from env / `~/.claude` OAuth;
  `ANTHROPIC_API_KEY` must be spread into `options.env` (env *replaces*, not
  merges). `accountInfo()` reports the resolved identity/provider.

### Codex ↔ Claude mapping (the parity Rosetta stone)

| Codex app-server | Claude Agent SDK |
| --- | --- |
| thread | session (`session_id`, JSONL-persisted) |
| thread id | `session_id` (UUID, settable) |
| turn (explicit object) | implicit: `user` → `result` span; `session_state_changed` + `result` mark boundaries |
| item: agent message | `assistant` text blocks |
| item: reasoning | thinking blocks + `stream_event` |
| item: command execution | Bash `tool_use`/`tool_result` + `SDKToolProgressMessage` |
| item: file change/patch | Edit/Write `tool_use`; `SDKFilesPersistedEvent`; `rewindFiles()` |
| item: MCP tool call | `tool_use` named `mcp__server__tool` |
| item deltas | `stream_event` (needs `includePartialMessages`) |
| approval request | `canUseTool` callback (interactive) / `PreToolUse` hook (auto) |
| approval decision | `PermissionResult` |
| interrupt turn | `Query.interrupt()` (graceful) / `close()` + abort (hard) |
| thread/list, read, fork, rename, delete | `listSessions`, `getSessionMessages`, `forkSession`, `renameSession`, `deleteSession` |
| slash inventory | `supportedCommands()` + `system/init.slash_commands` |
| model/config read | `system/init` + `supportedModels()` + `accountInfo()` |

The load-bearing difference: **Codex gives explicit turn/item objects on the
wire; the SDK gives a flatter message stream you reconstruct turns from** (user→
result spans) and items from `tool_use`/`tool_result` pairing via
`parent_tool_use_id`/`tool_use_id`. The desktop's existing thread-item projector
is exactly this reconstruction for Codex; Claude needs a parallel one.

## 3. Claude Bring-Up Plan (Reach Codex Parity)

The ordering is: build the abstraction the desktop lacks, land a minimal Claude
runtime behind it, then close parity surface by surface. This should be done
**on the Effect spine the integration audit recommends** — the ClaudeChatRuntime
is the ideal first real Effect service in the desktop, since it is greenfield
and the SDK maps so cleanly onto Stream + service methods + Schema.

### Phase 0 — the harness abstraction (prerequisite, small)

Introduce a `ChatRuntime` interface both Codex and Claude satisfy, and turn the
one-place branch into a real selector:

- `src/shared/rpc.ts`: extend `KhalaCodeDesktopRuntimeMode` (add
  `claude_runtime`) and `KhalaCodeDesktopBackendProjection.kind` (add
  `claude_app_sdk`); generalize the `codexItem` message field name to a
  harness-neutral `harnessItem` (alias for back-compat).
- `rpc-handlers.ts`: replace `useLegacyKhalaNativeRuntime()` with a three-way
  `selectChatRuntime()` and route `submitChatMessage`, `codexTurnStart`, and the
  thread-lifecycle RPCs through the selected runtime instead of
  `requireCodexChatRuntime()` unconditionally.
- `headless.ts`: widen `KhalaCodeDesktopHeadlessCodexRuntime` to a neutral
  `Pick<ChatRuntime, "interruptTurn"|"startThread"|"startTurn">` (it already
  only needs those three).

The minimal `ClaudeChatRuntime` surface (from the headless proof): `startTurn`
(streaming via injected `onEvent`, returning `KhalaCodeDesktopChatTurnResponse`),
`startThread`, `resumeThread`, `interruptTurn`, `threadIdForSession`. The
sidebar/slash methods (`listThreads`, `readThread`, rename/archive/fork/compact)
can throw a typed `unsupported` initially and light up in later phases.

### Phase 1 — minimal Claude chat (the "hello, Claude" bar)

- `src/bun/claude-app-sdk-chat-runtime.ts` (new): `createClaudeAppSdkChatRuntime`
  as an Effect service wrapping `query()`. The message iterable becomes a
  `Stream` via `Stream.fromAsyncIterable`, the `Query` handle an
  `acquireRelease` resource (release calls `close()` + aborts the owned
  `AbortController`). Own the `AbortController` explicitly — the SDK's forwarded
  spawn signal only fires after a ~2s grace, so fiber interruption must abort the
  controller directly for immediate teardown; map the user's "stop" button to
  `Query.interrupt()` (graceful) as a distinct service method.
- `src/bun/claude-thread-item-projector.ts` (new): map `SDKMessage` →
  `KhalaCodeDesktopChatTurnEvent`. `assistant` text → `message_delta`/`_replace`;
  thinking → reasoning card; `tool_use`/`tool_result` (paired via id) →
  `tool_event`; `result` → `message_done` with usage; `session_state_changed`/
  `status` → status. Model the `SDKMessage` union as an Effect `Schema.Union`
  (discriminate on `type` then `subtype`), keeping the inner `message:
  BetaMessage`/`MessageParam` as passthrough — decode at the SDK boundary so the
  rest of the desktop stays typed.
- `src/bun/claude-session-store.ts` (new): `~/.khala-code/claude-sessions.json`
  (schema `khala-code-desktop.claude-sessions.v1`, env override
  `KHALA_CODE_DESKTOP_CLAUDE_STATE_PATH`), mapping `desktopSessionId →
  {sessionId, lastTurnId?, updatedAt}`. Resume via the SDK `resume` option. The
  SDK's own `listSessions()`/`getSessionMessages()` can back the sidebar later,
  so this file only needs the desktop↔SDK id mapping.
- `src/bun/claude-harness-status.ts` (new): wrap Pylon's existing probes —
  `probeClaudeAgentReadiness` (SDK import test → `sdk_missing`) and
  `localClaudeSessionPresent` (`CLAUDE_CODE_OAUTH_TOKEN` →
  `CLAUDE_CONFIG_DIR/.credentials.json` → `~/.claude/.credentials.json` →
  keychain) from `apps/pylon/src/claude-agent.ts` — into the desktop
  `HarnessStatus` shape. States map cleanly: `ready|sdk_missing|
  credentials_missing|platform_unsupported|disabled_by_config`.
- `src/bun/index.ts`: construct the Claude runtime + pass it into the handlers
  and headless injector.

Parity bar for Phase 1: new session, submit prompt, stream assistant text +
reasoning, stream tool calls, interrupt, resume on reload. First cut may run
`permissionMode: 'acceptEdits'` (or `bypassPermissions` for owner-local, matching
Codex's `danger-full-access` posture) with an empty slash registry — documented
as a gap in a new `claude-app-sdk-gap-matrix.ts`, exactly as the Codex pivot
tracked its gaps.

### Phase 2 — approvals, telemetry, MCP, settings

- **Approvals** (`src/bun/claude-approvals`): wire `canUseTool` as an
  Effect-bridged edge callback — the SDK awaits a `Promise` your renderer
  resolves via a `Deferred`/`Queue`-backed approval service. The `options` bag
  (`title`/`displayName`/`description`/`suggestions`) drives the dialog; "always
  allow" returns `{behavior:'allow', updatedPermissions: options.suggestions}`;
  set `decisionClassification` for telemetry. This is Claude's parallel to
  `codex-approval-decisions.ts` — do not try to force Claude decisions through
  the Codex approval shapes. Respect `options.signal` → fiber interruption.
- **Token telemetry** (`src/bun/claude-token-usage-telemetry.ts`): from the
  `result` message's `usage`/`modelUsage`/`total_cost_usd`, reusing
  `apps/pylon/src/claude-turn-reporter.ts`'s body shape
  (`openagents.pylon.claude_turn.v1`, provider `pylon-claude-own-capacity`).
  Decide the ingest route deliberately: the desktop Codex path posts to
  `/api/stats/token-usage/events`; the Pylon Claude lane posts to
  `/api/pylon/claude/turns`. Keeping the desktop-Claude path on the stats route
  matches the desktop-Codex path; keep exact-only accounting either way.
- **MCP / fleet bridge** (`src/bun/claude-fleet-mcp-bridge.ts`): the Claude
  equivalent of the Codex MCP bridge is *simpler* — no config file to mutate.
  Inject the `khala_fleet` server descriptor directly into `options.mcpServers`
  at `startTurn` time (the SDK takes in-process/stdio MCP as an option). This is
  where message-triggered fleet delegation works in Claude mode.
- **Settings** (`src/ui/claude-settings-panel.ts`): model/permission-mode from
  `system/init` + `supportedModels()` + `accountInfo()`. Parallel to the Codex
  settings panel.

### Phase 3 — sidebar, slash, full parity

- Sidebar backed by the SDK's `listSessions()`/`getSessionMessages()` (a real
  advantage — no bespoke list projection needed).
- Slash registry from `supportedCommands()` + `system/init.slash_commands`,
  refreshed on `commands_changed`. Invoke by sending `/name args` as prompt.
- `claude-parity-contract.ts` + gap matrix parallel to the Codex ones, pinned to
  the installed SDK version (0.3.172; note the CHANGELOG head is 0.3.198 with
  `reinitialize()`, background-agent `agent_id` on `can_use_tool`, and a
  `canUseTool`-shadowing warning to heed when bumping).

### The UI switch

`runtimeMode` already rides every response (`backend.runtimeMode`). The composer
HUD now has the mirrored persisted setting for the **Axis A** chat-harness
toggle from the multi-harness doc: "Codex | Claude | Khala". Env vars remain
operator overrides, and the visible runtime badge renders from the backend
`runtimeMode` on the response.

### Files to touch (ordered)

`shared/rpc.ts` → `claude-app-sdk-chat-runtime.ts` →
`claude-thread-item-projector.ts` → `claude-session-store.ts` →
`claude-harness-status.ts` → `rpc-handlers.ts` (selector) → `index.ts` →
`headless.ts` → approvals/telemetry/mcp-bridge/settings → `main.ts` (pill) →
parity contract + gap matrix.

## 4. The Synergies (Where This Gets Interesting)

Parity is table stakes. The reason to want both harnesses in one app is that
their strengths differ, and our fleet substrate lets us route by strength. Three
crossovers, in order of leverage:

### 4.1 Fable-planning → Codex-coding (the headline crossover)

The pattern the owner named: use the Claude Agent SDK (Fable-powered) for
**planning** and delegate the **coding** to Codex. The SDK makes this
first-class in two complementary ways:

- **Plan mode as a phase.** Run a Claude session with `permissionMode: 'plan'`
  and `planModeInstructions` to produce a structured plan without touching the
  filesystem — then hand the plan to the fleet as Codex work units. The plan
  becomes the *decomposition* step feeding the fan-out doc's work planner: Claude
  reads the repo/issue, emits a typed task DAG, and each node dispatches as a
  `codex_agent_task`. Fable is the strong reasoner here (the SDK exposes
  `model`, `effort: max`, `thinking: enabled`), Codex is the high-precision
  executor (per the owner's stated preference in transcript 245: "you are not as
  good at Codex at the high precision engineering tasks").
- **Claude subagents that delegate to Codex via MCP.** The SDK's `agents`
  option + in-process MCP means a Claude planning agent can be given a
  `fleet_dispatch` MCP tool (the same `khala_fleet` server) as its only "coding"
  capability — so the planner literally calls `codex_spawn`/the deterministic
  `khala.fleet.delegate` program as a tool, watches lifecycle events, and
  composes results. This is the multi-harness doc's Axis B (`workerKind: codex |
  claude | auto`) with a Claude *orchestrator* on top of Axis A.

Concretely: a "plan-then-fan-out" run = Claude session (plan mode, Fable) →
emits `FleetRun` with N work units → supervisor dispatches each to Codex (or
Claude, or auto) → Claude reviews the returned diffs (its strength) and either
accepts, requests changes (the annotate-diff loop from the Orca doc), or
re-plans. The deterministic delegation program stays the control-flow authority;
Claude supplies decomposition and review judgment, not per-call control.

**T9.3 update (2026-07-02):** the `auto` target now has a classifier-aware
parameter layer. A typed workflow-classification hint can bias `auto` toward
Codex or Claude only when that lane has advertised free slots; admitted
parameters tune confidence threshold, classifier bonus, and tie-breaker. This
keeps Claude/Fable planning as advisory structure while `khala.fleet.delegate`
continues to own deterministic control flow.

### 4.2 Claude as reviewer / verifier-adjacent

Claude's review quality pairs with our verification gates. After a Codex worker's
verify command passes, a Claude session can do a second-pass semantic review
(the SDK's `outputFormat: json_schema` gives a structured verdict), feeding the
QA framework's oracle set and the merge policy. This keeps the honest-evidence
invariant (verify command is authority; Claude review is advisory signal) while
adding the judgment Codex-only closeouts lack.

### 4.3 Cross-harness session portability

Both harnesses now expose enumerable sessions with token/cost accounting. A
shared `SessionStore` (the SDK's @alpha adapter interface, with the reusable
conformance suite in `examples/session-stores/`) backed by our D1/SQLite means
fleet runs mixing Codex and Claude workers land in one queryable history with
uniform exact-token rows — the substrate the mobile companion and the fleet
cockpit both read. This is where the Orca-doc's "one status spine" and the
fan-out doc's FleetRun record meet the two-harness reality.

Khala Code Desktop now exposes the local version of this spine as a
schema-first `sessionCatalog` RPC. It merges the Codex local mapping file and
thread list with the Claude local mapping file and SDK `listSessions`, labels
each entry by harness, preserves public-safe refs/timestamps, and carries
per-session token totals only when the source explicitly reports exact totals.
The chat sidebar consumes that catalog so mixed Codex/Claude history renders in
one list with harness badges.

## 5. Effect-Wrapping Cheat Sheet (For The Implementer)

Per the integration audit's v4 baseline:

- **`query()` iterable → `Stream`** (`Stream.fromAsyncIterable`); the `Query`
  handle → `acquireRelease` (release: `interrupt` is graceful, `close()` +
  `controller.abort()` is teardown). Own the `AbortController`.
- **Control methods** (`setModel`, `setPermissionMode`, `setMcpServers`,
  `mcpServerStatus`, `supportedCommands/Models/Agents`, `getContextUsage`,
  `accountInfo`, `rewindFiles`, `interrupt`) → Effect service methods
  (`Effect.tryPromise`), gated behind a streaming-input `ClaudeSession` service.
- **Session functions** (`listSessions`, `getSessionMessages`, `forkSession`,
  …) → stateless service methods; candidates for a shared `SessionCatalog`.
- **`SDKMessage` union, `PermissionResult`, `PermissionUpdate`, hook I/O,
  `McpServerStatus`, `SDKSessionInfo`** → `Schema` (discriminated unions),
  decoded at the boundary.
- **`canUseTool`, hooks, `onElicitation`, `stderr`, `spawnClaudeCodeProcess`** →
  edge callbacks bridged via `Effect.runPromise` of a `Deferred`/`Queue`-backed
  service; wire `options.signal` to interruption.
- **`sessionStore`** → thin adapter whose methods `Effect.runPromise` our Effect
  data layer (D1/SQLite), validated by the SDK's conformance suite.

## 6. Invariants To Keep

- Isolated Claude homes only (`CLAUDE_CONFIG_DIR` per account, the `.claude-*`
  pattern); never touch the owner's live `~/.claude` for worker/fleet accounts,
  mirroring the Codex `~/.codex` rule.
- Owner-local full access (`bypassPermissions`) stays a local, visibly-labeled
  posture; it is never a public wire field. Approvals are Claude-native
  (`canUseTool`/hooks), not translated through Codex approval enums.
- Exact-only token accounting per harness (`pylon-claude-own-capacity` /
  `pylon-codex-own-capacity`); public counters remain projections; the SDK's
  `total_cost_usd`/`modelUsage` are recorded but never synthesized into the
  served-token counter.
- The deterministic `khala.fleet.delegate` program remains the control-flow
  authority for delegation; Claude planning supplies decomposition and review,
  not per-call control (the DSPy split the fan-out doc and episode 245 both
  teach).
- Version-pin discipline: pin the SDK, and update the Claude parity contract +
  gap matrix in the same change when bumping (the CHANGELOG moves ~daily).

## 7. Bottom Line

Claude is already a live *worker* lane at ~80% Codex parity in Pylon; the gap is
that Khala Code Desktop has no *chat harness* abstraction to host it. The
bring-up is therefore: introduce a `ChatRuntime` interface, land a
`ClaudeChatRuntime` as the desktop's first real Effect service (the SDK maps
beautifully onto Stream + service methods + Schema), and close parity surface by
surface behind a "Codex | Claude | Khala" composer pill — reusing the neutral
transcript event model that already exists and Pylon's Claude auth/telemetry
probes that already work. Once both harnesses share one app and one fleet
substrate, the payoff is the crossover the owner wants: **Fable-powered Claude
plans and reviews, Codex executes, the deterministic delegation program routes,
and the fleet runs them in parallel** — each harness doing what it is best at,
under one verified, exactly-accounted roof.
