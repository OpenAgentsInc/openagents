//! WASI runtime implementation using wasmtime
//!
//! Executes WebAssembly modules with WASI support, bridging OANIX namespaces
//! to WASI's preopened directories.

use std::path::PathBuf;

use wasmtime::*;
use wasmtime_wasi::pipe::MemoryOutputPipe;
use wasmtime_wasi::preview1::{self, WasiP1Ctx};
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

use crate::error::OanixError;
use crate::service::{FileService, OpenFlags};
use crate::Namespace;

/// Configuration for running a WASI module
#[derive(Debug, Clone, Default)]
pub struct RunConfig {
    /// Command-line arguments (argv[0] is typically the program name)
    pub args: Vec<String>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Working directory within the namespace
    pub working_dir: Option<String>,
}

/// Result of running a WASI module
#[derive(Debug, Clone)]
pub struct RunResult {
    /// Exit code (0 = success)
    pub exit_code: i32,
    /// Captured standard output
    pub stdout: Vec<u8>,
    /// Captured standard error
    pub stderr: Vec<u8>,
}

/// Default in-memory buffer size for captured stdio (1 MiB per stream)
const STDIO_BUFFER_CAPACITY: usize = 1024 * 1024;

/// WASI runtime powered by wasmtime
///
/// Executes WebAssembly modules with WASI support, capturing stdout/stderr
/// and providing access to OANIX namespace mounts.
///
/// # Example
///
/// ```rust,ignore
/// use oanix::wasi::{WasiRuntime, RunConfig};
/// use oanix::Namespace;
///
/// let runtime = WasiRuntime::new()?;
/// let namespace = Namespace::builder().build();
///
/// let result = runtime.run(
///     &namespace,
///     &wasm_bytes,
///     RunConfig {
///         args: vec!["program".into(), "arg1".into()],
///         ..Default::default()
///     },
/// )?;
///
/// println!("Exit code: {}", result.exit_code);
/// println!("Output: {}", String::from_utf8_lossy(&result.stdout));
/// ```
pub struct WasiRuntime {
    engine: Engine,
}

impl WasiRuntime {
    /// Create a new WASI runtime
    pub fn new() -> Result<Self, OanixError> {
        let engine = Engine::default();
        Ok(Self { engine })
    }

    /// Run a WASI module within the given namespace
    ///
    /// # Arguments
    ///
    /// * `namespace` - The OANIX namespace providing filesystem access
    /// * `wasm_bytes` - The WebAssembly module bytecode
    /// * `config` - Configuration for the execution (args, env, stdin)
    ///
    /// # Returns
    ///
    /// `RunResult` containing exit code and captured stdout/stderr
    pub fn run(
        &self,
        namespace: &Namespace,
        wasm_bytes: &[u8],
        config: RunConfig,
    ) -> Result<RunResult, OanixError> {
        // Build WASI context
        let mut wasi_builder = WasiCtxBuilder::new();

        // Capture stdout/stderr into in-memory pipes for RunResult
        let stdout_pipe = MemoryOutputPipe::new(STDIO_BUFFER_CAPACITY);
        let stderr_pipe = MemoryOutputPipe::new(STDIO_BUFFER_CAPACITY);

        // Set up arguments
        let args: Vec<&str> = config.args.iter().map(|s| s.as_str()).collect();
        wasi_builder.args(&args);

        // Set up environment
        for (key, value) in &config.env {
            wasi_builder.env(key, value);
        }

        // Inherit stdin from host while capturing stdout/stderr
        wasi_builder.inherit_stdin();
        wasi_builder.stdout(stdout_pipe.clone());
        wasi_builder.stderr(stderr_pipe.clone());

        // Mount namespace paths as preopened directories
        // For now, we'll sync MemFs contents to temp directories
        let temp_mounts = self.prepare_mounts(namespace)?;
        for (guest_path, host_path) in &temp_mounts {
            wasi_builder.preopened_dir(
                host_path,
                guest_path,
                DirPerms::all(),
                FilePerms::all(),
            ).map_err(|e| OanixError::Wasi(format!("Failed to preopen {}: {}", guest_path, e)))?;
        }

        // Build the WASI context
        let wasi_ctx = wasi_builder.build_p1();

        // Create store with WASI context
        let mut store = Store::new(&self.engine, wasi_ctx);

        // Compile the module
        let module = Module::from_binary(&self.engine, wasm_bytes)
            .map_err(|e| OanixError::Wasi(format!("Failed to compile module: {}", e)))?;

        // Create linker and add WASI functions
        let mut linker = Linker::new(&self.engine);
        preview1::add_to_linker_sync(&mut linker, |ctx: &mut WasiP1Ctx| ctx)
            .map_err(|e| OanixError::Wasi(format!("Failed to add WASI to linker: {}", e)))?;

        // Instantiate the module
        let instance = linker
            .instantiate(&mut store, &module)
            .map_err(|e| OanixError::Wasi(format!("Failed to instantiate module: {}", e)))?;

        // Get the _start function
        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| OanixError::Wasi(format!("Failed to get _start: {}", e)))?;

