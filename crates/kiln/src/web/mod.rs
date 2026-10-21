//! The browser side: the page (`app`), the tool worker (`worker`), the
//! relay socket (`relay`), and small helpers (`util`).

mod app;
mod relay;
mod util;
mod worker;

use wasm_bindgen::prelude::*;

/// Start the page.
#[wasm_bindgen]
pub fn start() {
    wasm_bindgen_futures::spawn_local(app::run());
}

/// Run one tool in this worker. `message` holds `module` (a compiled
/// `WebAssembly.Module`), `argv`, `stdin`, and `fs` (a workspace snapshot);
/// the answer holds `code`, `stdout`, `stderr`, `fs`, and timings.
///
/// # Errors
///
/// When the message is malformed or the module can't be instantiated.
#[wasm_bindgen]
pub async fn run_task(message: JsValue) -> Result<JsValue, JsValue> {
    worker::run(message).await
}
