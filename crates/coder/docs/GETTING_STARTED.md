# Getting Started with Coder Development

This guide helps you build, run, and develop Coder from source.

## Prerequisites

### Required

- **Rust 1.75+** (edition 2024 support)
- **wgpu-compatible GPU** (any modern GPU from the last 5 years)
- **Git** for version control

### Platform-Specific

#### macOS
```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Xcode Command Line Tools (for Metal)
xcode-select --install
```

#### Linux
```bash
# Install Rust via rustup
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install dependencies (Ubuntu/Debian)
sudo apt-get install libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev \
    libxkbcommon-dev libssl-dev pkg-config

# Or for Fedora
sudo dnf install libxcb-devel libxkbcommon-devel openssl-devel pkg-config
```

#### Windows
```powershell
# Install Rust via rustup
# Download from: https://rustup.rs/

# Ensure you have Visual Studio Build Tools or Visual Studio
# with "Desktop development with C++" workload
```

### Optional

- **VS Code** with rust-analyzer extension
- **cargo-watch** for auto-rebuild: `cargo install cargo-watch`
- **cargo-nextest** for faster tests: `cargo install cargo-nextest`

## Clone and Build

```bash
# Clone the repository
git clone https://github.com/OpenAgentsInc/openagents.git
cd openagents

# Build all crates
cargo build

# Or build just coder_app
cargo build -p coder_app

# Or use the alias
cargo coder --help
```

**First build** takes 5-10 minutes to compile dependencies. Subsequent builds are much faster (~30 seconds).

## Running Coder

### Development Mode

```bash
# Run with debug symbols (slower, better errors)
cargo run -p coder_app

# Or use the alias
cargo coder
```

### Release Mode

```bash
# Optimized build (2-3x faster runtime)
cargo run -p coder_app --release

# Or build then run
cargo build -p coder_app --release
./target/release/coder_app
```

### With Hot Reload (Future)

```bash
# Watch for changes and auto-rebuild
cargo watch -x 'run -p coder_app'
```

## Project Structure

```
crates/coder/
├── app/              # Main application
├── shell/            # Routing, navigation, chrome
├── surfaces_chat/    # Chat UI
├── surfaces_terminal/# Terminal emulator
├── surfaces_diff/    # Diff viewer
├── surfaces_timeline/# Timeline visualization
├── widgets/          # UI building blocks
├── ui_runtime/       # Reactive runtime
├── domain/           # Domain model
├── protocol/         # Wire protocol
└── docs/             # Documentation
```

## Development Workflow

### 1. Make Changes

Edit files in your preferred editor. We recommend **VS Code** with **rust-analyzer**.

```bash
# Open in VS Code
code .
```

**Recommended VS Code Settings** (`.vscode/settings.json`):
```json
{
  "rust-analyzer.checkOnSave.command": "clippy",
  "rust-analyzer.cargo.features": "all",
  "editor.formatOnSave": true,
  "files.watcherExclude": {
    "**/target/**": true
  }
}
```

### 2. Check Code

```bash
# Run clippy (linter)
cargo clippy --all-targets --all-features

# Format code
cargo fmt
```

### 3. Run Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p coder_domain

# Run a specific test
cargo test -p coder_domain test_message_added

# With output
cargo test -- --nocapture

# Using nextest (faster)
cargo nextest run
```

### 4. Build

```bash
# Debug build
cargo build -p coder_app

# Release build (optimized)
cargo build -p coder_app --release
```

### 5. Run

```bash
# Run the application
cargo run -p coder_app

# With arguments
cargo run -p coder_app -- --help

# With environment variables
RUST_LOG=debug cargo run -p coder_app
```

## Common Tasks

### Adding a New Widget

1. **Create the widget file**:
```bash
touch crates/coder/widgets/src/my_widget.rs
```

2. **Implement the Widget trait**:
```rust
// crates/coder/widgets/src/my_widget.rs

use crate::*;
use wgpui::*;

pub struct MyWidget {
    text: String,
}

impl MyWidget {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
        }
    }
}

impl Widget for MyWidget {
    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // Paint background
        cx.scene.draw_quad(
            bounds,
            theme::bg::SURFACE,
            None,
            4.0,
        );

        // Paint text
        cx.scene.draw_text(
            &self.text,
            bounds.origin,
            cx.text_system,
            16.0,
            theme::text::PRIMARY,
        );
    }

    fn event(&mut self, event: &InputEvent, bounds: Bounds, cx: &mut EventContext) -> EventResult {
        // Handle events
        EventResult::Unhandled
    }
}
```

3. **Export from lib.rs**:
```rust
// crates/coder/widgets/src/lib.rs

