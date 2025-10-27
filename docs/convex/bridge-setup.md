Restored placeholder for docs/convex/bridge-setup.md

Intended content (reconstruct from memory)
- Bridge starts/stops local Convex on startup (`0.0.0.0:7788` by default)
- SQLite DB: `~/.openagents/convex/data.sqlite3`
- Health probe: `GET /instance_version` on `http://127.0.0.1:7788`
- Environment overrides: `OPENAGENTS_CONVEX_PORT`, `OPENAGENTS_CONVEX_INTERFACE`, `OPENAGENTS_CONVEX_DB`
- First-time function push (dev): `bun run convex:dev:once` or `bun run convex:deploy`

Please re-populate with the original details.

