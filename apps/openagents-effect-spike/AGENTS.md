# `@openagents/effect-spike` Agent Instructions

- Read `node_modules/@effect/ui/AGENTS.md` before editing `@effect/ui` components, queries, commands, or tests.
- Implements `docs/effect/half-day-sprint.md` pinned to `Effect-TS/ui@9d672abe` and `effect ^4.1.6`.
- `OA_WORKER_BIN` defaults to `/Applications/OpenAgents.app/Contents/Resources/oa-worker`.
- Always pass `SSH_CONNECTION=1` when spawning `oa-worker` so OAuth probes never open a browser window.
- Every tracked `.ts` file must include a `// Firm reason: 1 (...)` header in its first 60 lines for `coax/web/tool.sh tools-check`.
