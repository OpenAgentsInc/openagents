# Convex Bridge Setup (what happens when you run `npx tricoder`)

This explains how the local Convex backend comes up when a user has installed nothing besides Node. The goal is: one command, one pairing code, and the app connects while the backend becomes ready.

Overview
- The Node CLI (`npx tricoder`) launches the Rust WebSocket bridge and opens two tunnels (bridge WS and Convex HTTP) so your phone can connect.
- The bridge supervises a local Convex backend on `127.0.0.1:7788` with a SQLite DB under `~/.openagents/convex/`.
- If the Convex backend binary is missing, the bridge triggers a one‑time install into `~/.openagents/bin/local_backend` (no Docker, no global install).
- Once healthy, the bridge pushes your Convex functions (schema + server functions) once so the app’s queries work.

What users see in the console
- A single “Bridge Code” to paste in the mobile app.
- A security warning: never share this code/token.
- Concise status while Convex prepares (quiet mode):
  - “Setting up local Convex backend (first run downloads a small binary)…” (first run only)
  - “Starting local Convex backend…”
  - “Convex backend ready in Ns.”
  - “Deploying functions…” and “Functions deploy finished (code 0)”
- Verbose mode (`--verbose`) shows full tunnel/bridge/Convex logs.

No manual installs required
- Backend binary: auto‑provisioned to `~/.openagents/bin/local_backend` on first run.
- Functions deploy: performed by the bridge (via `bun run convex:dev:once`), with a concise status from the CLI.
- Database: SQLite at `~/.openagents/convex/data.sqlite3` with local storage in `~/.openagents/convex/storage`.

Exact sequence
1) `npx tricoder` prints the pairing code and warning.
2) Bridge starts on `0.0.0.0:8787` and begins `convex.ensure`:
   - If `~/.openagents/bin/local_backend` is missing, a best‑effort background fetch is kicked off.
   - The process is spawned with `--db sqlite --interface 127.0.0.1 --port 7788 --disable-beacon`.
   - Health is polled via `GET /instance_version` until 200 OK.
3) On first healthy, the bridge runs a one‑time functions deploy (`bun run convex:dev:once`). In quiet mode, tricoder prints a small summary of “Deploying…”/“Finished”.
4) The mobile app connects to the printed public URLs (bridge WS and Convex HTTP) and starts streaming.

Defaults and paths
- HTTP base: `http://127.0.0.1:7788`
- Health: `GET /instance_version`
- DB: `~/.openagents/convex/data.sqlite3`
- Storage: `~/.openagents/convex/storage`

Security note
- The “Bridge Code” is a private pairing token. Do not share it. Anyone with this code can connect to your desktop bridge.

Troubleshooting
- “Convex backend ready” never appears
  - Re‑run with `--verbose` to see detailed logs.
  - Ensure Bun is available for the one‑shot functions deploy (the bridge will still run without it; only the initial functions push may be skipped).
- App shows no threads
  - After the backend is ready, a functions deploy occurs once per session. Re‑run with `--verbose` and look for “Functions deploy finished”.

Related docs
- See other docs in this folder for the broader Convex integration (schema, functions, app wiring) and audits.

