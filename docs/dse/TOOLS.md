# Tools (Schema + Execution Contract)

Canonical requirements for runtime tool execution.

## Tool Definition Requirements

Each tool must provide:

- stable `tool` id
- params schema
- deterministic error behavior
- bounded output policy
- timeout policy

## Runtime Execution Requirements

Runtime must:

1. Validate params against schema before execution.
2. Apply timeout and output bounds deterministically.
3. Emit replay events for call/result.
4. Emit receipt entries with hashes, latency, and side-effect tags.

## Side-Effect Tags

Recommended tags include:

- `fs_read`
- `fs_write`
- `network`
- `process_spawn`
- `deploy`
- `payment`

See `docs/execution/ARTIFACTS.md` and `docs/execution/REPLAY.md`.
