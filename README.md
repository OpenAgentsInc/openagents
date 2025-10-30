# OpenAgents: Project Tricoder

A mobile command center for your coding agents.

<img width="1000" height="470" alt="wouldnt" src="https://github.com/user-attachments/assets/0569c202-e7d8-43a7-b6ad-829fe761d31b" />

## Why

There is no good mobile app for managing coding agents while AFK.

Coding agent CLIs like Codex are good enough to handle most day-to-day coding such that devs almost never need to be in the editor.

The only reason we're still glued to our computers most the time is because there's no good way to code on your phone.

Remote terminals are not good enough. The labs' bolted-on "Code" features are underpowered afterthoughts.

We want unrestricted Codex running async, keeping us just updated enough to nudge them along when needed.

A good agent frees you from your computer, doesn't tie you to it.

## Tech stack

- React Native Expo
- TypeScript
- Rust
- Tinyvex (in‑process SQLite + WS changefeed)
- OpenAI Codex CLI
- Tailscale

## The name

Like a tricorder but for coding

## Contributing

You can submit PRs but they'd better be good.

Releases
- See `docs/bridge-release.md` for the oa-bridge release and tricoder publishing process.

## Version Compatibility

- v0.1.0 and v0.1.1 (mobile only): require the legacy bridge flow below. You must check out the old commit to match that app build: `git checkout 3cbd75e21a14951149d1c81a4ba0139676ffe935`.
- v0.2.0+ (desktop/tauri and forward): use the Desktop (Tauri) flow. The bridge is auto‑spawned, Convex runs as an embedded sidecar on 3210, and functions deploy automatically during `cargo tauri dev`.

## Desktop (Tauri) — v0.2+ (historical)

Single command, offline‑first dev.

1) Clone and run
   - `git clone https://github.com/OpenAgentsInc/openagents && cd openagents`
   - `bun run desktop:dev`
   - This script will:
     - Historical flow; current bridge uses Tinyvex for local sync. See docs/tinyvex/tinyvex.md.
     - Launch `cargo tauri dev` (bridge auto‑spawns, UI connects to `ws://127.0.0.1:8787/ws`)
3) Use it
   - Left sidebar shows Bridge WS, Convex (http://127.0.0.1:3210), and Codex PID
   - Recent threads load; click a thread to view messages (preface/instructions are hidden)

Options
- Historical Convex options no longer apply to Tinyvex.

## Legacy Bridge (Mobile) — v0.1.0 / v0.1.1

If you are using the early mobile builds (v0.1.x), use this legacy flow (ports/paths differ):

1) Clone and check out the legacy commit:
   - `git clone https://github.com/OpenAgentsInc/openagents && cd openagents`
   - `git checkout 3cbd75e21a14951149d1c81a4ba0139676ffe935`
2) Run the bridge (requires Rust toolchain):
   - `cargo bridge`
   - First run bootstrap:
     - Starts a local Convex backend on `0.0.0.0:7788`
     - Ensures Bun is installed, runs `bun install`
     - Installs the Convex local backend binary if missing
     - Deploys Convex schema/functions (`bun run convex:dev:once`) using a generated `.env.local`
     - Launches the Codex WebSocket bridge
3) Install the mobile app via TestFlight and connect:
   - Join TestFlight: https://testflight.apple.com/join/dvQdns5B
   - In the app, open the sidebar → Settings → set Bridge Host to your computer’s IP
   - The red dot turns green when connected

IP tips: Tailscale VPN works well to put devices on the same private network. It’s free and avoids local network headaches.

Any setup issues, DM us or open an issue.

Requirements (dev): Rust toolchain, `bash` + `curl` (for Bun install) and a working Node/npm. If Bun is already installed, we’ll use it.

Notes:
- You also need the Codex CLI installed and on your `PATH` (`codex --version`).
- Desktop (v0.2+): no manual `.env.local` needed; Tauri sets env when deploying functions to the sidecar.
- Mobile (v0.1.x): `.env.local` is generated on the first `cargo bridge` run (port 7788).

## Troubleshooting

- Bun not found / install fails
  - Ensure `curl` is available and network allows `https://bun.sh`.
  - You can preinstall Bun: `curl -fsSL https://bun.sh/install | bash`.
- Convex backend binary missing
  - The bridge will fetch it via the Convex CLI. If blocked, run: `bunx convex dev --once --skip-push --local-force-upgrade` once from the repo root, then rerun `cargo bridge`.
- Codex CLI not found
  - Install the Codex CLI and ensure `codex` is on your `PATH`.

## Stop / Restart

- Stop the bridge: `Ctrl+C` in the terminal running `cargo bridge`.
- Restart: run `cargo bridge` again (idempotent — bootstrap only happens when needed).

## Advanced

- Bind interface for Convex: set `OPENAGENTS_CONVEX_INTERFACE=127.0.0.1` to restrict to loopback (default is `0.0.0.0`).
- Skip bootstrap (if you manage Bun/Convex manually): set `OPENAGENTS_BOOTSTRAP=0`.

## Local Persistence

All builds now use Tinyvex (in‑process SQLite) for threads and messages.

- No external database process; the bridge serves WS snapshots and updates via `/ws`.
- For the historical Convex integration, see docs/convex/.

## Addendum (2025-10-26)

- Bridge docs clarified: Updated the `oa-bridge` entry comment to reflect the current architecture — WS is primarily for control and legacy JSONL streaming, while all persistence goes to Convex (threads, messages, tool rows). Filesystem watchers mirror projects/skills/sessions into Convex.
- Tauri sidebar: “New” thread button remains at the top of the Recent Threads block for quick thread creation; composer autofocus on selection is wired.
- Styling: Thread count badge keeps a fixed minimum width for better alignment and the status rows use a compact 12px type to keep the header condensed.
- Housekeeping: Verified no references to the removed legacy `events.rs`; remaining bridge helpers live in dedicated modules (`bootstrap`, `ws`, `convex_write`, `watchers`).
