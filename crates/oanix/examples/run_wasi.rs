//! Run a WASI binary within an OANIX namespace
//!
//! This example demonstrates running WebAssembly modules with WASI support,
//! using OANIX namespaces for filesystem access.
//!
//! # Usage
//!
//! ```bash
//! cargo run --features wasi --example run_wasi -- <wasm-file> [args...]
//! ```
//!
//! # Example
//!
//! ```bash
//! # Build the test WASI binary first
//! cd examples/hello-wasi && cargo build --target wasm32-wasip1 --release
//!
//! # Run it with OANIX
//! cargo run --features wasi --example run_wasi -- \
//!     examples/hello-wasi/target/wasm32-wasip1/release/hello_wasi.wasm
//! ```

use std::env;
use std::fs;
use std::path::PathBuf;

use oanix::{MemFs, Namespace, RunConfig, WasiRuntime};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Parse command line arguments
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <wasm-file> [args...]", args[0]);
        eprintln!();
        eprintln!("Example:");
        eprintln!("  {} hello.wasm", args[0]);
        eprintln!("  {} hello.wasm arg1 arg2", args[0]);
        std::process::exit(1);
    }

    let wasm_path = PathBuf::from(&args[1]);
    let wasm_args: Vec<String> = args[1..].to_vec(); // Include program name

    // Read the WASM file
    println!("Loading WASM module: {}", wasm_path.display());
    let wasm_bytes = fs::read(&wasm_path)?;
    println!("  Size: {} bytes", wasm_bytes.len());

    // Create an OANIX namespace with some mounted filesystems
    let workspace_fs = MemFs::new();
    let tmp_fs = MemFs::new();

    // Pre-populate workspace with some test files
    {
        use oanix::{FileHandle, FileService, OpenFlags};

        // Create a test file the WASM module can read
        let mut handle = workspace_fs
            .open(
                "/hello.txt",
                OpenFlags {
                    write: true,
                    create: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(b"Hello from OANIX namespace!\n").unwrap();
        handle.flush().unwrap();

        // Create a directory
        workspace_fs.mkdir("/data").unwrap();
        let mut handle = workspace_fs
            .open(
                "/data/test.txt",
                OpenFlags {
                    write: true,
                    create: true,
                    ..Default::default()
                },
            )
            .unwrap();
        handle.write(b"Test data file\n").unwrap();
        handle.flush().unwrap();
    }

    // Build the namespace
    let namespace = Namespace::builder()
        .mount("/workspace", workspace_fs)
        .mount("/tmp", tmp_fs)
        .build();

    println!();
    println!("Namespace mounts:");
    for mount in namespace.mounts() {
        println!("  {}", mount.path);
    }

    // Create WASI runtime
    println!();
    println!("Initializing WASI runtime...");
    let runtime = WasiRuntime::new()?;

    // Configure the run
    let config = RunConfig {
        args: wasm_args,
        env: vec![
            ("OANIX_VERSION".to_string(), "0.1.0".to_string()),
            ("HOME".to_string(), "/workspace".to_string()),
        ],
        working_dir: Some("/workspace".to_string()),
    };

    // Run the WASI module
    println!("Running WASM module...");
    println!("----------------------------------------");

    let result = runtime.run(&namespace, &wasm_bytes, config)?;

    println!("----------------------------------------");
    println!();
    println!("Execution complete:");
    println!("  Exit code: {}", result.exit_code);

    // Check if any files were created/modified in /tmp
    println!();
    println!("Checking namespace state after execution...");

    // Note: Due to the sync-to-temp approach, modifications made by the WASM
    // module should be synced back to our MemFs mounts.

    if result.exit_code == 0 {
        println!("Success!");
    } else {
        println!("WASI module exited with error code: {}", result.exit_code);
    }

    Ok(())
}
