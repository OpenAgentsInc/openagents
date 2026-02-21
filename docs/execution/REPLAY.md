# REPLAY.jsonl v1 (Canonical Replay Log)

This document specifies the canonical **REPLAY.jsonl v1** format.

REPLAY is a **newline-delimited JSON** event stream. It is designed to be:
- replayable (deterministic reconstruction of decisions + tool IO),
- auditable (hashes bind params/outputs),
- and exportable (can be redacted for publication).

See:
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0003-replay-formats.md`
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0006-deterministic-hashing.md`
- `docs/plans/archived/adr-legacy-2026-02-21/ADR-0017-telemetry-trace-contract.md` (Layer A/B/C privacy)

## File Structure

1. First line MUST be a `ReplayHeader`.
2. Remaining lines are `ReplayEvent` records.

## Common Fields

All events SHOULD include:
- `type` (string)
- `ts` (string, ISO-8601 timestamp)

Some events include:
- `session_id` (string)
- `step_id` (string)

## ReplayHeader

The header anchors the file and declares the format.

```jsonc
{
  "type": "ReplayHeader",
  "replay_version": 1,
  "producer": "openagents",
  "created_at": "2026-02-15T00:00:00.000Z"
}
```

Required:
- `type = "ReplayHeader"`
- `replay_version = 1`
- `producer` (string)
- `created_at` (string, ISO-8601)

## SessionStart

```jsonc
{
  "type": "SessionStart",
  "ts": "2026-02-15T00:00:01.000Z",
  "session_id": "…",
  "policy_bundle_id": "…"
}
```

Required:
- `session_id`
- `policy_bundle_id`

Optional:
- `repo` (object: `remote`, `branch`, `commit`)
- `lane` (string)

## ToolCall

Tool invocation event (emitted by the execution runtime).

```jsonc
{
  "type": "ToolCall",
  "ts": "2026-02-15T00:00:02.000Z",
  "session_id": "…",
  "step_id": "…",
  "tool": "shell_command",
  "params": { "command": "rg -n \"foo\" ." },
  "params_hash": "sha256:…"
}
```

Required:
- `step_id` (stable within session)
- `tool` (string)
- `params` (object)
- `params_hash` (string)

Normative:
- Tool params MUST validate against the tool JSON schema before execution (`docs/plans/archived/adr-legacy-2026-02-21/ADR-0007-tool-execution-contract.md`).
- `params_hash` MUST be computed deterministically (`docs/plans/archived/adr-legacy-2026-02-21/ADR-0006-deterministic-hashing.md`).

## ToolResult

Tool result event.

```jsonc
{
  "type": "ToolResult",
  "ts": "2026-02-15T00:00:03.000Z",
  "session_id": "…",
  "step_id": "…",
  "ok": true,
  "output_hash": "sha256:…",
  "latency_ms": 123,
  "side_effects": ["fs_read"]
}
```

Required:
- `step_id` (must match a prior ToolCall)
- `ok` (boolean)
- `output_hash` (string)
- `latency_ms` (number)
- `side_effects` (array of strings)

Optional:
- `error` (object: `name`, `message`, `stack?`)
- `step_utility` (number, -1.0..+1.0; see `docs/GLOSSARY.md`)

Normative:
- `output_hash` MUST be computed deterministically over canonicalized output (`docs/plans/archived/adr-legacy-2026-02-21/ADR-0006-deterministic-hashing.md`).

## Verification

Verification events record objective checks run by the agent (lint/test/build/smoke).

```jsonc
{
  "type": "Verification",
  "ts": "2026-02-15T00:00:10.000Z",
  "session_id": "…",
  "command": "cd apps/openagents.com && composer lint",
  "cwd": "apps/openagents.com",
  "exit_code": 0,
  "duration_ms": 12345,
  "verification_delta": 0
}
```

Required:
- `command`
- `exit_code`

Optional:
- `cwd`
- `duration_ms`
- `verification_delta` (definition in `docs/GLOSSARY.md`)

## SessionEnd

Final event that summarizes the session.

```jsonc
{
  "type": "SessionEnd",
  "ts": "2026-02-15T00:01:00.000Z",
  "session_id": "…",
  "status": "success | failure | cancelled",
  "confidence": 0.0
}
```

Required:
- `status`
- `confidence` (number, 0..1)

Optional (recommended):
- `summary` (string)
- `total_tool_calls` (number)
- `total_latency_ms` (number)

## Privacy (Layer B vs Layer C)

Layer B (local) replay MAY include full `params`.

Layer C (published/external) MUST:
- remove `params` and any raw `output`,
- keep only hashes (`params_hash`, `output_hash`),
- apply privacy policy redaction rules.

See `docs/plans/archived/adr-legacy-2026-02-21/ADR-0017-telemetry-trace-contract.md` and `docs/plans/archived/adr-legacy-2026-02-21/ADR-0016-privacy-defaults-swarm-dispatch.md`.
