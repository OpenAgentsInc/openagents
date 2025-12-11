# OANIX Sprint 1 Implementation Log

**Date:** 2025-12-11
**Commit:** 73705eea0 (Add OANIX Sprint 1: MemFs + Browser Namespace Explorer)

---

## Summary

Implemented Sprint 1 of the OANIX roadmap: a working in-memory filesystem (`MemFs`) and a browser-based Namespace Explorer demo. The demo runs in WebAssembly and provides an interactive UI for exploring Plan 9-style namespaces.

---

## Files Created

### 1. MemFs Implementation
**File:** `crates/oanix/src/services/mem_fs.rs` (~350 lines)

In-memory filesystem implementing `FileService` trait:

```rust
pub struct MemFs {
    root: Arc<RwLock<MemNode>>,
}

enum MemNode {
    File { content: Vec<u8>, modified: u64 },
    Dir { children: HashMap<String, MemNode> },
}
```

**Features:**
- Full read/write support via `FileHandle`
- `open()` with flags: read, write, create, truncate, append
- `readdir()` - sorted directory listing
- `stat()` - file/directory metadata
- `mkdir()` - create directories
- `remove()` - delete files/directories
- `rename()` - move/rename files
- Thread-safe via `RwLock`
- Proper path normalization and traversal

**Tests:** 8 passing unit tests covering all operations

### 2. Services Module
**File:** `crates/oanix/src/services/mod.rs`

Module re-exports for filesystem implementations.

### 3. Web/WASM API
**File:** `crates/oanix/src/web/mod.rs` (~200 lines)

JavaScript-accessible API via `wasm-bindgen`:

```rust
#[wasm_bindgen]
pub struct OanixWeb {
    namespace: Namespace,
}
```

**Exposed Methods:**
- `new()` - Create runtime with default namespace (`/workspace`, `/tmp`)
- `empty()` - Create runtime with empty namespace
- `read_text(path)` / `read_bytes(path)` - Read file contents
- `write_text(path, content)` / `write_bytes(path, content)` - Write files
- `list_dir(path)` - List directory (returns JSON array)
- `stat(path)` - Get metadata (returns JSON object)
- `mkdir(path)` - Create directory
- `remove(path)` - Delete file/directory
- `exists(path)` - Check if path exists
- `mounts()` - List all mount points

### 4. Browser Demo
**Files:** `crates/oanix/examples/browser/`

- `index.html` - Tokyo Night themed UI (~380 lines CSS + HTML)
- `main.js` - JavaScript application (~280 lines)

**Features:**
- Left sidebar: Mount list + file tree view
- Right panel: Directory listing or file editor
- Create new files/directories via dialogs
- Edit and save files (Ctrl+S shortcut)
- Pre-populated sample content in `/workspace`:
  - `README.md` - OANIX documentation
  - `src/main.rs` - Sample Rust code
  - `Cargo.toml` - Sample manifest
  - `/tmp/notes.txt` - Temporary file

---

## Files Modified

### 1. `crates/oanix/Cargo.toml`

Added dependencies and configuration:

```toml
[lib]
crate-type = ["cdylib", "rlib"]  # Required for WASM

[dependencies]
uuid = { version = "1.0", features = ["v4", "serde", "js"] }  # Added js feature for WASM

# WASM dependencies (optional)
wasm-bindgen = { version = "0.2", optional = true }
js-sys = { version = "0.3", optional = true }
serde-wasm-bindgen = { version = "0.6", optional = true }
console_error_panic_hook = { version = "0.1", optional = true }

# Native-only (tokio doesn't work in WASM)
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
tokio = { version = "1.0", features = ["full"] }

[features]
browser = ["dep:wasm-bindgen", "dep:js-sys", "dep:serde-wasm-bindgen", "dep:console_error_panic_hook"]
```

### 2. `crates/oanix/src/lib.rs`

Added module exports:

```rust
pub mod services;

#[cfg(feature = "browser")]
pub mod web;

pub use services::MemFs;
```

### 3. `crates/oanix/src/service.rs`

Added `Serialize` derive to `DirEntry` and `Metadata` for JSON serialization in browser.

---

## Build & Run Instructions

```bash
cd crates/oanix

# Build WASM package
wasm-pack build --target web --features browser

# Serve demo
python -m http.server 8080

# Open browser
# http://localhost:8080/examples/browser/
```

---

## Technical Notes

### WASM Compatibility Issues Resolved

1. **Tokio doesn't work in WASM** - Moved tokio to native-only target dependencies
2. **UUID needs JS random source** - Added `js` feature to uuid crate
3. **Need cdylib crate type** - Added `[lib] crate-type = ["cdylib", "rlib"]`

### Architecture Decisions

- **Single `OanixWeb` struct** - Wraps `Namespace` and exposes all operations
- **Default namespace** - Pre-mounts `/workspace` and `/tmp` with `MemFs`
- **JSON serialization** - Uses `serde-wasm-bindgen` for `DirEntry` and `Metadata`
- **Error handling** - Converts Rust errors to `JsValue` strings

---

## Next Steps (Sprint 2)

1. Add wasmtime integration for WASI binary execution
2. Wire OANIX namespace mounts as WASI preopened directories
3. Create CLI example: `cargo run --example run_wasi -- hello.wasm`

---

## Wanix Patterns Applied

From studying `~/code/wanix`:

- **MemNode structure** - Similar to wanix's `memfs` node-based storage
- **Path normalization** - Handle leading/trailing slashes, empty paths
- **FileHandle pattern** - Buffered read/write with position tracking
- **Write-back on flush/drop** - Dirty tracking for efficient writes
