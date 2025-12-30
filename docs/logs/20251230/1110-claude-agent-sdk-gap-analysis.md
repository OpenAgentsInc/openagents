# Claude Agent SDK Gap Analysis for Autopilot IDE

Date: 2025-12-30

## Scope reviewed
- crates/claude-agent-sdk/README.md
- crates/claude-agent-sdk/docs/GAP-REPORT.md
- crates/claude-agent-sdk/src/lib.rs
- crates/claude-agent-sdk/src/options.rs
- crates/claude-agent-sdk/src/query.rs
- crates/claude-agent-sdk/src/session.rs
- crates/claude-agent-sdk/src/protocol/messages.rs
- crates/autopilot/src/claude.rs
- crates/autopilot/src/startup.rs
- crates/autopilot/src/checkpoint.rs
- crates/autopilot-service/src/runtime.rs
- crates/autopilot-shell/src/shell.rs
- crates/autopilot-shell/src/claude_sessions.rs
- crates/autopilot-shell/src/panels/sessions.rs

## Current integration summary (what works today)
- Autopilot uses claude-agent-sdk::query for plan, execute, and review phases with cwd, model, permission_mode, and max_turns configured.
- Tool start and completion events are surfaced in Autopilot runtime as ClaudeEvent::Tool and rendered in the IDE.
- Session usage is extracted from SdkResultMessage and displayed in the System panel.
- Autopilot Shell can load historical Claude Code sessions from ~/.claude/projects and render ToolCallCard entries with params, output, and Task nesting.
- Autopilot Shell can resume a Claude Code session via unstable_v2_resume_session and stream assistant text back into the thread view.

## Gaps blocking 100 percent SDK implementation in the IDE

### 1) SDK session lifecycle and resume
- SessionCheckpoint includes claude_session_id, exec_session_id, review_session_id, fix_session_id but they are never populated, so SDK resume is not possible across IDE restarts or Full Auto toggles.
- Full Auto resume relies on local checkpoint data only; no SDK session resume is wired for plan, exec, review, or fix phases.
- Resume in the UI is based on ~/.claude/projects logs, not SDK session IDs, and does not use resume_session_at or fork_session.
- No interactive multi-turn conversation flow using Session::send and Session::receive outside the initial prompt.

### 2) Permission system and interactive approvals
- PermissionMode::Plan and PermissionMode::BypassPermissions are used, with dangerously_skip_permissions enabled for exec and review.
- No PermissionHandler, PermissionRules, or permission_handler usage is wired into runtime or UI.
- No UI for permission prompts or permission updates; permission_prompt_tool_name is unused.

### 3) QueryOptions coverage gaps (not exposed or configured)
- No support for fallback_model, max_budget_usd, max_thinking_tokens, additional_directories.
- No tools configuration (tools, allowed_tools, disallowed_tools) beyond defaults.
- No system_prompt or output_format configuration.
- No MCP server configuration (mcp_servers, strict_mcp_config).
- No custom agents (AgentDefinition) or subagent controls.
- include_partial_messages is never enabled, so StreamEvent messages are unused.
- No resume_session_at, fork_session, enable_file_checkpointing, or persist_session controls.
- No setting_sources or skills loading (from ~/.claude/skills).
- No betas, env, extra_args, sandbox settings, or plugins wiring.
- Model selection UI includes Haiku, but Autopilot runtime only supports Sonnet and Opus, so Haiku selection falls back to Sonnet.

### 4) Query control methods not wired
- IDE Interrupt toggles a local force_stopped flag only; it does not call Query::interrupt or Query::abort on active SDK sessions.
- No runtime or UI plumbing for set_permission_mode, set_model mid-session, set_max_thinking_tokens, mcp_server_status, rewind_files, supported_models, supported_commands, account_info, or stream_input.

### 5) Message type handling gaps
- Autopilot consumes SdkMessage::Assistant text and tool_use name only; tool inputs, tool outputs, and errors are not surfaced during live sessions.
- SdkMessage::User tool_use_result is only used to mark tool completion; output content is discarded.
- SdkMessage::System (init, status, api_error, stop_hook_summary, informational, local_command) is not surfaced in IDE.
- SdkMessage::StreamEvent, SdkMessage::ToolProgress, and SdkMessage::AuthStatus are not surfaced.
- Structured output in SdkResultMessage is not displayed or stored.

### 6) Tool call visualization parity gaps
- Historical sessions display ToolCallCard with params and output; live Autopilot sessions show only tool name and params, without output or error details.
- Resumed SDK sessions render tool uses as plain text ("Tool: name") instead of ToolCallCard.

**ADDENDUM (2025-12-30 evening):** Partially addressed. Historical sessions now parse `tool_use` blocks for input params and link to `tool_result` blocks by ID to get output and error status. Two-pass JSONL parser in `claude_sessions.rs` collects tool results first, then builds messages with linked data. Display logic in `shell.rs` creates `ToolCallCard` components with expandable sections, status indicators, and Task nesting. Commits: `2b377a489`. **Remaining:** Live sessions still discard tool output - need to surface `tool_result` content in live `ToolCallCard` entries.

### 7) MCP, plugins, and custom tools
- No IDE or runtime path to configure MCP servers or plugin definitions.
- No exposure of custom tool definitions or MCP tool invocation status in the UI.

### 8) System prompt and structured output
- No UI or runtime configuration for system prompts (preset append or custom).
- No output_format configuration or display of structured_output results.

### 9) Skills and setting sources
- No UI or runtime configuration for setting_sources, so skills in ~/.claude/skills are not loaded.

### 10) Testing and verification gaps
- No integration tests covering SDK session resume, permission callbacks, or tool output rendering in the IDE.
- No tests validating handling of SdkMessage variants beyond Assistant and Result.

## Blocking SDK level gaps (outside the IDE)
- In-process MCP servers are stubbed in claude-agent-sdk: McpServerConfig::Sdk lacks tool() and createSdkMcpServer support (see crates/claude-agent-sdk/docs/GAP-REPORT.md).

## Suggested priorities to reach 100 percent in the IDE
1) Capture and persist SDK session IDs in StartupState and SessionCheckpoint, and wire resume to QueryOptions::resume and Session::send/receive.
2) Implement permission handling UI and connect it to PermissionHandler or PermissionRules.
3) Surface ToolCallCard for live sessions with tool input and output, including tool_result parsing and error status.
4) Add UI controls for key QueryOptions (model, max_turns, max_budget_usd, permission_mode, system_prompt, output_format).
5) Enable StreamEvent and ToolProgress handling with include_partial_messages.
6) Add MCP configuration and status views, plus hooks and plugin configuration where needed.
7) Align model selector with supported_models from the SDK and remove unsupported entries.
