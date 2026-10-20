//! Vault in its own lazy GPUI module, over the shared page host.
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;

#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, path: String) {
    console_error_panic_hook::set_once();
    openagents_web_page_host::mount::<openagents_web_connection_pages::Vault>(fonts, path);
}
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(path: String) {
    openagents_web_page_host::show(path);
}
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn hide() {
    openagents_web_page_host::hide();
}