mod my_widget;
pub use my_widget::*;
```

4. **Use in your UI**:
```rust
MyWidget::new("Hello, world!")
```

### Adding a New Event

1. **Add to DomainEvent enum**:
```rust
// crates/coder/domain/src/event.rs

pub enum DomainEvent {
    // ... existing events

    MyNewEvent {
        some_id: SomeId,
        data: String,
        timestamp: DateTime<Utc>,
    },
}
```

2. **Handle in projections**:
```rust
// crates/coder/domain/src/projections/chat_view.rs

impl ChatView {
    pub fn apply(&mut self, event: &DomainEvent) {
        match event {
            DomainEvent::MyNewEvent { some_id, data, timestamp } => {
                // Update projection
                // ...
            }
            // ... other events
        }
    }
}
```

3. **Handle in AppState**:
```rust
// crates/coder/app/src/state.rs

impl AppState {
    pub fn apply_event(&mut self, event: DomainEvent) {
        match &event {
            DomainEvent::MyNewEvent { some_id, .. } => {
                // Update state
                // ...
            }
            // ... other events
        }
    }
}
```

### Adding a New Command

1. **Add to Command enum**:
```rust
// crates/coder/ui_runtime/src/command.rs

pub enum Command {
    // ... existing commands

    MyNewCommand {
        param1: String,
        param2: u32,
    },
}
```

2. **Register handler**:
```rust
// crates/coder/app/src/app.rs

impl App {
    pub fn init(&mut self) {
        // ... other setup

        self.commands.register(|cmd: &Command| {
            match cmd {
                Command::MyNewCommand { param1, param2 } => {
                    // Handle command
                    // ...
                    CommandResult::Success
                }
                _ => CommandResult::Unhandled,
            }
        });
    }
}
```

3. **Dispatch from UI**:
```rust
cx.commands.dispatch(Command::MyNewCommand {
    param1: "value".into(),
    param2: 42,
});
```

## Debugging

### Logging

Coder uses the `log` crate for logging:

```rust
use log::{debug, info, warn, error};

info!("Application started");
debug!("Chat view updated: {:?}", view);
warn!("Connection lost, retrying...");
error!("Failed to send message: {}", err);
```

**Enable logging**:
```bash
# All logs
RUST_LOG=debug cargo run -p coder_app

# Specific module
RUST_LOG=coder_app=debug cargo run -p coder_app

# Multiple modules
RUST_LOG=coder_app=debug,coder_ui_runtime=trace cargo run -p coder_app
```

### Debugging with LLDB (macOS/Linux)

```bash
# Build with debug symbols
cargo build -p coder_app

# Run with debugger
rust-lldb ./target/debug/coder_app

# Set breakpoint
(lldb) b coder_app::app::App::init
(lldb) run

# Inspect variables
(lldb) frame variable
(lldb) p some_variable
```

### Debugging with VS Code

Create `.vscode/launch.json`:

```json
{
  "version": "0.2.0",
  "configurations": [
    {
      "type": "lldb",
      "request": "launch",
      "name": "Debug coder_app",
      "cargo": {
        "args": [
          "build",
          "-p",
          "coder_app"
        ],
        "filter": {
          "name": "coder_app",
          "kind": "bin"
        }
      },
      "args": [],
      "cwd": "${workspaceFolder}",
      "env": {
        "RUST_LOG": "debug"
      }
    }
  ]
}
```

Press F5 to start debugging.

### Performance Profiling

#### CPU Profiling (macOS)

```bash
# Build with release symbols
cargo build -p coder_app --release

# Run with Instruments
instruments -t "Time Profiler" ./target/release/coder_app
```

#### CPU Profiling (Linux)

```bash
# Install perf
sudo apt-get install linux-tools-generic

# Record
cargo build -p coder_app --release
sudo perf record -F 99 -g ./target/release/coder_app

# Analyze
sudo perf report
```

#### GPU Profiling

Use **RenderDoc** or **PIX** (Windows) to profile GPU usage.

```bash
# Linux
renderdoc ./target/release/coder_app

