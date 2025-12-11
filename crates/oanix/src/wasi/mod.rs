//! WASI runtime for executing WebAssembly modules
//!
//! Provides integration with wasmtime to run WASI binaries within OANIX namespaces.

mod runtime;

pub use runtime::{RunConfig, RunResult, WasiRuntime};
