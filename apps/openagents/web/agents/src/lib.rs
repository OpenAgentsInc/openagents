//! Agents in the GPUI web app (#12330). The page's start script loads this
//! module at `?page=agents` and calls [`mount`]; it reads `/v1/agents` and
//! `/v1/land` and acts through `/v1/agents/actions` with the sign-in
//! cookie, as the site's `/settings/agents` page does.

#[cfg(target_family = "wasm")]
mod page;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

/// Starts the page at `path`, with the page's fonts.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, path: String) {
    console_error_panic_hook::set_once();
    openagents_web_page_host::mount::<page::Agents>(fonts, path);
}

/// Shows `path` again.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(path: String) {
    openagents_web_page_host::show(path);
}
