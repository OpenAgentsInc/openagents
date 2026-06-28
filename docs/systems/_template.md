# System Name

## Summary

One paragraph describing what this system does for OpenAgents and which user,
agent, or operator workflows depend on it.

## Responsibilities / Boundaries

- Owns:
- Does not own:
- Authority boundaries:
- Public/private data boundaries:

## Key Code Paths

| Path | Purpose |
| --- | --- |
| `path/to/code` | Primary implementation or contract surface. |

## Data Model

List D1 tables, Durable Object state, R2 objects, KV entries, Effect Schema
types, local files, or other durable records owned or projected by this system.

| Record / table / schema | Owner | Notes |
| --- | --- | --- |
| `record_name` | `path/to/owner` | Public-safe, owner-only, internal, or private. |

## Interfaces

Document routes, CLI commands, Worker bindings, Durable Object methods, queue
messages, WebSocket/SSE/event stream frames, package exports, MCP tools, or
other integration points.

| Interface | Direction | Contract |
| --- | --- | --- |
| `GET /example` | inbound | Effect Schema, JSON shape, event type, or package export. |

## Dependencies

- Upstream systems:
- Downstream systems:
- External services:
- Shared packages:

## Invariants

Link the applicable invariant sections instead of copying them. At minimum,
check the root ledger and any app-local ledger before changing this system.

- [`/INVARIANTS.md`](../../INVARIANTS.md):
- [`/apps/openagents.com/INVARIANTS.md`](../../apps/openagents.com/INVARIANTS.md):

## Related ADRs

Link records under `docs/adr/` when that directory exists for this system. If no
ADR exists yet, write `None yet`.

## Verification

List the focused tests, smoke commands, deploy checks, or public proof surfaces
that demonstrate the system still satisfies its contracts.

```sh
bun run --cwd apps/openagents.com check:deploy
```

## Open Questions

- Question:

## Change Log

| Date | Change | Evidence |
| --- | --- | --- |
| YYYY-MM-DD | Initial system doc. | PR / issue / verification command. |
