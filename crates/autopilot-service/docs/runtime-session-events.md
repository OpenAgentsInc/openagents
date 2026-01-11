# Runtime session events

## SessionEvent pipeline
AutopilotRuntime converts CodexEvent entries from StartupState into SessionEvent values for UI consumers:
- Text: assistant content for the current phase
- Tool: tool start/completion with params, output, and error flag
- ToolProgress: elapsed time updates for a running tool

## Snapshot session identifiers
RuntimeSnapshot exposes both the autopilot session ID and per-phase Codex SDK session IDs:
- autopilot_session_id: the IDE-level session identifier
- sdk_session_ids: plan, exec, review, and fix session IDs captured from SDK streams

Shell clients consume these fields to surface session IDs and show live tool progress.
