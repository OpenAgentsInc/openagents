# Orchestration Audit and Spec

Current Capabilities
- Explore orchestrator: On-device FM path with optional tool-calling loop (gated to iOS/macOS 26). Streams ACP updates (`plan`, `tool_call`, `tool_call_update`, agent chunks).
- Setup orchestrator: Conversational config creation via `orchestrate/setup.*`, persists to Tinyvex, maps chat session to setup state, streams updates to the user’s session.
- Config RPCs: `orchestrate/config.(get|set|list|activate)` implemented. Activation caches the config in-memory.
- Scheduler service: Actor computes next wake from cron-like schedule; RPCs: `orchestrate/scheduler.(status|reload|run_now|advance)`; `run_now` triggers an Explore orchestration and streams updates.
- UI: iOS Chat “Start” button sends `orchestrate.explore.start` and shows plan + tool updates inline.

Key Gap
- “Delegation to agent like Codex” is implemented in the AgentCoordinator + providers path, but not yet exposed via the orchestration RPC surfaces. Current JSON-RPC orchestration entry points don’t select a provider or run the decision engine; `run_now` calls ExploreOrchestrator directly.

Specification: Programmatic + UI Orchestration That Delegates to Agents

1) JSON-RPC Surface
- orchestrate/coordinator.run_once
  - params: `{ config_id?: string, config_inline?: OrchestrationConfig }`
  - result: `{ session_id: string, task_id: string, agent_mode: string, status: string }`
  - behavior: Loads config (active or inline), invokes AgentCoordinator.runCycle(config:), enqueues/executes decision, and starts the selected provider (Codex/Claude). Streams ACP updates to `session_id` via SessionUpdateHub.

- orchestrate/coordinator.status
  - result: `{ cycles_run: number, tasks_executed: number, tasks_completed: number, tasks_failed: number, tasks_cancelled: number, last_cycle_ts?: number }`
  - behavior: Returns metrics from AgentCoordinator.

- orchestrate/scheduler.bind
  - params: `{ config_id: string }`
  - result: `{ ok: boolean }`
  - behavior: Sets `activeOrchestrationConfig` and (re)wires scheduler’s trigger to call `coordinator.run_once`.

2) Desktop Integration
- DesktopWebSocketServer
  - Maintain a single AgentCoordinator actor: constructed with `TaskQueue`, `DecisionEngine`, `agentRegistry`, and the server’s `updateHub`.
  - Implement the three new methods above in `+Orchestration.swift`. For `run_once`, create a fresh `ACPSessionId` and pass its updates to the hub.
  - Update `orchestrate/scheduler.reload` and `run_now` to route to `coordinator.run_once` instead of ExploreOrchestrator.

3) UI Integration
- iOS Chat
  - Add a second action beside “Start” that triggers `coordinator.run_once` (e.g., “Run Overnight Plan”).
  - Surface status from `coordinator.status` in a lightweight badge or inspector panel.

- macOS Developer/Console
  - In OrchestrationConsole, add buttons for Activate + Run Now that call `orchestrate/scheduler.bind` then `coordinator.run_once`.
  - Show scheduler status and next wake inline with the active config.

4) Streaming/UX Consistency
- Continue to emit tool calls/results (codex.run, fs.*) as ACP `tool_call`/`tool_call_update` so the chat UI renders uniformly whether updates originate from Explore or a provider.
- When decisions are made, emit a brief plan update that includes the chosen task and agent mode, so users see intent before the provider’s first output.

5) Guardrails and Policies
- Respect `OrchestrationConfig.AgentPreferences` to choose between Codex/Claude with allowlist/prefer enforcement.
- Keep FM-only exploration available as a separate `orchestrate.explore.*` path (good for E2E demos); the coordinator RPCs form the “overnight automation” path.

6) Tests
- Add integration tests that call `coordinator.run_once` and assert:
  - A session_id is returned and ACP updates are streamed (plan + tool calls)
  - The selected provider matches config preferences
  - TaskQueue state transitions (pending → in_progress → completed/error)

