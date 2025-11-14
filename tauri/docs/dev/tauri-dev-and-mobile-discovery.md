# Tauri Dev Server, Ports, and iOS Mobile Discovery

This document explains how the Tauri/Vite dev server integrates with the desktop WebSocket runtime, how the iOS simulator connects to it, how ports are chosen, and how to troubleshoot when things don’t connect or stream.

Applies to the Tauri v2 app in `tauri/`.

## Big Picture

- Desktop runs two things during development:
  - Vite dev server (frontend) for hot‑reloading UI.
  - OpenAgents desktop runtime (Rust), which hosts the Tinyvex WebSocket server and the agent session manager (Codex/Claude Code via ACP).
- iOS app is a thin client:
  - It does not use the Vite dev URL; it loads embedded assets.
  - It discovers the desktop on the local network via mDNS and connects to the desktop Tinyvex WS server for streaming.
  - In the iOS Simulator, if mDNS doesn’t find anything, it falls back to scanning `127.0.0.1` for the desktop WS port.

## Commands You’ll Use Most

- Start desktop dev (Vite + desktop runtime):
  - `cd tauri`
  - `RUST_LOG=openagents_lib=info,mdns_sd=warn bun tauri dev`
  - Expect a log like: `WebSocket server listening on ws://0.0.0.0:9100/ws` (port may vary; see “Dynamic WS Port”).

- Start iOS simulator dev:
  - `cargo tauri ios dev "iPhone 16 Pro"`
  - If the app shows “No servers found”, tap “Try Again” after confirming desktop is running.

## Vite Dev Server (Port 1420)

- Tauri runs a wrapper `scripts/dev-tauri.ts` as `beforeDevCommand`:
  - If port `1420` is free → starts Vite.
  - If another Vite is already on `1420` → reuses it and keeps the process alive.
  - If `1420` is used by a non‑Vite process → prints a helpful error plus `lsof` output, and exits.
- You can override the default Vite ports:
  - `VITE_PORT` (defaults `1420`)
  - `VITE_HMR_PORT` (defaults `1421`)
  - Example: `VITE_PORT=1425 VITE_HMR_PORT=1426 bun tauri dev`
- Files involved:
  - `tauri/src-tauri/tauri.conf.json` → `beforeDevCommand: "bun run dev:tauri"`
  - `tauri/scripts/dev-tauri.ts` → wrapper logic
  - `tauri/vite.config.ts` → respects `VITE_PORT`/`VITE_HMR_PORT`

## Desktop WebSocket Server (Tinyvex)

- The desktop runtime hosts a WS server consumed by desktop UI and mobile clients.
- Dynamic port binding:
  - Tries `9100..9115`, then falls back to `:0` (OS‑assigned free port).
  - The chosen port is logged and used for mDNS advertising and for the default desktop WS URL.
- On macOS (desktop only), we also advertise via mDNS so mobile can discover it.
- On iOS, the app does NOT start a WS server; it only connects to one.
- Files involved:
  - `tauri/src-tauri/src/lib.rs` → WS server startup, dynamic port selection, iOS gating, `get_websocket_url()`
  - `tauri/src-tauri/src/tinyvex_ws.rs` → WS routes and control protocol

### mDNS Advertising and Discovery

- Advertising (desktop):
  - We publish `_openagents._tcp.local.` with explicit IPv4 addresses (non‑loopback if available).
  - If no LAN addresses are found, we fall back to `127.0.0.1` (helps simulator fallback).
  - File: `tauri/src-tauri/src/mdns_advertiser.rs`

- Discovery (iOS):
  - We browse for `_openagents._tcp.local.`; we prefer IPv4 addresses.
  - After discovery, the client tests reachability of each candidate.
  - If nothing is reachable, the simulator fallback scans `127.0.0.1` across `basePort..basePort+15`.
  - Files: `tauri/src-tauri/src/discovery.rs`, `tauri/src/lib/mobileServerDiscovery.ts`

### Simulator Fallback Logic

When discovery returns 0 or unreachable servers, the iOS Simulator tries connecting to the macOS host via loopback:

1. Read the desktop default WS URL (`get_websocket_url`) to get a base port (defaults to `9100`).
2. Test `127.0.0.1:<base..base+15>` until one connects.
3. If a port is found, the app connects and saves it as “localhost”.

This does not change real‑device behavior; real devices rely on mDNS and must be on the same network as the desktop.

## Typical Flows

### Desktop‑only

1. `bun tauri dev`
2. Watch for:
   - `WebSocket server listening on ws://0.0.0.0:<PORT>/ws`
   - `mDNS service advertising started: _openagents._tcp.local:<PORT>`
3. Desktop UI connects automatically to `ws://127.0.0.1:<PORT>/ws`.

