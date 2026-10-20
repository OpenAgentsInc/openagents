#![cfg(target_family = "wasm")]

mod dispatcher;
mod display;
mod events;
mod http_client;
mod image;
mod keyboard;
mod logging;
mod platform;
mod ready;
#[cfg(feature = "browser-text")]
mod text;
#[cfg(feature = "webgl")]
mod webgl;
mod window;
#[cfg(all(feature = "webgl", feature = "webgpu"))]
compile_error!("choose either webgpu or webgl, not both");

pub use dispatcher::WebDispatcher;
pub use display::WebDisplay;
pub use http_client::FetchHttpClient;
pub use image::{BrowserImage, img, mask_image, paint_mask};
pub use keyboard::WebKeyboardLayout;
pub use logging::init_logging;
pub use platform::{WebFonts, WebPlatform};
pub use ready::announce_ready;
#[cfg(feature = "first-frame")]
pub use ready::recorded_frame;
#[cfg(feature = "browser-text")]
pub use text::BrowserTextSystem;
pub use window::WebWindow;
