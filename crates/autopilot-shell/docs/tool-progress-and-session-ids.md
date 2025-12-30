# Tool progress and session IDs in the shell

## Tool progress display
AutopilotShell listens for SessionEvent::ToolProgress and updates running ToolCallCard entries with elapsed time.
- pending_tool_meta tracks tool metadata by entry index (name, params, elapsed time).
- When a progress event arrives, the shell rebuilds the running ToolCallCard with the latest elapsed time.
- Child tool progress is also tracked by updating Task child entries and rebuilding the parent card.

## Session IDs in the system panel
The system panel shows both the autopilot session ID and per-phase Claude SDK session IDs. RuntimeSnapshot exposes these IDs, and the shell forwards them into the Claude usage panel for visibility and copy.

## Resume options
The Sessions panel includes resume controls for SDK sessions:
- `resume_session_at` input accepts a message UUID to resume from.
- `fork_session` toggle controls whether resume continues the original session or forks a new ID.

These options are applied when Full Auto is toggled on to continue the selected session.
