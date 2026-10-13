//! Compiled guest modules, kept (#11513).
//!
//! Compiling a guest is the slow part of a firing (0.3–0.7 s in a debug
//! build), and a plugin's hooks fire the same module again and again. The
//! process keeps one engine, and each module it compiled, by the digest of
//! the module's bytes, so a second firing of the same module reuses the
//! compiled code. A changed module has a different digest and is compiled
//! anew. Each firing still gets a fresh store and instance: nothing a guest
//! did survives into the next firing.
//!
//! Compiled code is also kept on disk by Wasmtime's cache, keyed by the
//! module bytes, the engine configuration, and the Wasmtime version, so a
//! new process skips compiling a module an earlier one compiled. The
//! directory is `OPENAGENTS_PLUGIN_MODULE_CACHE` when set (`off` turns the
//! disk cache off), and otherwise the user's cache directory. Without a
//! usable directory, modules are kept in memory only.
//!
//! The compiled code does not change what a guest computes or the fuel it
//! consumes, so receipts and replays are the same with or without a kept
//! module.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};

use wasmtime::{Cache, CacheConfig, Config, Engine, Module};

use crate::engine::{HostError, digest};

/// The most modules kept in memory; past it the memory cache starts over.
const KEPT: usize = 64;

/// The engine every guest in this process runs on, and the modules it
/// compiled.
pub(crate) struct Modules {
    engine: Engine,
    disk: Option<Cache>,
    kept: Mutex<HashMap<String, Module>>,
    reused: AtomicU64,
    loaded: AtomicU64,
}

/// How often modules were reused and loaded in this process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ModuleStats {
    /// Firings that reused a module this process had already loaded.
    pub reused: u64,
    /// Modules loaded: compiled, or read from the disk cache.
    pub loaded: u64,
    /// Of those loaded, the ones read from the disk cache.
    pub disk_hits: u64,
    /// Of those loaded, the ones compiled (and written to the disk cache,
    /// when there is one).
    pub disk_misses: u64,
}

/// This process's module counts, to measure the cache.
#[must_use]
pub fn module_stats() -> ModuleStats {
    shared().map_or_else(|_| ModuleStats::default(), Modules::stats)
}

static SHARED: OnceLock<Result<Modules, String>> = OnceLock::new();

/// The process's engine and its kept modules.
pub(crate) fn shared() -> Result<&'static Modules, HostError> {
    SHARED
        .get_or_init(|| Modules::new(disk_directory()))
        .as_ref()
        .map_err(|error| HostError::Failed(error.clone()))
}

impl Modules {
    /// An engine with `replay::ENGINE`'s configuration, caching compiled
    /// code under `disk` when given.
    pub(crate) fn new(disk: Option<PathBuf>) -> Result<Self, String> {
        let mut config = Config::new();
        // `replay::ENGINE` names this configuration. Change both together.
        config.consume_fuel(true);
        config.epoch_interruption(true);
        // On macOS, Wasmtime's default trap handler is a Mach-port thread
        // that receives with `MACH_RCV_INTERRUPT` and aborts the process
        // when the receive is interrupted. The host installs a SIGCHLD
        // handler to reap child processes, so a child exiting while the
        // kernel delivers that signal to the trap thread killed the whole
        // host with SIGABRT. Unix signal handlers trap guests the same way
        // and survive other signals. Trap delivery does not change guest
        // results, so `replay::ENGINE` keeps its name. Every engine in a
        // process must agree on this.
        config.macos_use_mach_ports(false);
        // The disk cache stores what the compiler produced; it does not
        // change the code, so `replay::ENGINE` keeps its name.
        let disk = disk.and_then(|directory| {
            let mut cache = CacheConfig::new();
            cache.with_directory(directory);
            Cache::new(cache).ok()
        });
        config.cache(disk.clone());
        let engine = Engine::new(&config).map_err(|error| error.to_string())?;
        Ok(Self {
            engine,
            disk,
            kept: Mutex::default(),
            reused: AtomicU64::new(0),
            loaded: AtomicU64::new(0),
        })
    }

    pub(crate) fn engine(&self) -> &Engine {
        &self.engine
    }

    /// The compiled module for `wasm`: the kept one when this process
    /// already loaded these bytes, and otherwise compiled (or read from
    /// the disk cache) and kept.
    pub(crate) fn module(&self, wasm: &[u8]) -> Result<Module, HostError> {
        let key = digest(wasm);
        if let Some(module) = self.lock().get(&key) {
            self.reused.fetch_add(1, Ordering::Relaxed);
            return Ok(module.clone());
        }
        // Compiled outside the lock, so firings of other modules don't wait.
        let module = Module::new(&self.engine, wasm)
            .map_err(|error| HostError::Malformed(error.to_string()))?;
        self.loaded.fetch_add(1, Ordering::Relaxed);
        let mut kept = self.lock();
        if kept.len() >= KEPT {
            kept.clear();
        }
        kept.insert(key, module.clone());
        Ok(module)
    }

    pub(crate) fn stats(&self) -> ModuleStats {
        ModuleStats {
            reused: self.reused.load(Ordering::Relaxed),
            loaded: self.loaded.load(Ordering::Relaxed),
            disk_hits: self
                .disk
                .as_ref()
                .map_or(0, |disk| disk.cache_hits() as u64),
            disk_misses: self
                .disk
                .as_ref()
                .map_or(0, |disk| disk.cache_misses() as u64),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Module>> {
        self.kept.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Where compiled modules are kept on disk, or `None` for nowhere.
fn disk_directory() -> Option<PathBuf> {
    if let Some(directory) = std::env::var_os("OPENAGENTS_PLUGIN_MODULE_CACHE") {
        return (directory != "off" && !directory.is_empty()).then(|| directory.into());
    }
    let home = user_home::home_os().map(PathBuf::from);
    let base = if cfg!(target_os = "macos") {
        home?.join("Library/Caches")
    } else if let Some(cache) = std::env::var_os("XDG_CACHE_HOME") {
        PathBuf::from(cache)
    } else {
        home?.join(".cache")
    };
    base.is_absolute()
        .then(|| base.join("openagents").join("plugin-modules"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const GUEST: &str = r#"(module (memory (export "memory") 1)
        (func (export "oa_alloc") (param i32) (result i32) i32.const 0))"#;

    #[test]
    fn a_kept_module_is_reused_and_changed_bytes_compile_anew() {
        let modules = Modules::new(None).unwrap();
        let wasm = wat::parse_str(GUEST).unwrap();
        modules.module(&wasm).unwrap();
        modules.module(&wasm).unwrap();
        assert_eq!(modules.stats().loaded, 1);
        assert_eq!(modules.stats().reused, 1);
        let changed = wat::parse_str(GUEST.replace("i32.const 0", "i32.const 8")).unwrap();
        modules.module(&changed).unwrap();
        assert_eq!(modules.stats().loaded, 2);
        assert!(modules.module(b"not wasm").is_err());
    }

    #[test]
    fn a_new_process_reads_the_module_from_the_disk_cache() {
        let dir = tempfile::tempdir().unwrap();
        let wasm = wat::parse_str(GUEST).unwrap();
        let first = Modules::new(Some(dir.path().into())).unwrap();
        first.module(&wasm).unwrap();
        assert_eq!(first.stats().disk_misses, 1);
        // A second engine stands in for the next process.
        let second = Modules::new(Some(dir.path().into())).unwrap();
        second.module(&wasm).unwrap();
        assert_eq!(second.stats().disk_hits, 1);
        assert_eq!(second.stats().disk_misses, 0);
    }
}
