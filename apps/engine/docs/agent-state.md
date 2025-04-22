Agent state will be saved and restored from a local JSON file.

It should look like this:
```json
{
  // --- METADATA ---
  "agent_info": {
    "type": "solver", // Type of the agent
    "version": "1.0.0", // Version of the agent code that saved this state
    "instance_id": "solver-uuid-of-issue-session-id", // Unique ID for this agent run/session
    "state_schema_version": "1.1" // Version of this state structure
  },
  "timestamps": {
    "created_at": "2024-03-20T10:00:00Z", // When this state object was first created
    "last_saved_at": "2024-03-20T11:00:00Z", // Last time this state was persisted
    "last_action_at": "2024-03-20T10:59:55Z" // Timestamp of the agent's last significant action
  },

  // --- TASK FOCUS & STATUS ---
  "current_task": {
    "repo_owner": "username",
    "repo_name": "repository-name",
    "repo_branch": "main",
    "issue_number": 123,
    // Optional: Cache key issue details for faster restore/context
    "issue_details_cache": {
      "title": "Fix the crucial bug in the main feature",
      "description_snippet": "The system crashes when user clicks...",
      "status": "open", // Status from the source (e.g., GitHub)
      "labels": ["bug", "critical"],
      "source_url": "https://github.com/username/repository-name/issues/123"
    },
    "status": "in_progress", // Agent's status for this task: idle | planning | researching | implementing | testing | blocked | completed | error
    "current_step_index": 1 // Index (0-based) into the plan array below
  },

  // --- EXECUTION PLAN ---
  "plan": [
    {
      "id": "step-uuid-1", // Unique ID for the step
      "step_number": 1,
      "description": "Analyze issue requirements and context",
      "status": "completed", // pending | in_progress | completed | skipped | error
      "start_time": "2024-03-20T10:05:00Z",
      "end_time": "2024-03-20T10:15:00Z",
      "result_summary": "Identified core requirement: prevent crash on click.",
      "tool_calls": [] // Log of tools used specifically for this step
    },
    {
      "id": "step-uuid-2",
      "step_number": 2,
      "description": "Research existing codebase for relevant modules",
      "status": "in_progress",
      "start_time": "2024-03-20T10:15:05Z",
      "end_time": null,
      "result_summary": null,
      "tool_calls": [ // Example tool call log within a step
        {
          "timestamp": "2024-03-20T10:20:00Z",
          "tool_name": "fetchFileContents",
          "parameters": { "owner": "username", "repo": "repository-name", "path": "src/main.ts" },
          "status": "success",
          "result_preview": "```typescript\nfunction handleClick()...```"
        }
      ]
    },
    {
      "id": "step-uuid-3",
      "step_number": 3,
      "description": "Propose code changes",
      "status": "pending",
      "start_time": null,
      "end_time": null,
      "result_summary": null,
      "tool_calls": []
    }
    // ... more steps
  ],

  // --- DYNAMIC CONTEXT & LOGS ---
  "execution_context": {
    "current_file_focus": null, // { "path": "src/main.ts", "relevant_lines": [10, 25] }
    "relevant_code_snippets": [
      { "file_path": "src/utils.ts", "snippet": "export function utilFunc...", "reason": "Related utility function" }
    ],
    "external_references": [ // More structured dependencies/relations
      { "type": "issue", "identifier": "121", "relationship": "relates_to", "source": "github" },
      { "type": "pr", "identifier": "45", "relationship": "blocked_by", "source": "github" }
    ],
    "files_modified_in_session": ["src/main.ts", "tests/main.test.ts"] // Files touched in this run
  },
  "tool_invocation_log": [ // Separate, comprehensive log of all tool calls
    {
      "timestamp": "2024-03-20T10:20:00Z",
      "tool_name": "fetchFileContents",
      "parameters": { "owner": "username", "repo": "repository-name", "path": "src/main.ts" },
      "status": "success",
      "result_preview": "```typescript\nfunction handleClick()...```",
      "full_result_ref": "tool-result-uuid-1" // Optional: reference to full result stored elsewhere if large
    },
    {
      "timestamp": "2024-03-20T10:25:00Z",
      "tool_name": "updateIssueStatus",
      "parameters": { "issueNumber": 123, "status": "in_progress", "comment": "Starting investigation" },
      "status": "success",
      "result_preview": "{ \"success\": true, \"message\": \"Status updated...\" }",
      "full_result_ref": null
    }
  ],

  // --- AGENT MEMORY ---
  "memory": {
    // Structured conversation history
    "conversation_history": [
      { "role": "user", "content": "Fix the bug in the main feature", "timestamp": "2024-03-20T10:01:00Z" },
      { "role": "assistant", "content": "Okay, I will analyze the issue description.", "timestamp": "2024-03-20T10:05:00Z" },
      { "role": "assistant", "tool_calls": [{ "id": "tool-call-1", "name": "fetchFileContents", "input": { "path": "src/main.ts" } }], "content": "[Using tool fetchFileContents...]", "timestamp": "2024-03-20T10:18:00Z" },
      { "role": "tool", "tool_call_id": "tool-call-1", "content": "{ \"content\": \"...file content...\" }", "timestamp": "2024-03-20T10:20:05Z" },
      { "role": "assistant", "content": "I have reviewed src/main.ts. It seems the error occurs in the handleClick function.", "timestamp": "2024-03-20T10:21:00Z" }
    ],
    // Structured key decisions made by the agent
    "key_decisions": [
      { "timestamp": "2024-03-20T10:15:00Z", "decision": "Focus investigation on `handleClick` in `src/main.ts`.", "reasoning": "Issue description mentions crash on click, stack trace points here.", "confidence": 0.8 }
    ],
    // Key pieces of information derived or learned
    "important_findings": [
      { "timestamp": "2024-03-20T10:21:00Z", "finding": "`handleClick` doesn't handle null values from `getData()`.", "source": "code_analysis", "confidence": 0.95 }
    ],
    // Agent's internal monologue or temporary notes
    "scratchpad": "Need to check the return type of getData(). Possible null pointer exception."
  },

  // --- METRICS ---
  "metrics": {
    "steps_completed": 1,
    "total_steps_in_plan": 5,
    "session_start_time": "2024-03-20T10:00:00Z",
    "total_time_spent_seconds": 3600, // Cumulative time across sessions for this issue maybe? Or just this session? Clarify semantics.
    "llm_calls_made": 5,
    "llm_tokens_used": { "prompt": 4500, "completion": 800 },
    "tools_called": 2,
    "commits_made": 0 // Or intended commits if agent doesn't commit directly
  },

  // --- ERROR STATE ---
  "error_state": {
    "last_error": null, // { "timestamp": "...", "message": "...", "type": "api_error | tool_error | internal", "details": "..." }
    "consecutive_error_count": 0,
    "retry_count_for_current_action": 0,
    "blocked_reason": null // e.g., "Waiting for user input", "API rate limit hit"
  },

  // --- CONFIGURATION ---
  "configuration": {
    "agent_goal": "Resolve issue #123 by fixing the crash and adding tests.", // Specific goal for this run
    "llm_config": { // Specific LLM settings used
        "model": "claude-3-5-sonnet-latest",
        "temperature": 0.7,
        "max_tokens": 1024
    },
    "max_retries_per_action": 3,
    "allowed_actions": ["read_file", "write_file", "run_test", "update_issue"], // More specific actions
    "restricted_paths": ["/.git/*", "/secrets/", "/config/prod.json"], // Specific restricted file paths/patterns
    "action_timeout_seconds": 300, // Timeout for individual actions like tool calls
    "session_timeout_minutes": 120, // Timeout for the entire agent session
    "github_token_available": true // Status flag, NOT the token itself
  }
}
```
