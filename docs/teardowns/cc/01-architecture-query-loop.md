# Teardown 01 — Architecture, Lifecycle, Query Loop & Entrypoints

**Claude Code** (`~/work/projects/repos/cc`) vs **OpenAgents Coder** (`packages/openagents-cli/src`)

## Component / Subsystem Breakdown

| Concern | Claude Code | OpenAgents Coder |
|---|---|---|
| Bootstrap | `entrypoints/cli.tsx` (302 ln) | `main.ts` (70 ln) → `cli.ts` (4,599 ln) |
| App shell | `main.tsx` (4,683 ln) + `replLauncher.tsx` + ink TUI | `runtime.ts` (Effect Layer graph) + `coder-ui.ts` (raw ANSI) |
| Query loop | `query.ts` (1,729 ln), `QueryEngine.ts` (1,295 ln) | `coder-thread.ts` (1,195 ln), `coder-session.ts` (1,308 ln) |
| Tool execution | `services/tools/StreamingToolExecutor.ts` + `utils/queryHelpers.ts` | inline in `coder-thread.ts` (`merge()` fan-out) |
| Sub-agents | `Task.ts` + `tasks/*` (6 task types) | `coder-delegate.ts` (1,994 ln) + `coder-child-gateway.ts` |
| Persistence | `utils/sessionStorage.ts` (local JSONL) | `coder-transcript.ts` (server-owned threads API) |

## Claude Code Implementation Details

### Entrypoints

