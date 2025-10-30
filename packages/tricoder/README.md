OpenAgents Tricoder (CLI)

Desktop bridge for the OpenAgents mobile app. Currently iOS only via TestFlight — join here: https://testflight.apple.com/join/dvQdns5B

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
  - QR payload includes a prioritized `hosts[]` list (LAN and/or Tailscale). The app prefers `hosts[0]`.

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
- TRICODER_BRIDGE_VERSION
  - Pin to a specific GitHub Release tag for `oa-bridge` (e.g., `v0.2.3`). If the cached version differs, tricoder downloads the requested version.
- TRICODER_BRIDGE_FORCE_UPDATE
  - Set to `1` to bypass cache and fetch the latest release with matching assets.
- TRICODER_USE_PATH_BRIDGE
  - Set to `1` to prefer a bridge on your PATH over prebuilt/cargo.
- TRICODER_MIN_BRIDGE
  - Minimum prebuilt tag tricoder will accept (default `v0.2.2`). If an older cached binary is found, tricoder falls back to cargo to build latest from your local clone.

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

Verbose logging
- Add `--verbose` to print diagnostics:
  - Bind/port selection and fallback (e.g., `Bind: 0.0.0.0:8787`).
  - Token origin: persisted vs generated/persisted.
  - Claude CLI path: auto-detected from `~/.claude/local/claude` (exported via `CLAUDE_BIN`) or left to PATH.
  - Bridge binary details when using a prebuilt (path, version, source) and repo path when using cargo fallback.

Claude Code (headless)
- Tricoder prefers `~/.claude/local/claude` automatically so shell aliases don’t interfere.
- The bridge runs Claude in headless mode: `claude -p "<prompt>" --output-format stream-json --verbose`.
- The bridge emits:
  - `bridge.session_started` when Claude prints init; maps session → thread.
  - ACP events for both the user message (first turn) and assistant messages.
  - Error text from Claude (stderr or non‑JSON stdout) as error events and ACP agent messages — visible in UI and terminal logs.

Changelog
- 0.2.1
  - Claude Code: headless args aligned; session mapping + user_message emission; error surfacing.
  - Pairing: prevent scanner modal from re-opening; QR payload includes prioritized hosts[].
  - Bridge auto-update: compare cached vs latest release; `TRICODER_BRIDGE_VERSION` pin; `TRICODER_BRIDGE_FORCE_UPDATE=1` to bypass cache; minimum-prebuilt version gate with cargo fallback.
  - Logging: verbose prints for bind/port, token origin, CLAUDE_BIN path, bridge binary path/version, repo when using cargo.
- 0.2.0
  - Bridge runs by default; `--no-run` opt‑out.
  - Automatic port fallback.
  - Tailscale/LAN IP selection with multiple hosts in QR payload.
  - Removed legacy local network scanning and Convex fast‑start docs from README.
