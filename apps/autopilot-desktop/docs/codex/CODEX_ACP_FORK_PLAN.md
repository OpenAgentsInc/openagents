# Plan: Fork and Build codex-acp from Source

## Current Situation

We're currently downloading pre-built `codex-acp` binaries from GitHub releases. This means:
- ❌ Can't modify it to send custom extensions
- ❌ Dependent on Zed team's release schedule
- ❌ No control over features

## Solution: Fork and Build from Source

Since `codex-acp` is all Rust, we can:
1. **Fork it into our repo** (git submodule or direct copy)
2. **Build it from source** as part of our build process
3. **Add custom extensions** for missing events (token usage, rate limits, etc.)

## Implementation Options

### Option 1: Git Submodule (Recommended)

**Pros**:
- Easy to pull updates from upstream
- Keeps our repo clean
- Can merge upstream changes

**Cons**:
- Slightly more complex git workflow
- Need to manage submodule updates

**Steps**:
```bash
# Add as submodule
git submodule add https://github.com/zed-industries/codex-acp vendor/codex-acp

# Or fork it first, then:
git submodule add https://github.com/YOUR_USERNAME/codex-acp vendor/codex-acp
```

### Option 2: Direct Copy (Simpler)

**Pros**:
- Simpler git workflow
- Full control, no submodule complexity

**Cons**:
- Harder to merge upstream updates
- Larger repo size

**Steps**:
```bash
# Clone into vendor directory
git clone https://github.com/zed-industries/codex-acp vendor/codex-acp
# Or fork first, then clone your fork
```

### Option 3: Cargo Workspace Member

**Pros**:
- Can build it as part of our workspace
- Share dependencies
- Type-safe integration

**Cons**:
- Need to restructure codex-acp slightly
- More complex build setup

## Recommended Approach: Git Submodule + Build Script

### 1. Add as Submodule

```bash
cd /Users/christopherdavid/code/autopilot
git submodule add https://github.com/zed-industries/codex-acp vendor/codex-acp
```

### 2. Modify Build Process

Update `src-tauri/build.rs` or add a build script to:
- Build `codex-acp` from source
- Copy binary to a known location
- Use that location instead of downloading

### 3. Modify codex-acp to Add Extensions

In the forked `codex-acp`, add code to:
- Listen to Codex app-server events (it already does this)
- Translate `account/rateLimits/updated` → `codex/tokenUsage` notification
- Translate `thread/tokenUsage/updated` → `codex/tokenUsage` notification
- Send these as ACP custom notifications

## Code Changes Needed

### In codex-acp (forked version):

1. **Add event listener for Codex events**:
```rust
// When receiving account/rateLimits/updated from codex app-server
if event.method == "account/rateLimits/updated" {
    // Send ACP custom notification
    send_acp_notification("codex/tokenUsage", json!({
        "sessionId": session_id,
        "rateLimits": event.params,
    }));
}
```

2. **Add custom notification sender**:
```rust
fn send_acp_notification(method: &str, params: Value) {
    let notification = json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
    });
    // Send to ACP client (us)
}
```

### In autopilot (our code):

1. **Update `find_codex_acp()` to build from source**:
```rust
async fn find_codex_acp() -> Option<String> {
    // First, try to find built binary in target directory
    let built_binary = std::path::Path::new("target/release/codex-acp");
    if built_binary.exists() {
        return Some(built_binary.to_string_lossy().to_string());
    }
    
    // Otherwise, build it
    build_codex_acp_from_source().await?;
    
    // Return path to built binary
    Some(built_binary.to_string_lossy().to_string())
}

async fn build_codex_acp_from_source() -> Result<()> {
    // Use cargo to build codex-acp from vendor/codex-acp
    // This could be done in build.rs or as a separate step
}
```

2. **Update build.rs to build codex-acp**:
```rust
// In src-tauri/build.rs
fn main() {
    // Build codex-acp from source
    std::process::Command::new("cargo")
        .args(&["build", "--release", "--manifest-path", "vendor/codex-acp/Cargo.toml"])
        .status()
        .expect("Failed to build codex-acp");
    
    // Copy binary to target directory
    // ...
}
```

## Benefits

1. ✅ **Full control** - We can add any extensions we need
2. ✅ **No dependency on releases** - Build from source
3. ✅ **Can still pull updates** - Git submodule allows merging upstream
4. ✅ **Type-safe** - Same Rust ecosystem, can share types if needed

## Next Steps

1. Fork `codex-acp` to your GitHub (or keep as submodule of upstream)
2. Add as git submodule: `git submodule add https://github.com/zed-industries/codex-acp vendor/codex-acp`
3. Modify `codex-acp` to send custom notifications
4. Update our build process to build from source
5. Update `find_codex_acp()` to use built binary

## Files to Modify

### In autopilot:
- `src-tauri/build.rs` - Add build step for codex-acp
- `crates/autopilot-desktop-backend/src/acp.rs` - Update `find_codex_acp()` to build from source
- `.gitmodules` - Will be created when adding submodule

### In codex-acp (forked):
- Event handler - Add listeners for Codex events
- Notification sender - Add custom notification methods
- Main loop - Wire up event → notification translation
