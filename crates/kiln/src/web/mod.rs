//! The browser side: the page (`app`), the tool worker (`worker`), the
//! relay socket (`relay`), and small helpers (`util`).

mod app;
mod identity;
mod relay;
mod review;
mod util;
mod worker;

use wasm_bindgen::prelude::*;

/// Start the page.
#[wasm_bindgen]
pub fn start() {
    wasm_bindgen_futures::spawn_local(app::run());
}

/// Start Kiln inside another page, with its files under `base` (such as
/// `/admin/kiln/`) and no markup of its own (#12456).
#[wasm_bindgen]
pub fn start_headless(base: String) {
    wasm_bindgen_futures::spawn_local(app::run_headless(base));
}

/// Start a named creator lab with a separate local workspace.
#[wasm_bindgen]
pub fn start_lab(base: String, lab: String) -> Result<(), JsValue> {
    let lab = crate::shader::Lab::parse(&lab).map_err(JsValue::from_str)?;
    wasm_bindgen_futures::spawn_local(app::run_lab(base, lab));
    Ok(())
}

/// Start the patch viewer (`review.html`).
#[wasm_bindgen]
pub fn start_review() {
    wasm_bindgen_futures::spawn_local(review::run());
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
