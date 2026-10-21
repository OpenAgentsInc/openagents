//! Route handlers as WebAssembly components (wasm32-wasip2), compiled once
//! per content hash and instantiated fresh for each request. A new manifest
//! that names a new component loads it without restarting the host.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use wasmtime::component::{Component, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

wasmtime::component::bindgen!({ world: "route", path: "wit" });

pub use exports::openagents::livehost::handler::{Request, Response};

struct State {
    ctx: WasiCtx,
    table: ResourceTable,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

/// A compiled handler, ready to instantiate.
pub struct Handler {
    pre: RoutePre<State>,
    engine: Engine,
}

/// Compiles components and keeps them by content hash, shared by every
/// tenant and manifest.
pub struct Runtime {
    engine: Engine,
    linker: Linker<State>,
    compiled: Mutex<HashMap<String, Arc<Handler>>>,
}

impl Runtime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        // A handler that runs too long is stopped by fuel.
        config.consume_fuel(true);
        let engine = wasm(Engine::new(&config))?;
        let mut linker = Linker::new(&engine);
        wasm(wasmtime_wasi::p2::add_to_linker_sync(&mut linker))?;
        Ok(Runtime {
            engine,
            linker,
            compiled: Mutex::new(HashMap::new()),
        })
    }

    /// The compiled handler for `hash`, compiling `bytes` the first time.
    pub fn load(&self, hash: &str, bytes: &[u8]) -> Result<Arc<Handler>> {
        if let Some(handler) = self.compiled.lock().expect("handler cache").get(hash) {
            return Ok(handler.clone());
        }
        let component =
            wasm(Component::new(&self.engine, bytes)).context("compiling the handler")?;
        let pre = wasm(RoutePre::new(wasm(
            self.linker.instantiate_pre(&component),
        )?))?;
        let handler = Arc::new(Handler {
            pre,
            engine: self.engine.clone(),
        });
        self.compiled
            .lock()
            .expect("handler cache")
            .insert(hash.to_owned(), handler.clone());
        Ok(handler)
    }
}

/// The fuel one request may spend: far more than a render needs, and a
/// bound on a handler stuck in a loop.
const FUEL: u64 = 500_000_000;

impl Handler {
    /// Runs one request in a fresh instance with no files, network, or
    /// environment.
    pub fn handle(&self, request: &Request) -> Result<Response> {
        let mut store = Store::new(
            &self.engine,
            State {
                ctx: WasiCtx::builder().build(),
                table: ResourceTable::new(),
            },
        );
        wasm(store.set_fuel(FUEL))?;
        let route = wasm(self.pre.instantiate(&mut store))?;
        wasm(
            route
                .openagents_livehost_handler()
                .call_handle(&mut store, request),
        )
    }
}

/// Wasmtime has an error type of its own.
fn wasm<T>(result: wasmtime::Result<T>) -> Result<T> {
    result.map_err(|error| anyhow::anyhow!("{error:?}"))
}
