# OANIX Sprint 2: WASI Runtime Implementation

**Date:** 2025-12-11
**Commit:** 64aa04126 (Add OANIX Sprint 2: WASI runtime with wasmtime)

---

## Summary

Implemented Sprint 2 of the OANIX roadmap: a WASI runtime using wasmtime that executes WebAssembly modules within OANIX namespaces. The WASI module can read/write files in mounted filesystems.

---

## What Works

```bash
$ cargo run --features wasi --example run_wasi -- examples/hello-wasi/target/wasm32-wasip1/release/hello-wasi.wasm

Loading WASM module: examples/hello-wasi/target/wasm32-wasip1/release/hello-wasi.wasm
  Size: 114121 bytes

Namespace mounts:
  /workspace
  /tmp

Running WASM module...
----------------------------------------
=== OANIX WASI Test ===

Arguments:
  argv[0] = examples/hello-wasi/target/wasm32-wasip1/release/hello-wasi.wasm

Environment:
  OANIX_VERSION = 0.1.0
  HOME = /workspace

Reading /workspace/hello.txt:
  Content: Hello from OANIX namespace!

Listing /workspace:
  file       28 hello.txt
  dir        60 data

Writing /tmp/output.txt:
  Success!

Creating /tmp/test-dir:
  Success!

=== Test Complete ===
----------------------------------------

Execution complete:
  Exit code: 0
Success!
```

---

## Files Created

### 1. WASI Module
**File:** `src/wasi/mod.rs`

Module exports for WASI runtime.

### 2. WasiRuntime Implementation
**File:** `src/wasi/runtime.rs` (~280 lines)

```rust
pub struct WasiRuntime {
    engine: wasmtime::Engine,
}

pub struct RunConfig {
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub working_dir: Option<String>,
}

pub struct RunResult {
    pub exit_code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
```

Key methods:
- `new()` - Create runtime with wasmtime engine
- `run(namespace, wasm_bytes, config)` - Execute WASI module
- `prepare_mounts()` - Sync namespace to temp directories
- `sync_to_host()` - Copy FileService contents to host filesystem
- `sync_from_host()` - Copy modified files back to FileService

### 3. CLI Example
**File:** `examples/run_wasi.rs`

Command-line tool to run WASI binaries:
```bash
cargo run --features wasi --example run_wasi -- <wasm-file> [args...]
```

### 4. Test WASI Binary
**Files:** `examples/hello-wasi/`

Simple Rust program that compiles to WASI and demonstrates:
- Reading arguments and environment
- Reading files from /workspace
- Listing directories
- Creating files and directories in /tmp

Build with:
```bash
cd examples/hello-wasi
cargo build --target wasm32-wasip1 --release
```

---

## Architecture: Namespace-to-WASI Bridge

The bridge works by syncing FileService contents to temporary directories:

```
┌─────────────────────────────────────────────────────┐
│                   OANIX Namespace                    │
│  /workspace (MemFs) ──┐                             │
│  /tmp (MemFs) ────────┼─── sync_to_host() ──────┐   │
│  /logs (MemFs) ───────┘                         │   │
└─────────────────────────────────────────────────│───┘
                                                  │
                                                  ▼
┌─────────────────────────────────────────────────────┐
│              Temporary Directories                   │
│  /tmp/oanix-{uuid}/workspace/                       │
│  /tmp/oanix-{uuid}/tmp/                             │
│  /tmp/oanix-{uuid}/logs/                            │
└────────────────────────────────────┬────────────────┘
                                     │
                        WASI preopened dirs
                                     │
                                     ▼
┌─────────────────────────────────────────────────────┐
│                   WASI Module                        │
│  - Reads /workspace/hello.txt                       │
│  - Writes /tmp/output.txt                           │
│  - Creates /tmp/test-dir/                           │
└────────────────────────────────────┬────────────────┘
                                     │
                        sync_from_host()
                                     │
                                     ▼
┌─────────────────────────────────────────────────────┐
│          Changes synced back to MemFs               │
└─────────────────────────────────────────────────────┘
```

---

## Dependencies Added

```toml
[target.'cfg(not(target_arch = "wasm32"))'.dependencies]
wasmtime = { version = "27", optional = true }
wasmtime-wasi = { version = "27", optional = true }

[features]
wasi = ["dep:wasmtime", "dep:wasmtime-wasi"]
```

---

## Technical Notes

### wasmtime-wasi v27 API

- Uses `WasiCtxBuilder` for configuration
- `preview1::WasiP1Ctx` for preview1 (traditional WASI) compatibility
- `preview1::add_to_linker_sync()` to add WASI functions to linker
- `preopened_dir()` for mounting host directories

### Exit Code Handling

```rust
match start.call(&mut store, ()) {
    Ok(()) => 0,
    Err(e) => {
        if let Some(exit) = e.downcast_ref::<wasmtime_wasi::I32Exit>() {
            exit.0  // Normal exit with code
        } else {
            1  // Error
        }
    }
}
```

### Temp Directory Cleanup

Temp directories are cleaned up after execution:
```rust
for (_, host_path) in &temp_mounts {
    let _ = std::fs::remove_dir_all(host_path);
}
```

---

## Limitations / Future Work

1. **Stdio capture**: Currently inherits from host. TODO: capture to RunResult
2. **Streaming**: Files are fully synced before/after. TODO: lazy/streaming access
3. **Efficiency**: Full copy is expensive for large filesystems. TODO: virtual filesystem bridge
4. **Preview2**: Uses WASI preview1. TODO: support component model (preview2)

---

## Test Results

```
running 9 tests
test namespace::tests::test_namespace_resolution ... ok
test services::mem_fs::tests::test_create_and_read_file ... ok
test services::mem_fs::tests::test_mkdir_and_nested_files ... ok
test services::mem_fs::tests::test_readdir_root ... ok
test services::mem_fs::tests::test_remove ... ok
test services::mem_fs::tests::test_rename ... ok
test services::mem_fs::tests::test_stat ... ok
test services::mem_fs::tests::test_truncate ... ok
test wasi::runtime::tests::test_runtime_creation ... ok

test result: ok. 9 passed; 0 failed
```
