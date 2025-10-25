# OpenAgents: Project Tricoder

A mobile command center for your coding agents.

<img width="600" height="431" alt="tricoder" src="https://github.com/user-attachments/assets/a125680d-0c3e-4703-83ac-510385b43e3e" />

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
- Convex (self‑hosted local backend; auto‑bootstrapped)
- OpenAI Codex CLI
- Tailscale

## The name

Like a tricorder but for coding

## Contributing

You can submit PRs but they'd better be good.

## Quick Start

Basics to connect to your local Codex:

1) Clone the repo:
   - `git clone https://github.com/OpenAgentsInc/openagents && cd openagents`
2) Run the bridge (requires Rust toolchain):
   - `cargo bridge`
   - What this does the first time:
     - Starts a local Convex backend on `0.0.0.0:7788`
    - Ensures Bun is installed (via bun.sh) and runs `bun install`
    - Downloads and installs the Convex local backend binary if missing
    - Deploys Convex schema/functions (`bun run convex:dev:once`) using a generated `.env.local`
     - Launches the Codex WebSocket bridge
3) Install the app via TestFlight and connect:
   - Join TestFlight: https://testflight.apple.com/join/dvQdns5B
   - In the app, open the sidebar → Settings → set Bridge Host to your computer’s IP
   - The red dot turns green when connected — you’re ready to go

IP tips: Tailscale VPN works well to put devices on the same private network. It’s free and avoids local network headaches.

Any setup issues, DM us or open an issue.

Requirements: Rust toolchain, `bash` + `curl` (for Bun install) and a working Node/npm. If Bun is already installed, we’ll use it.

Notes:
- You also need the Codex CLI installed and on your `PATH` (`codex --version`).
- No manual `.env.local` setup is needed — the bridge writes it on first run.

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

## Local Convex Persistence (Required)

The app and bridge use a self‑hosted Convex backend (SQLite) for all threads and messages. The mobile app subscribes to Convex for live updates; the bridge mirrors Codex JSONL into Convex and also consumes pending runs from Convex to drive the Codex CLI. JSONL rollouts remain the source of truth for Codex resume.

- Zero‑setup: just run `cargo bridge`. The bridge will start Convex, install Bun if needed, create `.env.local` for you, and deploy the Convex functions automatically.
- The Convex screen in the app (Drawer → Convex) shows connection status and a live list once functions are deployed.

Details and advanced notes: docs/convex.md
