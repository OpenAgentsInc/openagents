# OpenAgents

A desktop app to command OpenAI Codex and other agents. Work in progress.

![OpenAgents Screenshot](docs/openagents-screenshot.png)

## Stack

- Rust
- Tauri
- Leptos 

## Documentation

### Local Development

To run the app locally, you'll need to set up a few dependencies:

#### Prerequisites

1. **Install Rust** (if not already installed):
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source ~/.cargo/env
   ```

2. **Install Tauri CLI**:
   ```bash
   cargo install tauri-cli
   ```

3. **Install Trunk** (for WebAssembly frontend builds):
   ```bash
   cargo install trunk
   ```

4. **Add WebAssembly target**:
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

#### Running the App

Once you have all dependencies installed, you can run the development server:

```bash
cargo tauri dev
```

This will start both the Rust backend and the Leptos frontend with hot reload enabled.

### Technical Documentation

- Overview of Codex systems docs: [docs/codex/README.md](docs/codex/README.md)
- Building a Chat UI with streaming: [docs/codex-chat-ui.md](docs/codex-chat-ui.md)
- Architecture: [docs/codex/architecture.md](docs/codex/architecture.md)
- Authentication: [docs/codex/authentication.md](docs/codex/authentication.md)
- Protocol overview: [docs/codex/protocol-overview.md](docs/codex/protocol-overview.md)
- Prompts: [docs/codex/prompts.md](docs/codex/prompts.md)
- Sandbox: [docs/codex/sandbox.md](docs/codex/sandbox.md)
- Tools: [docs/codex/tools.md](docs/codex/tools.md)
- Testing: [docs/codex/testing.md](docs/codex/testing.md)
