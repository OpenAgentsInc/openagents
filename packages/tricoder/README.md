OpenAgents Tricoder (CLI)

Quick Start
- npx tricoder@latest
  - Prints your Desktop IP, a QR code, a deep link, and the ws:// URL + token.
  - Runs the local Rust WebSocket bridge (oa-bridge) by default.
  - Scan the QR with the OpenAgents mobile app to connect.

Behavior
- QR + deep link
  - Shows an ultra‑compact terminal QR and the deep link: `openagents://connect?j=...`.
  - Payload includes the selected host, port, token, and a prioritized `hosts` list (LAN and/or Tailscale IPs).
- Bridge auto‑run
  - By default, tricoder starts the bridge locally. Use `--no-run` to only show the QR/deep link without launching the bridge.
  - Prefers a prebuilt `oa-bridge` binary (downloaded and cached); falls back to `cargo run -p oa-bridge` if no binary is available.
- Port fallback
  - Starts at `TRICODER_BRIDGE_PORT` (default 8787) and automatically picks the next available port if the preferred port is busy.
- Token persistence
  - Reuses `~/.openagents/bridge.json` token across runs; `--rotate-token` generates and persists a new token.
- Network selection
  - If Tailscale is active, advertises your Tailscale IPv4; otherwise uses your LAN IPv4. Override with `TRICODER_PREFER=lan`.

Flags
- --no-run
  - Do not launch the bridge; only print QR + deep link and exit.
- --rotate-token, -R
  - Rotate and persist the bridge token to `~/.openagents/bridge.json` (or `$OPENAGENTS_HOME/bridge.json`).
- --verbose, -v
  - Print additional diagnostics.

Environment
- TRICODER_PREFER
  - `tailscale` (default) or `lan` — choose which IP to advertise.
- TRICODER_BRIDGE_PORT
  - Starting port to probe (default 8787). Tricoder finds an available port automatically.
- TRICODER_BRIDGE_BIND
  - Full bind address, e.g. `0.0.0.0:8888`. Overrides the chosen port.
- TRICODER_PREFER_BIN
  - Set to `0` to force cargo fallback instead of using a prebuilt bridge binary.
- OPENAGENTS_REPO_DIR
  - Custom path for the auto‑cloned OpenAgents repo when falling back to cargo.

Output
- Desktop IP (LAN or Tailscale), deep link, ws URL, token, and optional hosts list.
- Example:
  - Desktop IP (LAN): 192.168.1.10
  - Deep link: openagents://connect?j=...
  - WS URL:    ws://192.168.1.10:8787/ws
  - Token:     abc...

Notes
- Requires Node 18+.
- Rust (cargo) is required only for the cargo fallback path when no prebuilt bridge binary is available.

Changelog
- 0.2.0
  - Bridge runs by default; `--no-run` opt‑out.
  - Automatic port fallback.
  - Tailscale/LAN IP selection with multiple hosts in QR payload.
  - Removed legacy local network scanning and Convex fast‑start docs from README.
