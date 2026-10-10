//! The live sealed-inference demo at openagents.com/att.
//!
//! A scene in the Grid's look drawn with WebGL2 (`scene`, `robot`, `gl`),
//! a chat transcript of the round in the site's own classes (`show`), and
//! the round itself (`flow`).

pub mod copy;
pub mod icons;
pub mod mesh;
pub mod robot;
pub mod scene;
pub mod steps;

#[cfg(all(target_arch = "wasm32", feature = "demo"))]
mod demo;
#[cfg(target_arch = "wasm32")]
pub mod flow;
#[cfg(target_arch = "wasm32")]
mod gl;
#[cfg(target_arch = "wasm32")]
pub mod show;

/// Mounts the page when the module loads.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    if let Some(show) = show::Show::mount() {
        #[cfg(feature = "demo")]
        demo::start(show);
        #[cfg(not(feature = "demo"))]
        flow::start(show);
    }
}
