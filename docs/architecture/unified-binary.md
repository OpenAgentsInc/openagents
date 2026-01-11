# Unified OpenAgents Binary Architecture

**Status**: Complete as of 2025-12-21

## Overview

OpenAgents uses a **single unified binary** that provides both a graphical user interface (GUI) and command-line interface (CLI). This architecture eliminates the need for multiple separate binaries and provides a consistent experience across all features.

```
openagents              # Launch GUI (default)
openagents wallet ...   # CLI subcommands
openagents marketplace ...
openagents autopilot ...
```

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│                    openagents binary                     │
├─────────────────────────────────────────────────────────┤
│  Entry Point: src/main.rs                               │
│  - Parse CLI args with clap                             │
│  - Route to GUI or CLI handler                          │
├──────────────────────┬──────────────────────────────────┤
│   GUI (src/gui/)     │   CLI (src/cli/)                 │
│   ├── app.rs         │   ├── wallet.rs                  │
│   ├── server.rs      │   ├── marketplace.rs             │
│   ├── routes/        │   ├── autopilot.rs               │
│   ├── views/         │   ├── gitafter.rs                │
│   ├── state.rs       │   └── daemon.rs                  │
│   └── ws.rs          │                                  │
└──────────────────────┴──────────────────────────────────┘
                         │
                         ▼
        ┌────────────────────────────────┐
        │   Library Crates (crates/)     │
        ├────────────────────────────────┤
        │  - wallet/                     │
        │  - marketplace/                │
        │  - autopilot/                  │
        │  - gitafter/                   │
        │  - nostr/                      │
        │  - ui/                         │
        │  - compute/                    │
        │  └── ...                       │
        └────────────────────────────────┘
```

## Directory Structure

```
openagents/
├── Cargo.toml                  # Workspace + unified binary config
├── src/                        # UNIFIED BINARY (thin wrapper)
│   ├── main.rs                 # Entry point, CLI parsing
│   ├── cli/                    # CLI command handlers
│   │   ├── mod.rs
│   │   ├── wallet.rs
│   │   ├── marketplace.rs
│   │   ├── autopilot.rs
│   │   ├── gitafter.rs
│   │   └── daemon.rs
│   │
│   └── gui/                    # GUI shell
│       ├── mod.rs
│       ├── app.rs              # Window creation (wry/tao)
│       ├── server.rs           # Actix server
│       ├── state.rs            # AppState (shared state)
│       ├── ws.rs               # WebSocket broadcaster
│       ├── routes/             # Route modules
│       │   ├── mod.rs
│       │   ├── wallet.rs
│       │   ├── marketplace.rs
│       │   ├── autopilot.rs
│       │   ├── gitafter.rs
│       │   └── daemon.rs
│       └── views/
│           ├── mod.rs
│           └── layout.rs       # HTML layouts
│
├── crates/                     # BUSINESS LOGIC
│   ├── wallet/
│   ├── marketplace/
│   ├── autopilot/
│   ├── gitafter/
│   ├── ui/                     # Shared components
│   ├── nostr/
│   └── ...
│
└── tests/                      # Integration tests
    ├── cli_integration.rs      # CLI command tests
    └── gui_server.rs           # GUI server tests
```

## Key Principles

### 1. Thin Wrapper Architecture

The `src/` directory contains **only thin wrappers**. All business logic lives in library crates under `crates/`.

**✅ GOOD:**
```rust
// src/cli/wallet.rs
pub fn run(cmd: WalletCommands) -> anyhow::Result<()> {
    wallet::cli::run(cmd)  // Delegate to library crate
}
```

**❌ BAD:**
```rust
// src/cli/wallet.rs
pub fn run(cmd: WalletCommands) -> anyhow::Result<()> {
    // Don't put business logic here!
    let wallet = Wallet::new()?;
    wallet.do_something()?;
    // ...
}
```

### 2. Library Crates Own Their Logic

Each feature has a library crate that exports:
- Core types and functions
- CLI command handlers
- GUI components/views
- Public API

### 3. Shared State via AppState

GUI routes share state through `AppState`:
```rust
pub struct AppState {
    pub broadcaster: Arc<WsBroadcaster>,
    pub active_tab: RwLock<Tab>,
    pub full_auto: RwLock<bool>,
    pub codex_info: RwLock<CodexInfo>,
}
```

## Adding a New CLI Subcommand

### Step 1: Create CLI Module

Create `src/cli/yourfeature.rs`:

```rust
use clap::Subcommand;

