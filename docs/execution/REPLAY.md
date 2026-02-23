# REPLAY.jsonl (Canonical Replay Log)

`REPLAY.jsonl` is newline-delimited JSON used for deterministic replay/audit.

## File Structure

1. First event: `ReplayHeader`
2. Remaining events: replay events for session lifecycle and tool execution

Each line is a JSON object tagged with an `"event"` discriminator (string).

## Core Event Types

- `ReplayHeader`
- `SessionStart`
- `ToolCall`
- `ToolResult`
- `Verification`
- `Payment` (optional)
- `RuntimeEvent` (optional; runtime authority surfaces)
- `SessionEnd`

## Normative Rules

1. `ToolCall.params` must validate against tool schema before execution.
2. `params_hash` and `output_hash` must be deterministic.
3. `ToolResult.id` must match a prior `ToolCall.id`.
4. Replay publication/export must apply privacy redaction policy.

## Privacy Layering

1. Local replay may include richer payload fields.
2. Published replay must redact sensitive fields while preserving hashes.

See `docs/dse/PRIVACY.md` and `docs/protocol/PROTOCOL_SURFACE.md`.
