     OANIX Short-Term Roadmap: Initial Primitives

     Approach: Parallel - Minimal MemFs, then browser demo, then expand primitives
     Demo Goal: Namespace Explorer - Visual browser UI to browse/edit filesystems
     WASI Scope: Basic wasmtime integration to run simple WASI binaries

     ---
     Current State

     OANIX crate (crates/oanix/):
     - ✅ Namespace + Mount with longest-prefix matching
     - ✅ FileService trait definition
     - ✅ FileHandle trait, OpenFlags, DirEntry, Metadata
     - ✅ Error types (FsError, OanixError)
     - ❌ No concrete FileService implementations
     - ❌ No WASI runtime integration
     - ❌ No browser/WASM support yet

     Wanix inspiration (from ~/code/wanix):
     - fskit building blocks: MapFS, Node, FuncFile
     - In-memory VFS with proper path traversal
     - JavaScript API layer with handle-based async operations

     ---
     Sprint 1: Minimal MemFs + Browser Shell

     Goal: Get something running in browser ASAP

     1.1 Minimal MemFs

     File: src/services/mem_fs.rs

     pub struct MemFs {
         root: Arc<RwLock<MemNode>>,
     }

     enum MemNode {
         File { content: Vec<u8>, modified: u64 },
         Dir { children: HashMap<String, MemNode> },
     }

     Implement only:
     - open() - read/write/create
     - readdir() - list directory
     - stat() - get metadata
     - mkdir() - create directory

     Skip for now: remove(), rename()

     1.2 Browser WASM Build

     Files:
     - src/web/mod.rs
     - src/web/api.rs

     #[wasm_bindgen]
     pub struct OanixWeb {
         namespace: Namespace,
     }

     #[wasm_bindgen]
     impl OanixWeb {
         #[wasm_bindgen(constructor)]
         pub fn new() -> Self;
         pub fn read_text(&self, path: &str) -> Result<String, JsValue>;
         pub fn write_text(&mut self, path: &str, content: &str) -> Result<(), JsValue>;
         pub fn list_dir(&self, path: &str) -> Result<JsValue, JsValue>;
         pub fn mounts(&self) -> JsValue;  // List all mount points
     }

     1.3 Namespace Explorer Demo

     Files: examples/browser/

     Simple HTML page:
     - Left panel: tree view of mounts and directories
     - Right panel: file content viewer/editor
     - Click directory to expand, click file to view/edit
     - Save button writes back to MemFs

     Build: cd crates/oanix && wasm-pack build --target web --features browser
     Serve: python -m http.server in examples/browser/

     ---
     Sprint 2: Basic WASI Runtime

     Goal: Execute simple .wasm binaries with namespace access

     2.1 WASI Bridge

     Files:
     - src/wasi/mod.rs
     - src/wasi/runtime.rs

     pub struct WasiRuntime {
         engine: wasmtime::Engine,
     }

     pub struct RunConfig {
         pub args: Vec<String>,
         pub env: Vec<(String, String)>,
     }

     pub struct RunResult {
         pub exit_code: i32,
         pub stdout: Vec<u8>,
         pub stderr: Vec<u8>,
     }

     impl WasiRuntime {
         pub fn run(&self, ns: &Namespace, wasm: &[u8], cfg: RunConfig) -> Result<RunResult, OanixError>;
     }

     Key: Wire WASI's preopened directories to OANIX namespace mounts.

     2.2 CLI Test

     # Build a simple WASI binary that reads/writes files
     cargo run --example run_wasi -- hello.wasm

     ---
     Sprint 3: Expand Primitives

     3.1 FuncFs - Computed Files

     File: src/services/func_fs.rs

     pub struct FuncFs {
         files: HashMap<String, Arc<dyn Fn() -> Vec<u8> + Send + Sync>>,
     }

     Use cases:
     - /status - returns current state as JSON
     - /time - returns current timestamp
     - Control files where writes trigger actions

     3.2 MapFs - Static Content

     File: src/services/map_fs.rs

     pub struct MapFs {
         tree: MemNode,  // Reuse MemNode but immutable
     }

     impl MapFs {
         pub fn builder() -> MapFsBuilder;
     }

     Use case: Embed task specs, bundled assets

     3.3 Complete MemFs

     Add remove() and rename() operations.

     ---
     Sprint 4: Standard Services

     4.1 TaskFs

     File: src/services/task_fs.rs

     /task/
     ├── spec.json    - Task definition (MapFs)
     ├── meta.json    - Metadata (MapFs)
     └── status       - Live status (FuncFs)

     4.2 LogsFs

     File: src/services/logs_fs.rs

     /logs/
     ├── stdout.log   - Append-only (MemFs variant)
     ├── stderr.log   - Append-only
     └── events/      - Structured events (JSONL)

     ---
     Files Summary

     Create

     crates/oanix/src/
     ├── services/
     │   ├── mod.rs
     │   ├── mem_fs.rs      # Sprint 1
     │   ├── func_fs.rs     # Sprint 3
     │   ├── map_fs.rs      # Sprint 3
     │   ├── task_fs.rs     # Sprint 4
     │   └── logs_fs.rs     # Sprint 4
     ├── web/
     │   ├── mod.rs         # Sprint 1
     │   └── api.rs         # Sprint 1
     └── wasi/
         ├── mod.rs         # Sprint 2
         └── runtime.rs     # Sprint 2

     crates/oanix/examples/
     ├── browser/
     │   ├── index.html     # Sprint 1
     │   └── main.js        # Sprint 1
     └── run_wasi.rs        # Sprint 2

     Modify

     - crates/oanix/src/lib.rs - add module exports
     - crates/oanix/Cargo.toml - add dependencies:
       - wasm-bindgen, web-sys, js-sys (browser feature)
       - wasmtime, wasmtime-wasi (wasi feature)

     ---
     Dependencies to Add

     [dependencies]
     # ... existing ...

     [target.'cfg(target_arch = "wasm32")'.dependencies]
     wasm-bindgen = "0.2"
     js-sys = "0.3"
     web-sys = { version = "0.3", features = ["console"] }

     [target.'cfg(not(target_arch = "wasm32"))'.dependencies]
     wasmtime = { version = "27", optional = true }
     wasmtime-wasi = { version = "27", optional = true }

     [features]
     default = []
     browser = []
     wasi = ["wasmtime", "wasmtime-wasi"]

     ---
     Milestone: Namespace Explorer Demo

     When Sprint 1 is complete, you'll have:

     1. Browser app at http://localhost:8000
     2. Pre-mounted namespace:
       - /workspace - MemFs (editable)
       - /task - Sample task spec
     3. Interactive UI:
       - Tree view showing all mounts
       - Click to browse directories
       - View/edit files in MemFs
       - See namespace resolution in action

     This demonstrates the Plan 9 "everything is a file" philosophy in an accessible, visual way.
