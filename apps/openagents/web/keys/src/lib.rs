//! Account credentials over the shared page host, loaded separately from chat.

#[cfg(target_family = "wasm")]
mod page;

#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, path: String) {
    console_error_panic_hook::set_once();
    openagents_web_page_host::mount::<page::Keys>(fonts, path);
}

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(path: String) {
    openagents_web_page_host::show(path);
}

/// Forget the new key and cancel reads when the page closes.
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn hide() {
    openagents_web_page_host::show("__closed".into());
}
