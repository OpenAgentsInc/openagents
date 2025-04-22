
# OpenAgents Engine: System Overview & State Management (Post-Phase 3a)

## 1. Introduction

This document provides a holistic overview of the OpenAgents Engine project as it stands after the initial implementation phases focused on establishing a robust agent state management system. The primary goal of the engine is to create autonomous agents capable of analyzing and processing GitHub issues, performing actions based on that analysis, and managing their work over potentially long periods.

A core requirement is **persistence and recoverability**. Agents need to handle complex tasks that may take time, involve multiple steps, and potentially encounter errors or interruptions. The state management system implemented is the foundation for achieving this.

## 2. Core Architecture & Technology Stack

The engine is built using Node.js and leverages the Effect.js ecosystem for robust, type-safe, functional programming.

**Key Architectural Components:**

1.  **Server (`src/Server.ts`):**
    *   Provides the primary interface via a simple web UI (using HTMX).
    *   Handles incoming HTTP requests (e.g., to start processing an issue).
    *   Manages real-time communication with the frontend using Server-Sent Events (SSE) for status updates and analysis streaming.
    *   Orchestrates the initial triggering of the agent process.
2.  **Agent State (`AgentState` defined in `src/github/AgentStateTypes.ts`):**
    *   The central data structure holding the agent's entire operational context.
    *   Persisted as JSON files (`./state/{instance_id}.json`).
    *   Enables long-running tasks, error recovery, and detailed progress tracking.
3.  **GitHub Client (`src/github/GitHub.ts`):**
    *   An Effect.js service (`GitHubClient`) responsible for:
        *   Interacting with the GitHub REST API (fetching issues, comments, repos; creating comments; updating issues) using `@effect/platform/HttpClient`. Requires a `GITHUB_API_KEY`.
        *   Handling the persistence (save, load, create) of `AgentState` objects to the local filesystem, using the injected `@effect/platform/FileSystem` service.
4.  **State Management Services:**
    *   **`PlanManager` (`src/github/PlanManager.ts`):** Manages the `plan` array within the `AgentState`. Responsible for adding steps, updating step status (`pending`, `in_progress`, `completed`, `error`), recording timing, and associating tool calls with specific steps. Ensures immutability.
    *   **`TaskExecutor` (`src/github/TaskExecutor.ts`):** The core orchestrator for executing the agent's plan. Its `executeNextStep` method encapsulates the main loop: load state -> get current step -> update status -> perform action (currently simulated) -> update status/metrics/errors -> save state -> advance. Designed to handle errors gracefully and update state accordingly.
    *   **`ContextManager` (`src/github/ContextManager.ts`):** Manages the dynamic `execution_context` within the `AgentState`. Tracks relevant information gathered during execution, such as the current file being examined, relevant code snippets, links to related issues/PRs, and files modified during the session. Ensures immutability.
    *   **(Upcoming) `MemoryManager`:** Will manage the `memory` section (conversation history, decisions, findings, scratchpad).
5.  **AI Integration (Placeholder/Basic):**
    *   Basic integration exists in `Server.ts` to call the Anthropic Claude API for initial issue analysis (via HTTPS).
    *   `GitHubTools.ts` defines AI tool schemas intended for use with `@effect/ai`, wrapping `GitHubClient` methods. State integration into tools is planned.
    *   `Program.ts` outlines the structure for using Effect's AI capabilities but isn't the primary execution driver currently.
6.  **UI (`public/`):**
    *   Simple HTML (`index.html`), CSS (`styles.css`), and minimal JS (`sse.js`).
    *   Uses HTMX for server interactions and dynamic updates triggered by SSE events.
    *   Provides inputs for GitHub repo/issue and displays status/analysis streams. State display is basic and needs enhancement.

**Technology Stack:**

