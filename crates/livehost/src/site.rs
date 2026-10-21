//! Handler components for a site that embeds them (the `site-route`
//! world): the same request and response as [`crate::handler`], plus one
//! narrow import, `data.read(name)`, through which the embedding site
//! shares what a route renders, such as a live snapshot. Without the
//! `host` feature, so a site links wasmtime but not livehost's server.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

wasmtime::component::bindgen!({ world: "site-route", path: "wit" });

pub use exports::openagents::livehost::handler::{Request, Response};

/// What a component may read through `data.read`.
pub type DataSource = Arc<dyn Fn(&str) -> Option<String> + Send + Sync>;

struct State {
    ctx: WasiCtx,
    table: ResourceTable,
    data: DataSource,
}

impl WasiView for State {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.ctx,
            table: &mut self.table,
        }
    }
}

impl openagents::livehost::data::Host for State {
    fn read(&mut self, name: String) -> Option<String> {
        (self.data)(&name)
    }
}

/// A compiled site handler, ready to instantiate.
pub struct SiteHandler {
    pre: SiteRoutePre<State>,
    engine: Engine,
}

/// Compiles site handlers and keeps them by content hash.
pub struct SiteRuntime {
    engine: Engine,
    linker: Linker<State>,
    compiled: Mutex<HashMap<String, Arc<SiteHandler>>>,
}

const FUEL: u64 = 500_000_000;

fn wasm<T>(result: wasmtime::Result<T>) -> Result<T> {
    result.map_err(|error| anyhow::anyhow!("{error:?}"))
}

impl SiteRuntime {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();
        config.consume_fuel(true);
        let engine = wasm(Engine::new(&config))?;
        let mut linker = Linker::new(&engine);
        wasm(wasmtime_wasi::p2::add_to_linker_sync(&mut linker))?;
        wasm(SiteRoute::add_to_linker::<State, HasSelf<State>>(
            &mut linker,
            |state| state,
        ))?;
        Ok(SiteRuntime {
            engine,
            linker,
            compiled: Mutex::new(HashMap::new()),
        })
    }

    /// The compiled handler for `hash`, compiling `bytes` the first time.
    pub fn load(&self, hash: &str, bytes: &[u8]) -> Result<Arc<SiteHandler>> {
        if let Some(handler) = self.compiled.lock().expect("handler cache").get(hash) {
            return Ok(handler.clone());
        }
        let component =
            wasm(Component::new(&self.engine, bytes)).context("compiling the handler")?;
        let pre = wasm(SiteRoutePre::new(wasm(
            self.linker.instantiate_pre(&component),
        )?))?;
        let handler = Arc::new(SiteHandler {
            pre,
            engine: self.engine.clone(),
        });
        let mut compiled = self.compiled.lock().expect("handler cache");
        // Keep only a few versions: a site swaps handlers, it doesn't
        // collect them.
        if compiled.len() >= 8 {
            compiled.clear();
        }
        compiled.insert(hash.to_owned(), handler.clone());
        Ok(handler)
    }
}

impl SiteHandler {
    /// Runs one request in a fresh instance with no files, network, or
    /// environment; `data` answers its reads.
    pub fn handle(&self, request: &Request, data: DataSource) -> Result<Response> {
        let mut store = Store::new(
            &self.engine,
            State {
                ctx: WasiCtx::builder().build(),
                table: ResourceTable::new(),
                data,
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