# macOS (use Xcode's GPU profiler)
open -a Xcode ./target/release/coder_app
```

## Testing Strategy

### Unit Tests

Test individual functions and types:

```rust
// crates/coder/domain/src/event.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_added() {
        let event = DomainEvent::MessageAdded {
            thread_id: ThreadId::new(),
            message_id: MessageId::new(),
            content: "Test".into(),
            role: Role::User,
            tool_uses: SmallVec::new(),
            timestamp: Utc::now(),
        };

        // Assertions
        assert!(matches!(event, DomainEvent::MessageAdded { .. }));
    }
}
```

Run with:
```bash
cargo test -p coder_domain
```

### Integration Tests

Test multiple components together:

```rust
// crates/coder/app/tests/integration_test.rs

#[test]
fn test_send_message_flow() {
    let mut app = App::new();
    let thread_id = ThreadId::new();

    // Create thread
    app.state.threads.insert(thread_id, Signal::new(ChatView::new(thread_id)));

    // Send message
    app.commands.dispatch(Command::SendMessage {
        thread_id,
        content: "Hello".into(),
    });

    // Process commands
    app.commands.process();

    // Verify message was sent (mock backend)
    // ...
}
```

### Benchmark Tests

Measure performance:

```rust
// crates/coder/widgets/benches/virtual_list.rs

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_virtual_list_render(c: &mut Criterion) {
    let mut list = VirtualList::new()
        .item_height(60.0)
        .item_count(10_000);

    c.bench_function("virtual_list_render", |b| {
        b.iter(|| {
            list.paint(black_box(bounds), black_box(&mut cx));
        });
    });
}

criterion_group!(benches, bench_virtual_list_render);
criterion_main!(benches);
```

Run with:
```bash
cargo bench -p coder_widgets
```

## Troubleshooting

### Build Errors

**Error**: `error: linking with 'cc' failed`

**Solution**: Install required development libraries (see Prerequisites).

---

**Error**: `error: wgpu backend not found`

**Solution**: Ensure you have a compatible GPU and drivers. wgpu requires Vulkan (Linux/Windows), Metal (macOS), or DX12 (Windows).

---

**Error**: `thread 'main' panicked at 'failed to create wgpu adapter'`

**Solution**: Your GPU may not support wgpu. Try software rendering:
```bash
WGPU_BACKEND=gl cargo run -p coder_app
```

### Runtime Errors

**Error**: Signal updates not triggering UI refresh

**Solution**: Ensure you're calling `.get()` inside an Effect or Memo to subscribe. Check that the signal is being `.set()` or `.update()`d.

---

**Error**: Widget not rendering

**Solution**:
1. Check that widget is added to parent's children
2. Verify bounds are non-zero
3. Enable debug logging: `RUST_LOG=coder_widgets=debug`

---

**Error**: High memory usage

**Solution**:
1. Check for signal/effect leaks (not being disposed)
2. Ensure virtual scrolling is used for large lists
3. Profile with Instruments/Valgrind

### Performance Issues

**Problem**: Low FPS (<30)

**Solutions**:
1. Build in release mode: `cargo build --release`
2. Reduce draw calls (batch similar widgets)
3. Use virtual scrolling for large lists
4. Profile with instruments/perf

---

**Problem**: High CPU usage when idle

**Solutions**:
1. Check for infinite effect loops
2. Ensure scheduler is sleeping between frames
3. Disable unnecessary effects

## Contributing

### Code Style

- Follow Rust style guidelines (enforced by `rustfmt`)
- Use `clippy` to catch common mistakes
- Write doc comments for public APIs
- Add tests for new features

### Commit Messages

Follow conventional commits:

```
feat: add support for inline code blocks
fix: prevent infinite loop in reactive graph
docs: update getting started guide
refactor: simplify widget composition API
test: add tests for ChatView projection
```

### Pull Requests

1. Create a feature branch: `git checkout -b feature/my-feature`
2. Make changes and commit
3. Push to your fork: `git push origin feature/my-feature`
4. Open a PR on GitHub

## Next Steps

- Read [ARCHITECTURE.md](./ARCHITECTURE.md) for system design
- Read [DOMAIN_MODEL.md](./DOMAIN_MODEL.md) for event sourcing
- Read [REACTIVE_RUNTIME.md](./REACTIVE_RUNTIME.md) for signals/effects
- Read [DATA_FLOW.md](./DATA_FLOW.md) for data flow patterns

## Getting Help

- **Documentation**: Check the `docs/` directory
- **Examples**: Look at existing widgets and surfaces
- **Issues**: Open an issue on GitHub
- **Discord**: Join our community server (if available)

Happy coding!