*   **Runtime:** Node.js
*   **Core Framework:** Effect.js (`effect`, `@effect/platform`, `@effect/platform-node`)
*   **Language:** TypeScript (strict mode)
*   **AI:** Anthropic Claude API (currently via HTTPS, planned via `@effect/ai-anthropic`)
*   **Real-time:** Server-Sent Events (SSE)
*   **Web UI:** HTMX, basic HTML/CSS
*   **Testing:** Vitest (`@effect/vitest`)

## 3. Agent State Management In-Depth

The state management system is central to the engine's design.

**3.1. Purpose:**

*   **Persistence:** Allows an agent's work on an issue to survive server restarts or crashes.
*   **Recoverability:** Enables resuming tasks from the last known good state after errors.
*   **Context:** Provides the agent with its history, plan, findings, and configuration, enabling more intelligent decision-making.
*   **Observability:** Offers a detailed record of the agent's actions, plan execution, tool usage, and errors for debugging and analysis.

**3.2. Structure (`AgentState`):**

Defined rigorously using `Schema.Class` in `src/github/AgentStateTypes.ts` and specified in `docs/agent-state.md`. Key sections include:

*   `agent_info`: Metadata about the agent instance and state schema version.
*   `timestamps`: Creation, last save, last action times.
*   `current_task`: Details of the primary GitHub issue being processed (repo, owner, number), cached details, overall status, and the index of the current plan step.
*   `plan`: An array of `PlanStep` objects detailing the sequence of actions the agent intends to take or has taken. Each step has an ID, description, status, timing, results, and associated tool calls.
*   `execution_context`: Dynamic information gathered during runtime (file focus, code snippets, references, modified files). Managed by `ContextManager`.
*   `tool_invocation_log`: A global, chronological log of all tools called by the agent.
*   `memory`: Stores conversation history, key decisions, important findings, and a scratchpad. Managed by `MemoryManager` (partially implemented).
*   `metrics`: Tracks performance data (time spent, LLM calls/tokens, tools used, steps completed).
*   `error_state`: Records the last error, consecutive errors, retry counts, and blocked reasons.
*   `configuration`: Agent's goal, LLM settings, allowed actions, timeouts, etc.

**3.3. Persistence:**

*   The `GitHubClient` service handles saving and loading state via its `saveAgentState` and `loadAgentState` methods.
*   It uses the injected `@effect/platform/FileSystem` service to interact with the filesystem.
*   State is serialized to JSON and saved in the `./state/` directory, named `{instance_id}.json`.
*   `createAgentStateForIssue` fetches initial issue details and creates/saves the starting state object.
*   State is saved strategically by the `TaskExecutor` after each step execution (success or failure) and potentially at other critical points.
*   Loading involves reading the JSON, parsing, and **validating** it against the `AgentStateSchema` to ensure type safety and structural integrity. Schema version mismatch triggers a warning.

## 4. Core Service Interactions & Workflow

The system operates around the `AgentState` and the services that manage it.

**Conceptual Workflow (Simplified):**

1.  **Trigger:** User submits repo/issue via the UI (`index.html`).
2.  **Request:** Server (`Server.ts`) receives the POST request (`/fetch-issue`).
3.  **State Initialization:**
    *   The server logic (or a dedicated orchestrator like `Program.ts` in the future) attempts to `loadAgentState` for the issue (using a generated or retrieved `instance_id`).
    *   If no state exists or loading fails non-recoverably, it calls `createAgentStateForIssue` (which uses `GitHubClient.getIssue` and `saveAgentState`).
    *   The resulting `AgentState` object becomes the initial context.
