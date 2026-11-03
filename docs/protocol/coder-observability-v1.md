# Coder observability v1

New local JSONL events carry `schema: "coder.observability.v1"` and `kind`
`model`, `usage` or `availability`. Their typed fields are declared in
`proto/openagents/coder/observability/v1/events.proto`. Existing session
events retain their identity. Readers may ignore these additional kinds.

`model` records the Codex model and effort resolved by `thread/start`, with
`source: "thread/start"`. Missing values and unobserved `cost_usd` are null.
It is an observation at thread creation, not proof of later per-turn settings.

`usage` records Codex `thread/tokenUsage/updated` for the active thread and
turn only, with `thread_id` and `turn_id` identifying their scope. `scope: "thread_total"` means a cumulative snapshot, including
any resumed history. Never sum snapshots or interpret them as per-turn
usage. Token keys use the upstream camelCase names (`inputTokens`, etc.).
Missing, invalid or negative counts are null; cached counts are subsets,
not additional billable tokens. No prices or costs are inferred.

`availability` records a typed terminal provider failure, the local retry
time in Unix seconds and whether its observation was persisted. It never
includes provider error prose or credentials. Routing also adds
`allowed_mask`, `blocked_mask` and `eligible_mask` to its existing diagnostic
event: bits 0..3 represent Claude, Codex, Microcoder and Psionic.

Automatic routing excludes recently failed providers for at least 60 seconds.
A rejected Claude rate-limit notification can extend this to its typed reset
time when it is within seven days. Neither quota nor reset is inferred from
error prose.
Explicit `--engine` remains an intentional retry. Nothing retries a failed
task on another engine automatically. `CODER_ROUTE_ENGINES` restricts the
automatic candidates using exact comma-separated names `claude,codex,micro,psionic`;
empty means all, malformed or duplicate entries mean none. Installation and
cooldown checks still apply. A configured local endpoint is not a health probe.

Observations are durable append-only local files under
`$CODER_HOME/.openagents/coder/availability-ENGINE.jsonl` (HOME fallback).
Concurrent failures cannot overwrite one another. Malformed, partial,
future-dated or oversized (64 KiB) histories conservatively exclude that
engine. An explicit retry bypasses this local routing hint. Old complete
histories can be removed to reset the hint; this does not change sessions.
Successful explicit retries do not erase other concurrent failures.

Session summaries with a durable user turn but no recorded end add
`effect_outcome: "unknown"`. The existing `state: "running"` describes the
log's open turn, not proof that a process remains alive. A process may have
changed files before termination. Replay reconstructs input without executing
historical effects; inspect current files before an explicit continuation.
This does not provide transactional edits or exactly-once execution.