`entrypoints/cli.tsx` sets `COREPACK_ENABLE_AUTO_PIN=0`, raises heap for remote containers, applies ablation-baseline env vars **at module scope** (BashTool/AgentTool capture them into module consts at import time — `init()` would be too late), then `main()` dispatches through dynamic imports so `--version` costs zero module evaluations. Fast paths exist for `--dump-system-prompt`, Chrome native host, `--computer-use-mcp`, `cc://` URL rewrite, and macOS deep links (detected via LaunchServices' overwritten `__CFBundleIdentifier`). `main.tsx:585 main()` then does Windows PATH-hijack defense (`NoDefaultCurrentDirectoryInExePath`), SIGINT routing that defers to print-mode handlers, and renders `screens/REPL.tsx` (5,005 ln) through a lazy-loaded ink tree. Compile-time dead-code elimination comes from `feature('FLAG')` (`bun:bundle`) — gated strings are physically absent from external builds — while *runtime* gates (statsig, env) are snapshotted once per query in `query/config.ts:buildQueryConfig()`. A second entrypoint family, `entrypoints/sdk/` (`agentSdkTypes.ts`, `coreSchemas.ts`, `controlSchemas.ts`), exposes the same loop as an embeddable SDK, plus `entrypoints/mcp.ts` for MCP-server mode.

### The query loop state machine

`query.ts:219 query()` wraps `queryLoop()`, an `AsyncGenerator<StreamEvent|Message|TombstoneMessage, Terminal>`. Three state classes are deliberately separated:

1. **Immutable params** — destructured once (`systemPrompt`, `canUseTool`, `fallbackModel`, `maxTurns`…). Never reassigned.
2. **Mutable cross-iteration `State` struct** — `messages`, `autoCompactTracking`, `maxOutputTokensRecoveryCount`, `hasAttemptedReactiveCompact`, `turnCount`, `pendingToolUseSummary`, `transition`. Continue sites write `state = {...state, x}` instead of nine loose assignments.
3. **`QueryConfig` snapshot** — sessionId + gates frozen at entry, so "a pure reducer can take (state, event, config)".

Each iteration: content-replacement budget enforcement → **microcompact** (operates purely by `tool_use_id`, invisible to prompt cache, composes with cached-MC) → **context-collapse projection** (read-time replay of a commit log; runs *before* autocompact so granular context survives if collapse alone gets under threshold) → **autocompact** with a `consecutiveFailures` circuit breaker → streaming model call (`deps.callModel`) inside a `while(attemptWithFallback)` retry wrapper → `StreamingToolExecutor` drains tool_use blocks → results appended → loop while any `toolUseBlock`s existed → `handleStopHooks` (a stop hook can veto completion and inject another iteration).

The `Terminal` return type enumerates exits: `'completed' | 'blocking_limit' | 'image_error' | 'model_error' | 'aborted_streaming' | 'prompt_too_long' | 'stop_hook_prevented'`, plus *transition* reasons (`max_output_tokens_escalate`, `reactive_compact_retry`, `collapse_drain_retry`) that mutate state and `continue` instead of returning. `task_budget.remaining` is carried across compaction boundaries so the server can count spend after history is summarized away.

### Supporting machinery

- **`StreamingToolExecutor`**: tracks each block as `queued→executing→completed→yielded`; concurrency-*safe* tools run in parallel, non-concurrent tools take exclusive access with order preserved; results buffer and emit in arrival order; a `siblingAbortController` kills sibling subprocesses on Bash error without aborting the parent turn; `discard()` drops stale executors after a mid-stream model fallback so orphaned `tool_results` can't be yielded.
- **DI seam**: `query/deps.ts` — `{callModel, microcompact, autocompact, uuid}`, typed as `typeof fn` so signatures stay synced; scope "intentionally narrow to prove the pattern."
- **`QueryEngine.ts`**: class owning conversation lifecycle (messages, readFileState cache, usage totals, permission denials, discovered skills) for the SDK/headless path; `ask()` at :1186 wraps it. `queryTracking.chainId/depth` tags every analytics event.
- **Tasks** (`Task.ts`): `TaskType = local_bash | local_agent | remote_agent | in_process_teammate | local_workflow | monitor_mcp | dream`; `TaskStatus = pending|running|completed|failed|killed` with `isTerminalTaskStatus` guarding dead-teammate message injection.
- **Prefetch overlap**: memory prefetch opened with `using` (disposed on all generator exits), skill discovery started per iteration and consumed post-tools — I/O hidden under the stream.

## OpenAgents Coder Implementation State

Entry is Effect-native: `cli.ts` defines 58 `Command.make` subcommands under a tagged `CliError` union (JSON error output with exit codes via `--json`), wired to `NodeRuntime.runMain` over `runtime.ts`'s composition of ~25 Layers (fetch transport + network policy, OS keychain credentials, git runner, computer-pairing stack). The `coder` command (`cli.ts:1896`) takes prompt/plain/offline/dev/resume/reasoning/model/child/concurrency flags; `--dev` auto-starts a local dev server behind a spinner.

There is **no monolithic query loop**. The closest analog:

- **`ReplySource`** (`coder-session.ts:283`): `reply(prompt, signal): AsyncIterable<ReplyChunk>` plus optional `steer()`, `cycleBackend()`, `history()`, `cycleReasoning()`, `useTools()`, `describeContext()`. `DummyReplySource` exercises every chunk kind offline.
- **`ThreadReplySource.reply()`** (`coder-thread.ts:562+`): per-turn budget reset → retrieval → steps until text-only answer or `MAX_TOOL_STEPS = 100` (then a forced `mustAnswer` user message: "answer now… say plainly what is still unfinished"). All calls in a round run concurrently via `merge()`; results push back in call order. Steering splices queued prompts into the wire transcript mid-turn and yields a `steered` chunk so the UI relocates the reader's entry. `finally { await this.refresh() }` reads spend even on interrupt.
- **`CoderSession.run()`** (`coder-session.ts:1025`): owns entries (`assistant|reasoning|tool|you` with `settled` flags), withdraws an empty opening caret entry when the turn opens with reasoning/tool use, routes chunks to entries, drains the pending queue (`submit(mode: "steer"|"queue")`).
- **Fleet** (`coder-delegate.ts`): `DelegateHarness` implemented by `SelfHarness`, `DevinHarness` (ACP), `ClaudeCodeHarness`, `CodexHarness`, `OpencodeHarness`. Fleet caps concurrency, refuses `fleet_full` *before* queue overflow, registers children as visible `pending` tasks, writes JSONL transcripts to a `0700` tmpdir, retries with `resumeSessionId`. **`coder-child-gateway.ts`** lends the session's thread grant over a loopback HTTP endpoint, flattening tool exchanges to plain turns (the proxy can't carry paired `function_call`/`_output` items) and never exposing the token to the child.
- **Transcript** (`coder-transcript.ts`): append-only POST to `/api/v1/threads/{id}/events`, background pump with backoff, "must never cost the session anything" — enqueue is sync, persistent failure surfaces one notice, never throws into the loop. Vocabulary: `turn.user | turn.reasoning | tool.ran | turn.assistant`.

