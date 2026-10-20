//! Traces in the GPUI web app (#12329). The page's start script loads this
//! module at `?page=traces` (or `traces/ID`, `traces/ID/AGENT`) and calls
//! [`mount`]; it reads and changes traces through `/v1/traces` with the
//! sign-in cookie, as the site's `/settings/traces` pages do.

#[cfg(target_family = "wasm")]
mod page;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Starts the page at `path`, with the page's fonts.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, path: String) {
    console_error_panic_hook::set_once();
    openagents_web_page_host::mount::<page::Traces>(fonts, path);
}

/// Shows `path` again.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(path: String) {
    openagents_web_page_host::show(path);
}
