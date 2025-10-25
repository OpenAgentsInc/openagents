# Convex Integration (Local, Self‑Hosted)

This repo uses a local, self‑hosted Convex backend as an optional persistence layer alongside Codex JSONL rollouts. Convex gives us:

- Reactive subscriptions (WebSocket) for multi‑client sync (mobile + web)
- A typed server function runtime for queries/mutations
- Backends for SQLite/Postgres; we start with SQLite only (no Docker)
- A path to full‑text + vector search (future)

We continue to keep Codex JSONL rollouts as the source of truth for resuming threads. Convex mirrors data in a normalized form to support richer queries and live sync.

## How it runs

- Bridge supervision
  - `codex-bridge` starts a local Convex backend automatically.
  - Use: `cargo bridge` (alias) or `cargo run -p codex-bridge --` (default port `7788`).
  - Bridge health‑checks `GET /instance_version` and logs “convex healthy …”.
  - SQLite DB: `~/.openagents/convex/data.sqlite3`
  - File storage: `~/.openagents/convex/storage`
  - Interface: by default we bind Convex to `0.0.0.0` so mobile devices on LAN/VPN can connect. You can force loopback by setting `OPENAGENTS_CONVEX_INTERFACE=127.0.0.1` or passing `--convex-interface 127.0.0.1`.

- App wiring
  - The app only calls queries/mutations; it never creates tables.
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

These need to be pushed to your local Convex backend once (or whenever changed). This is a developer/desktop step only — mobile clients do not need any env files.

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

## Self‑hosted configuration (developer machine)

We target the local backend on loopback (no cloud). To push functions during development, the Convex CLI accepts:

- `CONVEX_SELF_HOSTED_URL=http://127.0.0.1:7788`
- `CONVEX_SELF_HOSTED_ADMIN_KEY=<admin key>`

Place these in a repo‑root `.env.local` (git‑ignored) or set inline. This is required only for developers deploying server functions, not for end users on mobile.

## First‑time setup (one‑time)

1) Start the backend (recommended):
   - `cargo bridge` (starts Convex automatically)
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

## Migration note (threads.threadId)

We added an optional `threadId` field to `threads` to uniquely link rows with Codex threads. Legacy demo rows may lack this field. The schema now allows it to be absent; new upserts/mirrors will populate it. You can later normalize by patching legacy rows and making the field required.

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
- Move more read paths (e.g., history list) to Convex subscriptions (in progress; drawer reads from Convex when available)
- Live mirroring:
  - Client‑side: the app upserts threads/messages as the JSONL stream arrives (shipped).
  - Backfill: on first launch after connecting to Convex, the app backfills the last ~20 Codex sessions into Convex automatically.
  - Server‑side (planned): move mirroring to the bridge so history is captured even when the app is not connected.

## Mirroring and Backfill

We’re migrating “ALL CONVEX” for history. There are two cooperating pipelines so users see history whether or not the app is open:

- Live mirror (client) — shipped
  - As Codex JSONL streams in, the app calls Convex mutations:
    - `threads:upsertFromStream` on `thread.started` to create/update a thread row (with timestamps/title).
    - `messages:create` on user/assistant content (role, text, ts) to populate the transcript.
  - This gives fast, visible updates; Drawer History subscribes to `threads:list` and reflects them immediately.

- Live mirror (server) — phase 1 shipped (spool), ingester coming next
  - The bridge now captures the same events from the JSONL stream and writes a durable queue to disk:
    - Spool path: `~/.openagents/convex/mirror/spool.jsonl`
    - Events: `thread_upsert` and `message_create` lines in JSONL form.
  - An ingester (phase 2) will drain this spool and call the Convex mutations even if no client is connected. Until the ingester ships, the spool acts as a safe buffer and the client still mirrors live so you continue to see updates.

- Backfill (historical) — how to run
  - To stage historical data from Codex JSONLs into the spool, send a WS control to the bridge:
    - `{ "control": "convex.backfill" }`
  - The bridge scans `~/.codex/sessions`, parses new‑format rollouts, and enqueues `thread_upsert` + `message_create` for each thread. It emits a summary line:
    - `{ "type": "bridge.convex_backfill", "status": "enqueued", "count": N }`
  - Once the ingester is running, those queued events are written to Convex; Drawer History will auto‑update via subscription. Today, the client’s own backfill also runs on first launch and writes the last ~20 threads to Convex immediately.

### What users experience

- Drawer History
  - Shows Convex threads only. While the Convex client connects, a small spinner appears next to the “History” label. Once connected, titles/timestamps appear and update in real time.
  - Long titles are truncated to a single line; tap to open the live transcript view.

- After backfill
  - If Convex functions are deployed and reachable, historical threads appear as soon as they’re written (either by the client backfill or, soon, the server ingester). No bridge /history fallback is used in the UI anymore.

### Operational notes

- Convex backend is started by the bridge on `0.0.0.0:7788` by default so devices can connect over LAN/VPN. Health probes still use `http://127.0.0.1:7788` locally.
- The spool is append‑only and durable across restarts; the ingester will be idempotent by using the upsert mutation and inserting messages with timestamps.
- If Convex is unreachable, the client mirror pauses, but the server spool still records events for later ingestion.

## Production/TestFlight considerations

- End users do not need Node/Bun. The Convex backend is started by the Rust bridge; only developers need the CLI to deploy function changes during development.
- Mobile connects to the user’s desktop Convex over LAN/VPN. Ensure the Convex backend is reachable on the device network by binding to `0.0.0.0` (default) or a specific LAN/VPN IP.
- If your Convex functions change (e.g., new tables), re‑run the deploy step on the desktop. The app will pick them up on reconnect.
- iOS ATS: In production/TestFlight, plain HTTP to local/VPN hosts is blocked by default. We set `NSAppTransportSecurity -> NSAllowsArbitraryLoads: true` and `NSAllowsLocalNetworking: true` in `expo/app.json`. This requires a new native build; OTA updates cannot change Info.plist. If you prefer not to allow arbitrary loads, host Convex behind HTTPS and point Settings to that URL.
