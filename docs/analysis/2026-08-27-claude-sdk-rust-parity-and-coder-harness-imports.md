# Consuming Claude via ACP, and imports for the Coder harness (model-agnostic)

Date: 2026-08-27
**Direction update (2026-08-27, later the same day):** owner decision — Claude consumption is **Rust-only**, through `crates/claude_agent_sdk`, tracking **upstream latest continuously**. We maintain the port and update it as soon as upstream ships. The §1.5 verdict below ("consume via the ACP adapter") is **overridden**; §1 remains as an interop study and its harness-import recommendations (§2) stand. See "Part 3 — the Rust SDK at parity" for the active plan and what 0.3.247 makes newly possible.

Sources inspected: `~/work/claude-agent-acp` (HEAD `14d192d1`, upstream-current, 0 commits behind origin), `~/work/claude-agent-sdk-typescript` (0.3.247), npm `@anthropic-ai/claude-agent-sdk@0.3.172` in this workspace, `crates/openagents-cli/src/{acp.rs,coder/}`, `~/work/projects/agentclientprotocol/repos/registry/claude-acp/agent.json`.

Three parts:

1. What `claude-agent-acp` is and what consuming Claude through ACP would need (interop study).
2. What the Claude Agent SDK (and the adapter built on it) does that our own Coder harness should import for all models, Claude or not.
3. The active direction: bring `crates/claude_agent_sdk` to wire parity with upstream latest, what the 0.3.247 surface contains, and how parity is checked (`scripts/check-claude-sdk-parity.sh`).

---

## Part 1 — Claude via ACP

### 1.1 What `claude-agent-acp` is

The official ACP adapter for the Claude Agent SDK, maintained by Zed Industries with Anthropic and JetBrains. Registry entry `claude-acp` (v0.70.0, npx distribution `@agentclientprotocol/claude-agent-acp@0.70.0`). It pins `@anthropic-ai/claude-agent-sdk@0.3.238` — upstream-current, 9 releases behind the 0.3.247 changelog at inspection time. Node ≥ 22, single ~16k-line TypeScript codebase with `acp-agent.ts` (9,573 lines) as the core.

Its feature list, from README and source:

- Context @-mentions, images, embedded context in prompts
- Tool calls with permission requests, editable choices, durable effects (`docs/permission-extension.md`)
- Session modes: `default`, `acceptEdits`, `auto`, `bypassPermissions`, `plan`, with per-model availability and an `auto → acceptEdits` fallback when a model lacks auto (`src/session-mode.ts:18`)
- Subagent sessions behind capability negotiation; legacy flattened transcripts otherwise
- Interactive and background terminals
- Custom slash commands (`available_commands_update`)
- Client MCP servers (http, sse)
- Provider-neutral experimental extensions: **goal** (`_session/goal`, session-scoped objective with Stop-hook backing), **session failure** (typed, durable warning/error transcript records with categories `connection/access/limit/request/service/unknown` and recovery actions `retry/login/new_session`), **permission presentation** (`_meta.permission` titles and fixed option ids like `allow-once`, `allow-with-updates`, `exit-plan-*`), async tasks, file-change audit reports
- Model configuration for alternate providers via `CLAUDE_MODEL_CONFIG` (Bedrock overrides, availability restriction), plus `providers/list` / `providers/set` / `providers/disable` and a gateway auth method (`_meta.gateway`, protocols `anthropic` and `bedrock`)
- Session lifecycle: new, load (resume), fork, list, close, delete; settings watched via the SDK's `resolveSettings` merge engine

The load-bearing fact: **the adapter owns the Claude-parity treadmill.** Its whole job is mapping a fast-moving proprietary protocol (the SDK changelog runs ~75 releases in 8 months) onto a stable, provider-neutral one. Whoever consumes Claude through it inherits Zed's maintenance, not the treadmill.

### 1.2 How OpenAgents consumes ACP today