4.  **Execution Loop (`TaskExecutor.executeNextStep`):**
    *   Takes the current `AgentState`.
    *   Uses `PlanManager.getCurrentStep` to get the step defined by `current_task.current_step_index`.
    *   Uses `PlanManager.updateStepStatus` to mark the step `in_progress`.
    *   **(Action Phase - Currently Simulated):** Determines and executes the action required by the step description.
        *   *Future:* This will involve calling AI (passing relevant state context from `memory`, `execution_context`, `plan`), interpreting the response, and potentially calling `GitHubTools` handlers.
        *   *Future:* Tool calls will update state via `ContextManager` (e.g., adding snippets), `MemoryManager` (adding logs, findings), and `PlanManager` (adding tool calls to the step).
    *   **(Update Phase):** Based on action success/failure:
        *   Uses `PlanManager.updateStepStatus` to mark step `completed` or `error`, adding `result_summary`, updating timing.
        *   Updates `metrics` (LLM calls, time spent, etc.).
        *   If error, updates `error_state` fields.
        *   If success, increments `current_task.current_step_index`.
    *   **Save State:** Calls `GitHubClient.saveAgentState` to persist the *updated* `AgentState`.
    *   **Return:** Returns the *new* `AgentState` to the caller.
5.  **Iteration/Completion:** The caller (Server or Program) decides whether to call `executeNextStep` again based on the returned state's `current_task.status` and `current_step_index`, or if the task is complete/blocked.
6.  **UI Update:** Throughout the process, key state changes or results (step status, analysis chunks, errors) are broadcast via SSE (`Server.ts`) to the UI for real-time display.

**Immutability:** A core principle. State-modifying functions within services (`PlanManager`, `ContextManager`, `GitHubClient.saveAgentState`) always return *new* `AgentState` objects, preventing side effects and ensuring predictable state transitions.

## 5. Testing Strategy

*   **Schema Tests (`AgentStateTypes.test.ts`):** Verify that the schemas correctly decode valid data and reject invalid data, ensuring type safety at the boundaries (load/save).
*   **Unit Tests (`PlanManager.test.ts`, `ContextManager.test.ts`, etc.):** Test each service's methods in isolation. Use mock state objects as input and assert that the returned state has the expected changes and that immutability is preserved.
*   **Dependency Injection for Testing:** External dependencies like the filesystem are handled via injectable services (`FileSystem`). Tests provide *mock layers* (`Layer.succeed(FileSystem.FileSystem, mockImplementation)`) allowing verification without actual disk I/O or network calls, resolving previous mocking issues.
*   **Verification:** `pnpm verify` (running `tsc`, `eslint`, `vitest run`) is used to ensure code correctness, style adherence, and passing tests.

## 6. Current Status & Next Steps

**Implemented:**

*   Core `AgentState` structure and schemas (`AgentStateTypes.ts`).
*   State persistence (save/load/create) using `@effect/platform/FileSystem` DI (`GitHub.ts`).
*   `PlanManager` service and tests.
*   Basic `TaskExecutor` service and tests (with simulated work).
*   `ContextManager` service and tests.
*   Basic UI trigger and SSE streaming infrastructure (`Server.ts`).
*   Robust testing for implemented components (passing `pnpm verify`).

**Next Immediate Steps (Phase 3b & 4):**

1.  **Implement `MemoryManager`:** Create the service and tests for managing `memory` (`conversation_history`, `key_decisions`, `important_findings`, `scratchpad`) and `tool_invocation_log`.
2.  **Integrate State with AI/Tools:**
    *   Refactor `TaskExecutor` to call AI (using `@effect/ai`) instead of simulating work.
    *   Pass relevant `AgentState` context (plan, memory, context) into AI prompts.
    *   Process AI responses (text, tool calls) to update state using `PlanManager`, `MemoryManager`, `ContextManager`.
    *   Refactor `GitHubTools` handlers to be stateful (accept state, update logs/metrics, return updated state).
3.  **Enhance UI State Display:** Update `Server.ts` to broadcast more detailed state information (current plan/step, status, memory snippets, errors) via SSE. Update `index.html`/HTMX to display this information.
4.  **Error Handling/Recovery:** Implement retry logic in `TaskExecutor` based on `error_state`. Add checks on load to potentially resume from failed states.

This robust state management foundation enables the development of more sophisticated, resilient, and observable autonomous agents within the OpenAgents Engine.
