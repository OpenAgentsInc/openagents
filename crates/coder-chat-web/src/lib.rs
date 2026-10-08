//! Local draft, composer, and scroll interaction for server-rendered chats.
//! HTMX owns requests and streams; the server owns chat and request identity.

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(target_arch = "wasm32")]
pub use browser::start;
