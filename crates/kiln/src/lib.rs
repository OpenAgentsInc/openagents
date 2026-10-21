//! Kiln, a tech demo of a coding agent that runs entirely in a browser tab.
//!
//! - [`vfs`]: the workspace filesystem, persisted to OPFS by the page.
//! - [`wasi`]: a WASI preview 1 host over that filesystem; the page runs
//!   `wasm32-wasip1` tools in Web Workers through it.
//! - [`agent`]: a Microcoder-style loop: the model returns JSON commands
//!   and the host runs them as tools.
//! - [`cj`]: a NIP-CJ buyer that asks Pylons over the relay with a key the
//!   page generates.
//! - `web` (wasm32 only): the page, the worker, OPFS, and the relay socket.
//!
//! Read `docs/kiln/README.md`.

pub mod agent;
pub mod cj;
pub mod vfs;
pub mod wasi;

/// The SHA-256 of `bytes` as lowercase hex: how tools are named.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(target_arch = "wasm32")]
mod web;
