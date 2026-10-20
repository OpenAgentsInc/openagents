//! A Lab document in the GPUI web app (#12333). The page's start script
//! loads this module at `?page=labs/KIND/DOCUMENT/VERSION/SPACE` and calls
//! [`mount`]; it reads the card through `/v1/labs/documents` with the
//! sign-in cookie and mounts the document's player
//! (`/labs/{kind}/{document}/media`) in a frame over the canvas.

#[cfg(target_family = "wasm")]
mod page;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Starts the page at `path`, with the page's fonts.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, path: String) {
    console_error_panic_hook::set_once();
    openagents_web_page_host::mount::<page::Lab>(fonts, path);
}

/// Shows `path` again.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(path: String) {
    openagents_web_page_host::show(path);
}
