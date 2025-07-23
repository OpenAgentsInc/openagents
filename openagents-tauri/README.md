# OpenAgents Tauri

Cross-platform desktop app for OpenAgents with Claude Code integration.

## Development

```bash
# Install dependencies
bun install

# Run in development mode
bun run tauri dev

# Run with debug logging
./run-debug.sh
```

## Claude Code Integration

This app integrates with Claude Code CLI for AI-powered development. Make sure you have Claude Code installed:

```bash
npm install -g @anthropic-ai/claude-code
```

The app will automatically discover Claude Code on startup.

## Recommended IDE Setup

- [VS Code](https://code.visualstudio.com/) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer)
