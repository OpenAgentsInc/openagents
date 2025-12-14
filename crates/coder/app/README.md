# Coder App

The main application crate for Coder - works on both desktop and web.

## Running the App

### Desktop (Native)

Run the app natively with GPU acceleration:

```bash
cargo desktop
# or
cargo run -p coder_app
```

### Web (WASM)

Run the app in a web browser:

```bash
cargo web
```

This will:
1. Build the WASM binary with wasm-pack
2. Start a local server on http://localhost:8080
3. Open your browser to view the app

**Prerequisites:**
- wasm-pack: `curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh`
- Python 3 (for the local server)

## Architecture

Coder uses a custom "own all six layers" UI stack:

1. **Domain Model** (`coder_domain`) - Event sourcing with entities and projections
2. **UI Runtime** (`coder_ui_runtime`) - Fine-grained reactivity (Signal<T>, Memo<T>, Effect)
3. **Widgets** (`coder_widgets`) - Composable UI building blocks
4. **Surfaces** (`coder_surfaces_*`) - Higher-level UI components (Chat, Terminal, Diff, Timeline)
5. **Shell** (`coder_shell`) - Navigation, routing, chrome
6. **Renderer** (`wgpui`) - GPU-accelerated rendering with wgpu

## Features

- **Streaming Markdown** - Real-time markdown rendering with syntax highlighting
- **GPU Acceleration** - Hardware-accelerated rendering via wgpu (WebGPU/WebGL/Vulkan/Metal)
- **Cross-Platform** - Works on macOS, Linux, Windows, and Web (WASM)
- **Reactive UI** - Fine-grained reactivity with automatic dependency tracking
- **Event Sourcing** - Domain events for state management

## Demo

The default home screen displays a streaming markdown demo showcasing:
- Syntax-highlighted code blocks
- Bold, italic, inline code
- Blockquotes with styled accent bars
- Ordered and unordered lists
- Smooth fade-in animations

## Development

See [GETTING_STARTED.md](../docs/GETTING_STARTED.md) for full development instructions.