One client: `AcpHarness` (`crates/openagents-cli/src/acp.rs`, ~700 lines, hand-rolled ndjson JSON-RPC over the child's stdio). Per run it:

1. spawns the agent binary into its own process group (`stop_tree` on every exit path),
2. sends `initialize` (protocolVersion 1, `clientCapabilities.fs` read/write false) with a 60 s handshake timeout,
3. opens `session/new` or reattaches `session/load` (gated on the agent's advertised `loadSession` capability),
4. best-effort `session/set_mode`,
5. sends one `session/prompt`, pumping the stream until the response,
6. handles `session/request_permission` inline — first allow-kind option, or a caller-supplied `PermissionGate` — and answers every other reverse request via an optional handler or `method not found`,
7. reports four event kinds to the caller: `Session`, `Tool {kind, title}`, `Tokens`, `Text`.

Used by `delegate` (the `agent` parameter, one child per call) and the Computer controller. Discovery (`coder/acp.rs`) reads the on-disk ACP registry, checks availability per platform/npx/uvx, and builds launch commands; `delegate`'s tool description names the discovered agents.

### 1.3 Compatibility matrix: adapter capability vs our client

| Adapter behavior | Our client today | Result |
|---|---|---|
| `initialize` → `authMethods` (subscription, console, gateway, bedrock) | We ignore the initialize response beyond the JSON-RPC result | Degrades: relies on ambient `~/.claude` credentials or `ANTHROPIC_API_KEY` in the child's env. Fine for local dev, wrong for headless servers. |
| `agentCapabilities.loadSession`, `sessionCapabilities` (resume/fork/list/close/delete) | `session/load` supported, session id captured in `AcpOutcome` | Partially works: resume plumbing exists in the harness, but no caller persists the claude-acp session id per thread today. |
| Session modes with ids `default / acceptEdits / auto / bypassPermissions / plan` | `PermissionMode::mode_id()` sends `bypass`, `default`, `read-only` | **Broken mapping**: `bypass` and `read-only` are not adapter mode ids. `set_mode` is best-effort so the wrong id is silently ignored and the session stays in default. `delegate --mode dangerous` does not do what it says against Claude. |
| `session/new` response carries `modes: {currentModeId, availableModes}` | We read only `sessionId` | Missing: the correct fix for the above is to read the advertised modes and map by name, not hardcode. |
| Permission options: `allow-once`, `allow-with-updates`, durable-rule options, `exit-plan-*` (multiple allow-kind options in one request) | `first_allow_option` picks the first allow-kind id; the gate picks first allow/reject kind | Degrades: against Claude, the picked option may be an exit-plan variant when the intent was allow-once. There is no notion of "durable effect" — a picked `allow-with-updates` applies an SDK `PermissionUpdate` we never surfaced to the user. |
| Streaming updates: `agent_message_chunk`, `agent_thought_chunk`, `tool_call`, `tool_call_update`, `plan`, `available_commands_update`, `current_mode_update`, `session_info_update`, `config_option_update` | We handle `agent_message_chunk`, `tool_call`, `usage_update` (a Devin `_meta` extension); all else ignored | Degrades quietly: no TODO/plan rendering, no slash commands, no mode changes. Claude's token usage arrives through `session/update` `usage_update` only if the adapter emits the cognition.ai `_meta` form — otherwise usage is unreported. |
| `promptCapabilities: {image, embeddedContext}` | Prompts are plain text | Degrades: capability is advertised by the agent, unused by us. |
| Client MCP servers in `session/new` (`mcpServers: []`) | Always empty | Missing: no way to give a Claude session our own tools over MCP. |
| Steering (`_session/steering`), goal extension, async tasks, session-failure extension, file-change reports | Not sent, not negotiated | Missing: deliberate for a first integration. Note our Coder has its own goal system (`coder/goal.rs`) with a different shape; do not conflate the two. |
| Cancellation expectations | We never send `session/cancel`; the cancel path kills the process group | Works (adapter shuts down on stdin EOF), but a killed child forfeits graceful transcript write-back. |
| Launch: registry `npx` entry with no args | `launch_for` appends a bare `acp` argument to any npx/uvx launch that lacks it | Harmless here (the adapter ignores unknown argv) but wrong in principle — `acp` is Devin's subcommand, not a convention. `is_available` npx detection handles scoped packages correctly. |
| Installation | `claude-agent-acp` not installed globally on this machine | Discovery reports it unavailable; nothing offers Claude until installed (`npm i -g @agentclientprotocol/claude-agent-acp`) or fetched via `npx -y`. |

### 1.4 What is needed — ordered

1. **Install/launch path** (small): ensure `claude-acp` resolves — global install or a guaranteed `npx -y` fetch. Stop appending the `acp` argument in `launch_for`; honor `agent.json` args exactly and let Devin's entry carry its own.
2. **Mode mapping** (small, correctness bug): read `modes.availableModes` from the `session/new` response; map our `Dangerous/Prompt/ReadOnly` onto advertised ids by name (`bypassPermissions`/`default`/`plan`), refusing a mode the session did not advertise instead of silently ignoring it. This is the only item that is a genuine bug today.
3. **Permission option selection** (small): prefer exact option ids (`allow-once`) with kind-prefix fallback; treat `exit-plan-*` as its own decision; when a gate is present, still record which option was picked so a durable effect is never applied invisibly.
4. **Session persistence** (medium): store `AcpOutcome.session_id` per Coder thread (we already do this pattern for other lanes), pass it as `resume_session_id` on the next delegate to the same thread. Resume turns one-shot children into a continuing collaborator and is the difference between "ran a command" and "has a Claude."
5. **Headless auth** (medium, product lever): for server-side use, choose one: (a) `ANTHROPIC_API_KEY` in the child's scrubbed env, or (b) advertise `auth._meta.gateway` and route the child through an Anthropic-protocol gateway we operate (the adapter explicitly supports this; `api_passthrough.rs` is adjacent infrastructure). (b) keeps billing inside OpenAgents and mirrors the delegated-child "own budget" rule.
6. **Richer stream handling** (medium): at minimum `plan` and `tool_call_update` so TUI boxes show status transitions; `usage_update` fallback for Claude's own token reporting; `available_commands_update` optional.
7. **Graceful cancel** (small): send `session/cancel` before `stop_tree`, keep the kill as the backstop.

Deliberately not taken yet: goal extension (our own goal model wins for now), async tasks, file-change audit, session-failure records, interactive terminals — all are client-side commitments that only pay off with a real Claude surface in the TUI, which is gated on 1–6 landing first.

### 1.5 Interop verdict (overridden by Part 3)

**Overridden — kept for the record.** This section originally concluded that the ACP adapter was the recommended consumption path for Claude. The owner decision is the opposite: Rust-only consumption through `crates/claude_agent_sdk`, maintained by us at upstream-latest parity (Part 3). What remains true and useful here:

- The adapter is the reference for how a well-built client *handles* the SDK: capability negotiation, mode availability fallbacks, permission option id discipline, extension design (goal, session failure, permission presentation). Copy its judgment, not its process.
- The registry entry `claude-acp` stays the interop path for *third-party* tooling that wants Claude (Zed, JetBrains, anything ACP-speaking) — that is not OpenAgents' own consumption path.
- §1.4's compatibility bugs in *our* ACP client (`launch_for` arg bug, `bypass`/`read-only` mode-id mapping, permission option selection) are real bugs against any claude-acp-using third party and remain worth fixing on their own merits.

---

## Part 2 — Imports into the Coder harness (model-agnostic)

What follows is ideas from the SDK/adapter worth reimplementing in `crates/openagents-cli/src/coder/` for every model, ordered by leverage. "Today" describes the harness as of this writing (`runtime.rs` `Control`, `turn.rs` `TurnState`, `delegate.rs`, `goal.rs`, `session_store.rs`).

### 2.1 Terminal-reason taxonomy

SDK result messages carry a structured `terminal_reason` (`api_error`, `budget_exhausted`, `malformed_tool_use_exhausted`, `turn_setup_failed`, `structured_output_retry_exhausted`, …) so a consumer never string-matches prose to learn why a turn died. Coder's `Control::Failed(String)` is freeform and `Done` carries no cause. Import: a `TurnOutcome` enum on every turn end — `completed | cancelled | failed(reason) | budget_exhausted | refused` — persisted with the turn record. This is the single highest-leverage import: fleet dashboards, retry policy, and postmortems all read it.

### 2.2 Queued prompts and steering

The SDK/adapter model: `prompt` submissions while a turn runs are **queued** (`queued_turn_count` in results), and **steering** injects into the running turn with priority `now` (pre-empt: the aborted cycle emits its own terminal, the injected message runs next) or `later` (respect in-flight permission/elicitation waits). Coder's `TurnState` is `Idle/Active/Canceling` with no queue and no injection; a second prompt during a turn is impossible by construction. Import: `Queue(TurnId)` and `Steer(TurnId, priority)` actions in the reducer, with the adapter's two safety rules — never interrupt a pending permission wait, and mark steered-over turns so their terminal settles correctly. Multi-turn delegation and interactive follow-ups fall out of this.

### 2.3 A permission handler as harness infrastructure

The SDK's `canUseTool` contract is worth copying wholesale: one async decision point before every tool execution, returning allow (with optional `updatedInput`), deny (with **feedback text the model sees**, so it adapts), or deny-and-interrupt; optional `updatedPermissions` for durable rules; modes layered above it (`default/acceptEdits/plan/dontAsk/auto`). Coder has refusals scattered per-tool and a `PermissionGate` only for ACP children. Import: a `PermissionHandler` trait in the tool registry with `Rules` (allow/deny lists), `Callback`, and mode plumbing; our own tools and delegated children answer to the same policy object. The deny-with-feedback semantics matter more than the rules: today a refused tool call is a dead end for the model.

### 2.4 Plan mode as a session mode

`plan` + `ExitPlanMode` + the adapter's exit-plan permission options form a complete, model-agnostic pattern: read-only research until the agent proposes a plan, an explicit human approval gate, then a mid-session mode switch (`set_permission_mode`) into execution. Nothing here is Claude-specific. Coder has neither plan gating nor mid-session mode changes (its lane/model are fixed at open). Import: mode as a session property, `/plan`, an `ExitPlanMode`-equivalent harness tool whose approval flows through 2.3.

### 2.5 File checkpointing and rewind

`enableFileCheckpointing` + `rewindFiles(userMessageId)` with `dryRun` and `skippedLinks` safety reporting: snapshot tracked files at each user message, restore on demand, report what was refused. Coder agents edit files across providers with no undo beyond git. Import: checkpoint at turn start (git-stash-based or content-addressed), a `rewind` command keyed by turn id, dry-run first. Cheap version: refuse-to-rewind list for paths outside the workspace.

### 2.6 Retry and degradation visibility

The SDK emits `api_retry` events (attempt counts, next delay), `model_fallback` notices with triggers (`overloaded`, `server_error`, `last_resort`), and rate-limit events with typed buckets; the adapter maps them into warning-severity records that do not end the turn. Coder's transport retries (if any) are invisible; a model fallback is not a concept it has. Import: `Control::Retry { attempt, delay_ms, cause }` and `Control::Degraded { from, to, reason }` so the TUI shows "retrying in 4s" instead of a frozen stopwatch, and a fallback lane (claude→codex) becomes expressible.

### 2.7 Structured turn receipts (`command_lifecycle`)

Every submitted message gets a uuid-stamped lifecycle: `queued → started → completed | cancelled | discarded`, with `terminal_reason` for dead turns. This survives restarts and deduplicates across transports ("dup-over-loss"). Coder's `TurnRouter` does generation fencing in-process only. Import: persist one lifecycle record per submitted prompt in `session_store.rs`; a resumed session can then answer "did my 14:02 prompt ever run?" — which fleet and mobile surfaces both need.

### 2.8 Background task control

`stop_task(taskId)` and `backgroundTasks(toolUseId)` (the Ctrl+B semantic: a blocking tool call returns immediately with "running in the background", the turn continues, a notification arrives on completion). Coder tools run strictly to completion in the turn. Import: backgroundable harness tools (tests, builds, long delegates) with a task registry, `stop_task`, and a `task_notification` control event. Start with delegate children — they already have cancel plumbing.

### 2.9 Subagent event threading and caps

`parent_tool_use_id` on every nested event, `forwardSubagentText`, `tool_use_summary` rollups, `agentProgressSummaries`, and the 0.3.217 caps (spawn depth default 1, concurrent subagents default 20). Coder's delegate returns end-of-run results with no live child stream in the frame and no depth/concurrency policy. Import: carry `parent_call_id` on `Control::Tool/ToolOutput` from delegated children (the harness already has a chunk-observation seam proposed in issue #228), plus explicit depth and concurrency limits in `ChildOptions`.

### 2.10 Structured output with retry accounting

`outputFormat: {type: json_schema, schema}` plus a distinct `error_max_structured_output_retries` terminal. Coder prompts are prose-in/prose-out; anything that needs a shape parses prose. Import: an optional response schema on harness calls, validated, with a bounded retry loop and its own terminal reason. Works over any provider that supports response_format, falls back to prompt-embedded schema otherwise.

### 2.11 Context budget telemetry

`getContextUsage()` (breakdown by system prompt, tools, messages, MCP tools), `compact_boundary` events with `pre_tokens`, and the `/context` structured payload. The TUI shows nothing about how full a session is until it fails. Import: a per-turn context estimate in `Control::Usage` (token counts already flow for some lanes) plus a compact-boundary event when the session is truncated, so long-running fleet sessions can be supervised.

### 2.12 Live settings merge with precedence

`applyFlagSettings` (mid-session, shallow-merge per key, null clears) over a defined precedence: flag > user/project/local > managed, with escalating-mode filtering from repo-committed files. Coder fixes configuration at open. Import: a `SetConfig` control path for model, effort, and permission mode with an explicit precedence doc — this is what makes 2.4 and effort knobs composable instead of ad hoc.

### 2.13 Smaller imports

- **Effort/thinking knobs**: `effort: low|medium|high|xhigh|max` and `thinking: adaptive|enabled(budget)|disabled` normalized in the proxy per provider. (2.12 is the transport.)
- **Capability advertisement**: the SDK init frame lists tools/commands/models and capability strings (`interrupt_receipt_v1`); the adapter's extensions are all opt-in via capability negotiation. Coder's client/server handshake should advertise what it supports rather than version-sniff.
- **Deny feedback vs cancellation**: the adapter's distinction — a selected reject returns feedback to the model, a cancelled permission aborts the tool use — is exactly right; copy it into both our gate and our own refusals.
- **Session-store adapter hygiene**: `sessionStoreFlush` batching and `loadTimeoutMs` (fail a resume rather than hang the iterator) — `session_store.rs` should adopt the timeout rule even if it never grows pluggable backends.

### 2.14 What not to import

Process-per-session spawning of a CLI (we are the harness), plugin/skill filesystem discovery, sandbox settings, and the browser/Remote Control surfaces are Claude-CLI architecture, not harness ideas. The goal extension is redundant with `coder/goal.rs` — if anything, borrow its *status* vocabulary (`active/paused/blocked/limited/complete`) into our `GoalStatus`, which already nearly matches.

---

## Sequencing

Superseded by Part 3's plan for the Claude path. What remains of the original sequencing:

1. **Harness quick wins** (§2.1, §2.6, §2.13 deny-feedback): typed turn outcomes, retry visibility. No schema churn beyond one enum.
2. **Permissions and modes** (§2.3, §2.4, §2.12): one policy object, plan mode, live config.
3. **Queue/steer/background** (§2.2, §2.8, §2.9): the interactive-tier work.
4. **Durability** (§2.5, §2.7, §2.10, §2.11): checkpointing, receipts, structured output, context telemetry.
5. **ACP client bug fixes** (§1.4 items 1–3): on their own merits for third-party interop, not as a Claude path.

## Part 3 — the Rust SDK at parity (the active direction)

### 3.1 The corrected version arithmetic

The port commit `230485aa74` (2025-12-11 07:37 −0600 = 13:37 UTC) lines up with the upstream CHANGELOG head at `7a4b371` (2025-12-11 04:39 UTC) = **0.1.65**, parity with Claude Code v2.0.66. Upstream versioning was 0.1.x then — the "0.3.150-era" estimate in issue #232 was wrong. The real gap is **0.1.65 → 0.3.247**: ~180 releases, ~8.5 months, three minor-version generations of protocol.

### 3.2 The decision

- `crates/claude_agent_sdk` is **the** Claude consumption path for OpenAgents, in Rust.
- Parity target is **upstream latest, continuously** — we update as soon as upstream ships; the changelog cadence (weekly-ish) is the pace.
- No new TypeScript consumption paths; no adapter intermediary.
- The parity check is mechanical: `scripts/check-claude-sdk-parity.sh [version]` fetches any npm version, extracts the wire surface from `sdk.d.ts`, and diffs it against the Rust protocol types. Exit 1 = behind, with the missing subtypes/modes listed. As of this writing against 0.3.247 it reports **48 missing wire subtypes** and the **`auto` permission mode** — that is the work list.

### 3.3 What the 0.3.247 surface contains (what parity buys)

Extracted from the 0.3.247 `sdk.d.ts` (8,415 lines, vs 6,460 at the 0.3.172 npm pin):

**38 `SDKMessage` variants** (Rust models 7). The 31 missing fall into five groups, each new capability for a Rust consumer:

- *Conversation structure*: `SDKAPIRetryMessage` (in-turn retry progress), `SDKThinkingTokensMessage`, `SDKToolUseSummaryMessage` (per-tool-group rollups), `SDKConversationResetMessage`, `SDKInformationalMessage`, `SDKControlRequestProgressMessage` (long control ops stream progress).
- *Tasks & background work*: `SDKTaskStarted/Updated/Progress/NotificationMessage`, `SDKBackgroundTasksChangedMessage` — the subagent/background-shell lifecycle a fleet UI needs.
- *Hooks & permissions*: `SDKHookStarted/Progress/ResponseMessage`, `SDKPermissionDeniedMessage`, `SDKElicitationCompleteMessage` — without these, hook-driven and elicitation-driven flows are invisible.
- *Session health*: `SDKSessionStateChangedMessage`, `SDKWorkerShuttingDownMessage`, `SDKCommandsChangedMessage`, `SDKNotificationMessage`, `SDKModelRefusalFallback/NoFallbackMessage`, `SDKRateLimitEvent`, `SDKPromptSuggestionMessage`.
- *Bookkeeping*: `SDKLocalCommandOutputMessage`, `SDKFilesPersistedEvent`, `SDKMemoryRecallMessage`, `SDKMirrorErrorMessage`.

**33 control request types** (Rust models 10, and its `Initialize` is never sent). New drivable operations: `apply_flag_settings` (live settings merge), `mcp_set_servers` (hot MCP topology), `stop_task`, `background_tasks`, `cancel_async_message`, `read_file`, `seed_read_state`, `rewind_files` with dry-run, `get_context_usage`, `get_session_cost`, `get_usage`, `list_models`, `reload_plugins`/`reload_skills`, `mcp_reconnect`/`mcp_toggle`/`mcp_call`/`mcp_message`, `rename_session`, `set_color`, `register_repo_root`, `file_suggestions`, `get_binary_version`, `request_user_dialog` (blocking dialogs the host renders), `elicitation`.

**Permission modes**: `auto` is new since the port (and `'manual'` is an accepted input alias); unrecognized modes are now rejected server-side (0.3.214), so an outdated mode enum is a hard error, not a degrade.

**Result fidelity**: `terminal_reason` taxonomy on results (budget exhaustion vs clean completion vs malformed tool use), `api_error_status` (429/529 structural detection), `queued_turn_count`, `usage` vs `modelUsage` semantics (the latter is cumulative and the cost-accounting field), per-model `canonicalModel`/`provider`/`costBasis`, `user_message_uuid` linking.

**Wire discipline changes** a Rust consumer must adopt: unrecognized `type` values must surface as typed data, not be dropped (the Rust reader currently logs-and-skips); `initialize` must be sent first and is idempotent (0.3.161); control responses can carry `pending_permission_requests`; steering-adjacent fields (`steeredEchoes`-style turn marking) exist for mid-turn injection.

### 3.4 What this enables for OpenAgents

With the crate at parity, a Rust consumer (Coder, pylon-core via FFI, the fleet) gets, without any TypeScript in the loop:

- **Full-fidelity Claude sessions in Rust**: streaming, hooks, subagents, background tasks, elicitation, dialogs — the whole 0.3.247 surface behind typed enums.
- **Cost and budget governance** from `modelUsage`/`get_session_cost`/`get_usage`: exact per-model cost attribution for delegated Claude work, matching how `credit.rs` governs our own lanes.
- **Structured turn outcomes** (`terminal_reason`) — the same taxonomy §2.1 proposes importing into the generic harness, arriving first here.
- **Live steering of running Claude turns**: `set_model`, `set_permission_mode`, `apply_flag_settings`, `cancel_async_message`, queued sends — the harness features in §2.2/§2.12 with a working transport behind them.
- **A template for other providers**: the message/control-request/option structure is exactly what a `codex_agent_sdk`-style crate would model; parity work here is reusable design.

### 3.5 Plan

Order is issue #232's, unchanged by the version arithmetic:

1. Protocol catch-up: all 38 message variants + typed unrecognized-message surfacing (`Error::UnrecognizedMessage{..}` on the stream, never silent drop).
2. Lifecycle: send `initialize` first, `initializationResult()`, control timeouts (the currently-unreachable `Error::ControlTimeout`).
3. Control surface: the remaining ~23 control requests, prioritizing `apply_flag_settings`, `mcp_set_servers`, `stop_task`, `get_context_usage`, `list_models`.
4. Options: wire `system_prompt`, `mcp_servers`, `agents`, `sandbox`, `plugins`, `output_format`, `fallback_model` into `build_args()` (they exist as dead fields today) and add `tools`, `thinking`/`effort`, `auto` mode.
5. Result fidelity: `terminal_reason`, `api_error_status`, `modelUsage` extensions.
6. Hold at parity: run `scripts/check-claude-sdk-parity.sh` after each upstream release; update and release. A scheduled watcher can automate the check later — the script is the whole interface.

## Verification

- Part 3: `scripts/check-claude-sdk-parity.sh` exits 0 against upstream latest; `cargo test -p claude_agent_sdk` green; a live smoke (spawn the installed `claude` CLI: init handshake → prompt with `include_partial_messages` → interrupt → close) records zero unrecognized `type` values.
- §2: each import lands with a reducer or registry unit test in `crates/openagents-cli` (the existing pattern: `turn.rs` tests name the invariant, not the implementation).
- ACP client fixes (§1.4 items 1–3), when taken: `cargo test -p openagents-cli` plus `pnpm run check:coder-surfaces` if tool descriptions change.

## References

- Upstream SDK: `~/work/claude-agent-sdk-typescript` (git clone of `anthropics/claude-agent-sdk-typescript`, 0.3.247); wire surface extractable from any version via `npm pack @anthropic-ai/claude-agent-sdk@<v>`
- Parity check: `scripts/check-claude-sdk-parity.sh` (added with this document)
- Adapter (interop reference): `~/work/claude-agent-acp` — `src/acp-agent.ts`, `src/tools.ts`, `src/session-mode.ts`, `docs/{goal,permission,session-failure}-extension.md`
- Our side: `crates/claude_agent_sdk/`, `crates/openagents-cli/src/acp.rs`, `src/coder/{runtime,turn,goal,acp}.rs`, `src/delegate.rs`
- Related: issue #232 (Rust SDK gap analysis — the work list), issue #228 (acp/delegate tool consolidation)
