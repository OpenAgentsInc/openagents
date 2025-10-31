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
- OpenAI Codex CLI or Claude Code CLI
- Tailscale (optional)

## The name

Like a tricorder but for coding

## Contributing

You can submit PRs but they'd better be good.

Releases
- See `docs/bridge-release.md` for the oa-bridge release and tricoder publishing process.

## Getting Started

How it works
- Download the app (TestFlight; Android coming soon).
- On your desktop, run a single command to get a pairing QR code.
- Scan the QR code from the app and start chatting.

Desktop (pairing QR)
- Regular (recommended): `npx tricoder@latest`
  - Runs the latest published Tricoder CLI, boots the bridge, and prints a pairing QR code.
- Development (from source): `cd packages/tricoder && bun dev`
  - Builds/runs from this repo and prints the same pairing QR code.
  - For contributor workflows and local changes.
Note: Scanning the QR is required; there is no Settings/manual input flow.

Pairing options
- Local network (Wi‑Fi): direct pairing on the same network; no accounts.
- Tailscale VPN: stay connected anywhere by installing Tailscale on both devices.

Requirements
- Bun (or Node) to run the Tricoder CLI.
- Providers on desktop as needed: OpenAI Codex CLI, Claude Code CLI (on PATH).

## Local Persistence

We use Tinyvex for fast, local‑first sync: an in‑process SQLite DB plus a WebSocket changefeed.

- Mirrors data for UI queries/sync; Codex JSONL rollouts remain the source of truth.
- Single SQLite file at `~/.openagents/tinyvex/data.sqlite3`; no external DB.
- App subscribes/queries over the bridge WS using `tvx.subscribe` / `tvx.query`.

Details: see docs/tinyvex/tinyvex.md for schema, protocol, and goals.
