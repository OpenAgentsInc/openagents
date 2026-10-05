//! The wall clock for step and query profiling. A browser build has no
//! `std::time::Instant` (`wasm32-unknown-unknown` panics on its first
//! use), so there the profile reads `web_time`'s, which asks the page's
//! `performance.now()`.

#[cfg(not(target_arch = "wasm32"))]
pub use std::time::Instant;
#[cfg(target_arch = "wasm32")]
pub use web_time::Instant;
