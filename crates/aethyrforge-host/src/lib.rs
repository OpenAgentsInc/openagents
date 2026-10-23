//! Public integration types. This crate contains no engine implementation.

mod access;
pub use access::*;
mod catalog;
mod host;
mod protocol;
pub use catalog::*;
pub use host::*;
pub use protocol::*;

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub use browser::*;
mod engine;
pub use engine::*;

#[cfg(target_arch = "wasm32")]
mod demo;
#[cfg(target_arch = "wasm32")]
pub use demo::*;

#[cfg(target_arch = "wasm32")]
mod studio_browser;
#[cfg(target_arch = "wasm32")]
pub use studio_browser::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn protocol_version() -> u16 {
    VERSION
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn action_sha256(input: String) -> Result<String, wasm_bindgen::JsValue> {
    let action: Action = serde_json::from_str(&input)
        .map_err(|_| wasm_bindgen::JsValue::from_str("Invalid action."))?;
    action_digest(&action).map_err(|_| wasm_bindgen::JsValue::from_str("Invalid action."))
}

mod universe;
pub use universe::*;
#[cfg(target_arch = "wasm32")]
mod universe_browser;
#[cfg(target_arch = "wasm32")]
pub use universe_browser::*;

mod authoring;
pub use authoring::*;

mod studio_layout;
pub use studio_layout::*;

mod voice;
pub use voice::*;
#[cfg(target_arch = "wasm32")]
mod voice_browser;

mod project;
pub use project::*;

mod project_history;
pub use project_history::*;
mod project_session;
pub use project_session::*;
mod contribution;
pub use contribution::*;

mod material;
pub use material::*;

mod publication;
pub use publication::*;

#[cfg(target_arch = "wasm32")]
mod player_browser;
#[cfg(target_arch = "wasm32")]
pub use player_browser::*;
