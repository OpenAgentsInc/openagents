//! Analytics in its own lazy GPUI module (#12335).
#[cfg(target_family = "wasm")]
#[path = "../../admin_page.rs"]
mod page;
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;
#[cfg(target_family = "wasm")]
const NAME: &str = "admin-analytics";
#[cfg(target_family = "wasm")]
const TITLE: &str = "Analytics";
#[cfg(target_family = "wasm")]
const API: &str = "/v1/admin/analytics";
#[cfg(target_family = "wasm")]
const DRIVE: bool = false;
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn mount(fonts: js_sys::Array, path: String) {
    console_error_panic_hook::set_once();
    openagents_web_page_host::mount::<page::Admin>(fonts, path);
}
#[cfg(target_family = "wasm")]
#[wasm_bindgen]
pub fn show(path: String) {
    openagents_web_page_host::show(path);
}
