<!-- Moved from docs/convex-sidecar.md -->
Convex Sidecar (Desktop)

Goal
- Run the Convex local backend bundled with the Tauri app (offline‑first), without requiring `convex dev`.

Sidecar
- Path: `tauri/src-tauri/bin/local_backend` (not committed; ignored in git).
- Bundled via `tauri/src-tauri/tauri.conf.json` → `bundle.resources`.
- App spawns it at launch on `127.0.0.1:3210` with:
  - `<db> ~/.openagents/convex/data.sqlite3`
  - `--db sqlite --interface 127.0.0.1 --port 3210 --disable-beacon`
- Client default: `CONVEX_URL=http://127.0.0.1:3210` (overridable).

Install the binary locally
1) Ensure Bun is installed: https://bun.sh
2) Fetch the local backend into the sidecar path:

   bun run convex:fetch-backend

   The script tries:
   - Prefer the Convex CLI cache (~/.cache/convex/binaries)
   - Fallback to `~/.openagents/bin/local_backend` if present
3) Verify: `file tauri/src-tauri/bin/local_backend` and it’s executable (`chmod +x`).

Run
- Dev: `cd tauri && cargo tauri dev`
- Build: `cd tauri && cargo tauri build`

Notes
- First run may take time to initialize; the app spawns the process and will connect when ready.
- Disable embedded backend: `OPENAGENTS_SKIP_EMBEDDED_CONVEX=1`.
