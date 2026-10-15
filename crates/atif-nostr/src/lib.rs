//! NIP-ATIF exchange. Trajectories remain observations, never effect authority.

pub mod carriage;
pub mod cli;
pub mod relay;
mod trajectory;

pub use trajectory::{
    Form, Manifest, Options, Package, Verification, Verified, check_child, check_previous, prepare,
    verify, verify_bytes,
};

pub type Result<T> = std::result::Result<T, String>;
pub const MAX_BYTES: usize = 128 * 1024 * 1024;
pub const MAX_CHUNKS: usize = 4_096;
pub const MAX_TEXT: usize = 32_768;
pub const MANIFEST_SCHEMA: &str = "openagents.atif-manifest.v1";
pub const CHUNK_SCHEMA: &str = "openagents.atif-chunk.v1";

#[cfg(test)]
mod tests;