#[derive(Subcommand)]
pub enum YourFeatureCommands {
    /// Do something
    DoThing {
        #[arg(short, long)]
        name: String,
    },
}

pub fn run(cmd: YourFeatureCommands) -> anyhow::Result<()> {
    match cmd {
        YourFeatureCommands::DoThing { name } => {
            // Delegate to library crate
            yourfeature::do_thing(&name)?;
            println!("Done: {}", name);
            Ok(())
        }
    }
}
```

### Step 2: Register in mod.rs

Add to `src/cli/mod.rs`:

```rust
pub mod yourfeature;
```

### Step 3: Wire into main.rs

Add to `Commands` enum in `src/main.rs`:

```rust
#[derive(Subcommand)]
enum Commands {
    // ... existing commands

    /// Your feature commands
    #[command(subcommand)]
    Yourfeature(cli::yourfeature::YourFeatureCommands),
}
```

Add match arm:

```rust
let result = match cli.command {
    // ... existing matches
    Some(Commands::Yourfeature(cmd)) => cli::yourfeature::run(cmd),
};
```

### Step 4: Test It

```bash
cargo build
./target/debug/openagents yourfeature --help
./target/debug/openagents yourfeature do-thing --name test
```

## Adding a New GUI Route

### Step 1: Create Route Module

Create `src/gui/routes/yourfeature.rs`:

```rust
use actix_web::{web, HttpResponse};
use maud::html;
use crate::gui::state::AppState;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        .route("/", web::get().to(index))
        .route("/action", web::post().to(action));
}

async fn index(state: web::Data<AppState>) -> HttpResponse {
    let content = html! {
        div {
            h1 { "Your Feature" }
            p { "Content here" }
        }
    };

    HttpResponse::Ok()
        .content_type("text/html")
        .body(content.into_string())
}

async fn action(state: web::Data<AppState>) -> HttpResponse {
    // Handle action
    HttpResponse::Ok().body("Action complete")
}
```

### Step 2: Register Route

Add to `src/gui/routes/mod.rs`:

```rust
mod yourfeature;

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
        // ... existing routes
        .service(web::scope("/yourfeature").configure(yourfeature::configure));
}
```

### Step 3: Add Navigation

Update `src/gui/views/layout.rs` to add tab navigation if needed.

### Step 4: Test It

```bash
cargo run
# Navigate to http://localhost:<port>/yourfeature
```

## Adding Business Logic

**Never add business logic to `src/`!** Always create or extend a library crate.

### Step 1: Create Library Crate

```bash
mkdir -p crates/yourfeature/src
```

Create `crates/yourfeature/Cargo.toml`:

```toml
[package]
name = "yourfeature"
version.workspace = true
edition.workspace = true

[dependencies]
# Your dependencies
```

### Step 2: Implement Logic

In `crates/yourfeature/src/lib.rs`:

```rust
pub fn do_thing(name: &str) -> anyhow::Result<()> {
    // Your implementation
    Ok(())
}
```

### Step 3: Add to Workspace

Add to root `Cargo.toml`:

```toml
[workspace]
members = [
    # ... existing members
    "crates/yourfeature",
]

[dependencies]
yourfeature = { path = "crates/yourfeature" }
```

## Technology Stack

### GUI Stack

```
┌─────────────────────────────────────┐
│  wry/tao (native window)            │
├─────────────────────────────────────┤
│  Actix-web (local HTTP server)      │
├─────────────────────────────────────┤
│  Maud (HTML templating)             │
├─────────────────────────────────────┤
│  HTMX (dynamic updates)             │
├─────────────────────────────────────┤
│  WebSocket (live push updates)      │
└─────────────────────────────────────┘
```

### CLI Stack

- **clap**: Argument parsing with derive macros
- **anyhow**: Error handling
- **tokio**: Async runtime

## Testing Strategy

### CLI Integration Tests

Location: `tests/cli_integration.rs`

```rust
use assert_cmd::Command;

#[test]
fn test_wallet_help() {
    let mut cmd = Command::cargo_bin("openagents").unwrap();
    cmd.arg("wallet").arg("--help");
    cmd.assert().success();
}
```

### GUI Server Tests

Location: `tests/gui_server.rs`

```rust
use actix_web::test;

