OpenAgents Tricoder (CLI)

Usage
- npx tricoder@latest [flags]

Flags
- --yes, -y
  - Assume consent for interactive steps (e.g., rustup install).
- --verbose, -v
  - Print detailed probes and tails (bridge events, Codex deltas, Convex writes).
- --local-only
  - Skip public tunnels; run bridge on localhost only.
- --no-qr
  - Do not render the terminal QR.
- --qr=deeplink|code
  - QR contents:
    - deeplink (default): openagents://connect?j=<code> — OS camera opens the app.
    - code: base64url code only — smaller QR, intended for in‑app scanner.
- --delete
  - Danger: delete local OpenAgents clone and Convex artifacts to start fresh.
  - Removes:
    - ~/.openagents/openagents (auto‑cloned repo)
    - ~/.openagents/bin/local_backend (Convex local backend binary)
    - ~/.openagents/convex (local DB + storage)
  - Combine with -y to skip confirmation.

Notes
- Codex CLI
  - Tricoder warns when Codex CLI is older than 0.50.0.
- Convex
  - Tricoder supervises a local Convex backend and pushes functions automatically.
  - If Bun is unavailable, it falls back to npx convex dev one‑shot.

