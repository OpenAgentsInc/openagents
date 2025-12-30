# Claude SDK streaming and session tracking

## Overview
Autopilot uses claude-agent-sdk query streams for plan, execution, review, and fix phases. SDK events are converted into ClaudeToken messages and stored as ClaudeEvent entries for UI streaming and checkpoints.

## Session IDs and resume
- Each SDK stream emits ClaudeToken::SessionId when a session_id is observed from any message type.
- StartupState stores per-phase session IDs (plan, exec, review, fix) and persists them in SessionCheckpoint.
- On resume, run_claude_planning/run_claude_execution/run_claude_review set QueryOptions::resume and send an empty prompt to reattach to the session.
- If a checkpoint resumes without an active receiver, StartupState spawns a new streaming receiver with the saved session ID.

## Tool progress and partial messages
- QueryOptions::include_partial_messages(true) is enabled for all phases to receive StreamEvent and ToolProgress updates.
- Tool progress updates are forwarded as ClaudeToken::Progress and stored as ClaudeEvent::ToolProgress.
- Downstream runtime and shell layers map these events to update tool cards with elapsed time.

## Plan phase tool restrictions
- Planning uses QueryOptions::disallowed_tools to prevent Edit/Write/NotebookEdit usage.
- Execution and review retain full tool access with permission_mode bypass.
