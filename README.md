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
3) Install the app via TestFlight and connect:
   - Join TestFlight: https://testflight.apple.com/join/dvQdns5B
   - In the app, open the sidebar → Settings → set Bridge Host to your computer’s IP
   - The red dot turns green when connected — you’re ready to go

IP tips: Tailscale VPN works well to put devices on the same private network. It’s free and avoids local network headaches.

Any setup issues, DM us or open an issue.

## Optional: Local Convex Persistence

We’ve added a self‑hosted Convex backend (SQLite) as an optional persistence layer for live subscriptions and richer queries. JSONL rollouts remain the source of truth for Codex resume.

- To have the bridge start Convex automatically:
  - `cargo run -p codex-bridge -- --with-convex`
- To push the sample schema/functions (one‑time):
  - See docs/convex.md for steps using `bun run convex:dev:once`
- The Convex screen in the app (Drawer → Convex) shows connection status and a live list once functions are deployed.

Details: docs/convex.md