## Detailed Gap Analysis

1. **No context management.** CC has five cooperating mechanisms (content-replacement budget, microcompact, autocompact+circuit-breaker, collapse projection, snip tombstones) plus cross-compact budget accounting. Coder has *none* — grep finds zero compaction logic. A long session walks into the provider context ceiling with no recovery, and `MAX_TOOL_STEPS=100` rounds will get there quickly.
2. **Loop robustness.** CC recovers from mid-stream model fallback (tombstoning orphans, discarding executors), escalates `maxOutputTokens`, resizes images, detects prompt-too-long, and lets stop hooks force continuation. Coder's failure mode is `throw ThreadUnavailable` → fleet-level retry with resume; there is no in-loop degradation, no fallback model, no partial-result salvage.
3. **No formal terminal state.** CC's `Terminal` union drives lifecycle notifications, analytics, and retry policy. Coder's `reply()` just ends; callers cannot distinguish "answered," "hit budget," "provider died," or "user aborted" except by exception type.
4. **Concurrency semantics.** CC classifies tools safe/exclusive and preserves result order under parallelism; Coder runs every round fully parallel — correct for its current read-only-ish toolset, unsafe once write tools contend (child 2's territory, but the loop is where it must be enforced).
5. **Testing/DI seams.** CC's `deps` injection makes the 1,700-line loop unit-testable without network. Coder's model call and budget refresh are hardwired in `ThreadReplySource`; tests lean on `DummyReplySource`, which exercises rendering, not loop policy.
6. **Entrypoint surface.** CC ships SDK (`ask()`/`QueryEngine`), headless `-p` with stream-json, MCP serve, deep links. Coder ships TUI + `--plain` + one-shot prompt; `ReplySource` is SDK-shaped but not exported as a stable library surface, and there is no machine-readable output mode.
7. **Trade-off worth keeping:** Coder's server-owned transcript and grant-lending gateway are *architecturally better* than CC's local-file persistence for cost control and multi-machine resume. Do not port CC's persistence model; port only the loop's self-management.

## Actionable Porting Recommendations

1. **Add a compaction stage to `ThreadReplySource.reply()`** before each step: estimate tokens; past a threshold, summarize the oldest completed steps into one injected system note and drop their wire messages (the server thread retains raw `tool.ran`/`turn.*` events, so nothing is lost durably — this is cheaper for Coder than for CC precisely because history lives server-side). Start with tool-output elision keyed by callId — the exact microcompact trick.
2. **Give `ReplySource.reply()` a terminal value**, not just termination: `AsyncIterable<ReplyChunk, TurnTerminal>` with `reason: 'completed'|'budget'|'provider_error'|'aborted'|'max_steps'`. Map these onto `CoderSession` notices and fleet retry decisions, mirroring `query.ts`'s Terminal/transition split.
3. **Extract `threadDeps`** (`callModel`, `refreshBudget`, `uuid`) out of `ThreadReplySource`, typed `typeof fn` à la `query/deps.ts`, so loop policy is testable without a proxy.
4. **On provider failure, fall back before failing**: catch stream errors in-loop and offer the existing tier-cycling machinery as an automatic downgrade (one retry), reserving `ThreadUnavailable` for exhaustion.
5. **Port executor discipline**: extend `CoderTool` with `concurrencySafe: boolean`; in the round runner, run safe tools via `merge()`, serialize unsafe ones, and emit results in call order regardless of finish order.
6. **Post-turn validators (stop-hook analog)**: an optional `validate?: (turn) => 'accept'|{inject: string}` on `ReplySource`, enabling checks like "export file exists" that can force exactly one more step.
7. **Headless SDK mode**: export `openThread`/`CoderSession` from package root and add `openagents coder --print --output-format json` emitting the `ReplyChunk` stream — this unlocks CI/embedding use with near-zero new logic, since the loop is already an AsyncIterable.
