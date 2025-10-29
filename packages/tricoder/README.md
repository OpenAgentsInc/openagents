OpenAgents Tricoder (CLI)

Usage
- npx tricoder@latest [flags]

Flags
- --yes, -y
  - Assume consent for interactive steps (e.g., rustup install).
- --verbose, -v
  - Print detailed probes and tails (bridge events, Codex deltas, Convex writes).
- --rotate-token, -R
  - Rotate the bridge token and persist it to ~/.openagents/bridge.json (or $OPENAGENTS_HOME/bridge.json). Normally, tricoder reuses the persisted token so you don't need to rescan on every restart.
- --local-only
  - Skip public tunnels; run bridge on localhost only.
- --no-qr
  - Do not render the terminal QR.
- --qr=deeplink|code
  - QR contents (default = deeplink):
    - deeplink (default): openagents://connect?j=<code> — OS camera opens the app.
    - code: base64url code only — smaller QR, intended for in‑app scanner.
  - Rendering: Tricoder prints an ultra‑compact braille QR by default (~50% width/height). If your terminal supports inline images (iTerm2/WezTerm) or you set `TRICODER_QR_IMAGE=1`, it will render a PNG inline for perfect spacing.
- --delete
  - Danger: delete local OpenAgents clone and Convex artifacts to start fresh.
  - Removes:
    - ~/.openagents/openagents (auto‑cloned repo)
    - ~/.openagents/bin/local_backend (Convex local backend binary)
    - ~/.openagents/convex (local DB + storage)
  - Use `-y` to confirm non‑interactively (required in some npx environments with no TTY).

Notes
- Codex CLI
  - Tricoder warns when Codex CLI is older than 0.50.0.
- Convex
  - Tricoder supervises a local Convex backend and pushes functions automatically.
  - If Bun is unavailable, it falls back to npx convex dev one‑shot.

Environment
- TRICODER_PREFER
  - 'tailscale' (default) or 'lan' — prefer advertising a Tailscale IP when available, otherwise fall back to LAN IPv4. Set to 'lan' to force LAN even when Tailscale is connected.
- TRICODER_BRIDGE_PORT
  - Port for the oa-bridge bind and printed WS URL (default 8787). When unset, tricoder binds 0.0.0.0:8787; if set, it also sets TRICODER_BRIDGE_BIND to `0.0.0.0:<port>`.
- TRICODER_BRIDGE_BIND
  - Full bind address (e.g., 0.0.0.0:8787). Overrides TRICODER_BRIDGE_PORT.
