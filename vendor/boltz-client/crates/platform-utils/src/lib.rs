//! Platform utilities for cross-platform abstractions.
//!
//! This crate provides platform-agnostic utilities, including HTTP client abstractions
//! built on reqwest that work on both native and WASM platforms.

mod auth;
pub mod http;
pub mod proxy;

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub use tokio;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use tokio_with_wasm::alias as tokio;

#[cfg(all(target_family = "wasm", target_os = "unknown"))]
pub use web_time as time;

#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
pub use std::time;

pub use auth::{
    ContentType, add_basic_auth_header, add_content_type_header, make_basic_auth_header,
};
pub use http::{
    DefaultHttpClient, HttpClient, HttpError, HttpResponse, MAX_RESPONSE_BYTES, REQUEST_TIMEOUT,
    create_http_client, create_http_client_with_proxy, read_capped_bytes, read_capped_text,
};
pub use proxy::ProxyConfig;
