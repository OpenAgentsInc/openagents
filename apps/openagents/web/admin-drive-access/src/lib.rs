//! Google Drive access in its own lazy GPUI module (#12335).
#[cfg(target_family = "wasm")]
#[path = "../../admin_page.rs"]
mod page;
#[cfg(target_family = "wasm")]
use wasm_bindgen::prelude::*;
#[cfg(target_family = "wasm")]
const NAME: &str = "admin-drive-access";
#[cfg(target_family = "wasm")]
const TITLE: &str = "Google Drive access";
#[cfg(target_family = "wasm")]
const API: &str = "/v1/admin/access/drive-connections";
#[cfg(target_family = "wasm")]
const DRIVE: bool = true;
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
