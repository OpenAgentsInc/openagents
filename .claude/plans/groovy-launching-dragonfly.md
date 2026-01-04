# Plan: Auto-Start FM Bridge from Pylon Desktop

**Goal:** Pylon desktop should automatically start and manage the FM Bridge server, making it fully self-contained.

---

## Current State

- FM Bridge server is a **Swift binary** at `swift/foundation-bridge/`
- pylon-desktop currently assumes FM Bridge is already running externally
- `fm_runtime.rs` only creates an FMClient that connects to an existing server

---

## Architecture

```
┌─────────────────────────────────────────────────────┐
│                  pylon-desktop                       │
│  ┌──────────────────────────────────────────────┐   │
│  │  BridgeManager                                │   │
│  │  - spawn foundation-bridge binary             │   │
│  │  - wait for health check                      │   │
│  │  - kill on exit                               │   │
│  └──────────────────────────────────────────────┘   │
│            │ (spawns)                               │
│            ▼                                        │
│  ┌──────────────────────────────────────────────┐   │
│  │  foundation-bridge (Swift process)            │   │
│  │  - HTTP server on localhost:11435             │   │
│  │  - Apple Foundation Models API                │   │
│  └──────────────────────────────────────────────┘   │
│            ▲                                        │
│            │ (HTTP client)                          │
│  ┌──────────────────────────────────────────────┐   │
│  │  FmRuntime → FMClient                         │   │
│  │  - connects to http://localhost:11435         │   │
│  │  - streams tokens via SSE                     │   │
│  └──────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

---

## Implementation Steps

### 1. Create BridgeManager module

**New file:** `crates/pylon-desktop/src/bridge_manager.rs`

```rust
use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct BridgeManager {
    child: Option<Child>,
    port: u16,
}

impl BridgeManager {
    pub fn new(port: u16) -> Self { ... }

    /// Start the FM Bridge process
    pub fn start(&mut self) -> Result<(), BridgeError> {
        // Find binary (bundled or in swift/ directory)
        // Spawn with port argument
        // Store child handle
    }

    /// Wait for bridge to be healthy (with timeout)
    pub fn wait_ready(&self, timeout: Duration) -> Result<(), BridgeError> {
        // Poll GET /health until success or timeout
    }

    /// Get the base URL for FMClient
    pub fn url(&self) -> String {
        format!("http://localhost:{}", self.port)
    }
}

impl Drop for BridgeManager {
    fn drop(&mut self) {
        // Kill child process on exit
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
        }
    }
}
```

### 2. Find/bundle the Swift binary

**Binary location strategy:**
1. First check `~/.pylon/bin/foundation-bridge` (user-installed)
2. Then check relative to executable (bundled in app)
3. Finally check development path `swift/foundation-bridge/.build/release/foundation-bridge`

### 3. Update app.rs to use BridgeManager

**Changes to:** `crates/pylon-desktop/src/app.rs`

In `resumed()`:
```rust
// Before creating FmRuntime:
let mut bridge = BridgeManager::new(11435);
bridge.start()?;
bridge.wait_ready(Duration::from_secs(10))?;

// Update FMClient to use bridge URL
std::env::set_var("FM_BRIDGE_URL", bridge.url());

let fm_runtime = FmRuntime::new();
```

Store `bridge` in `RenderState` so it lives for app lifetime.

### 4. Update fm_runtime.rs

Use `FM_BRIDGE_URL` env var (already supported by fm-bridge client).

### 5. Handle startup errors gracefully

Show status in UI:
- "Starting FM Bridge..."
- "FM Bridge ready"
- "FM Bridge failed: {error}"

---

## Files to Modify/Create

| File | Action |
|------|--------|
| `src/bridge_manager.rs` | **New** - subprocess management |
| `src/main.rs` | Add `mod bridge_manager` |
| `src/app.rs` | Start bridge before FmRuntime |
| `src/state.rs` | Add bridge status to FmVizState |
| `src/ui/fm_panel.rs` | Show bridge startup status |

---

## Binary Discovery Order

```rust
fn find_bridge_binary() -> Option<PathBuf> {
    // 1. User-installed
    if let Some(home) = dirs::home_dir() {
        let path = home.join(".pylon/bin/foundation-bridge");
        if path.exists() { return Some(path); }
    }

    // 2. Bundled with app (macOS app bundle)
    if let Ok(exe) = std::env::current_exe() {
        let path = exe.parent()?.join("foundation-bridge");
        if path.exists() { return Some(path); }
    }

    // 3. Development path
    let dev_path = PathBuf::from("swift/foundation-bridge/.build/release/foundation-bridge");
    if dev_path.exists() { return Some(dev_path); }

    None
}
```

---

## Health Check Loop

```rust
async fn wait_ready(url: &str, timeout: Duration) -> Result<(), BridgeError> {
    let client = reqwest::Client::new();
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        match client.get(format!("{}/health", url)).send().await {
            Ok(resp) if resp.status().is_success() => return Ok(()),
            _ => tokio::time::sleep(Duration::from_millis(100)).await,
        }
    }

    Err(BridgeError::Timeout)
}
```

---

## Success Criteria

- [ ] `cargo pylon` starts FM Bridge automatically
- [ ] UI shows "Starting FM Bridge..." then "Connected"
- [ ] If bridge fails to start, show clear error message
- [ ] Bridge process killed cleanly on app exit
- [ ] Works with bundled binary or development path