### iOS Simulator

1. Ensure desktop is running (above).
2. `cargo tauri ios dev "iPhone 16 Pro"`
3. The app will:
   - Try mDNS → prefer reachable IPv4 server.
   - If none → scan `127.0.0.1` ports.
4. If you see “No servers found”, tap “Try Again” after verifying the desktop is still running.

### Real Device (same LAN)

1. Ensure the desktop advertises via mDNS (see logs) and the WS server is listening.
2. Connect the device to the same network; ensure multicast/mDNS isn’t blocked by your network.
3. Launch the app; it should discover and connect automatically.

## Troubleshooting

### “Port 1420 is already in use” during dev

- Wrapper behavior:
  - If a Vite is already on `1420`, we reuse it and keep the process alive.
  - If another process is on `1420`, we print who is listening and exit.
- Inspect and fix:
  - `lsof -nP -iTCP:1420 -sTCP:LISTEN`
  - Stop that process, or run with `VITE_PORT`/`VITE_HMR_PORT` overrides.

### iOS shows “No servers found” or connects but no streaming

1. Confirm desktop WS server is running and advertising:
   - Look for `WebSocket server listening on ws://0.0.0.0:<PORT>/ws`.
   - Optional mDNS checks:
     - `dns-sd -B _openagents._tcp`
     - `dns-sd -L "OpenAgents Desktop" _openagents._tcp`
2. Simulator fallback will try `127.0.0.1` ports automatically; make sure the desktop is running on the same Mac.
3. For real devices, ensure the device and desktop are on the same Wi‑Fi and mDNS is allowed (some enterprise networks block multicast).

### iOS console shows `mdns_sd::service_daemon ... sending on a closed channel`

- Harmless noise from the `mdns-sd` crate during shutdown of a quick discovery scan.
- We filter to `mdns_sd=warn` in the suggested `RUST_LOG` to reduce noise.

### I see WebSocket connects/disconnects but no agent responses

- Remember: mobile streams from the desktop. The agent processes (Codex/Claude Code) run on the desktop.
- If the UI shows “Working…” forever, check the desktop logs for ACP spawn/auth messages:
  - You should see: `spawning ACP agent`, `ACP initialize completed`, `ACP new_session completed` and streaming updates.
  - If your agent requires credentials, provide them in the desktop environment:
    - Codex/OpenAI: `export CODEX_API_KEY=...` or `export OPENAI_API_KEY=...`
    - Claude Code: `export ANTHROPIC_API_KEY=...`
  - Restart `bun tauri dev` after exporting keys.

### “WebSocket protocol error: Connection reset without closing handshake”

- Usually transient reconnects (window reload, dev server refresh) or switching between threads.
- If persistent, confirm only one desktop instance is running and firewall is not blocking local connections.

## Diagnostics & Useful Env

- Desktop logs:
  - `RUST_LOG=openagents_lib=info,mdns_sd=warn bun tauri dev`
  - Raise to `RUST_LOG=info` to see more (WS connection requests, ACP client stdout lines, etc.).
- Port inspection:
  - `lsof -nP -iTCP:9100-9120 -sTCP:LISTEN`
- mDNS inspection (macOS):
  - `dns-sd -B _openagents._tcp`
  - `dns-sd -L "OpenAgents Desktop" _openagents._tcp`

## Implementation Notes (for maintainers)

- Dev command wrapper:
  - `scripts/dev-tauri.ts` checks for a Vite server on `1420` (IPv4/IPv6), reuses if present, otherwise launches Vite or errors with guidance. This avoids `beforeDevCommand` crashes.
- Desktop WS port selection:
  - Implemented in `src-tauri/src/lib.rs`. We store the selected port and use it for `get_websocket_url()` and mDNS.
- mDNS publishing and discovery:
  - Desktop publishes IPv4 addresses when possible; iOS discovery prefers IPv4 to match our `0.0.0.0` bind and avoid `::1` surprises.
- iOS build mode:
  - iOS loads embedded assets (not Vite). The mobile app is a thin client and never starts its own Tinyvex WS.

## FAQ

- Q: Can I force a specific desktop WS port?
  - A: The desktop tries `9100..9115` before falling back to auto. Today there’s no env override in code; if you need a fixed port for a demo/network policy, we can add a small `OA_TINYVEX_PORT` override.

- Q: Can I enter `ws://host:port/ws` manually on mobile?
  - A: Not yet — discovery + simulator fallback are automatic. If you need manual entry, open an issue; it’s straightforward to add a small override screen.

- Q: Why reuse Vite on 1420?
  - A: Tauri’s dev tooling expects a consistent dev URL. Reusing avoids duplicate servers and keeps Tauri happy.

