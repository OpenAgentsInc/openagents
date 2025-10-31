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

## Getting Started

Bridge (Rust)
- `cargo bridge`
- A WebSocket token is created on first run in `~/.openagents/bridge.json`.

Mobile app (Expo)
- `cd expo && bun install && bun run start`
- Launch on a device/simulator (`bun run ios|android|web`).
- In Settings, set Bridge Host (e.g., `ws://<your-ip>:8787/ws`) and paste the bridge token.

Requirements
- Rust toolchain and the OpenAI Codex CLI (`codex --version` must work).
- Bun for the Expo app (`curl -fsSL https://bun.sh/install | bash`).

## Local Persistence

All current builds use Tinyvex (in‑process SQLite) for threads and messages.
- No external database process; the bridge serves WS snapshots and updates via `/ws`.
