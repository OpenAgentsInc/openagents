**Objective:** Fully define the TypeScript types and corresponding Effect `Schema.Class` definitions for the entire `AgentState` structure, strictly adhering to the specification outlined in `docs/agent-state.md`. This involves creating or updating classes for every field and nested object described in the example JSON.

**Target File:** `src/github/AgentStateTypes.ts`

**Source of Truth:** The JSON structure documented in `docs/agent-state.md`.

**Instructions for the AI Coding Agent:**

1.  **Setup:**
    *   Ensure the file `src/github/AgentStateTypes.ts` exists.
    *   Add the necessary import at the top: `import { Schema } from "effect";`
    *   Review the existing content of the file. You will be adding new classes and potentially modifying existing ones to match the specification precisely.

2.  **Define Schemas Section-by-Section:** Go through the `docs/agent-state.md` JSON structure field by field. For each top-level key and any nested objects, create a corresponding `Schema.Class`.

    *   **`AgentInfo` Class:**
        *   Define `export class AgentInfo extends Schema.Class<AgentInfo>("AgentInfo")({...})`.
        *   Include fields:
            *   `type`: `Schema.String` (Example value: "solver")
            *   `version`: `Schema.String` (Example value: "1.0.0")
            *   `instance_id`: `Schema.String` (Example value: "solver-uuid-of-issue-session-id")
            *   `state_schema_version`: `Schema.String` (Example value: "1.1" - Use the exact version from the spec).
        *   Add a JSDoc comment explaining this class holds agent metadata.

    *   **`Timestamps` Class:**
        *   Define `export class Timestamps extends Schema.Class<Timestamps>("Timestamps")({...})`.
        *   Include fields (all should be ISO 8601 date strings):
            *   `created_at`: `Schema.String`
            *   `last_saved_at`: `Schema.String`
            *   `last_action_at`: `Schema.String`
        *   Add a JSDoc comment explaining this class holds relevant timestamps.

    *   **`IssueDetailsCache` Class:** (Used within `CurrentTask`)
        *   Define `export class IssueDetailsCache extends Schema.Class<IssueDetailsCache>("IssueDetailsCache")({...})`.
        *   Include fields:
            *   `title`: `Schema.String`
            *   `description_snippet`: `Schema.String`
            *   `status`: `Schema.String` (Example value: "open")
            *   `labels`: `Schema.Array(Schema.String)`
            *   `source_url`: `Schema.String`
        *   Add a JSDoc comment.

    *   **`CurrentTask` Class:**
        *   Define `export class CurrentTask extends Schema.Class<CurrentTask>("CurrentTask")({...})`.
        *   Include fields:
            *   `repo_owner`: `Schema.String`
            *   `repo_name`: `Schema.String`
            *   `repo_branch`: `Schema.String`
            *   `issue_number`: `Schema.Number`
            *   `issue_details_cache`: `Schema.Union(IssueDetailsCache, Schema.Null)` (Reference the `IssueDetailsCache` class defined above. The spec shows it present, but it's marked optional for flexibility).
            *   `status`: `Schema.String` (Spec mentions: `idle | planning | researching | implementing | testing | blocked | completed | error`. Consider `Schema.Literal(...)` if these are strictly enforced, otherwise `Schema.String` is acceptable based on the spec example). Let's use `Schema.String` for now to match the flexibility implied.
            *   `current_step_index`: `Schema.Number` (0-based index).
        *   Add a JSDoc comment explaining this holds the agent's current focus.

    *   **`ToolCall` Class:** (Used within `PlanStep` and `tool_invocation_log`)
        *   Define `export class ToolCall extends Schema.Class<ToolCall>("ToolCall")({...})`.
        *   Include fields:
            *   `timestamp`: `Schema.String` (ISO 8601)
            *   `tool_name`: `Schema.String`
            *   `parameters`: `Schema.Struct({})` (Represents an arbitrary JSON object of parameters. `{}` allows any fields).
            *   `status`: `Schema.String` (Example values: "success", "error")
            *   `result_preview`: `Schema.String` (A short representation of the result).
            *   `full_result_ref`: `Schema.Union(Schema.String, Schema.Null)` (Optional reference to externally stored full result).
        *   Add a JSDoc comment explaining this represents a logged tool invocation.

    *   **`PlanStep` Class:** (Used within `plan`)
        *   Define `export class PlanStep extends Schema.Class<PlanStep>("PlanStep")({...})`.
        *   Include fields:
            *   `id`: `Schema.String` (Unique step identifier).
            *   `step_number`: `Schema.Number` (1-based index).
            *   `description`: `Schema.String`
            *   `status`: `Schema.String` (Spec mentions: `pending | in_progress | completed | skipped | error`. Use `Schema.String` for now, or `Schema.Literal(...)` if strictly enforced).
            *   `start_time`: `Schema.Union(Schema.String, Schema.Null)` (ISO 8601).
            *   `end_time`: `Schema.Union(Schema.String, Schema.Null)` (ISO 8601).
            *   `result_summary`: `Schema.Union(Schema.String, Schema.Null)`.
            *   `tool_calls`: `Schema.Array(ToolCall)` (Reference the `ToolCall` class defined above).
        *   Add a JSDoc comment.

    *   **`FileFocus` Class:** (Used within `ExecutionContext`)
        *   Define `export class FileFocus extends Schema.Class<FileFocus>("FileFocus")({...})`.
        *   Include fields:
            *   `path`: `Schema.String`
            *   `relevant_lines`: `Schema.Array(Schema.Number)`
        *   Add a JSDoc comment.

    *   **`CodeSnippet` Class:** (Used within `ExecutionContext`)
        *   Define `export class CodeSnippet extends Schema.Class<CodeSnippet>("CodeSnippet")({...})`.
        *   Include fields:
            *   `file_path`: `Schema.String`
            *   `snippet`: `Schema.String`
            *   `reason`: `Schema.String`
        *   Add a JSDoc comment.

    *   **`ExternalReference` Class:** (Used within `ExecutionContext`)
        *   Define `export class ExternalReference extends Schema.Class<ExternalReference>("ExternalReference")({...})`.
        *   Include fields:
            *   `type`: `Schema.String` (e.g., "issue", "pr")
            *   `identifier`: `Schema.String` (e.g., "121", "45")
            *   `relationship`: `Schema.String` (e.g., "relates_to", "blocked_by")
            *   `source`: `Schema.String` (e.g., "github")
        *   Add a JSDoc comment.

    *   **`ExecutionContext` Class:**
        *   Define `export class ExecutionContext extends Schema.Class<ExecutionContext>("ExecutionContext")({...})`.
        *   Include fields:
            *   `current_file_focus`: `Schema.Union(FileFocus, Schema.Null)` (Reference `FileFocus`).
            *   `relevant_code_snippets`: `Schema.Array(CodeSnippet)` (Reference `CodeSnippet`).
            *   `external_references`: `Schema.Array(ExternalReference)` (Reference `ExternalReference`).
            *   `files_modified_in_session`: `Schema.Array(Schema.String)`.
        *   Add a JSDoc comment explaining this holds dynamic context during execution.

    *   **`ConversationToolCall` Class:** (Used within `ConversationMessage`)
        *   Define `export class ConversationToolCall extends Schema.Class<ConversationToolCall>("ConversationToolCall")({...})`. *Note:* This structure is slightly different from the logged `ToolCall`.
        *   Include fields based on the example `{"id": "tool-call-1", "name": "fetchFileContents", "input": { "path": "src/main.ts" }}`:
            *   `id`: `Schema.String`
            *   `name`: `Schema.String`
            *   `input`: `Schema.Struct({})` (Represents the parameters passed to the tool).
        *   Add a JSDoc comment explaining this represents a tool call within the AI conversation flow.

    *   **`ConversationMessage` Class:** (Used within `Memory`)
        *   Define `export class ConversationMessage extends Schema.Class<ConversationMessage>("ConversationMessage")({...})`.
        *   Include fields:
            *   `role`: `Schema.String` (Example values: "user", "assistant", "tool")
            *   `content`: `Schema.String`
            *   `timestamp`: `Schema.String` (ISO 8601)
            *   `tool_calls`: `Schema.Union(Schema.Array(ConversationToolCall), Schema.Null)` (Reference `ConversationToolCall`. Note: The spec example implies this can be null or missing for user/assistant text messages, but present for tool usage messages).
        *   Add a JSDoc comment.

    *   **`KeyDecision` Class:** (Used within `Memory`)
        *   Define `export class KeyDecision extends Schema.Class<KeyDecision>("KeyDecision")({...})`.
        *   Include fields:
            *   `timestamp`: `Schema.String` (ISO 8601)
            *   `decision`: `Schema.String`
            *   `reasoning`: `Schema.String`
            *   `confidence`: `Schema.Number` (Value between 0 and 1).
        *   Add a JSDoc comment.

    *   **`ImportantFinding` Class:** (Used within `Memory`)
        *   Define `export class ImportantFinding extends Schema.Class<ImportantFinding>("ImportantFinding")({...})`.
        *   Include fields:
            *   `timestamp`: `Schema.String` (ISO 8601)
            *   `finding`: `Schema.String`
            *   `source`: `Schema.String` (Example value: "code_analysis")
            *   `confidence`: `Schema.Number` (Value between 0 and 1).
        *   Add a JSDoc comment.

    *   **`Memory` Class:**
        *   Define `export class Memory extends Schema.Class<Memory>("Memory")({...})`.
        *   Include fields:
            *   `conversation_history`: `Schema.Array(ConversationMessage)` (Reference `ConversationMessage`).
            *   `key_decisions`: `Schema.Array(KeyDecision)` (Reference `KeyDecision`).
            *   `important_findings`: `Schema.Array(ImportantFinding)` (Reference `ImportantFinding`).
            *   `scratchpad`: `Schema.String`.
        *   Add a JSDoc comment explaining this holds the agent's memory components.

    *   **`LLMTokensUsed` Class:** (Used within `Metrics`)
        *   Define `export class LLMTokensUsed extends Schema.Class<LLMTokensUsed>("LLMTokensUsed")({...})`.
        *   Include fields:
            *   `prompt`: `Schema.Number`
            *   `completion`: `Schema.Number`
        *   Add a JSDoc comment.

    *   **`Metrics` Class:**
        *   Define `export class Metrics extends Schema.Class<Metrics>("Metrics")({...})`.
        *   Include fields:
            *   `steps_completed`: `Schema.Number`
            *   `total_steps_in_plan`: `Schema.Number`
            *   `session_start_time`: `Schema.String` (ISO 8601)
            *   `total_time_spent_seconds`: `Schema.Number`
            *   `llm_calls_made`: `Schema.Number`
            *   `llm_tokens_used`: `LLMTokensUsed` (Reference `LLMTokensUsed`).
            *   `tools_called`: `Schema.Number`
            *   `commits_made`: `Schema.Number`
        *   Add a JSDoc comment explaining this holds performance and usage metrics.

    *   **`LastError` Class:** (Used within `ErrorState`)
        *   Define `export class LastError extends Schema.Class<LastError>("LastError")({...})`.
        *   Include fields:
            *   `timestamp`: `Schema.String` (ISO 8601)
            *   `message`: `Schema.String`
            *   `type`: `Schema.Literal("api_error", "tool_error", "internal")` (Use Literal based on spec description).
            *   `details`: `Schema.String` (Can contain stack trace or more info).
        *   Add a JSDoc comment.

    *   **`ErrorState` Class:**
        *   Define `export class ErrorState extends Schema.Class<ErrorState>("ErrorState")({...})`.
        *   Include fields:
            *   `last_error`: `Schema.Union(LastError, Schema.Null)` (Reference `LastError`).
            *   `consecutive_error_count`: `Schema.Number`
            *   `retry_count_for_current_action`: `Schema.Number`
            *   `blocked_reason`: `Schema.Union(Schema.String, Schema.Null)`.
        *   Add a JSDoc comment explaining this holds information about errors encountered.

    *   **`LLMConfig` Class:** (Used within `Configuration`)
        *   Define `export class LLMConfig extends Schema.Class<LLMConfig>("LLMConfig")({...})`.
        *   Include fields:
            *   `model`: `Schema.String`
            *   `temperature`: `Schema.Number`
            *   `max_tokens`: `Schema.Number`
        *   Add a JSDoc comment.

    *   **`Configuration` Class:**
        *   Define `export class Configuration extends Schema.Class<Configuration>("Configuration")({...})`.
        *   Include fields:
            *   `agent_goal`: `Schema.String`
            *   `llm_config`: `LLMConfig` (Reference `LLMConfig`).
            *   `max_retries_per_action`: `Schema.Number`
            *   `allowed_actions`: `Schema.Array(Schema.String)`
            *   `restricted_paths`: `Schema.Array(Schema.String)`
            *   `action_timeout_seconds`: `Schema.Number`
            *   `session_timeout_minutes`: `Schema.Number`
            *   `github_token_available`: `Schema.Boolean`
        *   Add a JSDoc comment explaining this holds agent configuration settings.

3.  **Define the Top-Level `AgentState` Class:**
    *   Define `export class AgentState extends Schema.Class<AgentState>("AgentState")({...})`.
    *   Include fields corresponding to *all* the top-level keys in the `docs/agent-state.md` JSON spec, referencing the classes you defined above:
        *   `agent_info`: `AgentInfo`
        *   `timestamps`: `Timestamps`
        *   `current_task`: `CurrentTask`
        *   `plan`: `Schema.Array(PlanStep)`
        *   `execution_context`: `ExecutionContext`
        *   `tool_invocation_log`: `Schema.Array(ToolCall)`
        *   `memory`: `Memory`
        *   `metrics`: `Metrics`
        *   `error_state`: `ErrorState`
        *   `configuration`: `Configuration`
    *   Add a JSDoc comment explaining this is the main, top-level state object for the agent.

4.  **Verification:**
    *   Carefully re-read your generated `AgentStateTypes.ts` file.
    *   Compare it side-by-side with the JSON structure in `docs/agent-state.md`.
    *   Ensure every field is present, has the correct `Schema` type (including `Schema.Union` for nullability, `Schema.Array`, `Schema.Literal`, `Schema.Struct`, etc.), and correct nesting using references to the other defined classes.
    *   Confirm that `state_schema_version` in `AgentInfo` matches the version specified in the documentation ("1.1").
    *   Ensure all exported classes have unique names and corresponding string identifiers in the `Schema.Class` constructor (e.g., `"AgentInfo"`).

5.  **Important Notes:**
    *   **Focus:** Your *only* task here is to define the types and schemas. Do *not* implement any logic for saving, loading, or manipulating the state in this file.
    *   **Accuracy:** Be meticulous. Mismatches between the schema and the actual state data will cause runtime validation errors later.
    *   **Clarity:** Use JSDoc comments generously to explain the purpose of each class and complex field.

---

Execute these instructions carefully. Upon completion, the `src/github/AgentStateTypes.ts` file should contain a complete and accurate Effect Schema representation of the agent state as defined in the specification.