#[actix_web::test]
async fn test_route() {
    let app = test::init_service(
        App::new().configure(routes::configure)
    ).await;

    let req = test::TestRequest::get().uri("/").to_request();
    let resp = test::call_service(&app, req).await;
    assert!(resp.status().is_success());
}
```

### Unit Tests

Each library crate has its own unit tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_functionality() {
        // Test implementation
    }
}
```

## Development Workflow

### Building

```bash
# Build everything
cargo build

# Build release
cargo build --release

# Build specific crate
cargo build -p wallet
```

### Running

```bash
# GUI (default)
cargo run

# CLI commands
cargo run -- wallet whoami
cargo run -- marketplace compute providers
cargo run -- autopilot run "task"
```

### Testing

```bash
# All tests
cargo test

# CLI tests only
cargo test --test cli_integration

# GUI tests only
cargo test --test gui_server

# Specific crate
cargo test -p wallet
```

### Installing

```bash
# Install globally
cargo install --path .

# Now available as
openagents
openagents wallet whoami
```

## Common Patterns

### WebSocket Broadcasting

Routes can broadcast updates to all connected clients:

```rust
async fn action(state: web::Data<AppState>) -> HttpResponse {
    // Do something

    // Broadcast update to all clients
    state.broadcaster.broadcast(&html! {
        div id="update" { "New content" }
    }.into_string());

    HttpResponse::Ok().body("Done")
}
```

### Shared State Access

```rust
async fn handler(state: web::Data<AppState>) -> HttpResponse {
    // Read
    let value = state.full_auto.read().await;

    // Write
    let mut value = state.full_auto.write().await;
    *value = true;

    HttpResponse::Ok().body("OK")
}
```

### Async CLI Commands

For CLI commands that need async:

```rust
pub fn run(cmd: YourCommands) -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        match cmd {
            YourCommands::AsyncThing => {
                async_function().await?;
                Ok(())
            }
        }
    })
}
```

## Migration from Old Architecture

The following binaries were consolidated:

| Old Binary | New Command |
|-----------|-------------|
| `wallet` | `openagents wallet` |
| `marketplace` | `openagents marketplace` |
| `autopilot` | `openagents autopilot` |
| `autopilotd` | `openagents daemon` |
| `gitafter` | `openagents gitafter` |
| Desktop app | `openagents` (default) |

The `crates/desktop/` directory was deleted and merged into `src/gui/`.

## Best Practices

### 1. Keep src/ Minimal

The unified binary in `src/` should be:
- Thin wrappers over library crates
- No business logic
- Minimal dependencies
- Easy to understand

### 2. Library Crates Own Features

Each feature crate should:
- Export clean public API
- Contain all business logic
- Be testable independently
- Not depend on `src/`

### 3. Use Type Safety

Leverage Rust's type system:
- Use enums for commands
- Strong types over strings
- Result types for errors

### 4. Write Tests

Every feature should have:
- Unit tests in the crate
- Integration tests in `tests/`
- CLI test if it has commands
- GUI test if it has routes

### 5. Document Public APIs

Library crates should have:
- Doc comments on public items
- Examples in doc comments
- Module-level documentation
- README.md if complex

## Troubleshooting

### Binary not found after build

```bash
# Check what was built
ls target/debug/

# Should see: openagents

# Try running directly
./target/debug/openagents --help
```

### Command not recognized

Check that:
1. Command is in `Commands` enum
2. Module is in `src/cli/mod.rs`
3. Match arm exists in `main.rs`
4. Binary was rebuilt

### Route returns 404

Check that:
1. Route is configured in `routes/mod.rs`
2. Service scope is correct
3. Server was restarted
4. URI path matches scope + route

### State not shared between requests

Ensure:
1. Using `web::Data::new()` for state
2. State is `.app_data()` in App builder
3. Handler has `state: web::Data<AppState>`
4. Using Arc<RwLock<T>> for shared mutable state

## Further Reading

- [Actix-web Documentation](https://actix.rs/docs/)
- [clap Documentation](https://docs.rs/clap/)
- [Maud Documentation](https://maud.lambda.xyz/)
- [wry Documentation](https://docs.rs/wry/)

## Related Documentation

- `d-010` directive: Full specification
- `AGENTS.md`: Running commands
- Individual crate READMEs for feature details