        // Run the module
        let exit_code = match start.call(&mut store, ()) {
            Ok(()) => 0,
            Err(e) => {
                // Check if it's a normal exit
                if let Some(exit) = e.downcast_ref::<wasmtime_wasi::I32Exit>() {
                    exit.0
                } else {
                    // Some other error
                    tracing::error!("WASI execution error: {}", e);
                    1
                }
            }
        };

        // Sync any modified files back to namespace
        self.sync_mounts_back(namespace, &temp_mounts)?;

        // Clean up temp directories
        for (_, host_path) in &temp_mounts {
            let _ = std::fs::remove_dir_all(host_path);
        }

        Ok(RunResult {
            exit_code,
            stdout: stdout_pipe.contents().to_vec(),
            stderr: stderr_pipe.contents().to_vec(),
        })
    }

    /// Prepare namespace mounts as host directories for WASI
    ///
    /// For MemFs and other virtual filesystems, we sync to temp directories.
    /// For real filesystem mounts (future), we could use them directly.
    fn prepare_mounts(&self, namespace: &Namespace) -> Result<Vec<(String, PathBuf)>, OanixError> {
        let mut mounts = Vec::new();

        for mount in namespace.mounts() {
            let guest_path = mount.path.clone();

            // Create a temp directory for this mount
            let temp_dir = std::env::temp_dir().join(format!("oanix-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir_all(&temp_dir)
                .map_err(|e| OanixError::Wasi(format!("Failed to create temp dir: {}", e)))?;

            // Sync files from the FileService to the temp directory
            self.sync_to_host(&*mount.service, "/", &temp_dir)?;

            mounts.push((guest_path, temp_dir));
        }

        Ok(mounts)
    }

    /// Recursively sync files from a FileService to a host directory
    fn sync_to_host(
        &self,
        service: &dyn FileService,
        service_path: &str,
        host_path: &std::path::Path,
    ) -> Result<(), OanixError> {
        let entries = service
            .readdir(service_path)
            .map_err(|e| OanixError::Wasi(format!("Failed to read dir {}: {}", service_path, e)))?;

        for entry in entries {
            let entry_service_path = if service_path == "/" {
                format!("/{}", entry.name)
            } else {
                format!("{}/{}", service_path, entry.name)
            };
            let entry_host_path = host_path.join(&entry.name);

            if entry.is_dir {
                std::fs::create_dir_all(&entry_host_path)
                    .map_err(|e| OanixError::Wasi(format!("Failed to create dir: {}", e)))?;
                self.sync_to_host(service, &entry_service_path, &entry_host_path)?;
            } else {
                // Read file content and write to host
                let mut handle = service
                    .open(&entry_service_path, OpenFlags::read_only())
                    .map_err(|e| OanixError::Wasi(format!("Failed to open {}: {}", entry_service_path, e)))?;

                let mut content = Vec::new();
                let mut buf = [0u8; 4096];
                loop {
                    let n = handle
                        .read(&mut buf)
                        .map_err(|e| OanixError::Wasi(format!("Failed to read: {}", e)))?;
                    if n == 0 {
                        break;
                    }
                    content.extend_from_slice(&buf[..n]);
                }

                std::fs::write(&entry_host_path, &content)
                    .map_err(|e| OanixError::Wasi(format!("Failed to write to host: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Sync modified files from host directories back to namespace
    fn sync_mounts_back(
        &self,
        namespace: &Namespace,
        mounts: &[(String, PathBuf)],
    ) -> Result<(), OanixError> {
        for (guest_path, host_path) in mounts {
            if let Some((service, _)) = namespace.resolve(guest_path) {
                self.sync_from_host(service, "/", host_path)?;
            }
        }
        Ok(())
    }

    /// Recursively sync files from host directory back to FileService
    fn sync_from_host(
        &self,
        service: &dyn FileService,
        service_path: &str,
        host_path: &std::path::Path,
    ) -> Result<(), OanixError> {
        let entries = std::fs::read_dir(host_path)
            .map_err(|e| OanixError::Wasi(format!("Failed to read host dir: {}", e)))?;

        for entry in entries {
            let entry = entry.map_err(|e| OanixError::Wasi(format!("Dir entry error: {}", e)))?;
            let file_name = entry.file_name().to_string_lossy().to_string();
            let entry_service_path = if service_path == "/" {
                format!("/{}", file_name)
            } else {
                format!("{}/{}", service_path, file_name)
            };
            let entry_host_path = entry.path();

            let metadata = entry.metadata()
                .map_err(|e| OanixError::Wasi(format!("Metadata error: {}", e)))?;

            if metadata.is_dir() {
                // Ensure directory exists in service
                let _ = service.mkdir(&entry_service_path);
                self.sync_from_host(service, &entry_service_path, &entry_host_path)?;
            } else {
                // Read from host and write to service
                let content = std::fs::read(&entry_host_path)
                    .map_err(|e| OanixError::Wasi(format!("Failed to read host file: {}", e)))?;

                let mut handle = service
                    .open(
                        &entry_service_path,
                        OpenFlags {
                            write: true,
                            create: true,
                            truncate: true,
                            ..Default::default()
                        },
                    )
                    .map_err(|e| OanixError::Wasi(format!("Failed to open for write: {}", e)))?;

                handle
                    .write(&content)
                    .map_err(|e| OanixError::Wasi(format!("Failed to write: {}", e)))?;
                handle
                    .flush()
                    .map_err(|e| OanixError::Wasi(format!("Failed to flush: {}", e)))?;
            }
        }

        Ok(())
    }
}

impl Default for WasiRuntime {
    fn default() -> Self {
        Self::new().expect("Failed to create WasiRuntime")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Namespace;

    #[test]
    fn test_runtime_creation() {
        let _runtime = WasiRuntime::new().unwrap();
        assert!(true); // Just verify it doesn't panic
    }

    #[test]
    fn captures_stdout_and_stderr() {
        let runtime = WasiRuntime::new().unwrap();
        let namespace = Namespace::builder().build();
        let wasm = wat::parse_str(
            r#"
            (module
              (type $fd_write_ty (func (param i32 i32 i32 i32) (result i32)))
              (import "wasi_snapshot_preview1" "fd_write" (func $fd_write (type $fd_write_ty)))
              (memory 1)
              (export "memory" (memory 0))
              (data (i32.const 8) "stdout\n")
              (data (i32.const 32) "stderr\n")
              (func $_start (export "_start")
                ;; iovec for stdout at offset 0
                (i32.store (i32.const 0) (i32.const 8))
                (i32.store (i32.const 4) (i32.const 7))
                ;; iovec for stderr at offset 16
                (i32.store (i32.const 16) (i32.const 32))
                (i32.store (i32.const 20) (i32.const 7))
                ;; write stdout
                (call $fd_write (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 64))
                drop
                ;; write stderr
                (call $fd_write (i32.const 2) (i32.const 16) (i32.const 1) (i32.const 68))
                drop))
            "#,
        )
        .unwrap();

        let result = runtime
            .run(&namespace, &wasm, RunConfig::default())
            .expect("WASI execution failed");

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, b"stdout\n");
        assert_eq!(result.stderr, b"stderr\n");
    }
}
