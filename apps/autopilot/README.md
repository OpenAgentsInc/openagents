# Autopilot Tauri Shell

This is the starter Tauri shell for the next Autopilot desktop product surface.

The app uses:

- Tauri 2 for the desktop host and Rust IPC.
- React and TypeScript for the product UI.
- Bun for package management and local frontend commands.

Run from this directory:

```bash
bun install
bun run tauri dev
```

Keep privileged state and authority in Rust-backed commands, events, channels,
or lower-level OpenAgents services. Keep the TypeScript UI as product projection
and interaction.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
