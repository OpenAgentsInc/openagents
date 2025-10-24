# Convex Integration (Local, Self‑Hosted)

This repo uses a local, self‑hosted Convex backend as an optional persistence layer alongside Codex JSONL rollouts. Convex gives us:

- Reactive subscriptions (WebSocket) for multi‑client sync (mobile + web)
- A typed server function runtime for queries/mutations
- Backends for SQLite/Postgres; we start with SQLite only (no Docker)
- A path to full‑text + vector search (future)

We continue to keep Codex JSONL rollouts as the source of truth for resuming threads. Convex mirrors data in a normalized form to support richer queries and live sync.

## How it runs

- Bridge supervision
  - `codex-bridge` can start a Convex backend automatically on loopback.
  - Use: `cargo run -p codex-bridge -- --with-convex` (default port `127.0.0.1:7788`).
  - Bridge health‑checks `GET /instance_version` and logs “convex healthy …”.
  - SQLite DB: `~/.openagents/convex/data.sqlite3`
  - File storage: `~/.openagents/convex/storage`

- Manual start (advanced)
  - We install a prebuilt `local_backend` binary to `~/.openagents/bin/local_backend`.
  - Example: `~/.openagents/bin/local_backend ~/.openagents/convex/data.sqlite3 --db sqlite --interface 127.0.0.1 --port 7788 --disable-beacon`

- App wiring
  - A Convex React client is provided via `expo/providers/convex.tsx`.
  - The Convex screen (`/convex`) shows connection status (via the bridge) and a live list of threads from Convex React once functions are deployed.

## Why Convex (vs JSONL only)

- JSONL rollouts are great for Codex compatibility and resuming sessions.
- For multi‑device sync, subscriptions, and richer queries (search, vectors), a database with live queries is a better fit.
- We use both: rollouts for resume, Convex for queryability and live views.

## Functions and schema in this repo

- `convex/schema.ts`: defines the `threads` table
- `convex/threads.ts`: server functions
  - `list` query: returns all threads (desc order)
  - `createDemo` mutation: inserts a demo thread row

These need to be pushed to your local Convex backend once (or whenever changed).

## What `npx convex dev` does and when to use it

`npx convex dev` is the primary CLI for pushing Convex functions to a backend during development. It:

1) Configures a Convex project for the current directory (writes `convex.json` if not present)
2) Generates types under `convex/_generated/` (ignored by git)
3) Pushes your functions (e.g., `convex/schema.ts`, `convex/threads.ts`) to the selected backend
4) Watches for changes and re‑pushes on save

Use `npx convex dev` when:
- You want the Convex React subscription in the app to work (e.g., `/convex` screen live list)
- You’ve edited any `convex/*.ts` functions or schema
- You need to (re)create `convex.json` after a clean clone

Non‑watch alternatives:
- One‑shot push: `npx convex deploy` (helpful for CI or scripting)
- One‑time dev cycle: `npx convex dev --once`

## Self‑hosted configuration

We target the local backend on loopback (no cloud). Two env vars are accepted by the Convex CLI for self‑host setups:

- `CONVEX_SELF_HOSTED_URL=http://127.0.0.1:7788`
- `CONVEX_ADMIN_KEY=<admin key>`

You can place these in a `.env.local` (git‑ignored) or set them inline per command. The scripts accept either the official names or fallbacks:

- Preferred: `CONVEX_SELF_HOSTED_URL` and `CONVEX_SELF_HOSTED_ADMIN_KEY`
- Fallbacks (mapped automatically by scripts): `CONVEX_URL` and `CONVEX_ADMIN_KEY`

Admin key (dev):
- The Convex backend ships with a default dev instance name/secret (`carnitas` / a hex secret). We use `keybroker` to generate an admin key for local use.
- Bridge supervision does not require an admin key; the key is only for pushing functions via the CLI.

## First‑time setup (one‑time)

1) Start the backend (recommended):
   - `cargo run -p codex-bridge -- --with-convex`
   - Logs should show: `convex healthy (already running)`
2) Generate an admin key (optional if you already have one):
   - From `~/code/convex-backend`:
     - `cargo run -p keybroker --bin generate_key -- carnitas <secretHex>`
   - Save output to your `.env.local`:
     - `CONVEX_SELF_HOSTED_URL=http://127.0.0.1:7788`
     - `CONVEX_ADMIN_KEY=<the key>`
3) From repo root, push functions:
   - Recommended: `bun run convex:dev:once` (uses `.env.local`, writes `convex.json`, pushes once)
   - Watch mode: `bun run convex:dev`
   - One‑shot deploy (after configured): `bun run convex:deploy`

After this, the app’s Convex screen (“Live Threads”) will render `threads:list` and live‑update.

## App demo helpers

Even without deploying functions, you can test SQLite structure via the bridge:
- Create `threads` table: the “Create threads table” button on `/convex`
- Insert a demo row: “Create demo thread” button

These use bridge WS controls to manipulate the SQLite DB directly. They are for bootstrap/testing only; the React subscription is powered by the Convex functions you deploy.

## Paths and ports

- HTTP base: `http://127.0.0.1:7788`
- DB: `~/.openagents/convex/data.sqlite3`
- Storage: `~/.openagents/convex/storage`
- Health endpoint: `GET /instance_version` (may return `unknown` on prebuilt binaries but indicates liveness)

## Security & networking

- The backend binds to `127.0.0.1` (loopback only) by default. For remote/mobile access, route via LAN/VPN (e.g., Tailscale), or expose carefully.
- Admin keys are sensitive; keep them in `.env.local` (git‑ignored).

## Troubleshooting

- Convex screen shows “No Convex project deployed or query missing (threads:list)”
  - You haven’t pushed functions yet. Run `npx convex dev` from repo root.
- CLI says “No CONVEX_DEPLOYMENT set”
  - Run `npx convex dev` once to configure `convex.json`, then use `npx convex deploy` next time.
- `instance_version` returns `unknown`
  - Expected on some precompiled builds; the health endpoint is still up.

## Roadmap

- Index messages and add full‑text search
- Add embeddings and vector search using the Convex vector crates (future)
- Move more read paths (e.g., history list) to Convex subscriptions
- Integrate bridge upserts for live Codex -> Convex mirroring
